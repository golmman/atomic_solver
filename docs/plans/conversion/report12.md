# Report 12: Restart policy vs. hard work budget

Initiative: `conversion`, backlog #9. Plan: `plan12.md` (executed
2026-10-27). Gate: `make test` / `make test-full` (search-semantics change).

## Verdicts (pre-registered)

- **D1 — keep the shipped schedule (S0).** No non-shipped arm passes all four
  bars. The full grid is in `measurements/plan12/results/grid.json`.
- **D2 — ship the plumbing split.** The unbudgeted path is bit-identical,
  budget-mode contracts hold, wall is within +2%, the `max_work`/budget
  entanglement is gone, and both test tiers are green.

Both outcomes were the pre-registered expectations (D1 "keep S0", D2
"ship"). The session answers the item's question: the restart schedule is a
load-bearing, TT-size-dependent diversification policy worth keeping as an
explicit named policy; the hard budget no longer rides on it.

## Phase 0 — identity anchors

All anchors reproduced **exactly** at HEAD (`8d711b0`, shipped schedule,
release build). No stop-and-investigate events.

| case | tt 256 | tt 128 |
| --- | --- | --- |
| stress FO `4r2k/.../1R4RK w - - 0 21` | 287,872,755 win 147 | 305,404,068 win 669 |
| stress default (60 s) | 502,873,436 win, `cap-cut` | 431,483,928 win, `cap-cut` |
| m22 FO | 15,023,911 win 75 | 19,111,111 win 301 |
| m23_white FO | 11,621,376 win 27 | 9,553,618 win 65 |
| dec31 FO | 510,426 loss 22 | 510,426 loss 22 |
| `benchmark --suite quick --first-outcome --timeout 3` | 42,109,768, 59/59 | 40,042,745, 59/59 |
| m22 default (tt 256, 20 s) | 18,779,890, `cap-cut` | — |

`test_trajectory_golden --include-ignored` green before the grid. S0 in the
grid reproduced every Phase 0 cell exactly, proving the env gate itself was
neutral. The gate lived only in an isolated copy of HEAD; the product tree
was never patched, so no gate revert was needed (the copy was deleted after
the run).

## Phase 1 — grid and D1

Arms: S0 shipped, S1 no restart, S2 single restart, S3 early-restart only
(cutoff 5M), S4 ×1.25, S5 ×4. Values are `child_evals`; `D@t` means a
timeout-censored `Draw` at `t` seconds. Quick is `total evals, ok/59`.

### FO single cases

| arm | stress 256 | stress 128 | m22 256 | m22 128 | m23 256 | m23 128 | dec31 256 | dec31 128 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| S0 | 287.9M win | 305.4M win | 15.02M win | 19.11M win | 11.62M win | 9.55M win | 0.51M loss | 0.51M loss |
| S1 | 227.3M win | D@200 1,966M | 19.38M win | 18.26M win | D@60 643.8M | 11.31M win | 141.85M loss | 141.71M loss |
| S2 | 320.3M win | 436.6M win | 57.73M win | D@60 731.6M | 11.37M win | 8.89M win | 0.51M loss | 0.51M loss |
| S3 | 309.0M win | D@200 3,138M | 15.02M win | 19.11M win | 11.62M win | 9.55M win | 0.51M loss | 0.51M loss |
| S4 | 243.3M win | D@200 1,779M | 24.07M win | 17.79M win | 11.46M win | 11.70M win | 0.51M loss | 0.51M loss |
| S5 | 274.0M win | D@200 2,549M | 19.73M win | 17.05M win | 11.25M win | 11.56M win | 0.51M loss | 0.51M loss |

### Default / quick suites

| arm | stress default 256 | stress default 128 | quick 256 | quick 128 |
| --- | --- | --- | --- | --- |
| S0 | 502.9M win | 431.5M win | 42.11M 59/59 | 40.04M 59/59 |
| S1 | 284.1M win | D@60 601.6M | 103.88M 57/59 | 84.56M 58/59 |
| S2 | 400.4M win | 545.8M win | 42.32M 59/59 | 39.83M 59/59 |
| S3 | 386.3M win | D@60 886.0M | 42.11M 59/59 | 40.04M 59/59 |
| S4 | 304.1M win | D@60 589.2M | 39.19M 59/59 | 39.43M 59/59 |
| S5 | 345.1M win | D@60 706.9M | 47.11M 59/59 | 52.86M 59/59 |

No `wrong` outcome appeared in any quick-suite cell.

### D1 against the pre-registered bars

