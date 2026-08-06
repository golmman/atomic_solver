## Core problem

Transposition table entries are volatile so we face problems in these areas:

- Providing proof for an outcome
- Extracting PPVs and SPPVs
- Search monitoring and debugging

## Idea

**Basics:**

- during search build dedicated tree with proven nodes
- on key press 'q'
  - if outcome was found already, traverse proof tree to find a ppv
  - save the proof tree as a postgres db
    - via ltree
    - path = uci-moves?
    - all AND-nodes, only "shortest" OR-nodes

The ltree-postgres db can then also be analyzed by external tools.

**Stretch goals:**

- proof tree operations run in dedicated thread
- mpsc queue for all operations

## Testability

PPVs should be easily extractable from the proof tree and can be verified via `verify_ppv`.

More plausibility checks and tests can be done by analyzing the proof tree (tbd).

## Task

Help me formulate a concept.

I am aiming for an mvp-solution:

- build the core feature first
- test it extensively
- extend it later

Propose a rough roadmap: which sub-features need to be build in what order?

Ask questions until ambiguity is at an acceptable level. Questions should come with options and their tradeoffs and should be numerated like 1a, 1b, 1c, 2a, 2b, ... .
Push back where necessary.
Write the results to `docs/plans/storage/concept.md`.

---

I feel we need to increase the Testability of the mvp phases.

Core idea: not only hitting q+return but also the timeout triggers the pre-exit hook.
That way a coding agent can easily verify the trigger functionality by waiting for the timeout.

Proposal where each phase has a testable result:

- Phase 1: --outcome-only flag and timeout/q-trigger -> simple log
- Phase 2: timeout / q -> dump dummy/test postgres ltree
- Phase 3: add worker thread and proof tree creation, on timeout / q -> log simple proof-tree statistics
- Phase 4: timeout / q -> extract PPV from proof-tree and add to the logged statistics
- Phase 5: timeout / q -> export proof tree to db

What do you think?

---

My idea adressing the unbounded memory risk:

- new option --pt-size (pt = proof tree) with a default of 256mb
- when exceeded: execute pre-exit hook and exit
- could be added in phase 3

Answers to the open questions:

1. on `q` the pre-exit hook is executed, then the program quits
2. `mpsc` should be sufficient for now, evaluation for more sophisticated techniques comes after the mvp
3. i'd prefer `src/proof_tree/` since it runs in another thread and this decoupling should be reflected in the directory structure
4. partial trees are fine for the mvp, no `complete` boolean for now

Again, push back if necessary.

With these clarifications, are there any open questions that need answers before we go into implementation plan creation stage?

---

Another idea.
Here is my simple, generic implementation of a tree in rust (not adapted to our use case yet):

```rust
struct Graph {
    nodes: Vec<Node>,
}

struct Node {
    parent: Option<usize>,
    value: String,
    children: Vec<usize>,
}
```

Maybe this saves some overhead in the `ProofTree` structure? What would be the tradeoffs compared to your structure?

---

Create implementation plans for the 5 phases of `docs/plans/storage/concept.md`. Store them in `docs/plans/storage/` as `plan1.md`, `plan2.md`, etc. .

---

We just implemented `docs/plans/storage/plan4.md` (see `docs/plans/storage/report4.md`).

The size of the output `proof_tree.sql` is exploding and doesn't scale well.
We need to address this before we continue with `docs/plans/storage/plan5.md`

One idea is to simply dump an adjacency list (id, parent_id, label) and build the ltree path in PostgreSQL on import with a recursive CTE externally.

No implementation here, let's discuss this. What options do we have here?

---

cargo run --release -- --fen "4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23" --timeout 10

Given the proof-tree has N nodes, with `docs/plans/storage/plan5.md` implemented what would be the complexity of the proof-tree export in O-notation?

clarifications

- pre-exit hook is not time bound
- before dump

---

When i run

```
cargo run --release -- --fen "4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23" --timeout 10
```

an outcome is found and a proof tree with 6040 nodes is exported.

