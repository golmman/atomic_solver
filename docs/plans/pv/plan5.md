# Plan 5: Add an independent `verify_ppv` example

## Summary

Add a new example binary `examples/verify_ppv.rs` that takes a FEN and a
space-separated UCI move list and checks whether the list is a Proof Principal
Variation (PPV) for the position. The example is independent of the main CLI and
uses the existing `Search` API.

The verifier replays the moves (legality-checked), checks that the final
position is decisive, and then walks backwards from the terminal position. At
each defender node (AND node) it runs a bounded DF-PN search on *every* legal
reply, including the chosen one, and requires the attacker to force a win
within the remaining number of plies of the supplied line. Attacker nodes are
skipped because a non-winning attacker move would be exposed by a refutation at
the following defender node. The example prints timings, progress, and clear
error messages, ending with `is_ppv: true` or `is_ppv: false`.

## Challenges to the proposed pseudocode

1. **Off-by-one in the child budget.**  
   The pseudocode passes `bound = 1 + proven_depth[i+1]` to the bounded search
   for a defender reply. The defender's reply already consumes one ply, so the
   attacker only has `proven_depth[i+1]` plies left to force a win from the
   resulting child. Passing `1 + proven_depth[i+1]` would allow a reply that is
   one ply longer than the supplied line to slip through, producing false
   positives. In the verifier this is `next_remaining = n - i - 1`.  
   **Fix:** use `child_budget = n - i - 1` (the remaining PPV length after the
   move).

2. **The chosen defender reply must also be verified.**  
   The original pseudocode checks defender alternatives (`m != moves[i]`) but
   not the chosen reply. That is not sound: the chosen reply could be a
   refutation while the final position is still a terminal win because later
   defender replies are non-refutations. The verifier must search *every* legal
   reply at a defender node (including the chosen one). Attacker nodes can still
   be skipped: a non-winning attacker move gives the defender a refutation, and
   that refutation is either the chosen reply or an alternative, so it is caught
   at the next defender node.

3. **Repetition path context.**  
   `Search::search_depth` starts each search with an empty repetition path. An
   alternative that returns to a position from the PPV prefix is a draw in the
   real game, but an independent search from the child will not see the prefix
   and may incorrectly claim a win. To be fully correct the bounded search must
   be seeded with the PPV prefix.  
   **Fix:** add one small public helper on `Search` that runs a bounded search
   from a position with a pre-populated repetition path and path code.

The plan below uses the helper. It is the only change outside of
`examples/`/`tests/` and is justified by the correctness requirement.

## Goal

- Provide `cargo run --example verify_ppv -- --fen <FEN> --moves <MOVES> --timeout <SEC>`.
- Correctly reject illegal moves, non-decisive finals, and defender replies
  that are not losing within the remaining PPV length (including the chosen
  defender reply).
- Correctly accept the two verified PPVs from the task description.
- Keep all new application code in `examples/` and `tests/` except for the one
  `Search` helper needed for repetition-correct bounded search.

## Non-goals

- SPPV verification or shortest-PV refinement.
- Changes to the main CLI (`src/main.rs`).
- New solver heuristics or TT layout changes.
- Parallel search.

## Background

### PPV as a bounded certificate

A supplied move list of length `n` is a PPV if the final position is terminal
with the claimed outcome and, for every defender node in the list, *every*
legal reply (including the chosen one) leads to a position from which the
attacker can force the root outcome within the remaining plies. Attacker nodes
do not need to be searched explicitly: if the next defender node is fully
closed, the attacker's move into it is automatically a winning move; if the
attacker move were non-winning, the defender would have a reply that is not a
forced loss, and that reply would be caught at the next defender node.

This is a *bounded* win/loss verification: each defender reply is a separate
bounded search whose maximum depth is the remaining PPV length. Unbounded
`solve_outcome` on every alternative would be far too expensive for deep
positions, which is why the user emphasizes bounded search.

### Correct repetition handling

The existing `Search::search_depth` begins with an empty `path_stack`. For an
independent verifier this is insufficient: a move that reaches a position already
seen in the PPV prefix is a draw by repetition, but the child search has no way
to know. Seeding `path_stack` with the prefix repetition keys and `path_code`
with the corresponding XOR of `zobrist::path_random(...)` makes the child search
see the same history as the verifier and therefore detect the same draws.

## Design

### 1. `Search` helper for prefix-seeded bounded search

Add to `src/search/dfpn/mod.rs`:

```rust
pub fn search_depth_with_prefix(
    &mut self,
    pos: &mut Position,
    max_depth: u32,
    prefix_keys: &[u64],
    prefix_path_code: u64,
) -> (Outcome, u64) {
    self.begin_run();
    self.proof_mode = ProofMode::Outcome;
    self.path_stack = prefix_keys.to_vec();
    self.path_code = prefix_path_code;
    let outcome = self.dfpn(pos, INF, INF, max_depth, u64::MAX, true);
    (outcome, self.nodes)
}
```

- `prefix_keys` is the repetition-key path *to* the child, i.e. the positions
  before the child is pushed by `dfpn`.
