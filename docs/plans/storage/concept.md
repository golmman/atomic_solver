# Concept: persistent proof-tree storage for `atomic_solver`

## Core problem

The transposition table (TT) is volatile: entries can be overwritten, evicted, or
stale by the time the solver finishes. This makes it hard to

* give an independent proof for a reported outcome,
* reliably extract a Proof Principal Variation (PPV) or Shortest PPV (SPPV),
* inspect or debug the search afterwards.

The idea is to build a separate, dedicated proof tree alongside the search,
persist it on demand, and use it for proof extraction and external analysis.

## Idea in one sentence

During the search the solver emits only the nodes that belong to the final proof
subtree; a background worker thread collects them into an in-memory proof tree
and, on `q` + Enter, writes the tree to a PostgreSQL-compatible `.sql` dump
using an `ltree` path of UCI moves.

## Decisions made

| Topic | Decision |
|---|---|
| Trigger | Interactive `q` + Enter in the CLI; dumps immediately with whatever tree exists so far and then stops. |
| Build timing | During search, via events from `dfpn`. |
| Concurrency | Dedicated worker thread + `mpsc` queue from the start of the MVP. |
| OR-node contents | One selected winning child per Win node (attacker/OR). |
| AND-node contents | All legal replies per Loss node (defender/AND). |
| Duplicate handling | Duplicate per path; each path from the root is a distinct node. |
| Path representation | `ltree` path = `root.<uci1>.<uci2>...` (root sentinel + UCI moves). |
| Dump format | PostgreSQL `.sql` file with `ltree` `COPY` load. No DB driver in the solver. |
| Event payload | `path, uci_move, outcome, depth`. |
| FEN in event | No. The worker does not need the position; it only records events. |
| PPV extraction | From the in-memory proof tree; external `verify_ppv` for validation. |
| Memory | Keep the whole proof tree in memory for the MVP. |
| `--outcome-only` | Optional flag that disables the entire proof-tree feature (no worker, no dump, no `q` handling, no PPV/SPPV dump logic). |

## Node semantics: attacker/defender, OR/AND

The proof tree is described from the point of view of the player trying to prove
the root outcome.

* A node whose **side-to-move outcome is `Win`** is an **OR / attacker node**:
  one winning move is enough. Only the selected winning child is stored.
* A node whose **side-to-move outcome is `Loss`** is an **AND / defender node**:
  every legal reply must lose for the defender. All children are stored.

This maps directly to the DF-PN proof structure and to the user's statement
"for the attacker one OR-node suffices, for the defender all AND-nodes need to
be proven/disproven".

## Architecture

### Three threads

1. **Search thread** (`src/search/dfpn/`)
   * Maintains a `move_stack` parallel to `path_stack` and an incremental
     `proof_path` string (e.g. `root.e2e4.e7e5`).
   * Carries an `in_proof_tree` flag through `dfpn` calls.
   * Emits a `ProofEvent::NodeProven { path, uci_move, outcome, depth }` for
     every node on the proof subtree.

2. **Proof-tree worker thread** (`src/search/proof_tree/`)
   * Receives events on an `mpsc` receiver.
   * Stores nodes in a `HashMap<String, ProofNode>` keyed by the full ltree path.
   * Builds child adjacency by deriving `parent_path` from each path.
   * On a `Dump(path)` command, serializes the tree to `.sql`.

3. **Input thread**
   * Reads line-buffered stdin. When it sees `q` it sets an `AtomicBool` stop
     flag and sends `Dump` to the worker.

### Search integration

`dfpn` is extended with an `in_proof_tree` indication. A recursive child call is
in the proof tree when:

* the parent is a `Loss` node (all children belong to the proof), or
* the parent is a `Win` node and this child is the selected `best_move`.

When a node is proven with `in_proof_tree == true`, the solver emits
`NodeProven` using the current incremental path.

> **Important:** `solve_outcome` often stops at the first winning child, so its
> OR edges may be arbitrary. For a meaningful PPV, the proof tree should be
> built or finalized during `find_ppv` / `refine_sppv`, where `best_move` reflects
> the chosen principal variation. If the tree is accumulated across all stages,
> reset it before the final PPV/SPPV pass.

