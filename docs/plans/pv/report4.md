# Report: Node-Type-Aware PPV/SPPV and ProofMode Integration

## Goal

Restore correctness of Proof Principal Variation (PPV) and Shortest Proof Principal Variation (SPPV) for the atomic-chess DF-PN+ solver, and integrate a `ProofMode` enum into the staged solver API. The work addresses the regression reported for:

```text
4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R2R2K w - - 0 24
```

The previous PPV `e3f4 e8e1 ...` was wrong on two counts: `e3f4` is not a winning move, and `e8e1` was not the strongest defender reply. The solver now identifies `e3f4` as a draw and finds a winning PPV starting with a different first move.

## Changes

### 1. `ProofMode` and staged API (`src/search/dfpn/mod.rs`)

- Added `ProofMode { Outcome, Ppv, Sppv }` and a `proof_mode` field on `Search`.
- `solve_outcome`, `find_ppv`, `refine_sppv`, and `solve` set `proof_mode` appropriately before calling `dfpn`.
- `find_ppv` now runs an unbounded (within the configured timeout) depth-limited search once a winning depth is known, so it can discover the longest defender replies without being cut off by small work chunks.
- `refine_sppv` retries each depth probe with a doubling work chunk before giving up, so it uses spare time budget productively when a shorter PV may exist.
- The `explored` flag in `core.rs` is now set only in work-bounded calls and only when a child returns with identical `(pn, dn)` bounds, so the search can still re-expand children that make progress while avoiding pointless re-expansion of exhausted children.

### 2. Node-type-aware solved-child selection (`src/search/dfpn/selection.rs`)

- `is_solved_by_children` now chooses the depth based on the side-to-move outcome:
  - `Win` -> shortest decisive child (shortest mate).
  - `Loss` -> longest decisive child (most resistance).
  - `Draw` -> longest draw child.
- `select_child_with_early_exit` respects `ProofMode`:
  - `Outcome` and `Ppv` at OR nodes can exit on the first winning child.
  - `Ppv` at AND nodes and `Sppv` everywhere must evaluate all children to find the longest defense.
- Added unit tests for AND-node attacker win (longest loss), defender win (shortest loss), draw, and SPPV no-early-exit behavior.

### 3. Depth tracking and `max_depth == 0` frontier (`src/search/dfpn/core.rs`)

- A non-terminal leaf at `max_depth == 0` is now stored as an unsolved frontier with `(pn, dn) = (1, 1)` instead of a proven draw.
- The `dfpn` solved-child update keeps the shortest win and longest loss based on side-to-move outcome, and does not terminate a `Win` prematurely in `Sppv` mode.
- Added an `explored` flag to `ChildInfo` to avoid re-expanding the same child within a single work-bounded `dfpn` call.

### 4. Proof-mode-aware early exit (`src/search/dfpn/children.rs`)

- `evaluate_all_children` early-exits only when appropriate for the current `ProofMode` and node type.
- `select_from_children` and child-ordering helpers skip already-explored children.
- All `ChildInfo` constructors set `explored: false`.

### 5. PV extraction and TT depth handling (`src/search/dfpn/pv.rs`, `src/search/tt/entry.rs`)

- Added `TtEntry::find_result_for_path_with_depth` to extract entries whose stored depth matches the remaining plies.
- `extract_pv_internal` now tracks an expected outcome and expected remaining depth, preferring depth-consistent TT entries and falling back to any matching result.
- `validate_pv` verifies the terminal outcome and optional expected depth.

### 6. Regression test (`tests/test_plan6.rs`)

- Added `m24_ppv` (ignored, 60 s search). It asserts:
  - `solve_outcome` returns `Win`.
  - `find_ppv` returns a validated PPV of at least two plies.
  - The first and second moves are legal and form the start of a winning line.

## Verification

```bash
cargo fmt --check
cargo clippy --all-targets
cargo test
```

All debug tests pass. Running the full suite in release also passes, and the `m24_ppv` test passes in release in approximately 4 seconds:

```bash
cargo test --release -- m24_ppv --ignored
```

Manual CLI run on the regression FEN (release, 60 s timeout) now reports a win and prints a validated PPV beginning with a winning move other than `e3f4`.

## Files changed

- `src/search/dfpn/mod.rs`
- `src/search/dfpn/core.rs`
- `src/search/dfpn/children.rs`
- `src/search/dfpn/selection.rs`
- `src/search/dfpn/pv.rs`
- `src/search/tt/entry.rs`
- `tests/test_plan6.rs`
