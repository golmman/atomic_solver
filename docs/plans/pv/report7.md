# Report: Remove PPV extraction and validation from `Search`

## Summary

Implemented `docs/plans/pv/plan7.md`.

`Search` now focuses on finding the decisive `Outcome`. The PV returned by
`Search::solve` is an informational best-effort line extracted from the
transposition table's `best_move` chain; it is not validated as a proof. The
PPV reconstruction helpers (`extract_ppv` / `extract_pv_checked`) were removed
from `Search`, the fallback re-search in `solve_with_progress` was removed, and
the CLI no longer prints `ppv_valid` or `proof_tree_ppv`. Proof generation will
be addressed separately by the proof-tree layer.

## Changes

### 1. `src/search/dfpn/pv.rs`

- Removed `extract_ppv`, `extract_ppv_internal`, and `extract_pv_checked`.
- Kept `extract_pv` as the single PV extractor and `validate_pv` as a public
  utility for external tools/tests that want to validate a line themselves.
- Updated the module doc to state that the PV is informational and added a
  >10 KB cohesion justification.

### 2. `src/search/dfpn/mod.rs`

- `bounded_search` now uses only `self.extract_pv(pos)` for the PV.
- `solve_with_progress` no longer calls `validate_pv` and no longer performs a
  fallback re-search when the PV is invalid. Iterative refinement is kept as a
  best-effort bound-tightening step, but it no longer validates the PV.

### 3. `src/main.rs`

- Removed `proof_tree_ppv` and `ppv_valid` output from the pre-exit hook.
- The CLI still prints `outcome:`, `pv:`, and writes `proof_tree_dump` when a
  proof-tree worker is active, but it does not claim the PV is valid.
- Updated `--first-outcome` help text from "shortest-PV refinement" to
  "iterative PV refinement".

### 4. Test helpers (`tests/common/mod.rs`)

- `assert_solves_to` and `assert_solves_to_timeout` now assert only the expected
  `Outcome`. They still accept an optional `_max_pv_len` argument for
  compatibility with existing call sites, but it is no longer enforced.
- Removed `assert_solves_with_first_move` and `assert_pv_valid` from the common
  helpers.

### 5. Test updates

- `tests/test_plan5.rs`, `tests/test_epsilon.rs`, `tests/test_review.rs`,
  `tests/test_ghi.rs`, `tests/test_cli.rs`, `tests/test_corpus.rs`, and
  `tests/test_plan6.rs`: removed all `assert_pv_valid`, `Search::validate_pv`,
  `assert_solves_with_first_move`, and exact-PV assertions. Tests now check
  only outcomes, presence of `pv:`/`outcome:` lines, or non-empty PVs where
  appropriate.
- `tests/test_proof_tree.rs`: tests that validated the solver's PV against the
  proof tree were ignored with a note that proof-tree PV validation is deferred.
  The remaining proof-tree serialization and structure tests were kept.

### 6. `AGENTS.md`

- Updated the architecture and output-priority sections to describe the
  informational PV and the separation of proof generation into the proof-tree
  layer.

## Files changed

| File | What changed |
|------|--------------|
| `src/search/dfpn/pv.rs` | Removed `extract_ppv` / `extract_pv_checked`; kept `extract_pv` and `validate_pv`. |
| `src/search/dfpn/mod.rs` | Simplified `bounded_search`; removed `validate_pv` calls and fallback re-search. |
| `src/main.rs` | Removed `ppv_valid` / `proof_tree_ppv` output; updated help text. |
| `tests/common/mod.rs` | `assert_solves_to*` now outcome-only; removed `assert_pv_valid` / `assert_solves_with_first_move`. |
| `tests/test_plan5.rs` | Removed PV validation and first-move assertions. |
| `tests/test_epsilon.rs` | Removed `assert_pv_valid` calls. |
| `tests/test_review.rs` | Removed `assert_pv_valid` / `Search::validate_pv` and exact PV assertions. |
| `tests/test_ghi.rs` | Removed `Search::validate_pv` calls. |
| `tests/test_cli.rs` | Removed `ppv_valid` assertions and PV validation test. |
| `tests/test_corpus.rs` | Corpus test now asserts outcome only. |
| `tests/test_plan6.rs` | Removed exact-PV / length assertions; kept outcome checks. |
| `tests/test_proof_tree.rs` | Ignored PV-validation tests; kept serialization/structure tests. |
| `AGENTS.md` | Documented informational PV and proof-tree separation. |
| `docs/plans/pv/report7.md` | This report. |

## Verification

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --release
cargo doc --no-deps
```

All passed with no warnings.

The release test suite included the previously slow positions from
`tests/test_plan6.rs` and `tests/test_corpus.rs`:

```text
running 20 tests
test m22_black_loses ... ok
test m22_white_wins ... ok
test m23_black_loses ... ok
test m23_white_wins ... ok
test m24_black_loses ... ok
test m24_solve_with_pv ... ok
test m24_white_wins ... ok
test m25a_black_loses ... ok
test m25a_white_wins ... ok
test m25b_black_loses ... ok
test m25b_white_wins ... ok
test m26_black_loses ... ok
test m27_kh7_fast_win_with_commoners ... ok
test m27_ppv_only ... ok
test m27_streaming_output ... ok
test m27_white_wins ... ok
test m28_black_loses ... ok
test m29_black_loses ... ok
test m29_white_wins ... ok
test timeout_message ... ok

test result: ok. 20 passed; 0 failed; 0 ignored
```

`tests/verify_ppv.rs` also passed in release:

```text
running 7 tests
test illegal_move_is_not_ppv ... ok
test legal_non_decisive_first_move_is_not_ppv ... ok
test long_line_is_valid_ppv ... ok
test mate_in_one_is_ppv ... ok
test non_decisive_final_is_not_ppv ... ok
test verified_ppv_one ... ok
test verified_ppv_two ... ok

test result: ok. 7 passed; 0 failed; 0 ignored
```

## Manual regression

The originally reported FEN now prints an outcome and an informational `pv:`
line, with no `ppv_valid:` line:

```bash
cargo run --release -- \
  --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K b - - 0 25" \
  --timeout 60
```

Output (excerpt):

```text
outcome: loss length: 8
pv: g8g7 b1b8 c5c4 b8f8 g7h7 f8h8 h7g7 h8f8
pre_exit: reason=Complete outcome=loss nodes=969636
proof_tree: nodes=169 win=84 loss=85 root_depth=18
proof_tree_dump: proof_tree.bin
```

The PV may be cyclic/incomplete, which is expected now that `Search` does not
validate it.

## Notes and open ends

- The optional `_max_pv_len` parameter in `assert_solves_to` and
  `assert_solves_to_timeout` was kept to avoid touching every test call site;
  it is documented as ignored and may be removed later.
- `validate_pv` and `validate_pv_prefix` remain public in `pv.rs` so external
  verifiers (e.g. the `verify_ppv` example) and future proof-tree work can reuse
  them.
- `Search::search_depth_with_prefix` still uses `bounded_search` and the
  returned `pv.len()` as the proven depth for the `verify_ppv` example; this
  worked for the existing verifier tests but may need attention once the proof
  tree takes over PPV extraction.
- The proof-tree tests that validate the solver's PV are `#[ignore]`d pending a
  dedicated proof-tree PPV extractor.
