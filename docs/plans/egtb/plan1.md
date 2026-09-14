# Plan 1: Go/no-go spike — men-count histogram + 3-man generator prototype

Initiative: `egtb` backlog #1. This plan is a **measurement-and-prototype
spike**: no search-behavior change, no default-mode trajectory change, no
drift protocol. It gates every other item in the initiative: the "faster
solves" goal is only alive if deep positions actually reach ≤4 men often
enough to matter, and the generator's ruleset semantics must be
cross-validated against the solver before any 4-man work builds on it.

Prerequisite reading: `initiative.md` (Motivation "Why 4 men", Ruleset
grounding, Soundness invariants), `docs/plans/lean/initiative.md` backlog #9
(superseded by this initiative), `docs/plans/move_order/ideas.md` #8
(ordering angle, backlog #4 here).

## Goal

Answer three questions with evidence:

1. **Coverage**: what fraction of child evaluations on the decisive/stress
   suites occur at positions with ≤4 men? This decides go/no-go for the
   initiative's primary goal and, on a go, drives the coverage decision in
   backlog #2.
2. **Semantics**: can a retrograde generator built purely on
   `atomic-movegen 2.2` reproduce the solver's outcomes on the full 3-man
   space (including stalemate-as-Draw and rule50 handling), with zero
   mismatches on sampled cross-validation?
3. **Feasibility shape**: what do per-table size and generation time look
   like at 3 men, extrapolated to 4 men (~30M entries/table)?

## Deliverables

- `examples/men_histogram.rs` — men-count histogram over child evaluations,
  per suite position.
- `examples/egtb_gen3.rs` — 3-man retrograde generator prototype writing a
  raw WDL table plus a human-readable summary.
- A cross-validation runner (may live inside `egtb_gen3.rs` or a third
  example) comparing generator outcomes against `Search` solves on sampled
  3-man FENs.
- `docs/plans/egtb/report1.md` with the go/no-go decision and raw outputs
  under `docs/plans/egtb/measurements/plan1/`.

## Task A: men-count histogram

Implementation sketch (keep it minimal and non-invasive):

- Add to `Search` an always-on histogram: `u64` bins indexed by
  `popcount(occupied)` (men 2..=32), incremented at the top of
  `evaluate_child` (`src/search/dfpn/children.rs`, where the child board's
  occupancy is already available). One `popcount` + one increment is well
  under 1% of a ~59 ns eval round-trip; measure the wall delta on one
  benchmark case anyway and report it.
- Expose `Search::men_histogram()`.
- New example `examples/men_histogram.rs` (using `examples/common.rs`
  helpers) that solves each suite position (`--suite` like the benchmark
  example) and prints per-position and aggregate histograms. Two views:
  - **Per-bin counts and shares** of non-terminal child evals at 2, 3, 4,
    5, 6, 7–8, 9+ men (terminal children are excluded because they are
    already decided in O(1) by the existence-query path — a probe saves
    nothing for them, so only non-terminal evals count as probeable
    work).
  - **Cumulative shares** at ≤4, ≤5, ≤6 men. The marginal value of
    extending coverage by one man is the difference between consecutive
    cumulative numbers; report it explicitly.
  - **Halfmove-clock distribution** for the ≤4-men (and ≤6-men)
    non-terminal evals: share at clock 0 vs. clock > 0, plus a coarse
    bucketing (1–25 / 26–50 / 51–99). This feeds the initiative's open
    decision 2 (DTZ granularity): WDL-only probing is fully usable at
    clock 0, so the nonzero-clock share is the upper bound on what exact
    DTZ could additionally unlock.

Important metric detail: the histogram must count **child evaluations**
(the ~95% of children never searched — these are exactly what a leaf probe
replaces), not searched nodes. Note the distinction in the report.

Run over at least: the `decisive` and `stress` suites (`make stress` class
included) with `--first-outcome`, plus `startpos` for reference.

**Interpretation caveat (goes into the report):** the ≤N-men share is an
*upper bound* on directly replaceable work, not the predicted net saving.
Material is non-increasing in atomic chess, so once a ≤4-men node is
probed, all its descendants (also ≤4 men) vanish from the tree — the raw
share already contains some of this double-counting, in our favor. But
changed search dynamics cut both ways: a solver that can prove more
positions sometimes explores *more* (lines that previously died in unknown
terrain now continue into a proven tablebase win). The net effect is only
measurable as an A/B in backlog #3; the histogram is the sizing estimate,
not the prediction.

**Go/no-go criterion** (record the actual number, then decide): if the
non-terminal ≤4-men share of child evals on the decisive suite is below
~0.1%, the
"faster solves" goal is a **no-go** — the initiative pivots to the
correctness-oracle goal or closes. Between 0.1% and 1% is a judgment call:
document the reasoning against the alternative uses of the same effort
(`lean` #7, #10). Above 1% is an unambiguous go for backlog #2.
The ≤5/≤6 cumulative shares do **not** enter the go/no-go: covering 5 men
means ~64× the entries per material class (hundreds of MB each) and 6 men
another ~100× — a different engineering regime, recorded here only as
intelligence for a potential future pivot.

## Task B: 3-man generator prototype

`examples/egtb_gen3.rs`:

- Enumerate K+x vs K placements (x ∈ {Q, R, B, N, P}) for both side-to-move
  colors: 64·63·62 ≈ 248k placements per table, pawn excluded from ranks
  1/8. Roughly 0.5M entries per material class — trivial to hold in RAM.
- Retrograde fixpoint over `atomic_movegen::board::Board`:
  - base cases from `Board::outcome()` (explosion checkmate, bare kings,
    stalemate-Draw, rule50 excluded — WDL is computed at clock 0, per
    soundness invariant 2);
  - iterate until no outcome changes, resolving into the 2-man base
    constants (KvK = Draw, K+1vK = Win side-to-move-relative; encode as
    constants, verify them against `Board::outcome()` on the enumerated
    positions rather than assuming).
- Write a raw byte-per-entry WDL file plus a summary (Win/Loss/Draw counts,
  positions with no legal moves, maximal mate distance for curiosity only —
  no DTM claim).
- Keep the index scheme and file layout trivially replaceable: this is a
  format sketch, not the plan2 format. Note in the report what plan2 will
  have to change (e.g., block compression, shared king-square indexing).

## Task C: cross-validation (merge gate)

- Sample FENs from the generated 3-man space, stratified by material class
  and outcome; include the edge cases called out in `src/position.rs` tests
  (stalemate draw, no-legal-moves-in-check loss, bare kings) and
  explosion-derived positions (king captured by blast, multi-men explosions
  where reachable at 3 men).
- Solve each sample with the existing `Search` (short timeout,
  first-outcome) and compare outcomes. **Zero mismatches is the gate.**
- Every mismatch is a defect to triage in the report: generator bug,
  solver bug, or convention misunderstanding. A solver-side defect found
  here is the highest-value possible outcome of the spike (ground-truth
  oracle working as intended) but does not block the go/no-go decision
  once triaged.

## Constraints

- No changes to the search's default behavior; the histogram counter is
  observable but inert.
- No storage-format commitment (backlog #2 owns that).
- Files under ~10 KB per the AGENTS.md sizing rule; examples may borrow
  `examples/common.rs` helpers.

## Validation

```bash
cargo fmt
cargo clippy --all-targets
cargo test
cargo run --release --example men_histogram -- --suite decisive
cargo run --release --example men_histogram -- --suite stress   # if exposed
cargo run --release --example egtb_gen3 -- --material q --out /tmp/kq3.bin
# + cross-validation run over the sampled FENs
```

Primary metrics (in priority order):
1. Cross-validation mismatch count (must be 0, or fully triaged).
2. Non-terminal ≤4-men child-eval share per suite position, plus
   cumulative ≤5/≤6 shares and the ≤4/≤6-men clock distribution
   (go/no-go + coverage/DTZ intelligence).
3. Wall-time delta of the histogram counter on one benchmark case (≤1%).
4. 3-man generation time and table size (extrapolation input for #2).

## Final task

Write `docs/plans/egtb/report1.md`: the histogram tables (raw outputs under
`docs/plans/egtb/measurements/plan1/`), the go/no-go decision with the
criterion applied, cross-validation results and any triaged defects,
deviations from this plan, problems encountered, missing tests, and next
steps (re-rank the backlog; on a go, plan2 is the 4-man generator).
