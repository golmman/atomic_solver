# Report 6: Proven-subtree PPV extraction in `find_ppv`

## What was implemented

`Search::find_ppv` now extracts the Proof Principal Variation (PPV) from the
proven subtree instead of following the live transposition table's `best_move`
chain. The key changes are in `src/search/dfpn/pv.rs` and
`src/search/dfpn/mod.rs`:

- Added `Search::extract_ppv_from_proven_subtree` in `pv.rs`. It walks the
  proven subtree recursively, picking the child that matches the minimax PPV
  rule: at an attacker node (`expected == Win`) the shortest winning child, and
  at a defender node (`expected == Loss`) the longest losing child.
- The helper maintains `path_stack` and `path_code` with the same 1-indexed
  `zobrist::path_random` convention as `dfpn` and `extract_pv_internal`, and
  treats repeated positions as draws.
- It uses alpha-beta pruning for attacker nodes: once a child wins in `D`
  plies, later children only need to win in fewer than `D` plies. Defender
  nodes evaluate every legal reply with the full remaining budget.
- A success cache (`PpvCache`, keyed by `(position_hash, path_code, expected)`)
  memoizes completed subtrees so transpositions and repeated calls are shared.
- `find_ppv` in `mod.rs` first tries `extract_ppv_from_proven_subtree` with a
  bound tightened from `bootstrap_success_depth` (or from `extract_pv_checked`
  if the bound is missing). If that times out or fails, it falls back to
  `extract_pv_checked` plus `refine_sppv`, and finally to `extract_ppv` /
  `extract_pv`.
- Updated `bootstrap_success_depth` and `bootstrap_fail_depth` after a successful
  extraction so later stages start from a tight bound.
- Fixed example binaries (`examples/test_find_ppv.rs`,
  `examples/test_solve_time.rs`, `examples/test_refine.rs`) to use only the
  public API.

`src/main.rs`, `src/search/tt/`, and `src/search/ordering.rs` were not
modified. A small change in `src/search/dfpn/core.rs` was kept from the
previous session: `best_from_tt` is only used for OR-node move ordering so
stale `best_move` hints do not affect defender tie-breaking during the
`dfpn` search.

## Deviations from the plan

- The plan proposed a pure recursive pass with no TT consultation. The final
  implementation still uses `sort_moves` and `probe_best_move` for move ordering
  inside `extract_ppv_from_proven_subtree` because ordering only affects speed,
  not correctness.
- Memoization was added even though the plan listed it as optional. Without it
  the pass timed out on the m27 regression in debug builds.
- The fallback path in `find_ppv` uses `refine_sppv` rather than only
  `extract_ppv` / `extract_pv`. This gives a second chance to find a short PPV
  when the recursive pass does not finish within the time budget.

## Problems encountered and solutions

1. **Stale `best_move` chains produced non-PPVs.** `solve_outcome` stored the
   11-plies `b1b8 g8h7 ...` line because it stops at the first winning child.
   Following the TT `best_move` chain therefore returned a valid win but not the
   strongest defense. The recursive pass ignores those hints and recomputes the
   selection from child outcomes/depths.

2. **Plain recursive evaluation was too slow.** The first version evaluated all
   children to `remaining - 1` and then selected. Adding attacker alpha-beta
   pruning and a success cache reduced the m27 extraction from timing out to
   completing in tens of milliseconds.

3. **`dfpn` Sppv probes were unreliable within the budget.** Attempts to drive
   `find_ppv` with bounded `dfpn` Sppv searches produced non-shortest or
   work-limited results because cached solved entries from `Outcome` mode are
   upper bounds, not exact shortest depths. The pure recursive extractor avoids
   this by not relying on cached solved depths.

4. **Defender `child_bound` initially used attacker alpha-beta.** Using
   `best_total - 2` for defender replies caused the pass to reject valid
   defenses or return wrong results. The final code evaluates defender replies
   with the full `remaining - 1` budget.

5. **Example binaries broke after internal API changes.** Examples that accessed
   private fields/methods were rewritten to use the public `solve` / `find_ppv`
   / `refine_sppv` APIs.

## Verification

Standard checks:

```bash
cargo fmt --check
cargo clippy --all-targets
cargo test
cargo test --release
cargo doc --no-deps
```

All passed with no warnings.

Regression commands:

```bash
cargo run --release -- --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26" --timeout 60 --no-refine-shortest
# -> outcome: win
# -> pv: b1b8 g8f7 b8f8 f7g7 d6e5 g7h7 f8h8

cargo run --release -- --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26" --timeout 60
# -> outcome: win
# -> pv: b1b8 g8f7 b8f8 f7g7 d6e5 g7h7 f8h8
# -> sppv search finished

cargo run --release --example verify_ppv -- --timeout 60 \
    --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26" \
    --moves "b1b8 g8f7 b8f8 f7g7 d6e5 g7h7 f8h8"
# -> is_ppv: true
```

Test matrix (debug and release):

- `test_plan6::m27_ppv_only` ... ok
- `test_plan6::m27_streaming_output` ... ok
- `test_plan6::m27_shortest_pv` ... ok
- `test_plan6::m27_kh7_fast_win` ... ok
- `test_plan6::m27_white_wins` ... ok
- `test_plan6::black_root_report6_fen` ... ok
- `test_plan6::m28_black_loses` ... ok
- `test_plan6::m28_white_wins` ... ok
- `test_plan6::m29_white_wins` ... ok
- `test_plan6::timeout_message` ... ok

All other non-ignored tests in `cargo test` and `cargo test --release` passed.

## Open ends

- `extract_ppv_from_proven_subtree` evaluates wide defender subtrees exhaustively.
  A future optimization could use a bounded `dfpn` call per child (with
  `proof_mode = Sppv` and no reliance on cached `best_move` depths) to handle
  very wide subtrees faster.
- The success cache is currently keyed by `(hash, path_code, expected)` and stores
  the exact PPV/depth. It does not cache failures; caching failures by remaining
  budget could avoid redundant work during binary-search-like fallback paths.
- Tie-breaking follows `sort_moves` ordering. If a position has multiple
  same-depth optimal defender replies, the chosen PPV may differ from the one
  `refine_sppv` would emit; both are valid PPVs.
