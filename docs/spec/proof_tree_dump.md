# `proof_tree.bin` format specification

## Version

Version 1.

## Overview

`proof_tree.bin` is a compact, driver-free binary serialization of the
`atomic_solver` in-memory proof tree. It stores one adjacency record per node:
the id of the parent node and a 16-bit move code. Because no materialized
`ltree` path strings are stored, the file size is `O(nodes)` and does not grow
as `O(nodes × depth)` for deep principal variations.

External tools read this file, derive `outcome`, `depth`, `terminal`, and
`uci_move` for each node, and can rebuild an `ltree` path on import if desired.

## File layout

All multi-byte integers are **little-endian**.

| Field | Size | Description |
|---|---|---|
| `magic` | 8 bytes | ASCII `"ATOMTREE"` |
| `version` | 1 byte | Format version; currently `1` |
| `fen` | variable | Root position in FEN notation, terminated by `\n` (`0x0A`) |
| `root_outcome` | 1 byte | `0` Draw, `1` Win, `2` Loss |
| `root_depth` | 4 bytes | `u32` LE; proven distance from root to a terminal node |

After the header, every node is written as one record in node-creation order
(root first):

| Field | Size | Description |
|---|---|---|
| `parent_id` | 4 bytes | `u32` LE; `0xFFFFFFFF` for the root |
| `move_code` | 2 bytes | `u16` LE; `0` for the root (`Move::NONE`) |

The implicit node id of record `i` (0-indexed) is `i`. The proof tree is built
top-down, so a child always has a higher id than its parent; `parent_id` is
always `< i` for non-root nodes.

## Move encoding

The 16-bit `move_code` matches the public bit layout of
`atomic_movegen::types::Move`:

| Bits | Field | Meaning |
|---|---|---|
| 0-5 | `to_sq` | Destination square index (0-63) |
| 6-11 | `from_sq` | Origin square index (0-63) |
| 12-13 | `move_type` | `0` Normal, `1` Promotion, `2` EnPassant, `3` Castling |
| 14-15 | `promotion_idx` | For promotions: `0` Queen, `1` Rook, `2` Bishop, `3` Knight |

`to_sq` and `from_sq` map to `atomic_movegen::types::Square` using
`Square::from_u8`. `move_type` and `promotion_idx` select the appropriate
`Move::make_*` constructor.

`Move::NONE` encodes as `0` (`from_sq = to_sq = A1`, `move_type = Normal`,
`promotion_idx = 0`). This is the value stored for the root.

For display, a loader reconstructs `Move` from `move_code` and then uses
`move_to_uci` to obtain the UCI string. `move_to_uci` normalizes castling
moves to the standard UCI king-destination form (`e1g1`, `e1c1`, `e8g8`,
`e8c8`) regardless of whether `to_sq` is the king or rook square.

## Deriving node metadata

The binary file intentionally does **not** store `outcome`, `depth`,
`terminal`, or `uci_move` per node. A loader derives them:

### `outcome`

Outcomes alternate from the root with each edge. If `root_outcome` is `Win`
(`1`), nodes at even tree depths are `Win` and nodes at odd tree depths are
`Loss`; for `Loss` (`2`) the parity is reversed.

### `depth`

`depth` is the proven distance from the node to a terminal node. It is
computed by a post-order traversal:

- A terminal node (no children, or equivalently `depth == 0`) has `depth = 0`.
- A `Win` (OR) node is `1 + min(child.depth)`.
- A `Loss` (AND) node is `1 + max(child.depth)`.

### `terminal`

`terminal` is true when `depth == 0`.

### `uci_move`

For non-root nodes, reconstruct `Move` from `move_code` using
`Square::from_u8` and the appropriate `Move::make_*` constructor, then compute
`uci_move = move_to_uci(move)`.

## Worked example

Consider the three-node tree:

```
root (Win, depth 2)
└── e2e4 (Loss, depth 1)
    └── e7e5 (Win, depth 0)
```

Root FEN: `rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1`

Header:

- `magic`: `ATOMTREE`
- `version`: `1`
- `fen`: the FEN above followed by `\n`
- `root_outcome`: `1` (Win)
- `root_depth`: `2`

Records (decimal `move_code` values):

| node id | `parent_id` | move | `move_code` |
|---|---|---|---|
| 0 | `0xFFFFFFFF` | (root, `Move::NONE`) | `0` |
| 1 | `0` | `e2e4` | `796` |
| 2 | `1` | `e7e5` | `3364` |

`e2e4` encodes as `(from_sq = 12, to_sq = 28, move_type = 0)`:
`12 << 6 | 28 = 796`.

`e7e5` encodes as `(from_sq = 52, to_sq = 36, move_type = 0)`:
`52 << 6 | 36 = 3364`.

A promotion such as `e7e8q` would be `(from_sq = 52, to_sq = 60,
move_type = 1, promotion_idx = 0)`: `52 << 6 | 60 | (1 << 12) = 7484`.

## Versioning

Future versions may append fields to the header or introduce additional record
fields. Loaders must read the `version` byte and reject or skip unknown
versions.

## Notes

- No FEN is stored per node; positions are replayed from the root FEN and the
  move path.
- No `path` string is stored; the tree is reconstructed from `parent_id`
  records.
- `outcome` and `depth` are not stored because they can be derived exactly from
  the tree structure, keeping the file minimal.
