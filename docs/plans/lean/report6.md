# Lean Report 6 — micro-wins bundle: #14 wrapper slimming, #13 clock sampling, #12a TT non-terminal skip (+ #17 sizing spike)

Implements `docs/plans/lean/plan6.md`. Three wall-time-only levers with **no
search-semantics change**, validated by the full drift protocol, plus the
phase-0 #17 sizing spike (measurement only, reverted). Each lever is
independently revertible; no AGENTS.md change was made.

## Summary of results

| lever | mechanism | measured result |
| --- | --- | --- |
| #17 (spike, not implemented) | path-membership scan sizing | ~1.0% wall m22, ≤ ~2.2% shuffle-win → **below bar, demoted** |
| #14 (anchor) | `do_move_with_scratch`/`undo_move_with_scratch` over a pooled dirty `StateInfo` | wrapper round-trip **12.2 → ~7.0 ns** (target ≤ 8.5) |
| #13 | `Instant::now()` sampled every 4096 `dfpn` entries | clock leaves gone from the profile table (~2.2% → <0.1%) |
| #12a | skip existence check on `outcome == None` TT hits | existence cluster **20.7% → 7.5%** of the profile pie |
| **bundle** | — | **m22 first-outcome −7.1% wall, shuffle-win −5.9% wall**, all trajectories bit/byte-identical |

## Phase 0 — #17 sizing spike (reverted)

Temporary `Cell<u64>` counters on `Search` (plus a temporary accessor and a
temporary stderr print in `main.rs`), measured on both validation cases, then
**completely reverted** — the final diff contains no trace (`git diff` scope
check below).

Raw instrumented runs (`measurements/plan6/m22_spike.err`,
`shuffle_spike.err`):

| case | child_evals | `path_contains` calls | scanned elements | mean / max scan | repeat-guard calls | hits |
| --- | --- | --- | --- | --- | --- | --- |
| m22_white first-outcome (3.23 s) | 14,158,593 | 14,989,706 | 141,450,218 | 9.4 / 107 | 113,005 | 7 (0.006%) |
| shuffle-win first-outcome (77.4 s) | 351,297,052 | 371,253,625 | 3,434,503,881 | 9.3 / 406 | 2,041,489 | 40 (0.002%) |

(`path_contains` calls ≈ child evals + `dfpn` entries — 14.2M + 0.83M and
351M + 20M respectively.)

The plan's "~1 ns/element" heuristic is too pessimistic: a calibration
micro-benchmark (`measurements/plan6/contains_bench.rs`, `rustc -O`,
miss path, matching scan lengths) shows `slice::contains` on `u64` is
vectorized — **2.15 ns/call at length 9 (0.24 ns/element), 4.5 ns/call at
length 40, 10.1 ns/call at length 100** — i.e. short scans are dominated by
per-call overhead, not by scan length.

Wall estimate:

- m22: 15.0M calls × ~2.2 ns ≈ **0.033 s ≈ 1.0%** of 3.24 s.
- shuffle-win: 371M calls × 2.2–4.5 ns ≈ **0.8–1.7 s ≈ 1.1–2.2%** of 77.4 s
  (upper bound assumes every scan pays the deep-path per-call cost).
- `best_move_repeats_path` (full do/undo + scan per TT-resolved hit):
  ≤ 2.1M × ~13 ns ≈ **≤ 0.03%** — negligible; hit rate ~0.01% means the
  guard almost never fires.

**Verdict per the plan's decision rule: below the ~3% bar on both cases →
#17 demoted to "spiked, below bar"** in the backlog row with these numbers;
no #17 implementation plan. Nothing else promoted or demoted from this
spike.

## Phase 1 — #14 solver-side wrapper slimming (anchor)