### In-memory proof tree

```rust
pub struct ProofNode {
    pub uci_move: String,
    pub outcome: Outcome,
    pub depth: u32,
    pub children: Vec<String>, // child paths
}

pub struct ProofTree {
    pub root_fen: String,
    pub nodes: HashMap<String, ProofNode>,
}
```

`path` is the primary key. `parent_path` is computed by removing the last ltree
label. The worker inserts nodes as they arrive, creating parent placeholders if
necessary.

### Event payload

```rust
pub struct NodeProven {
    pub path: String,      // e.g. "root.e2e4.e7e5"
    pub uci_move: String,  // move from parent; empty for root
    pub outcome: Outcome,  // Win or Loss from side to move
    pub depth: u32,        // proven mate/loss distance from this node
}
```

No FEN is sent. The worker records; external tools replay the UCI path from the
root FEN (stored separately in `proof_meta`) when they need a board.

### Stop handling

A `Arc<AtomicBool>` is checked alongside `time_exceeded` inside `dfpn`. When the
input thread sets it, `dfpn` returns as soon as the current recursion unwinds,
the search thread stops sending events, and the main thread tells the worker to
dump and exit.

## SQL / `ltree` dump

### File contents

A plain `.sql` file that can be loaded with `psql < proof_tree.sql`:

```sql
CREATE EXTENSION IF NOT EXISTS ltree;

CREATE TABLE proof_meta (
    key text PRIMARY KEY,
    value text
);

CREATE TABLE proof_nodes (
    path ltree PRIMARY KEY,
    parent_path ltree,
    uci_move text,
    outcome text CHECK (outcome IN ('Win', 'Loss')),
    depth int,
    terminal boolean
);

CREATE INDEX idx_proof_nodes_parent ON proof_nodes USING btree (parent_path);
CREATE INDEX idx_proof_nodes_path ON proof_nodes USING gist (path);

INSERT INTO proof_meta (key, value) VALUES ('root_fen', '<FEN>');

COPY proof_nodes (path, parent_path, uci_move, outcome, depth, terminal)
FROM STDIN;
root			Win	7	false
root.e2e4	e2e4	Loss	6	false
root.e2e4.e7e5	e7e5	Win	5	false
...\.
```

### Path encoding

* Root uses the sentinel label `root` because `ltree` does not allow empty paths.
* Each subsequent label is one UCI move, lowercased.
* PostgreSQL `ltree` labels allow alphanumeric characters, underscores, and
  hyphens and are at most 1000 bytes long. A UCI move such as `e7e8q` satisfies
  this; any unexpected character is sanitized or rejected during export.
* A single ltree path can contain up to 65,535 labels, far above any realistic
  atomic-chess PPV.

## PPV extraction from the proof tree

Because the tree contains one child per `Win` node and all children per `Loss`
node, a PPV can be extracted without an explicit `is_principal` flag:

1. Start at `root`.
2. If the node is `Win` (OR), take its only child.
3. If the node is `Loss` (AND), take the child with the largest `depth`
   (longest defense).
4. Append the child's `uci_move` to the PPV and repeat until `depth == 0`.

The extracted line is validated with `Search::validate_pv` and, for full
confidence, with the existing `verify_ppv` example.

## CLI integration

* New optional flag `--outcome-only`. When present, the solver behaves exactly
  as it did before the proof-tree feature existed: it runs `solve_outcome`,
  prints the outcome, and exits. No proof tree is built, no worker thread is
  started, `q` does nothing, and no dump is produced. It takes precedence over
  `--dump-path` and implies `--no-refine-shortest` for the dump-related PPV
  logic.
* New optional flag `--dump-path <FILE>` (default `proof_tree.sql`).
* When `--dump-path` is present and `--outcome-only` is absent, the solver
  starts the worker thread and the stdin reader before solving.
* Press `q` + Enter to stop the search and write the dump.
* The program prints the dump path and exits.

