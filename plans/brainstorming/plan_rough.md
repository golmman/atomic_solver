# Rough Plan: Atomic Chess Solver

Goal: Build a pure atomic-chess solver from scratch, leveraging the existing `atomic-movegen` crate for move generation and position representation. The solver determines the exact game-theoretic outcome of a position (win / loss / draw) and, when the outcome is a win, produces a principal variation.

## 1. Why Proof-Number Search

Atomic chess is highly tactical, with many forced sequences and king-explosion motifs. The game graph is large, but the **target is binary** (win/loss/draw). A best-first solver is usually more efficient than deep alpha-beta for finding forced wins or refutations.

**Primary search paradigm:**
- **Proof-Number Search (PN)**: Tree-based best-first solver with proof/disproof numbers.
- **Depth-First Proof-Number (DF-PN)**: Iterative-deepening variant that avoids memory explosion; best suited for atomic chess.
- **PN² / PDS-PN**: For deeper/more complex positions where standard DF-PN runs into horizon effects.
- **Retrograde analysis / endgame tablebases**: For exact low-material endgames.

A traditional evaluation function is not needed for a pure solver: leaves are only terminal or tablebase-known positions.

## 2. Core Components

### 2.1. Position & Move Generation
- **Outsource:** `atomic-movegen` (already provides legal move generation, position representation, FEN, etc.)
- **In-house:** Lightweight wrapper that exposes the position to the search loop, hashes, and repeated-position logic.

### 2.2. Search Core
- **Proof-Number Search (PN)**: Expand the most-proving node until the root is proven or disproven.
- **DF-PN**: Iterative deepening with proof/disproof thresholds; the main practical solver.
- **PN² / PDS-PN**: Optional enhancements for very deep lines.
- **Terminal detection**: Mate, stalemate, threefold repetition, 50-move rule, and (if any) atomic-specific draw rules.
- **Iterative deepening**: Drive DF-PN with increasing thresholds, with the ability to stop cleanly on memory/time limits.

### 2.3. Transposition Table
- Use **Zobrist hashing** from `atomic-movegen` if available, otherwise implement our own.
- Store the key facts for each node: proof number, disproof number, best child, and outcome.
- Replace with a custom, tightly packed table if `atomic-movegen`’s position is large.
- Consider **two-tier/bucketed tables** for memory efficiency.

### 2.4. Move Ordering and Static Heuristics
Since there is no evaluation function, we still need good move ordering to keep the DF-PN tree small:
- **Capture ordering**: prefer captures and king threats, especially explosive captures.
- **King safety / threat heuristics**: move kings toward dangerous squares or away from forced explosions.
- **Pattern database**: small atomic-mate patterns (e.g., `KxK` adjacency motifs) can short-circuit search.
- **History / killer**: optional, less central than in alpha-beta but still cheap.

### 2.5. Endgame Tablebases (EGTB)
- Atomic endgames are small-material situations where exact outcome is critical.
- Generate **retrograde tablebases** offline for low-material endgames (K vs K, K+N vs K, etc.).
- Probe tablebases at search leaves for perfect conversion.
- If tablebase construction is out of scope, probe the search tree directly until material is very low.

### 2.6. Resource Limits & Termination
- Memory cap and node-count cap for DF-PN.
- Time cap (optional): allow a user to ask "spend at most N seconds" and return best-effort result.
- Clean interruption so that the current proof/disproof status can be reported.

### 2.7. Save / Load State
- Serialize the DF-PN search tree and/or transposition table to disk so a long solving run can be interrupted and resumed.
- Two options:
  - **TT-only snapshot**: smaller, faster, but loses the tree structure and may require re-expansion.
  - **Full tree snapshot**: preserves the exact state, but can be large and slow to write.
- Use a compact binary format (custom or `serde` + `bincode`).
- Optional compression (e.g., `zstd`) to reduce disk usage.
- Add CLI commands: `--save <file>`, `--load <file>`, `--autosave-interval <sec>`.

### 2.8. CLI / I/O
- A simple command-line interface to feed FENs and receive the solved outcome.
- Optional PGN/FEN input parsing (reuse `atomic-movegen` or a small crate).
- Output: `win`, `loss`, `draw`, principal variation, and proof/disproof statistics.

