# Initiative: `dfpn` — search semantics and algorithm-level node reduction

## Status

**Dormant** (set 2026-09-17), maintained **agile** while open: plans are
single-lever and sized to one session; the backlog is re-ranked after every
report. **The backlog is empty** — every item is closed: #1 done (plan9),
#2 closed permanently (plan10/plan11), #3 closed as subsumed by
`--refine-cap 0` (2026-09-17, History), #4 closed no-go (the `conversion`
plan5 mining round), #5 closed no-go (plan12), #6 done (plan13). The last
live lever (threshold-cut-frame pricing) was handed to `conversion` backlog
#6 at dormancy. **Reopen triggers**: (a) a new *measured* search-semantics
diagnostic for the deep, repetition-dominated class that has no home in
`lean` (wall time), `conversion` (the deep-conversion class: reuse
envelope, line guidance, parallelism, threshold-cut pricing), or `egtb`
(tablebase); (b) a soundness regression traced to DF-PN+ search semantics
(e.g. `test_repetition`). On reopen, re-rank the backlog and open the next
plan, **plan14**. Plans 1–9 are done
(`report1.md`–`report9.md`) and predate this file; plan10 was executed as a
Phase 0 no-go with no code changes (`report10.md`); plan11 closed backlog #2
permanently (`report11.md`). This `initiative.md` was created on 2026-09-11
(after the position study below) to host the open algorithmic backlog; until
then the initiative was plan/report-driven only. The next plan number is
**plan14** (backlog #5 closed 2026-09-15 by plan12's evidence-based no-go;
backlog #6 — the horizon/threshold-pricing pre-phase — **closed 2026-09-16
by plan13**: architecture R implemented and validated, ladder fixed,
`report13.md`; backlog #3 — the last item from the 2026-09-11 opening batch —
**closed 2026-09-17** as subsumed by `--refine-cap 0`, decision record in
History).

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
| 3 | Continue refinement after cap-cut | Report8's named next step: when a refinement round ends `CapCut` and global budget remains, resume bounded refinement instead of stopping, while keeping the deterministic budget contract intact | recovers part of the 24.6M-node refinement tail that currently ends `cap-cut` at PV 115; outcome-finding unaffected | behavior (`PvStatus` semantics; possibly a new label) | M | **closed — subsumed by `--refine-cap 0` (measured no-go, 2026-09-17, decision record in History; no code changes)**: as a capability, #3 ≡ cap-0 — capped rounds at one depth bound over a persistent TT trace the same nodes as one uncapped round, so `--refine-cap 0` already recovers everything; no determinism win exists (both stop on the wall clock without an eval budget; with one, both are deterministic). Measured: extra uncapped refinement is poor PV value — stress case PV 129 → 109 (−20 plies, still `cut-short`) for 60 s → 300 s / 19.9M → 588M nodes; m22_white PV 95 → 91 for ~20 s → 60 s. The cap default (fast time-to-answer) is vindicated; a default flip (Option B) rejected |
| 5 | KQvK ladder: repetition-semantics resolution + repetition-contaminated work-mass reduction | New measurement class from `egtb` plan1 (`docs/plans/egtb/report1.md` addendum): 3-man KQvK wins unproven at >756M nodes (root `8/2K5/k7/8/8/8/8/4Q3 w`, win in 15; ~20× node growth per 2 plies; depth-limited `search_depth(·,15)` also fails at 1.09B nodes). Hypothesis: repetition-contaminated subtrees defeat TT memoization (first-player-loss shortcut) and overwhelm the plan9 cache's ancestor-context key. Phase 0 must first resolve whether the win exists under path-repetition semantics at all (the `egtb` spike's depth-16 prover ignored repetitions) before any fix arm is selected | ladder root >756M nodes/unproven → target ≤20M child evals proven; see `plan12.md` | nodes | M–L | **closed — evidence-based no-go (plan12, `report12.md`, 2026-09-15)**: G1 GO — the ladder wins are genuine under path-repetition semantics (T1a exact-DTM strategy certificate + T1b independent repetition-aware prover, dtm 15/13/11/9); G2 FAIL — cacheable mass 8.08% of the ~2.83B-eval gap on the gate object vs the ≥50% gate (repetition machinery measured inert: 0% path-rep frame exits, ≤0.66% plan9-cache hits, clock fragmentation = 0; Arm A2's lemma vacuous, Arm D dead, Arm C not "cheap"). Dominant mass (91.3%) is unsolved frontier churn — no repetition-semantics lever exists for this class. Successor lever: bounded-search horizon/threshold pricing (layered
bounded fixpoint pre-phase; region-closure draw proofs as by-product) —
**opened as backlog #6**; archived oracle `measurements/plan12/plan12_t1.rs`; ladder baselines reproduce bit-for-bit |
| 4 | Bounded cross-path verification (research_ghi §9 "Option A") | When a cached solved result's path does not match the current prefix, run a bounded fresh `dfpn` call at `max_depth = entry.depth` under the current path and accept only on agreement | strengthens the one-ply guard toward full cross-path soundness; enables safer reuse in cyclic regions | correctness first, nodes second | L | **closed — evidence-based no-go** (`research_ghi_journal.md`, 2026-09-13, mining round for `conversion` #5d): the journal GHI mechanism is the soundest reuse shape measured — node-entry return, verification-bounded volume, (1, 1) base re-init structurally exclude the plan10 hazard — but those same safety properties cap the win at plan11 arm A (−8.8% stress FO / −2.6% default, the unverified strict superset) and make draw-side reuse cost ≥ the ~1.1-eval re-proofs it saves (`research_repetition_cache.md` §2; plan9 already intercepts ~97% per `conversion/report1.md`). The paper's simulation is unavailable as machinery (our TT stores no proof trees) and its soundness is assumed there, not proven; the paper's framework extends to repetition-as-draw only vacuously for decisive facts and offers nothing for draw-proof reuse. The dependency "do not attempt before #1's spike lands" resolves to: `conversion` #1 closed no-go, its monotonicity lemma adopted by the contract as the value claim. Contract retained as (a) soundness-argument template, (b) design constraint for the parallel spike (`conversion` #4 / `lean` #2), where cross-worker contexts make the journal mechanism load-bearing |
| 6 | Bounded pre-phase for small decisive regions (horizon/threshold pricing) | plan12's measured asymmetry: a region fixpoint decides ~420k positions with certificates in ~6 s while bounded DF-PN collapses **zero** frames in 300 s (91.3% unsolved frontier churn; lean9: 99.7–99.9% of AND-frame evals in threshold-cut frames). A detector-gated pre-phase (occupied ≤ 3 men, no pawns) decides the **root** via a non-DF-PN architecture — PN bounded prover, layered certificate fixpoint, or region-closure fixpoint (Phase 0 bake-off) — with mandatory replay-verified cycle-free certificates and no TT interaction (plan10/11 hazard excluded by construction) | ladder root >756M nodes/unproven → ≤ 20M child evals and ≤ 60 s, certified (plan12's criterion); byte-identical everywhere the detector does not fire | nodes; plus a `PvStatus` variant and a `preflight:` CLI line | M–L | **done (plan13, `report13.md`, 2026-09-16)**: Phase 0 bake-off selected architecture R (region-closure fixpoint; P and L measured no-gos — path-tree memory / unsound-to-store facts, see `report13_phase0.md`). Session B implemented `src/search/preflight/` (exact packed keys, closure + exact-rank fixpoint, D3 rank guard `halfmove + rank ≤ 99`, standalone replay verifier, A5 principal-line PV, no TT interaction, no `ProofEvent` emission per R2, R4 budget accounting) with the `--no-preflight` flag and the `preflight:` CLI line; `PvStatus::PreflightProof` added (D1). Ladder root certified Win at 5,866,690 child evals / 0.76 s (exact DTM 15; all four positions 15/13/11/9) — plan12's criterion met at 0.29× evals. Draw claims (D2) proven by full closure, incl. the cyclic rook (factual fix: it is 3 men and gates; claimed Draw, `test_repetition` green) and an adjacency-immunity KQvK draw (table-confirmed). Battery: quick drift 59/59 byte-identical, m22/stress byte-identical, G-B oracle 240 samples / 0 contradictions, `make test-full` 387/0 |