1. **Stress FO ≥10% better at both TT sizes.** S1 improves 21.0% at tt 256
   but times out at tt 128. S4 improves 15.5% at tt 256 but times out at
   tt 128. All others are worse at tt 256 or time out at tt 128. **Fail.**
2. **No outcome change.** S1 loses m23_white at tt 256 and two quick cases;
   S2 loses m22 at tt 128; S3/S4/S5 lose stress at tt 128. **Fail.**
3. **Quick total not worse than S0 (+5%).** S4 passes, S3 ties, S2 ties; S1
   and S5 fail. Checked but not decisive given bar 2.
4. **No case regresses ≥2×.** S1's dec31 is 278× (the known price of removing
   the restart), S2's m22 tt 128 is 38×, and every tt-128 timeout is a
   censored >2× regression. **Fail.**

**D1 = keep S0.** The shipped `500_000` geometric ×2 schedule is retained
unchanged as `RestartSchedule::SHIPPED`. The grid confirms the plan's
hypothesis: the schedule is a measured, TT-capacity-dependent
diversification policy, not a performance guarantee; no arm dominates.

## Phase 2 — plumbing split (shipped)

### D-a: explicit pacing policy

New `src/search/dfpn/restart_schedule.rs`:

- `RestartSchedule { base, growth, cutoff }`, `Growth::{Geometric, Linear}`.
- `SHIPPED` reproduces the historical constants exactly; `NEVER` is the
  unbounded spelling; `geometric` uses the existing exact `fraction_from_f64`;
  `with_cutoff`, `first_slice`, `next`.
- `Search::set_restart_schedule` / `restart_schedule` / `restarts`.
- `Search::new` seeds `SHIPPED`; `begin_run` resets `restarts`.
- The old `linear_chunks` / `chunk_increment` / `chunk_multiplier_*` fields
  and the two public setters are gone. The only in-repo caller,
  `examples/chunk_growth`, now uses the new API and gained `--none`,
  `--cap`, and `--linear` (`Growth::Linear`); its doc warns that its fixed
  64 MB TT makes the comparison TT-size-specific.

The generic `fraction_from_f64`/`gcd` helper moved to a small
`src/search/dfpn/fraction.rs` module so ε arithmetic and restart growth share
it without keeping it in the large search module.

### D-b: hard budget per node, pacing in the slice

- `dfpn`'s `max_work` is renamed `restart_slice` (and `child_max_work` →
  `child_slice`) with unchanged slice semantics.
- A per-node `child_eval_budget_exceeded()` check now sits at the top of the
  frame loop beside `time_exceeded()`.
- `bounded_search` computes `call_slice = chunk.min(round_remaining)`; the
  global budget is no longer shrunk into the call. The round cap stays in the
  slice (pacing).
- `RoundTermination` attribution is re-derived at loop exit exactly as the
  plan specifies (resource/clock first, under-spent slice → `Exhausted`,
  zero slice → `CapCut` unless the budget is the smaller constraint; ties
  stay `ResourceCut`).
- `set_child_eval_budget(0)` still exits before the first call and reports
  `BudgetExhausted`.

### D-c: documentation

Corrected `core.rs`'s false "resume from the same bounds" comment to restart
semantics (unwind, re-enter at `INF/INF`, TT only), rewrote the
`set_child_eval_budget` and `RoundTermination` docs, added the per-node
budget/restart-schedule sentence to the dfpn module header and the `AGENTS.md`
bullet.

### D2 against the pre-registered bars

1. **Unbudgeted path bit-identical.** Quick suite: 42,109,768 / 59 at tt 256
   and 40,042,745 / 59 at tt 128 (exact Phase 0 totals), zero `wrong`.
   All five single-case anchors at both TT sizes exact; m22 default-mode
   18,779,890 with `pv_status: cap-cut`; `test_trajectory_golden
   --include-ignored` green without re-baselining. **Pass.**
2. **Budget-mode contract holds.** `test_decisive_remaining`,
   `test_smoke`, `test_cli::cli_budget_exhaustion_returns_draw`, and
   `test_repetition --include-ignored` green. No eval-budget fixture value
   changed. **Pass** (see "Pre-existing stale tests" below for two
   unrelated re-baselines).
3. **No wall-clock regression.** Interleaved medians of 3 against HEAD:
   stress FO 29.701 s → 29.788 s (+0.29%); m22 default 2.009 s → 2.031 s
   (+1.1%). Within +2%. **Pass.**
4. **Simplification is real.** No remaining `max_work`/budget entanglement in
   docs or comments (`max_work` is gone; the only "call_max_work" mentions are
   historical comments in tests). Net line count of
   `src/search/dfpn/{mod,core}.rs` versus HEAD is **0** (mod +75/−86,
   core +21/−10). `cargo clippy --all-targets` clean; `cargo doc` warnings
   unchanged. **Pass.**