- `prefix_path_code` is the XOR of `zobrist::path_random(mv, depth)` for the
  prefix moves, using 1-indexed depths as `dfpn` does.
- The helper always runs an OR-node win search (`is_or_node = true`), which is
  exactly what the verifier needs: every child it is called on is reached by a
  defender reply, so the side to move is the attacker trying to force a `Win`.
- The method does not extract a PV; the verifier only needs the outcome and the
  node count.
- `begin_run()` resets timing, nodes, and the path; the method immediately
  restores the supplied prefix. The example sets `self.timeout` to the current
  remaining whole seconds before each call, which gives a near-global deadline.

### 2. UCI parsing helper

Extend `examples/common.rs` with a UCI parser that compares against the legal
move list using `Move::to_uci()`:

```rust
pub fn parse_uci(pos: &Position, uci: &str) -> Option<Move> {
    use atomic_solver::notation::move_to_uci;
    let mut moves = MoveList::new();
    pos.legal_moves(&mut moves);
    for i in 0..moves.len() {
        let m = moves[i];
        if move_to_uci(m) == uci {
            return Some(m);
        }
    }
    None
}
```

This automatically handles promotion, castling, and en-passant UCI strings as
long as `Move::to_uci()` does.

### 3. Example algorithm

`examples/verify_ppv.rs`:

```rust
mod common;
use common::parse_uci;
```

1. Parse CLI: `--fen` (default startpos), `--moves` (required),
   `--timeout` (default 60), `--help`. Unknown options exit with an error.
   Set `global_deadline = Instant::now() + Duration::from_secs(timeout)`.
2. Replay the moves:
   - `positions[0]` = root.
   - For each UCI token, parse it with `parse_uci(&positions[i], token)`. If
     illegal, print error and `is_ppv: false`.
   - If a position is terminal before all moves are consumed, error.
   - `positions[i+1]` = clone + `do_move(parsed)`.
3. Final check:
   - `positions[n]` must be terminal. Its `Outcome` is from the side-to-move
     perspective. Derive `root_outcome` from it using parity:
     `root_outcome = if n % 2 == 0 { final_outcome } else { final_outcome.flip() }`.
   - If `root_outcome` is `Draw`, error (`PPV` only applies to decisive
     outcomes).
   - Print input statistics to stdout:
     `println!("moves: {}", n);` and `println!("outcome: {}", outcome_str(root_outcome));`.
4. Set `attacker_color`:
   - If root outcome is `Win`, attacker = root side to move.
   - If `Loss`, attacker = opponent.
5. Backward pass, `i` from `n-1` down to `0`:
   - `next_remaining = n - i - 1` (plies left for the attacker after this defender reply).
   - If `positions[i]` is attacker to move: skip it. Its correctness is handled
     by the next defender node (or the final terminal check if it is the last
     move).
   - If `positions[i]` is defender to move:
     - Generate all legal moves.
     - For every legal move `m` (including `moves[i]`):
       - `child = positions[i].clone(); child.do_move(m);`
       - `prefix_keys = positions[0..=i].iter().map(|p| p.repetition_key()).collect::<Vec<_>>();`
       - `prefix_path_code = path_codes[i] ^ zobrist::path_random(m, i + 1);`
       - Before the search, compute `wall_remaining = global_deadline.saturating_duration_since(Instant::now())`. If it is zero, fail with a timeout error; otherwise set `search.set_timeout(wall_remaining.as_secs().max(1))`.
       - Run `let (outcome, nodes) = search.search_depth_with_prefix(&mut child, next_remaining, &prefix_keys, prefix_path_code);`.
       - Accumulate `nodes` into a running total.
       - If `outcome` is not `Outcome::Win`, log the ply, the supplied defender
         move `moves[i]`, the failing reply `m` (if different), `next_remaining`,
         and `outcome`, then print `is_ppv: false` and exit.
     - Progress log: `eprintln!("verifying defender ply {}/{} ({} replies)", i+1, n, legal_count);`.
6. If the loop completes, print elapsed time, total nodes, and `is_ppv: true`.

`path_codes[0] = 0` and `path_codes[k] = path_codes[k-1] ^ zobrist::path_random(moves[k-1], k)`.

### 4. Output format

- Statistics (stdout):
  ```rust
  println!("moves: {}", n);
  println!("outcome: {}", outcome_str(root_outcome));
  ```
- Progress: `eprintln!("checking defender ply {}/{} ({} replies)", i+1, n, legal_count);`
- On failure:
  `eprintln!("PPV refuted at defender ply {}/{}, supplied move '{}': reply '{}' not proven lost within {} plies (outcome: {:?})", i+1, n, moves[i], uci, next_remaining, outcome);`
- Error examples:
  - `error: move 'a1a2' at ply 1 is not legal`
  - `error: final position is not decisive (outcome: draw)`
  - `error: PPV refuted at defender ply 2/11, supplied move 'g8h7': reply 'g8f7' not proven lost within 9 plies (outcome: draw)`
