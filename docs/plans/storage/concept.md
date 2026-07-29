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
| Trigger | A pluggable `pre_exit_hook` runs whenever the search ends (timeout, `q` + Enter, or normal completion). |
| Build timing | During search, via events from `dfpn`. |
| Concurrency | Dedicated worker thread + `mpsc` queue added in Phase 3 of the testable roadmap. |
| OR-node contents | One selected winning child per Win node (attacker/OR). |
| AND-node contents | All legal replies per Loss node (defender/AND). |
| Duplicate handling | Duplicate per path; each path from the root is a distinct node. |
| Path representation | `ltree` path = `root.<uci1>.<uci2>...` (root sentinel + UCI moves). |
| Dump format | PostgreSQL `.sql` file with `ltree` `COPY` load. No DB driver in the solver. |
| Event payload | `path, uci_move, outcome, depth`. |
| FEN in event | No. The worker does not need the position; it only records events. |
| PPV extraction | From the in-memory proof tree; external `verify_ppv` for validation. |
| Memory | Keep the whole proof tree in memory for the MVP. |
| `--outcome-only` | Optional flag that disables the entire pre-exit hook (no proof-tree output, no dump, no `q`-triggered action, no extra log). |

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
   * Receives `NodeProven` events on an `mpsc` receiver.
   * Stores nodes in a `HashMap<String, ProofNode>` keyed by the full ltree path.
   * Builds child adjacency by deriving `parent_path` from each path.
   * Can be queried by the main thread / `pre_exit_hook` for statistics or the
     full `ProofTree`.

3. **Input thread**
   * Reads line-buffered stdin. When it sees `q` it sets an `Arc<AtomicBool>`
     stop flag; the `pre_exit_hook` handles the actual dump/export.

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

### Pre-exit hook

The CLI uses a single pluggable `pre_exit_hook(reason, ...)` that is invoked
after the search ends, no matter why it ended. The `reason` is one of:

* `Timeout` — the configured deadline was reached.
* `Quit` — the user pressed `q` + Enter.
* `Complete` — the solver finished on its own.

Each MVP phase installs a different hook body:

* Phase 1: log a simple summary.
* Phase 2: dump a small dummy/test tree to a `.sql` file.
* Phase 3: ask the worker for proof-tree statistics and log them.
* Phase 4: dump the real proof tree and extract/log its PPV.
* Phase 5: export the proof tree to Postgres.

`--outcome-only` sets the hook to `None`, so the solver exits exactly as it did
before this feature existed.

### Stop handling

An `Arc<AtomicBool>` stop flag is checked alongside `time_exceeded` inside
`dfpn`. When the input thread sets it, `dfpn` returns as soon as the current
recursion unwinds and the search thread stops. The main thread then invokes the
configured `pre_exit_hook`, waits for the worker if needed, and exits.

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

* New optional flag `--outcome-only`. When present, the pre-exit hook is
  disabled entirely: the solver runs `solve_outcome`, prints the outcome (and
  PV if not `--no-refine-shortest`), and exits. No proof tree, no worker, no
  dump, and no `q` handling.
* New optional flag `--dump-path <FILE>` (default `proof_tree.sql`). Used by
  phases that write a `.sql` dump.
* A background stdin reader watches for `q` + Enter and sets the stop flag.
* When the search ends for any reason (timeout, `q`, or completion) the main
  thread calls the configured `pre_exit_hook`.
* The hook prints/log its result, then the program exits.

## MVP roadmap: sub-features in order

The roadmap is designed so that every phase has a runnable, testable CLI result.
The same `pre_exit_hook` is invoked for `Timeout`, `Quit`, and `Complete`; each
phase swaps in a more capable hook body.

### Phase 1: `--outcome-only` flag and simple pre-exit log
* Add the `--outcome-only` flag and the `pre_exit_hook` machinery.
* The default hook logs a one-line summary when the search ends:
  `pre_exit: reason=Timeout|Quit|Complete outcome=... nodes=...`.
* `--outcome-only` disables the hook completely, restoring the legacy behavior.
* **Test:** run with a short timeout and see the log; press `q` + Enter and see
  the log; run with `--outcome-only` and confirm the log is absent.

### Phase 2: dummy/test SQL `ltree` dump
* Implement the `ProofTree -> .sql` serializer independently of the search.
* The pre-exit hook builds a small hard-coded test `ProofTree` and writes it to
  `--dump-path` (default `proof_tree.sql`).
* **Test:** load the generated `.sql` into Postgres and query the root and a few
  paths; this validates the `ltree` encoding and `COPY` format before the real
  tree is wired up.

### Phase 3: worker thread and proof-tree statistics
* Add the dedicated worker thread and `mpsc` queue.
* Instrument `dfpn` with `move_stack`, `proof_path`, and `in_proof_tree` event
  emission (`NodeProven { path, uci_move, outcome, depth }`).
* The worker builds the real `ProofTree`.
* The pre-exit hook requests statistics from the worker and logs them:
  `proof_tree: nodes=... win=... loss=... root_depth=...`.
* **Test:** run to timeout or press `q` and compare the logged stats to the
  expected proof-tree size for known FENs; check that no events are lost.

### Phase 4: real dump + PPV extraction
* Reuse the Phase-2 serializer to dump the real `ProofTree` on timeout/q.
* Implement `extract_ppv_from_proof_tree` and have the hook log the extracted
  PPV alongside the statistics.
* Validate the PPV with `Search::validate_pv` and the `verify_ppv` example.
* **Test:** regression FENs; load the dumped `.sql` into Postgres; compare the
  extracted PPV to `Search::find_ppv`/`refine_sppv`.

### Phase 5: direct Postgres export
* Add optional direct export of the proof tree to a live Postgres database
  (behind a feature flag or a separate `--pg-url` flag).
* The pre-exit hook inserts nodes into the DB instead of (or in addition to)
  writing a `.sql` file.
* **Test:** run with a Postgres connection string, then query the DB for the
  root, the PPV, and child counts of `Loss` nodes.

## Testability

* **Phase-by-phase CLI tests** that rely on timeout and `q` + Enter to trigger
  the same `pre_exit_hook`. A coding agent can wait for `--timeout` to fire and
  inspect the hook output without driving interactive input.
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
