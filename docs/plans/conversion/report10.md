# Report 10: Oracle-path ε — plan10 execution (backlog #8)

Executed `plan10.md` (backlog #8, oracle-path ε — exogenous threshold
conditioning). **Verdict: NO-GO.** The exogenous-conditioning leg of the ε
surface is closed (fifth closure leg). Question B (plan11, external PV
fidelity) is **not draftable**.

## Execution summary

All plan steps ran to completion in one session (resumed after a
mid-session crash that lost only the `/tmp` prespike baseline build — the
workspace spike, oracle PVs, and measurement state survived; the prespike
baseline was rebuilt from HEAD and re-verified byte-identical before any
arm was read):

- **Step 0** — oracle PVs captured for all 5 cases with the post-plan9
  release binary (`--first-outcome --timeout 300`, 128 MB TT). Files under
  `measurements/plan10/oracle_pv/`. Validation: `verify_ppv` reports
  `is_ppv: true` for m22_white/dec13/dec01; stress and dec10 are
  first-outcome (not shortest-DTM) lines, so the plan's pre-authorized
  fallback applied — all 5 lines replay cleanly via `examples/replay` and
  end in a terminal `Loss` for the defending side (outcome assertion
  passed for the full set).
- **Step 1** — env-gated spike (`CONV10_GUIDE`/`CONV10_PAD`/`CONV10_STATS`)
  implemented exactly as planned: guide replay into a Zobrist-key set
  (root included), OR-site-only conditioning (`Conv10Pad::Factor` as exact
  integer fractions / `Conv10Pad::ParentThreshold`), stats counters
  (on-path OR sites, clamps, cut-backs, frame-local child evals). AND site
  untouched; `child_eval_budget`, repetition handling, and TT semantics
  untouched.
- **Step 2** — full 7-arm × 5-case matrix (35 runs, raw transcripts in
  `measurements/plan10/runs/` — gitignored; parsed results in
  `measurements/plan10/results.json`).
- **Step 3** — gates evaluated below.

## Step-0 reproduction (H3)

All five baseline FO child-eval counts reproduce report9's numbers exactly
on this host (spike binary, `CONV10_STATS=1`, no guide/pad — pure
diagnostics, zero behavior change):

| case | BASE child_evals | report9 baseline | match |
|---|---:|---:|---|
| stress | 249,480,478 | 249,480,478 | ✓ |
| m22_white | 14,156,269 | 14,156,269 | ✓ |
| dec13 | 3,822,602 | 3,822,602 | ✓ |
| dec01 | 5,713,706 | 5,713,706 | ✓ |
| dec10 | 4,262,128 | 4,262,128 | ✓ |

No host/config drift; the gate precondition held.

## Hygiene gates

- **H1a** — `benchmark --suite quick --json --first-outcome`, spike build
  env-unset vs prespike binary: all 59 cases identical on every
  deterministic field including `child_evals`; the only differing keys are
  wall-clock `time_*`/`total_time` (inherent timing noise). PASS.
- **H1b** — stress FO stdout byte-identical between the prespike binary and
  the env-unset spike build. PASS.
- **H2** — HYGIENE arm (guide + pad=0.125) byte-identical to BASE on all
  five cases (stdout and `total_child_evals`). PASS. Guide loading and the
  instrumentation introduce no search deviation.
- **S1/S3** — every finishing arm reports `win` on expectation; zero
  `wrong` claims anywhere; censored runs are clean timeout-draws (empty
  PV); stress never flips outcome on a finishing arm. PASS.
- **S2** — `cargo test --release --test test_repetition -- --include-ignored`
  green on the spike build (env unset). PASS.

## Phase-0 diagnostics (HYGIENE counters; pad-independent)

| case | on-path OR sites | clamps | cut-backs | frame child evals | total child evals |
|---|---:|---:|---:|---:|---:|
| stress | 4,663 | 2,171 (46.6%) | 3,174 | 281,162,706 | 249,480,478 |
| m22_white | 2,395 | 381 (15.9%) | 2,033 | 18,000,183 | 14,156,269 |
| dec13 | 435 | 20 (4.6%) | 383 | 4,116,639 | 3,822,602 |
| dec01 | 1,070 | 435 (40.7%) | 975 | 10,428,679 | 5,713,706 |
| dec10 | 60,660 | 60,144 (99.2%) | 55,163 | 9,227,110 | 4,262,128 |

- **dec10's on-path surface is structurally thin**: 99.2% of on-path OR
  sites are clamps (`ε(second) ≥ th` — padding would not engage). No pad
  arm could move that case materially; its small deltas come from
  trajectory perturbation alone. This is exactly the plan's
  "harvestable surface structurally thin" reading, now measured.
- **stress** has the healthiest engagement profile (46.6% clamps, 3,174
  cut-backs) — the mechanism had real surface there and still lost.

## Arm matrix (child_evals; FO, ε=0.125 base, 128 MB TT, timeout 300 s)

