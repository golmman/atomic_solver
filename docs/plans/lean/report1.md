# Lean Report 1 — Refinement accounting + tail cap + TT probe consolidation

Implements `docs/plans/lean/plan1.md` (items 1, 1a, and 4 of the lean
initiative). All measurements were taken 2026-09-07 on the release build.
Raw outputs: `docs/plans/lean/measurements/plan1/`.

## Summary of changes

- `src/search/dfpn/mod.rs`: per-refinement-round child-eval cap
  (`refine_cap_factor_num/den`, `refine_cap_min`, `first_outcome_evals`,
  `refinement_rounds`, `refinement_evals`), `set_refine_cap_factor`
  (factor `0.0` = disabled via `refine_cap_min = u64::MAX`),
  accessors `first_outcome_evaluations()` / `refinement_rounds()` /
  `refinement_evaluations()`, a `#[cfg(test)]` helper
  `set_refine_round_cap_for_test`, and a `round_work_cap` parameter on
  `bounded_search` (the non-refinement entry points pass `u64::MAX`).
- `src/cli.rs` + `src/main.rs`: `--refine-cap <FACTOR>` (f64, default
  `0.25`, `0` disables), help text and doc-comment updates.
- `src/search/dfpn/core.rs` + `children.rs`: single TT probe per node and
  per child evaluation; the copied `TtEntry` feeds the depth checks
  (`resolved_from_entry`), the one-ply repetition guard, the ordering hint,
  and the previous-bounds snapshot. The one-ply guard now plays/undoes the
  best move on the actual position instead of cloning it.
- `src/search/tt/`: removed `probe_summary`, `probe_best_move`, and
  `TtSummary` (no remaining callers); `probe`/`store` unchanged.
- `src/search/dfpn/tests.rs`: three new tests (see below) plus the
  rewritten `tt_resolved_rejects_win_when_best_move_repeats`.
- `AGENTS.md`: dfpn bullet and CLI flag list updated.

## Step 6 — refinement test position

Candidate scan (`--timeout 3`, default mode, stderr progress lines):

| case | first | refined | verdict |
| --- | --- | --- | --- |
| dec06 | 22 | 20 → 16 | converges, 0.63 s total |
| dec10 | 35 | 33 → 31 | still refining at 5 s |
| dec14 | 21 | 17 → 15 | still refining at 5 s |
| dec15 | 273 | … → 169 | far from converged at 3 s |
| dec44 | 50 | 48 | converges, **0.049 s** total |

Chosen fixture: **dec44** (`r7/1Rp4k/4P2B/6p1/3P2P1/p6p/P6K/8 b - - 0 35`,
loss, refines 50 → 48 plies, fully converges in ~50 ms release). All three
new tests use it and stay well inside the fast tier (the whole dfpn unit
test module runs in 0.2 s release).

## New tests

1. `refinement_counters_accumulate` — dec44: `first_outcome_evaluations()`
   > 0, `refinement_rounds() >= 1`,
   `child_evaluations() >= first_outcome_evaluations()` (cumulative
   semantics locked), and `refinement_evaluations()` equals the total-minus-
   first-outcome delta.
2. `refine_cap_zero_disables_capping` — factor `0.0` still refines the
   fixture (escape hatch preserves pre-cap behavior).
3. `refine_cap_bounds_round_work` — `set_refine_round_cap_for_test(1_000)`
   forces the cap to bind: refinement work stays below the first-outcome
   work and within `rounds × 10_000` evals (cap + dfpn overshoot).

Existing budget suites (`test_epsilon`, `ExitReason::BudgetExhausted` tests)
pass unchanged.

## A/B measurements (m22_white, `--timeout 20`, default mode)

Before (baseline, uncapped): first win at ~9.9 s (155 plies), refinement
155 → 153 → 23 plies by ~11.1 s, then the futile bound-21 round ran until
the 20 s global timeout (44% of wall).

