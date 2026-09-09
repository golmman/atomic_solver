# Lean Plan 4 — TT capacity default: `--tt-size` 64 MB → 128 MB

Implements item **#16** of the lean initiative
(`docs/plans/lean/research_tt_capacity.md`). Unlike plans 1–3 this is an
**intentionally behavior-changing** plan: search trajectories shift on
positions where the 64 MB table is capacity-bound, so the bit-identical
drift protocol does not apply. The validator is the move-order benchmark
suite (the hard tier), plus the quick suite as a shallow-control group.

## Rationale (from the research)

At the 64 MB default the m22_white first-outcome search fills the table
(~1.02M live entries for 2.45M nodes, ~60% churn) and does **2.6× the
work / 2.0× the wall** of a 128 MB table (37.5M → 14.2M child evals,
5.26 s → 2.57 s). The quick suite is size-insensitive (±2% aggregate).
Above 128 MB there is no monotone trend (chaotic DF-PN trajectory
sensitivity) and wall time degrades from cache misses (nps 434k → 238k at
1 GB). Entry slimming cannot close a 2× capacity gap (`INF = 1 << 60`
requires u64 pn/dn).

## Changes

1. `src/cli.rs`: `tt_size` default `64` → `128`; update the unit test
   asserting the default.
2. `src/main.rs`: doc-comment and `--help` text default 64 → 128.
3. `examples/benchmark.rs`: `tt_size` default `64` → `128` (the optimizer
   evaluator must match the CLI default; both are listed in the spec).
4. `docs/spec/optimizer_interface.md`: example JSON `"tt_size": 64` →
   `128` (standalone-spec rule: no repo-internal references).
5. `AGENTS.md`: CLI bullet default 64 → 128.
6. Re-baseline `tests/fixtures/m22_default_stdout_golden.txt` from the
   post-change binary (default mode, `--timeout 20 --outcome-only`), and
   refresh the header comment in `tests/test_lean3.rs` that ties the
   golden to plan3's drift protocol.

Explicit `Search::new(64)` call sites in tests/examples are left alone:
they pin deterministic fixtures, not defaults.

## Validation

1. **Before/after on the hard tier**: `benchmark --suite move-order
   --json --first-outcome --runs 1 --timeout 30` at `--tt-size 64` and
   `--tt-size 128` (same binary, explicit sizes). Acceptance: aggregate
   `child_evals` improves and no case regresses beyond the chaotic-regime
   noise; no case flips to `status != ok` or a wrong outcome.
2. **Quick suite control**: 59/59 `status == ok`, no wrong outcomes,
   aggregate within the researched ±2% band (either direction is
   acceptable and documented).
3. **m22 default mode**: stdout re-derived and stored as the new golden;
   the PV must still be a Win with a valid proof (checked via the
   `verify_ppv` example).
4. Fast test tier green (`cargo test --release`); the slow golden test
   re-run explicitly.
5. `--help` output and spec JSON consistent with the new default.

## Deliverable

`report4.md` in this directory, including the measurement tables, the
golden re-baseline, spec/AGENTS updates, and any deviations.
