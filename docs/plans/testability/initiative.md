# Initiative: `testability` — test suite structure and determinism

## Status

**Closed.** Plans 1–3 are done (`report1.md`–`report3.md`); last
activity 2026-08-29. The conventions it established (test tiers,
`#[ignore = "slow: ..."]` gating, deterministic child-eval budgets
instead of wall-clock assertions) are normative in AGENTS.md now; the
last open item (m22 flakiness) was resolved by `cleanup` report3.

## Arc (summary)

- **plan1** — strengthened the suite; audited stale `#[ignore]`d tests;
  found the unsolved-`Loss` position that seeded the `pv/` initiative.
- **plan2** — refactored for unit-testability (CLI module under
  `#[cfg(test)]`, `Position::try_do_move` `pub(crate)`, TT
  `with_capacity`, assertion helpers in `tests/common`).
- **plan3** — tiered tests: fast gate (`make test`) ≈ 1 min,
  `make test-full` for the slow tier, `make test-lite` debug run;
  converted wall-clock caps to deterministic
  `child_eval_budget`/`ExitReason::BudgetExhausted` assertions.

## Follow-ups resolved elsewhere

- `m22_black_loses` wall-clock dependency → deterministic tripwire
  (`cleanup/report3.md`).
- `#[cfg_attr(debug_assertions, ignore)]` gating convention violation →
  converted to plain `#[ignore = "slow: ..."]` (`dfpn/initiative.md`
  history, 2026-09-12).
