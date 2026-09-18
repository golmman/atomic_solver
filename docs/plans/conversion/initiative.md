# Initiative: `conversion` — solving deep tempo/progression conversions

## Status

New initiative, opened 2026-09-13 after a missed-research review triggered by
the `make stress` position remaining hard after `dfpn` plans 1–10 and the
`lean` micro-optimization rounds. Plans are single-lever, sized to one
session, agile like `dfpn`/`lean`. The next plan number is **plan6**
(plan1/plan2/plan4 closed as no-gos; plan3 was the #5a/#5b reading round —
EWS + MOPNS, both no-go; plan5 was the #5d reading round — journal GHI,
mined and closed 2026-09-13 with an evidence-based no-go for `dfpn` #4;
the ordering-guidance half of #2a
is parked behind the reading rounds, its premise weakened by report2's
budget-instability observation). Backlog #6 (threshold-cut-frame pricing)
was opened 2026-09-17, handed over from `dfpn` when that initiative was set
dormant.

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
| 5 | **Research reading round** | Verified, not yet mined (canonical index with links: `docs/bibliography.md`): (a) *Expected Work Search* (Randall, Müller, Wei, Hayward, 2024, arXiv:2405.05594) — combines win-rate estimates with proof-size estimates, minimized expected work; solved 5×5 Go under positional superko (repetition-dominated!) and 8×8 Hex; evaluate as a child-selection/ordering paradigm against DF-PN thresholds; (b) *Multiple-Outcome Proof Number Search* (Saffidine & Cazenave, ECAI-12 — earlier misattributed to Kishimoto IJCAI-11, corrected in plan3) — formal multi-outcome framework; cross-check our draw propagation for missing machinery; (c) *Massively Parallel Proof-Number Search* (Čížek, Balko, Schmid, 2025, arXiv:2511.10339) — feeds #4; (d) Kishimoto & Müller journal version (*Information Sciences* 175(4), 2005) of the GHI paper — the AAAI-04 PDF in `dfpn/` is abbreviated; the journal version's full algorithm is the reference for `dfpn` backlog #4; (e) Gao, *On Computation Complexity of True Proof Number Search* (arXiv:2102.04907) — true pn/dn in DAGs is NP-hard, useful framing for why DAG-aware pn/dn are heuristics | information | — | S | **items (a)+(b)+(d) mined, closed** ((a)+(b): `plan3`, `research_ews.md` + `research_mopns.md`, 2026-09-13; (d): `plan5`, `dfpn/research_ghi_journal.md`, 2026-09-13): (a) EWS — **documented no-go**, its measured benefit requires a win-rate estimator a pure solver lacks (paper's own EWS-WR ablation is 8.5× worse; estimator-free surrogates collapse to that form), its GHI handling is caching-everything + simulation-verified reuse (evidence for `dfpn` #4/#5d, not a new cacheable surface), and the ordering-layer salvage is blocked by `lean`'s oracle floor / plan4's empty AND surface; (b) MOPNS — **closed as a valuable negative result**: the paper is Saffidine & Cazenave ECAI-12 (the "Kishimoto IJCAI-11" attribution was wrong; corrected in `docs/bibliography.md`), our `Outcome`-based draw propagation is formally the MOPNS Draw-threshold slice (`G(n,Draw)=0 ∧ S(n,Draw)=0`), MOPNS explicitly defers repetitions to GHI (no published caching-soundness argument here), and df-MOPNS threshold targeting is in the plan10 hazard class. **(d) mined and closed** (`plan5`, `dfpn/research_ghi_journal.md`, 2026-09-13): the journal version adds the proofs (Theorems 3.1/3.2) but *not* a step-by-step simulation procedure — the twin's ancestor-context gap is in the paper's own specification; the framework maps onto repetition-as-draw only vacuously (our decisive facts are path-independent by rule; the paper has no machinery for path-dependent draws); the journal reuse path structurally excludes the plan10 hazard (node-entry return, verification-bounded volume, (1, 1) re-init) but is economically empty here (plan11 arm A ceiling −8.8% FO / −2.6% default; ~1.1-eval draw re-proofs) — **`dfpn` #4 closed as an evidence-based no-go**, the contract retained as soundness template + parallel-spike design constraint. Items (c)/(e) stay open with their owning backlogs (#4/#4, `conversion` #5e) |
| 6 | **Threshold-cut-frame pricing** (handed over from `dfpn` at its dormancy, 2026-09-17) | `lean` plan9's diagnostic: 99.7–99.9% of AND-frame own child evals sit in **threshold-cut frames** (frames exhausting their DF-PN threshold without a refutation exit: m22 8,625,570/8,645,613; stress case 148,390,289/148,602,798). Cut frames never reach a refutation exit, so no move-ordering signal can touch them; reducing the mass means changing how unsolved-subtree exploration is **priced** (DF-PN threshold dynamics), the same territory as the plan10/11 bound-folding hazard and the EWS/MOPNS no-gos | **Diagnostic spike executed** (`plan6.md` Phase 0, `report6.md`, 2026-09-18): cut mass measured at 98.9% of stress FO child evals; the three pre-registered classes failed their probes (A: 19.8%/34.7% ceiling but needs in-run resumption state = reuse-widening; B: empty — `epsilon_ceil(second) − best = 1` in 93.5% of AND cuts, ε inert; C: refuted — churn broad-shallow, top-20 positions ≈ 0.04%). **GO via a fourth class the spike surfaced**: the partial-sum sweep short-circuit (stop a cut frame's initial sweep once the summed bound crosses its threshold) measures a 74.2% first-order ceiling (FO) / 73.0% (default), present on all controls, and passes the plan10 hazard test on paper (frame-local, no solved facts, no new cache). Mechanism plan opened as **`plan7.md`** | nodes | M | **open — plan7 (partial-sum sweep short-circuit) drafted from `report6.md`; plan6 spike reverted, tree byte-identical, fast gate green** |

