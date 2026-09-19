# Report: Plan 5 — intermediate code-review & refactoring round

Implements `plan5.md` (analyze → issue list → fix). All step-3 items were
small and mechanical, so the whole plan was executed in one session; no
follow-up plan was needed.

## Baseline / verification

- Baseline commit `423de74`, clean tree, `make test` green (347/0),
  clippy `--all-targets` 0 warnings.
- Applied: `cargo fmt` clean, `cargo clippy --release --all-targets`
  0 warnings, `cargo doc --no-deps` warning-free, `make test` 347/0.
- Drift check (before vs. after release binary): the queen-corner
  preflight smoke is byte-identical, and the m22-black unproven run
  shows an identical deterministic chunk-work sequence
  (`work_done=500004, 1000024, … 8000006`) — search trajectory
  unchanged, as required.

## Applied fixes

Dead code / YAGNI:

- Removed `Search::set_chunk_increment` (zero callers); the
  `chunk_increment` field stays — it is read by the bounded-search
  chunk-growth logic.
- Deleted `outcome_from_pn_dn` outright. The re-check prompted by its
  visibility change showed it had *no* callers at all outside its own
  unit test (dfpn report5 had already flagged it as unused), and it
  could not distinguish `Loss` from `Draw` — a latent footgun. The
  public `pub use` re-exports of `outcome_from_pn_dn` and `zobrist::INF`
  from `search::dfpn` were removed with it; internal users
  (`children.rs`, `core.rs`, `selection.rs`, `mod.rs`) now import
  `INF` from `crate::zobrist` directly.
- `tests/common/mod.rs`: deleted the seven dead helpers
  (`assert_solves_with_first_move`, `assert_pv_valid`,
  `solve_refined_moves`, `solve_refined_moves_timeout`,
  `solve_with_timeout`, `pv_from_uci`, `assert_solves_first_outcome`)
  and the unused `M19_FEN` copy. Dropped the vestigial `_max_pv_len`
  parameter from `assert_solves_to` (51 call sites updated) and
  `assert_solves_to_timeout` (2 call sites).
- `examples/common.rs`: deleted `decisive_case` (zero users).
- The blanket `#![allow(dead_code)]` in `tests/common/mod.rs` now
  carries a justification comment: each integration-test binary
  compiles the shared module independently, so most binaries see unused
  items — the allow is scoped to that artifact, not to hiding dead code.

Consistency — test-file renames (content unchanged, cross-references in
`test_move_order.rs`/`test_proof_validate.rs` updated):

| Old                 | New                          |
| ------------------- | ---------------------------- |
| `test_plan2.rs`     | `test_terminal_outcomes.rs`  |
| `test_plan3.rs`     | `test_longest_defense.rs`    |
| `test_plan5.rs`     | `test_pv_refinement.rs`      |
| `test_plan6.rs`     | `test_deep_outcomes.rs`      |
| `test_plan7.rs`     | `test_cli_proof_pipeline.rs` |
| `test_lean3.rs`     | `test_trajectory_golden.rs`  |
| `test_lean8.rs`     | `test_approach_map.rs`       |
| `test_review.rs`    | `test_regressions.rs`        |

Leave-as-is items (rationale recorded in `plan5.md`): `STARTPOS_FEN`
duplication, the tests/examples fixture-parser mirror, `(planN)` comment
tags in `src` (all carry genuine rationale; user-facing help text is
clean), `#[allow(clippy::too_many_arguments)]` on the three hot-path
ctors, `tt_stats()`'s 5-tuple return.

Net diff: 22 files, +42 / −1431 lines (most of it the renames plus
helper/param removal).

## Additional tools / examples used

- Python one-off scripts for the mechanical `_max_pv_len` call-site
  rewrite (top-level-comma-aware, multiline-call handling) and the
  helper deletion in `tests/common/mod.rs`; output reviewed by diff.

## Problems encountered

- **Flawed analysis step caught by the compiler:** the initial
  helper-usage scan extracted names with a trailing colon
  (`awk '{print $3}'` on `pub const M19_FEN: …` yields `M19_FEN:`),
  which made call sites like `common::M19_FEN` read as 0. `M19_FEN` was
  briefly deleted and restored once `cargo clippy --all-targets`
  flagged the seven example users. `decisive_case`'s death was
  re-confirmed with a clean grep. Lesson recorded: for this kind of
  audit, trust the compiler/`clippy` over grep-based usage counts.
- `cargo check --release --tests` surfaces the per-binary
  dead-code warnings from `tests/common/mod.rs` that the old blanket
  allow used to hide; the justification comment now documents this.

## Unresolved parts / deferred

- None from this plan's list. The recorded leave-as-is items are the
  standing answer to their findings.

## Missing tests

- None identified. The existing suite (corpus, validator, deterministic
  eval budgets, per-module unit tests) covers the touched surfaces; all
  changes are deletions/renames with no new behavior to test.

## Next steps

- The `cleanup` initiative stays active-as-needed; the next trigger is
  the usual one (a batch of landed plans leaving named follow-ups).
- `dfpn/mod.rs` (40 KB) remains above the ~20 KB split guideline; its
  header justification only claims the 10 KB threshold. If it keeps
  growing, a follow-up plan could extract the config/setter surface
  into a submodule — deliberately not attempted here, since moving
  `Search` fields across modules is churn without a measured payoff.
