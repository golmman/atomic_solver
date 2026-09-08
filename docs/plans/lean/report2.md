# Lean Report 2 — Hot-path compute: movegen dedup + allocation removal + scorer cost

Implements `docs/plans/lean/plan2.md` (items 3, 6, and 8 of the lean
initiative). All measurements were taken 2026-09-07 on the release build of
this container. Raw outputs: `docs/plans/lean/measurements/plan2/`.

## Summary of changes

- `src/search/ordering.rs`: new `pub(crate) struct ScoreContext` holding the
  per-node scorer invariants (`us`/`them`, `them_commoners` bitboard,
  `them_commoners_count`, `lone_commoner`, `enemy_back_rank`,
  `back_rank_mask`, `nearest`). `score_with_map` builds a context and
  delegates to the new `score_with_context`; the public signatures used by
  examples/tests are unchanged.
- `src/search/dfpn/history.rs`: `sort_moves` takes `&mut self`, a
  `&StateInfo` parameter (the caller's `legal_moves_with_state` output; the
  `populate_state` rebuild is gone), and scores through a single per-node
  `ScoreContext`. The per-sort `Vec<(Move, i32)>` is now a reusable
  `sort_scratch` buffer on `Search`. `move_order_breakdown` uses the same
  context.
- `src/search/dfpn/children.rs`: new `pub(super) struct ChildPrecompute`
  (`MoveList` + `StateInfo`), stored in **per-depth pooled slots** (see
  deviation 2). `evaluate_child` restructures the terminal check into
  cost-ordered fast paths (extinction → repetition (rule50-gated) →
  rule50-terminal → TT-resolved → movegen terminal → unsolved bounds) and
  writes the generated child movegen directly into the frame's slot.
  `evaluate_all_children` fills a caller-owned `Vec<ChildInfo>` and grows
  the slot vector lazily.
- `src/search/dfpn/core.rs`: `dfpn` takes `precomputed:
  Option<ChildPrecompute>` and consumes it instead of regenerating; the
  recursion passes the selected child's slot content (`mem::replace` with a
  fresh empty slot). Per-frame `Vec<ChildInfo>` tables are pooled in
  `child_pool`, slots in `precompute_pool`, both indexed by
  `path_stack.len()` after `path_push`, taken with `mem::take` at frame
  start and returned at the single frame exit.
- `src/search/dfpn/mod.rs`: `child_pool`, `precompute_pool`,
  `sort_scratch` fields.
- `src/search/dfpn/children.rs` + `src/search/dfpn/tests.rs`: five new
  tests (see below).
- `AGENTS.md`: dfpn bullet (hot-path reuse) and file-size justification
  for `children.rs` (crossed 20 KB; see step f).

## Step 0 — baseline + profile

`perf` is **not usable in this container** (no hardware counters;
`perf record` fails to open events, `perf stat` reports nothing) and
valgrind/callgrind are not installed. *(Superseded 2026-09-08 — see the
addendum at the end of this report: software-event `perf record/stat` now
work; hardware counters remain unavailable.)* Per the plan's fallback, a
poor-man's sampling profiler was used instead: attach `gdb -batch -ex bt` to
the running process ~6-7×/s and aggregate leaf frames (55-61 samples per run;
`perf_before_gdb_samples.txt`, script preserved at
`/tmp/opencode/sample_prof.sh`).

m22_white, `--timeout 20 --first-outcome`, baseline binary:

| symbol (anywhere in stack / leaf) | % of samples |
| --- | --- |
| `generate_legal_with_state` (leaf) | 36.4% (49.1% incl. callers) |
| `evaluate_child` (leaf, incl. inlined movegen fragments) | 21.8% (74.5% incl. callers) |
| `Board::legal` (legality filter, leaf) | 12.7% |
| `dfpn` (leaf) | 12.7% |
| `Board::populate_state` | 7.3% |
| `score_with_map` | not visible (0%) |
| `malloc` / `free` | not visible (0%) |
| `emit_proof_node` | 0% |

Conclusions that steered the work: movegen dominates (~50%), so item #3 is
the lever; `populate_state` ~7% (step a); `score_with_map` far below the
10% gate → **step d skipped**; malloc invisible at baseline.

Deterministic baselines: quick suite 59 cases, **38,714,551**
`child_evals` total (matches report1); m22 first-outcome wall 12.375 s,
final chunk `elapsed=10.738s nodes=2019863`; m22 chunk `work_done`/`nodes`
sequence recorded in `m22_before.log`.

## Drift checks (after every step)

- `benchmark --suite quick --json --first-outcome`: 59/59 cases with
  identical `child_evals` and `pv_len`; total 38,714,551 before and after
  (`quick_before.json` vs `quick_after.json`).
- m22 `--first-outcome`: stdout byte-identical; `work_done` and `nodes`
  chunk sequences identical.