| mode | wall | result |
| --- | --- | --- |
| before, default | 20.0 s (timeout) | win, 23-ply PV |
| after, `--refine-cap 0` | 20.0 s (timeout) | byte-identical stdout; chunk logs identical in `work_done`/`nodes`/`max_depth` |
| after, default cap 0.25 | **14.0 s** | win, byte-identical 23-ply PV |

The default cap cut the futile round deterministically: its chunks ran
500k → 1M → 2M → 4M, then the final chunk was clamped by the remaining
round budget (`work_done=1,875,776` at 14.0 s) and the round was abandoned.
Derived numbers: `first_outcome_evals ≈ 37.5M` (phase 1 cumulative chunks
31.5M + the winning partial chunk), so the round cap was
`0.25 × 37.5M ≈ 9.38M` — the plan's estimate (~17M first-outcome work,
4.25M cap, cut at ~4M) underestimated phase-1 work; the measured cut point
was ~9.4M round evals, saving ~6 s of the 20 s budget. Improving rounds
(each ≤ 1.5M evals) were untouched, which is exactly the intended
selectivity.

Drift checks:

- `benchmark --suite quick --json --first-outcome`: 59/59 cases identical
  `child_evals` (total 38,714,551 before and after) and identical
  `pv_len` per case — the TT consolidation and accounting changes do not
  perturb the search.
- m22 `--refine-cap 0`: byte-identical stdout vs baseline; stderr chunk
  logs identical in every deterministic field.

## Deviations from the plan

1. **`if call_max_work == 0 { break; }` added in the `bounded_search` chunk
   loop.** The plan argued no new break condition is needed. That holds for
   the refinement *loop* (a cap-cut round returns `Draw` and the existing
   non-improving check stops refinement), but without this guard the chunk
   loop itself would keep calling `dfpn` with `max_work = 0` and grow chunks
   until the global deadline. The break fires only when the round cap or the
   global budget is exactly exhausted, so the uncapped path is unchanged
   (verified bit-identical).
2. **`try_use_tt` was removed rather than kept as an entry-based wrapper.**
   Both remaining hot paths (node level, `evaluate_child`) now probe once
   and use `resolved_from_entry` + `best_move_repeats_path` directly, so a
   wrapper would have been dead code. The repetition-guard unit test was
   rewritten against the two helpers (`tt_resolved_rejects_win_when_best_
   move_repeats`).
3. **`refine_cap_bounds_round_work` does not assert `rounds == 1`.** With a
   1,000-eval cap the fixture still found improving lines inside the cap, so
   the assertion was relaxed to "each round stays near the cap"
   (`refinement_evals <= rounds × 10_000`), which is the property the cap
   actually guarantees.
4. The plan's cap-estimate arithmetic in the rationale was optimistic (see
   A/B numbers above); the mechanism works exactly as specified, only the
   predicted cut point (~4M) differs from the measured one (~9.4M).

## Problems encountered

- None blocking. The `&self` → `&mut self` upgrade for the one-ply guard
  compiled cleanly at every call site (node level, `evaluate_child`, tests);
  no clone had to be kept.
- `probe_summary`/`probe_best_move`/`TtSummary` removal required touching
  only `tt/tests.rs` (two tests rewritten against `probe`).

## Missing tests

- No test covers the CLI-to-search wiring for `--refine-cap` (the CLI
  parser is unit-tested; the flag's effect is covered by the m22 A/B run in
  this report rather than an automated test).
- No test asserts that the *default* cap (factor 0.25, 1M floor) never binds
  on the quick suite; the drift check confirms it empirically.

## Next steps

- Tune the cap factor on the decisive/move-order suites in default mode
  (`benchmark` does not yet expose `--refine-cap`; add it if the optimizer
  needs to tune the factor itself).
- plan2: movegen deduplication, allocation removal, scorer-cost profiling
  (items 3/6/8).
- plan3: parallelism (item 2, determinism design required).
