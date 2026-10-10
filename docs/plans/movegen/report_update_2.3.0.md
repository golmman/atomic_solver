# Upstream report: `atomic-movegen` 2.3.0 (`hash_after` / `rule50_after`)

Implements `docs/plans/movegen/plan_hash_after.md`. Recorded 2026-10-10.

## Release

- **Published version:** `atomic-movegen` 2.3.0 on crates.io
  (2026-09-10), semver-minor, additive only.
- The consumer bumped its requirement to `"2.3"` directly
  (`cargo update -p atomic-movegen`); no patch/registry staging was needed.

## Public API as implemented

Exactly the planned signatures (verified in the vendored crate source):

```rust
pub fn hash_after(&self, m: Move) -> u64;
pub fn rule50_after(&self, m: Move) -> u16;
```

`hash_after` reproduces every component of the incremental `do_move` hash:
side key, castling move geometry + rights transition + ep clearing, en-passant
and normal captures, promotion piece swap, the blast set over the exact
intermediate occupancy `do_move` uses, and the castling-rights cascade
evaluated post-blast. `rule50_after` mirrors the castling increment and the
pawn-move/capture reset.

## Correctness gates from the plan

- **Property test:** seeded random playouts over 5 start FENs × 16 seeds ×
  300-ply walks — 8,592 visited positions, every pseudo-legal move compared
  against a cloned board's `do_move` for both functions.
- **Fixtures:** castling (both sides, both colors), en-passant with a blasting
  capture, quiet and capturing promotions (all four pieces), blast removals
  clearing castling rights via the presence cascade (corner rook; e1/e8
  commoner), a double push replacing an existing ep square, rule50
  reset/increment cases.
- **Refactor safety:** the castling-rights transition is now a single pure
  helper (`castling_rights_after`) shared by `do_move` and `hash_after`, so
  the two paths cannot drift; the full upstream gate suite (41/41 perft at
  depth 6, move-order goldens) verified unchanged.
- **Consumer-side re-check (lean plan12):** a dedicated
  `position.rs` test asserts `Position::hash_after(m) == { do_move(m); hash() }`
  for every legal move on startpos, m22, shuffle-win, an en-passant, two
  castling, and a promotion-capture fixture, plus rule50 = 99/100 clamp
  positions — green, and the full drift protocol is bit-identical.

## Performance

Upstream micro-benchmark (changelog): `hash_after` is well under the plan's
≤ 1/3 target of a `do_move`+`hash`+`undo_move` round-trip (quiet moves are a
handful of XORs and table loads; captures pay the blast-set iteration only).

Consumer-side effect (lean plan12, x86_64 host): replacing the plan11
pre-pass's do/undo round-trip with `Position::hash_after` (board hash XOR
`rule50_key(rule50_after)`) measured **−12.5% m22 default, −12.9% m22
first-outcome, −12.6% shuffle-win FO, −13.9% quick suite** — above the −5–10%
expectation; the pre-pass's residual share fell from ≈14% to ≈5% of wall
(V12-style double-pre-pass probe). All outputs bit-identical.
