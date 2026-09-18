# Plan 7: Partial-sum sweep short-circuit at threshold cuts

Initiative: `conversion` backlog #6 (threshold-cut-frame pricing). Direct
successor of `plan6.md`/`report6.md`: the Phase 0 diagnostic spike measured
a 74.2% first-order addressable surface (stress FO child evals) for exactly
one mechanism class and closed the others (A: measured but reuse-widening;
B: empty — ε is inert, `epsilon_ceil(second) − best = 1` in 93.5% of AND
cuts; C: refuted — churn is broad-shallow, top-20 positions ≈ 0.04% of
evals). Prerequisite reading: `report6.md` (the anatomy and the design
brief this plan implements), `dfpn/report10.md` (the folding hazard this
mechanism must structurally avoid), `conversion/report1.md` (the closed
reuse-widening lane), and `src/search/dfpn/children.rs`
(`evaluate_all_children`, whose existing decisive-child early exit this
mechanism generalizes).

## Scope decision

**One lever**: the partial-sum sweep short-circuit at summed-bound
thresholds, implemented behind an env gate (`CONV7_SPIKE=1`) first and
promoted to default behavior only if the Phase 1 bars clear. No other
threshold, ordering, caching, or budget change. Phase 2 (productionization)
happens in this plan only on a clear Phase 1 go; otherwise the spike is
reverted and backlog #6 closes on the combined plan6/plan7 evidence.

## Mechanism

At a `dfpn` frame, the initial child sweep (`evaluate_all_children`)
currently evaluates *every* child before selection computes the frame's
aggregate bounds. But a threshold-cut frame never descends — it burns its
full sweep (13.6 evals per AND cut frame on the stress case; 89–98% of
cut-frame own evals are sweep evals) and then exits because its summed
bound crossed the threshold. The summed bound is a monotone running
quantity:

- AND frame (defender to move): `pn = Σ child.pn` (saturating). Once the
  running Σ `child.pn` reaches `th_pn`, the final sum must also reach it
  (remaining children can only add). The frame may stop the sweep there and
  exit through the existing threshold-cut path.
- OR frame (attacker to move): `dn = Σ child.dn` (saturating). Symmetric
  with `th_dn`.

Implementation sketch:

1. Thread `th_pn`/`th_dn` (and `is_or_node`, already present) into
   `evaluate_all_children`. Inside the sweep loop, accumulate the running
   sum (AND: `child.pn`; OR: `child.dn`, saturating) and `break` when it
   reaches the corresponding threshold (finite only; `INF` thresholds never
   cut, as today).
2. On the early stop, the frame proceeds exactly like today's threshold
   cut: `select_from_children` over the partial table, store unsolved
   bounds `(partial_sum.max(1), …)`, `best_move`/`best_child`/depth from
   the partial table, `Outcome::Draw` return, no `NodeProven` emission.
   Children after the crossing point are absent from the table — code
   iterating the table must not assume the full move list (audit
   `store_best_child` indexing and the proof-tree path tracking).
3. The re-evaluation loop is unchanged: a re-entered frame re-sweeps the
   missing children (TT-resolved children cost 1 eval each) and the
   `explored`-marking path handles unchanged bounds as today. Re-entered
   frames start from their stored partial bound, which grows monotonically.
4. Gate: the mechanism is compiled unconditionally but enabled via
   `CONV7_SPIKE=1` in Phase 1 (default off ⇒ bit-identical, verified by the
   anchor protocol); Phase 2 flips the default and removes the gate.

## Soundness contract (normative)

- **No solved-fact folding (plan10 hazard).** The crossing test reads only
  the frame's own children bounds and its own thresholds. Solved outcomes
  never enter unsolved parents' threshold arithmetic through this
  mechanism; the early cut returns an unsolved bound, never an outcome.
- **No reuse widening (plan1 lane).** No new cache, key, context, or
  adoption rule. All state is frame-local, recomputed per entry, discarded
  at exit. TT interaction is exactly today's (same stores, same keys).
