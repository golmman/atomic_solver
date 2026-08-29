# Report 3: Tiered test suite with deterministic budgets

This report documents the work done for `docs/plans/testability/plan3.md`.
All three phases are implemented: Phase 1 ("Relief" — tasks 1, 2, 6),
Phase 2 (smoke suite — task 3), and Phase 3 (deterministic child-eval budgets —
tasks 4, 5, plus the `m22` smoke entry of task 3.3).

## Summary

* Converted all 24 `#[cfg_attr(debug_assertions, ignore = "...")]` attributes
  in `tests/` to plain `#[ignore = "slow: ...; run with -- --include-ignored"]`.
  `grep -rn "cfg_attr(debug_assertions" tests/` now returns nothing.
* Added `test`, `test-full`, and `test-lite` make targets; all existing targets
  (`quick_export`, `quick_export2`, `macos_cleanup`, `nn_corpus`) are untouched.
* Added a "Testing tiers" section to `AGENTS.md`, updated its "Conventions"
  section, and pointed the README "Development" section at the make targets.
* Created the always-on smoke suite: `tests/fixtures/smoke_positions.txt`
  (13 entries incl. the `m22` deep tripwire) and `tests/test_smoke.rs` (not
  ignored), with a new `common::assert_smoke` helper.
* Added `Search::set_child_eval_budget` / `Search::child_eval_budget_exceeded`
  and `ExitReason::BudgetExhausted`; a budget-exhausted search returns `Draw`,
  caches only unsolved TT entries, and is never reported as a timeout.
* Converted the `decisive_remaining.txt` entries to deterministic child-eval
  budgets (fixture notes `solvable_evals:<B>` / `unproven_evals:<B>`), moving
  15 unproven tripwires into the fast tier (~1 s each) and making the 10
  solvable regressions deterministic (they stay in the slow tier, see the
  tier-split deviation below). Converted `m22_white_wins` to a budget as well.
* Behavior change (intended): `cargo test --release` is now the **fast tier**
  and skips the 25 slow tests; debug builds behave exactly as before (those
  tests were already skipped there). Anyone relying on
  `cargo test --release` as the full suite must switch to `make test-full`.

## Decisions for the open questions

* **Ignore reason format.** All converted attributes use the plan's suggested
  shape `#[ignore = "slow: <what makes it slow>; run with -- --include-ignored"]`
  with a per-test description (e.g. `slow: 60 s budget per position`,
  `slow: 10 s refined search on m22`), instead of one generic reason.
* **Attribute conversion only.** No test bodies were changed. The only non-
  attribute edits are three doc comments that referenced the removed profile
  coupling (`tests/stress.rs` header said "run only in release builds";
  `tests/test_plan6.rs` had two comments tied to debug/release behavior);
  they were reworded to describe the tier instead.
* **`CARGO_PROFILE_RELEASE_LTO=thin` override.** Kept, per the plan's make
  snippet. Measured on the container host (not the asahi M1 reference host):
  a full rebuild+link of all test binaries took ~60 s with fat LTO
  (`lto = true`) vs ~72 s with the thin override — within noise, no clear win
  here. Since the override targets the edit-test loop on the memory-bound M1,
  it should be re-measured there; if it is negligible on the reference host
  too, drop it for simplicity (plan task 2.2).
* **README.** It did not mention `cargo test --release` as the full suite, so
  task 6.3 required no correction; the "Development" section was still updated
  to list `make test` / `make test-full` / `make test-lite` so the tiers are
  discoverable. Historical `docs/plans/` documents were left as-is (per
  `AGENTS.md`, older plans/reports may not reflect the current state).

## Converted attributes (24) and resulting tier membership

| File | Count | New reason (abbreviated) |
|------|-------|--------------------------|
| `tests/stress.rs` | 2 | `slow: 60 s budget per position` |
| `tests/test_corpus.rs` | 1 | `slow: 60 s CLI timeout per corpus position` |
| `tests/test_decisive.rs` | 1 | `slow: 5 s timeout per decisive position` |
| `tests/test_decisive_remaining.rs` | 2 | `slow: up to 60 s per solvable position` / `slow: burns a full 60 s per unproven position` |
| `tests/test_epsilon.rs` | 2 | `slow: 5 s timeout per position` |
| `tests/test_move_order.rs` | 2 | `slow: 5 s timeout per move-order position` / `slow: 10 s refined search on m22` |
| `tests/test_plan6.rs` | 12 | `slow: 5 s default-timeout regression` (9), `slow: 60 s regression budget` (2, the m22 pair), `slow: 60 s stress test` (1, `m24_solve_with_pv`) |
| `tests/test_review.rs` | 2 | `slow: shortest-PV refinement` |

