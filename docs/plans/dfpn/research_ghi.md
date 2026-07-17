# Report: Fixing the Graph-History Interaction Problem for DF-PN

This report is based on the paper **"A General Solution to the Graph History Interaction Problem"** by Akihiro Kishimoto and Martin Müller (`plans/dfpn/ghi.pdf`). It extracts the parts of the paper that are directly relevant to making a depth-first proof-number (df-pn) search reuse its transposition table safely in the presence of repeated positions.

## 1. Source summary

- **Authors / venue:** Akihiro Kishimoto and Martin Müller, AAAI 2004.
- **Problem:** The *Graph History Interaction (GHI) problem*: when a search graph contains cycles, a transposition table entry for a position can depend on the path taken to reach that position. Reusing such entries without checking the path can produce incorrect proofs or disproofs.
- **Scope:** The paper gives a single scheme that handles both common rule variants:
  - **first-player-loss:** a repetition is a loss for the first player (e.g. checkmate-problem searches, where a repetition does not help the attacker).
  - **current-player-loss:** a repetition is a loss for the player who repeats the position (e.g. situational super-ko in Go).
- **Algorithms covered:** df-pn (Nagai 2002) and alpha-beta.
- **Main idea:** When a previously proven or disproven position is reached via a *new* path, do not trust the table entry blindly. Verify it with a small *simulation* search. If the simulation succeeds, the proof/disproof is valid for the new path and is reused; otherwise, the new path is treated as a separate position. Unproven entries are shared as ordinary transpositions.

## 2. Why GHI is dangerous for df-pn

The standard df-pn algorithm uses a transposition table to cache proof and disproof numbers (`pn`, `dn`). If a position is proven, its stored numbers are `(0, INF)`; if disproven, `(INF, 0)`. The table is keyed only by the position, not by the path to the position.

In a cyclic graph, the *same* position can have different game-theoretic values depending on the history:

- **First-player-loss example:** A node that was reached through a loop is a disproof for the attacker, but when reached through a different, shorter path, the same node can be part of a winning plan.
- **Current-player-loss example:** A node reached through one path may have a legal move that is a repetition for the opponent, and is therefore a win. Through a different path, the same move may be a repetition for the player to move, and is therefore illegal.

Using a naive transposition table, the result from the first path is stored and then retrieved for the second path, which can flip a win into a loss or vice versa. Because df-pn relies on `(pn, dn)` bounds to drive the search, a single corrupted entry can propagate an incorrect solved result all the way to the root.

## 3. The general solution

Kishimoto and Müller solve GHI by adding **path information** to the transposition table and by using **Kawano's simulation** to verify cached results.

### 3.1 Base and twin transposition-table entries

A transposition table entry is split into a *base* part and zero or more *twin* parts:

- **Base entry:** stores the ordinary `pn`/`dn` bounds for an *unsolved* position. It is used as a normal transposition for all unsettled paths.
- **Twin entry:** stores a proof or disproof *together with the exact path* that produced it. When a position is proven (or disproven) via path `p`, the solver creates a twin entry keyed by the position and the encoded path `p`.

When the same position is reached later via path `q`:

1. If `q` matches the path in an existing twin entry, the cached proof/disproof is reused directly.
2. If `q` does not match, the twin's proof/disproof is *simulated* to see whether it still holds for path `q`.
3. If at least one twin simulates successfully, the result is reused for `q` and a new twin entry for `q` is created.
4. If no twin verifies, the base entry's `pn`/`dn` bounds are used and the search continues.

This design keeps the ordinary transposition-table behavior for unsolved nodes and only pays the extra cost for proven/disproven nodes in cyclic parts of the graph.

### 3.2 Encoding the path

A path signature is stored in each twin entry. It is a 64-bit Zobrist-style hash of the sequence of moves from the root to the node:

```text
let path = (m1, m2, ..., mk)
code(path) = R[m1][1] xor R[m2][2] xor ... xor R[mk][k]
```

`R` is a precomputed table of 64-bit random numbers indexed by `(move, depth)`. The depth index makes the encoding order-sensitive: the same moves in a different order produce different codes.

For a domain with many distinct moves (e.g. shogi or Amazons), a move can be split into parts (from-square, to-square, promotion), increasing `MaxDepth` but reducing `MaxMove` and keeping the table small.

### 3.3 Kawano's simulation

Simulation is a fast, small search that tries to *borrow* a proof tree already stored in the transposition table.

- For a **proof simulation**, the algorithm starts from the candidate position and follows the winning move for each OR node as recorded in the existing proof tree. AND nodes must still expand all of their children.
- For a **disproof simulation**, the algorithm follows the best disproof tree.