- **Bounds stay valid lower estimates.** A partial sum over a subset of
  children is a valid underestimate of the true summed bound; DF-PN stores
  lower estimates for unsolved nodes and grows them monotonically, so
  convergence semantics are preserved. The one asymmetry (an AND frame's
  stored `dn` = min over the evaluated subset may overestimate the true
  min) affects guidance only — no outcome is ever derived from bounds.
- **No repetition-semantics change.** `suppress_draw`, the repetition
  cache, and the path-repetition terminal checks are untouched. The
  early-cut bound is stored with the same TT-store path as today's cut.
- **Deterministic budget contract.** The crossing test is a pure function
  of deterministic child bounds; no wall-clock input. The mechanism only
  reduces evals consumed per frame; `child_eval_budget` /
  `ExitReason::BudgetExhausted` and the refinement caps keep their exact
  semantics. No new `ProofEvent`s (Draw cut frames emit none).
- **Preflight untouched.** The pre-phase (plan13) is upstream of the loop
  and unaffected; its soundness contract (no TT interaction, no wall
  clock, no events) is unchanged.

## Phase 1 — gated measurement spike (go/no-go)

Implement the mechanism behind `CONV7_SPIKE=1` (default off), plus a
temporary counter dump (pattern: `spike6.rs`/`spike_plan4.rs`) reporting
per-run: early-cut frames, evals saved in-sweep, extra re-entry evals at
early-cut positions, and the bound-weakening distribution (partial vs
full-sum gap). Then measure, with the spike-off identity check first
(stress FO, m22, m24 reproduce HEAD bit-for-bit):

- stress FO and default (primary metric: `child_evals`; wall secondary)
- m22 control (destabilization canary, plan10 lineage)
- m24/m23 small-class points
- `benchmark --suite quick --json --first-outcome` drift
  (all outcomes unchanged; work deltas expected *by design* on every
  position the mechanism touches — the drift protocol's "surface a plan
  explicitly changes" clause covers the early-cut surface; `wrong=false`
  everywhere is the hard gate)
- `cargo test --release --test test_repetition -- --include-ignored`
  (cyclic rook stays a draw)

**GO to Phase 2** iff all of:

1. Stress FO net saving ≥ 10% child evals (the first-order ceiling is
   74.2%; anything under 10% realized means the re-entry/bound-weakening
   offsets dominate — close as no-go).
2. Stress default net saving ≥ 5% (the shipped mode; `make stress` runs
   default).
3. m22 control: win found, wall time ≤ 2× baseline (no plan10-style
   destabilization; the control's early-cut surface is 72.5%, so a *gain*
   is expected, but the bar is a non-regression guard).
4. Quick suite: 59/59 outcomes unchanged; no case regresses > 2× evals.
5. Cyclic-rook repetition gate green.

**NO-GO** otherwise: revert everything (tree byte-identical to HEAD), close
backlog #6 citing report6 (surface exists but the realized mechanism
offsets it) + report7 (measured offsets), and re-rank the initiative's
remaining levers (#4 parallel, jointly owned with `lean` #2).

## Phase 2 — productionization (only on a Phase 1 go)

- Remove the env gate (mechanism becomes unconditional), delete the spike
  counters, keep the code path minimal (the crossing test lives in the
  sweep loop; no allocation, no extra TT probes).
- `cargo fmt`, `cargo clippy`, `cargo doc` clean; unit tests in
  `children.rs`/`tests.rs` for: crossing at the first child, crossing never
  firing on INF thresholds, AND vs OR sides, re-entry after early cut
  resumes from the stored partial bound, early cut never stores a solved
  outcome, `best_child` indexing over the partial table. Integration:
  quick-suite outcome stability test (existing) stays green.
- Update `AGENTS.md`'s `dfpn` paragraph (one sentence on the sweep
  short-circuit) and this initiative's `initiative.md`/`report7.md`.
- Deliverable: `report7.md` in this directory — Phase 1 tables (net
  savings, controls, drift), the production diff summary, and the
  backlog-#6 closure or hand-off decision.