- Final: `println!("is_ppv: true")` / `println!("is_ppv: false")`
- Timing: `eprintln!("elapsed: {:.3}s, nodes: {}", elapsed.as_secs_f64(), total_nodes);`

### 5. Exit codes

- `0` for a verified PPV (so scripts can use `&&`).
- `1` for illegal input, non-decisive final, failed move verification, or timeout.

## File changes

### `src/search/dfpn/mod.rs`

- Add `pub fn search_depth_with_prefix(...)` as documented above.
- No other changes; existing `search_depth` remains unchanged.

### `examples/common.rs`

- Add `pub fn parse_uci(pos: &Position, uci: &str) -> Option<Move>`.

### `examples/verify_ppv.rs`

- New example binary implementing the verifier.

### `tests/verify_ppv.rs`

- New integration tests that invoke the example binary and assert on stdout/stderr.

## Testing and verification

### Unit / integration checks

```bash
cargo fmt
cargo clippy --all-targets
cargo test
cargo test --release
cargo doc
```

### Manual CLI regression checks

```bash
# Invalid move
cargo run --release --example verify_ppv -- \
    --timeout 60 \
    --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26" \
    --moves "a1a2"
# expected: is_ppv: false, error about illegal move

# Non-decisive final
cargo run --release --example verify_ppv -- \
    --timeout 60 \
    --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26" \
    --moves "b1b8 g8h7"
# expected: is_ppv: false, error about non-decisive final

# Non-PPV line (g8h7 is not a strong reply - g8f7 cannot be refuted within the remaining plies)
cargo run --release --example verify_ppv -- \
    --timeout 60 \
    --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26" \
    --moves "b1b8 g8h7 b8h8 h7g7 h8h7 g7g8 h7g7 g8h8 g7g8 h8h7 g8g6"
# expected: is_ppv: false, error with supplied move and failing reply

# Verified PPV 1
cargo run --release --example verify_ppv -- \
    --timeout 60 \
    --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26" \
    --moves "b1b8 g8f7 c3e2 c5c4 e2f4 c4c3 f4g6"
# expected: is_ppv: true

# Verified PPV 2 (longer but still valid)
cargo run --release --example verify_ppv -- \
    --timeout 60 \
    --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26" \
    --moves "b1b8 g8f7 c3e2 c5c4 c2c3 f7e6 e2f4 e6f5 f4g6"
# expected: is_ppv: true

# Summary CLI example from the prompt
cargo run --release --example verify_ppv -- \
    --timeout 60 \
    --fen "4r1k1/3p4/2pB2p1/6Pp/p4p1P/2N1PP2/P1PP4/1R2R2K w - - 0 24" \
    --moves "e3f4 e8e1 b1b4 c6c5 b4b8 g8f7 a2a3 c5c4 b8g8 f7e6 g8g7 e6f5 g7g6"
# expected: is_ppv: false (a defender reply cannot be refuted within the remaining PPV length)
```

### Integration test cases (`tests/verify_ppv.rs`)

- `a1a2` illegal => `is_ppv: false` + illegal move error.
- `b1b8 g8h7` final not decisive => `is_ppv: false`.
- The `g8h7` 11-ply line => `is_ppv: false`.
- The two `g8f7` verified PPVs => `is_ppv: true`.
- A simple mate-in-1 PPV, e.g. `4k3/8/8/8/8/8/8/4R1K1 w - - 0 1` with
  `e1e8` => `is_ppv: true`.
- A move that is legal but does not lead to a decisive outcome => `is_ppv: false`.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Child budget off-by-one produces false positives. | Use `next_remaining = n - i - 1`, not `n - i`. |
| Repetition draws involving the PPV prefix are missed. | Seed `path_stack`/`path_code` via `search_depth_with_prefix`; this is the only src change. |
| `search_depth_with_prefix` adds public API surface. | It is a small, pure addition; existing methods are unchanged. |
| `zobrist::path_random` depth indexing is subtle. | Derive prefix path codes using 1-indexed move numbers, matching `dfpn`. Add a unit test in `zobrist` or the example if needed. |
| Verifying many alternatives on deep positions is slow. | Each search is bounded by the remaining PPV length and by the global timeout; the TT is reused across alternative searches. |
| UCI parsing for castling/en-passant is tricky. | Match against `Move::to_uci()` from the legal move list rather than reconstructing `Move` flags manually. |
| `cargo test` invoking `cargo run` is slow or racy. | Use `cargo run --release` and `--quiet` from the test; Cargo's file locking serialises builds. Alternatively build the example once in CI and point tests at the built binary. |

## Success criteria

- `cargo test`, `cargo fmt --check`, and `cargo clippy --all-targets` pass.
- The example prints `is_ppv: true` for both `g8f7` PPVs and `is_ppv: false`
  for the `a1a2`, `b1b8 g8h7`, `g8h7` 11-ply, and `e3f4` summary cases.
- `cargo doc` builds with no new warnings.
- No changes to `src/main.rs` except the single `Search` helper in
  `src/search/dfpn/mod.rs`.
