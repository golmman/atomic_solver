# Upstream report: `atomic-movegen` 2.2.0 (`has_legal_move`)

Implements `docs/plans/movegen/plan_has_legal_move.md` (Phase 1 of
`docs/plans/lean/plan3.md`). Recorded 2026-09-08.

## Release

- **Published version:** `atomic-movegen` 2.2.0 on crates.io
  (2026-09-08), semver-minor, additive only.
- The `[patch.crates-io]` interim from plan3 Decision 6 was not needed;
  the consumer bumps its requirement to `"2.2"` directly.

## Public API as implemented

```rust
pub fn has_legal_move(board: &Board) -> bool;
pub fn has_legal_move_with_state(board: &Board, state: &StateInfo) -> bool;
```

`has_legal_move_with_state` skips the candidate stream entirely when
`board.commoners(board.side_to_move()).is_empty()` (every candidate would
be rejected by the `our_commoners.is_empty()` guard in `Board::legal`),
otherwise drives `for_each_pseudo_legal` with the exact
`generate_legal_with_state` filter
(`is_move_trivially_legal(board, m, state) || board.legal(m, state)`) and
returns at the first accepted candidate. It does not call
`populate_state` itself; the convenience wrapper does.

## Gates from the plan

- **Order golden (step 0):** `tests/move_order_golden.rs` pins the exact
  `generate_pseudo_legal` / `generate_legal` output order for a fixed
  position list; goldens were generated from the pre-refactor
  implementation. Passes unchanged after the `for_each_pseudo_legal`
  refactor.
- **Refactor (step 1):** `generate_pseudo_legal` is now a thin collecting
  wrapper over the internal `for_each_pseudo_legal` stream
  (`ControlFlow`-based, same stage order). Existing crate tests and perft
  pass; clippy/fmt clean.
- **Equivalence (step 2):** deterministic seeded playouts (~8,600
  positions from varied start FENs, capture-heavy lines included) assert
  `has_legal_move_with_state(b, s) == (generate_legal_with_state(...),
  list.len() > 0)` on every position, plus the deterministic fixtures
  from the plan (checkmate, stalemate, one-escape, castling-only,
  en-passant-only, promotion-heavy, extinction, K vs K, all-pinned).
- **Benchmark (step 3):** 2M-iteration `--release` loop per position:

  | Position         | Legal moves | `generate_legal` | `has_legal_move` | Speedup |
  |------------------|------------:|-----------------:|-----------------:|--------:|
  | Quiet middlegame | 42          | 87.8 ns          | 8.9 ns           | 9.9×    |
  | Start position   | 20          | 49.6 ns          | 8.8 ns           | 5.6×    |
  | In check         | 4           | 38.0 ns          | 12.1 ns          | 3.1×    |
  | Captures only    | 2           | 40.6 ns          | 12.9 ns          | 3.2×    |
  | No legal move    | 0           | 125.6 ns         | 125.3 ns         | 1.0×    |

## Deviations

None. The optional fast pre-pass (plan design item 3) was not added: the
closure-driven stream already reaches 8.9 ns on quiet positions, leaving
no measurable headroom for a separate pre-pass.
