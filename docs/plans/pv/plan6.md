# Plan 6: Proven-subtree PPV extraction in `find_ppv`

HINT: This plan has been realized in branch `pv_plan6`.

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
It does not trust pre-existing `best_move` / `best_child` hints and, for this
first version, it does not consult the transposition table at all, guaranteeing
that stale TT data cannot poison the result. If the pass times out or fails,
`find_ppv` falls back to `extract_ppv` / `extract_pv`. Before returning,
`find_ppv` updates `bootstrap_success_depth` to the length of the PV it
extracted, so the follow-up `refine_sppv` stage starts from a tight bound.

## Challenges to the proposed approach

1. **Attacker/defender terminology.** The plan avoids separate OR/AND-node
   terminology. The correct rule is derived directly from `expected`:
   - `expected == Outcome::Win`: the side to move is the attacker and needs
     only one child that is a proven `Loss` for the opponent.
   - `expected == Outcome::Loss`: the side to move is the defender and every
     legal reply must be a proven `Win` for the opponent; the chosen reply is
     the one that delays the loss longest.
   The existing `is_solved_by_children` already encodes exactly this rule, but
   `find_ppv` currently extracts from stored `best_move` entries instead of
   running `is_solved_by_children` on freshly evaluated children.

2. **Do not trust the live TT `best_move`.** The whole point of the pass is that
   `solve_outcome` may have stored a suboptimal `best_move` and `best_child`
   (e.g. `g8h7` instead of `g8f7`). The recursive pass must ignore `best_move`
   hints and derive the selection from child outcomes/depths. For this first
   version it also avoids reading TT outcomes, so even path-dependent twin
   entries cannot influence the result.

3. **Depth bounds must match the proven certificate.** The pass starts with an
   upper bound (`bootstrap_success_depth`, tightened if necessary) and
   decrements by one each ply. A non-terminal leaf at depth `0` is a failure:
   the line is not proven within the claimed bound. This is the same
   bounded-win semantics the `verify_ppv` example uses.

4. **Path-dependent repetition.** Positions can repeat depending on the path,
   especially in atomic chess. The extraction must maintain `path_stack` and
   `path_code` the same way `dfpn` does, and treat a repeated position as a
   draw (which cannot be part of a decisive PPV). It must also handle
   `zobrist::path_random` 1-indexed depth arithmetic consistently with
   `extract_pv_internal`.

5. **Attacker-node choice and SPPV overlap.** Picking the *shortest* winning
   child at attacker nodes makes the extracted line look like an SPPV. That is
   acceptable — a PPV is still a PPV — but the implementation should be careful
   not to confuse the `refine_sppv` contract. The fallback behaviour and the
   public API remain unchanged.

6. **Tie-breaking.** If several children tie for the optimal depth, the chosen
   move depends on the iteration order. For the first version we use the
   natural legal-move generation order, which is deterministic.

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
nodes it chooses the shortest winning reply. Because it only follows already-
proven positions and is bounded by `D`, it is much smaller than the original
`dfpn` search and does not depend on the bulk of the TT entries being optimal.

## Design

### 1. `Search::extract_ppv_from_proven_subtree`

Add a recursive helper in `src/search/dfpn/pv.rs` inside `impl Search`:

```rust
fn extract_ppv_from_proven_subtree(
    &mut self,
    pos: &mut Position,
    expected: Outcome,
    remaining: u32,
) -> Option<(Vec<Move>, u32)> {
    // Returns (pv_from_this_node, proven_depth_from_this_node) or None.
}
```

The function:

1. Checks `self.time_exceeded()` and returns `None` if the deadline is gone.
2. Checks `pos.outcome()`. If terminal and equal to `expected`, returns
   `(Vec::new(), 0)`. If terminal with a different outcome or a draw, returns
   `None`.
3. If `remaining == 0`, returns `None` (unproven within the remaining budget).
4. Checks whether `pos.repetition_key()` is already on the `path_stack`. If so,
   it is a repetition draw; for a decisive PPV this is a failure.
