# Plan 13: Bounded-search horizon/threshold pricing — gated small-space pre-phase

Initiative: `dfpn`, successor lever of backlog #5 (recorded in the backlog
row's closure entry and `report12.md` "Next steps"; plan13 was named as the
candidate subject in `initiative.md` Status on 2026-09-15). Prerequisite
reading: `plan12.md`, `report12_phase0.md`, `report12.md`,
`initiative.md` (plan5→plan7 twin history, plan10/plan11 no-gos, lean9
AND-cut diagnostic).

## Problem

plan12 measured **why** the solver fails the KQvK ladder
(`8/2K5/k7/8/8/8/8/4Q3 w - - 0 1`, win in 15, >756M nodes unproven) and the
answer is not repetition semantics:

- The repetition machinery is inert (0% path-repetition frame exits, ≤0.66%
  plan9-cache hits, class-1 mass ≈ 0.00%).
- The dominant mass (91.3% class-3) is **unsolved frontier churn**: 79% of
  bounded-DFPN frames are depth-0 leaves, ~99% of child evals are unsolved,
  and **zero** frames are TT-resolved — the bounded search re-walks the
  horizon hundreds of times without ever collapsing a node.
- Meanwhile the plan12 T1a **fixpoint over the reachable region decides all
  ~420k positions, certificate and all, in ~6 s** — table-free. The T1b
  forward prover (same repetition semantics, subset-transfer memo) needed
  1.02B nodes *with* table guidance.

The lever: give the solver a **bounded pre-phase whose pricing does not
depend on DF-PN threshold arithmetic**, so that a small decisive region is
resolved by fixpoint/proof-number dynamics instead of being churned by
threshold-cut frames. This is the same disease lean9 measured on the big
class (99.7–99.9% of AND-frame own evals in threshold-cut frames); the
ladder is where it is provable in isolation and where the alternative
architecture has a measured ~50× cost advantage.

Scope decision (explicit): this plan attacks the class behind a
**small-space detector gate** — the pre-phase must be a no-op on ordinary
positions. A general change to DF-PN threshold/pricing dynamics (which would
touch the stress case and the whole quick suite) is a different, much
riskier lever; this plan only records what the bake-off teaches about it.

## Why this is not plan10/plan11/plan12-ArmC again

- **plan10/plan11** died reusing *solved facts across TT keys* (solved-child
  bounds folding into unsolved parents' thresholds). This plan's pre-phase
  decides the **root** and hands back outcome + PV; round 1 stores nothing
  into the TT and adopts nothing from it. The hazard is excluded by
  construction, not by gating.
- **plan12 Arm C** was a cycle-free bounded pre-phase and failed its own
  precondition: T1b's prover cost ≈ the solver's failing cost. This plan
  attacks exactly that cost — the Phase 0 bake-off asks whether a
  **table-free** prover with better pricing (proof-number search, layered
  certificate reuse, or closure enumeration) reaches plan12's success
  criterion (root ≤ 20M child evals / ≤ 60 s). If no candidate does, the
  plan closes as a measured no-go and the class stays closed.

## Phase 0 — architecture bake-off (temporary example binaries, no `src/` changes)

Pattern of plan12's Session A: instruments live under `examples/` (or a
temp runner), archived under `measurements/plan13/`, reverted, `src/`
byte-identical. All candidates are **table-free** — no egtb table, no
external oracle. The egtb q-table and the archived
`measurements/plan12/plan12_t1.rs` are used only as *validation oracles*,
never as search input. This is the hard lesson from T1b: 1.02B nodes
*with* monotonicity pruning against the table proves nothing about a
solver-integrable pre-phase.

### T0 — baseline confirmation (light)

The plan12 T0 numbers are the baseline of record; re-confirm only that the
post-revert clean build still reproduces them (steps 1–3 bit-for-bit via
the search CLI; root bounded-15 as a band plus exact child evals). No
re-derivation. Recorded baselines (128 MB TT, default ε):

| FEN | outcome | metric (plan12 T0) |
| --- | --- | --- |
| root `8/2K5/k7/…/4Q3 w - - 0 1` | unproven | 5,396,315,458 child evals (default, 600 s); bounded-15: 2,832,550,718 / Timeout |
| step1 `8/1k1K4/…/4Q3 w - - 2 2` | Win in 13 | 322,826,558 child evals / 36.0 s |
| step2 `8/8/2k1K3/…/4Q3 w - - 4 3` | Win in 11 | 39,383,178 child evals / 5.2 s |
| step3 `8/5K2/…/4Q3 w - - 6 4` | Win in 9 | 2,047,932 child evals / 0.24 s |

