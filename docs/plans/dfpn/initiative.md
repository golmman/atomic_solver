# Initiative: `dfpn` — search semantics and algorithm-level node reduction

## Status

Active, maintained **agile**: plans are single-lever and sized to one session;
the backlog is re-ranked after every report. Plans 1–8 are done
(`report1.md`–`report8.md`) and predate this file. This `initiative.md` was
created on 2026-09-11 (after the position study below) to host the open
algorithmic backlog; until then the initiative was plan/report-driven only.
The next plan number is **plan9**.

## Motivation

### Position study 2026-09-11 — deep shuffle win (`4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`)

A solver study of this position (sibling of `m22_white`, pawns still on)
established the baseline for this initiative's problem class:

- Default mode (`--timeout 100`): win proven at **38,694,766 nodes / ~95 s**;
  PV refined 405 → 115 plies, ends `cap-cut` (unproven shortest).
- First-outcome mode: win at **20,313,879 nodes / ~65 s**; the first proven
  line is **405 plies**.
- The decisive point is **invariant across ε ∈ {0, 0.125, 0.5} and TT ∈
  {32 MB, 128 MB, 2 GB}** (~14.1M cumulative nodes at the 128M-work-chunk
  boundary in every variant). Tuning does not move it; the cost is
  algorithmic.
- The win is a *tempo/progression* conversion against a shuffling rook: the
  attacker's waiting moves in the refined PV are pawn pushes (`c2c3`,
  `h4h5`) that reset the 50-move clock between rook triangulation cycles
  (`g8f8/h8g8`). The defender's drawing resources — threefold repetition and
  rule50 expiry — are exactly the two conditions the current solver cannot
  cache across, so the shuffle-heavy subtrees are re-proved in every path
  context.
- 65% of the default-mode work (24.6M of 38.7M nodes) is PV refinement, not
  outcome finding.

This is the AGENTS.md priority class (decisive outcomes for ~60+ ply
positions), and the study shows the remaining headroom is in search
semantics, not parameters.

### The constraint every backlog item must respect: the plan7 twin removal

The GHI problem has been through a full implement-and-revert cycle here, and
any repetition-handling plan must build on that history:

- **plan5** implemented the Kishimoto–Müller base/twin transposition table
  with path-code hashing and Kawano-style simulation
  (`research_ghi.md`, `report5.md`).
- **plan7** **removed** it (`report7.md`): the simulation had soundness holes
  (a cyclic rook position `8/8/8/8/2k5/8/8/4KR2 w - - 0 1` returned a **false
  win**); `simulate` does not carry the twin's original ancestor set, and on
  simulation failure the solver fell back to base bounds instead of
  re-searching (see `research_ghi.md` §9, "Current implementation status").
  The current code is the conservative first-player-loss shortcut:
  repetition-dependent draws are stored as unsolved `(1, 1)` bounds and a
  one-ply `best_move_repeats_path` guard invalidates TT-resolved reuse that
  would immediately repeat the path.
- **Do not re-propose twins+simulation as implemented in plan5.** The two
  sound directions named by report7 are backlog #1 (per-search repetition
  cache) and #4 (Option A bounded cross-path verification).
- Similarly, **rule50 was added to the Zobrist key deliberately**
  (`research_ghi.md` §7 recommendation 2) as a cheap path-dependence guard.
  Backlog #2 keeps the key and attacks the resulting miss rate from the
  value side, not the key side.

## Goal

Reduce child-evals/nodes to reach a decisive outcome on deep,
repetition-dominated positions, with proofs that remain sound. Behavior
changes are expected here (unlike `lean`), but each must be explicit,
validated for correctness first, and gated on the drift protocol below.

Priorities follow AGENTS.md: correctness first, then performance, memory,
maintainability.

## Working agreement (agile cadence)