Because the existing tree is already known, simulation is much cheaper than a fresh search. The key difference from ordinary df-pn is that OR-node moves are taken from the transposition table rather than from the move generator.

If simulation succeeds, the proof/disproof is valid for the new path. If it fails, the cached result is path-dependent and must not be reused for that path.

### 3.4 Reducing simulation calls

Most positions do not need the twin mechanism at all. The solver should:

- Create a **twin** only when a position is proven or disproven *and* a repetition was involved in the search.
- If a node is (dis)proven *without* detecting a repetition, store the result directly in the **base** entry. Such results are path-independent and can be reused for any path.

In the paper's experiments, this simple rule made the number of necessary simulations small and the overhead negligible.

## 4. Df-pn specific modifications

The paper lists two modifications to the ordinary df-pn search that are required for the GHI fix to work correctly.

### 4.1 Reinitialize the base entry after a twin proof/disproof

When a proof or disproof is stored in a twin entry, the `pn`/`dn` numbers in the *base* entry are re-initialized to `(1, 1)`. This avoids a practical problem: df-pn tends to grow large proof and disproof numbers before it finally finds a proof/disproof, and those stale bounds can prevent the search from solving positions that the twin mechanism would otherwise handle.

### 4.2 Root threshold initialization

In the original df-pn, the thresholds at the root are initialized with one large value. In the paper's modified version, the root thresholds are initialized to `(1, 1)`. This is necessary because df-pn stores thresholds in the transposition table before expanding a node. Starting with `(1, 1)` avoids the GHI problem at the root itself.

A result returned as `(0, INF)` or `(INF, 0)` is a correct proof/disproof. Any other result at the end is treated as `unknown`.

## 5. Sketch of a Rust implementation

The following code is a conceptual, `std`-only sketch of the data structures and the table lookup logic. It is meant to capture the paper's algorithm, not to be the final, production-ready code.

### 5.1 Path code

```rust
const MAX_MOVE: usize = 512;
const MAX_DEPTH: usize = 64;

static PATH_RANDOM: [[u64; MAX_DEPTH]; MAX_MOVE] = {
    // precomputed random numbers
    [[0; MAX_DEPTH]; MAX_MOVE]
};

fn path_code(path: &[usize]) -> u64 {
    let mut code = 0u64;
    for (depth, &mv) in path.iter().enumerate() {
        code ^= PATH_RANDOM[mv][depth];
    }
    code
}
```

### 5.2 Transposition-table entry

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Win,
    Loss,
    Draw,
}

#[derive(Clone, Copy)]
struct PnDn {
    pn: u32,
    dn: u32,
}

struct TwinEntry {
    path_code: u64,
    outcome: Outcome,
    best_move: Move,
}

struct Entry {
    key: u64,
    base: Mutex<EntryData>,
    twins: Mutex<Vec<TwinEntry>>, // one entry per path that produced a proof/disproof
}