5. Pushes the current repetition key onto `path_stack`.
6. Generates all legal moves.
7. For each move:
   - `pos.do_move(mv)`.
   - Updates `path_code` with
     `zobrist::path_random(mv, self.path_stack.len())`, using the same
     1-indexed convention as `dfpn` (the current stack length equals the ply
     count of the move being made, because the current position has already
     been pushed).
   - Calls `self.extract_ppv_from_proven_subtree(
       pos,
       expected.flip(),
       remaining.saturating_sub(1),
     )`.
   - Restores `path_code` by XORing the same `path_random` value again.
   - `pos.undo_move(mv)`.
   - Collects successful results as `(Move, u32)` pairs of `(child_move,
     child_depth)`.
8. Pops the current repetition key from `path_stack`.
9. Selects the best child based on `expected`:
   - `expected == Outcome::Win` (attacker to move): pick a child whose
     recursive call returned `Some` with `expected == Outcome::Loss` for the
     opponent, i.e. a proven `Loss` child. Prefer the **smallest**
     `1 + child_depth` (shortest decisive attacker move). If no child is a
     proven loss, return `None`. Ties are broken by first legal move in
     move-generation order.
   - `expected == Outcome::Loss` (defender to move): require **every** legal
     child to return `Some` with `expected == Outcome::Win` for the opponent.
     Pick the child with the **largest** `1 + child_depth` (strongest
     defense). If any child is not a proven win, return `None`. Ties are
     broken by first legal move in move-generation order.
10. Builds the PV by prepending the chosen move to the recursive result and
    returns it with depth `1 + child_depth`.

The helper is entirely self-contained: it pushes and pops its own repetition
key, updates `path_code`, and never reads the transposition table. This makes
correctness straightforward and avoids the stale-`best_move` problem entirely.

### 2. Integration with `find_ppv`

In `src/search/dfpn/mod.rs`, rewrite `find_ppv` roughly as:

```rust
pub fn find_ppv(&mut self, pos: &mut Position, outcome: Outcome) -> Option<Vec<Move>> {
    if self.time_exceeded() {
        return None;
    }

    // Tighten the depth bound if solve_outcome only stored a loose value.
    let mut bound = self.bootstrap_success_depth;
    if bound.is_none() || bound == Some(self.max_ply as u32) {
        self.reset_search_state();
        if let Some(pv) = self.extract_pv_checked(pos, outcome, None) {
            bound = Some(pv.len() as u32);
        }
    }
    if bound.is_none() {
        bound = Some(self.max_ply as u32);
    }

    self.reset_search_state();
    if !self.time_exceeded() {
        if let Some((pv, proven_depth)) =
            self.extract_ppv_from_proven_subtree(pos, outcome, bound)
        {
            self.last_pv = pv.clone();
            self.bootstrap_success_depth = Some(proven_depth);
            return Some(pv);
        }
    }

    if self.time_exceeded() {
        return None;
    }

    // Fallback: follow the TT best_move chain.
    let pv = self
        .extract_ppv(pos, outcome)
        .unwrap_or_else(|| self.extract_pv(pos));
    if pv.is_empty() {
        return None;
    }
    self.last_pv = pv.clone();
    self.bootstrap_success_depth = Some(pv.len() as u32);
    Some(pv)
}
```

`extract_ppv_from_proven_subtree` is the first attempt. If it returns `None`
(timeout, bound too small, or inconsistent subtree), `find_ppv` falls back to
the existing TT-chain extraction. In both success paths `bootstrap_success_depth`
is updated to the actual extracted PPV length so that `refine_sppv` starts from
a tight upper bound.

### 3. Child outcome verification

For the first version the recursive pass evaluates children recursively with no
TT memoisation. This is correct and simple, although it may be slow for very
wide subtrees because every legal child at a defender node (and every winning
child at an attacker node) must be expanded to determine the optimal depth. The
recursion is bounded by the proven depth and only follows positions that are
already decisive, so the tree is normally small relative to the original `dfpn`
search.

As a future optimisation, a child can be checked with a bounded `dfpn` call
(`max_depth = remaining - 1`, `max_work` large enough to prove it) before the
recursive extraction continues. If that is done, `dfpn` must be invoked with
`proof_mode = ProofMode::Ppv` and, crucially, the old `best_move` / `best_child`
hints must be ignored or the TT cleared for the duration of the pass, otherwise
the same suboptimal hints will poison the result. For this plan, keep the pure
recursive pass; note the `dfpn`-per-child variant as a future optimisation.

### 4. Depth source

