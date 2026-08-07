# Implementation Plan: Decouple search and proof tree with `ProofEvent`

## Goal

Introduce a neutral `proof_event` protocol module so that `search` no longer
depends on `proof_tree`. The proof-tree worker consumes `ProofEvent` messages
and knows nothing about the search.

The immediate scope is the `ProofEvent` message type and the plumbing around it.
A `ProofSink` trait is intentionally left as a stretch goal for a follow-up plan.

## Scope

- Create `src/proof_event.rs` with a small, dependency-light event schema.
- Replace `NodeProven`/`ProofMessage` usage inside `search/dfpn` with `ProofEvent`.
- Update `proof_tree` to consume `ProofEvent` while keeping the query API
  (`GetStats`/`GetTree`) available through a worker handle.
- Remove UCI path-string construction from `search`; `ProofEvent` carries
  `Vec<Move>` and the worker builds the string keys it already needs.
- Update `main.rs`, tests, examples, `AGENTS.md`, and `README.md`.

## Background and constraints

- `Move` from `atomic-movegen` is `Copy + PartialEq + Eq` but **not** `Hash`
  (verified in `~/.cargo/registry/src/.../atomic-movegen-2.0.0/src/types.rs`).
- `Outcome` derives `Copy + Clone + PartialEq + Eq + Hash`.
- `std::sync::mpsc` has no `select`, so a worker that must receive both search
  events and control queries needs either a single message enum or a multiplex
  loop with `recv_timeout`/`try_recv`.
- `ProofMessage` currently mixes search events (`Clear`, `NodeProven`) with
  worker control (`GetStats`, `GetTree`).

## Architectural decisions

1. **`src/proof_event.rs` owns the search-to-worker contract.**
   - `ProofEvent` has `Clear` and `NodeProven(NodeProven)` variants.
   - `NodeProven` carries `path: Vec<Move>`, `mv: Move`, `outcome: Outcome`,
     `depth: u32`. `mv` is the last move of `path` (or `Move::NONE` for the
     root), so the worker does not need to re-derive it.
   - `proof_event` depends only on `position` (`Outcome`) and `atomic-movegen`
     (`Move`). It does **not** depend on `proof_tree`.

2. **`search` depends only on `proof_event`.**
   - `Search` stores `Option<Sender<ProofEvent>>` and exposes
     `set_proof_event_sender`.
   - `Search` stops importing `crate::proof_tree` entirely.

3. **`proof_tree` consumes `ProofEvent` and keeps its own query protocol.**
   - The worker receives `ProofEvent` on one channel and `ProofTreeWorkerMessage`
     (`GetStats`/`GetTree`) on another.
   - `ProofTreeWorkerHandle` is the public handle returned by `spawn`.
     It exposes `event_sender()`, `stats()`, and `tree()`.
   - `ProofTreeWorkerMessage` and the reply `ProofResponse` stay inside
     `proof_tree`; `search` never sees them.

4. **The worker loop multiplexes the two channels with `recv_timeout`.**
   - `std::sync::mpsc` has no `select`, so the worker uses a short
     `recv_timeout` on the event channel and `try_recv` on the query channel.
   - Trade-off: queries may wait up to ~1 ms and the worker wakes periodically.
   - If this ever becomes a bottleneck, switch to `crossbeam-channel` or a future
     `ProofSink` trait.

5. **`ProofTree` keeps its existing `HashMap<String, usize>` index.**
   - `Move` is not `Hash`, so the worker converts `Vec<Move>` to a UCI string key
     using `notation::moves_to_uci_path` (new helper) when inserting nodes.
   - This moves UCI formatting from `search` to `proof_tree`, which is the side
     that already persists/inspects the tree.

6. **`ProofSink` is a future stretch goal.**
   - Once `ProofEvent` is in place, a `ProofSink` trait can hide the `Sender` from
     `Search` entirely and make unit testing simpler.

## Detailed implementation tasks

### 1. Create `src/proof_event.rs`

```rust
use atomic_movegen::types::Move;
use crate::position::Outcome;

#[derive(Debug, Clone)]
pub enum ProofEvent {
    Clear,
    NodeProven(NodeProven),
}

#[derive(Debug, Clone)]
pub struct NodeProven {
    pub path: Vec<Move>,
    pub mv: Move,
    pub outcome: Outcome,
    pub depth: u32,
}

impl NodeProven {
    pub fn new(path: Vec<Move>, outcome: Outcome, depth: u32) -> Self {
        let mv = path.last().copied().unwrap_or(Move::NONE);
        Self { path, mv, outcome, depth }
    }
}
```

- Add `pub mod proof_event;` to `src/lib.rs`.

### 2. Add `moves_to_uci_path` to `src/notation.rs`

```rust
#[must_use]
pub fn moves_to_uci_path(moves: &[Move]) -> String {
    if moves.is_empty() {
        "root".to_string()
    } else {
        let mut s = "root".to_string();
        for mv in moves {
            s.push('.');
            s.push_str(&move_to_uci(*mv));
        }
        s
    }
}
```

This is the only place that builds UCI path strings from `Vec<Move>`.

