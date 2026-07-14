# DF-PN+ Solver Implementation Report

## Summary

Implemented a sequential DF-PN+ solver for atomic chess with the GHI fix and the epsilon trick, following `plans/dfpn/plan2.md`. The solver is time-boxed to 5 seconds and reports `Win`/`Loss`/`Draw` plus a principal variation for decisive results.

## Files Changed

| File | What changed |
|------|--------------|
| `src/zobrist.rs` | Added `rule50` keys to the Zobrist hash. Added `path_random(move, depth)` using a compact XOR table of move keys and depth keys for the GHI path code. |
| `src/position.rs` | Updated `hash` to include `rule50`. Changed `Outcome::Draw` to map to `(INF, 0)` (same as `Loss` from `pn`/`dn` alone). Added `pn_dn_for(is_or_node)`. Added a fast `outcome` draw path when only the two commoners remain on the board. |
| `src/search/tt.rs` | Extended `TtEntry` to store `outcome`, `pn`, `dn`, `path_code`, `repetition_seen`, and `valid`. `store` caps `pn`/`dn` at `INF` and resets `(INF, INF)` unsolved entries to `(1, 1)`. |
| `src/search/dfpn.rs` | Replaced with the full DF-PN+ algorithm. Added `Search` with `tt`, `path` set, `path_stack`, `path_code`, epsilon handling, and move ordering. |
| `tests/test_plan2.rs` | New test file covering the 10 positions listed in `plan2.md`. |
| `src/main.rs` / `src/search/ordering.rs` / `tests/test_inf.rs` | Only `cargo fmt` formatting changes. |

## Algorithm Details

- **OR/AND nodes**: `is_or_node` is `true` for the side trying to prove a win; `pn`/`dn` are always expressed in the attacker's perspective.
- **Children bounds**:
  - OR: `pn = min child pn`, `dn = sum child dn` (saturating, capped at `INF`).
  - AND: `pn = sum child pn`, `dn = min child dn` (saturating, capped at `INF`).
- **Epsilon trick**: thresholds are `ceil(x * 1.25)`. `epsilon` is stored as `0.25`.
- **GHI fix**: a `path: HashSet<u64>` detects local cycles. A `path_code` (XOR of `zobrist::path_random(move, depth)`) is stored in the TT. A stored `outcome` with `repetition_seen = true` is only reused when the stored `path_code` matches the current path code; otherwise the node is re-searched.
- **INF threshold arithmetic**: for the sum-based thresholds (`nd` in OR, `np` in AND) the value is forced to `INF` when the parent threshold is `INF`, avoiding the `INF - INF` collapse that would otherwise produce a useless finite threshold.
- **Outcome source of truth**: `Outcome` is stored explicitly in the TT; `pn`/`dn` are lower bounds. `Loss` and `Draw` collapse to the same `(INF, 0)` pair, so `outcome` is used to distinguish them.

## Bugs Found and Fixed

1. **`is_solved_by_children` returned `Loss` too eagerly**.
   The original `AND` branch checked for a `Win` child before a `Loss` child, which could declare a `Loss` for the parent even when a winning `Loss` child existed elsewhere. More importantly, the parent returned `Loss` as soon as any child was `Win` regardless of unresolved siblings. The fix is to only return `Loss`/`Draw` when *all* children are resolved; `Win` is returned as soon as any `Loss` child is found.

2. **INF threshold collapse**.
   With `th_dn = INF` and parent `dn = INF`, the formula `th_dn - dn + child_dn` could reduce to `child_dn` (often 0 or small), causing the child to return immediately and creating deadlock. This is avoided by treating an `INF` threshold as unbounded in those computations.

3. **Premature draw by `moves.is_empty()`**.
   `outcome()` now handles insufficient material (only two commoners), which also turns `7k`/`4k3` king-only positions into instant draws instead of timing out.

## Test Results

```bash
cargo fmt --check
cargo clippy --all-targets
cargo test
cargo doc
```

All pass. Selected outputs:

```
running 10 tests
test mate_in_1_black_to_move ... ok
test mate_in_1_white_to_move ... ok
test mate_in_2_black_to_move ... ok
test mate_in_2_white_to_move ... ok
test mate_in_3_black_to_move ... ok
test mate_in_4_white_to_move ... ok
test only_two_kings_draw_black_to_move ... ok
test only_two_kings_draw_white_to_move ... ok
test win_with_exploded_black_king_black_to_move ... ok
test win_with_exploded_black_king_white_to_move ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
```

`tests/test_inf.rs` also passes.

## Manual Verification Examples

```bash
# Mate in 4
cargo run -- --fen "rnbqkbnr/ppppp1pp/5p2/8/8/4P3/PPPP1PPP/RNBQKBNR w KQkq - 0 2"
# -> outcome: win
# -> pv: d1h5 g7g6 h5d5 c7c5 d5d7

# Mate in 1
cargo run -- --fen "rnbqkbnr/ppp1p2p/3p1pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 4"
# -> outcome: win
# -> pv: d5f7 e8d7 f7e7

# Only kings
cargo run -- --fen "7k/8/8/8/8/8/8/7K w - - 0 1"
# -> outcome: draw
```

## Known Limitations / Future Work

- Parallelization is out of scope for this iteration; the `Search` struct is sequential.
- Full GHI twin/base table entries and Kawano simulation were not implemented; the current `path` set + `path_code` trust check is the first-layer fix described in the plan.
- `outcome_from_pn_dn` is retained but cannot distinguish `Loss` from `Draw` because both map to `(INF, 0)`; it is not used internally.
- `solve_rook_mate_black_to_move_draw` still relies on the 5-second cutoff to return `Draw`.
