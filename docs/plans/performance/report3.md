# Performance Report 3: TT pre-probe before make/unmake

Initiative: `performance`, backlog **item 2** (report §10; measured in §7.3 of
[`docs/performance/performance_report1.md`](../../performance/performance_report1.md)).
Plan: [`plan3.md`](plan3.md). Execution date: 2026-10-28 (resumed after a
plan-session/execution crash; no product code was lost).

**Verdict: ADOPT (GO).** All six pre-registered bars pass. The transposition-
dense `dec10` cell is **0.9461** (bar ≤ 0.96), every other deep cell is between
**0.9639 and 0.9818** (bar ≤ 0.99), and the two latency-floor cells are **1.0000**
with the same-filesystem, 151-rep guard (bar ≤ 1.05). Every corpus cell is
`child_evals`-, stdout-, outcome- and PV-identical to the pristine baseline; the
`m22` golden passes without regoldening; the `dec10` portfolio anchor is exact;
the quick suite is 59/59 eval-identical.

## What shipped

- `Position::repetition_key_after` / `Position::keys_after` (`pub(crate)`,
  non-mutating): the board-only repetition key, full TT key and raw halfmove
  clock after a move, without playing it.
- `evaluate_all_children`'s existing TT-prefetch pre-pass now stores each
  child's `(tt_key, repetition_key, rule50)` triple in `Search::keys_scratch`
  and passes it to `evaluate_child_keyed`; the prefetch and the keys share the
  same non-mutating work.
- The pre-probe ladder: path repetition first, then one TT probe, then either a
  solved entry with no best-move hint, an outcome-less entry's reused bounds
  (under the shipped degeneracy rule), or the make/unmake path with the already
  read entry passed in (one probe per child, never two).
- Hinted solved entries fall through to the played path because the one-ply
  best-move guard needs the child board. That path now returns the
  authoritative solved entry **before** terminal classification/existence query,
  and only falls through to the shipped neutral result when the guard fires.
- Unit tests: `keys_after` triple equality over the `hash_after` fixture set;
  live-unsolved reuse; solved-entry proof-event identity; repetition, rule50,
  degenerate-bounds and best-move-guard fixtures; the per-run test-only skip
  counter.
- Docs: `children.rs` module/type/function docs, `core.rs` store-site notes,
  one `AGENTS.md` clause.

## Phase 0 — baseline, after, census

Corpus and flags are plan3's (`--tt-size 256 --first-outcome --outcome-only`
plus per-case args); 5 runs/case on the current container. Baseline is a
pristine `git archive HEAD` (`9276e53`) build; after is the final tree.

| cell | baseline wall | after wall | evals | RSS base→after | minflt base→after |
|---|---:|---:|---:|---:|---:|
| `mate1` | 0.0007 s | 0.0018 s | 2 | 17→17 MB | 175→171 |
| `easy_kq` | 0.0015 s | 0.0034 s | 378 | 56→60 MB | 469→365 |
| `dec13` | 0.5309 s | 0.5126 s | 5,030,497 | 226→226 MB | 1387→1377 |
| `dec10` | 0.3532 s | 0.3433 s | 3,901,847 | 226→228 MB | 1340→1336 |
| `m22` | 1.4470 s | 1.3959 s | 15,023,911 | 228→229 MB | 1471→1465 |
| `m22_b` | 0.9588 s | 0.9455 s | 10,000,032 | 228→228 MB | 1455→1448 |
| `stress_b` | 0.9104 s | 0.8771 s | 10,000,015 | 228→226 MB | 1395→1386 |
| `rem12` | 7.0584 s | 6.7203 s | 66,604,453 | 228→229 MB | 1467→1461 |

The short-cell single-arm rows are process-start dominated (the baseline lives
on `/tmp`, the after build in the workspace overlay); the authoritative short
comparison is the same-filesystem A/B below.

Census on a copy of the final tree (`census.py`; counters only, no search
decision changed):

| cell | child evals | solved-entry skip | live-unsolved skip | path-rep fallback | guard | rule50 | skip rate |
|---|---:|---:|---:|---:|---:|---:|---:|
| `mate1` | 2 | 0 | 0 | 0 | 0 | 0 | 0.0% |
| `easy_kq` | 378 | 4 | 56 | 2 | 0 | 0 | 15.9% |
| `dec13` | 5,030,497 | 75,328 | 349,008 | 1,124 | 0 | 0 | 8.4% |
| `dec10` | 3,901,847 | 51,600 | 1,435,215 | 43,726 | 15 | 0 | 38.1% |
| `m22` | 15,023,911 | 115,879 | 2,308,917 | 14,100 | 8 | 0 | 16.1% |
| `m22_b` | 10,000,032 | 62,331 | 1,294,098 | 4,463 | 6 | 0 | 13.6% |
| `stress_b` | 10,000,015 | 28,951 | 1,402,274 | 3,236 | 0 | 0 | 14.3% |
| `rem12` | 66,604,453 | 685,161 | 4,598,868 | 16,746 | 0 | 0 | 7.9% |