Cross-references: AND-side move-ordering signals lived in `lean` backlog #5
— **closed no-go by the lean plan9 spike** (`lean/report9.md`, 2026-09-15):
the refuter is already at final-sorted rank 0 in 100% of refuted AND frames
and pre-refuter eval mass is 0.00–0.02% of child evals, so there is no
ordering surface on the AND side at all. The same spike surfaced the one
open AND-side observation that *does* belong here: **99.7–99.9% of AND-frame
own child evals sit in threshold-cut frames** (frames exhausting their DF-PN
thresholds without an outcome — 8,625,570 of 8,645,613 on m22, 148,390,289
of 148,602,798 on the shuffle-win stress case). That is a threshold-dynamics
property, not an ordering property: cut frames never reach a refutation
exit, so no move-ordering signal can touch them; reducing that mass means
changing how unsolved-subtree exploration is priced — the same DF-PN
threshold-arithmetic territory as the plan10/plan11 bound-folding diagnostic
and the EWS/MOPNS reading named below. **This observation moved to
`conversion` backlog #6 on 2026-09-17** (dfpn dormancy handover). Wall-time engineering (path-scan
cost, StateInfo reuse, clock sampling) stays in `lean`. PV labeling and
`PvStatus`/`pv_status` semantics are the search layer's own contract (the
`pv/` initiative closed 2026-09-13; its PPV-from-proof-tree item was moved to
`proof` backlog #8 and closed won't-fix there on 2026-09-17); #3 only
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
- **KQvK ladder** for repetition-semantics plans (#5, opened 2026-09-14):
  root `8/2K5/k7/8/8/8/8/4Q3 w - - 0 1` plus the three simplified positions
  in `plan12.md` (win in 15/13/11/9; baselines >756M unproven / 42.7M /
  3.9M / <201k nodes, 600 s default mode). Ground-truth asset: the 3-man
  q-table from `egtb` plan1 (`docs/plans/egtb/measurements/plan1/egtb3-q.bin`,
  regenerable via `egtb_gen3`), cross-validated by three oracles there.
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
- **2026-09-13** — **backlog #4 closed as an evidence-based no-go**
  (`research_ghi_journal.md`): the journal GHI paper (Kishimoto & Müller,
  *Information Sciences* 175(4), 2005; author copy vendored as
  `ghi_journal.pdf`) was mined for `conversion` #5d. Key findings: the journal
  version adds the proofs (Theorems 3.1/3.2) but **not** a step-by-step
  simulation procedure — the twin's ancestor-context gap
  (`research_ghi.md` §9) is in the paper's own specification; the theorems
  assume a sound verification oracle rather than proving one; the paper's
  framework maps onto repetition-as-draw only vacuously (decisive facts are
  path-independent by our rule, and the paper has no machinery for
  path-dependent *draws*); the reuse path cannot reproduce the plan10 hazard
  (node-entry return + verification cost + (1, 1) re-init) but is
  economically empty here (arm A ceiling −8.8% FO; ~1.1-eval draw re-proofs).
  Also corrects `research_ghi.md` §4.2 (root thresholds ∞ − 1, not (1, 1)).
  Remaining implementation-level source: Kishimoto's 2005 Ph.D. dissertation
  (open access) — flagged if the parallel spike needs the full procedure.
  The contract's reuse rule is retained as a design constraint for the
  parallel-search spike (`conversion` #4 / `lean` #2).
- **2026-09-15** — **`lean` plan9 measured the AND-side ordering surface empty
  and handed this initiative one structural diagnostic** (`lean/report9.md`;
  `LEAN9_SPIKE=1` frame-level counters, reverted, `src/` byte-identical).
  At refuted AND frames the refuter sits at final-sorted rank 0 in ~100% of
  frames and is found in the initial sweep in 100% of them (0 recursion
  resolutions) — it is a statically-ranked-0 terminal reply (extinction
  blast-capture or moves-empty mate) — so `lean` #5 is closed no-go and no
  AND-side ordering lever exists. The transferable finding for this
  initiative's next levers: the frame-level own-eval partition (exact, delta
  0 vs `child_evals`) shows **99.7–99.9% of AND-frame own evals in
  threshold-cut frames** (m22 8,625,570/8,645,613; shuffle-win
  148,390,289/148,602,798), i.e. the AND-side cost is unsolved-subtree
  exploration priced by DF-PN disproof thresholds. Any lever for the class
  must therefore move threshold dynamics or pricing (backlog #3's bounded
  refinement, the plan10-era bound-folding hygiene diagnostic, EWS/MOPNS
  reading, parallelism), not ordering. Same-run M1 sanity: OR winning child
  already at rank 0–1 in ~97% of OR-Win frames (rank-optimal; the nn 90.6%
  work-share figure is a population/attribution difference, OR ordering
  stays closed).
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

- **2026-09-14** — **backlog #5 opened from the `egtb` plan1 cross-validation;
  plan12 drafted** (`plan12.md`). The `egtb` spike found KQvK positions the
  solver cannot prove (root win-in-15 unproven at >756M nodes; depth-limited
  `search_depth(·,15)` also fails at 1.09B nodes vs a ~7.5M transposition
  upper bound). plan12's Phase 0 must first resolve whether the win exists
  under the solver's path-repetition semantics — the `egtb` spike's
  independent prover ignored repetitions, so its "solver defect" conclusion
  is provisional (caveat recorded in the `egtb` report addendum) — and only
  then selects a fix arm from T2 work-mass diagnostics, under the plan10/11
  arm-matrix + hard-gate pattern. The `egtb` 3-man tables are adopted as
  this initiative's ground-truth asset for the class (Measurement
  conventions).
