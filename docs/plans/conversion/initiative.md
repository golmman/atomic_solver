# Initiative: `conversion` — solving deep tempo/progression conversions

## Status

New initiative, opened 2026-09-13 after a missed-research review triggered by
the `make stress` position remaining hard after `dfpn` plans 1–10 and the
`lean` micro-optimization rounds. Plans are single-lever, sized to one
session, agile like `dfpn`/`lean`. The next plan number is **plan1**.

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
| 1 | **Monotone draw cache v2** (superset-context, cross-clock, draws-only) | Extend plan9's repetition cache: key on the board-only `repetition_key` (not the clock-qualified hash); store `(proven rule50 clock, complete ancestor rep-key set)`. Reuse iff probe clock ≥ stored clock **and** probe ancestor set ⊇ stored set. Soundness lemma: under the two-fold-repetition-as-draw semantics, solved value is monotone — Win and Loss are downward-closed in both the ancestor set and the clock (a Win/Loss proof is structurally repetition-free and clock-independent except that smaller clocks can only *add* budget), hence Draw at a stored (A, c) is Draw at any (A′ ⊇ A, c′ ≥ c). Draws-only means the cache can never fold solved (0, INF)/(INF, 0) bounds into unsolved parents' thresholds — the exact defect mechanism that killed plan10 (report10) — because hits return at frame entry like path-repetition terminals, never as child bounds | plan10 measured −37.7%/−41.6% for full cross-clock reuse (no-go only because of Win/Loss folding); plan9's exact-context cache got −29%; v2 targets the *remaining* re-descent churn (contexts that differ by unrelated earlier excursions, and clock-drift revisits of the same board). Unmeasured — Phase 0 spike required | nodes | M | planned — `plan1.md` (2026-09-13) |
| 2 | **Candidate-guided outcome search** (engine-hybrid) | Use a strong atomic engine (fairy-stockfish with NNUE) as a line oracle: (a) *ordering guidance* — seed `sort_moves`/killer/history so the candidate line's moves are searched first (zero soundness risk: ordering only); (b) *verification mode* — prove the candidate line as a PPV first (`search_depth_with_prefix` + the `verify_ppv` machinery already exist); only if verification fails, fall back to the unguided solve. Literature analog: Kawano simulation at root scale; standard practice in tsume-shogi solving (engine-guided DF-PN) | unmeasured. The first-outcome phase's cost is mostly exploring *wrong* subtrees; a correct candidate line can collapse it to near-verification cost. Also attacks the 65%-of-default-mode refinement tail: the engine's line is a short-PV candidate before any bounded refinement round runs | nodes + behavior (new mode; drift protocol N/A for the mode, ordering-guidance variant must keep quick-suite outcomes) | M–L | open |
| 3 | **Clock-pressure ordering signal** | The stress win is a tempo conversion; the static scorer is blind to the clock. OR side: bonus for clock-resetting moves (pawn pushes/captures) once rule50 passes a threshold; AND side: prefer *reversible* (shuffling) defender moves — they are the actual drawing resource and where disproving work concentrates. DF-PN+ `H`/`Cost` flavor: frontier estimates scaled by remaining clock budget | unmeasured; S-effort spike (temporary counters: how often the AND refutation is a shuffle at high clock). Behavior-changing → validated like plan9 (outcomes unchanged, drift confined to repetition-heavy cases) | nodes | S–M | open |
| 4 | **Parallel search design spike** | `lean` backlog #2 owns the lever; this initiative tracks the *new research inputs*: Kaneko AAAI-10 (already mined, `dfpn/research_parallel.md`) is 15 years old — the 2025 paper below (massively parallel PNS, two-level parallelization + shared worker info, 333× on 1024 cores) and JLPNS (Saffidine et al., ACG 2011) supersede its assumptions. Spike question: which design fits a shared fixed-size TT with per-shard locks, and what is the determinism story for `child_eval_budget` (sequential path stays bit-identical; `--threads > 1` is an explicitly nondeterministic opt-in that must be documented as such) | 2–8× wall on multicore (multiplicative; the only lever of that size left) | wall | L (spike first) | open — owned jointly with `lean` #2 |
| 5 | **Research reading round** | Verified, not yet mined (canonical index with links: `docs/bibliography.md`): (a) *Expected Work Search* (Randall, Müller, Wei, Hayward, 2024, arXiv:2405.05594) — combines win-rate estimates with proof-size estimates, minimized expected work; solved 5×5 Go under positional superko (repetition-dominated!) and 8×8 Hex; evaluate as a child-selection/ordering paradigm against DF-PN thresholds; (b) *Multiple-Outcome Proof Number Search* (Kishimoto, IJCAI-11) — formal 3-outcome framework; cross-check our draw propagation for missing machinery; (c) *Massively Parallel Proof-Number Search* (Čížek, Balko, Schmid, 2025, arXiv:2511.10339) — feeds #4; (d) Kishimoto & Müller journal version (*Information Sciences* 175(4), 2005) of the GHI paper — the AAAI-04 PDF in `dfpn/` is abbreviated; the journal version's full algorithm is the reference for `dfpn` backlog #4; (e) Gao, *On Computation Complexity of True Proof Number Search* (arXiv:2102.04907) — true pn/dn in DAGs is NP-hard, useful framing for why DAG-aware pn/dn are heuristics | information | — | S | open |

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

Per repo convention, every plan ends with the task of writing its
`report<N>.md` in this directory.