The census reproduces the plan-session numbers exactly, including `dec10`
38.1% and `m22_b` 13.6%. It is an **opportunity ceiling**: the implemented
ladder decides the outcome-less subset and hintless solved entries; hinted
solved entries still make the child to evaluate the guard, then reuse the entry
before the terminal query.

## Throughput — authoritative interleaved A/B

`state/ab_wall_final.json` (11–21 reps/case) with counterbalanced arm order;
`state/ab_wall_short_cb.json` for the two sub-2 ms cells at 151 reps with both
binaries on the same filesystem. Per-run `child_evals` and stdout identity is a
hard abort in the driver.

| cell | reps | base median | after median | ratio | paired quartiles (p25/median/p75) |
|---|---:|---:|---:|---:|---|
| `mate1` | 151 | 0.0005 s | 0.0005 s | **1.0000** | 1.0000 / 1.0000 / 1.0000 |
| `easy_kq` | 151 | 0.0011 s | 0.0011 s | **1.0000** | 0.9167 / 1.0000 / 1.1000 |
| `dec13` | 15 | 0.5215 s | 0.5120 s | **0.9818** | 0.9747 / 0.9825 / 0.9865 |
| `dec10` | 21 | 0.3528 s | 0.3338 s | **0.9461** | 0.9389 / 0.9472 / 0.9659 |
| `m22` | 15 | 1.4587 s | 1.4060 s | **0.9639** | 0.9350 / 0.9637 / 0.9707 |
| `m22_b` | 15 | 0.9616 s | 0.9279 s | **0.9650** | 0.9442 / 0.9656 / 0.9753 |
| `stress_b` | 15 | 0.9101 s | 0.8801 s | **0.9670** | 0.9608 / 0.9686 / 0.9754 |
| `rem12` | 11 | 6.9315 s | 6.7329 s | **0.9713** | 0.9675 / 0.9728 / 0.9762 |

No deep/budget cell regresses; the maximum deep ratio is `dec10`'s 0.9461 and
the maximum non-dec10 deep ratio is `dec13`'s 0.9818. The short-cell guard is
1.0000. Peak RSS and minor faults are flat within run noise.

## Identity evidence

- Every A/B run asserted equal `child_evals` and stdout across arms and
  repetitions; no abort fired. The single-arm corpus has the same `child_evals`
  and stdout as the baseline for all eight cells.
- `test_trajectory_golden` (`m22` default mode) passes **without regoldening**.
- `portfolio_smoke` reproduces the `dec10` anchor exactly at **3,901,847**
  child evals, with identical outcome/PV.
- Quick suite (`benchmark --suite quick --first-outcome --timeout 3 --json`):
  **59/59** eval-identical to `measurements/plan2/state/quick_after.json`,
  **42,109,768** total child evals in both, 0 wrong, 0 timeouts.
- `state/quick_after.json` is the final-tree run.

## Equivalence as implemented

The ladder is the shipped order with make/unmake hoisted out:

1. A non-expired path repetition is a Draw with `repetition_seen = true` and
   precedes the TT probe, as shipped.
2. A rule50-expired child goes to the played path's rule50 branch before any TT
   use, as shipped.
3. Otherwise one TT probe decides:
   - a solved entry with `best_move == NONE` (depth guards included) returns the
     solved child;
   - an `outcome == None` entry returns reused bounds under the shipped
     degeneracy rule (`pn > 0 && dn > 0 && remaining_depth != u32::MAX &&
     remaining_depth <= child_max_depth`), else `(1, 1)`.
4. Everything else takes the played path with the pre-probe's entry, so exactly
   one probe per child.

The terminal classification may be skipped for two distinct reasons:

- **Outcome-less entries** were written only by non-terminal store sites
  (`core.rs`'s depth-0 leaf store and GHI-draw-suppressed store; the snapshot
  import preserves the shape), so the child has legal moves, `rule50 < 100` and
  `occupied > 2`.
- **Solved entries** carry an outcome computed for this exact key; terminality
  is position-static, so replaying the terminal check cannot change the result.
  The terminal store is an authoritative solved entry, not an outcome-less one.

The plan's D-d paragraph stated the invariant too broadly (it said no live entry
can belong to a terminal position; `core.rs`'s terminal store does write one).
The implementation and its docs use the corrected split above. Step 2 remains
gated on `!rule50_expired` because the shipped order is rule50-before-TT, not
because a rule50 entry cannot exist.

