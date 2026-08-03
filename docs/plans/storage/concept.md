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
and, on `q` + Enter or completion, writes the tree to a compact binary adjacency
dump. External tools can import the binary dump into PostgreSQL (with or without
an `ltree` path) for analysis.

## Decisions made

| Topic | Decision |
|---|---|
| Trigger | A pluggable `pre_exit_hook` runs whenever the search ends (timeout, `q` + Enter, or normal completion). |
| Build timing | During search, via events from `dfpn`. |
| Concurrency | Dedicated worker thread + `mpsc` queue added in Phase 3 of the testable roadmap. |
| OR-node contents | One selected winning child per Win node (attacker/OR). |
| AND-node contents | All legal replies per Loss node (defender/AND). |
| Duplicate handling | Duplicate per path; each path from the root is a distinct node. |
| Path representation | In the worker, `path` strings (`root.<uci1>.<uci2>...`) are used only to attach out-of-order events. The dump stores parent indices, not materialized paths. |
| Dump format | Compact binary adjacency list (`parent_id: u32`, `move_code: u16`). No DB driver in the solver; an external loader can import to Postgres. |
| Event payload | `path, mv, outcome, depth` (`mv` is `atomic_movegen::types::Move`). |
| FEN in event | No. The worker does not need the position; it only records events. |
| PPV extraction | From the in-memory proof tree; external `verify_ppv` for validation. |
| Memory | Keep the whole proof tree in memory for the MVP; bounded by `--pt-size`. |
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
   * Sends `ProofMessage::NodeProven { path, mv, outcome, depth }` for every
     node on the proof subtree. The `path` string is still built from UCI move
     labels for event ordering, but the stored node carries `mv: Move`.

2. **Proof-tree worker thread** (`src/proof_tree/`)
   * Receives `NodeProven` events on an `mpsc` receiver.
   * Stores nodes in a `Vec<ProofNode>`; maintains a `HashMap<String, usize>`
     mapping `path -> node id`.
   * Buffers child events whose parent has not arrived yet (`pending` map keyed
     by `parent_path`). When the parent event arrives, all buffered children are
     linked in.
   * Updates existing nodes when a `path` is seen again (e.g. a re-proven node or
     a new best child for a `Win` parent). For `Win` nodes the child list is
     replaced by the latest emitted child.
   * Answers query messages (e.g. `GetStats`, `GetTree`) from the main thread /
     `pre_exit_hook` via a reply channel.

3. **Input thread**
   * Reads line-buffered stdin. When it sees `q` it sets an `Arc<AtomicBool>`
     stop flag; the `pre_exit_hook` handles the actual dump/export.
   * If stdin is closed or piped, the thread exits gracefully; the search still
     runs until timeout or completion.

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

Use a `Vec`-backed tree with `usize` indices and a separate `HashMap` for
path-to-id lookup. `ProofNode` stores the actual `Move` value, not a UCI
string:

```rust
use atomic_movegen::types::Move;

pub struct ProofNode {
    pub parent: Option<usize>,
    pub mv: Move,          // move from parent; Move::NONE for root
    pub outcome: Outcome,
    pub depth: u32,
    pub children: Vec<usize>, // child node ids
}

pub struct ProofTree {
    pub root_fen: String,
    pub nodes: Vec<ProofNode>,
    pub index: HashMap<String, usize>, // event path -> node id
}
```

`index` maps the incoming event `path` to a node id so the worker can attach
each child to its parent in `O(1)`. Because DF-PN proves a child before its
parent, child `NodeProven` events can arrive first; the worker keeps a
`pending: HashMap<String, Vec<NodeProven>>` buffer keyed by `parent_path` and
links the children once the parent is inserted.

The `path` strings and the `index` are used only for event ordering. The dump
itself writes `parent` indices and `Move` codes, so no materialized path string
is repeated.

### Event payload

```rust
pub struct NodeProven {
    pub path: String,      // e.g. "root.e2e4.e7e5"
    pub mv: Move,          // move from parent; Move::NONE for root
    pub outcome: Outcome,  // Win or Loss from side to move
    pub depth: u32,        // proven mate/loss distance from this node
}
```

No FEN is sent. The worker records moves; external tools replay the move path
from the root FEN (stored separately in the binary header) when they need a
board.

### Pre-exit hook

The CLI uses a single pluggable `pre_exit_hook(reason, ...)` that is invoked
after the search ends, no matter why it ended. The `reason` is one of:

* `Timeout` — the configured deadline was reached.
* `Quit` — the user pressed `q` + Enter.
* `MemoryLimit` — the proof tree reached `--pt-size`.
* `Complete` — the solver finished on its own.

Each MVP phase installs a different hook body:

* Phase 1: log a simple summary.
* Phase 2: dump a small dummy/test tree (prototype serializer).
* Phase 3: ask the worker for proof-tree statistics and log them.
* Phase 4: dump the real proof tree and extract/log its PPV.
* Phase 4.1: replace the dump format with the compact binary adjacency format.
* Phase 5: export the proof tree to Postgres.

`--outcome-only` sets the hook to `None`, so the solver exits exactly as it did
before this feature existed.

