# Lean Report 3 — Upstream `has_legal_move` fast path: terminal-check movegen elimination

Implements `docs/plans/lean/plan3.md` (Phase 2 in this repo; Phase 1 in the
`atomic-movegen` crate). Everything is behavior-preserving: identical
outcomes, identical move ordering, identical `child_evals` per position.

**Headline:** m22_white first-outcome wall **9.889 s → 5.317 s (−46%)** on
this container, with the full drift protocol bit-identical
(quick suite 59/59 `child_evals` total 38,714,551; m22 first-outcome stdout
byte-identical; m22 default-mode chunk trajectory bit-identical).

## Summary of changes

- `Cargo.toml`: `atomic-movegen` `"2.1.0"` → `"2.2"` (2.2.0 was already
  published on crates.io before Phase 2 started; the plan's
  `[patch.crates-io]` interim was never needed). `Cargo.lock` regenerated.
- `src/position.rs`: new `Position::has_legal_move(&self, state: &StateInfo)
  -> bool` delegating to `atomic_movegen::movegen::has_legal_move_with_state`,
  placed next to `legal_moves_with_state`.
- `src/search/dfpn/children.rs`:
  - The terminal check in `evaluate_child` no longer generates a move list.
    The final `else` branch runs `populate_state` + the existence query and
    classifies: no legal move ⇒ checkmate (`Loss`) vs stalemate (`Draw`) by
    the checkers bit; legal moves and `occupied == 2` ⇒ `Draw` (the
    board-static branch of `outcome_from_state`, applied explicitly after
    the existence check to preserve precedence); otherwise fall through to
    the unchanged unsolved-bounds TT path. The `rule50_expired` branch is
    rewired the same way (any legal move ⇒ rule50 draw, which outranks
    `occupied == 2`; no legal move ⇒ mate/stalemate).
  - `evaluate_child` lost its `slot` parameter, `evaluate_all_children` its
    `slots` parameter: the parent no longer pre-fills anything.
  - `ChildPrecompute` documentation rewritten for the frame-local slot
    lifecycle (one slot per active depth; gained a `Default` impl for
    `mem::take`).
- `src/search/dfpn/core.rs`: `dfpn` lost its `precomputed` parameter. The
  entry takes its slot from the flat `precompute_pool` at
  `path_stack.len() + 1` (the depth the frame occupies after `path_push`),
  clears and regenerates into it, and returns it at every exit (the four
  early returns and the normal frame exit); early-exit frames that skip the
  restore simply drop the slot contents, and the next frame at that depth
  regenerates — correctness never depends on pooled contents.
- `src/search/dfpn/mod.rs`: `precompute_pool` changed from
  `Vec<Vec<ChildPrecompute>>` (per-child slots, branching-scaled) to
  `Vec<ChildPrecompute>` (one slot per active depth) — strictly less memory
  than the plan2 pool.
- `AGENTS.md`: the dfpn bullet now describes the existence-query terminal
  check and the frame-local slot flow; the file-size justification for
  `children.rs` updated.
- New tests: see "New tests" below.

Phase 1 outcome is recorded in
`docs/plans/movegen/report_update_2.2.0.md`: `atomic-movegen` 2.2.0
published 2026-09-08 with `has_legal_move`/`has_legal_move_with_state` and
the order-pinned `for_each_pseudo_legal` refactor, gated by an order golden,
a ~8,600-position equivalence property test, perft, and query benchmarks
(1.0× on no-legal-move positions up to 9.9× on quiet middlegames). No
deviations from the handoff spec; the optional fast pre-pass was not added
(the closure stream already reaches 8.9 ns).

## Step 0 — baseline + profile

- Quick suite: 59/59 cases, total `child_evals` **38,714,551** — matches the
  plan2 artifact exactly (`quick_before.json`).
- m22_white first-outcome (`--timeout 20 --first-outcome --outcome-only`):
  final chunk `elapsed=8.135s nodes=2019863 nps=248306`; stdout identical to
  plan2's stored artifact (win, 155-ply PV). Wall 9.494 s via `cargo run`;
  re-timed later with the stashed baseline binary invoked directly:
  **9.889 s** (`m22_before_fo_wall.txt`).
- m22_white default mode (`--timeout 20 --outcome-only`): wall 12.704 s,
  stdout identical to plan1's stored artifact (win, 23-ply PV), chunk
  `work_done`/`nodes` sequence bit-identical to plan2's post-plan2 artifact.