All 24 previously profile-coupled tests are now in the **slow tier**
(`#[ignore]`d in every profile, runnable via `make test-full`). None moved
into the fast tier in this phase; that happens in Phase 3 when wall-clock
budgets become `child_evals` budgets.

## Measured wall times

Host caveat: the reference host in the plan is the asahi M1. This session ran
in a Linux container, so absolute numbers differ; ratios and the pass/fail
behavior are what matter.

* `make test` (fast gate): **1 m 28 s wall including a cold release build**;
  ~61 s of actual test time (sum of the harness `finished in` values, the
  largest single contributor being `tests/test_benchmark_json.rs` at 24 s,
  then the smoke suite at 6.3 s). 154 unit tests + all fast integration tests
  passed; exactly 24 tests ignored.
* `make test-lite`: 45 s wall including compile (before the smoke suite);
  same 24 ignored.
* `make test-full`: not executed end-to-end in this session (≈25 min on the
  reference host, longer here). Spot-checked via
  `cargo test --release --test test_plan6 -- --include-ignored`
  (see below) and via `--list --ignored`, which confirms the ignored tests
  are selected by `--include-ignored`.

## Smoke suite (Phase 2, task 3)

* **Composition** (`tests/fixtures/smoke_positions.txt`, `name;fen;expected;note`):
  * all 7 entries of `tests/fixtures/positions.txt`, converted from the CLI
    corpus format (`fen;expected;max_pv_plies`) — rook back-rank mate, queen
    corner mate, two bare-kings two-commoners cases, promotion
    transposition, two-rook transposition, and the terminal stalemate draw;
  * 5 representative entries from `tests/fixtures/decisive_positions.txt`
    chosen for distinct mate themes: `dec02` (pawn endgame, Loss),
    `dec04` (promotion + rook mate, Win), `dec08` (3-ply queen mate, Win),
    `dec37` (rook back-rank, Loss), `dec45` (queen sacrifice, Win);
  * the `m22` deep tripwire (task 3.3): `smoke_m22_black_unproven` — the
    move-order fixture's `m22_black` FEN with note `unproven_evals:1000000`,
    routed to `assert_unproven_within_evals` instead of the 2 s wall clock.
    The tripwire stays unproven within 1M child evals today (measured: the
    position does not solve even a 90-second first-outcome search, ≥ 250M
    evals), so it is cheap (~1 s) and machine-independent; a move-ordering
    improvement that solves it within 1M evals fails deterministically and
    the entry is re-categorized.
* **Assertion** (`common::assert_smoke`): no wrong decisive result — a decisive
  outcome must match `expected`; a `Draw` is accepted only when the search was
  cut short (`time_exceeded()`) *or* when `Draw` is the expected outcome
  (terminal stalemate draws return instantly without timing out, so the
  existing `assert_solves_or_times_out` would misclassify them). A decisive
  result where `Draw` is expected fails the test. Entries whose note carries a
  child-evals budget are routed to the Phase-3 helpers instead.
* **Measured wall time**: 6.7 s (`finished in`) in release with the `m22`
  tripwire, 13 s in debug for the 13-entry loop — well under the 60 s cap and
  the ~20 s target. Entry count and budgets need no reduction.
* Both smoke tests run in the fast tier of `make test` (not ignored).

## `Search::set_child_eval_budget` (Phase 3, task 4)

* **API** (as specified by the plan): `set_child_eval_budget(&mut self,
  budget: u64)` (`u64::MAX` = unbounded, the default) and
  `child_eval_budget_exceeded(&self) -> bool` (`child_evals >= budget`).
* **Enforcement.** The budget is enforced at the same boundaries a timeout
  would use, without adding any branch to the innermost recursion: the
  `bounded_search` work-chunk loop gained a `!child_eval_budget_exceeded()`
  condition, and each `dfpn` call is capped at `min(chunk, remaining_budget)`
  so the **existing** `max_work` checks in `core.rs` cut the recursion. No
  `core.rs` changes were needed. The refinement loop in `solve_with_progress`
  also stops on budget exhaustion.
* **Exit/caching semantics.** A budget-exhausted search returns
  `Outcome::Draw`; partial results unwind through the ordinary work-chunk
  cutoff path and are stored as unsolved `(pn, dn)` entries — there is no new
  caching code path at all. `ExitReason::BudgetExhausted` is a new variant;
  `exit_reason()` reports it (after Quit/MemoryLimit, before Timeout), and
  `time_exceeded()` stays exclusively about wall clock, preserving the
  "Draw is OK only when timed out" idiom. The CLI prints `budget exhausted`
  (not `timeout`) for this reason; the CLI exposes no budget flag, so the
  reason can only appear for programmatic users, but the output stays
  truthful.
* **Deviation from the plan:** none for the API itself. One nuance worth
  recording: a budget-exhausted search *can* still return a decisive outcome
  when the proof completes within the budget (that is the solvable case);
  decisive results always come from completed child evaluations, never from
  partial work.