struct EntryData {
    pn: u32,
    dn: u32,
    best_move: Move,
}
```

### 5.3 Lookup with GHI handling

```rust
fn lookup_with_ghi(
    entry: &Arc<Entry>,
    current_path_code: u64,
    path: &[usize],
    pos: &Position,
) -> LookupResult {
    let base = entry.base.lock().unwrap();
    let twins = entry.twins.lock().unwrap();

    // First, try to find an exact twin for the current path.
    for twin in twins.iter() {
        if twin.path_code == current_path_code {
            return LookupResult::Solved(twin.outcome, twin.best_move);
        }
    }

    // Next, try to simulate a twin from a different path.
    for twin in twins.iter() {
        if simulate(entry, twin, path, pos) {
            // Simulation succeeded: create a twin for this path.
            let new_twin = TwinEntry {
                path_code: current_path_code,
                outcome: twin.outcome,
                best_move: twin.best_move,
            };
            drop(base);
            drop(twins);
            entry.twins.lock().unwrap().push(new_twin);
            return LookupResult::Solved(twin.outcome, twin.best_move);
        }
    }

    // No twin verified; use the base entry's bounds.
    LookupResult::Bounds(PnDn {
        pn: base.pn,
        dn: base.dn,
    }, base.best_move)
}
```

The `simulate` function would run a small df-pn search, borrowing moves from the twin's proof or disproof tree and verifying that the terminal nodes are still valid under the current path.

### 5.4 Storing a result

```rust
fn store(entry: &Arc<Entry>, outcome: Option<Outcome>, pndn: PnDn, best_move: Move, path_code: u64, repetition_seen: bool) {
    let mut base = entry.base.lock().unwrap();
    let mut twins = entry.twins.lock().unwrap();

    if let Some(o) = outcome {
        if repetition_seen {
            // Path-dependent result: create a twin.
            twins.push(TwinEntry {
                path_code,
                outcome: o,
                best_move,
            });
            // Reset the base entry bounds so future searches do not get stuck.
            base.pn = 1;
            base.dn = 1;
        } else {
            // Path-independent result: store in the base entry.
            let (pn, dn) = outcome_to_pndn(o);
            base.pn = pn;
            base.dn = dn;
            base.best_move = best_move;
        }
    } else {
        // Unsolved node: store only the bounds.
        base.pn = pndn.pn;
        base.dn = pndn.dn;
        base.best_move = best_move;
    }
}
```

### 5.5 Integration with df-pn

The `or_node`/`and_node` functions need only two extra hooks:

1. Before trusting a solved table entry, call `lookup_with_ghi` and pass the current path code.
2. When storing a result, call `store` with the current path code and a flag that records whether the subtree that produced it saw a repetition.

The rest of the df-pn machinery — child selection, threshold propagation, iterative deepening — remains unchanged.

## 6. Correctness and performance

The paper proves (Theorem 1) that the scheme does not suffer from either the *draw-first* or the *draw-last* case of GHI, provided that **all proven and disproven nodes are saved in the transposition table**. Unproven `pn`/`dn` bounds may be slightly inaccurate, but the returned proofs and disproofs are always correct.

The experimental results in the paper show a tiny overhead:

- In Go (current-player-loss), the df-pn solver that handles GHI expanded about 1.5% *fewer* nodes than the solver that ignored GHI.
- In checkers (first-player-loss), the GHI-aware solver solved slightly more positions and expanded slightly fewer nodes than the GHI-ignoring solver.

The overhead is dominated by the number of simulation calls. The paper reports that simulation typically explored only a few hundred nodes per call and caught many corrupted transposition-table entries. The key to the low overhead is the rule that only nodes whose proof/disproof depends on a repetition are stored as twins.

## 7. Recommendations for the atomic solver

The atomic solver currently uses a simpler, safer baseline: it stores an explicit `Outcome` in the transposition table and does not reuse transposition-table results for repeated positions in the current thread's `path` set. The paper's scheme can be adopted incrementally:

1. **Start with the existing `path` set.** Any child already on the current thread's path is treated as a local draw (or as a loss for the current player, depending on the chosen rule). Do not store these transient results in the TT.

2. **Add `rule50` to the Zobrist key.** The 50-move draw is a path-dependent condition; the paper's path encoding handles the move sequence, but including the halfmove clock directly in the key is a cheap way to keep positions with different draw clocks distinct.

3. **Implement the twin mechanism as the next step.** Once the basic solver is correct, add path codes and a twin list per table entry. Only create twins when a proof or disproof is found while a repetition is on the current path. This keeps the table small and the simulation count low.

4. **Use simulation for verification.** A simulation search for df-pn can borrow the `best_move` chain from the cached proof tree and re-verify it. If it succeeds, the result is reused for the new path; if it fails, fall back to the base `pn`/`dn` bounds.

5. **Keep the `outcome` field as the source of truth.** The `pn`/`dn` pair are still search bounds. A solved result is only trusted when the `outcome` field is `Some` and the path verification succeeds.

## 8. Summary

- The GHI problem arises because transposition-table entries keyed only by position ignore the path that created them.
- Kishimoto and Müller solve it by:
  - storing path-dependent proofs/disproofs in **twin entries** keyed by a path code,
  - verifying a twin against a new path using **Kawano's simulation**, and
  - keeping path-independent results in the **base entry**.
- For df-pn, the two extra changes are:
  - reinitialize the base entry to `(1, 1)` when a twin proof/disproof is stored, and
  - initialize the root threshold to `(1, 1)`.
- The overhead is negligible and the correctness guarantee is strong: every returned proof or disproof is correct, both in first-player-loss and current-player-loss scenarios.

## References

- A. Kishimoto and M. Müller, "A General Solution to the Graph History Interaction Problem," *Proceedings of the 19th National Conference on Artificial Intelligence (AAAI-04)*, 2004.
- A. Nagai, "Df-pn Algorithm for Searching AND/OR Trees and Its Applications," Ph.D. Dissertation, University of Tokyo, 2002.
- Y. Kawano, "Using Similar Positions to Search Game Trees," *Games of No Chance*, MSRI Publications, 1996.
- A. L. Zobrist, "A New Hashing Method with Applications for Game Playing," Technical Report, University of Wisconsin, 1970.
