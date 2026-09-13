# Initiative: `dfpn` — search semantics and algorithm-level node reduction

## Status

Active, maintained **agile**: plans are single-lever and sized to one session;
the backlog is re-ranked after every report. Plans 1–9 are done
(`report1.md`–`report9.md`) and predate this file; plan10 was executed as a
Phase 0 no-go with no code changes (`report10.md`). This `initiative.md` was
created on 2026-09-11 (after the position study below) to host the open
algorithmic backlog; until then the initiative was plan/report-driven only.
The next plan number is **plan11**.

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
| 1 | Per-search repetition cache | Report7's named follow-up: cache repetition-dependent draw results **within a single search run, outside the TT**, keyed by (position hash, ancestor repetition-key set), discarded at `begin_run`. Recovers plan7's "cyclic drawn positions are slower" cost without polluting path-independent TT entries | spike measured (2026-09-12, `research_repetition_cache.md` §2): 31,620 of 33,087 repetition-draw proofs on the stress case are re-proofs of 1,467 distinct positions; the direct proof frames are cheap, so the win must come from making the whole draw chain cacheable | nodes | M–L | **done (plan9, `report9.md`)**: stress case first-outcome 351M → 249M child evals (−29%), default mode 466M → 338M (−27%); quick-suite drift limited to the 14 repetition-heavy cases (all outcomes unchanged); capacity default `1 << 18` after measuring the 1.6k-entry working set |
| 2 | Clock-budget-aware solved-entry reuse | Keep rule50 in the key (per research_ghi §7.2). Store solved Win/Loss with a conservative budget `100 − rule50` at store time; on probe, reuse across clocks only when the new position's budget covers the stored proof depth (ignores mid-line pawn/capture resets — conservative by construction) | unmeasured; shuffle lines revisit the same boards at many clock values, so TT hit rate in exactly the dominant subtrees should rise. Sizing spike: instrument "same board, different clock" probe misses | nodes | M | **closed — measured no-go (plan10, `report10.md`, 2026-09-12)**: sound (444/444 adopted-claim re-verifications, 59/59 quick outcomes unchanged) and large on the target (stress first-outcome −37.7%, default −41.6%), but the benefit and a DF-PN destabilization are one mechanism (solved children's 0/INF bounds folded into unsolved parents' thresholds); the m22_white control collapses 3.1 s win → 120 s timeout and 15/59 quick cases regress up to +185%. Flooring the folding rescues the control but eliminates the entire stress win. All instrumentation reverted; tree byte-identical to post-plan9. **Reopened and re-closed as plan11** (2026-09-13, `report11.md`): the two decoupling mechanisms were measured in a seven-arm Phase 0 matrix — (A) frame-entry adoption with TT store-back (−8.8% stress FO / −2.6% default, m22 1.28×, quick suite byte-identical: sound and drift-free but below the 10% gate and useless in default mode), (B1–B3) site-2 direction split with quarantine (all fail: quarantining AND-parent Win facts kills the stress win, +110%; adopting them solved kills m22 via 10.55M folds), (AB) hybrid (+0.5%), (A2) site-2 prefetch store-back (timeout). The decisive diagnostic: **84.8% of the adoption mass is AND-parent Win facts — simultaneously the stress-win driver and the m22 destabilizer**; no tested decoupling separates them. Arm C (bounded verification) not run — negative prior documented in report11. Backlog #2 **closed permanently**; the sequential node count on this class is measured-optimal for DF-PN+ + plan9 cache under current repetition semantics |
| 3 | Continue refinement after cap-cut | Report8's named next step: when a refinement round ends `CapCut` and global budget remains, resume bounded refinement instead of stopping, while keeping the deterministic budget contract intact | recovers part of the 24.6M-node refinement tail that currently ends `cap-cut` at PV 115; outcome-finding unaffected | behavior (`PvStatus` semantics; possibly a new label) | M | open |
| 4 | Bounded cross-path verification (research_ghi §9 "Option A") | When a cached solved result's path does not match the current prefix, run a bounded fresh `dfpn` call at `max_depth = entry.depth` under the current path and accept only on agreement | strengthens the one-ply guard toward full cross-path soundness; enables safer reuse in cyclic regions | correctness first, nodes second | L | open — pairs with #1; do not attempt before #1's spike lands |