* **Unit tests** (`src/search/dfpn/tests.rs`), grounded on measured,
  bit-identical-across-runs numbers for the promotion-transposition position
  (full solve W = 426,882 evals, first decisive line W1 = 7,449):
  * `child_eval_budget_zero_causes_immediate_exit` — budget 0 returns Draw
    with an empty PV, reports `BudgetExhausted` (not `Timeout`), and stores
    nothing;
  * `child_eval_budget_generous_still_solves` — budget 5,000,000 (~10× W)
    still solves to `Win` with `ExitReason::Complete`;
  * `child_eval_budget_fraction_does_not_solve` — budget 5,000 (< W1) returns
    Draw, reports `BudgetExhausted`, and a follow-up unbounded solve on the
    same TT still finds the Win (no proven poison entry);
  * `budget_exhausted_pv_is_empty_or_valid` — budget 5 on the two-rook
    position: the informational PV is empty or fully legal (replayed with
    `Position::try_do_move`).

## Fixture conversion (Phase 3, task 5)

Measurement (task 5.2) used a temporary example binary (removed afterwards)
with `Search::new(64)`, default epsilon, `first_outcome_only`. Two runs
produced **bit-identical** eval counts for every solvable entry — the
determinism assumption holds. Measured first-outcome effort W:

| entry | W (child evals) | budget B | tier after conversion |
|-------|-----------------|----------|----------------------|
| rem01 | 66,452,175 | `solvable_evals:200000000` (3×W) | slow (deterministic) |
| rem04 | 11,798,950 | `solvable_evals:36000000` (3×W) | slow (deterministic) |
| rem07 | 30,826,494 | `solvable_evals:93000000` (3×W) | slow (deterministic) |
| rem08 | 28,781,735 | `solvable_evals:87000000` (3×W) | slow (deterministic) |
| rem09 | 131,141,572 | `solvable_evals:394000000` (3×W) | slow (deterministic) |
| rem10 | 23,300,929 | `solvable_evals:70000000` (3×W) | slow (deterministic) |
| rem11 | 82,081,357 | `solvable_evals:247000000` (3×W) | slow (deterministic) |
| rem12 | 59,988,758 | `solvable_evals:180000000` (3×W) | slow (deterministic) |
| rem13 | 41,634,958 | `solvable_evals:125000000` (3×W) | slow (deterministic) |
| rem23 | 90,152,959 | `solvable_evals:271000000` (3×W) | slow (deterministic) |
| rem02/03/05/06/14–22/24/25 (15 entries) | unsolved ≥ 90 s first-outcome (≥ ~250M evals) | `unproven_evals:1000000` each | **fast** (~1 s each) |

* **Tier-split deviation (documented per task 5.3).** The plan expected
  converted solvable entries to leave the slow tier, but their wall time is
  ≈ W regardless of the budget (a deterministic budget only avoids *wasting*
  time; it cannot make a 131M-eval solve cheap). Summing W over the ten
  solvable entries gives ~570M evals (≈ 3.7 min measured), which cannot fit
  the < 60 s fast-gate target. They were therefore converted to
  deterministic budgets (killing the wall-clock flakiness — the same
  misclassification lifecycle now fails reproducibly instead of
  machine-dependently) while remaining `#[ignore]`d. Only the 15 unproven
  tripwires joined the fast tier, at B = 1M each (~1 s).
