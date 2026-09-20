# Plan 4: Depth- and clock-scheduled ε POC (threshold-schedule heterogeneity)

Initiative: `research` backlog #11 (POC candidate). This is the documented
NO-GO fallback of plan3 (`report3.md`): the mid-search recognizer direction
is closed for the outlier family, and #11 is the highest-priority surviving
generic lever.

## Goal

Measure whether scheduling the DF-PN+ `1 + ε` threshold factor by *where the
frame sits in the search* — path depth from the root, remaining `max_depth`,
or the rule50 clock — reduces first-outcome `child_evals` on the hard class,
where a single *global* ε was measured inert. The report answers backlog
#11: **is the threshold-cut churn mass (plan1: ~80% of frame evals) homogeneous
in its threshold response, or do different regions of the searched tree want
different ε?**

This is a pure-POC plan: throwaway, env-gated instrumentation, reverted
before the report. No production change is made here; a GO result is handed
off to `conversion` as a sized backlog item.

## Context

Three prior results shape this plan:

1. **Global ε was measured inert — but on a noisy, pre-plan9, suite-aggregate
   sweep.** The 2026-09-11 measurement (`ultimattt/report2.md`) swept
   ε ∈ {0, 0.0625, 0.125, 0.25, 0.5} with `--timeout 5` over the default
   suite: total child_evals spread only ~1.4% (33.36M–33.84M), but the totals
   were dominated by two timeout positions whose counts are wall-clock noise,
   not search determinism. There is **no clean per-case first-outcome ε sweep
   on the stress case or m22_white anywhere in the record.** The inert claim
   must be re-established on the gate object before any scheduling conclusion.
2. **The threshold channel is high-leverage in this solver.** `dfpn` plan10/11
   measured threshold-adjacent arithmetic swings of −37.7% and +110% on the
   same cases (cross-clock solved-entry reuse and its decoupling arms). ε
   itself is a *different* knob — it scales the second-child threshold
   (`epsilon_ceil`, `src/search/dfpn/core.rs:274/282`, Pawlewicz & Lew 2007,
   `dfpn/research_epsilon.md`) — but the precedent shows small arithmetic
   changes on this surface are not second-order.
3. **plan1 located the churn.** ~80% of frame-eval mass sits in
   threshold-cut frames; ε governs exactly the re-search growth of that mass
   (`O(threshold)` → `O(log threshold)` re-searches at a node). plan3 showed
   the mass lives at ≥9 men, i.e. in deep, material-rich frames — but said
   nothing about its *depth or clock distribution*.

The scheduling hypothesis is that the optimal ε is not constant across the
tree: near the root (large thresholds, wide fan-out) coarser thresholds may
cut re-search churn, while near the refinement horizon (small thresholds)
coarse ε may overshoot bounds and force re-proofs; long capture-free
conversion stretches (rule50 climbing) inflate bounds with tempo work that a
coarser threshold could absorb. Whether *any* of these gradients exists is
unmeasured — and the global sweep cannot detect it: a schedule that helps in
one region and hurts in another nets to zero globally, which is consistent
with (1).

## Hypotheses

- **H1 (heterogeneous response)**: at least one scheduling variable (path
  depth, remaining depth, rule50 clock) partitions the churn mass into
  regions with materially different optimal ε; some arm beats every global ε
  by ≥10% on the stress case at control parity.
- **H2 (homogeneous response)**: the churn mass responds flatly to ε
  everywhere — every global value and every schedule lands within a few
  percent of the default. Backlog #11 closes with data, and the threshold
  surface is marked exhausted (further ε work would need a *new* mechanism,
  not a new constant).

## Method

### Phase 0 — clean global ε sweep (no code changes)

The CLI already exposes `--epsilon`. Run the gate object and control
first-outcome, one ε per run:

```bash
cargo build --release

# stress case = m21_white (baseline: 249,480,478 child evals, default ε=0.125)
for eps in 0 0.0625 0.125 0.25 0.5; do
  target/release/atomic_solver \
    --fen "4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21" \
    --timeout 300 --first-outcome --outcome-only --epsilon $eps
done

# m22_white control (baseline: 14,156,269 child evals)
for eps in 0 0.0625 0.125 0.25 0.5; do
  target/release/atomic_solver \
    --fen "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22" \
    --timeout 30 --first-outcome --outcome-only --epsilon $eps
done
```

Outcomes and PV lengths must be identical across all runs (ε is
soundness-neutral; only the search trajectory may move). If any single
global ε already clears the GO gate on the stress case, the scheduling
question is moot — record the constant as the finding and skip Phase 1.

### Phase 1 — env-gated scheduled-ε spike (`dfpn` plan11 spike pattern)

Temporary, default-off, env-gated arm selection (`RESEARCH4_ARM`,
`RESEARCH4_SPIKE_OFF=1`); unset ⇒ bit-identical to HEAD. All spike code in
one `src/search/dfpn/spike4.rs` module plus the two-line call-site hook,
removed before the report.

**Hook site**: the two `epsilon_ceil` calls in `core.rs` (OR second-child
pn threshold, AND second-child dn threshold). A scheduled variant
`epsilon_ceil_sched(x, num, den)` reuses the existing u128
multiply/div_ceil arithmetic with an arm-computed fraction
(`fraction_from_f64`, already available); the global path keeps
`self.epsilon_num/den` untouched.