- m22 default mode (`--timeout 20 --outcome-only`): chunk `work_done` and
  `nodes` sequences **bit-identical** to the pre-change binary (verified by
  rebuilding the baseline via `git stash` after a first divergence was
  found — see deviation 4), stdout byte-identical to plan1's stored
  artifact (win, 23-ply PV).
- GHI/repetition/transposition suites
  (`tests/test_ghi.rs`, `tests/test_repetition.rs`,
  `tests/test_transpositions.rs`): all pass.

## Stepwise measurements (m22_white, first-outcome, this container)

| step | wall (incl. startup + dump) | quick-suite drift |
| --- | --- | --- |
| baseline | 12.375 s (chunk-6 phase 10.738 s) | — |
| a (StateInfo into `sort_moves` + `ScoreContext`) | 12.42 s (noise-level) | 59/59 identical |
| b as planned (boxed precompute) | **13.85 s — regression** | 59/59 identical |
| b+c revised (pooled slots, pools, scratch) | 11.87–11.96 s (chunk-6 phase 10.30 s) | 59/59 identical |

First-outcome wall improved **~4%** (12.38 → 11.87 s; chunk-6 phase
10.74 → 10.30 s). Default mode is trajectory-identical; wall is dominated
by machine variance across days (plan1's host measured `nps≈233k`, this
container `nps≈200k` on identical work), so no default-mode wall claim is
made.

The profile after b+c confirms the mechanism: `malloc`/`free`/`memset`/
`memcpy` all at **0%** of samples (baseline had the per-eval
`MoveList::new()` memset plus, in the boxed intermediate, 11.5% malloc +
6.6% memset + 1.6% memcpy), `populate_state` down from 7.3% to 1.9%, and
movegen remains dominant (~51%) because the per-evaluated-child generation
for the terminal check is irreducible inside `atomic_solver` (an upstream
`has_legal_move` fast path would be needed).

## New tests

In `src/search/dfpn/children.rs`:

1. `evaluate_child_fills_slot_for_searchable_child` — a non-terminal,
   TT-less child fills its slot with the exact move list the child
   position generates.
2. `evaluate_child_tt_resolved_child_leaves_slot_untouched` — TT-resolved
   children never generate; the slot stays empty.
3. `evaluate_child_extinction_terminal_skips_movegen` — extinction
   terminals are detected before movegen; slot untouched.
4. `evaluate_child_rule50_expired_repetition_is_not_repetition_seen` —
   locks the rule50∧repetition semantics fixed in deviation 4 (verified to
   FAIL if the gating is removed).
5. `evaluate_child_rule50_checkmate_is_loss` — a rule50-expired checkmate
   child (reached via Qb3-b2, clock 99→100) is a Loss; moves-empty
   outranks rule50 in the fast path.
6. `dfpn_recursion_consumes_precompute` — solving dec44 twice with fresh
   `Search` objects yields identical counters.

In `src/search/dfpn/tests.rs`:

7. `solve_twice_identical_counts` — dec44 twice, identical `nodes` and
   `child_evals` (catches pool/scratch state leaking between runs).
8. `default_refine_cap_leaves_improving_rounds` — with the plan1 default
   factor (0.25), dec44 still refines (≥ 1 round) to the converged 48-ply
   PV, locking that the default cap does not bind on the quick fixtures.

Full tier (`make test-full`): 283 passed, 0 failed, 0 ignored.

## Deviations from the plan

1. **`perf` unavailable** — replaced by the gdb sampling profiler (the
   plan's documented fallback). Percentages are from 55-61 samples per run,
   so ±2-3% granularity.
2. **Boxed `ChildPrecompute` replaced by pooled per-depth slots.** The plan
   specified `ChildInfo.precompute: Option<Box<ChildPrecompute>>`. Measured
   on m22: a **regression** (12.38 → 13.85 s). Root cause: there are ~19×
   more child *evaluations* (37.5 M) than child *searches* (dfpn entries,
   ~2 M), so the dedup only pays for searched children while the box churn
   (1 KiB alloc + copy + free per evaluated child) applies to all of them —
   the plan's "one alloc replaces a movegen (~10× more expensive)" premise
   holds only for searched children. The plan's own fallback (pooled
   free-list) would still leave ~20 GB of 530 B copies. Final design:
   `precompute_pool[depth][i]` slots written *directly* by
   `evaluate_child`'s movegen (zero alloc, zero copy; the slot's
   `MoveList::clear()` replaces the per-eval 512 B `MoveList::new()`
   memset), consumed by value at the recursion and dropped as plain stack
   locals at the child frame exit. This also subsumes part of step c.
3. **Steps b and c landed together** (one drift check): the slot pool, the
   `children` pool, and `evaluate_all_children` filling a caller-owned vec
   are one coherent restructuring; splitting them would have introduced a
   throwaway intermediate. Step c's proof-event `move_stack.clone()` was
   left unchanged per plan (0% in both profiles, protocol-bound).