1. **Measure before planning.** Sizing spikes use temporary instrumentation
   and are reverted after measuring (lean's pattern).
2. **One lever per plan.** Soundness arguments belong in the plan, not the
   report.
3. **Validation is two-sided.** Behavior-changing plans must pass:
   - the **drift protocol** on cases the lever does not intend to change
     (`benchmark --suite quick --json --first-outcome` bit-identical per
     case), and
   - the **repetition soundness gate** (`cargo test --release --test
     test_repetition -- --include-ignored`: the cyclic rook safe-area
     position from report7 must never claim a win — see Measurement
     conventions) plus the move-order suite for regressions.
4. **The deterministic budget contract holds.** `child_eval_budget` /
   `ExitReason::BudgetExhausted` semantics must keep working exactly as
   documented (AGENTS.md, lean non-goals).
5. **Soundness beats node counts.** A lever that cannot state why it never
   returns a false decisive outcome is not implementable (this is the lesson
   of plan5 → plan7).

## Backlog (opened 2026-09-11; confidence-weighted)

| # | Item | Mechanism | Potential | Affects | Effort | Status |
|---|------|-----------|-----------|---------|--------|--------|
| 1 | Per-search repetition cache | Report7's named follow-up: cache repetition-dependent draw results **within a single search run, outside the TT**, keyed by (repetition key, path context), discarded at `reset_search_state`. Recovers plan7's "cyclic drawn positions are slower" cost without polluting path-independent TT entries | unmeasured; largest suspected lever for this class (the 14.1M-eval decisive phase is dominated by re-proving shuffle subtrees). Sizing spike: count repetition-draw `suppress_draw` stores and their subtree re-entry work | nodes | M–L | open — soundness design spike first: must state why per-search scoping cannot reintroduce the plan5 false win |
| 2 | Clock-budget-aware solved-entry reuse | Keep rule50 in the key (per research_ghi §7.2). Store solved Win/Loss with a conservative budget `100 − rule50` at store time; on probe, reuse across clocks only when the new position's budget covers the stored proof depth (ignores mid-line pawn/capture resets — conservative by construction) | unmeasured; shuffle lines revisit the same boards at many clock values, so TT hit rate in exactly the dominant subtrees should rise. Sizing spike: instrument "same board, different clock" probe misses | nodes | M | open |
| 3 | Continue refinement after cap-cut | Report8's named next step: when a refinement round ends `CapCut` and global budget remains, resume bounded refinement instead of stopping, while keeping the deterministic budget contract intact | recovers part of the 24.6M-node refinement tail that currently ends `cap-cut` at PV 115; outcome-finding unaffected | behavior (`PvStatus` semantics; possibly a new label) | M | open |
| 4 | Bounded cross-path verification (research_ghi §9 "Option A") | When a cached solved result's path does not match the current prefix, run a bounded fresh `dfpn` call at `max_depth = entry.depth` under the current path and accept only on agreement | strengthens the one-ply guard toward full cross-path soundness; enables safer reuse in cyclic regions | correctness first, nodes second | L | open — pairs with #1; do not attempt before #1's spike lands |

Cross-references: AND-side move-ordering signals stay in `lean` backlog #5
(duplicating them here would fork the oracle-floor constraints recorded
there). Wall-time engineering (path-scan cost, StateInfo reuse, clock
sampling) stays in `lean`. PV labeling itself is `pv/` territory; #3 only
changes *when* refinement stops, not how lines are labeled.

## Non-goals

- **Re-implementing the plan5 twin/simulation code.** Any repetition-handling
  plan starts from report7's analysis and the `research_ghi.md` §9 open
  items.
- **Removing rule50 from the Zobrist key** (see Motivation).
- **Throughput/wall-time micro-optimizations** — `lean`'s mandate, with its
  bit-identical drift gate.
- **Proof-tree layer, `ProofEvent` protocol, optimizer interface contract**
  (`docs/spec/optimizer_interface.md`) beyond what a plan explicitly
  specifies.

## Measurement conventions

- **Primary metric**: `child_evals` to first decisive outcome (per the
  optimizer spec); nodes and wall time secondary.
- **Stress case** for repetition-handling plans (#1, #2, #4):
  `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21` — baseline
  2026-09-11, 128 MB TT, default ε, reference host: first-outcome
  **20,313,879 nodes / ~65 s** (first line 405 plies); default mode
  **38,694,766 nodes / ~95 s**, PV 405 → 115 `cap-cut`; decisive point
  invariant across ε ∈ {0, 0.5} and TT ∈ {32 MB, 2 GB}. `m22_white`
  (`tests/fixtures/move_order_positions.txt`) remains the regression case —
  it is too easy for this class. Adding the stress case to the fixture file
  is a plan-level decision (it changes suite contents and JSON keys), not a
  silent edit.
- **Soundness gate**: `tests/test_repetition.rs` carries the report7
  cyclic-repetition regression tests (`rook_alone_does_not_claim_win_against_safe_king`,
  `reversible_cycle_keeps_repetition_key_and_stays_draw`; moved from
  `test_ghi.rs` by testability plan1). They must pass on every plan that
  touches repetition handling. Note: they are currently gated with
  `#[cfg_attr(debug_assertions, ignore = ...)]`, which AGENTS.md forbids
  ("the build profile must not select which tests run") — converting them to
  plain `#[ignore = "slow: ..."]` is a pending convention fix that changes
  which tier runs them and must be a deliberate decision, not a side effect
  of a repetition-handling plan.
- **Drift check**: `benchmark --suite quick --json --first-outcome`
  bit-identical before vs. after, except on the search surface a plan
  explicitly changes (then the plan lists the intended deltas and validates
  regressions on the move-order suite).

## History

- **plan1–plan8** — pre-initiative work (done, `report1.md`–`report8.md`):
  solver foundations, the GHI twin implementation (plan5), its removal in
  favor of the first-player-loss shortcut (plan7), and refinement-round
  termination reporting (plan8).
- **2026-09-11** — `initiative.md` created after the deep-shuffle-win
  position study (Motivation). Backlog #1–#4 opened from the study's
  algorithmic findings plus report7/report8's named follow-ups; baseline
  recorded under Measurement conventions.

Per repo convention, every plan ends with the task of writing its
`report<N>.md` in this directory.