- **2026-09-15** — **plan12 Phase 0 executed (Session A; G1 GO, G2 FAIL,
  no-go recommendation pending user checkpoint)** (`report12_phase0.md`, raw
  logs + archived spike sources under `measurements/plan12/`, `src/`
  byte-identical to HEAD after revert, post-revert baselines reproduce
  bit-for-bit, fast gate green). T0: the addendum's ladder numbers are
  last-chunk-boundary node counts and reproduce exactly (755,986,909 /
  42,682,820 / 3,874,308 / <201,482); bounded `search_depth(·,15)` 1.106B
  nodes Timeout (band). T1: both instruments agree — the ladder wins are
  **genuine under path-repetition semantics**; T1a (exact-DTM retrograde
  fixpoint over the ~420k-position region, 0 table mismatches per root)
  certifies the rank-decreasing strategy cycle-free on every line (the naive
  greedy winning-child strategy cycles on every root), exact dtm 15/13/11/9;
  T1b (independent repetition-aware prover; subset-transfer memo +
  monotonicity pruning after the planned exact-key memo measured useless)
  proves WIN at depth = dtm on all four (root: 1.02B nodes at a 3B cap).
  T2: the repetition machinery is inert on the class — 0% path-repetition
  frame exits, ≤0.66% plan9-cache hits, class-1 mass ≈ 0.00%, clock
  fragmentation = 0 events (Arm D dead); attributed mass on the G2 gate
  object (bounded-15 root) = **8.08% of the ~2.83B-eval gap vs the ≥50%
  gate** (Arm A2's lemma additionally vacuous: Reach(P) ≈ all ancestors on
  the near-strongly-connected 3-man graph; Arm C fails its "cheaply"
  precondition: T1b root proof ≈ the solver's own failing cost). The
  dominant mass (91.3% class-3) is unsolved frontier churn — 79% of frames
  are depth-0 leaves, 99% of evals unsolved, 0 TT-resolved frames — pointing
  at bounded-search horizon/threshold pricing, not repetition semantics.
- **2026-09-15** — **plan12 closed at the user checkpoint; backlog #5 marked
  closed** (`report12.md`). The no-go recommendation was confirmed: no
  Session B, no code changes, no fixture change (the ladder is not fixed).
  The successor lever (bounded-search horizon/threshold pricing; layered
  bounded fixpoint pre-phase as the plan13 candidate) is recorded in the
  backlog row; the next plan number is plan13.
- **2026-09-15** — **backlog #6 opened; plan13 drafted** (`plan13.md`):
  the backlog #5 successor lever (bounded-search horizon/threshold
  pricing) as a small-space-gated pre-phase that decides the root only —
  no TT interaction, so the plan10/11 bound-folding hazard is excluded by
  construction. Phase 0 is a table-free architecture bake-off over the
  KQvK ladder (PN bounded prover / layered certificate fixpoint /
  region-closure fixpoint, certificate-replay mandatory), gated on
  plan12's success criterion (root ≤ 20M child evals / ≤ 60 s). The plan's
  five review questions are resolved in-plan: R1 (new `PvStatus::
  PreflightProof` variant — `ProvenShortest` would be a false claim for
  bound-only certificates) decided; R2 (no `ProofEvent` emission in
  round 1, gap documented), R3 (detector predicate), R4 (budget
  accounting), R5 (CLI/integration surface) recorded as veto-able
  assumptions with stated reversibility.