`bootstrap_success_depth` is the upper bound from `solve_outcome`. If it is
`None` or the loose `max_ply` sentinel, `find_ppv` tries `extract_pv_checked`
to obtain a concrete length from the TT chain. If that also fails, it falls back
to `self.max_ply` as a last resort. The recursive pass will only succeed if the
root is actually proven within the supplied bound, which is guaranteed when
`solve_outcome` returns a decisive outcome and stores a concrete depth.

Because `extract_ppv_from_proven_subtree` minimises attacker wins, the PV it
extracts may be shorter than the bound. `find_ppv` records that shorter length
in `bootstrap_success_depth` before returning.

### 5. Memoization (optional, later)

The proof-relevant subtree may contain transpositions. A future optimisation can
memoize `extract_ppv_from_proven_subtree` results by `(position_hash,
path_code, expected, remaining)` in a local `HashMap`. The first version does
not use memoization or the TT because the depth bound keeps the recursion finite
and the tree is usually small relative to the original search.

## File changes

### `src/search/dfpn/pv.rs`

- Add `extract_ppv_from_proven_subtree` as a private `fn` inside `impl Search`.
- Add any small helpers needed for path stack management (reuse existing
  `path_push` / `path_pop`).
- Keep `extract_pv`, `extract_ppv`, and `extract_pv_checked` unchanged; they
  become the fallback used by `find_ppv`.

### `src/search/dfpn/mod.rs`

- Modify `find_ppv` to:
  1. Tighten `bootstrap_success_depth` with `extract_pv_checked` if it is
     missing or loose.
  2. Call `extract_ppv_from_proven_subtree` first.
  3. Fall back to `extract_ppv` / `extract_pv` if the recursive pass fails.
  4. Update `bootstrap_success_depth` to the extracted PPV length before
     returning.
- No changes to `solve_outcome`, `refine_sppv`, or the core `dfpn` routine.

### `tests/test_plan6.rs`

- Make `m27_ppv_only` and `m27_streaming_output` pass. They are active tests
  (not `#[ignore]`) and already expect the 7-plies PPV; no test expectations
  need to change.
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
- `m27_ppv_only`
- `m27_streaming_output`
- `m24_ppv` (run with `--ignored`)
- `m19_*`, `m20_*`, `m21_*`, `m22_*`, `m26_*` from `test_plan6.rs`

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Pure recursive expansion is too slow on deep/wide positions. | Bound the recursion by `bootstrap_success_depth`; check `time_exceeded()` at every node; fall back to `extract_ppv` / `extract_pv` if the pass returns `None`. |
| Path-code / repetition-key arithmetic is off-by-one. | Match the 1-indexed `zobrist::path_random(mv, depth)` convention used by `dfpn` and `extract_pv_internal`; add unit tests for path-code round-trips. |
| The recursive pass returns an SPPV-like line, surprising `refine_sppv`. | `refine_sppv` binary search will simply find no shorter PPV and exit; the public output is still correct. Document that `find_ppv` may return a tight PPV. |
| `bootstrap_success_depth` is `None` or too loose. | Tighten it with `extract_pv_checked` before the recursive pass; if that fails, fall back to `max_ply`. |
| A child is terminal with `Draw` but the expected outcome is `Win`. | The recursive pass returns `None` for that branch, causing the parent to fail the "all children are losing for the defender" check at a defender node; this is correct behaviour. |
| The TT says the root is a win but `extract_ppv_from_proven_subtree` cannot reconstruct it. | Fallback to `extract_ppv` / `extract_pv` ensures the solver still prints a winning line. |
| First-legal tie-breaking produces a different PPV than `refine_sppv` on positions with multiple same-depth optimal moves. | The PPV is still valid; `refine_sppv` will find and emit the shortest if refinement is enabled. If tests break, revisit tie-breaking in a follow-up plan. |

## Success criteria

- `cargo test` and `cargo test --release` pass.
- `m27_ppv_only` and `m27_streaming_output` pass.
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
  transpositions in the proof tree (e.g. a local `HashMap` keyed by
  `(position_hash, path_code, expected, remaining)`).
- Optional `dfpn`-per-child evaluation for very wide subtrees.
- Whether `extract_pv_internal` should still try to follow depth-matching twins
  after `find_ppv` is rewritten.