- `src/position.rs`: new `pub(crate)` pair
  `do_move_with_scratch` / `undo_move_with_scratch` — same body as
  `do_move`/`undo_move` minus the fresh `StateInfo::new()` zeroing and the
  undo-stack push/pop. The documented soundness contract mirrors the
  upstream argument from the initiative spike: `Board::do_move` writes every
  field `Board::undo_move` reads (`castling_rights`, `ep_square`, `rule50`,
  `captured_count`, `captured[..count]`, `cap_sq`/`cap_piece`/`cap_pt`,
  `hash`), so stale bytes are safe; pooled slots are initialized once with
  `StateInfo::new()`, so no field is ever read unwritten. The caller must
  strictly pair the calls around the move and must not consume
  `checkers`/`pinned` in between; the untouched undo stack keeps regular
  do/undo pairs (e.g. `best_move_repeats_path`) and `Position::clone` legal
  inside the scratch pair.
- `src/search/dfpn/children.rs` + `mod.rs`: `evaluate_child`'s make/unmake
  pair now runs on a per-depth pooled scratch slot
  (`Search::eval_state_pool`, indexed by frame depth, taken/returned with
  `mem::take` like the other pools). The slot-staleness invariant is
  documented next to `ChildPrecompute` (see deviation 1 for why it is a
  sibling pool rather than a field on `ChildPrecompute`).
- Aliasing verified: the scratch slot is distinct from the frame slot's
  `state` (parent movegen/`sort_moves`) and from the local existence-check
  `StateInfo`s inside `evaluate_child`; `evaluate_child` never recurses.
- Unit test `scratch_make_unmake_matches_regular_pair` (module test in
  `position.rs`; no new test file, and no `test_plan6.rs`): quiet move and
  blast capture round-trip to identical board/hash, scratch make equals
  regular make, undo stack stays empty.

### Wrapper micro-benchmark (5M round-trips, m22 root, quiet rook move b1b8, release)

| harness | ns/round-trip |
| --- | --- |
| `Position::do_move` + `undo_move` (this session, same harness) | 12.2 (stable runs 12.21/12.16; two noisy runs 15.7/18.8) |
| `Position::do_move` + `undo_move` (2026-09-09 spike) | 11.9 |
| **scratch pair (after)** | **6.9–7.2** |
| raw `Board` floor (2026-09-09 spike) | 7.6 |

