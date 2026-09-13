# Initiative: `tune` — external optimizer interface

## Status

**Closed.** Single plan (plan1, 2026-08-13), done (`report1.md`). The
contract it implemented is normative in
`docs/spec/optimizer_interface.md` and summarized in AGENTS.md
("Tuning workflow"); parameter search itself is the external optimizer's
responsibility by design.

## Outcome

- `benchmark --json` / `--output-file` with a schema-tested integration
  test (`tests/test_benchmark_json.rs`).
- `Suite::Quick` (~12 s wall, 23/23 solved) and `Suite::Thorough`
  (~2.5 min, mixed solved/timeout — the m20–m22 pairs time out by
  design at 5 s) as the optimizer-facing evaluation suites.
- Documented `ScorerParams` validation constraints in the spec.

## Notes / possible future work (optimizer-side by contract)

- Stricter schema validation if the external optimizer needs it.
- Baseline generation stays with the optimizer.
- History/killer constants are outside `ScorerParams` (`lean` backlog
  #10 owns any change there).