5. **Gates.** `make test` green; `make test-full` green (36 test binaries,
   0 failures). **Pass.**

## Tests added

- `restart_schedule::shipped_matches_legacy_formula` — geometric and linear
  `next()` against the two legacy formulas.
- `restart_schedule::never_and_zero_cutoff_are_equivalent` — `NEVER` and
  `SHIPPED.with_cutoff(Some(0))` coincide.
- `restart_schedule::never_restarts_keeps_the_first_slice_unbounded`.
- `dfpn::tests::budget_overshoot_is_bounded` — budgets 1k/10k/100k/1M on m22:
  `Draw`, `BudgetExhausted`, `child_evals <= budget + 256`.
- `dfpn::tests::bounded_search_reports_resource_cut_when_budget_and_cap_tie`.
- `dfpn::tests::never_restarts` — `RestartSchedule::NEVER`, no budget,
  `restarts() == 0`.
- `dfpn::tests::shipped_schedule_restarts_past_the_first_slice` — dec31,
  `restarts() >= 1`.

## Pre-existing stale tests found by the full gate

Two slow tests failed for reasons unrelated to this plan. The isolated HEAD
copy reproduced both failures identically, so they were already stale at
`8d711b0`. They were re-baselined as test maintenance (recorded here, not
silent scope expansion):

- `tests/stress.rs::move_order_hard_positions_unproven_in_60s` asserted
  `m21_white` still times out at 60 s with a 64 MB TT. It now solves in
  ~52 s / 596,102,838 child evals (`Win` 200 plies, `cap-cut`) at both HEAD
  and the new code. `m21_white` is excluded from the unproven assertion; the
  other m20/m21 positions still time out. This follows the test's own
  documented instruction ("move the position to the regression suite").
- `tests/test_deep_outcomes.rs::m22_white_wins` used a 120M-eval budget at a
  64 MB TT, but that configuration does not prove m22 within 120M at HEAD
  either (nor within 1.2B; an unbudgeted 64 MB run spent 3.71B evals in
  300 s without proving it). The test now uses the shipped 256 MB TT with a
  60M budget; the measured first-outcome cost is 15,023,911 evals, so the
  budget has 4× headroom. A new
  `assert_solves_within_evals_with_tt` helper keeps the default at 64 MB for
  the other regressions.

No production behavior was changed to satisfy either test.

## Tools / examples used

- `atomic_solver` CLI (`--fen`, `--timeout`, `--tt-size`, `--first-outcome`,
  `--budget`), `examples/benchmark --suite quick`, `examples/chunk_growth`,
  `tests/test_trajectory_golden`, and the `make test` / `make test-full`
  tiers.
- `measurements/plan12/driver.py` for the arm × TT × case grid; all runs on an
  isolated copy so `git diff` in the product tree only ever contained Phase 2.

## Problems encountered

- The plan's ~20-minute grid estimate was optimistic: the full grid took
  ~35 minutes of solver time (stress default is ~40–50 s per cell; no-restart
  cells hit their per-case timeouts). It still fit one session.
- The full gate's first two failures were the stale tests above, not the
  split. Diagnosing them required re-running the two configurations against
  the isolated HEAD build; that comparison is the evidence that the split is
  neutral.
- `clippy` rejected the first `first_slice` comparison (`0 >= c`) and an
  identity-op in a test; both trivial fixes.

## Unresolved / missing tests

- No end-to-end test exercises a non-zero `with_cutoff` firing mid-run or
  `Growth::Linear`; `chunk_growth` covers both manually, but the pure tests
  only pin the formulas. A cheap deterministic cutoff test would close this.
- The grid ran the `decisive` suite? **No** — the session spent its run budget
  on the mandatory matrix; the plan marked `--suite decisive` as optional and
  it is recorded here as a gap. D1 did not depend on it (bar 1 already failed
  at tt 128 for every arm).
- No stress tax was reclaimed. D1 deliberately keeps S0; the ~21% stress FO
  tax vs. no-restart remains the measured price of the dec31/m23_white tail
  benefit.

## Next steps

- Close `conversion` backlog #9 (this report). Update the initiative row and
  the index.
- If the restart schedule is ever revisited, treat it as a TT-size-dependent
  policy and measure both TT budgets; the grid is the re-baseline.
- The remaining `conversion` thread is item #5(e) (the complexity-paper
  reading); no new search-semantics lever is opened by this plan.