### Worker query protocol

The main thread and search thread send messages to the worker on the same
`mpsc` channel. Queries carry a reply sender:

```rust
pub enum ProofMessage {
    NodeProven(NodeProven),
    GetStats(Sender<ProofResponse>),
    GetTree(Sender<ProofResponse>),
}

pub enum ProofResponse {
    Stats(ProofStats),
    Tree(ProofTree),
}

pub struct ProofStats {
    pub nodes: usize,
    pub win_nodes: usize,
    pub loss_nodes: usize,
    pub root_depth: u32,
}
```

`NodeProven` events are sent from the search thread; `GetStats` / `GetTree` are
sent by the `pre_exit_hook`. The worker keeps a `pending` buffer and replies on
the provided sender.

### Stop handling

An `Arc<AtomicBool>` stop flag is checked alongside `time_exceeded` inside
`dfpn`. The input thread is only spawned when the proof-tree feature is enabled
(not `--outcome-only`). When it sees `q` it sets the flag, `dfpn` returns as
soon as the current recursion unwinds, and the search thread stops. The main
thread then invokes the configured `pre_exit_hook`, waits for the worker if
needed, and exits.

## Binary adjacency dump

### File contents

A compact binary file (default `proof_tree.bin`). The encoder/decoder lives in
`src/proof_tree/binary.rs`. External tools load it and can rebuild an `ltree`
path on import if desired.

```
<8-byte magic> "ATOMTREE"
<1-byte version> 1
<FEN>\n
<root_outcome: u8>    // 0 Draw, 1 Win, 2 Loss
<root_depth: u32 LE>

for each node (root first, topological order):
    parent_id: u32 LE   // u32::MAX for the root
    move_code: u16 LE   // Move::NONE == 0 for the root
```

### Move encoding

`move_code` mirrors `atomic_movegen::types::Move`'s documented bit layout:

* bits 0-5: `to_sq`
* bits 6-11: `from_sq`
* bits 12-13: move type (0 Normal, 1 Promotion, 2 EnPassant, 3 Castling)
* bits 14-15: promotion piece index (0 Queen, 1 Rook, 2 Bishop, 3 Knight)

The `Move` value is encoded and decoded using only public API
(`from_sq`, `to_sq`, `move_type`, `promotion_type`, `Square::from_u8`, and the
`make_*` constructors), so the private `u16` field is never accessed and no
`unsafe` is needed.

### Deriving node metadata

* `outcome` is obtained by parity from the root outcome.
* `depth` is obtained by post-order traversal: terminal nodes have `depth == 0`;
  `Win` nodes are `1 + min(child depths)`; `Loss` nodes are
  `1 + max(child depths)`.
* `uci_move` for display can be produced with `move_to_uci` when the binary is
  loaded.

## PPV extraction from the proof tree

Because the tree contains one child per `Win` node and all children per `Loss`
node, a PPV can be extracted without an explicit `is_principal` flag:

1. Start at `root`.
2. If the node is `Win` (OR), take its only child.
3. If the node is `Loss` (AND), take the child with the largest `depth`
   (longest defense).
4. Append the child's `Move` to the PPV and repeat until `depth == 0`.
5. Convert the collected `Move`s to UCI strings for display or external tools.

The extracted line is validated with `Search::validate_pv` and, for full
confidence, with the existing `verify_ppv` example.

## CLI integration

* New optional flag `--outcome-only`. When present, the pre-exit hook is
  disabled entirely: the solver runs `solve_outcome`, prints the outcome (and
  PV if not `--no-refine-shortest`), and exits. No proof tree, no worker, no
  dump, and no `q` handling.
* New optional flag `--dump-path <FILE>` (default `proof_tree.bin`). Used by
  phases that write a binary proof-tree dump.
* New optional flag `--pt-size <MB>` (default `256`). Hard limit on the
  in-memory proof-tree size. When exceeded, the worker sets the stop flag with
  reason `MemoryLimit`, the search unwinds, and the pre-exit hook is invoked.
* A background stdin reader watches for `q` + Enter and sets the stop flag.
* When the search ends for any reason (timeout, `q`, `MemoryLimit`, or
  completion) the main thread calls the configured `pre_exit_hook`.
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

### Phase 2: prototype proof-tree serializer
* Implement a first standalone `ProofTree` serializer independently of the search.
* The pre-exit hook builds a small hard-coded test `ProofTree` and writes it to
  a dump file to validate serialization before the live worker is wired.
* In `report2.md` this was a PostgreSQL `ltree` `.sql` serializer (`proof_tree.sql`).
  It was later superseded by the compact binary format from Phase 4.1.

### Phase 3: worker thread, proof-tree statistics, and `--pt-size`
* Add the dedicated worker thread and `mpsc` queue.
* Instrument `dfpn` with `move_stack`, `proof_path`, and `in_proof_tree` event
  emission (`NodeProven { path, mv, outcome, depth }`).
* The worker builds the real `ProofTree`.
* Add `--pt-size <MB>` (default `256`). When the worker estimates the tree has
  exceeded this budget, it sets the stop flag with reason `MemoryLimit` and the
  pre-exit hook runs.