## MVP roadmap: sub-features in order

### Phase 1: core proof-tree emission (synchronous)
* Add `ProofEvent`, `ProofNode`, `ProofTree`.
* Add `move_stack` and incremental `proof_path` to `Search`.
* Add `in_proof_tree` propagation to `dfpn` and emit `NodeProven` events into a
  temporary in-memory collector.
* Implement basic shape checks:
  * `Win` nodes have exactly one child with `outcome == Loss`.
  * `Loss` nodes have children for every legal reply, all with `outcome == Win`.
  * Depths satisfy `parent.depth == 1 + max(child.depth)` for `Loss` and
    `parent.depth == 1 + min(child.depth)` for `Win`.

### Phase 2: dedicated worker thread and `mpsc` queue
* Move `ProofTree` into a worker thread.
* `Search` sends `NodeProven` events over `std::sync::mpsc`.
* Add `Dump` command handling.
* Test that no events are lost, that partial trees after a timeout are
  consistent, and that the search thread is not blocked by the worker.

### Phase 3: `q` trigger and SQL/`ltree` dump
* Spawn a stdin-reading thread that reacts to `q` + Enter.
* Wire the stop flag into `dfpn`.
* Serialize the tree to a `.sql` dump with `COPY`.
* Test by loading the dump into Postgres and running simple `ltree` queries.

### Phase 4: PPV extraction and plausibility tests
* Implement `extract_ppv_from_proof_tree`.
* Compare its output to `Search::find_ppv` and `refine_sppv`.
* Run `verify_ppv` on extracted PPVs for a regression suite of FENs.
* Add plausibility checks for depth consistency and AND-node completeness.

### Phase 5: extensions (post-MVP)
* Bounded `mpsc`, backpressure, and optional memory cap / spill-to-disk.
* Richer schema: per-node FEN, `work`, generation, `is_principal` edge flag.
* Direct PostgreSQL export behind a Cargo feature.
* True single-key `q` (e.g. via `crossterm`) or signal triggers.
* Repeated snapshots without stopping the search.

## Testability

* **Unit tests** for `ProofTree` insertion, path parsing, and PPV extraction on
  small forced-mate positions.
* **Integration tests** solving known FENs, dumping, loading into Postgres, and
  verifying the PPV with `verify_ppv`.
* **Property checks** on the in-memory tree:
  * terminal nodes have `depth == 0`,
  * internal `Win` nodes have exactly one `Loss` child,
  * internal `Loss` nodes contain a child for every legal defender reply,
  * depths are monotonically consistent.
* **Regression tests** comparing `extract_ppv_from_proof_tree` to the existing
  `find_ppv` / `refine_sppv` output.

## Risks and pushbacks

* **Minimal event, no FEN.** External consumers replay the UCI path from the
  root FEN. If per-node FEN becomes necessary, add `fen` to the event.
* **Unbounded memory.** The MVP keeps all proof-tree nodes in RAM. Deep or
  highly transpositional positions may require a cap or spill-to-disk later.
* **Line-buffered `q`.** The MVP needs `Enter`; true raw single-key support is a
  Phase-5 extension.
* **Partial dumps on `q`.** A stopped search may leave some `Loss` nodes with
  incomplete children. Add a `complete` boolean column to make this explicit.
* **OR child quality.** Building the tree during `solve_outcome` may store an
  arbitrary first winning child. Finalize the tree during `find_ppv` /
  `refine_sppv` for a PPV/SPPV-aligned dump.
* **Duplicate-per-path blowup.** Transpositions are duplicated. This keeps the
  `ltree` model simple but can grow large; the post-MVP can merge by
  `(hash, path_code)` if needed.

## Open questions for implementation

1. Should the `q` dump stop the program or only write a snapshot and let the
   search continue? This concept assumes stop for the MVP.
2. Is `std::sync::mpsc` sufficient, or should `crossbeam-channel` be evaluated
   once profiling data is available?
3. Should the new code live in `src/search/proof_tree/` or as a top-level
   module?
4. Should the dump include a `complete` boolean per node for partial trees?