- Before-profile (`perf record -e cpu-clock -g`, `perf report --no-children`,
  leaf attribution), `perf_before_fo.txt`:

  | symbol (leaf) | % of samples |
  | --- | --- |
  | `generate_legal_with_state` | 41.72% |
  | `evaluate_child` | 23.11% |
  | `Board::legal` | 13.83% |
  | `dfpn` | 6.11% |
  | `Position::do_move` | 3.71% |
  | `score_with_context` | 3.17% |
  | `Board::populate_state` | 2.19% |
  | `vdso` (`Instant::now`) | 1.61% |
  | `Board::undo_move` | 1.22% |

  Movegen-related leaves (`generate_legal_with_state` + `Board::legal` +
  `populate_state`) ≈ **57.7%**, confirming the plan's sizing.

## Drift checks (after every step)

- **Step a** (dep bump + `Position::has_legal_move` wrapper, no call-site
  change yet): quick suite 59/59 identical, total 38,714,551.
- **Step b** (terminal-check rewiring; recursion temporarily passes `None`
  so each searched child regenerates at its own frame): quick suite
  identical; m22 first-outcome stdout byte-identical, chunk sequence
  identical.
- **Step c** (frame-local slot plumbing): quick suite identical
  (38,714,551); m22 first-outcome stdout byte-identical + chunk sequence
  identical; m22 **default-mode** stdout identical and chunk
  `work_done`/`nodes` trajectory **bit-identical** (the check that caught
  plan2's deviation 4).
- Final re-check after clippy/fmt (the `rule50` branch was merged to a
  single `||` condition): all of the above re-verified.
- `tests/test_ghi.rs`, `tests/test_repetition.rs`,
  `tests/test_transpositions.rs`: green.
- `cargo test --release` (fast tier) green; `make test-full` green
  (~9 min wall on this container, including the 60 s stress suites).

Raw outputs: `docs/plans/lean/measurements/plan3/` (`quick_before.json`,
`quick_after.json`, `m22_before_fo_*`, `m22_before_default_*`,
`m22_after_fo_*`, `*_chunks.txt`, `perf_before_fo.txt`, `perf_after_fo.txt`).

## Stepwise measurements (m22_white, first-outcome, this container)

| step | wall (direct binary) | quick-suite drift |
| --- | --- | --- |
| step 0 baseline | 9.889 s | — (38,714,551) |
| step a (wrapper) | not measured | identical |
| step b (rewiring) | not measured | identical |
| step c (plumbing) | 5.317 s | identical |

- Final chunk: `elapsed=4.651s nodes=2019863 nps=434316` — same node count,
  **1.75× the baseline search nps** (248,306 → 434,316).
- After-profile (`perf_after_fo.txt`):

  | symbol (leaf) | % of samples | before |
  | --- | --- | --- |
  | `evaluate_child` | 38.81% | 23.11% |
  | `Board::populate_state` | 11.98% | 2.19% |
  | `dfpn` | 11.65% | 6.11% |
  | `Position::do_move` | 11.15% | 3.71% |
  | `score_with_context` | 5.43% | 3.17% |
  | `has_legal_move_with_state` | 4.06% | — |
  | `Board::legal` | 3.97% | 13.83% |
  | `Board::undo_move` | 2.60% | 1.22% |
  | `vdso` (`Instant::now`) | 2.44% | 1.61% |
  | `generate_legal_with_state` | 2.42% | 41.72% |

- `generate_legal_with_state` collapsed from 41.7% to 2.4% (only the
  searched-children generation at `dfpn` entries remains);
  `has_legal_move_with_state` lands at 4.1% leaf. Aggregate
  movegen-related leaf share went **57.7% → 22.4%** of a much smaller total.
  The plan's "double-digit-percent improvement" target is met: **−46% wall**.

## New tests

- `evaluate_child_checkmate_child_classifies_loss_without_movegen` and
  `evaluate_child_stalemate_child_classifies_draw_without_movegen` — the
  boolean path's classification (checkers bit) for no-legal-move children.
- `evaluate_child_two_commoners_is_draw` — guards the `occupied == 2`
  precedence (K vs K child with legal moves and `rule50 < 100` is a Draw).
- Plan2's `evaluate_child_rule50_expired_repetition_is_not_repetition_seen`
  and `evaluate_child_rule50_checkmate_is_loss` kept unchanged and still
  green — they lock the semantics this plan had to preserve.
- `dfpn_entry_generates_into_pooled_slot` — after a bounded depth-2 search
  the root frame's slot covers exactly the root position's legal moves
  (order differs: the frame sorts in place), depth 0 stays empty, and a
  searched child's frame filled its own slot; replaces the plan2
  parent-pre-fill slot tests.
- `dfpn_frame_local_slots_deterministic` (renamed from
  `dfpn_recursion_consumes_precompute`) — double-solve identical counters,
  catches pool state leaking through the new slot lifecycle.
- `precompute_pool_stays_within_active_depth_bound` — after a full solve,
  pooled movegen storage stays under 1 MB (the plan2-era per-child pool
  documented ~3–4 MB on m22 scale; the flat pool is one slot per depth).
- `tests/test_lean3.rs::m22_default_mode_trajectory_matches_golden`
  (`#[ignore = "slow: ..."]`) — the default-mode trajectory golden plan2's
  report called out as missing: solves m22_white in default mode via the
  CLI (`--timeout 20 --outcome-only`) and asserts stdout equals
  `tests/fixtures/m22_default_stdout_golden.txt`, derived from the
  drift-verified post-plan3 binary. Note the run takes ~7 s now — the
  solver exhausts the refinement bounded tree before the 20 s deadline.

## Deviations from the plan

1. **`populate_state` exceeded the 10% gate (Decision 5).** Decision 5
   predicted the double-populate trade would keep `populate_state` below
   10% of samples; it lands at **11.98%** (up from 2.19%). The absolute
   cost grew because the existence query needs a populated `StateInfo` for
   *every* evaluated child (~19× the node count), not only for searched
   children, and the denominator shrank. The aggregate movegen share
   (22.4%) and the wall result (−46%) are strong enough that I did not
   re-open the upstream API for this; see Next steps.
2. **Step-b intermediate state.** Step b's drift check ran with the
   recursion passing `None` (regeneration at the child frame) and the slot
   parameters still present-but-unused, exactly as Decision 4 implies;
   plan2's slot-content tests were updated at step b and replaced by the
   frame-local lifecycle tests at step c, so the tree was test-green at
   both checkpoints rather than only at the end.
3. **Pool memory assertion shape.** Step e suggested asserting
   `precompute_pool.len()` against "branching × active depth". With the
   flat pool that bound is vacuous, and `max_depth_reached` resets per work
   chunk so it cannot bound the pool (the unbounded first-outcome phase
   reaches deeper recursion than the last chunk reports). The test instead
   asserts the meaningful quantity: pooled bytes < 1 MB after a full solve.
4. **`ChildPrecompute: Default`.** Added so `mem::take` can empty a pool
   entry; the plan said the type "survives unchanged", and the trait impl
   is the only API change.
5. **`[patch.crates-io]` interim unused.** 2.2.0 was published before Phase
   2 began, so the requirement went straight to `"2.2"` (Decision 6's
   contingency was not needed).

## Problems encountered

- The root-slot equality test initially compared the slot's move list
  against a fresh generation and failed on order: `dfpn` sorts the list in
  place (`sort_moves`), so the slot holds the *sorted* list. The test now
  compares as sets and documents why.
- `max_depth_reached` is per-chunk state (reset in `reset_search_state`),
  which makes it useless as a recursion-depth bound for pool assertions;
  see deviation 3.
- `perf report` leaf attribution after the change shows `evaluate_child`
  at 38.8% own-frame — that number includes partially-inlined fragments of
  the existence-query path (LTO splits attribution unpredictably); treat
  it as "the child-evaluation loop" rather than a single optimization
  target.

## Missing tests

- No solver-level property test that `Position::has_legal_move` agrees with
  `legal_moves_with_state` + `outcome_from_state` over random playouts;
  correctness is inherited from the upstream equivalence test (~8,600
  positions) plus the deterministic fixtures here. A cheap playout-based
  cross-check in `tests/` would lock the *wrapper* too.
- The trajectory golden is wall-clock-dependent by construction: it passed
  on this container across repeated runs, but a sufficiently different
  machine could exhaust refinement bounds at different chunk boundaries.
  The golden stores only stdout (outcome + PV), which has been stable, but
  this is an accepted risk, not a proof.
- No automated guard that `has_legal_move`'s share stays small (perf
  regression checks remain manual).

## Additional tools/examples used

- `perf record -e cpu-clock -g` / `perf report --stdio --no-children`
  (per AGENTS.md profiling setup; software `cpu-clock` events only).
- `benchmark --suite quick --json --first-outcome` for the drift metric.
- `git stash` round-trip to re-time the pristine baseline binary directly
  (both before-profiles and the honest wall delta).
- No new dependencies; the upstream 2.2.0 diff was developed in the
  movegen repo per the handoff spec and is recorded in
  `docs/plans/movegen/report_update_2.2.0.md`.

## Next steps

- **plan4 — parallelism (#2)**, now unblocked; still needs the
  determinism story for the `child_eval_budget` contract.
- If the remaining movegen share (22.4%, of which `populate_state` ~12%)
  justifies another upstream round, the lever is the out-of-scope item from
  this plan: a resumable generator (generate-once-with-early-exit *and*
  reuse for the searched child) or a state-returning existence query that
  lets `evaluate_child` hand its populated `StateInfo` to the recursion,
  eliminating the double populate for searched children.
- `score_with_context` (5.4%) is back above the noise floor — item #8
  (cheaper static scoring) is the next solver-side candidate.