Cross-references: `dfpn` backlog #3 (refinement after cap-cut) **closed 2026-09-17** as
subsumed by `--refine-cap 0` (measured no-go; decision record in
`dfpn/initiative.md`), and `dfpn` was set dormant the same day — its last
live observation (threshold-cut-frame pricing) is tracked here as backlog
#6. `dfpn` #4 (bounded cross-path verification) closed as an
evidence-based no-go on 2026-09-13 (`dfpn/research_ghi_journal.md`, mined by
plan5 here); the monotonicity lemma in this initiative's #1 is the value
claim of the retained contract, which now lives on as a design constraint
for the parallel spike (#4 here / `lean` #2). AND-side ordering signals in
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

- **2026-09-13** — backlog #5 **item (d) mined and closed** (plan5 reading
  round; `dfpn/research_ghi_journal.md`; author-copy PDF vendored as
  `dfpn/ghi_journal.pdf` — located via the OpenAlex CiteSeerX location
  record preserving Müller's Alberta page URL). Verdict: the journal version
  adds the complete literature review, the df-pn pseudo-code, and the
  soundness proofs (Theorems 3.1/3.2) — but **not** the step-by-step
  simulation procedure plan5 expected: simulation carries neither the twin's
  ancestor set nor a failure-side re-search, and the theorems *assume* a
  sound verification oracle, so `dfpn/research_ghi.md` §9's gap is in the
  primary source's specification, not only in our port. Repetition-as-draw
  mapping: decisive facts are path-independent by our rule (stronger than
  the paper's first-player-loss, where disproofs still need twins), so the
  twin machinery is never needed for them; the paper offers nothing for our
  actual path-dependent object (draw proofs). Plan10 hazard: the journal
  reuse path structurally excludes it (node-entry return, verification-
  bounded volume, (1, 1) base re-init) — but those same properties make it
  economically empty (plan11 arm A is its unverified strict superset at
  −8.8% FO / −2.6% default; draw-side re-proofs cost ~1.1 evals each).
  **`dfpn` #4 closed as an evidence-based no-go**; the soundness contract
  (plan1's monotonicity lemma as value claim, frame-entry reuse direction)
  is retained as the soundness-argument template and as a design constraint
  for the parallel spike (#4 here / `lean` #2), where cross-worker contexts
  make the journal mechanism load-bearing. Bonus correction:
  `dfpn/research_ghi.md` §4.2's root-threshold claim was inverted (both
  papers: modified scheme initializes root thresholds to ∞ − 1). Items (c)
  and (e) remain open.

- **2026-09-17** — **backlog #6 opened; handover from `dfpn`** (no code,
  docs only): with `dfpn` #3 closed (subsumed by `--refine-cap 0`) and its
  backlog empty, the initiative was set dormant and its one live lever —
  the threshold-cut-frame pricing observation from the `lean` plan9 spike
  (99.7–99.9% of AND-frame own child evals in threshold-cut frames) — moved
  here as backlog #6. Scope guard recorded in the row: diagnostic-first
  Phase 0; any candidate pricing mechanism must pass the plan10 hazard test
  (no solved-fact folding into unsolved parents' threshold arithmetic) and
  must not widen the reuse envelope (plan1's closed lane). The item is
  class-native here: the diagnostic was measured on m22 and the stress
  case, this initiative's objects.

- **2026-09-18** — `plan6.md` drafted for backlog #6, scoped (per the plan2/
  plan4 scope-decision precedent) to **Phase 0 only**: a counter-only,
  env-gated (`CONV6_SPIKE=1`) diagnostic spike, fully reverted after
  measuring, with the mechanism implementation explicitly deferred to plan7
  on a go. Phase 0 step 0 re-baselines at HEAD (the inherited post-plan9
  stress numbers predate `dfpn` plans 12–13); the diagnostic battery covers
  frame-exit classification with cut-gap anatomy (lean report9's method,
  delta-0 attribution check), a per-position churn-vs-frontier map,
  chunk-boundary waste, ε-sensitivity at cut exits, and the OR/AND split —
  shaped to discriminate the three candidate pricing classes (A chunk
  resumption, B threshold-growth shaping, C per-frame work allocation), each
  pre-screened against the plan10 hazard (no solved-fact folding) and plan1's
  closed reuse lane. Go bar: a measured addressable surface ≥ 10% of stress
  FO child evals plus an eval-count-deterministic, hazard-passing mechanism
  sketch confirmed on the m22 control.

- **2026-09-18** — **plan6 Phase 0 executed; GO via a new mechanism class; plan7 opened**
  (`report6.md`). Step 0 re-baselined at HEAD: stress FO 249,480,478 /
  default 338,094,183 / m22 14,156,269 / m24 406,737 — all four reproduce
  the inherited numbers exactly (the plan's stated "m24" FEN was actually
  `m23_white` per the fixture; both measured). The `CONV6_SPIKE=1`
  counter-only spike (temporary `spike6.rs` + hooks + `solve_stats`
  runner, delta-0 attribution verified on every run, decision-identity
  spike-on and spike-off) measured the cut-frame anatomy: 98.9% of stress
  FO child evals sit in threshold-cut frames whose own evals are 89–98%
  initial-sweep work; 55.5% of all evals are spent at positions entered
  exactly once; ε is inert (`eps_step = 1` in 93.5% of AND cuts); churn is
  broad-shallow (hottest position 7,722 evals). Verdict per the plan's go
  bars: classes A (chunk resumption, 19.8%/34.7% ceiling but
  reuse-widening), B (threshold-growth shaping, empty surface), and C
  (per-frame work allocation, no concentration) all failed; a fourth class
  the gap histograms surfaced — the **partial-sum sweep short-circuit** —
  measured a 74.2% (FO) / 73.0% (default) first-order addressable surface
  with 72.5–74.0% on every control, and passes the plan10 hazard test on
  paper (frame-local state, no solved-fact folding, no new cache,
  eval-count-deterministic). All spike code reverted (tree byte-identical;
  post-revert m22 stdout md5 `ea72f7ea…` matches report9; `make test`
  green). `plan7.md` drafted from the report's design brief (gated
  implementation spike with a ≥10%-realized go bar; productionization only
  on a go). Raw artifacts: `docs/plans/conversion/measurements/plan6/`.

Per repo convention, every plan ends with the task of writing its
`report<N>.md` in this directory.