4. **The plan's terminal-check reorder was not result-equivalent; fixed.**
   The plan argued "the repetition check keeps its place before any TT
   use" but missed that the repetition key ignores the halfmove clock: a
   **rule50-expired child can repeat a path position**. The old code ran
   `outcome()` before the repetition check, so such a child was always a
   rule50-terminal draw with `repetition_seen = false` and no TT probe; the
   planned order flagged it `repetition_seen = true`, which shifts GHI
   draw-suppression and selection tie-breaks. Found via the bit-identical
   drift protocol: the quick suite and m22 first-outcome were identical,
   but the m22 **default-mode refinement trajectory** diverged by 11 nodes
   (first divergence in refinement round 1, bound 153). Fix: the repetition
   check and TT probe are gated on `rule50 < 100`; rule50-expired children
   go straight to movegen + `outcome_from_state` (always `Some` there),
   restoring the old semantics exactly — the default-mode trajectory is now
   bit-identical. Tests 4 and 5 lock it.
5. **Test-shape change for the precompute assertions.** The plan expected
   `precompute.is_none()` for terminal children; with slots, extinction
   terminals never generate (slot empty) while moves-empty terminals do
   fill the slot (the terminal check needs the move list) but are never
   consumed. Tests 2/3 assert the actual invariants.
6. **`ScoreContext::build` takes the `StateInfo`** and reuses its
   `them_commoners_count` (equal by construction to recounting), so the
   public `score_with_map` signature keeps using its `state` parameter;
   `score_with_context` drops the redundant `state` argument the plan's
   sketch carried.

## Problems encountered

- The boxed-approach regression (deviation 2) — caught by the stepwise
  wall measurement, root-caused with the sampling profile.
- The rule50∧repetition divergence (deviation 4) — caught by the drift
  protocol, root-caused by enumerating every old-vs-new ordering pair and
  their impossibility arguments (moves-empty and two-piece terminality are
  board-static and thus cannot co-occur with a path repetition; rule50
  terminality is *not* board-static because the repetition key ignores the
  clock).
- `perf`/valgrind unavailable (deviation 1).

## Missing tests

- No automated end-to-end guard that the *default-mode* trajectory stays
  bit-identical (the m22 default run is manual, ~16 s; the drift protocol
  in this report documents it). A cheap proxy (quick suite, first-outcome)
  is covered but would not have caught deviation 4 — that corner only
  manifests in depth-bounded refinement with long clocks.
- The peak-memory bound of the pools (~branching × 530 B × active depth ≈
  3-4 MB on m22-scale searches) is documented, not asserted by a test.
- `benchmark --refine-cap` remains unexposed (out of scope, per plan1).

## Next steps

- plan3: parallelism (initiative item 2), determinism design first.
- Profile-gated leftovers: after b+c, `score_with_context` is 5.7% and
  `populate_state` 1.9% — both below the plan's 10% gate; step d was
  skipped.
- Upstream `atomic-movegen` fast paths (`has_legal_move`, early-exit
  legality): movegen is now ~51% of samples and the dominant remaining
  cost; a cheap "has any legal move" check would remove most of the
  per-evaluated-child generation that this plan could not.
- Items #5/#7/#9/#10 remain open in the initiative.

## Addendum 2026-09-08 — `perf` became usable

Host-side launcher changes (capabilities + seccomp/SELinux flags; details in
the "Profiling in this container" section of `AGENTS.md`) made `perf` usable
for per-process profiling of the container's own processes. Hardware PMU
events (`cycles`, `instructions`) remain unavailable — the guest still has no
virtual PMU — and kernel symbols do not resolve, so `cpu-clock` software
sampling with leaf attribution is the ceiling. The gdb sampling fallback of
deviation 1 is no longer needed.

The report's conclusions were re-verified against a real perf profile of the
same workload (m22_white, `--timeout 6 --first-outcome`, release build; a
single run now produces thousands of samples instead of 55-61 gdb samples):

| symbol (leaf) | % of samples |
| --- | --- |
| `generate_legal_with_state` | 46.2% |
| `evaluate_child` | 23.2% |
| `Board::legal` | 12.1% |
| `dfpn` | 5.0% |
| `do_move` | 3.3% |
| `score_with_context` | 2.9% |
| `populate_state` | 1.8% |
| `malloc`/`free`/`memset` | 0% (memcpy 0.35%, from `sort_moves`) |

This confirms the post-change profile above (movegen dominant, allocation
removal effective, `score_with_context` and `populate_state` below the 10%
gate) and validates the gdb-based measurements despite their small sample
count. The percentages also sharpen the "upstream `has_legal_move` fast path"
next step: `generate_legal_with_state` + `Board::legal` together are ~58% of
samples.
