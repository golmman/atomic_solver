# Report: Report Refinement-Round Termination Status (cap-cut vs. natural exhaustion)

## Summary

Implemented `docs/plans/dfpn/plan8.md`. `bounded_search` now reports *why* it
returned without a decisive line (`RoundTermination::Decisive | Exhausted |
CapCut | ResourceCut`), `solve_with_progress` maps the last round's
termination onto the new public `Search::pv_status()` (`PvStatus::None |
FirstOutcome | ProvenShortest | Unproven`), and the CLI prints
`pv_status: proven-shortest | first-outcome | cap-cut | cut-short` after the
`pv:` line for decisive outcomes. Zero search-behavior change (drift-verified
below).

## What shipped

- `src/search/dfpn/mod.rs`
  - Private `RoundTermination` enum; `bounded_search` now returns
    `(Outcome, Vec<Move>, RoundTermination)`. Break-site mapping per the
    plan's table: decisive → `Decisive`; `call_max_work == 0` → `CapCut` when
    `round_remaining < remaining_budget`, else `ResourceCut` (tie →
    `ResourceCut`, conservative); loop-condition exit → `ResourceCut` (the
    enum default); `work_done < call_max_work` → `Exhausted`, **unless** a
    resource limit also fired during the final chunk (`time_exceeded()` /
    `child_eval_budget_exceeded()` re-checked before claiming exhaustion —
    the plan's "conservative exhaustion" direction).
  - Public `PvStatus` enum with doc comments carrying the plan's semantics
    wording (length, not validity; not a PPV validation).
  - `Search::pv_status()` and `Search::last_refine_round_cap_cut()`
    accessors; both reset in `begin_run`.
  - Refinement-loop status bookkeeping: every break site sets an explicit
    status; the loop-condition fallback distinguishes `n <= 2` →
    `ProvenShortest`, improving-then-cut → `Unproven`, never-improved →
    `FirstOutcome`. `search_depth` / `search_depth_with_prefix` ignore the
    termination (signatures unchanged).
  - `solve`/`solve_with_progress` doc comments: PV is proven shortest iff
    `pv_status() == ProvenShortest`.
- `src/main.rs` — prints the `pv_status:` line for decisive outcomes
  (`Unproven` splits into `cap-cut` / `cut-short` via
  `last_refine_round_cap_cut()`); module doc updated.
- `src/search/dfpn/tests.rs` — 8 new fast tests (below); comment fix in
  `refine_cap_bounds_round_work`.
- `tests/test_cli.rs` — 3 new integration tests (proven-shortest,
  first-outcome, draw-prints-nothing).
- `tests/fixtures/m22_default_stdout_golden.txt` — re-baselined (added
  `pv_status: cap-cut`); provenance note added in `tests/test_lean3.rs`.
- `AGENTS.md` — CLI bullet and "Output priorities" section mention
  `pv_status` / `PvStatus`.

No changes to `ExitReason`, the `ProofEvent` protocol, the proof-tree layer,
the benchmark JSON, or `docs/spec/optimizer_interface.md`.

## Tests

Fast tier, all in `src/search/dfpn/tests.rs` unless noted:

1. `bounded_search_reports_exhaustion_below_mate_depth` — KRR vs K at
   `max_depth = 1` → `Draw` + `Exhausted`.
2. `converged_fixture_is_proven_shortest` — `REFINE_FIXTURE_FEN` with default
   settings → 48-ply PV + `ProvenShortest` + cap flag unset.
3. `refine_cap_cut_reports_unproven_status` — round cap 100 → `Unproven` +
   `last_refine_round_cap_cut()`.
4. `resource_cut_reports_unproven_without_cap_flag` — promotion fixture with
   a global budget (8,000) that admits the first outcome (7,449 evals) but
   cuts the first refinement round → `Unproven` without the cap flag.
5. `first_outcome_only_reports_first_outcome_status`.
6. `drawn_position_reports_none_status` — bare kings (terminal
   `occupied == 2` draw) → `PvStatus::None`.
7. `bounded_search_reports_resource_cut_on_budget` /
   `bounded_search_reports_cap_cut_on_round_cap` — direct break-site
   attribution tests via the (private, submodule-visible) `bounded_search`.
8. CLI: `cli_prints_pv_status_proven_shortest_for_mate_in_one`,
   `cli_prints_pv_status_first_outcome` (promotion fixture),
   `cli_draw_prints_no_pv_status` (in `tests/test_cli.rs`).

Plus the plan's no-drift item: all pre-existing tests
(`solve_twice_identical_counts`, `default_refine_cap_leaves_improving_rounds`,
chunk-trajectory tests, m22 golden) pass unchanged.

## Fixture used for the `ProvenShortest` test

`REFINE_FIXTURE_FEN` (`r7/1Rp4k/4P2B/6p1/3P2P1/p6p/P6K/8 b - - 0 35`) with
default settings needed **no adjustment**: the run refines 50 → 48 plies and
the final round (bound 46) exhausts naturally (`Exhausted`), so the test is
deterministic as-is (verified in release and debug). Measured round costs on
the reference host: phase 1 = 102,690 child evals; round 1 (bound 48,
decisive-improving) ≈ 52 evals; round 2 (bound 46, natural exhaustion)
≈ 340 evals. The fixture's refinement is heavily TT-cushioned by phase 1,
which matters for the problems below.

## Tools/examples used

- `cargo test`/`fmt`/`clippy`/`doc`, `make test` (26/26 test binaries green).
- `benchmark --suite quick --json --first-outcome` for the drift check.
- A temporary scratch probe unit test (direct `bounded_search` calls across
  round caps `u64::MAX … 100`) to locate the actual cap-cut window; deleted
  after debugging.
- CLI runs on `4R1K1`, the promotion fixture, bare kings, dec44, and m22.

## Verification (drift checks)

1. `benchmark --suite quick --json --first-outcome`, before vs. after:
   every case's `nodes`, `child_evals`, `pv_len`, `outcome`, `wrong`,
   `timeout` identical (only wall-clock `total_time` differs). Status
   reporting touches no search decision.
2. m22 default-mode run (`4r2k/3p4/… w - - 0 22`, `--timeout 20
   --outcome-only`) before vs. after: stdout byte-identical except the added
   `pv_status: cap-cut` line. The golden was re-baselined accordingly; the
   pre-existing lines are unchanged and the `test_lean3` golden test passes.
3. `cargo fmt --check`, `cargo clippy --all-targets`, `cargo doc`,
   `make test`, `cargo test` (debug) all clean.
4. Manual CLI checks: dec44 default → `pv_status: proven-shortest`; m22 →
   `pv_status: cap-cut` (with the default factor, and with
   `--refine-cap 0.001`).

## Problems encountered

1. **Plan test 3's setup does not produce a cap-cut.** The plan expected
   `set_refine_round_cap_for_test(1_000)` to yield `Unproven` with the
   cap-cut cause. Empirically the fixture's rounds are so cheap (≈52 +
   ≈340 evals) that a 1,000-eval cap never binds: the final bound-46 round
   exhausts *naturally* under it, correctly yielding `ProvenShortest`.
   Fixed by lowering the test cap to 100 (probe-verified window: ≤100 binds,
   ≥500 does not) and correcting the stale "cap binds on the very first
   round" comment in `refine_cap_bounds_round_work`.
