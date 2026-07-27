# Plan 6: Proven-subtree PPV extraction in `find_ppv`

## Summary

Rewrite `Search::find_ppv` so that, once `solve_outcome` has proven a decisive
root outcome, it extracts a Proof Principal Variation (PPV) from the proven
subtree itself rather than from the live transposition table's `best_move`
chain. The live TT is reliable for *outcomes* but not for *which* child
represents the strongest defense, because `Outcome`-mode DF-PN stops at the
first winning child and never revisits siblings to look for a longer defense.

The new pass is a bounded recursive walk of the proof-relevant subtree. At each
node it evaluates all legal children, selects the child that matches the PPV
minimax rule (shortest attacker win, longest defender resistance), and recurses.
It does not trust pre-existing `best_move` / `best_child` hints. If the pass
times out or fails, `find_ppv` falls back to the current `dfpn`-then-extract
behaviour.

## Challenges to the proposed approach

1. **OR vs. AND node terminology.** The prompt says "OR-node (defender to move)".
   In the project's definitions the attacker moves at OR nodes (any winning move
   suffices) and the defender moves at AND nodes (all replies must be losing; the
   strongest reply is the one that delays the loss longest). The implementation
   must follow the AND-node semantics for defender selection, not the literal
   `is_or_node` parity if that would pick the wrong child.

2. **Node-type-aware selection.** `Outcome`-mode search only needs *one* winning
   child. A PPV extraction must apply different rules:
   - Attacker node (side to move can win): pick a child that is a proven loss
     for the opponent. For a tight PPV, prefer the shortest such child.
   - Defender node (side to move is losing): *every* legal reply must be a
     proven win for the opponent. The chosen reply is the one with the longest
     resistance (largest `1 + child_depth`).
   The existing `is_solved_by_children` already encodes exactly this rule, but
   `find_ppv` currently extracts from stored `best_move` entries instead of
   running `is_solved_by_children` on freshly evaluated children.

3. **Do not trust the live TT `best_move`.** The whole point of the pass is that
   `solve_outcome` may have stored a suboptimal `best_move` and `best_child`
   (e.g. `g8h7` instead of `g8f7`). The recursive pass must ignore `best_move`
   hints and derive the selection from child outcomes/depths. It may still
   consult the TT for cached *outcomes* to avoid redundant work, but it must not
   let a stale `best_move` short-circuit the search.

