# Plan 8 — Salt-seeded process portfolio at gate grade (default-inclusive racers, take-first decisive)

**Status: READY (owner-directed re-open, 2026-10-11) — pre-registered before
any race run.** Executes the `parallel` backlog item **#5** (diversified
portfolio re-run at gate grade), the direct successor of the plan7 portfolio
and of `reexamination.md` §1/§5 idea A as amended by its addendum. Final
task: `report8.md`.

## Motivation

Plan7 measured the process portfolio NO-GO, but its portfolio was not
diversified (`--refine-cap` is inert under `--first-outcome`; P0/P1/P2 were
the identical computation) and the re-examination's diversified ε race
(0.63×/0.75×/0.91×) is a single-draw result — *unjudged*, not a win
(addendum item 1). Since then the record has moved decisively:

- All ε/history/clock-budget *config* diversity arms are closed at gate
  grade (research plan13/plan17/plan18 — REJECT as defaults; racer
  relevance formally unjudged but magnitudes only ever measured
  single-draw).
- The single-core competitor (#20 salted TT-keeping restarts) is closed
  no-go (research plan20 Phase-0 PIVOT), dissolving the addendum's
  sequencing rule — there is no cheaper harvest-left-of-the-same-tail
  lever left to compare against.
- The `--salt` TT bucket-index salt is a **shipped product option**
  (research plan11/12): semantics-neutral, per (case, salt) deterministic
  to the bit, salt-0 bit-identical to shipped behavior.
- The pinned gate (`research/gate_methodology.md` v1.0) is normative for
  any lever measurement and retires single-draw comparisons; its plan12
  rollout (22 cases × 6 salts) **is** the per-salt single-run distribution.

This plan therefore re-tests the portfolio question with the only
diversity source that survived: **K concurrent sequential solver processes
racing at different `--salt` values**, first decisive wins. Zero product
code — each racer *is* the unmodified solver; soundness is inherited per
process (the plan7 property), and nondeterminism is confined to *which*
valid proof wins (the 2026-10-02 premise).

## The pre-computation (what is already measured)

A salt racer's trajectory does not depend on concurrency: private TT, no
shared state, deterministic per (case, salt) (plan12: reruns reproduce
bit-for-bit). Hence each racer's `child_evals` equals its solo plan12
value, and the portfolio's evals-space result — min over racer salts — is
**computable from the committed plan12 rollout before any race**
(`state/runs.json`). Derived table (default-inclusive racers, evals):

| case (plan12) | salt-0 evals | per-salt draws | basin count | portfolio min vs salt-0 |
| --- | --- | --- | --- | --- |
| m20_white | **censored @ 2 B** | s0,s1 censored; s2 1.384 B, s3 0.862 B, s4 0.912 B, s5 0.791 B | 4 | **censored → win** (s5 0.791 B, 93.7 s solo) |
| dec10 | 4,262,128 | 2.92–4.80 M, all uncensored | 4 | **0.685×** (s2) |
| dec01 | 5,713,706 | 5.52–6.90 M | 6 | 0.967× (s3) |
| m23_white | 9,673,403 | 9.44–12.06 M | 5 | 0.976× (s2) |
| stress | 249,480,478 | s0 249.5 M, s1 758 M, s5 252 M; s2/s3/s4 censored @ 2.5 B | 3 | 1.000× (salt-0 is the min draw) |
| m22_white | 14,156,269 | 14.2–23.7 M | 6 | 1.000× |
| dec13 | 3,822,602 | 3.82 M / 5.03 M × 5 | 2 | 1.000× |
| dec14 | 2,397,489 | 2.40–2.50 M | 3 | 1.000× |
| 14 remaining cases | — | salt-invariant (1 basin) | 1 | 1.000× — racers are the *same computation* |

Two structural consequences, pre-registered:

1. **Pointwise dominance in evals space.** Any portfolio that includes the
   salt-0 racer satisfies `portfolio_evals ≤ salt0_evals` on every case
   (min includes the default draw). The lever cannot lose in evals space;
   the only loss channels are *wall* (4-way contention slowing the winning
   racer) and *CPU cost* (K× core-seconds).
2. **The evals-space campaign is already run.** What remains unmeasured is
   exactly: (a) wall reality under 4-way concurrency (contention tax),
   (b) `W_first` on the conversion case, (c) out-of-sample validity
   (rem12), (d) the ship-shape decision. This plan measures only that.

The predicted win surface is the AGENTS.md priority-1 surface the
re-examination §4 said the old gates missed: **m20_white is a shipped
timeout** (2 B budget-censored, ~340 s, no answer) that a portfolio
containing salt 3 or 5 converts into a proven win at ≤ 0.7 B evals
(~94–104 s solo). Stress, m22_white, dec13, dec14 sit on their lucky
salt-0 draw — the portfolio matches, never regresses (in evals).

## Gate adaptation (declared, with data)

The pinned v1.0 comparison rule pairs `candidate_s / baseline_s` per salt.
A portfolio candidate is not indexed by a single salt, so the pairing
structure cannot be applied verbatim. Adaptation (stated per the
methodology's revision clause, justified by the plan12 distribution data):

- **Candidate statistic per case:** `W_first` = wall of the first decisive
  racer in the race; `evals_first` = that racer's `child_evals`;
  `evals_min` = min over decisive racers (deterministic metric).
- **Baseline statistic per case:** fresh interleaved solo salt-0 runs
  (wall + evals), plus the frozen plan12 per-salt distribution as the
  six-draw reference (single-draw denominators are retired; salt-0 happens
  to be the *best or near-best* draw on stress/m22/dec13 — the plan12 data
  is the correction, not the fresh median).
- **Verdict categories** map 1:1 onto the pinned rule: censored-win
  (candidate solves where the shipped salt-0 run is budget-censored, never
  the reverse), censored-loss (mirror), win/regression via the
  `W_LO`/`W_HI` + direction-agreement thresholds applied to
  `W_first / baseline_wall`, unclear otherwise.
- **Diversity clause** applied as pinned: basin counts of the plan12
  baseline distribution and the racer-set's basin coverage are reported
  next to every verdict. The pre-registered racer set is basin-crossing on
  every raced case (see racer table).

All pinned soundness invariants apply unchanged (salt-0 identity, zero
decisive-outcome conflicts across salts, censored runs never read as
draws). Budgets, censoring rules, and the under-budgeted rule are the
pinned ones.

## Hypotheses (pre-registered, mechanical)

- **H1 (conversion, priority-1; the adopt trigger).** On m20_white, the
  portfolio returns a decisive `win` where the shipped default run is
  budget-censored: every race has ≥ 1 uncensored decisive racer, and
  median `W_first` ≤ 160 s (salt-5 solo 93.7 s × ≤ 1.7 contention
  allowance; the shipped run burns ≥ 300 s and returns nothing).
- **H2 (predicted gains replicate in wall).** On dec10, median `W_first`
  ≤ 0.80× the interleaved fresh salt-0 baseline median (evals prediction
  0.685×; wall tracks evals). On m23_white, ≤ 1.00× (prediction 0.976× —
  within noise; recorded, not gated).
- **H3 (no wall regression anywhere; the pinned no-regression clause).**
  On stress, m22_white, dec13: median `W_first` ≤ 1.10× the interleaved
  baseline median (evals-space prediction is exactly 1.0×; the margin
  covers contention only).
- **H4 (fidelity, hard gate; halt-and-diagnose on failure).**
  (a) Stage-0 solo identity: re-run salt 0 and salt 2 on m22_white, dec10,
  stress — `child_evals`, outcome, and stdout match the plan12 record
  exactly. (b) In-race: every racer that finishes decisively at a salt
  with a plan12 anchor reproduces that anchor's evals exactly (determinism
  survives concurrency). (c) Zero decisive-outcome conflicts across racers
  on any case. (d) Budget-censored racers are recorded as censored, never
  as proven draws; a wall-killed racer is recorded as killed, its evals
  excluded from verdicts.

**Verdict rule.** *adopt* ⇔ H1 ∧ H3 ∧ H4 (the pinned adopt shape: a
censored-win on a hard-class case, no regression/censored-loss, soundness
clean). *reject* ⇔ H3 fails on any case, or any H4 violation (H4 ⇒ soundness
defect → halt per invariant 2, not a lever verdict). *defer* ⇔ m20_white
racers all censored at the race wall cap (under-budgeted rule: raise the
pre-registered cap once or move the case to the unresolved list — it
contributes no win and no regression). H2 failing while H1 ∧ H3 hold is a
**contention anomaly**: adopt stands, the report must diagnose why wall did
not track evals before any ship-shape decision.

## Fixed parameters

Corpus: the frozen plan12 22-case set, raced subset pre-registered below
(extensions only; the 14 salt-invariant cases and dec01/dec14 are *not*
raced — racing a ≤ 1 s or draw-identical case measures contention noise
only; their portfolio results are the pre-computed table, recorded in the
report from plan12 data).

| case | FEN (plan12 `driver.py`) | racer evals budget | race wall cap |
| --- | --- | --- | --- |
| m20_white | (plan12 CASES) | 2 B | 500 s |
| stress | (plan12 CASES) | 2.5 B | 450 s |
| dec10 | (plan12 CASES) | 1 B | 120 s |
| m22_white | (plan12 CASES) | 1 B | 60 s |
| dec13 | (plan12 CASES) | 1 B | 60 s |
| rem12 (extension, out of sample) | `rnbqkbnr/8/6pp/pppppp1B/3PPP2/N5PN/PPP4P/R1BQ1RK1 w kq - 0 10` | 1 B | 120 s |

Racer portfolio — fixed before any race, chosen from the plan12 data
(in-sample selection, acknowledged; rem12 is the out-of-sample check;
no post-selection swaps):

| racer | `--salt` | rationale |
| --- | --- | --- |
| R0 | 0 | the shipped default — pointwise-dominance anchor |
| R1 | 2 | dec10 winner basin; m20 uncensored; m23 winner basin |
| R2 | 3 | m20 uncensored (0.862 B); dec01 winner basin |
| R3 | 5 | m20 winner basin (0.791 B); stress second basin (252 M) |

Salt 1 excluded: dominated on every multi-basin corpus case (m20 censored;
stress 758 M; m22/m23 worst-or-tied; dec10 beaten by s2) — recorded
in-sample justification. All racers: default config, TT 128 MB, `--budget
<case cap>`, `--first-outcome --outcome-only`, `--timeout <race wall cap>`.
4 racers = the container's 4 CPUs (concurrency is real, per convention N ≤ 4);
4 × 128 MB TT = 512 MB fits the RAM contract.

Selection-bias caveat, stated: the racer set was picked knowing the plan12
per-salt table; the honest defense is (i) the pointwise-dominance
structure (salt 0 aboard) makes losses structural in evals regardless of
selection, and (ii) rem12 is measured out of sample. The reexamination's
ε = 0.5 selection-bias caveat does not arise: salts are not claimed as
"good configs", only as distinct draws.

## Stages

1. **Stage 0 — identity gate (cheap).** Clean tree (`git diff --exit-code`),
   release build. Solo runs: (m22_white, s0), (m22_white, s2), (dec10, s0),
   (dec10, s2), (stress, s0) — evals/outcome/stdout must match the plan12
   record exactly (H4a). Any mismatch aborts.
2. **Stage 1 — pre-computation freeze.** `derive_portfolio8.py` recomputes
   the evals-space portfolio table from
   `../../research/measurements/plan12/state/runs.json` →
   `state/derived_evals.json`. This freezes the predictions above before
   any race; the report compares prediction vs measurement.
3. **Stage 2 — rem12 baseline extension.** 6 solo runs (salts 0–5, 1 B)
   → per-salt distribution, basin count, censoring pattern. If rem12 turns
   out salt-invariant, it is recorded as such and stays only as a
   no-regression control (no signal possible — stated, not a failure).
4. **Stage 3 — pilot (1 race, dec10).** Harness sanity: 4 racers launch,
   fit memory, terminate ≤ cap, `W_first`/evals extraction works, machine
   otherwise idle. Campaign not started until clean.
5. **Stage 4 — campaign.** R = 3 interleaved rounds per case; per round:
   (a) one fresh solo salt-0 baseline (machine exclusive), then (b) one
   4-racer race. Per-racer record: wall, outcome, `child_evals`,
   censored/wall-killed flag. After the first decisive finish, remaining
   racers run until a 240 s grace elapses or their own cap, then are
   killed and recorded as wall-killed (their evals excluded from verdicts;
   H4b applies only to decisive finishers). Pre-registered extension
   trigger: if within-case round spread of `W_first` exceeds ±20 %, run 3
   more rounds for that case (contention variance, not trajectory
   variance — within-salt reruns are bit-identical). Estimated compute:
   ≈ 3 × (500 + 340) s (m20) + 3 × (450 + 31) s (stress) + small cases ≈
   1–1.5 h worst case.
6. **Stage 5 — analysis.** `analyze_portfolio8.py`: per case, baseline
   (fresh median + plan12 distribution) vs `W_first`/`evals_first`/
   `evals_min`, full per-round distributions, basin counts + racer-set
   basin coverage, H1–H4 evaluated mechanically, CPU cost = Σ racer
   core-seconds per answer (and vs the shipped run's core-seconds on
   m20_white). Results → `state/races8.json` + `README.md` provenance
   (drivers, env, parsed results committed; raw transcripts never).
7. **Final task — report.** `report8.md`: verdict against the pre-registered
   rule, prediction-vs-measurement table, cost accounting, deviations;
   update `parallel/initiative.md` backlog #5 and the
   `docs/plans/README.md` `parallel` row. On adopt, the report records the
   ship-shape decision space (documented operator pattern vs an
   `examples/portfolio` driver vs a product flag) — it does not implement
   one; on reject/defer, the closure follows the verdict mechanically.

## Scope boundary

- Product solver unchanged: `--salt`/`--budget` are shipped options;
  sequential path untouched by construction (no `src/` change; stage 0 is
  an identity check, not a drift gate — there is nothing to drift).
- All artifacts under `docs/plans/parallel/measurements/plan8/`.
- One machine, 4 CPUs: races monopolize the container; nothing else runs
  during a race (plan7 convention).
- The other reexamination ideas (cooperative shared-facts portfolio #6,
  any-worker-can-finish SPDFPN #7, Kaneko root threads #8) are **not** in
  scope; #5 gates their sequencing — if #5 adopts, the addendum's
  draw-luck regime is confirmed and #6/#7 pre-registrations read it; if it
  rejects at gate grade, the portfolio family (plan7 + plan8) is closed
  end-to-end.

## Deviation policy

Mid-campaign mechanism, racer-set, or cap changes are forbidden; anything
discovered goes into `report8.md` as a finding, and any follow-up is a new
plan with its own pre-registration under the pinned gate.
