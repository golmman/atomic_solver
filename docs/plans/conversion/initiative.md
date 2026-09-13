# Initiative: `conversion` — solving deep tempo/progression conversions

## Status

New initiative, opened 2026-09-13 after a missed-research review triggered by
the `make stress` position remaining hard after `dfpn` plans 1–10 and the
`lean` micro-optimization rounds. Plans are single-lever, sized to one
session, agile like `dfpn`/`lean`. The next plan number is **plan6**
(plan1/plan2/plan4 closed as no-gos; plan3 was the #5a/#5b reading round —
EWS + MOPNS, both no-go; plan5 is the #5d reading round — journal GHI,
drafted 2026-09-13; the ordering-guidance half of #2a
is parked behind the reading rounds, its premise weakened by report2's
budget-instability observation).

## Motivation

### The problem class

`make stress` (`4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`)
is a 60+ ply win whose decisive mechanism is a *tempo/progression
conversion* against a shuffling rook: the attacker interleaves
clock-resetting pawn pushes with rook triangulation; the defender's
resources are exactly the two conditions the TT cannot cache across
(threefold repetition, rule50). Post-plan9 baselines (`dfpn/initiative.md`):
first-outcome 249M child evals, default mode 338M. The `lean` initiative
bounded the per-node ordering headroom (OR side at oracle floor) and the
wall-time micro-costs; `dfpn` plans 9/10 showed the remaining headroom is
repetition/cache semantics, not parameters or per-node speed.

### Why plain PNS variants are not the answer

The classic DF-PN lever set (1+ε trick, DF-PN+ heuristics, PDS, PN²) is
either implemented or measured out: the cost here is *structural* —
repetition-dependent draw chains are uncacheable in a path-independent TT,
and plan10 showed that folding cross-clock **Win/Loss** facts back into the
search destabilizes DF-PN threshold dynamics catastrophically (m22_white
3.1 s → 120 s timeout). Any new lever must change *what is cacheable* or
*where the line comes from*, not the threshold arithmetic.

### What an alpha-beta engine does differently

Fairy-stockfish reaches the same conclusion on this position far faster in
wall time — but it does not *prove* anything; its NNUE search finds a
plausible winning line and prunes aggressively. Two consequences:

1. Comparing a proof solver's wall time to an engine's play strength is
   not apples-to-apples; the honest question is which product is needed
   (proven outcome vs. fast winning line).
2. The engine's speed is *usable* by a sound solver as a **line
   oracle**: propose candidate lines, then verify them exhaustively.
   The `dfpn` initiative's constraint (plan7) only forbids re-using
   engine-strength *beliefs* as facts; using them for ordering and for
   candidate generation followed by sound verification is exactly the
   Kawano-simulation idea done at the root instead of per-TT-entry.

## Goal

Reduce child-evals and wall time on the deep-conversion position class
while keeping proofs sound, by (a) enlarging the sound reuse envelope for
repetition-dependent draws, (b) importing line guidance from strong
non-solver engines, and (c) parallelism. Priorities follow AGENTS.md:
correctness first, then performance, memory, maintainability.

## Working agreement (agile cadence)

