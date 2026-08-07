# Plan 7: Remove PPV extraction and validation from `Search`

## Summary

Decouple the search from proof generation: `Search` is responsible for finding
the correct `Outcome`; the returned PV is an informational best-effort line from
the transposition-table `best_move` chain. Remove `extract_ppv` and
`extract_pv_checked`, stop validating the PV inside `Search::solve`, and stop
printing `ppv_valid` from the CLI. Proof-tree correctness will be addressed in
a later plan.

## Goal

- `Search::solve` and `Search::search_depth` return an `Outcome` and an
  optional/informational `Vec<Move>` PV.
- The PV is extracted with the cheap `extract_pv` (follow `best_move` chain)
  only.
- No PPV reconstruction, no `validate_pv` calls, and no fallback re-search inside
  the solver.
- The CLI prints `pv:` without a `ppv_valid` line.
- Tests assert only `Outcome` (and maybe a non-empty PV for decisive,
  non-terminal positions), not PV validity or exact move sequence.
- `cargo test`, `cargo clippy --all-targets`, and `cargo fmt --check` pass.

## Non-goals

- Do not change the core `dfpn` search, TT layout, move ordering, or proof-tree
  worker.
- Do not implement a new proof-tree PPV extractor now.
- Do not redesign the public `Search` return type (keep `Vec<Move>`; document
  that it is informational).

## Background

`Search` currently attempts to produce a verifiable PPV:

1. `bounded_search` tries `extract_ppv` → `extract_pv_checked` → `extract_pv`.
2. `solve_with_progress` re-searches with a fresh TT generation if `validate_pv`
   fails.
3. The CLI prints `ppv_valid: true/false` after validating against the proof
   tree and a fresh board replay.

This couples outcome search to proof generation. The transposition table is
reliable for `Outcome` but not for a sound `best_move` chain in the presence of
transpositions, so the search keeps adding validation/re-search layers to fix
PV extraction. Instead, we accept that the solver's PV is a
debugging/informational hint and leave sound proof generation to the proof tree
later.

## Design

### 1. `src/search/dfpn/pv.rs`

Remove:
- `extract_ppv` and `extract_ppv_internal`.
- `extract_pv_checked`.

Keep:
- `extract_pv` / `extract_pv_internal` as the single PV extractor.
- `validate_pv` / `validate_pv_prefix` as public utilities for external
  tools/tests that want to validate a line themselves.
- Update the module doc to note that `extract_pv` is informational.

Remove the `std::collections::HashMap` import if it becomes unused.

### 2. `src/search/dfpn/mod.rs`

In `bounded_search`, simplify to:

```rust
let pv = self.extract_pv(pos);
(outcome, pv)
```

In `solve_with_progress`:

- Remove the fallback re-search block that triggers when `validate_pv` fails.
- Remove the `validate_pv` check in the iterative refinement loop.
- Keep `on_progress` and refinement using `pv.len()`, but document that the
  refinement is best-effort and the PV is not guaranteed to be a proof.

`emit_proof_tree` can stay as-is for now; the CLI will stop validating the PV
against it.

### 3. `src/main.rs`

In the pre-exit hook, remove:

```rust
println!("proof_tree_ppv: {}", pv_str(pv));
let valid = tree.validate_ppv(pv) && Search::validate_pv(pv, &fresh, outcome, None);
println!("ppv_valid: {valid}");
```

Keep `pv:` printing and keep writing `proof_tree_dump` if the proof-tree worker
is active, but without validation.

### 4. `tests/common/mod.rs`

- Change `assert_solves_to(fen, expected, max_pv_len)` to
  `assert_solves_to(fen, expected)` and remove all PV validation/length checks.
- Same for `assert_solves_to_timeout`.
- Remove `assert_solves_with_first_move` and `assert_pv_valid` from `common`
  (or move them to a test-only proof-validation module if useful for
  `verify_ppv` tests; they should not be called from solver tests).

### 5. Test updates

Update tests that currently rely on PV validity:

- `tests/test_plan5.rs`: replace `assert_solves_with_first_move` with
  `assert_solves_to`; remove `assert_pv_valid` calls.
- `tests/test_epsilon.rs`: remove `assert_pv_valid`; only assert outcome and
  a non-empty PV for decisive positions.
- `tests/test_review.rs`: remove `assert_pv_valid` calls.
- `tests/test_plan6.rs`: change `assert_solves_to` to two-argument; remove or
  `#[ignore]` tests that assert exact PV sequences (`m27_ppv_only`,
  `m27_shortest_pv`, `m27_kh7_fast_win_with_commoners` exact PV). The new
  `m25b_black_loses` test should use `assert_solves_to` without max length.
- `tests/test_proof_tree.rs`: tests that validate the solver's PV against the
  proof tree should be removed or `#[ignore]` pending proof-tree work.
- `tests/test_cli.rs`: update `cli_pv_validates_for_decisive_position` to only
  parse `outcome:` and `pv:`; rename if needed.
  `cli_first_outcome_validates_and_dumps_proof_tree` can be renamed to
  `cli_first_outcome_dumps_proof_tree` and no longer assert `ppv_valid`.

## Testing and verification

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --release
cargo doc --no-deps
```

Manual CLI regression:

```bash
cargo run --release -- --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K b - - 0 25"
```

Expected output contains `outcome: loss` and a `pv:` line, but no `ppv_valid:`
line.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Losing regression coverage for exact PVs. | Keep all FENs in tests for outcome coverage; move PV assertions to `#[ignore]` or delete and re-add with the proof-tree plan. |
| Iterative refinement uses an incorrect/incomplete PV length. | Document that refinement is best-effort; outcome correctness is unchanged. If needed, disable refinement for `--first-outcome` (already the case). |
| Users may still expect a valid PV from the CLI. | Update CLI help/docs to state `pv:` is an informational line, not a verified proof. |
| `extract_pv` returns empty for some decisive positions. | This was already possible for Draw; for decisive it follows `best_move` and usually returns a non-empty line. Add a non-empty check only if required by tests. |

## Success criteria

- `extract_ppv`, `extract_ppv_internal`, and `extract_pv_checked` are removed
  from `src/search/dfpn/pv.rs`.
- `Search::solve` / `bounded_search` use only `extract_pv`.
- `Search::solve` no longer calls `validate_pv` or performs fallback re-search.
- `main.rs` no longer prints `ppv_valid` or `proof_tree_ppv`.
- `cargo test --release`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo fmt --check` pass.
- The reported FEN prints `outcome: loss` and a `pv:` line without `ppv_valid`.

## Open ends for follow-up plans

- Rebuild proof-tree PPV extraction/validation as a separate post-processing
  step.
- Decide whether `Search` should eventually return `Option<Vec<Move>>` to make
  the optional nature explicit in the type system.
- Add a `--validate-pv` CLI flag or `verify_ppv`-style tool for users who want a
  checked PPV.
