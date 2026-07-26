# Report: `verify_ppv` Example

## Summary

Implemented `docs/plans/pv/plan5.md` in `atomic_solver`.

A new example binary, `examples/verify_ppv.rs`, takes a FEN and a
space-separated UCI move list and checks whether the list is a Proof Principal
Variation (PPV). The verifier replays the line, confirms that the final position
is decisive, and then checks every defender node backwards: every legal reply
(including the chosen one) must be a forced loss for the defender within the
remaining PPV length. It also requires the chosen defender reply to be one of
the longest defenses, so a non-resistant reply cannot be accepted.

## Changes

### 1. `Search` prefix-path helper (`src/search/dfpn/mod.rs`)

- Added a `prefix_path: Option<(Vec<u64>, u64)>` field to `Search` to carry a
  pre-populated repetition path for bounded searches run in the context of a
  longer line.
- `begin_run()` and `reset_search_state()` seed `path_stack`/`path_code` from
  `prefix_path` when it is present, so the child search sees the same history
  as the verifier and detects the same repetition draws.
- Added `pub fn search_depth_with_prefix(...)`:

```rust
pub fn search_depth_with_prefix(
    &mut self,
    pos: &mut Position,
    max_depth: u32,
    prefix_keys: &[u64],
    prefix_path_code: u64,
) -> (Outcome, u32, u64)
```

The helper saves the current `Search` state, installs the prefix path, enables
shortest-PV refinement, and runs the staged solver (`solve`). It returns `Win`
only when the outcome is decisive and the returned PV length is within
`max_depth`. The returned depth is the shortest proven win from the child
position.

### 2. UCI parsing helper (`examples/common.rs`)

- Added `pub fn parse_uci(pos: &Position, uci: &str) -> Option<Move>`.
- The helper matches the supplied UCI string against `Move::to_uci()` on the
  legal move list, so promotion, castling, and en-passant strings are handled
  correctly without manual flag reconstruction.

### 3. `examples/verify_ppv.rs`

- Parses `--fen` (default start position), `--moves` (one or more UCI tokens),
  `--timeout` (default 60), and `--help`. Unknown or malformed options exit
  with code `1` and print `is_ppv: false`.
- Replays the supplied moves with legality checking and stops early if a
  position is terminal before the list is consumed.
- Verifies the final position is decisive and derives the root outcome from it
  using parity.
- Walks backwards through the line. At each defender node it generates all
  legal replies, runs a bounded search for each with the PPV prefix, and
  rejects the line if any reply is not a forced win within the remaining plies.
- In addition to the per-reply outcome check, it records the proven depth of
  each reply and requires the chosen defender reply to have the same depth as
  the longest reply. This catches non-resistant defender moves that the
  remaining line happens to beat.
- Prints `is_ppv: true` on success (exit code `0`) or `is_ppv: false` and a
  clear error on failure (exit code `1`).

### 4. Integration tests (`tests/verify_ppv.rs`)

- Added tests that invoke the example via `cargo run --release --quiet` with a
  global mutex to serialize the subprocess builds.
- Covers: illegal move, legal non-decisive first move, non-decisive final,
  refuted long line, the two `g8f7` verified PPVs, and a mate-in-one.

## Files changed

| File | What changed |
|------|--------------|
| `src/search/dfpn/mod.rs` | Added `prefix_path` field, `search_depth_with_prefix`, and prefix seeding in `begin_run`/`reset_search_state`. |
| `examples/common.rs` | Added `parse_uci`. |
| `examples/verify_ppv.rs` | New example binary implementing the PPV verifier. |
| `tests/verify_ppv.rs` | New integration tests for the example. |
| `docs/plans/pv/report5.md` | This report. |

## Verification

```bash
cargo fmt --check
cargo clippy --all-targets
cargo test --release
```

All passed with no new warnings.

The release test suite included the new `tests/verify_ppv.rs`:

```text
running 7 tests
test illegal_move_is_not_ppv ... ok
test legal_non_decisive_first_move_is_not_ppv ... ok
test mate_in_one_is_ppv ... ok
test non_decisive_final_is_not_ppv ... ok
test refuted_long_line_is_not_ppv ... ok
test verified_ppv_one ... ok
test verified_ppv_two ... ok

test result: ok. 7 passed; 0 failed; 0 ignored
```

## Manual verification

Verified PPV 1:

```bash
cargo run --release --quiet --example verify_ppv -- \
    --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26" \
    --moves "b1b8 g8f7 c3e2 c5c4 e2f4 c4c3 f4g6" \
    --timeout 60
```

Output:

```text
moves: 7
outcome: win
checking defender ply 6/7 (2 replies)
checking defender ply 4/7 (3 replies)
checking defender ply 2/7 (3 replies)
is_ppv: true
elapsed: 0.076s, nodes: 27769
```

Verified PPV 2:

```bash
cargo run --release --quiet --example verify_ppv -- \
    --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26" \
    --moves "b1b8 g8f7 c3e2 c5c4 c2c3 f7e6 e2f4 e6f5 f4g6" \
    --timeout 60
```

Output:

```text
moves: 9
outcome: win
checking defender ply 8/9 (2 replies)
checking defender ply 6/9 (2 replies)
checking defender ply 4/9 (3 replies)
checking defender ply 2/9 (3 replies)
is_ppv: true
elapsed: 0.067s, nodes: 29327
```

Refuted non-PPV line:

```bash
cargo run --release --quiet --example verify_ppv -- \
    --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26" \
    --moves "b1b8 g8h7 b8h8 h7g7 h8h7 g7g8 h7g7 g8h8 g7g8 h8h7 g8g6" \
    --timeout 60
```

Output:

```text
moves: 11
outcome: win
checking defender ply 10/11 (1 replies)
checking defender ply 8/11 (1 replies)
checking defender ply 6/11 (1 replies)
checking defender ply 4/11 (1 replies)
checking defender ply 2/11 (3 replies)
is_ppv: false
PPV refuted at defender ply 2/11, supplied move 'g8h7' is not a longest defense (depth 3, longest 5)
```

## Notes

- The implementation differs slightly from the plan's pseudocode: instead of a
  direct bounded `dfpn` call, `search_depth_with_prefix` uses the existing staged
  `solve` with shortest-PV refinement enabled. This reuses the existing
  bootstrapping and refinement logic while still respecting the depth limit.
- The verifier additionally checks that the chosen defender reply is a longest
  defense. This prevents accepting a line where the supplied defender move is
  refutable more quickly than the PPV length allows.
- Repetition handling is correct because the child search is seeded with the
  full prefix path; moves that return to a position already seen in the PPV
  prefix are therefore recognized as draws.
