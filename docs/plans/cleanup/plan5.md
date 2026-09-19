# Cleanup Plan 5 — intermediate code-review & refactoring round

## Context and goal

Nine plans landed since the last housekeeping pass (lean 10, dfpn 14?,
conversion 6–7, proof 8, and the plan13 pre-phase work), including whole
new modules (`src/search/preflight/`, `src/cli.rs`, `src/config.rs`,
`src/tt_snapshot/`, `src/reconstruct/`). This plan runs the user's
three-step structure:

1. **Analyze** — dead code, outdated/unnecessary comments, DRY/YAGNI,
   consistency, unnecessary coupling, missing/unnecessary tests, code
   smells.
2. **Concise issue list** — the `## Findings` section below.
3. **Fix** — in this same session (all items are mechanical and carry no
   hot-path or game-theoretic risk); if anything grows beyond that, it
   is split into a follow-up plan.

Baseline: commit `423de74` ("implement conversion plan7: failure"),
working tree clean, `make test` green, `cargo clippy --all-targets`
0 warnings.

Guardrail: no changes to search semantics, TT layout, or hot-path code.
Verification: `cargo fmt --check`, `cargo clippy --release --all-targets`
0 warnings, `make test`, plus a before/after smoke solve (m19) with
identical stdout.

## Findings (step 2)

Dead code / YAGNI:

1. `Search::set_chunk_increment` (dfpn/mod.rs) — zero callers anywhere.
2. `search::dfpn` public re-exports `outcome_from_pn_dn` and `INF` —
   used only inside `dfpn` internals and their unit tests; no test,
   example, or reconstruct-side caller. Public API surface without a
   consumer.
3. `tests/common/mod.rs` helpers with zero call sites (hidden by the
   blanket `#![allow(dead_code)]`): `assert_solves_with_first_move`,
   `assert_pv_valid`, `solve_refined_moves`,
   `solve_refined_moves_timeout`, `solve_with_timeout`, `pv_from_uci`,
   `assert_solves_first_outcome`.
4. Vestigial `_max_pv_len` parameter on `assert_solves_to` (51 call
   sites) and `assert_solves_to_timeout` (2 call sites) — "kept for
   test compatibility", no longer enforced: a YAGNI artifact.
5. `examples/common.rs`: `decisive_case` has zero users in any example
   (found during a re-check: `M19_FEN` initially looked dead due to a
   flawed name-extraction in the analysis script, but is used by seven
   examples and stays).

Consistency:

6. Test files named after process artifacts instead of behavior:
   `test_plan2.rs`, `test_plan3.rs`, `test_plan5.rs`, `test_plan6.rs`,
   `test_plan7.rs`, `test_lean3.rs`, `test_lean8.rs`, `test_review.rs`.

Leave-as-is (recorded with rationale):

- `STARTPOS_FEN` duplication between `src/cli.rs` and
  `src/position.rs` — documented decision: the CLI module is
  deliberately `std`-only. Keep.
- `MoveOrderCase`/`parse_move_order_fixture` duplication between
  `tests/common/mod.rs` and `examples/common.rs` — example binaries and
  integration tests cannot share modules; the fixture files themselves
  are single-sourced via `include_str!`. Keep.
- `(planN)`/`(reportN)` tags in `src` comments — spot-checks found they
  all carry genuine rationale (e.g. the plan9 repetition-cache comment
  explains *why* the cache exists and where it must sit relative to the
  TT check); the user-facing `--help` text is already free of process
  vocabulary. Keep.
- `#[allow(clippy::too_many_arguments)]` (3 sites) — hot-path
  constructors; threading a context struct through them would cost
  performance. Keep.
- `examples/common.rs` `#![allow(dead_code)]` /
  `#[allow(dead_code)] fn main() {}` — the standard shared-example
  helper pattern. Keep.
- `Search::tt_stats()` returning a 5-tuple — a struct would be nicer,
  but it changes a public accessor used by `twin_stats`, tests, and
  reconstruct for zero behavioral gain; not worth the churn this round.

Missing tests: none identified beyond plan4's closures — the corpus,
replay validator, deterministic eval budgets, and per-module unit tests
cover the current surfaces (preflight, tt_snapshot, reconstruct, config
all ship with tests).

## Task 1 — Dead code removal

- Remove `Search::set_chunk_increment` (field `chunk_increment` stays if
  still used internally — verify; remove if it becomes write-only).
- De-publish `outcome_from_pn_dn` (`pub fn` → `pub(super) fn` in
  `core.rs`) and drop both re-exports from `dfpn/mod.rs`; fix the
  `core.rs` test import. Internal users (`children.rs`, `core.rs`,
  `pv.rs`) switch to `crate::zobrist::INF` / local paths as needed.
- Delete the seven dead helpers from `tests/common/mod.rs`; drop the
  `_max_pv_len` parameters (mechanically update all call sites).
- Delete `decisive_case` from `examples/common.rs`; update the header
  comment that describes the mirror relationship.

## Task 2 — Test-file renames

Behavior-descriptive names, content unchanged:

- `test_plan2.rs` → `test_terminal_outcomes.rs`
- `test_plan3.rs` → `test_longest_defense.rs`
- `test_plan5.rs` → `test_pv_refinement.rs`
- `test_plan6.rs` → `test_deep_outcomes.rs`
- `test_plan7.rs` → `test_cli_proof_pipeline.rs`
- `test_lean3.rs` → (name chosen from content during implementation)
- `test_lean8.rs` → `test_approach_map.rs`
- `test_review.rs` → `test_regressions.rs`

(`test_epsilon.rs`, `test_inf.rs` already behavior-named; keep.)

## Non-goals

- No API redesign (tt_stats struct, ProofSink trait, …).
- No comment rewrites beyond what dead-code removal invalidates.
- No dependency changes.

## Verification

`cargo fmt --check && cargo clippy --release --all-targets && make test`
plus the before/after m19 smoke solve. Per repo convention this plan
ends with `report5.md` in this directory.