* The pre-exit hook requests statistics from the worker and logs them:
  `proof_tree: nodes=... win=... loss=... root_depth=...`.
* **Test:** run to timeout or press `q` and compare the logged stats to the
  expected proof-tree size for known FENs; check that no events are lost; test
  `--pt-size` with a small value and verify `MemoryLimit` is logged.

### Phase 4: real dump + PPV extraction
* Dump the real `ProofTree` on timeout/q.
* Implement `extract_ppv_from_proof_tree` and have the hook log the extracted
  PPV alongside the statistics.
* Validate the PPV with `Search::validate_pv` and the `verify_ppv` example.
* **Test:** regression FENs; compare the extracted PPV to `Search::find_ppv` /
  `refine_sppv`.

### Phase 4.1: compact binary dump
* Replace the `ltree` `.sql` serializer with a compact binary adjacency dump
  (`parent_id: u32`, `move_code: u16`). See `plan4-1.md` for the full format.
* Store `Move` values in `ProofNode` and `NodeProven` instead of UCI strings.
* Update `--dump-path` default to `proof_tree.bin`.
* **Test:** confirm file sizes are far smaller, round-trip tests pass, and the
  extracted PPV is still valid.

### Phase 5: full proof-tree dump
* Emit the entire proven subtree as `NodeProven` events during the
  `extract_ppv_from_proven_subtree` pass, so the compact binary dump contains
  the complete OR-AND proof tree, not just the PPV line.
* `ProofTreeWorker` keeps the shortest child for `Win`/OR parents and all
  distinct children for `Loss`/AND parents.
* PostgreSQL import remains an external-loader/post-MVP option; the binary
  adjacency dump is the stable solver output.
* **Test:** existing PPV-match tests still pass; `proof_tree_contains_defender_replies`
  checks that `Loss` nodes contain all defender replies; manual CLI checks show
  `proof_tree.bin` contains more nodes than `pv.len() + 1`.

## Testability

* **Phase-by-phase CLI tests** that rely on timeout and `q` + Enter to trigger
  the same `pre_exit_hook`. A coding agent can wait for `--timeout` to fire and
  inspect the hook output without driving interactive input.
* **Unit tests** for `ProofTree` insertion, `Move` encoding/decoding, and PPV
  extraction on small forced-mate positions.
* **Integration tests** solving known FENs, dumping to `.bin`, loading the binary
  with an external loader, and verifying the PPV with `verify_ppv`.
* **Property checks** on the in-memory tree:
  * terminal nodes have `depth == 0`,
  * internal `Win` nodes have exactly one `Loss` child,
  * internal `Loss` nodes contain a child for every legal defender reply,
  * depths are monotonically consistent.
* **Regression tests** comparing `extract_ppv_from_proof_tree` to the existing
  `find_ppv` / `refine_sppv` output.

## Risks and pushbacks

* **Minimal event, no FEN.** External consumers replay the move path from the
  root FEN. If per-node FEN becomes necessary, add `fen` to the event.
* **Memory cap.** `--pt-size` bounds the proof tree, but the measurement is
  approximate (Rust `Vec`/`HashMap` heap overhead is not exact). The cap does
  not cover the `mpsc` channel backlog, the transposition table, or the search
  stack.
* **Line-buffered `q`.** The MVP needs `Enter`; true raw single-key support is a
  Phase-5 extension.
* **Partial dumps on `q`.** A stopped search may leave some `Loss` nodes with
  incomplete children. Child events whose parent was never proven stay in the
  worker's `pending` buffer and are not linked into the dumped tree. For the MVP
  this is accepted; post-MVP can add a `complete` flag or orphan-node handling
  if external tools need it.
* **OR child quality.** Building the tree during `solve_outcome` may store an
  arbitrary first winning child. Finalize the tree during `find_ppv` /
  `refine_sppv` for a PPV/SPPV-aligned dump.
* **Duplicate-per-path blowup.** Transpositions are duplicated per tree path.
  This keeps the data model simple but can grow large; the post-MVP can merge
  repeated positions by `(hash, path_code)` if needed.
* **Multiple positions in one database.** The binary file contains one root per
  file. A loader that imports many dumps into a single table must add a
  run/session id to avoid collisions. Post-MVP.

## Open questions / resolved

1. `q` runs the pre-exit hook and then the program exits.
2. `std::sync::mpsc` is sufficient for the MVP; evaluate other channels after.
3. New code lives in `src/proof_tree/` (top-level module) to reflect the
   dedicated-thread decoupling.
4. Partial trees are fine for the MVP; no `complete` boolean.
5. `--pt-size` uses an **approximate byte budget**: `size_of(ProofNode)` +
   the `index` `HashMap` (still keyed by `path` strings) + the `pending` buffer,
   plus a small safety factor. `ProofNode` no longer stores a UCI string.
6. Out-of-order events: the worker buffers child `NodeProven` events until the
   matching parent event arrives. No placeholder nodes; `ProofNode` stays
   `outcome: Outcome` and `depth: u32`, and `NodeProven` carries `mv: Move`.