### 2.9. Testing & Validation
- **Perft** from `atomic-movegen` to validate move generation.
- **Test suite** of known atomic-mate problems (e.g., `test/mates/`).
- **Self-tests** on small tablebases to verify the solver agrees with retrograde results.
- **Benchmarks** with `criterion` to measure nodes/sec and tree efficiency.

## 3. What to Outsource as Libraries

Some parts can be pulled from crates without meaningful performance loss:

| Component | Outsource? | Candidate Crates / Notes |
|-----------|-----------|--------------------------|
| Move generation | **Yes** | `atomic-movegen` (already chosen) |
| FEN / SAN / PGN parsing | **Yes** | `shakmaty`, `chess`, `vocabulary`, or use `atomic-movegen` if it exposes them |
| CLI parsing | **Yes** | `clap` |
| State serialization | **Yes** | `serde` + `bincode` for compact binary snapshots |
| Compression | **Yes** | `zstd` or `lz4` for large snapshot files |
| EGTB compression/serialization | **Partially** | `serde` + custom binary; probe logic in-house |
| Transposition table | **Partially** | `hashbrown` for fast hash map; custom bucketed table for low memory |
| Async / logging | **Yes** | `tokio`, `tracing` (only if needed) |
| Benchmarking / testing | **Yes** | `criterion`, `insta`, `proptest` |

### What to keep in-house for performance
- **Search loop** (PN / DF-PN): The hottest path; custom allocation and table access are critical.
- **Move ordering heuristics**: Atomic-specific and tightly coupled to the position representation.
- **Transposition table layout**: Align to position size and cache lines.
- **DF-PN node expansion and memory management**: `bumpalo` or `slab` arenas, or a custom compact node pool.
- **Terminal detection**: Must be exact and fast.

## 4. Recommended Architecture

```
atomic_solver/
├── src/
│   ├── main.rs              # CLI entry point
│   ├── cli.rs               # Argument parsing and command dispatch
│   ├── search/
│   │   ├── pn.rs            # Proof-number search
│   │   ├── dfpn.rs          # Depth-first proof-number search
│   │   ├── transposition.rs # Zobrist + TT
│   │   ├── ordering.rs      # Move ordering for PN/DF-PN
│   │   └── persistence.rs   # Save/load search state
│   ├── tablebase/
│   │   ├── generator.rs     # Retrograde tablebase generation
│   │   └── probe.rs         # Tablebase lookup
│   └── tests/
├── tables/                  # Generated endgame tablebases
└── benches/
```

## 5. Rough Milestones

1. **Skeleton**: CLI, `atomic-movegen` integration, position I/O, terminal detection.
2. **Basic PN solver**: Implement proof-number search, prove a small set of forced mates.
3. **DF-PN solver**: Convert to depth-first proof-number search to handle larger positions.
4. **Transposition table**: Add Zobrist hashing and a fast TT.
5. **Move ordering**: Implement capture ordering and atomic-specific threats.
6. **Tablebases**: Generate 3- and 4-man atomic tablebases and probe them at leaves.
7. **Resource limits**: Memory, node-count, and optional time limits with clean termination.
8. **Save/load state**: Serialize the TT or full search tree to disk and resume a run.
9. **Polish**: CLI ergonomics, autosave, test suite, benchmarks, and documentation.

## 6. Key Risks

- **Memory explosion** in PN search; mitigate with DF-PN, node pools, and strict memory limits.
- **Cycles / repetitions** in atomic chess; handle in the search (threefold, 50-move rule).
- **Position representation size** may affect TT density; profile with real data.
- **King adjacency / explosion rules** are subtle; rely on `atomic-movegen` and test thoroughly.
- **Draw detection**: In atomic chess, kings can be adjacent without check; exact terminal/draw rules must be correct.
- **Persistence overhead**: Saving a full DF-PN tree can be slow and consume large disk space; prefer TT-only snapshots or periodic autosave.
- **Serialization consistency**: Restored state must exactly match the original search tree to avoid invalid proof/disproof numbers.

## 7. Open Questions

- Does `atomic-movegen` expose Zobrist hashing or a canonical hash? If not, we implement it.
- Does it expose FEN/SAN/PGN parsing? If so, we can avoid extra parser crates.
- What endgame material limits do we need for tablebases?
- Do we want multi-threaded search? (DF-PN can be parallelized with shared TT, but adds complexity.)
- What should the CLI output format be? (JSON, plain text, SAN line, etc.)
- How much state should be persisted: TT only, full tree, or both? Is autosave at fixed intervals acceptable?