- **2026-09-16** — **plan13 Phase 0 executed (Session A; G-A GO via R, G-B PASS; recommendation: architecture R, awaiting checkpoint)** (`report13_phase0.md`, raw logs + archived spike sources under `measurements/plan13/`, `src/` byte-identical to HEAD after revert, T0 reproduces bit-for-bit incl. the bounded-15 chunk boundary 796,353,729). T0 confirmed steps 1–3 exactly. T1 table-free bake-off over the ladder (root/step1/step2/step3 × horizons dtm−1/dtm/dtm+2, mandatory replay-verified certificates): **R (region-closure fixpoint)** certifies the root at 5.83M child evals / 0.67 s, region 420,532, exact dtm 15/13/11/9, 0 region-vs-table mismatches, and proves all 120 sampled oracle draws as a free by-product; **P (PN bounded prover)** certifies dtm-9 step3 at 81k evals / 0.03 s (370× over plan12 T1b) but is node-memory-censored from dtm ~11 up (8M and 16M node caps; the path-tree object has no unproven-frontier sharing under repetition cuts — pricing was not the missing ingredient); **L (layered certificate fixpoint)** is eval-cap-censored at layer 8 on the root with 77k path-bound fact fallbacks vs 1,077 context-free facts, and all 85 of its oracle WIN claims failed certificate replay (safe downgrades, zero yield). G-A PASS via R (0.29× eval bar), G-B PASS (240 samples, 0 contradictions). R1–R5 stand as drafted; Session B (Phase 1 implementation of `src/search/preflight.rs`) pending the user checkpoint.
- **2026-09-16** — **plan13 user checkpoint resolved; Session B fully specified** (see the "Checkpoint record" section at the end of `report13_phase0.md`, self-contained). Architecture R confirmed. Decisions: D1 new `PvStatus::PreflightProof`; D2 WIN **and** DRAW at root (closure draw proofs in scope — measured 120/120 correct; clock-independent by the monotonicity lemma); D3 post-fixpoint rank guard `halfmove + root_rank ≤ 99` for WIN claims (draw claims exempt); D4 region budget default 1,000,000 (covers all 3-man components; transient closure memory ~100–120 MB to be documented). A1–A8 informed assumptions recorded (incl. A3 certificate bound = `max_depth` under `search_depth`, A5 rank-decreasing principal-line PV). Session B to be run in a fresh session against the checkpoint record + `plan13.md` Phase 1.