The only exported move from the starting position is e3f4 for white, which is fine.
After that i expected ALL black (defender) moves in the dump but i only see c6c5.
Since all defender AND nodes were proven/disproven in this position why were they not available for the export?

Do i have a misconception here or does this point to an issue that needs fixing?

---

I want the search for an PPV or SPPV removed and cleaned up.

Instead i want this process:

- search for an outcome
- when an outcome of length N was found, switch to a bounded search of depth N-2
- when a shorter outcome line of length M is found, switch to a bounded search of depth M-2
- each new found outcome is logged with its line length
- all proven/disproven nodes are pumped to the proof tree
- the proof tree is exported via the pre-exit hook

Create a plan for these changes in `docs/plans/storage/plan6.md`.

---

When i run

```
cargo run --release -- --fen "4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23" --timeout 10
```

an outcome is found and a proof tree with 5737 nodes is exported.

One line of exported half move is this:

1. e3f4 a5a4
2. g1e1 a4a3
3. e1e7 c6c5

The line stops there but does not prove the attackers win, it is incomplete.
One simple verification check is that the last move must be an attacker move, i.e. the number of half moves must be odd in each line.

Please investigate what went wrong.

---

I have more questions.

**Q1**
You say "it asks the TT for the best_move / children".
Why does the TT store any children? What are they needed for? Wouldn't the prove/disprove numbers be sufficient?

**Q2**
I'd like to decouple TT and proof tree completely, is that possible? Is it reasonable?
Idea: We could store nodes that were not expanded (tt hits) with a flag.
In the pre-exit hook we add the children by searching in the proof tree for the continuation.

---

The current “build the proof tree from the TT after the search” approach has several concrete shortcomings:

1. TT eviction loses branches
   The proof tree can be much larger than the TT budget (default 64 MB vs 256 MB for the proof tree). If the TT evicts a proven child entry before the pre-exit rebuild, emit_proof_subtree cannot expand that branch. The dump then has a non-terminal leaf and ppv_valid fails, even though the solver actually proved the position.
2. Iterative refinement can overwrite solved entries
   We added a guard in TranspositionTable::store to prevent unsolved bounds from overwriting solved ones, but the very fact that we need such a guard shows the coupling: the dump’s correctness depends on the TT still containing the solved results at the end. Without the guard, a bounded refinement search could overwrite the root’s solved entry with an unsolved (1, 1) bound, and the rebuild would stop immediately.
3. Path-dependent / Kawano twins are not expanded
   emit_proof_subtree uses find_result_for_path on the TT. For cross-path twins that only exist because simulate.rs verified a result from another path, there may be no TT entry matching the current path_code. The rebuild can emit the node but not its subtree, leaving an incomplete branch. That is hard to detect because the node is marked solved.
4. The TT becomes the source of truth for the dump
   In plan6.md the proof tree is supposed to be the independent record of every proven/disproven node. By rebuilding from the TT, the dump is derived from the search table instead. Any inconsistency in the TT (e.g., a solved entry whose best_move is Move::NONE, or a child entry evicted) is inherited by the dump.
5. It discards the event-built tree
   We Clear the worker and re-emit everything. The event stream that dfpn already produced is thrown away. If dfpn had already emitted useful path-specific subtrees, the rebuild does not reuse them; it starts over.
6. It shifts work to the pre-exit hook
   For deep positions the TT walk and re-emission can be large and happens after the timeout or after solve returns. That can delay exit and increases the risk that the pre-exit hook is interrupted before it finishes.
7. It still cannot produce a minimal / merged tree
   The rebuild is path-keyed, so transpositions are duplicated. It does not solve the “transposition merging” open end from report6.md; it just rebuilds a bigger path-keyed tree from a different source.
8. Validation becomes indirect
   ppv_valid: true no longer means “the event-built proof tree is complete”; it means “the TT-based reconstruction succeeded.” Those are not the same thing, and failures become harder to debug because they come from the interaction of two separate systems.

In short, the TT-rebuild gives a correct dump for the reported FEN, but it makes the dump fragile and couples the proof tree to the search table, which is exactly what the proof tree was introduced to avoid.