4. **Depth bounds must match the proven certificate.** The pass starts with
   `bootstrap_success_depth` (or the root TT entry's `depth`) and decrements by
   one each ply. A non-terminal leaf at depth `0` is a failure: the line is not
   proven within the claimed bound. This is the same bounded-win semantics the
   `verify_ppv` example uses.

5. **Path-dependent repetition.** Positions can repeat depending on the path,
   especially in atomic chess. The extraction must maintain `path_stack` and
   `path_code` the same way `dfpn` does, and treat a repeated position as a
   draw (which cannot be part of a decisive PPV). It must also handle
   `zobrist::path_random` 1-indexed depth arithmetic consistently with
   `extract_pv_internal`.

6. **Attacker-node choice and SPPV overlap.** Picking the *shortest* winning
   child at attacker nodes makes the extracted line look like an SPPV. That is
   acceptable — a PPV is still a PPV — but the implementation should be careful
   not to confuse the `refine_sppv` contract. The fallback behaviour and the
   public API remain unchanged.

## Goal

- `find_ppv` returns a PPV whose defender replies are strongest defenses.
- The reported `6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26` position
  produces a 7-plies PPV starting with `b1b8 g8f7 ...` even with
  `--no-refine-shortest`.
- `m27_ppv_only` and `m27_streaming_output` pass.
- Existing `m24_ppv`, `m27_shortest_pv`, and `m27_kh7_fast_win` tests continue
  to pass.

## Non-goals

- Do not modify `solve_outcome` or the core `dfpn` loop.
- Do not change the transposition-table entry layout or replacement policy.
- Do not change `StaticAtomicScorer` or history/killer heuristics.
- Do not change the public `Search` API beyond possibly adding one private or
  `pub(super)` helper.
- Do not remove `refine_sppv`; it remains available for strict shortest-PV
  refinement.

## Background

### Why the live TT loses the strongest defense

`dfpn` in `Outcome` mode calls `select_child_with_early_exit` as soon as a node
is solved. At an OR node (attacker to move) it returns the *first* `Loss` child
found, not necessarily the one that gives the shortest overall PPV. At an AND
node (defender to move) it does wait for `all_solved` and `is_solved_by_children`
picks the longest `Win` child — *but* `evaluate_all_children` stops evaluating
as soon as it has enough children to satisfy the early-exit test, and
`select_from_children` reuses the previous `best_move` / `best_child` from the
TT. The TT entry that survives may therefore contain a `best_move` that is a
valid winning continuation but not the defender's most resistant reply.

When `find_ppv` later runs `dfpn` in `ProofMode::Ppv` and then extracts by
following `best_move` chains, it can follow a chain of suboptimal defender
replies and emit a non-PPV (e.g. the 11-plies `b1b8 g8h7 ...` line).

### What a second pass needs

The second pass is a *reconstruction* of the proof tree. It knows the root is
proven within `D` plies. It walks from the root and, at every defender node,
verifies that every legal reply is a forced loss within the remaining budget and
chooses the reply that forces the attacker to take the longest. At attacker
nodes it chooses any winning reply (preferably the shortest for a tight PPV).
Because it only follows already-proven positions and is bounded by `D`, it is
much smaller than the original `dfpn` search and does not depend on the bulk of
the TT entries being optimal.

## Design

### 1. `Search::extract_ppv_from_proven_subtree`

Add a recursive helper in `src/search/dfpn/pv.rs` inside `impl Search`:

```rust
fn extract_ppv_from_proven_subtree(
    &mut self,
    pos: &mut Position,
    expected: Outcome,
    remaining: u32,
    is_attacker_node: bool,
) -> Option<(Vec<Move>, u32)> {
    // Returns (pv_from_this_node, proven_depth_from_this_node) or None.
}
```

The function:

1. Checks `self.time_exceeded()` and returns `None` if the deadline is gone.
2. Checks `pos.outcome()`. If terminal and equal to `expected`, returns
   `(Vec::new(), 0)`. If terminal with a different outcome or a draw, returns
   `None`.
3. Checks whether `pos.repetition_key()` is already on the `path_stack`. If so,
   it is a repetition draw; for a decisive PPV this is a failure.
4. If `remaining == 0`, returns `None` (unproven within the remaining budget).
5. Generates all legal moves.
6. For each move:
   - Pushes the repetition key onto `path_stack` and updates `path_code` with
     `zobrist::path_random(mv, path_stack.len())`.
   - Calls `self.extract_ppv_from_proven_subtree(
       pos,
       expected.flip(),
       remaining.saturating_sub(1),
       !is_attacker_node,
     )`.
   - Pops the path and undoes the move.
   - Collects successful results as `(Move, u32)` pairs of `(child_move,
     child_depth)`.
7. Selects the best child:
   - `is_attacker_node == true` (expected `Win` for side to move): pick the
     child that is a proven `Loss` for the opponent with the **smallest**
     `1 + child_depth` (shortest decisive attacker move). If no child is a
     proven loss, return `None`.
   - `is_attacker_node == false` (expected `Loss` for side to move): require
     **every** legal child to be a proven `Win` for the opponent. Pick the one
     with the **largest** `1 + child_depth` (strongest defense). If any child is
     not a proven win, return `None`.
8. Builds the PV by prepending the chosen move to the recursive result and
   returns it with depth `1 + child_depth`.

The `is_attacker_node` flag is `true` at the root when the root outcome is `Win`
(and flips each ply). When the root outcome is `Loss` the root node is a
`Loss` for the side to move, so the role is inverted: the side to move is the
defender and the opponent is the attacker. The helper can determine this from
`expected`: `expected == Outcome::Win` means the current side to move is the
attacker; `expected == Outcome::Loss` means the current side to move is the
defender.

### 2. Integration with `find_ppv`

In `src/search/dfpn/mod.rs`, rewrite `find_ppv` roughly as:

```rust
pub fn find_ppv(&mut self, pos: &mut Position, outcome: Outcome) -> Option<Vec<Move>> {
    if self.time_exceeded() {
        return None;
    }

    if let Some(depth) = self.bootstrap_success_depth {
        self.reset_search_state();
        if !self.time_exceeded() {
            if let Some((pv, _proven_depth)) =
                self.extract_ppv_from_proven_subtree(pos, outcome, depth, true)
            {
                self.last_pv = pv.clone();
                return Some(pv);
            }
        }
        if self.time_exceeded() {
            return None;
        }
    }

    // Fallback: the old behaviour.
    let pv = self
        .extract_ppv(pos, outcome)
        .unwrap_or_else(|| self.extract_pv(pos));
    if pv.is_empty() {
        return None;
    }
    self.last_pv = pv.clone();
    Some(self.last_pv.clone())
}
```

- `extract_ppv_from_proven_subtree` is the first attempt.
- If it returns `None` (timeout, bound too small, or inconsistent TT outcome),
  `find_ppv` falls back to the current `dfpn`-then-extract path so the solver
  still returns *some* winning line.

### 3. Child outcome verification

For the first version, the recursive pass itself evaluates children recursively.
This is correct and simple, but it may be slow for very wide subtrees. As an
optional refinement, a child can be checked with a bounded `dfpn` call
(`max_depth = remaining - 1`, `max_work` large enough to prove it) before the
recursive extraction continues. If that is done, `dfpn` must be invoked with
`proof_mode = ProofMode::Ppv` and, crucially, the old `best_move` /
`best_child` hints must be ignored or the TT cleared for the duration of the
pass, otherwise the same suboptimal hints will poison the result. For the plan,
keep the pure recursive pass; note the `dfpn`-per-child variant as a future
optimization.

### 4. Depth source

`bootstrap_success_depth` is the upper bound from `solve_outcome`. If it is
`None`, fall back to the root TT entry's `depth` or `self.max_ply` as a last
resort. The recursive pass will only succeed if the root is actually proven
within the supplied bound, which is guaranteed when `solve_outcome` returns a
decisive outcome and stores a concrete depth.

### 5. Memoization (optional, later)

The proof-relevant subtree may contain transpositions. A future optimisation can
memoize `extract_ppv_from_proven_subtree` results by `(position_hash,
path_code, expected, remaining)` in a local `HashMap` or in the TT as twin
entries. The first version does not need memoization because the depth bound
keeps the recursion finite and the tree is usually small relative to the
original search.

## File changes

### `src/search/dfpn/pv.rs`

- Add `extract_ppv_from_proven_subtree` as a private `fn` inside `impl Search`.
- Add any small helpers needed for path stack management (reuse existing
  `path_push` / `path_pop` if possible).
- Keep `extract_pv`, `extract_ppv`, and `extract_pv_checked` unchanged; they
  become the fallback used by `find_ppv`.

### `src/search/dfpn/mod.rs`

- Modify `find_ppv` to call `extract_ppv_from_proven_subtree` first and fall
  back to the existing `dfpn` + `extract_ppv` / `extract_pv` path.
- No changes to `solve_outcome`, `refine_sppv`, or the core `dfpn` routine.

### `tests/test_plan6.rs`

- Remove or re-enable `m27_ppv_only` and `m27_streaming_output` (they are
  currently failing because `find_ppv` returns the 11-plies line). Update them
  to expect the 7-plies PPV if they are not already.
- Add a regression test that calls `Search::find_ppv` directly on
  `6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26` and asserts the
  returned PV starts with `b1b8` and `g8f7`.

### `examples/verify_ppv.rs` (no change, but used for verification)

- Use `verify_ppv` to confirm that `find_ppv` outputs are genuine PPVs for the
  reported FENs.

## Testing and verification

### Standard quality checks

```bash
cargo fmt
cargo clippy --all-targets
cargo test
cargo test --release
cargo doc --no-deps
```

### Regression checks

```bash
# Should now print a 7-plies PPV starting with b1b8 g8f7
cargo run --release -- --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26" --timeout 60 --no-refine-shortest

# Should print a PPV, not the 11-plies non-PPV
cargo run --release -- --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26" --timeout 60

# Should still be a valid PPV
cargo run --release -- --fen "4r1k1/3p4/2pB2p1/6Pp/p4p1P/2N1PP2/P1PP4/1R2R2K w - - 0 24" --timeout 60 --no-refine-shortest
```

### `verify_ppv` checks

```bash
# Should now be is_ppv: true
cargo run --release --example verify_ppv -- \
    --timeout 60 \
    --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26" \
    --moves "b1b8 g8f7 b8f8 f7g7 d6e5 g7h7 f8h8"

# Should still be is_ppv: true
cargo run --release --example verify_ppv -- \
    --timeout 60 \
    --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26" \
    --moves "b1b8 g8f7 c3e2 c5c4 e2f4 c4c3 f4g6"
```

### Test matrix

- `m27_white_wins`
- `m27_shortest_pv`
- `m27_kh7_fast_win`
- `m27_ppv_only` (re-enabled)
- `m27_streaming_output` (re-enabled)
- `m24_ppv` (run with `--ignored`)
- `m19_*`, `m20_*`, `m21_*`, `m22_*`, `m26_*` from `test_plan6.rs`

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Pure recursive expansion is too slow on deep/wide positions. | Bound the recursion by `bootstrap_success_depth`; check `time_exceeded()` at every node; fall back to the old `find_ppv` if the pass returns `None`. |
| Path-code / repetition-key arithmetic is off-by-one. | Use the same 1-indexed `zobrist::path_random(mv, depth)` convention as `dfpn` and `extract_pv_internal`; add unit tests for path-code round-trips. |
| The recursive pass returns an SPPV-like line, surprising `refine_sppv`. | `refine_sppv` binary search will simply find no shorter PPV and exit; the public output is still correct. Document that `find_ppv` may return a tight PPV. |
| `bootstrap_success_depth` is `None` or too small. | Fall back to the root TT entry `depth`, then `max_ply`; if none are available, fall back to old `find_ppv`. |
| A child is terminal with `Draw` but the expected outcome is `Win`. | The recursive pass returns `None` for that branch, causing the parent to fail the "all children are losing for the defender" check at a defender node; this is correct behaviour. |
| The TT says the root is a win but `extract_ppv_from_proven_subtree` cannot reconstruct it. | Fallback to `dfpn` + `extract_pv` ensures the solver still prints a winning line. |
| Attacker nodes choose the shortest win, which may not match an expected PPV from an older test. | Any winning attacker move is a valid PPV; tests should be updated to the actual tight line if they previously relied on a specific non-shortest move. |

## Success criteria

- `cargo test` and `cargo test --release` pass.
- `m27_ppv_only` and `m27_streaming_output` pass without `
#[ignore]`.
- The reported FEN `6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26` prints
  a 7-plies PPV starting with `b1b8 g8f7` in both refined and
  `--no-refine-shortest` modes.
- `cargo run --release --example verify_ppv` confirms that the newly printed
  PPVs for the regression FENs are valid PPVs.
- `cargo fmt --check`, `cargo clippy --all-targets`, and `cargo doc --no-deps`
  produce no new warnings.
- No changes to `src/main.rs`, `src/search/tt/`, or `src/search/ordering.rs`.

## Open ends for follow-up plans

- Optional memoization for `extract_ppv_from_proven_subtree` to handle
  transpositions in the proof tree.
- Optional `dfpn`-per-child evaluation for very wide subtrees.
- Whether `extract_pv_internal` should still try to follow depth-matching twins
  after `find_ppv` is rewritten.