Cross-references: AND-side move-ordering signals stay in `lean` backlog #5
(duplicating them here would fork the oracle-floor constraints recorded
there). Wall-time engineering (path-scan cost, StateInfo reuse, clock
sampling) stays in `lean`. PV labeling and `PvStatus`/`pv_status`
semantics are the search layer's own contract (the `pv/` initiative
closed 2026-09-13; its PPV-from-proof-tree item moved to `proof`
backlog #8); #3 only changes *when* refinement stops, not how lines
are labeled.

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
  touches repetition handling. Their gating was converted to plain
  `#[ignore = "slow: ..."]` on 2026-09-12 (the former
  `#[cfg_attr(debug_assertions, ignore)]` violated AGENTS.md; recorded in
  History).
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
- **2026-09-12** — Convention fix preceding plan9: the two
  `tests/test_repetition.rs` gates converted from
  `#[cfg_attr(debug_assertions, ignore)]` to plain
  `#[ignore = "slow: cyclic GHI regression; run with -- --include-ignored"]`
  per AGENTS.md. No grep-matchable occurrence of
  `cfg_attr(debug_assertions` remains under `tests/`, `src/`, or
  `examples/`. Verified: excluded by the fast gate, passing with
  `--include-ignored` in release (both tests pass, ~5 s).
- **2026-09-12** — Backlog #1 soundness design + sizing spike
  (`research_repetition_cache.md`, temporary instrumentation reverted after
  measuring) and `plan9.md` written. Key refinement surfaced by the spike:
  the cache key uses the *full position hash* (not the repetition key) for
  the node component, because boards sharing a repetition key can differ in
  halfmove clock and value.
- **2026-09-12** — Backlog #1 **implemented** (plan9,
  `repetition_cache.rs`): per-search cache of repetition-dependent draws
  keyed by (full position hash, order-independent ancestor-set context
  hash), probed after the TT solved-result check and stored where the
  first-player-loss shortcut suppresses the TT store; cleared once per run
  in `begin_run`. Stress case first-outcome child evals 351,297,052 →
  249,480,478 (−29.0%), default mode 465,827,574 → 338,094,183 (−27.4%);
  working set 1,314/1,643 entries → capacity default shrunk `1 << 20` →
  `1 << 18`. Quick-suite drift confined to the 14 repetition-heavy cases
  (outcomes unchanged; dec10 first line 49 → 41 plies, stress-case first
  line 405 → 477 plies — both verified legal wins). Next ranked lever:
  backlog #2 (clock-budget-aware solved-entry reuse).
- **2026-09-12** — **plan10 drafted** (backlog #2, clock-budget-aware
  solved-entry reuse): a cross-clock shadow index keyed by the board-only
  repetition key, storing `(best_move, outcome, depth)` for fully solved
  Win/Loss facts only, probed at both reuse sites (`dfpn` node entry and
  `evaluate_child`) after the exact-key solved check misses, under the
  adoption rule `rule50 + proven_depth ≤ 100` plus the existing one-ply
  guard. Soundness rests on a shift lemma (plan §Soundness contract): Win/Loss
  proof trees replay identically across clocks because interior nodes stay
  ≤ clock 99 under the budget rule and leaf values (mate/extinction, both
  outranking rule50) are clock-independent. The lever is unmeasured, so the
  plan's Phase 0 is a temporary-instrumentation sizing spike with a hard
  go/no-go before implementation; a negative result closes the item and
  re-ranks to #3. Next ranked lever after #2's outcome: backlog #3.