### T1 — three pre-registered candidate architectures

Implement each as a standalone prover sharing the solver's terminal
classification and real `atomic_movegen` movegen (the plan12 T1 pattern;
`plan12_t1.rs` is the starting point). All three must enforce the solver's
path-repetition semantics (a position recurring on the current line is a
Draw for the attacker — the T1b cut) and produce, for every WIN claim, a
**replay-verifiable certificate**: the rank-decreasing winning strategy
whose full line enumeration is cycle-free and mates within the bound
(T1a machinery). A claim without a replayable certificate is not a result.

- **P — PN bounded prover.** Proof-number/disproof-number search to a fixed
  depth horizon (start: dtm, dtm+2), repetition cut on the path,
  subset-transfer memo (WIN under ancestor set A is valid under A′ ⊆ A;
  NOT_WIN under A valid under A′ ⊇ A — T1b's lemmas, re-stated in the
  source). PN ordering replaces both DF-PN thresholds and the table
  guidance T1b leaned on. Hypothesis under test: PN's least-work-first
  pricing avoids the poisoned-line re-verification that made T1b
  depth-sensitive (depth dtm+1/dtm+2 hit the cap).
- **L — layered certificate fixpoint.** Iterated depth layers with a
  persistent fact set: a cycle-free WIN proven at layer d is
  horizon-independent (its line never repeats a board), so it stays valid
  at every layer d′ > d and shrinks the unknown mass monotonically. Within
  a layer, order by PN (or the solver's `StaticAtomicScorer`), never by a
  table. Hypothesis under test: persistence across layers removes T1b's
  re-exploration of already-proven subtrees without needing subset tests
  at all (facts are stored certificate-first, not bound-first).
- **R — region-closure fixpoint.** Forward-enumerate the region reachable
  from the root within a position budget (~420k for the root; abort above
  it), then run the T1a-style fixpoint (terminal-initiated relabeling to
  exact ranks over the closure). Ranks strictly decreasing ⇒ the extracted
  strategy is cycle-free by construction. Hypothesis under test: the ~6 s
  T1a measurement was not an artifact of starting from the table — the
  closure itself is the cheap object, and a position-budget-gated closure
  is the whole pre-phase.

Measurement per candidate × ladder position: child evals (primary, per the
initiative's metric convention), nodes, wall, decided outcome, PV length
vs exact dtm (15/13/11/9 — known from T1a/T1b, usable as a correctness
cross-check), and certificate verification time. Runs at horizon
dtm − 1 (must be unknown), dtm, and dtm + 2 (must not explode).

### Gates

- **G-A (architecture, go):** at least one candidate decides the root as a
  certified WIN at ≤ **20M child evals** and ≤ **60 s** wall, table-free,
  with all four ladder positions decided consistently and dtm-matching PV
  lengths. (This is plan12's success criterion, inherited unchanged.)
- **G-B (soundness, hard):** every WIN claim's certificate replays
  cycle-free; a sampled oracle cross-check (≥ 200 KQvK placements against
  `egtb3-q.bin`, plan12 R8 hygiene: illegal placements excluded, timeout
  = unknown, never a proven Draw) shows zero contradictions on the
  spike binary.
- **No-go:** no candidate within 3× of G-A's cost bar → close the plan
  with the measured architecture costs (this would also be strong evidence
  that the ladder class needs a fundamentally different proof technology,
  e.g. retrograde tablebases at solve time — recorded, not pursued here);
  or a candidate passes G-A but certificates cannot be made replay-sound →
  close on the plan5→plan7 principle (a lever that cannot state why it
  never returns a false decisive outcome is not implementable).

## Pre-implementation refinement (resolved at plan review)

All five questions are resolved. One is a **[decided]** binding choice (it
changes a public contract — changing it later costs a plan); four are
**[assumption]** — informed, cheaply reversible before Phase 1 lands, and
explicitly veto-able at the Session A checkpoint (plan12's R1–R8 pattern,
with the reversibility stated per item).

### R1 — PV labeling for certificate wins [decided: new `PvStatus` variant]

One correction to the draft's premise first: a certificate-derived PV has
provably minimal length **only when the architecture produces exact ranks**(region-closure does; a PN proof shows "win within bound", not minimality) —so the label must not hard-code shortestness.

- (a) Reuse `PvStatus::ProvenShortest`. Pro: zero public-API churn; correct
  about length whenever the certificate carries exact ranks. Con: conflates
  two proof technologies under one flag; *wrong* for bound-only
certificates (PN), where "proven shortest" would be a false claim — and a
  false claim in the one direction this project never accepts (correctness
  first). Also destroys the flag's existing discriminating power: consumers
  can no longer ask whether the refinement loop itself terminated
  naturally.
- (b) New variant — **CHOSEN: `PvStatus::PreflightProof`** (final name at
  implementation; doc: PV extracted from a replay-verified pre-phase
  certificate — a winning, cycle-free line; length ≤ horizon bound,
  minimal only if the certificate carries exact ranks). Pro: honest under
  every architecture, keeps `ProvenShortest`'s documented meaning exactly,
  and the blast radius is measured: `PvStatus` lives in
  `src/search/dfpn/mod.rs` + `src/main.rs` only — it appears in no spec,
  no example, no benchmark JSON. Con: one more public enum variant and one
  more `pv_status:` string; exhaustive external matches would need a new
  arm (acceptable: the flag's own doc says variants may exist for exactly
  such qualification).

### R2 — `ProofEvent` emission from the pre-phase [assumption: none in round 1, gap documented]

- (a) Emit nothing — **CHOSEN (assumption).** The proof pipeline is
  TT-driven offline (search → TT snapshot → `reconstruct_pt`); a pre-phase
  proof never enters the TT, so reconstruction cannot reproduce it
  *regardless*. Synthesizing live `NodeProven` events would create a class
  of positions whose live-built tree is structurally more complete than
  anything the offline pipeline can rebuild — an asymmetry with no consumer
  in round 1. Accepted cost: the "every proven node emits an event"
  property documented for `dfpn` does not extend to the pre-phase; the
  module doc and AGENTS.md architecture note must say so explicitly.
- (b) Synthesize events from the certificate strategy. Pro: live
  worker-based consumers (tests, examples) see a complete tree; the event
  invariant stays uniform. Con: a producer for data nothing consumes yet,
  touching the proof-tree contract.

Reversibility: (b) is purely additive — a later plan can add event
synthesis without re-architecting anything, which is why this can stay an
assumption.

### R3 — detector predicate [assumption: occupied ≤ 3 men, no pawns, no castling rights]

- Round-1 class: 3-man piece sets (KQ/KR/KB/KN vs K). The measured KQvK
  region is ~420k positions, and the same bound order holds for the other
  3-man sets. Pawns are excluded: promotions and clock resets break the
  monotone closure and explode the region. Castling rights excluded: extra
  state for a region that cannot occur in the round-1 class.
- Safety margin: the report7 cyclic-rook safe-area position is 4 men, so
  the detector never fires on the regression class; defense-in-depth is
  the certificate replay — a cyclic position cannot yield a cycle-free
  certificate, so even a misgate degrades to "deferred", never to a false
  decisive claim.
- A popcount-4 tier (KRvK+P endings, etc.) with a region-budget guard is a
  plausible round-2 extension; deliberately not round 1.

Reversibility: the predicate is one function; widening it later is a
one-line change plus region measurements.

### R4 — budget accounting [assumption: pre-phase consumes the deterministic budget]

- Pre-phase child evals count against the same `child_eval_budget` and are
  included in the run's eval counters — the benchmark metric must see
  pre-phase work or comparisons become dishonest. On exhaustion: defer;
  the normal search then immediately reports
  `ExitReason::BudgetExhausted`, so the documented contract (AGENTS.md)
  holds verbatim. Wall-clock timeout stays global; the pre-phase's own cap
  is the eval budget (deterministic), never a wall-clock check.
- Alternative considered and rejected: a separate pre-phase budget. It
  splits the metric, weakens the determinism story, and makes
  "pre-phase + search ≤ budget" unenforceable.

Reversibility: low-stakes either way; changing the accounting later does
not touch outcomes, only counters.

### R5 — CLI surface and integration point [assumption]

- CLI: automatic when the detector fires; `--no-preflight` as the A/B
  escape hatch (added to `-h`, unknown-option handling unchanged); one
  `preflight: decided|deferred reason=… evals=…` line in non-
  `--outcome-only` mode (same spirit as `pre_exit:`). No changes to the
  summary format or to `docs/spec/optimizer_interface.md` — benchmark JSON
  keys are untouched.
- Integration point: the pre-phase hooks both `Search::solve` and
  `Search::search_depth` before the DF-PN loop. Under `solve` it runs
  iterative bounds with its own eval cap; under `search_depth` the fixed
  `max_depth` is the bound (plan12's T0 bounded run is then reproducible
  with the pre-phase in play, which is what the ladder gates measure).

Reversibility: both are surface plumbing; moving the hook or the flag
later is mechanical.

## Phase 1 — implementation (only on G-A ∧ G-B, one selected architecture)

- New module under `src/search/` (name per the winning architecture, e.g.
  `preflight.rs`); sizes per AGENTS.md (split before 10 KB).
- Integration point: a root-only pre-phase invoked from `Search::solve`
  and `Search::search_depth` *before* the DF-PN loop (per R5). On a
  certified decision it returns outcome + certificate PV directly; on
  anything else (detector off, budget exhausted, region too large,
  unknown) it defers and the existing search runs **bit-identically**.
- No TT interaction of any kind: no stores, no probes, no bound folding.
  The pre-phase's internal memo is run-local and dropped after the call
  (the repetition-cache precedent: only work, never outcomes, crosses
  runs — here not even that).
- The certificate verifier is a separate, self-contained function with
  unit tests including negative cases (corrupted rank, injected cycle ⇒
  verification fails, claim downgraded to deferred).
- Validation: `cargo fmt` / `clippy` / fast gate;
  `test_repetition --include-ignored` (cyclic rook must stay Draw);
  drift protocol `benchmark --suite quick --json --first-outcome`
  byte-identical (detector never fires on the suite — verify by counter);
  m22_white and the stress case within 1.0× child evals (they must be
  exactly baseline; the pre-phase is a no-op for them); the KQvK oracle
  sample of G-B re-run on the release binary; `make test-full` before
  closing. A ladder-root integration test is added
  `#[ignore = "slow: ..."]` per the tier conventions (fixture-file
  additions remain a plan-level decision; not triggered in round 1 since
  the slow test covers it).
- If the selected architecture misses its Phase 0 numbers in integration
  by a wide margin, close as measured no-go — do not iterate into a second
  architecture in the same session (plan11's rule).

## Execution: two sessions, checkpoint between

- **Session A — Phase 0 only** (T0 confirmation + T1 bake-off). No `src/`
  changes even on early go. Deliverable: `report13_phase0.md` with the
  per-candidate cost table, G-A/G-B outcomes, the architecture
  recommendation, raw logs + archived sources under
  `measurements/plan13/`, spike reverted (`src/` byte-identical), and the
  R1–R5 assumption overrides, if any. Update the initiative History line.
- **Checkpoint (user):** confirm architecture; veto any R1–R5 assumption
  (the one binding decision, R1, changes only on explicit override). The
  no-go branches of G-A/G-B terminate the plan here.
- **Session B — implementation** per Phase 1 and the review decisions,
  full validation, final `report13.md` (subsumes the Phase 0 report),
  backlog re-rank, History update. Per repo convention, the plan's final
  task is the report.

## Goal / success criteria

- Ladder root proven Win, certified cycle-free, at ≤ 20M child evals and
  ≤ 60 s (from >756M nodes / 600 s unproven); steps 1–3 improve ≥ 10× in
  child evals to first outcome (plan12's criterion, unchanged).
- Byte-identical behavior wherever the detector does not fire (quick
  suite, m22, stress case, all existing tests).
- Zero false decisive outcomes: certificates replay-verified; oracle
  sample clean; `test_repetition` gates pass.

## Constraints

- Table-free search: the egtb table and `plan12_t1.rs` are validation
  oracles only; a pre-phase that needs external tables is not integrable
  and would make the G-A measurement meaningless.
- No re-proposal of the plan5 twin/simulation machinery; no TT reuse of
  pre-phase facts (round 1); rule50 stays in the Zobrist key; the
  deterministic budget contract holds as documented.
- Phase 0 instrumentation is temporary (examples / temp runners), archived
  under `measurements/plan13/`, and the Phase 1 diff is clean of spike code.
- New public API prefers full words over abbreviations (AGENTS.md
  conventions); `src/` files under the sizing rules.

## Final task

Write `docs/plans/dfpn/report13.md`: T0 confirmation, the bake-off table
(child evals / nodes / wall / outcome / PV-vs-dtm per candidate × ladder
position), G-A/G-B outcomes, the implemented architecture and its
integration numbers vs the Phase 0 prediction, soundness argument as
implemented (certificate verifier + detector + budget accounting), the
R1–R5 resolutions as implemented, deviations, problems, missing tests, and next steps
(including what the bake-off taught about general DF-PN threshold pricing
— the lean9-class lever — and whether a popcount-4 tier or closure draw
proofs are worth a follow-up item). Update the initiative's History and
the backlog/status rows.