- **2026-09-16** — **plan13 closed — backlog #6 implemented and done** (`report13.md`, Session B). Architecture R (region-closure pre-phase) implemented as `src/search/preflight/` per the checkpoint record: detector R3 (≤ 3 men, no pawns/castling — with the factual correction that the report7 cyclic-rook position is 3 men and gates; it is claimed Draw by full closure, sound and `test_repetition`-green), exact collision-free packed keys, AND/OR fixpoint with exact ranks, D3 rank guard (`halfmove + rank ≤ 99`, boundary-tested end-to-end at rank 15: clock 84 claims, 85/99 defer), standalone replay verifier with negative-case unit tests, A5 principal-line PV, D1 `PvStatus::PreflightProof`, D2 draw claims, D4 region budget 1,000,000, R4 eval-budget accounting, R5 `--no-preflight` + `preflight:` line. Validation: quick drift 59/59 byte-identical; m22/stress byte-identical to HEAD; G-B oracle on the release binary 240 samples / 0 contradictions (94 wins certified, 120/120 draws); `make test` and `make test-full` (387/0) green. Ladder root 5,866,690 child evals / 0.76 s, exact DTM 15 — the plan's goal met. New engine-semantics finding documented: commoner adjacency immunity makes adjacent-king KQvK positions genuine draws (table-confirmed), which the pre-phase now proves by closure. Four pre-existing tests recalibrated to non-gated 4-men fixtures (they exercised DF-PN invariants on 3-men positions now claimed by the pre-phase). Next plan number: plan14.

