# Plan 3: Tiered test suite with deterministic budgets

## Start

1. Read `AGENTS.md`, `docs/plans/testability/report2.md`, and this plan.
2. Reproduce the problem: run `cargo test --release` on the host and note the
   total wall time (reported as > 25 minutes on the asahi M1 host).
3. Confirm the cause before changing anything: every test listed in
   `Background` carries `#[cfg_attr(debug_assertions, ignore = "...")]`, so
   release builds run all of the 60-second suites by default.

## Goal

Split the test suite into tiers so that the default fast gate
(`make test`, i.e. `cargo test --release`) finishes in about one minute of test
time (compile time excluded), while the full deep-search regression and stress
suites remain available via `make test-full`
(`cargo test --release -- --include-ignored`). Additionally, replace
wall-clock-budget regression assertions with deterministic `child_evals`
budgets where feasible, so tests stop being machine- and load-dependent.

Priorities (from `AGENTS.md`): correctness, performance, efficient memory
usage, maintainability, testability. This plan trades *local* deep-search
coverage for a fast feedback loop, and mitigates that loss by (a) an always-on
smoke suite and (b) converting machine-time budgets into deterministic
`child_evals` budgets, which lets several deep tests move back into the fast
tier.

## Background

* The suite currently takes > 25 minutes under `cargo test --release`. The
  dominant costs are serial loops that each burn a full wall-clock timeout:
  * `tests/test_decisive_remaining.rs` — `decisive_remaining_unproven_in_60s`
    iterates 16 `unproven_60s` fixture entries, each spending a guaranteed
    60 s (`tests/fixtures/decisive_remaining.txt`); plus
    `decisive_remaining_solvable_in_60s` with 11 entries of up to 60 s
    (rem09 is borderline at ~46 s).
  * `tests/stress.rs` — m19 plus the m20/m21 move-order positions, 60 s each.
  * `tests/test_plan6.rs` — m22 regressions with 10 s budgets,
    `m24_solve_with_pv` with a 60 s budget; twelve tests ignored only in
    debug builds.
  * `tests/test_move_order.rs` — fixture loops with 10 s budgets.
* Test selection is currently coupled to the build profile: 24 tests use
  `#[cfg_attr(debug_assertions, ignore = "...")]`
  (`grep -rn "cfg_attr(debug_assertions" tests/`). In debug they are skipped;
  in release `cargo test --release` silently becomes the full 25-minute suite.
  This violates least astonishment: the build profile should not select which
  tests run.
* Wall-clock budgets are machine-dependent and flaky by construction:
  * "unproven" tests must always burn their entire budget.
  * The fixture header documents borderline cases (`rem09 ... ~46 s`), and
    `tests/stress.rs` notes that `m22` "is occasionally solved in release
    within 60 seconds".
* The project already treats `child_evals` as the preferred deterministic
  efficiency metric (`AGENTS.md`, "Tuning workflow"). `Search` tracks
  `child_evals` internally (`src/search/dfpn/mod.rs`), exposes
  `Search::child_evaluations()` and `Search::exit_reason()`, and the bounded
  refinement in `src/search/dfpn/core.rs` already enforces internal
  `max_work` budgets — but there is no public way to bound a search by
  child evaluations instead of seconds.
* `Makefile` has no `test` targets at all; there is no CI configuration in the
  repository.