**Schedule inputs** (all frame-local and deterministic — no wall clock, no
TT interaction, no `ProofEvent` changes, so search determinism and the
soundness contract are untouched):

- `max_depth` — already a frame parameter (remaining depth).
- `path_depth` = `root_max_depth − max_depth`; the spike stores
  `root_max_depth` on `Search` at each refinement round start (temporary
  field; `max_depth_reached` exists but is set post-round).
- `rule50` — `pos.board().rule50()` at the frame's position.

**Arms** (default ε for the unscheduled region is always 0.125):

| Arm | Schedule | Rationale |
|---|---|---|
| `D_ROOT` | ε = 0.5 while `path_depth < 8`, else 0.125 | Wide fan-out near the root: absorb re-search churn coarsely early |
| `D_LEAF` | ε = 0.5 while `max_depth < 16`, else 0.125 | Near the refinement horizon thresholds are small; test the opposite gradient |
| `R50` | ε = 0.5 while `rule50 ≥ 60`, else 0.125 | Deep capture-free conversion stretches: bounds inflated by tempo work |
| `LINEAR` | ε = min(1.0, 0.125 · (1 + path_depth/16)) | Smooth version of `D_ROOT`; tests whether the step schedules' boundaries matter |

Known interaction, documented up front: `max_depth` and `path_depth` vary
across iterative-deepening rounds at the same position, so depth schedules
are round-dependent. That is inherent to the lever (a production version
would face the same), deterministic, and therefore measurable.

**Run matrix** per arm (and once with the spike off, as the bit-identical
control): stress case (timeout 300), m22_white (30), plus two
structurally-diverse decisive-suite outliers as regression controls — dec13
(`r1bq1k1r/ppN4p/n1p1p3/3p1n1P/1b1P2P1/2P5/PP6/RNBQKB1R w KQ - 1 14`,
timeout 30) and dec10 (FEN via the `tests/fixtures/` name→FEN lookup, plan2
step 3; timeout 60).

**Per-run invariants**: outcome and PV length identical across arms on every
case (any divergence = soundness or timeout red flag — with 53.7 s actual
vs 300 s budget on the stress case there is ample headroom, so an outcome
flip indicates a spike bug, not a property of the schedule). With
`RESEARCH4_ARM` unset, quick-suite `benchmark --suite quick --json
--first-outcome` must be bit-identical per case to a clean HEAD baseline.

### Step 3 — revert

- Remove `spike4.rs`, both call-site hooks, the temporary
  `root_max_depth` field, and all arm parsing; `git diff --exit-code src/`
  must pass before the report is written.
- `cargo fmt --check`, `cargo clippy --release --all-targets`, `make test`.
- Re-run one global-ε stress run post-revert to confirm the Phase 0 numbers
  reproduce.

## Decision gates

| Gate | Criterion (stress case, first-outcome `child_evals`) | Consequence |
|---|---|---|
| **GO** | best arm ≥10% below the 249,480,478 baseline, m22_white and dec13 within +5%, all outcomes unchanged | Hand off to `conversion` as a sized backlog item (the winning schedule or constant); spike code and per-arm tables archived under `measurements/plan4/` as the sizing evidence. |
| **PARTIAL** | best arm 3–10% below baseline, no control regression | Record the measured gradient; weigh a `conversion` hand-off against the remaining literature targets (#5 child-level early termination is the top open item). |
| **NO-GO** | best global ε and best arm <3% below baseline, or any control regression | Close backlog #11 with data: the threshold surface is response-flat on the hard class; next plan is literature target #5 (child-level early termination in DF-PN), which attacks the same churn mass from the child-granularity side. |

The stress case is the gate object; m22_white and dec13 are regression
controls (both must stay within gate bounds for a GO).

## Deliverables

- `docs/plans/research/measurements/plan4/` — Phase 0 sweep table, per-arm
  raw outputs, derived child-eval table (arms × cases).
- `docs/plans/research/report4.md` — tables, hypothesis verdict, gate
  decision, hand-off or closure record.

## Verification

- Spike-off bit-identity: quick suite per-case identical to clean HEAD.
- Outcome/PV invariants across arms (above).
- Post-revert: `git diff --exit-code src/`, `cargo fmt --check`,
  `cargo clippy --release --all-targets`, `make test`, and one stress-case
  run reproducing the 249,480,478 baseline.

## Non-goals

- No production ε/schedule change, no new CLI flags, no benchmark-suite
  changes.
- No refinement-cap scheduling (the other half of backlog #11's phrasing):
  cap scheduling acts across iterative rounds, a different surface; if a GO
  schedules by depth, a follow-up plan may test the cap analog — one lever
  per plan.
- No work on #12 (TT eviction) or #13 (frontier prior) — both remain
  pre-weakened per plan3's context.
- No proof-tree, `ProofEvent`, or optimizer-interface changes.

## Final task

Write `docs/plans/research/report4.md` (Phase 0 + arm tables under
`measurements/plan4/`, the gate decision, and the hand-off or closure
record), and update the backlog row for #11 and the History in
`initiative.md`.