**12.2 → ~7.0 ns (−5.2 ns, −43%) — beats the ≤ 8.5 ns target.** The
re-measured baseline was taken with the same harness/move as the after run so
the delta is same-methodology; the scratch pair landing at the raw-board
floor (below the old spike's 7.6 ns figure) is plausible because that figure
came from a different harness/move. ~5 ns × 14.2M evals ≈ 0.07 s of the m22
wall.

## Phase 2 — #13 clock sampling

`time_exceeded` keeps the `stop_flag`/`memory_limited` atomic loads on every
call and gates only the `Instant::now()` read behind a node-count sampler:
the clock is re-read when `self.nodes` has advanced by
`CLOCK_SAMPLE_INTERVAL = 4096` (constant sits with the other search
tunables) since the last read; otherwise the cached `Cell<Instant>` is
compared to the deadline. Both sampler fields are private `Cell`s (the hot
site takes `&self`); `begin_run` initializes them per run. All call sites are
unchanged; the per-chunk/per-round sites are negligible either way, the hot
per-entry site is what buys the ~2.2% back. Contract notes (deadline
detection delayed by ≤ 4096 entries → coarse `ExitReason::Timeout`
granularity; budget mode untouched — `child_eval_budget_exceeded` is a pure
integer compare and `ExitReason` checks `BudgetExhausted` first) are in the
method doc. `exit_reason()` deliberately keeps a fresh `Instant::now()` read
(non-hot path).

## Phase 3 — #12a TT non-terminal skip

In `evaluate_child`, after the probe and the resolved-entry reuse, the
populate/`has_legal_move`/`occupied == 2` block is skipped when the live
entry has `outcome == None`, falling through to the (unchanged) unsolved-
bounds reuse.

Soundness invariant, documented at the skip site and in the backlog row:
`outcome == None` entries are only stored after the node passed the
entry-level `outcome_from_state` full-movegen check. Verified against every
`None` store site in `core.rs`:

- depth-0 leaf store (`None, 1, 1, 0, 0`): only reached after
  `outcome_from_state` returned `None` at entry ⇒ legal moves, rule50 < 100,
  occupied > 2;
- GHI draw-suppressed stores (`None` with `(1, 1)`): the node was fully
  expanded (same entry-level check) ⇒ invariant covers them too;
- terminal stores always carry `Some` (never hit the skip path; the resolved
  reuse handles them before).

Not keyed off `remaining_depth` shape; the existing degeneracy guard
(`remaining_depth != u32::MAX` etc.) in the bounds reuse is unchanged.
Probes are generation- and key-matched (`tt::probe`), and TT-snapshot-seeded
entries (`tt_mut`, reconstruct side) are outputs of the same store logic, so
they inherit the invariant.

Sharing check (plan phase 3.3): a subsequently *searched* child regenerates
its own movegen state at its `dfpn` entry into its own pooled frame slot
(`pos.legal_moves_with_state(&mut slot.moves, &mut slot.state)`, `core.rs`
entry) — it never depended on the existence-check's `StateInfo`, so no
lazy-populate hand-off was needed. (The skip only avoids populating a local
that was exclusively consumed by the existence query.)

## Drift protocol (the gate) — all green

1. **Baselines captured before any change** (2026-09-12):
   `benchmark --suite quick --json --first-outcome --runs 1`; m22
   first-outcome stdout (`--timeout 30 --outcome-only`); committed
   `tests/fixtures/m22_default_stdout_golden.txt` untouched.
2. **After all three levers**:
   - quick suite: **all 60 cases bit-identical** on `child_evals`, `nodes`,
     `outcome`, `pv_len`, `status`, `wrong` (comparator script diff of the
     two JSON documents);
   - m22 first-outcome stdout **byte-identical** (re-verified after the
     final rebuild);
   - slow-tier lean3 golden **byte-identical**:
     `cargo test --release --test test_lean3 -- --include-ignored` → ok;
   - `dfpn_frame_local_slots_deterministic` green (fast tier).
3. **Wall measurement** (release, same machine, sequential runs):

   | case | baseline | after | Δ |
   | --- | --- | --- | --- |
   | m22 first-outcome `--timeout 30`, 5 runs median | 3.240 s (3.135–3.253) | 3.010 s (2.983–3.092) | **−7.1%** |
   | shuffle-win first-outcome, 1 run | 78.55 s | 73.88 s | **−5.9%** |

   Both stdouts byte-identical; both inside/above the plan's −3–7% band, no
   regression on either case → no bisect needed.
4. **Budget-mode regression**: `cargo test --release --test
   test_decisive_remaining -- --include-ignored` → 5 passed unchanged.
5. **`make test`** green (29/29 test binaries, ~60 s gate), run twice
   (before and after adding the phase-1 unit test). `cargo fmt --check`
   clean; `cargo clippy --release --all-targets` clean.
6. **`git diff` scope check**: `src/position.rs`,
   `src/search/dfpn/{mod,children}.rs`, `docs/plans/lean/` docs only.
   `src/search/dfpn/core.rs` ended up **unchanged** in the final diff (the
   phase-0 instrumentation there was reverted and no lever needed a core
   change). Phase-0 instrumentation is absent from the final diff; no test
   file named `test_plan6.rs` was created.

## Post-plan6 profile (companion task)

`perf record -e cpu-clock -g`, leaf attribution, m22_white first-outcome,
default 128 MB TT; raw table
`measurements/plan6/m22_first_outcome_post_plan6_leaf.txt` (pre-plan6 table
kept alongside). Wall −7.1% vs the post-plan4 baseline run.

| cluster | leaves | post-plan6 (post-plan4) |
| --- | --- | --- |
| child-eval loop | `evaluate_child` (incl. inlined scans) | 26.9% (26.6%) |
| move make/unmake | `do_move` 29.4% + `undo_move` 2.5% | 31.9% (26.9%) |
| frame overhead | `dfpn` | 15.5% (9.9%) |
| static scoring | `score_with_context` | 5.5% (4.3%) |
| existence check | `populate_state` 2.1% + `has_legal_move_with_state` 5.4% | 7.5% (20.7%) |
| full movegen | `Board::legal` 3.1% + `generate_legal_with_state` 0.8% | 3.9% (4.0%) |
| clock sampling | `vdso` | <0.1% (~2.2%) |
| move sort | `sort_moves` sort leaves | ~2.6% (~2.2%) |

Reading (numbers only, no promotions without spikes): the pie shrank ~7% so
untouched clusters' shares grew; #12a over-delivered relative to its 2.5%
ceiling estimate (the skip removes populate + zeroing + existence query on
the ~15% `None`-hit share); #13 removed the clock from the leaf table; what
remains of make/unmake is the measured-tight upstream core. Shares updated
in `initiative.md`.

## Deviations from the plan

1. **Eval-scratch pool structure.** The plan said to "extend the existing
   `ChildPrecompute` pooling". The scratch slot lives in a sibling per-depth
   pool (`Search::eval_state_pool`) rather than as a field of
   `ChildPrecompute`: the frame slot's `state` cannot serve as the
   make/unmake scratch because a later `populate_state` into the same slot
   would clobber the undo fields before `undo_move_with_scratch` reads them,
   and the frame slot is held locally by the `dfpn` frame while
   `evaluate_child` runs. The pooling *approach* (per-depth, `mem::take`,
   initialized once with `StateInfo::new()`, reused dirty) is extended as
   documented next to `ChildPrecompute`.
2. **Micro-benchmark methodology.** The 2026-09-09 spike's exact harness was
   not in-repo; both sides of the 12.2 → 7.0 ns comparison were measured
   this session with the same temp harness (quiet rook move b1b8 from the
   m22 root, 5M round-trips), and the temp example was deleted afterwards.
   The scratch pair's ~7.0 ns sits at the old spike's raw-board floor
   (7.6 ns) — consistent, since that floor came from a different
   harness/move.
3. **#17 ns/element calibration.** The plan's "~1 ns/element" heuristic was
   replaced by a direct calibration micro-benchmark of `slice::contains`
   (vectorized, overhead-dominated for short scans); the wall estimate is
   given as a band, still unambiguously below the bar.
4. `core.rs` is not in the final diff (see scope check above) — the plan
   listed it as an expected potential touch point, not a requirement.

## Problems encountered

- The committed m22 golden (`tests/fixtures/m22_default_stdout_golden.txt`)
  is a default-mode reference only; the first-outcome byte-compare
  baseline had to be captured this session (the plan's prescribed method).
  The golden itself was not touched.
- The wrapper micro-benchmark's baseline timings were bimodal on this host
  (12.2 vs 15.7–18.8 ns in some runs); the scratch pair was consistently
  stable at ~7 ns, and the 12.2 ns stable-mode baseline matches the
  2026-09-09 spike's 11.9 ns, so the delta is trustworthy.
- None blocking.

## Missing tests

- No test asserts that the #12a skip *actually skips* (e.g. a
  populate-call counter); correctness is pinned indirectly by the
  bit-identical trajectories and the existing terminal/bounds unit tests,
  which all exercise the changed branch.
- Clock-sampling granularity has no dedicated test (wall-clock tests use
  generous margins by design; the delay bound is ≤ 4096 entries).
- The scratch-pair-with-nested-regular-pair pattern (as used by
  `best_move_repeats_path` between the scratch make/unmake) is covered by
  full-solve bit-identity but not by a minimal unit test.

## Additional tools/examples used

- `perf record/report` (per AGENTS.md profiling section) for the phase-0
  cross-check and the post-plan6 profile.
- `python3` for the JSON drift comparator, wall-timing arithmetic, and
  report tables.
- `rustc -O` standalone micro-benchmarks (path-scan calibration; temp
  wrapper-round-trip example, deleted after measuring).
- `benchmark` example for the quick-suite drift runs.

## Next steps

- **#2 parallelism determinism spike** remains the named successor
  candidate (the only remaining multiplicative lever; needs the
  `child_eval_budget` determinism design spike first).
- The post-plan6 pie puts `dfpn` frame overhead (15.5%) and the upstream
  make/unmake core (29.4% of a smaller pie) on top; both are bounded by
  measured-tight floors — further movement there is algorithmic
  (`docs/plans/dfpn/initiative.md`), not solver-fat.