- **2026-09-17** — **backlog #3 closed as subsumed by `--refine-cap 0`** (decision record; no code changes, `src/` untouched). A design walkthrough retracted both of the item's original justifications: (a) *capability* — refinement-resume as specified (a series of capped rounds at the same depth bound with the TT carrying over) explores exactly the nodes of cap-0's single long round, so `--refine-cap 0` is not an approximation of #3, it **is** #3; (b) *determinism* — in a plain CLI run both #3's loop and cap-0 stop on the wall clock, and under an eval budget both are equally deterministic (the budget is the stopping condition), so the deterministic-budget contract is untouched either way. The cap's real function is time-to-answer: a round cut at the timeout earns the identical `Unproven` label whether it ran 3 s or 60 s, so letting a hopeless bound burn the full remaining timeout only makes the user wait for an identically-labeled result — a dial `--refine-cap` already exposes at both ends. The report8 "next step" measurement was run before closing (default ε, 128 MB TT, reference host, post-plan9 binary — the pre-plan9 baseline numbers in Motivation are historical): stress case cap 0.25 → win, PV 129 plies, `cap-cut`, 19,943,731 nodes / 60.0 s; `--refine-cap 0 --timeout 300` → PV 109 plies, `cut-short`, 588,050,432 nodes / 300 s (29.5× nodes, 5× time, −20 plies, still not shortest). m22_white (`--timeout 60 --refine-cap 0`) → PV 91 plies, `cut-short`, 100,167,680 nodes vs the 95-ply `cap-cut` golden at ~20 s (3× time, −4 plies). Extra uncapped refinement marginally shortens the PV and never reaches `ProvenShortest` on this class — the tail is the shuffle re-proof churn plan12 localized as unsolved frontier churn with no repetition-semantics lever, so grinding it is exactly the "burns the clock" outcome. Option A adopted (close as subsumed; document nothing new); Option B (cap-0 default flip) rejected: the marginal gains do not justify making 5×/3× time-to-answer the default when the same thoroughness is one flag away.

- **2026-09-17** — **initiative set dormant** at the user checkpoint
  following backlog #3's closure: no open backlog items remain. The one
  live observation (threshold-cut-frame pricing, `lean` plan9 diagnostic)
  was handed to `conversion` backlog #6, whose class mandate and plan10
  hazard documentation make it the natural owner; scope guard there:
  diagnostic-first Phase 0, no solved-fact folding into threshold
  arithmetic. Reopen triggers recorded in Status; the next plan number
  remains **plan14**. Remaining class levers: the threshold-cut-frame pricing observation (lean plan9 diagnostic — a threshold-dynamics change, the same territory as the plan10/11 bound-folding hazard); EWS/MOPNS reading and parallelism live in `conversion`.

Per repo convention, every plan ends with the task of writing its
`report<N>.md` in this directory.