### 3. Update `src/proof_tree/mod.rs`

- Remove the `NodeProven` definition; import `NodeProven` and `ProofEvent` from
  `crate::proof_event`.
- Keep `ProofNode`, `ProofTree`, `extract_ppv`, `validate_ppv`, and `to_bin`/`from_bin`.
- Change `ProofTree::add_node` to accept the full UCI path string so the worker
  can index the new node directly:
  ```rust
  pub(crate) fn add_node(
      &mut self,
      parent_id: usize,
      full_path: &str,
      mv: Move,
      outcome: Outcome,
      depth: u32,
  ) -> usize
  ```
- Replace the reverse-lookup `path_for` helper; the caller now supplies the path.
- Update `insert_event`, `attach_child`, and `process_pending` to:
  - Compute the parent path string with `moves_to_uci_path(&event.path[..len-1])`.
  - Compute the child path string with `moves_to_uci_path(&event.path)`.
  - Key pending events by the parent path string.
- Update unit tests in `mod.rs` to pass explicit `root.e2e4...` path strings to
  `add_node`.

### 4. Update `src/proof_tree/worker.rs`

- Import `ProofEvent`/`NodeProven` from `crate::proof_event`.
- Rename the control enum to `ProofTreeWorkerMessage` (private) and keep only
  `GetStats(Sender<ProofResponse>)` and `GetTree(Sender<ProofResponse>)`.
- Introduce `ProofTreeWorkerHandle`:
  ```rust
  #[derive(Clone)]
  pub struct ProofTreeWorkerHandle {
      event_tx: Sender<ProofEvent>,
      query_tx: Sender<ProofTreeWorkerMessage>,
  }

  impl ProofTreeWorkerHandle {
      pub fn spawn(
          root_fen: String,
          pt_size_mb: usize,
          memory_limited: Arc<AtomicBool>,
      ) -> (Self, JoinHandle<()>) { ... }

      pub fn event_sender(&self) -> Sender<ProofEvent> { self.event_tx.clone() }
      pub fn stats(&self) -> ProofStats { ... }
      pub fn tree(&self) -> ProofTree { ... }
  }
  ```
- The internal worker thread owns both an `event_rx: Receiver<ProofEvent>` and a
  `query_rx: Receiver<ProofTreeWorkerMessage>`:
  ```rust
  loop {
      match event_rx.recv_timeout(Duration::from_millis(1)) {
          Ok(ProofEvent::Clear) => worker.clear(),
          Ok(ProofEvent::NodeProven(np)) => worker.process_event(np),
          Err(RecvTimeoutError::Timeout) => {
              while let Ok(query) = query_rx.try_recv() {
                  worker.handle_query(query);
              }
          }
          Err(RecvTimeoutError::Disconnected) => {
              while let Ok(query) = query_rx.try_recv() {
                  worker.handle_query(query);
              }
              break;
          }
      }
  }
  ```
- Update `process_event` to accept `ProofEvent`.
- Update the `pending` bookkeeping to account for `NodeProven.path` now being
  `Vec<Move>`:
  - `pending` keyed by parent path `String`.
  - Memory estimate should include the `Vec<Move>` capacity for pending events.
- Update `ProofTreeWorker::spawn` callers: return `(ProofTreeWorkerHandle, JoinHandle<()>)`.

### 5. Update `src/search/dfpn/mod.rs`

- Replace `proof_tree_sender` with `proof_event_sender`:
  ```rust
  proof_event_sender: Option<Sender<crate::proof_event::ProofEvent>>,
  ```
- Rename `set_proof_tree_sender` to `set_proof_event_sender`.
- `clear_proof_tree` becomes `clear_proof_events` and sends `ProofEvent::Clear`.
- `emit_proof_node` becomes:
  ```rust
  fn emit_proof_node(&self, outcome: Outcome, depth: u32) {
      if let Some(sender) = &self.proof_event_sender {
          let event = ProofEvent::NodeProven(NodeProven::new(
              self.move_stack.clone(),
              outcome,
              depth,
          ));
          let _ = sender.send(event);
      }
  }
  ```
- Remove the `proof_path: String` field from `Search`.
- Update `reset_search_state` to stop resetting `proof_path`.

### 6. Update `src/search/dfpn/core.rs`

- `with_child_path` only pushes/pops `move_stack` (remove `proof_path` string building).
- `emit_proof_node` uses `self.move_stack.clone()`.
- Remove the `move_to_uci` import if it was only used for path strings.

### 7. Update `src/search/dfpn/children.rs`

- For a solved child, build the child path and emit:
  ```rust
  if self.proof_event_sender.is_some() && info.outcome == Some(Outcome::Loss) {
      let mut path = self.move_stack.clone();
      path.push(mv);
      let _ = self.proof_event_sender.as_ref().unwrap().send(
          ProofEvent::NodeProven(NodeProven::new(path, outcome, info.depth))
      );
  }
  ```
- Remove the `move_to_uci` import.

### 8. Update `src/search/dfpn/pv.rs`