* The relevant quality attribute ordering means the deep suites must not be
  deleted: they are the only regression net for search correctness at depth
  (misclassification, GHI/repetition handling, proof-tree correctness). The
  fixture files encode documented expectations ("when a move-ordering
  improvement makes m20 decisive, move it to the regression suite"), and that
  lifecycle must be preserved.

## Implementation tasks

### 1. Make test selection orthogonal to the build profile

1.1 Replace every `#[cfg_attr(debug_assertions, ignore = "<reason>")]` in
`tests/` with a plain `#[ignore = "<reason>"]` attribute. Keep the reason
short and actionable, e.g.:

```rust
#[ignore = "slow: 60 s budget per position; run with -- --include-ignored"]
```

1.2 Affected files (24 attributes as of this plan):
`tests/stress.rs` (2), `tests/test_corpus.rs` (1),
`tests/test_decisive.rs` (1), `tests/test_decisive_remaining.rs` (2),
`tests/test_epsilon.rs` (2), `tests/test_move_order.rs` (2),
`tests/test_plan6.rs` (12), `tests/test_review.rs` (2).
Re-run `grep -rn "cfg_attr(debug_assertions" tests/` and confirm the count is
zero afterwards.

1.3 Do not change any test bodies in this step. The observable behavior
change is intentional: debug builds behave exactly as before (those tests
were already skipped), while release builds become the fast tier.

### 2. Add make targets for the tiers

2.1 Extend the `Makefile` with:

```make
.PHONY: test test-full test-lite

test:       ## fast gate: unit + fast integration tests (~1 min of test time)
	CARGO_PROFILE_RELEASE_LTO=thin cargo test --release

test-full:  ## everything, incl. 60 s regression/stress suites (~25 min)
	cargo test --release -- --include-ignored

test-lite:  ## debug build, quick logic check
	cargo test
```

2.2 `CARGO_PROFILE_RELEASE_LTO=thin` speeds up test-binary linking for the
edit-test loop without touching the shipped `[profile.release]`. Measure the
difference; if it is negligible on the target host, drop the override for
simplicity.

2.3 Keep the existing targets (`quick_export`, `nn_corpus`, ...) untouched.

### 3. Add an always-on smoke suite for the fast tier

3.1 Create `tests/fixtures/smoke_positions.txt` using the existing fixture
format (`name;fen;expected;note`), seeded with:

* all entries of `tests/fixtures/positions.txt`, converted to the
  `name;fen;expected;note` format (that file uses the CLI corpus format
  `fen;expected;max_pv_plies`; the smoke suite does not need PV-length
  assertions),
* one borderline deep case, `m22`, to keep a deep-search tripwire in the fast
  tier,
* 3–5 representative entries from `tests/fixtures/decisive_positions.txt`
  chosen to exercise different mate themes (rook, promotion, transposition).

3.2 Create `tests/test_smoke.rs` (not `#[ignore]`d) that solves each entry
with a small per-position timeout (2 s) and asserts the property used
elsewhere for timing-sensitive tests: **no wrong decisive result**;
`Outcome::Draw` is accepted only when the search was cut short
(`time_exceeded()` or, for the `m22` entry, `child_eval_budget_exceeded()`).
Reuse the helpers in `tests/common/mod.rs` (`assert_solves_or_times_out` or
a new `assert_smoke` helper).

3.3 The `m22` entry uses a child-evals budget from task 4
(`assert_solves_within_evals` / `assert_unproven_within_evals` as
appropriate) instead of a wall-clock timeout, so the deep tripwire is
deterministic and cheap. Until task 4 lands, ship the smoke suite without
the `m22` entry and add it in the same PR as tasks 4–5 (see Phasing).

3.4 The total wall time of the smoke suite must stay under 60 s on the asahi
M1 host (target ~20 s). If it exceeds that, reduce entries or budgets rather
than relaxing the assertion.

### 4. Add a deterministic child-evals budget to `Search`

4.1 Add to `Search` (`src/search/dfpn/mod.rs`):

```rust
/// Bound the search by cumulative child evaluations instead of wall time.
/// `u64::MAX` (the default) means unbounded.
pub fn set_child_eval_budget(&mut self, budget: u64);
pub fn child_eval_budget_exceeded(&self) -> bool;
```

4.2 Enforce the budget where `max_work` is already checked in
`src/search/dfpn/core.rs` and where the chunk loop checks the time budget in
`src/search/dfpn/mod.rs`, so a budget-exhausted search exits at the same
boundaries a timeout would. Zero and tiny budgets must exit promptly
(mirror `set_timeout(0)` semantics; there is an existing test pattern:
`set_timeout_zero_causes_immediate_exit`).

4.3 Correctness constraints (highest priority):

* A budget-exhausted search must return `Outcome::Draw` with
  `child_eval_budget_exceeded() == true`, never a decisive result derived
  from partial work.
* Partial results must only be cached as *unsolved* entries, exactly like the
  existing bounded-refinement path ("The result is stored as an unsolved
  entry..." in `src/search/dfpn/core.rs`). Budget-dependent results must not
  enter the TT as proven — same rule as repetition-dependent results.
* Add a dedicated `ExitReason::BudgetExhausted` variant so `exit_reason()`
  remains truthful. Do **not** reuse `ExitReason::Timeout`: `time_exceeded()`
  must stay exclusively about wall clock, because the existing "Draw is OK
  only when timed out" test idiom (`assert_solves_or_times_out`) depends on
  that distinction. The CLI prints `budget exhausted` (not `timeout`) for
  this exit reason.
* Do not introduce any allocation or branching into the innermost recursion
  beyond the existing budget comparisons; combine the check with the existing
  `max_work`/time check location rather than adding a second one.

4.4 Unit tests in `src/search/dfpn/` (or `tests/`):

* budget 0 exits immediately and reports `child_eval_budget_exceeded()`,
* a position that solves in `W` child evaluations (measure once, e.g. one of
  the `solvable_60s` entries) still solves when the budget is generous
  (e.g. `10 × W`) and does not solve when the budget is a fraction of `W`,
* the informational PV on a budget-exhausted search is empty-or-valid
  (never a wrong decisive line).

### 5. Convert wall-clock regression assertions to child-evals budgets

5.1 Extend `tests/common/mod.rs` with:

```rust
pub fn assert_unproven_within_evals(fen: &str, budget: u64);
pub fn assert_solves_within_evals(fen: &str, expected: Outcome, budget: u64);
```

Both use `Search::set_child_eval_budget` and assert on
`child_eval_budget_exceeded()` instead of `time_exceeded()`.

5.2 Measure first, then convert. For every `unproven_60s` and
`solvable_60s` entry in `tests/fixtures/decisive_remaining.txt` (and the m20
–m22 move-order entries), record the cumulative `child_evals` at solve time
(or the spent budget for unproven entries) with a one-off run — the
`benchmark` example's JSON output or a temporary example binary can collect
this. Because TT size, epsilon, and move ordering are deterministic and
fixed by the test configuration, these numbers are reproducible; verify
reproducibility with two runs before committing budgets.

5.3 Convert `tests/test_decisive_remaining.rs`:

* `unproven_60s` entries → `assert_unproven_within_evals(fen, B)` with
  `B = ½ ×` the measured effort, rounded to a round number. `B` must sit
  comfortably *below* the measured solve effort so that an ordering
  improvement fails the test deterministically and the entry is
  re-categorized — same lifecycle as today, minus the flakiness and the
  guaranteed 60 s burn. Converted entries leave the slow tier.
* `solvable_60s` entries → `assert_solves_within_evals(fen, expected, B)`
  with `B = 3 ×` the measured effort. Generous headroom is cheap now
  (a deterministic budget never wastes wall time proportionally), but not so
  large that a real solver slowdown would go unnoticed. Converted entries
  leave the slow tier.

Document any per-entry deviation from these ratios (e.g. entries measured
near a GHI/path discontinuity) in the report.

5.4 Keep a handful of genuine wall-clock stress tests in the slow tier
(`tests/stress.rs`, `m24_solve_with_pv`): they guard against hangs and
time-related bugs (chunk timing, `time_exceeded`, stop flag), which node
budgets cannot cover. Keep their `#[ignore]` status.

5.5 Converted entries use new note values carrying their budget, e.g.
`solvable_evals:15000000` / `unproven_evals:5000000`; the loader in
`tests/common/mod.rs` gains a small parser for them. Unconverted entries
keep their `solvable_60s`/`unproven_60s` note. This file is consumed only
by `tests/` (the examples read `decisive_positions.txt` and
`move_order_positions.txt`), so the format extension has no blast radius
outside the test tier; update the fixture header comments accordingly.

5.6 If any entry turns out to have unstable `child_evals` across runs
(indicating genuine non-determinism, e.g. path-dependent GHI results), do not
convert it; leave it wall-clock in the slow tier and record the observation
in the report.

### 6. Document the tiers

6.1 Add a "Testing tiers" section to `AGENTS.md`:

* `make test` (default gate, < ~60 s of test time on the reference host),
* `make test-full` (60 s regression/stress suites; required for search,
  move-ordering, TT/GHI, and proof-tree changes, and before releases),
* `make test-lite` (debug build for quick logic checks),
* the rule that pre-commit hooks must never run `make test-full`.

6.2 Update the "Conventions" section: slow tests are marked with
`#[ignore = "slow: ..."]` and are excluded from the default gate; do not
reintroduce `cfg_attr(debug_assertions, ignore)`.

6.3 If a README or developer docs mention `cargo test --release` as the full
suite, update them to point at `make test-full`.

### 7. (Document-only) Alternative runners and CI

7.1 Document cargo-nextest in the report (not as a requirement): per-test
retries for the remaining wall-clock tests, better parallel scheduling, and
hanging-test isolation. Do not adopt it now — retries on correctness
assertions can mask genuine nondeterministic misclassification, and task 5
shrinks the wall-clock population that would motivate adoption. The make
targets must keep working with the standard harness.

7.2 There is **no CI** (project decision). The enforcement point for "run
`make test-full` for extraordinary changes" is the make targets plus the
`AGENTS.md` conventions, not automation. Record this decision and its
consequence — regressions caught only by the slow tier surface when someone
chooses to run it — in the report.

## Phasing

Land as three PRs:

1. **Relief (mechanical, no logic changes):** tasks 1, 2, 6 — attribute
   conversion, make targets, `AGENTS.md`. Immediately fixes the 25-minute
   default gate.
2. **Smoke suite:** task 3 in its fast-only form (without the `m22` entry).
3. **Deterministic budgets:** tasks 4, 5 — budget API, unit tests, fixture
   conversion; then add the `m22` entry to the smoke suite (task 3.3).

## File changes

* `Makefile`
* `AGENTS.md`
* `tests/stress.rs`, `tests/test_corpus.rs`, `tests/test_decisive.rs`,
  `tests/test_decisive_remaining.rs`, `tests/test_epsilon.rs`,
  `tests/test_move_order.rs`, `tests/test_plan6.rs`, `tests/test_review.rs`
  (attribute conversion only in task 1)
* `tests/test_smoke.rs` (new)
* `tests/fixtures/smoke_positions.txt` (new)
* `tests/fixtures/decisive_remaining.txt` (comments/notes)
* `tests/common/mod.rs` (new budget helpers)
* `src/search/dfpn/mod.rs`, `src/search/dfpn/core.rs` (child-eval budget)
* `src/search/dfpn/tests.rs` (budget unit tests)

## Risks and trade-offs

* **Later regression detection.** Moving the 60 s suites out of the default
  gate means deep-search regressions surface only when someone runs
  `make test-full`. There is no CI to run it automatically (project
  decision), so the make targets, the `AGENTS.md` conventions, and the smoke
  suite are the only enforcement. The node-budget conversions mitigate this
  by moving several deep assertions into the fast tier; the report must
  state the final split (which entries stayed wall-clock).
* **Behavior change of `cargo test --release`.** Anyone relying on it as the
  full suite will silently get the fast tier after task 1. This is the
  intended change; it must be called out in the report and in `AGENTS.md`.
* **Hard-coded solver strength.** Node budgets encode current move-ordering
  strength; they will fail (deterministically, by design) when ordering
  improves. That is the same lifecycle as the current timeout tests, but the
  failure is reproducible. Budgets must be picked with the documented
  headroom so unrelated noise does not trip them.
* **Budget-exhaustion correctness.** The largest correctness risk in this
  plan is caching a partial, budget-cut result as proven. Follow the existing
  unsolved-entry convention exactly and cover it with the task 4.4 tests.
* **Determinism assumptions.** The conversion rests on `child_evals` being
  reproducible across runs and machines (fixed TT size, deterministic
  Zobrist keys and ordering). If any entry proves non-deterministic, keep it
  wall-clock (task 5.6) rather than shipping a flaky test.
* **Compile time is not addressed.** `lto = true, codegen-units = 1` still
  applies to `make test-full` (only `make test` gets the thin-LTO override).
  Reducing the shipped profile is out of scope.

## Verification

```bash
grep -rn "cfg_attr(debug_assertions" tests/        # must return nothing
cargo fmt -- --check
cargo clippy --all-targets
time make test          # target: < ~2 min wall incl. compile, < 60 s test time
time make test-full     # expected: several minutes, all green
make test-lite
cargo test --release --test test_smoke -- --nocapture
```

New behavior to verify explicitly:

* `cargo test --release` no longer runs the 60 s suites (count `test result:`
  lines / skipped tests),
* `cargo test --release -- --include-ignored` runs them and passes,
* a child-eval budget of 0 exits immediately without caching a proven entry
  (unit test from task 4.4),
* converted fixture entries pass repeatedly (`make test` twice in a row) and
  report no `time_exceeded()`-dependent results.

## Final task

Write `docs/plans/testability/report3.md` documenting:

* the decisions taken for the open questions (plain `#[ignore]` selection,
  dedicated `ExitReason::BudgetExhausted`, budget numbers in fixture notes,
  headroom policy `½×`/`3×`, wall-clock keepers, no CI),
* the measured before/after wall times of the default gate and the full suite
  on the reference host (asahi M1),
* how many `cfg_attr(debug_assertions, ignore)` attributes were converted and
  the resulting tier membership of each previously profile-coupled test,
* the smoke-suite composition and its measured wall time,
* the `Search::set_child_eval_budget` API and the exit/caching semantics
  chosen, including any deviation from the plan,
* the fixture conversion table: per `decisive_remaining.txt` entry, the
  measured `child_evals`, the chosen budget, and whether it was converted or
  left wall-clock (with the reason),
* additional tools/examples used, problems encountered, unresolved parts,
  missing tests, and next steps (e.g. nextest adoption — no CI, per project
  decision — and converting any remaining `solvable_60s` entries).