2. **Plan verification item 4 (`--refine-cap 0.001` → cap-cut on dec44) is
   unachievable on this fixture**: `MIN_REFINE_ROUND_EVALS` (1M floor) makes
   the effective cap `max(1M, 0.001 × 102,690) = 1M`, which never binds.
   The m22 position cap-cuts even with the default 0.25 factor and was used
   for the manual cap-cut check instead.
3. **Unit tests calling `bounded_search` directly must call `begin_run()`
   first**: `Search::new` leaves `deadline = Instant::now()`, so without it
   every call immediately reports `ResourceCut`. Two of the new tests
   document this.
4. **False-exhaustion under small round caps (pre-existing, now visible).**
   With a small `max_work`, dfpn's work-bounded "explored" marking can end a
   round via the exhaustion break site (`work_done < call_max_work`) even
   though the cap shaped the search — e.g. the cap-1,000 run of the plan's
   original test-3 setup reported `Exhausted` on a round that an uncapped
   search would have made decisive. This is the pre-existing meaning of the
   exhaustion break site; plan8 only labels it, per the plan's stance that
   `ProvenShortest` holds "within the solver's search semantics". In
   production the 1M cap floor keeps rounds effectively uncapped unless they
   genuinely exceed 1M evals, but the label is only as trustworthy as the
   underlying break site. See next steps.
5. **Mate-in-1 + `--first-outcome`** reports `ProvenShortest`, not
   `FirstOutcome`: the `pv_len <= 2` rule (enum doc: "a 1-move win cannot be
   shortened") takes precedence over the `first_outcome_only` row of the
   plan's mapping table. The CLI first-outcome integration test therefore
   uses the promotion fixture (7-ply first PV) rather than `4R1K1`
   (mate in 1).

## Unresolved parts

- None functional. The `PvStatus::None` arm of the CLI label match is
  unreachable for decisive outcomes and maps defensively to `first-outcome`.

## Missing tests

- No fast integration test covers the `cap-cut`/`cut-short` CLI labels (the
  only known cap-cut position, m22, needs a ~20 s run; the golden test
  already asserts its `pv_status: cap-cut` line in the slow tier).
- The stop-flag and memory-limit flavors of `ResourceCut` are not directly
  tested (only the child-eval-budget flavor).
- The `cut-short` label (resource cut or decisive-not-shorter after an
  improving round) has no dedicated CLI test; the unit-level mapping is
  covered by `resource_cut_reports_unproven_without_cap_flag`.

## Next steps

- The natural follow-up named by the plan: a "continue after cap-cut" plan
  (resuming refinement after a cap-cut when budget remains), which changes
  search behavior and must respect the deterministic `child_eval_budget`
  contract.
- Optionally, make the exhaustion break site trustworthy under finite caps
  (e.g. have `dfpn` distinguish "all children genuinely resolved" from
  "children marked explored by the work-bounded heuristic") so
  `ProvenShortest` under a binding cap cannot rest on stuck bounds. Out of
  scope here by the plan's zero-drift constraint.
- The proof-tree layer could eventually consume `PvStatus` for its own
  PPV-quality reporting; deliberately not coupled in this plan.