- **2026-09-13** — **plan11 executed — backlog #2 closed permanently**
  (`report11.md`). Phase 0 ran a seven-arm matrix (PLAN10 validation arm +
  A, B1, B2, B3, plus two arms added mid-spike: AB hybrid and A2 prefetch)
  after reproducing the post-plan9 baselines and report10's full-lever
  numbers bit-for-bit. Results: B1/B3 timeout on stress (+110% — AND-side
  quarantine stalls proof completion), B2 wins stress at −36.1% but the
  m22 control times out via 10,553,135 AND-side folds, A is sound,
  drift-free (quick suite byte-identical) and control-safe (1.28×) but
  only −8.8% FO / −2.6% default, AB +0.5%, A2 timeout. The new
  direction × parent-type histogram (report10's named missing diagnostic)
  localized the pathology: 84.8% of adoptions are AND-parent Win facts,
  which are both the stress win and the control's death. Arm C
  (bounded verification) was skipped with a documented negative prior.
  All spike instrumentation reverted; the restored binary reproduces the
  stress baseline exactly (13,907,467 nodes, 477-ply line). Remaining
  levers for the class: parallelism, EWS/MOPNS reading, clock-pressure
  ordering, refinement-after-cap-cut.
- **2026-09-13** — **backlog #2 reopened as plan11** (`plan11.md`): the
  resurrection attempts plan10's −37.7% via two decoupling mechanisms,
  grounded in two structural facts established by code reading. (1) The
  parent's only child channel is `evaluate_child` (the recursive `dfpn`
  return value is discarded; the TT is the channel), so a frame-entry-only
  hit must store back into the TT and folds through the pre-existing
  exact-key path at plan10's volume — mechanism A is measured as the
  control arm. (2) report10's differential table shows the destabilizer is
  direction-specific: adopted Loss facts (winning move found) were
  m22-safe (3.1 s, 13.4M vs 14.2M baseline), adopted Win facts are the
  pathology — so mechanism B quarantines Win facts at site 2 (`outcome:
  None`, `explored: true`, conservative `(INF,1)`/`(1,INF)` bounds —
  suppress-only, can never enable a decisive claim) and adopts Loss facts
  solved, with parent-type topology sub-arms B1/B2/B3. Phase 0 is one arm
  matrix (A, B1–B3, plus `PLAN10` as a spike-build validation arm) with
  the direction × parent-type adoption histogram report10 named as the
  missing diagnostic; contingency arm C (bounded cross-path verification
  per research_ghi §9 Option A) is documented but not built in round 1.
  Go gates: stress ≥ 10% improvement, m22 ≤ 2× baseline wall, all 59
  quick outcomes unchanged, cyclic rook safe.
- **2026-09-12** — **plan10 executed — backlog #2 closed as a measured
  no-go** (`report10.md`). Phase 0 spike: the cross-clock shadow index
  (rep_key → best_move/outcome/depth, adoption rule `rule50 + depth ≤ 100`
  + one-ply guard, probed at `dfpn` node entry and `evaluate_child`) is
  sound and delivered stress-case first-outcome child evals 249,480,478 →
  155,394,450 (−37.7%) and default mode 338,094,183 → 197,745,729 (−41.6%)
  with a 170k-entry working set. But the designated no-adoption control
  m22_white collapsed (3.1 s win → 120 s timeout; root dn explodes
  9.3k → 503k), and 15/59 quick cases regressed (up to +185%) — with all 59
  outcomes unchanged and 444/444 sampled adopted Wins independently
  re-verified, isolating the defect as search dynamics, not soundness.
  Differential experiments (Win/Loss × OR/AND slices, gap-filling gate,
  folded-bound flooring) showed the stress win and the destabilization are
  inseparable: both come from solved children's (0, INF)/(INF, 0) bounds
  folding into unsolved parents' DF-PN threshold arithmetic. No-go per the
  plan's Phase 0 gate; all temporary instrumentation reverted (baseline
  reproduced bit-for-bit after the revert). New diagnostic surfaced for a
  possible future plan: solved-child bound-folding hygiene
  (`select_from_children`). Next ranked lever: backlog #3.

Per repo convention, every plan ends with the task of writing its
`report<N>.md` in this directory.