Δ vs BASE. Censored = timeout (plan8 `AND_TIGHT` convention).

| arm | stress | m22_white | dec13 | dec01 | dec10 |
|---|---:|---:|---:|---:|---:|
| BASE | 249,480,478 | 14,156,269 | 3,822,602 | 5,713,706 | 4,262,128 |
| HYGIENE | = BASE (H2) | = BASE | = BASE | = BASE | = BASE |
| P050 | 254,016,727 (+1.8%) | 17,577,251 (+24.2%) | 3,482,492 (−8.9%) | 16,255,078 (+184.5%) | 6,274,277 (+47.2%) |
| P100 | 525,164,346 (+110.5%) | 20,498,491 (+44.8%) | 4,030,917 (+5.4%) | 23,908,386 (+318.5%) | 6,749,706 (+58.4%) |
| PINF | draw @2.50B (censored) | 1,024,094,190 (+7134.8%) | 256,112,991 (+6599.6%) | draw @2.51B (censored) | 8,966,705 (+110.4%) |
| G050 | 156,394,866 (−37.3%) | 15,952,085 (+12.7%) | 5,827,389 (+52.5%) | 7,742,095 (+35.5%) | 3,273,293 (−23.2%) |
| G100 | draw @2.87B (censored) | 51,843,616 (+266.2%) | 7,947,572 (+107.9%) | draw @2.71B (censored) | 2,367,895 (−44.5%) |

All finishing arms report `win`.

## Gate verdict

**NO-GO**, on every pre-registered clause:

1. **Stress ≥10% reduction fails**: the best pad arm (P050) is *+1.8%* on
   the gate object; P100 is +110.5%; PINF does not finish.
2. **Marginal-value clause fails decisively**: the equal-value global
   control beats every pad arm on stress — G050 −37.3% vs P050 +1.8% —
   and G050 is also the *only* arm improving stress at all. Path
   machinery buys nothing the closed global constant does not; it is
   strictly worse here.
3. PARTIAL is unreachable (it presumes a ≥10% stress improvement).

**Interpretation (per the plan's guidance):**

- **PINF ≈ much worse than BASE (censored on stress and dec01, 65–7100×
  elsewhere)**: report9's load-bearing claim is *confirmed and priced* —
  sibling exploration under the oracle line is not overhead, it is where
  the TT/repetition-cache warming happens. Committing frames to the
  known-winning child removes that warming and the search pays for it
  many-fold. This is the strongest possible negative, and it retroactively
  explains report9's mechanism diagnosis: the 80.9% sibling mass is
  load-bearing *because of* threshold pacing, not despite it.
- The two pad arms' mild, non-monotone effects across the small cases
  (dec13 −8.9% at P050, +5.4% at P100) are consistent with the trajectory
  chaos caveat (research plan4/plan8): any perturbation reshuffles the
  trajectory, and the reshuffle is directionless at this scale.
- G050's stress improvement (−37.3%, the first stress improvement recorded
  by any ε arm) is a *global-constant* observation on the already-closed
  #7 surface — worth a line, not a reopening: #7 was closed on
  default-mode regressions at 0.375 (report8), and G050 here is FO-only
  with two censored control runs (G100) showing the same
  FO-vs-default divergence that closed #7. It does suggest stress is
  unusually ε-sensitive; if a future initiative re-opens the ε surface,
  stress should be its primary case.

## Question B (plan11)

**Not draftable.** With a *perfect, free* oracle the lever is a no-go on
the gate object; an external PV (Fairy-Stockfish MultiPV, proofdb seeding)
is a strictly weaker guide carrying question B's own fidelity confound.
Per the plan's two-question decomposition, a negative on A makes B moot.

## Additional tools / notes

- `examples/verify_ppv` (validation), `examples/replay` (the plan's
  fallback outcome assertion — its FEN+UCI-replay+solve semantics matched
  exactly).
- The prespike baseline was rebuilt from HEAD after the crash; the
  copied `target/release` artifacts made the rebuild ~8 s instead of the
  original ~80 s.
- Spike hygiene held end-to-end: with `CONV10_*` unset the build is
  bit-identical everywhere measured; post-revert stress stdout is
  byte-identical to the pre-spike binary (`runs/postrevert_stress.out` vs
  `runs/h1b_stress_prespike.out`). `make test` green on the reverted tree.
  The spike diff is fully reverted; no product code landed.

## Unresolved parts / next steps

- Backlog #8 closed no-go in `initiative.md`; the `conversion` initiative
  now has only #5(e) (item (e) reading) open.
- The ε surface is closed on five legs (constant, path schedule, regional
  structure, node-local signals, exogenous/oracle conditioning). The
  remaining recorded leads are outside the ε surface: `parallel`
  (wall-clock), `lean` #10, and the `structural_floor.md` record.
- No missing tests: the spike was env-gated and reverted; S2 covers the
  repetition-sensitive soundness surface. `make test-full` not required
  (no product code changed).