Proof events are unchanged: the fast path emits `NodeProven` with `child_key`
(equal to the played child's `pos.hash()` by the `keys_after` contract) and the
same outcome/depth; Draw emits nothing. The event-stream unit test compares the
two paths.

## Deviations from `plan3.md` and why

1. **The D-c hinted-solved guard formula is not usable.** The plan proposed
   `path_contains(pos.repetition_key_after(entry.best_move))` on the parent.
   `entry.best_move` is a move from the **child**, so applying it to the parent
   hashes an illegal move from the wrong base; the probe session found it fired
   spuriously and diverged within a few thousand child evaluations. The shipped
   guard needs the grandchild repetition key, which upstream can only produce by
   playing the child move. The implementation therefore decides only hintless
   solved entries before the make; hinted solved entries take the played path,
   where the entry is reused before the terminal query.
2. **The played path gained an authoritative-entry shortcut.** Without it the
   `dec13` bar was missed at ~0.992 (201 reps). With it `dec13` reached ~0.989
   at the same rep count, then improved further with (3).
3. **`evaluate_child_played` is `#[inline(always)]`.** The out-of-line fallback
   call added ~1.5–1.7% wall on `dec10`: pre-inline 201-rep ratios were
   `dec10` 0.9642 / `dec13` 0.9893; post-inline 0.9455 / 0.9782. Forcing the
   fallback into the ladder restored the single call depth of the pre-plan
   `evaluate_child`. This is a codegen hint only, not a semantic change.
4. **Measurement driver corrections.** `ab_wall.py` now counterbalances arm
   order per repetition (a fixed base-then-after order systematically penalized
   the after arm under within-pair drift) and the latency-floor cells are
   measured with both binaries on the same filesystem. Both changes are
   recorded in the driver docstring and `state/env.json`.

## Bars

| bar | requirement | result |
|---|---|---|
| 1 | bit-identical corpus, golden unregoldened, `dec10` anchor exact, quick 59/59, new units | **PASS** — all eight cells equal; golden, anchor and quick suite exact |
| 2 | skip counter fires on a forced entry; census per cell, `dec10` ≥ 30% | **PASS** — unit counter > 0; `dec10` 38.1%, all cells recorded |
| 3 | `dec10` ≤ 0.96; each other deep cell ≤ 0.99 | **PASS** — `dec10` 0.9461; other deep `max` 0.9818 |
| 4 | no deep/budget cell > 1.01; short cells ≤ 1.05 | **PASS** — deep `max` 0.9818; short 1.0000 |
| 5 | no CLI/public API/TT/ordering/`ChildInfo`/`ProofEvent`/preflight change | **PASS** — only `pub(crate)` helpers and private state/docs |
| 6 | `make test`, `make test-full`, fmt, clippy, doc | **PASS** — both gates green; fmt/clippy/doc clean |

## Findings and follow-ups

- **The lever is real and class-shaped.** `dec10` (38.1% skip) gains 5.4% wall;
  `rem12` (7.9%) gains 2.9%; `dec13` (8.4%) gains 1.8%. The skip rate remains a
  transposition-density property, not a tree-size property, matching the plan
  session's census.
- **Call depth mattered as much as the algorithmic skip.** The pre-probe alone
  was borderline on `dec10`; forcing the fallback inline recovered ~1.7%. Any
  future split of this hot path should re-check the inlining.
- **The plan's guard formula was a genuine design bug**, caught by trajectory
  divergence. The corrected design necessarily leaves hinted solved entries on
  the played path; the census's solved-entry count is therefore an upper bound
  on what a key-only pre-probe can decide without upstream two-ply hashing.
- **The terminal existence query remains the next cost centre** (report §7.3:
  8.5% + 7.3% of `dec10` CPU). It is out of scope here and still needs its own
  invariant.
- **Recommended next plan: item 3, incremental repetition-context hash.** It is
  bit-identical, measured at ~2–3% on `m22`/`m20` and ~1% on `dec10`, and touches
  the `path_push`/`path_pop` bookkeeping rather than this child-evaluation
  surface. Items 4 (path Bloom) and 6 (repetition-cache hasher) remain behind
  item 3.

## Tools, examples and problems encountered

- Measurements used `driver.py`, `ab_wall.py` and `census.py` under
  `measurements/plan3/`; raw transcripts and `/tmp` build trees are not
  committed (per the measurement layout).
- The execution session was resumed after a crash that occurred during a
  throwaway `/tmp` diagnostic patch. No working-tree product code was lost; the
  measurement files and the implementation diff were present and were
  re-validated from scratch.
- The optional `perf` before/after leaf profile was **not** rerun. The mechanism
  is supported by the census skip rates, the unit skip counter, and the wall
  ratios; the report does not claim a new `perf` attribution.
- No missing tests were identified. The hinted-solved shortcut is covered
  behaviourally by `preprobe_best_move_guard_matches_make_path` and the
  proof-event identity test; it has no separate existence-query counter.

## Unresolved parts / next steps

1. Draft `plan4.md` for backlog item 3 (incremental repetition-context hash).
2. Keep item 4 behind item 3; revisit only if the path bookkeeping makes the
   Bloom-filter invariant cheap.
3. If a future change needs to decide hinted solved entries without the child
   board, that requires an upstream two-ply hash API in `atomic-movegen`
   (backlog item 9), not another local workaround.