1. **Measure before planning** (lean's cadence): sizing spikes use
   temporary instrumentation, reverted after measuring.
2. **One lever per plan**; soundness arguments belong in the plan.
3. **Validation is two-sided** (dfpn's protocol): the drift protocol on
   unintended surfaces, plus the repetition soundness gate
   (`cargo test --release --test test_repetition -- --include-ignored`;
   the cyclic rook must never claim a win).
4. **The deterministic budget contract holds**: `child_eval_budget` /
   `ExitReason::BudgetExhausted` semantics unchanged.
5. **A monotonicity lemma for every reuse rule.** Any plan that widens
   cache reuse must state the value-monotonicity it relies on (see
   backlog #1 for the template) and its reuse direction, plus a
   property test that a second `solve` in one process cannot flip a
   decisive outcome.

## Backlog (opened 2026-09-13; confidence-weighted)

| # | Item | Mechanism | Potential | Affects | Effort | Status |
|---|------|-----------|-----------|---------|--------|--------|
| 1 | **Monotone draw cache v2** (superset-context, cross-clock, draws-only) | Extend plan9's repetition cache: key on the board-only `repetition_key` (not the clock-qualified hash); store `(proven rule50 clock, complete ancestor rep-key set)`. Reuse iff probe clock ≥ stored clock **and** probe ancestor set ⊇ stored set. Soundness lemma: under the two-fold-repetition-as-draw semantics, solved value is monotone — Win and Loss are downward-closed in both the ancestor set and the clock (a Win/Loss proof is structurally repetition-free and clock-independent except that smaller clocks can only *add* budget), hence Draw at a stored (A, c) is Draw at any (A′ ⊇ A, c′ ≥ c). Draws-only means the cache can never fold solved (0, INF)/(INF, 0) bounds into unsolved parents' thresholds — the exact defect mechanism that killed plan10 (report10) — because hits return at frame entry like path-repetition terminals, never as child bounds | plan10 measured −37.7%/−41.6% for full cross-clock reuse (no-go only because of Win/Loss folding); plan9's exact-context cache got −29%; v2 targets the *remaining* re-descent churn (contexts that differ by unrelated earlier excursions, and clock-drift revisits of the same board). **Measured no-go** (`report1.md`, 2026-09-13): the v2-only hit surface is thin — plan9's exact cache already intercepts ~97% of the v2 hit surface (stress first-outcome: 7,884 v2 hits, 7,651 at clock-delta 0, only 267 v2-only); net effect +0.1% child evals vs the ≥5% go bar; default mode +8.0% | nodes | M | **closed — no-go** (plan1 Phase 0, `report1.md`); the monotonicity lemma is retained as a candidate soundness argument for `dfpn` #4 |
| 2 | **Candidate-guided outcome search** (engine-hybrid) | Use a strong atomic engine (fairy-stockfish with NNUE) as a line oracle: (a) *ordering guidance* — seed `sort_moves`/killer/history so the candidate line's moves are searched first (zero soundness risk: ordering only); (b) *verification mode* — prove the candidate line as a PPV first (`search_depth_with_prefix` + the `verify_ppv` machinery already exist); only if verification fails, fall back to the unguided solve. Literature analog: Kawano simulation at root scale; standard practice in tsume-shogi solving (engine-guided DF-PN) | **Measured no-go for the verification lever 2b** (`report2.md`, 2026-09-13): on the stress case no engine candidate (1M/10M/100M nodes) verified — the 10M/100M PVs burned >1.0B/>1.4B verification evals at the 600/900 s budget with defender reply a5a4 unproven (resource-cut Draw), i.e. the engine's 15–19-ply "mate" lines are cooperative-horizon lines, not proofs; m22 control: engine 6.5 s + 300 s failed verification vs the 3.1 s unguided baseline; dec sample (6 cases): 6/6 verified in ≤0.55 s but the 1.2–5.3 s engine query makes guided ≈ unguided or worse on easy positions. Engine time alone is a real additive cost the unguided solve does not pay | nodes + behavior (new mode; drift protocol N/A for the mode, ordering-guidance variant must keep quick-suite outcomes) | M–L | **closed — no-go for 2b** (`plan2.md` Phase 0, `report2.md`); ordering guidance (2a) stays open but **parked behind plan3/plan4**: it pays engine cost only once per run as a seed and needs no verification loop, but its premise ("search the candidate's moves first") has weaker support — the engine's first moves on stress (g3g4@10M vs b1b8@100M vs solver's d6e5-class PVs) proved unstable across budgets |
| 3 | **Clock-pressure ordering signal** | The stress win is a tempo conversion; the static scorer is blind to the clock. OR side: bonus for clock-resetting moves (pawn pushes/captures) once rule50 passes a threshold; AND side: prefer *reversible* (shuffling) defender moves — they are the actual drawing resource and where disproving work concentrates. DF-PN+ `H`/`Cost` flavor: frontier estimates scaled by remaining clock budget | unmeasured; S-effort spike (temporary counters: how often the AND refutation is a shuffle at high clock). Behavior-changing → validated like plan9 (outcomes unchanged, drift confined to repetition-heavy cases) | nodes | S–M | **closed — measured no-go, both halves** (`report4.md`, 2026-09-13): Phase 0 counter spike (`PLAN4_SPIKE=1`, reverted) on the stress case reproduced the inherited baselines exactly and showed the surface absent — at T=60, high-clock AND frames are 0.011% (FO) / 0.15% (default) of nodes with 0.002–0.02% of child evals; the selected child is already at rank 0 in 152/153 (FO) / 817/818 (default) of proven high-clock AND frames; composition is 90%+ all-shuffle so a uniform bonus cannot reorder. Clock distribution: AND expansions sit at clock 0–9 for 97–98% — the searched tree rarely sustains a high clock. The OR-side histogram (same spike) is equally empty (0.02% of OR expansions at clock ≥ 60), closing the OR half too; the DF-PN+ `H`/`Cost` clock flavor is recommended closed on the same evidence (plus the plan10 hazard class) |
| 4 | **Parallel search design spike** | `lean` backlog #2 owns the lever; this initiative tracks the *new research inputs*: Kaneko AAAI-10 (already mined, `dfpn/research_parallel.md`) is 15 years old — the 2025 paper below (massively parallel PNS, two-level parallelization + shared worker info, 333× on 1024 cores) and JLPNS (Saffidine et al., ACG 2011) supersede its assumptions. Spike question: which design fits a shared fixed-size TT with per-shard locks, and what is the determinism story for `child_eval_budget` (sequential path stays bit-identical; `--threads > 1` is an explicitly nondeterministic opt-in that must be documented as such) | 2–8× wall on multicore (multiplicative; the only lever of that size left) | wall | L (spike first) | open — owned jointly with `lean` #2 |
| 5 | **Research reading round** | Verified, not yet mined (canonical index with links: `docs/bibliography.md`): (a) *Expected Work Search* (Randall, Müller, Wei, Hayward, 2024, arXiv:2405.05594) — combines win-rate estimates with proof-size estimates, minimized expected work; solved 5×5 Go under positional superko (repetition-dominated!) and 8×8 Hex; evaluate as a child-selection/ordering paradigm against DF-PN thresholds; (b) *Multiple-Outcome Proof Number Search* (Saffidine & Cazenave, ECAI-12 — earlier misattributed to Kishimoto IJCAI-11, corrected in plan3) — formal multi-outcome framework; cross-check our draw propagation for missing machinery; (c) *Massively Parallel Proof-Number Search* (Čížek, Balko, Schmid, 2025, arXiv:2511.10339) — feeds #4; (d) Kishimoto & Müller journal version (*Information Sciences* 175(4), 2005) of the GHI paper — the AAAI-04 PDF in `dfpn/` is abbreviated; the journal version's full algorithm is the reference for `dfpn` backlog #4; (e) Gao, *On Computation Complexity of True Proof Number Search* (arXiv:2102.04907) — true pn/dn in DAGs is NP-hard, useful framing for why DAG-aware pn/dn are heuristics | information | — | S | **items (a)+(b) mined, closed** (`plan3`, `research_ews.md` + `research_mopns.md`, 2026-09-13): (a) EWS — **documented no-go**, its measured benefit requires a win-rate estimator a pure solver lacks (paper's own EWS-WR ablation is 8.5× worse; estimator-free surrogates collapse to that form), its GHI handling is caching-everything + simulation-verified reuse (evidence for `dfpn` #4/#5d, not a new cacheable surface), and the ordering-layer salvage is blocked by `lean`'s oracle floor / plan4's empty AND surface; (b) MOPNS — **closed as a valuable negative result**: the paper is Saffidine & Cazenave ECAI-12 (the "Kishimoto IJCAI-11" attribution was wrong; corrected in `docs/bibliography.md`), our `Outcome`-based draw propagation is formally the MOPNS Draw-threshold slice (`G(n,Draw)=0 ∧ S(n,Draw)=0`), MOPNS explicitly defers repetitions to GHI (no published caching-soundness argument here), and df-MOPNS threshold targeting is in the plan10 hazard class. (d) **plan5 drafted** (2026-09-13): journal-GHI reading round producing `dfpn/research_ghi_journal.md` plus a soundness-contract sketch for `dfpn` #4. Items (c)/(e) stay open with their owning backlogs (#4/#4, `conversion` #5e) |

Cross-references: `dfpn` backlog #3 (refinement after cap-cut) and #4
(bounded cross-path verification) stay open there — #1 here changes the
*cache*, #4 there changes *reuse across paths of solved results*, and the
monotonicity lemma in this initiative's #1 may end up being the soundness
argument #4 needs; do not fork the two. AND-side ordering signals in
general stay in `lean` #5; #3 here is only the clock/shuffle-specific
signal. Wall-time micro-engineering stays in `lean`.

## Non-goals

- **Re-proposing twins + simulation** (plan5 → plan7) or any
  cross-path reuse of solved **Win/Loss** facts into the search
  (plan10's measured no-go). The monotone draw cache is deliberately
  draws-only and hit-at-frame-entry for exactly this reason.
- Removing rule50 from the Zobrist key (dfpn non-goal).
- Re-opening OR-side move-ordering quality (lean non-goal; oracle-floor
  measurement).
- Unsound speed: any lever that cannot state why it never returns a
  false decisive outcome is not implementable here.

## Measurement conventions

- Primary metric: `child_evals` to first decisive outcome; wall time
  secondary.
- Stress case and baseline: see `dfpn/initiative.md` Measurement
  conventions (`make stress` FEN; post-plan9 first-outcome 249,480,478
  child evals; default mode 338,094,183). This initiative does not
  re-baseline; it inherits the dfpn numbers.
- Soundness gate: `tests/test_repetition.rs --include-ignored` green on
  every plan; plus for #1 a new property test (two solves in one
  process agree; a cache hit never flips a decisive outcome).
- Drift check: `benchmark --suite quick --json --first-outcome`;
  deviations allowed only on the surface a plan explicitly changes, as
  in `dfpn`'s working agreement.

## History

- **2026-09-13** — Initiative opened after a missed-research review
  (docs/plans/dfpn + lean read end-to-end; arXiv sweep for PNS/solving
  literature 2021–2025). Backlog #1–#5 opened; #1's monotonicity lemma
  sketched in the backlog entry (a plan must restate it formally before
  implementation).
- **2026-09-13** — `plan1.md` drafted for backlog #1 (monotone draw cache
  v2): the lemma restated formally with a proof (decisive monotonicity in
  the ancestor set and the clock), the reuse rule derived as its
  contrapositive, and a Phase 0 go/no-go spike over the inherited
  post-plan9 baselines.
- **2026-09-13** — backlog #1 **closed as a no-go** (`report1.md`): the
  Phase 0 spike (real v2 mechanism behind `CONV1_SPIKE=1` with dual v1/v2
  maps) measured the stress first-outcome case at 249,728,076 child evals
  vs the 249,480,478 inherited baseline (+0.1%, bar was ≥5% improvement);
  only 267 of 7,884 v2 hits were v2-only (plan9's exact cache already
  intercepts ~97%); clock-delta was 0 for 7,651 of 7,884 hits. m22 control
   bit-identical; quick-suite outcomes all unchanged. All spike code
   reverted (working tree byte-identical to the pre-plan1 state; fast gate
   green). Backlog #2 (candidate-guided outcome search) becomes next.
- **2026-09-13** — `plan2.md` drafted for backlog #2 after Fairy-Stockfish
  was added as a submodule (`libs/Fairy-Stockfish`, pinned
  `226c7f18`, fairy_sf_14-322). Scope decision recorded in the plan:
  verification lever only (2b) — ordering guidance (2a) is a separate lever
  under the one-lever-per-plan agreement and opens as plan3 if measurements
  justify it. Engine invocation lives in a new `sf_guide` example; the
  search CLI only gains an opt-in `--guide-moves "<uci>"` flag and never
  spawns processes. Phase 0 is a zero-production-code measurement spike
  (existing `verify_ppv` example verifies the engine line) with a hard
  go/no-go bar (stress-case total cost ≤ 50% of the inherited baseline).
- **2026-09-13** — backlog #2's **verification lever (2b) closed as a
  no-go** (`report2.md`): Phase 0 measured Fairy-Stockfish (built
  `ARCH=armv8` — the container is aarch64, not x86-64) on the stress case at
  1M/10M/100M nodes: PV lengths 1/19/17; none verified. The 10M/100M
  verifications each burned their whole 600/900 s budget (>1.0B/>1.4B child
  evals) with defender reply a5a4 left unproven — the engine's lines end in
  genuine terminal positions but earlier defenses escape within the bounded
  length (cooperative-horizon lines, the belief-vs-fact gap plan7 warned
  about, now measured at root scale). m22 control failed harder (engine
  6.5 s + 300 s unverified vs 3.1 s unguided); the 6-case dec sample
  verified 6/6 in ≤0.55 s but the 1.2–5.3 s engine query erases the gain.
  No production code existed to revert (Phase 0 used the existing binaries);
  a parallel-prototyped `src/verify.rs` extraction was also reverted to keep
  the tree pre-plan2. Ordering guidance (2a) stays open, re-ranked, and is
  the candidate plan3 — with its premise weakened by the observed
  budget-instability of the engine's root move.

- **2026-09-13** — `plan3.md` drafted for backlog #5 items (a)+(b): a
  docs-only reading round producing `research_ews.md` and
  `research_mopns.md`. Each extraction must end in a mapping section whose
  candidate mechanisms survive the initiative's structural constraint
  (change what is cacheable or where the line comes from) and the plan10
  hazard test (no decisive-fact folding into threshold arithmetic). Output
  is the re-ranked backlog, not code.
- **2026-09-13** — `plan4.md` drafted for backlog #3, scoped to the
  **AND-side clock-pressure signal only** (bonus for non-capture, non-pawn
  defender moves at `rule50 ≥ threshold`, applied in `sort_moves` next to
  history/killer). Scope Decision recorded in the plan: the OR-side
  clock-reset bonus is parked as a successor lever (lean's OR oracle-floor
  measurement means it needs its own sizing evidence); the DF-PN+
  `H`/`Cost` frontier flavor is excluded as threshold arithmetic. Phase 0
  is a counter-only spike (refutation-shuffle rank histograms at high-clock
  AND nodes) with an explicit no-go if history/killers already keep
  shuffles at rank 0–1.
- **2026-09-13** — backlog #3 **closed as a measured no-go, both halves**
  (`report4.md`): the Phase 0 counter-only spike (env-gated `PLAN4_SPIKE=1`
  instrumentation in a temporary `spike_plan4.rs`, verified non-perturbing
  by exact baseline reproduction, then fully reverted — working tree
  byte-identical to pre-plan4) failed all three go-bar criteria on the
  stress case (FO + default) and m22: high-clock AND frames are
  0.011–0.15% of nodes / 0.002–0.02% of child evals, the selected child is
  already rank 0 in 152/153 and 817/818 of proven high-clock AND frames,
  and shuffle composition is ~0.9 mean (mostly all-shuffle lists a uniform
  bonus cannot reorder). The clock histogram shows AND expansions at
  clock 0–9 for 97–98% of frames — the searched tree rarely sustains a
  high halfmove clock. The OR-side clock histogram collected by the same
  spike is equally empty (566/2.85M OR expansions at clock ≥ 60), closing
  the OR-side half as well; the DF-PN+ `H`/`Cost` clock flavor is
  recommended closed on the same evidence. Observation filed for the
  `lean` #5 lane: history/killers already serve the AND-side refutation at
  rank 0 from move one on the stress profile. No ordering term, tests, or
  drift protocol were needed (no code survived Phase 0).

- **2026-09-13** — backlog #5 items **(a)+(b) mined and closed** (plan3
  reading round; `research_ews.md` + `research_mopns.md`). Verdicts:
  **(a) EWS — documented no-go** (no new backlog item): the mechanism's
  measured benefit requires a win-rate estimator (paper's own EWS-WR
  ablation: 8.5× worse on average, »24 h on 5×5 Go vs 5.25 h); every
  estimator-free surrogate collapses to that measured-weak form; EWS
  changes neither cacheability (TT + Kishimoto–Müller
  simulation-verified reuse, exactly the `dfpn` #4/#5d lever) nor any
  threshold we run; the ordering-layer salvage is blocked by `lean`'s
  OR oracle floor and plan4's measured-empty AND surface. **(b) MOPNS —
  closed as a valuable negative result**: the solver's `Outcome`-based
  draw propagation is formally the MOPNS Draw-threshold slice
  (`G(n,Draw)=0 ∧ S(n,Draw)=0` ⇔ all-children-solved/no-win/some-draw);
  the only cacheable-surface candidate (MOPNS `pess/opti` half-facts) has
  a measured-thin stress surface (first proofs of repetition-draw chains
  complete within ~1 child eval; `dfpn/research_repetition_cache.md` §2)
  and no published soundness argument — MOPNS assumes acyclic games and
  defers cycles to the journal GHI paper; df-MOPNS threshold targeting is
  in the plan10 hazard class (same per-outcome sum/min folding). No new
  backlog items opened. **Attribution correction:** MOPNS is Saffidine &
  Cazenave, ECAI 2012, pp. 708–713 — the "Kishimoto, IJCAI-11" attribution
  in the backlog and bibliography was wrong (no such paper exists; the
  only Kishimoto IJCAI-11 entry is "Evaluations of Hash Distributed A\*").
  `docs/bibliography.md` updated (both entries **Mined**; #5d/#5e links
  kept open).

- **2026-09-13** — `plan5.md` drafted for backlog #5d: a docs-only
  reading round mining the **journal** GHI paper (Kishimoto & Müller,
  *Information Sciences* 175(4), 2005 — the complete algorithm the AAAI-04
  PDF only abbreviates). Five goal questions: the full twin/simulation
  procedure (specifically whether simulation carries the twin's original
  ancestor context — the concrete gap in `dfpn/research_ghi.md` §9), the
  journal soundness theorem's preconditions, the first-player-loss →
  repetition-as-draw mapping, the reuse-site economics against the plan10
  hazard (frame-entry return vs child-bound folding), and the
  soundness-contract sketch for `dfpn` #4 in the backlog #1 lemma shape.
  Placement decision recorded in the plan: the extraction goes to
  `docs/plans/dfpn/research_ghi_journal.md` (consumer is `dfpn` #4; direct
  continuation of `dfpn/research_ghi.md`), cross-linked from both
  initiatives. The plan ends at the contract; the `dfpn` #4 implementation
  plan is not drafted here.

Per repo convention, every plan ends with the task of writing its
`report<N>.md` in this directory.