- `emit_proof_tree` builds `Vec<Move>` paths and emits `ProofEvent` nodes:
  ```rust
  fn emit_proof_subtree(
      &mut self,
      pos: &mut Position,
      path: &mut Vec<Move>,
      expected: Outcome,
      pv_tail: &[Move],
  ) -> Option<u32>
  ```
- At each node send `NodeProven::new(path.clone(), expected, depth)`.
- For children, push `mv`, recurse, pop.
- Remove the `move_to_uci` import.

### 9. Update `src/main.rs`

- Replace `ProofMessage`/`ProofResponse` usage with `ProofTreeWorkerHandle`:
  ```rust
  use atomic_solver::proof_tree::{ProofTreeWorkerHandle, ProofStats, ProofTree};
  ```
- Spawn the worker:
  ```rust
  let (pt_handle, pt_join) = ProofTreeWorkerHandle::spawn(
      fen.clone(),
      pt_size,
      Arc::clone(&memory_limited),
  );
  ```
- Pass the event sender to the search:
  ```rust
  search.set_proof_event_sender(Some(pt_handle.event_sender()));
  ```
- In the pre-exit hook, query via the handle:
  ```rust
  let stats = pt_handle.stats();
  let tree = pt_handle.tree();
  ```
- Drop the handle before joining the worker thread.

### 10. Update tests

- `src/proof_tree/mod.rs` tests: update `add_node` calls to include path strings.
- `src/proof_tree/worker/tests.rs`: use `ProofEvent`/`NodeProven` with `Vec<Move>`
  paths; query via `ProofTreeWorkerHandle::stats()`/`tree()`.
- `src/search/dfpn/pv.rs` tests (`emit_proof_tree_populates_validate_ppv`): update
  to use `ProofTreeWorkerHandle` and `ProofEvent`.
- Run `cargo test --all-targets` and fix any remaining `NodeProven`/`ProofMessage`
  references in integration tests.

### 11. Update `AGENTS.md`

- Add `src/proof_event.rs` to the architecture list.
- Describe the dependency direction:
  - `search -> proof_event`
  - `proof_tree -> proof_event`
  - `proof_tree` knows nothing about `search`.
- Mention `ProofSink` as a future stretch goal.

### 12. Update `README.md`

- Add `atomic_solver::proof_event` to the public API bullet list.
- Update the "Proof-tree emission" description to say the search emits
  `ProofEvent` nodes and the proof-tree worker builds the in-memory tree.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo test --release
$ cargo doc --no-deps
$ cargo run -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
$ cargo run --example inspect_pt
$ cargo run --example verify_ppv -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1" --moves "f1f7"
```

Specific checks:

- `search` no longer contains `use crate::proof_tree`.
- `search` no longer calls `move_to_uci`.
- `proof_tree` consumes `ProofEvent` from `proof_event`.
- All existing `outcome`/`pv` output is unchanged.
- `proof_tree.bin` round-trip still works.
- All source files remain under ~10 KB (split into submodules if a file exceeds
  the soft limit).

## Risks and trade-offs

- **Two-channel multiplexing.** `std::sync::mpsc` lacks `select`; using
  `recv_timeout` adds a small query latency and periodic wakeups. Acceptable for
  the current CLI; revisit with `crossbeam-channel` or `ProofSink` if needed.
- **`Move` is not `Hash`.** The worker keeps a UCI-string index, which is the
  existing design. String formatting is now done in the worker instead of the
  search, which is the desired separation. A future optimization could use a
  bit-packed `PathKey` derived from `move_to_bits`.
- **`Vec<Move>` clone per proven node.** This replaces the previous UCI-string
  clone and is likely smaller. For extremely high event rates a parent-id-based
  event could avoid the allocation, but that requires the search to manage
  opaque node IDs.
- **`proof_event` depends on `Outcome`.** If the proof tree is externalized later,
  `Outcome` (or `ProofEvent`) must move to a shared crate or become generic.
- **Public API churn.** `set_proof_tree_sender` is renamed and `ProofMessage` is
  no longer part of the search API. `main` and tests must be updated. Examples
  that do not touch the proof tree are unaffected.

## Future work (stretch goals)

1. **Add `ProofSink`.** Define `ProofSink` in `proof_event` so `Search` holds
   `Option<Box<dyn ProofSink>>` and the worker implements it. This removes the
   `Sender` from `Search` entirely and makes unit tests with a `Vec`-collecting
   sink trivial.
2. **Externalize the proof tree.** With `ProofEvent` as a stable protocol, the
   proof-tree worker can become a separate crate or binary without `search`
   knowing it.
3. **Optimize the `ProofTree` index.** If `Move` ever becomes `Hash`/`Ord`, or if
   we introduce a stable `u16` move code, switch the index to
   `HashMap<Vec<Move>, usize>` or a compact bit-packed key.

## Final task

Write `docs/plans/proof_event/report.md` after implementation. Include:

- Which design choices were confirmed or changed during implementation.
- Any problems with the two-channel worker loop (latency, deadlocks, dropped
  senders).
- Test results and performance comparison (node counts, timing, PV length).
- Unresolved parts and missing tests.
- Next steps (`ProofSink`, external proof tree, index optimization).
