# Plan 11 Implementation Report

This report documents the implementation of `docs/plans/review/plan11.md`, which
fixes `simulate` cross-path verification so that twins from different paths can
be reused correctly.

## Approach

A **path-code-aware** implementation was chosen over the bounded fresh-search
fallback. The `simulate` function now receives the twin's original `path_code`
and a new `path_length` field, and it computes child path codes with the same
`zobrist::path_random(move, path_length + 1)` formula used by `dfpn`. This lets
`simulate` probe the same path-dependent twin entries that were generated during
the original search.

## Changes made

### `src/search/tt.rs`

- Added `path_length: u32` to `TwinEntry`.
- Updated `TtEntry::store_twin` and `TranspositionTable::store` / `store_twin`
  to accept and store `path_length`.
- Updated the `tt.rs` unit tests that call `store_twin`.

### `src/search/dfpn.rs`

- Added `path_length: u32` to `try_use_tt` and passed it through from `dfpn`
  and `evaluate_child`.
- When a twin from another path is found, `try_use_tt` now calls `simulate`
  with `twin.path_code`, `twin.path_length`, and the node's best move.
- If `simulate` succeeds, `try_use_tt` stores a new twin for the current path
  with the current `path_code` and `path_length`.
- Rewrote `simulate`:
  - Added `path_length` and `remaining_depth` parameters.
  - Uses `path_length + 1` as the depth index for `zobrist::path_random`.
  - Passes `remaining_depth - 1` on recursion.
  - Stops with `false` when `remaining_depth` reaches `0`, independent of
    `sim_stack.len()`.
  - After each move, checks `pos.outcome()` for an immediate terminal result
    before probing the TT. This lets `simulate` verify mates and stalemates
    without requiring a child TT entry.
- Updated all call sites of `tt.store`, `tt.store_twin`, `try_use_tt`, and
  `simulate` to supply `path_length`.
- Added a cross-path twin reuse unit test
  (`try_use_tt_accepts_cross_path_win_twin`) that stores a `Win` twin for a
  rook-mate position under an arbitrary path code and verifies that
  `try_use_tt` accepts it for a different current path.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test
$ cargo doc
$ cargo test --release
```

All commands completed successfully.

Manual CLI check on a transposition-heavy position:

```text
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
outcome: win
pv: f1f7 e8d8 g1g8
```

The solver still returns `win` with a short PV, confirming that the
path-code-aware simulation does not break transposition handling.

## Performance and size notes

- The `tt_entry_size_is_reasonable` test still passes after adding `path_length`
  to `TwinEntry`, so the per-entry size remains within the 512-byte budget.
- `simulate` is now bounded both by `SIM_MAX_NODES` (1000 nodes) and a
  `remaining_depth` counter initialized to `SIM_MAX_DEPTH` (1000 plies), which
  keeps cross-path verification cheap.

## Remaining limitations

- `simulate` relies on the stored twin tree being present and consistent in the
  TT. If a child twin has been evicted, `simulate` falls back to `false` for
  non-terminal positions rather than re-searching the child.
- `SIM_MAX_DEPTH` is a fixed cap; very deep cyclic lines may still fail
  verification, but this is intentional to avoid runaway simulations.