* **Budget-ratio deviations (documented per task 5.3).**
  * Solvable entries use the planned 3× headroom, rounded up to round numbers.
  * Unproven entries deviate from the planned "½ × measured effort": the
    measured effort of an unproven entry is just "however much the former 60 s
    burn buys" (≥ 250M evals), so ½ × that would burn ~2 minutes per entry and
    defeat the purpose. B = 1,000,000 was chosen instead: ~5× below the
    cheapest known non-solve (the 5-second default budget ≈ 5M evals) and
    ~250× below the 60 s burn, so the tripwire property ("fails when an
    ordering improvement solves it") is fully preserved.
* **Converted `m22_white_wins`** (`tests/test_plan6.rs`): measured
  W = 37,503,264 (two identical runs); converted to
  `assert_solves_within_evals(fen, Win, 120_000_000)` (~3× headroom), still
  `#[ignore]`d (≈ 13 s wall). `m22_black_loses` **stays wall-clock**: this
  position does not solve within a 90-second first-outcome search here
  (≥ ~330M evals), so no deterministic budget could be measured; it is the
  one remaining machine-dependent borderline test (it failed on this
  container host even before this plan — see Phase 1 findings).
* **Wall-clock keepers (task 5.4).** `tests/stress.rs` (m19, m20/m21 loop)
  and `m24_solve_with_pv` keep their 60 s wall-clock budgets and `#[ignore]`
  status: they guard against hangs and time-related bugs (chunk timing,
  `time_exceeded`, stop flag) that node budgets cannot cover. Measured m20/m21
  first-outcome efforts (≥ 243M evals, unsolved at 90 s) confirm they are far
  from convertible today.
* The two legacy 60 s runners in `tests/test_decisive_remaining.rs` are kept
  (currently iterating zero entries) so re-categorized entries have a runner;
  the fixture header documents the note lifecycle.

## Verification results

```bash
grep -rn "cfg_attr(debug_assertions" tests/   # 0 matches ✔
cargo fmt -- --check                          # clean ✔
cargo clippy --all-targets                    # clean ✔
make test                                     # all green, 25 ignored, twice in a row ✔
make test-lite                                # all green, 25 ignored ✔
cargo test --release --test test_smoke        # 2 passed, 6.7 s ✔
cargo test --test test_smoke                  # 2 passed in debug, 13 s ✔
cargo test --release --lib                    # 158 passed, incl. 4 budget unit tests ✔
cargo test --release --test test_decisive_remaining \
    decisive_remaining_solvable_within_evals -- --include-ignored   # passed, 222 s ✔
cargo test --release --test test_plan6 m22_white_wins -- --include-ignored  # passed, 12.5 s ✔
```

New behavior verified explicitly:

* `make test` no longer runs the 60 s suites: 25 tests ignored, and the two
  consecutive runs were both fully green (deterministic budget stability);
* the converted unproven tripwires report `child_eval_budget_exceeded()`, not
  `time_exceeded()`, and the unit tests assert the budget-0 exit stores
  nothing and the fraction-budget exit caches no proven poison entry;
* a full `make test-full` run remains machine-dependent in exactly one place:
  `m22_black_loses` (see Problems).

## Measured wall times

Host caveat: the reference host in the plan is the asahi M1. This session ran
in a Linux container, so absolute numbers differ; ratios and the pass/fail
behavior are what matter.

* `make test` (fast gate): ~2 m 09 s wall including a cold release build;
  ~67 s of actual test time. Largest contributors:
  `tests/test_benchmark_json.rs` 24 s, the smoke suite 6.7 s, the 15 unproven
  eval-budget tripwires ~5.5 s, `test_plan2`/`test_inf`/`test_repetition`
  5 s each. Slightly over the 60 s target; the dominant single cost is
  `test_benchmark_json`, which is outside this plan's scope.
* `make test-lite`: 2 m 26 s wall incl. compile (the eval-budget tripwires
  cost ~69 s in debug because the budget is in evals, not seconds; behavior
  is identical, only slower).
* `make test-full`: not executed end-to-end in this session. Validated via
  targeted `--include-ignored` runs (test_plan6: all pass except the
  known-flaky `m22_black_loses`; decisive_remaining solvable budgets: pass in
  222 s) plus `--list --ignored`.

## Problems encountered

* `m22_black_loses` (`tests/test_plan6.rs`) does not solve within its 60 s
  budget on this container host (≥ ~330M child evals at first-outcome), so it
  failed both before and after this plan; it is the one remaining
  machine-dependent test and would fail `make test-full` on hosts slower than
  the asahi M1. It could not be converted because no solve effort is
  measurable for it here.
* `tests/test_benchmark_json.rs` alone consumes ~24 s of the fast gate's
  ~67 s test-time budget on this host. Not part of this plan, but it is the
  first candidate to look at if the 60 s target is ever missed on the
  reference host.
* The plan's expectation that converted *solvable* entries become cheap did
  not hold (wall time ≈ W regardless of budget); the tier split was adjusted
  and documented above instead of blowing the fast-gate target.

## Unresolved parts / missing tests

* `m22_black_loses` remains wall-clock and machine-dependent; convert it once
  a solve effort can be measured for it (e.g. on a host fast enough, or after
  a move-ordering improvement).
* Reference-host (asahi M1) measurements of the fast gate and of the thin-LTO
  override.
* A complete green `make test-full` run was not performed in this session.

## Next steps

1. Re-measure `CARGO_PROFILE_RELEASE_LTO=thin` on the asahi M1 host; drop the
   override if it shows no benefit there.
2. Consider trimming `tests/test_benchmark_json.rs` (24 s of the fast gate).
3. Convert `m22_black_loses` and any remaining `solvable_60s`-style entries
   when their solve efforts become measurable.
4. cargo-nextest was considered and not adopted (per plan task 7.1): retries
   on correctness assertions can mask genuine nondeterministic
   misclassification, and the budget conversions shrank the wall-clock
   population that would motivate adoption. The make targets keep working
   with the standard harness. There is no CI (project decision); the make
   targets plus the `AGENTS.md` conventions are the enforcement point, so
   regressions caught only by the slow tier surface when someone chooses to
   run `make test-full`.
