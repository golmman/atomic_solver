//! Background worker that collects `ProofEvent` messages and maintains the
//! in-memory proof tree.
//!
//! This module is intentionally larger than 10 KiB because the worker state
//! machine, the public `ProofTreeWorkerHandle`, and the query protocol are
//! tightly coupled; splitting them would add cross-module boilerplate without
//! improving readability. Worker-specific tests live in `worker/tests.rs` to
//! keep this file under the 20 KiB soft module-size limit.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Duration;

use atomic_movegen::types::Move;

use crate::notation::moves_to_uci_path;
use crate::position::Outcome;
use crate::proof_event::{NodeProven, ProofEvent};

use super::{ProofNode, ProofTree};

/// Control messages sent to the proof-tree worker.
#[derive(Debug)]
enum ProofTreeWorkerMessage {
    GetStats(Sender<ProofResponse>),
    GetTree(Sender<ProofResponse>),
    Finalize,
}

/// Replies from the proof-tree worker.
#[derive(Debug)]
enum ProofResponse {
    Stats(ProofStats),
    Tree(ProofTree),
}

/// Simple statistics over the in-memory proof tree.
#[derive(Debug, Clone, Copy)]
pub struct ProofStats {
    pub nodes: usize,
    pub win_nodes: usize,
    pub loss_nodes: usize,
    pub root_depth: u32,
}

/// Public handle to a spawned proof-tree worker.
#[derive(Clone)]
pub struct ProofTreeWorkerHandle {
    event_tx: Sender<ProofEvent>,
    query_tx: Sender<ProofTreeWorkerMessage>,
}

impl ProofTreeWorkerHandle {
    /// Spawn a worker thread with the given memory budget and return a handle.
    pub fn spawn(
        root_fen: String,
        pt_size_mb: usize,
        memory_limited: Arc<AtomicBool>,
    ) -> (Self, std::thread::JoinHandle<()>) {
        let (event_tx, event_rx) = channel();
        let (query_tx, query_rx) = channel();
        let mut worker = ProofTreeWorker::new(
            root_fen,
            pt_size_mb.saturating_mul(1024 * 1024),
            memory_limited,
        );
        let handle = std::thread::spawn(move || worker.run(event_rx, query_rx));
        (Self { event_tx, query_tx }, handle)
    }

    /// Return a clone of the event sender for use by the search.
    #[must_use]
    pub fn event_sender(&self) -> Sender<ProofEvent> {
        self.event_tx.clone()
    }

    /// Request statistics from the worker.
    pub fn stats(&self) -> ProofStats {
        let (tx, rx) = channel();
        self.query_tx
            .send(ProofTreeWorkerMessage::GetStats(tx))
            .expect("worker thread alive");
        match rx.recv().expect("worker response") {
            ProofResponse::Stats(s) => s,
            _ => panic!("expected Stats response"),
        }
    }

    /// Request a clone of the in-memory proof tree from the worker.
    pub fn tree(&self) -> ProofTree {
        let (tx, rx) = channel();
        self.query_tx
            .send(ProofTreeWorkerMessage::GetTree(tx))
            .expect("worker thread alive");
        match rx.recv().expect("worker response") {
            ProofResponse::Tree(t) => t,
            _ => panic!("expected Tree response"),
        }
    }

    /// Trigger the post-search finalization pass in the worker thread.
    pub fn finalize(&self) {
        self.query_tx
            .send(ProofTreeWorkerMessage::Finalize)
            .expect("worker thread alive");
    }
}

/// Background worker that collects `ProofEvent` messages and maintains the
/// in-memory proof tree.
pub(crate) struct ProofTreeWorker {
    tree: ProofTree,
    pending: HashMap<String, Vec<NodeProven>>,
    expanded_by_hash: HashMap<(u64, Outcome), Vec<usize>>,
    budget: usize,
    memory_limited: Arc<AtomicBool>,
    // Memory-accounting totals updated incrementally so `estimate_memory` is O(1).
    index_path_bytes: usize,
    pending_path_bytes: usize,
    pending_move_bytes: usize,
    pending_event_count: usize,
}

impl ProofTreeWorker {
    /// Build a worker for the given memory budget (in bytes).
    pub(crate) fn new(root_fen: String, budget: usize, memory_limited: Arc<AtomicBool>) -> Self {
        let tree = ProofTree::new(root_fen, 0, Outcome::Draw, 0);
        let index_path_bytes = tree.index.keys().map(|k| k.len()).sum();
        Self {
            tree,
            pending: HashMap::new(),
            expanded_by_hash: HashMap::new(),
            budget,
            memory_limited,
            index_path_bytes,
            pending_path_bytes: 0,
            pending_move_bytes: 0,
            pending_event_count: 0,
        }
    }

    /// Run the worker loop, consuming events and handling queries.
    fn run(&mut self, event_rx: Receiver<ProofEvent>, query_rx: Receiver<ProofTreeWorkerMessage>) {
        loop {
            match event_rx.recv_timeout(Duration::from_millis(1)) {
                Ok(event) => self.handle_event(event),
                Err(RecvTimeoutError::Timeout) => {
                    while let Ok(query) = query_rx.try_recv() {
                        self.handle_query(query, Some(&event_rx));
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    while let Ok(query) = query_rx.try_recv() {
                        self.handle_query(query, Some(&event_rx));
                    }
                    break;
                }
            }
        }
    }

    /// Handle a single proof-tree event.
    fn handle_event(&mut self, event: ProofEvent) {
        match event {
            ProofEvent::Clear => self.clear(),
            ProofEvent::NodeProven(np) => self.process_event(np),
        }
    }

    /// Handle a single query message, sending a reply if one is produced.
    fn handle_query(
        &mut self,
        msg: ProofTreeWorkerMessage,
        event_rx: Option<&Receiver<ProofEvent>>,
    ) {
        match msg {
            ProofTreeWorkerMessage::GetStats(tx) => {
                let _ = tx.send(ProofResponse::Stats(self.stats()));
            }
            ProofTreeWorkerMessage::GetTree(tx) => {
                let _ = tx.send(ProofResponse::Tree(self.tree.clone()));
            }
            ProofTreeWorkerMessage::Finalize => {
                self.finalize_tree(event_rx);
            }
        }
    }

    fn clear(&mut self) {
        self.tree = ProofTree::new(self.tree.root_fen.clone(), 0, Outcome::Draw, 0);
        self.pending.clear();
        self.expanded_by_hash.clear();
        self.index_path_bytes = self.tree.index.keys().map(|k| k.len()).sum();
        self.pending_path_bytes = 0;
        self.pending_move_bytes = 0;
        self.pending_event_count = 0;
    }

    fn process_event(&mut self, event: NodeProven) {
        if self.memory_limited.load(Ordering::Acquire) {
            return;
        }
        self.insert_event(event);
        if self.estimate_memory() > self.budget {
            self.memory_limited.store(true, Ordering::Release);
        }
    }

    fn insert_event(&mut self, event: NodeProven) {
        if event.path.is_empty() {
            self.tree.nodes[0].mv = event.mv;
            self.tree.nodes[0].hash = event.hash;
            self.tree.nodes[0].outcome = event.outcome;
            self.tree.nodes[0].depth = event.depth;
            self.insert_expanded(0);
            self.process_pending("root");
            return;
        }

        let parent_path = moves_to_uci_path(&event.path[..event.path.len() - 1]);
        if let Some(&parent_id) = self.tree.index.get(&parent_path)
            && self.tree.nodes[parent_id].outcome != Outcome::Draw
        {
            let child_path = moves_to_uci_path(&event.path);
            self.attach_child(parent_id, event, &child_path);
        } else {
            self.pending_path_bytes += parent_path.len();
            self.pending_move_bytes += event.path.capacity() * std::mem::size_of::<Move>();
            self.pending_event_count += 1;
            self.pending.entry(parent_path).or_default().push(event);
        }
    }

    fn attach_child(&mut self, parent_id: usize, event: NodeProven, full_path: &str) {
        let parent_outcome = self.tree.nodes[parent_id].outcome;
        let valid = match parent_outcome {
            Outcome::Win => event.outcome == Outcome::Loss,
            Outcome::Loss => event.outcome == Outcome::Win,
            Outcome::Draw => false,
        };
        if !valid {
            return;
        }

        if parent_outcome == Outcome::Win {
            if let Some(&existing_id) = self.tree.nodes[parent_id].children.first() {
                let same_path = self.tree.index.get(full_path) == Some(&existing_id);
                if same_path {
                    if event.depth < self.tree.nodes[existing_id].depth {
                        self.tree.nodes[existing_id].depth = event.depth;
                    }
                    self.insert_expanded(existing_id);
                    return;
                }
                if event.depth >= self.tree.nodes[existing_id].depth {
                    return;
                }
            }
            self.tree.nodes[parent_id].children.clear();
        }

        let id = if let Some(&id) = self.tree.index.get(full_path) {
            self.tree.nodes[id].mv = event.mv;
            self.tree.nodes[id].hash = event.hash;
            self.tree.nodes[id].outcome = event.outcome;
            if event.depth < self.tree.nodes[id].depth {
                self.tree.nodes[id].depth = event.depth;
            }
            id
        } else {
            let id = self.tree.nodes.len();
            self.tree.nodes.push(ProofNode {
                parent: Some(parent_id),
                mv: event.mv,
                hash: event.hash,
                outcome: event.outcome,
                depth: event.depth,
                children: Vec::new(),
            });
            self.index_path_bytes += full_path.len();
            self.tree.index.insert(full_path.to_string(), id);
            if !self.tree.nodes[parent_id].children.contains(&id) {
                self.tree.nodes[parent_id].children.push(id);
            }
            self.insert_expanded(parent_id);
            self.insert_expanded(id);
            self.process_pending(full_path);
            return;
        };

        if !self.tree.nodes[parent_id].children.contains(&id) {
            self.tree.nodes[parent_id].children.push(id);
        }

        self.insert_expanded(parent_id);
        self.insert_expanded(id);

        self.process_pending(full_path);
    }

    fn process_pending(&mut self, path: &str) {
        if let Some(children) = self.pending.remove(path)
            && let Some(&parent_id) = self.tree.index.get(path)
        {
            self.pending_path_bytes -= path.len();
            for child in children {
                self.pending_move_bytes -= child.path.capacity() * std::mem::size_of::<Move>();
                self.pending_event_count -= 1;
                let child_path = moves_to_uci_path(&child.path);
                self.attach_child(parent_id, child, &child_path);
            }
        }
    }

    /// Mark `id` as an expanded node if it is a terminal leaf or has children.
    /// Expanded nodes are indexed by `(hash, outcome)` for the finalization pass.
    fn insert_expanded(&mut self, id: usize) {
        let (hash, outcome, depth, has_children) = {
            let node = &self.tree.nodes[id];
            (
                node.hash,
                node.outcome,
                node.depth,
                !node.children.is_empty(),
            )
        };
        if depth == 0 || has_children {
            let key = (hash, outcome);
            let present = self
                .expanded_by_hash
                .get(&key)
                .is_some_and(|v| v.contains(&id));
            if !present {
                self.expanded_by_hash.entry(key).or_default().push(id);
            }
        }
    }

    /// Attach any pending children whose parents now exist in the tree.
    /// Repeated until no more pending entries can be resolved.
    fn flush_pending(&mut self) {
        loop {
            let keys: Vec<String> = self.pending.keys().cloned().collect();
            let mut progress = false;
            for path in keys {
                if self.tree.index.contains_key(&path) {
                    self.process_pending(&path);
                    progress = true;
                }
            }
            if !progress {
                break;
            }
        }
    }

    /// Finalize the proof tree after search has stopped.
    ///
    /// This drains any remaining events, flushes pending children, selects a
    /// canonical expanded node for every `(hash, outcome)` key, and rebuilds a
    /// complete proof tree by copying canonical subtrees onto unexpanded
    /// occurrences. If the final tree contains an unexpanded internal node,
    /// the process logs an error and exits.
    fn finalize_tree(&mut self, event_rx: Option<&Receiver<ProofEvent>>) {
        // Drain any remaining events.
        if let Some(rx) = event_rx {
            while let Ok(event) = rx.try_recv() {
                self.handle_event(event);
            }
        }

        // Flush pending children whose parents have arrived.
        self.flush_pending();

        // Select the canonical expanded node for each (hash, outcome).
        let mut canonical: HashMap<(u64, Outcome), usize> =
            HashMap::with_capacity(self.expanded_by_hash.len());
        for (&key, ids) in &self.expanded_by_hash {
            let mut best_id = None;
            let mut best_consistent = false;
            let mut best_depth = u32::MAX;
            for &id in ids {
                let node = &self.tree.nodes[id];
                let implied = if node.depth == 0 {
                    0
                } else {
                    let child_depths: Vec<u32> = node
                        .children
                        .iter()
                        .map(|&c| self.tree.nodes[c].depth)
                        .collect();
                    match node.outcome {
                        Outcome::Win => child_depths
                            .iter()
                            .min()
                            .copied()
                            .unwrap_or(0)
                            .saturating_add(1),
                        Outcome::Loss => child_depths
                            .iter()
                            .max()
                            .copied()
                            .unwrap_or(0)
                            .saturating_add(1),
                        Outcome::Draw => 0,
                    }
                };
                let consistent = node.depth == implied;
                let better = if consistent != best_consistent {
                    consistent
                } else {
                    node.depth < best_depth
                };
                if best_id.is_none() || better {
                    best_id = Some(id);
                    best_consistent = consistent;
                    best_depth = node.depth;
                }
            }
            if let Some(id) = best_id {
                canonical.insert(key, id);
            }
        }

        // Rebuild the tree from the root.
        let root_fen = self.tree.root_fen.clone();
        let root_hash = self.tree.nodes[0].hash;
        let root_outcome = self.tree.nodes[0].outcome;
        let root_depth = self.tree.nodes[0].depth;
        let old_tree = std::mem::replace(
            &mut self.tree,
            ProofTree::new(root_fen.clone(), root_hash, root_outcome, root_depth),
        );
        let mut new_tree = ProofTree::new(root_fen, root_hash, root_outcome, root_depth);

        enum Action {
            Enter(usize, usize, Move), // raw_id, parent_new_id, edge_mv
            Exit,
        }

        let mut actions: Vec<Action> = Vec::new();
        let mut path: Vec<Move> = Vec::new();
        let mut path_hashes: HashSet<u64> = HashSet::new();
        let mut path_hash_stack: Vec<u64> = Vec::new();
        actions.push(Action::Enter(0, 0, Move::NONE));

        while let Some(action) = actions.pop() {
            match action {
                Action::Enter(raw_id, parent_new_id, edge_mv) => {
                    let node = &old_tree.nodes[raw_id];
                    let eff_id = canonical
                        .get(&(node.hash, node.outcome))
                        .copied()
                        .unwrap_or(raw_id);
                    let eff_node = &old_tree.nodes[eff_id];
                    let hash = eff_node.hash;
                    let outcome = eff_node.outcome;
                    let depth = eff_node.depth;

                    let is_root = edge_mv == Move::NONE && parent_new_id == 0;

                    // Cycle guard: if this hash already appears on the current
                    // path, create a leaf and stop expanding this branch.
                    if !is_root && path_hashes.contains(&hash) {
                        path.push(edge_mv);
                        let full_path = moves_to_uci_path(&path);
                        let _ = new_tree.add_node(
                            parent_new_id,
                            &full_path,
                            edge_mv,
                            hash,
                            outcome,
                            depth,
                        );
                        path.pop();
                        continue;
                    }

                    let new_id = if is_root {
                        new_tree.nodes[0].hash = hash;
                        new_tree.nodes[0].outcome = outcome;
                        new_tree.nodes[0].depth = depth;
                        0
                    } else {
                        path.push(edge_mv);
                        path_hashes.insert(hash);
                        path_hash_stack.push(hash);
                        let full_path = moves_to_uci_path(&path);
                        new_tree.add_node(parent_new_id, &full_path, edge_mv, hash, outcome, depth)
                    };

                    actions.push(Action::Exit);
                    for &child_id in eff_node.children.iter().rev() {
                        let child_mv = old_tree.nodes[child_id].mv;
                        actions.push(Action::Enter(child_id, new_id, child_mv));
                    }
                }
                Action::Exit => {
                    if let Some(h) = path_hash_stack.pop() {
                        path_hashes.remove(&h);
                    }
                    path.pop();
                }
            }
        }

        // Recompute proven depths from the leaves up.
        for i in (0..new_tree.nodes.len()).rev() {
            if new_tree.nodes[i].children.is_empty() {
                new_tree.nodes[i].depth = 0;
            } else {
                let child_depths: Vec<u32> = new_tree.nodes[i]
                    .children
                    .iter()
                    .map(|&c| new_tree.nodes[c].depth)
                    .collect();
                new_tree.nodes[i].depth = match new_tree.nodes[i].outcome {
                    Outcome::Win => child_depths
                        .iter()
                        .min()
                        .copied()
                        .unwrap_or(0)
                        .saturating_add(1),
                    Outcome::Loss => child_depths
                        .iter()
                        .max()
                        .copied()
                        .unwrap_or(0)
                        .saturating_add(1),
                    Outcome::Draw => 0,
                };
            }
        }

        // Any non-terminal node without children violates the assumption that
        // every proven node has an expanded twin.
        for (id, node) in new_tree.nodes.iter().enumerate() {
            if node.outcome != Outcome::Draw && node.depth != 0 && node.children.is_empty() {
                eprintln!(
                    "error: proof-tree finalization left unexpanded internal node id={} outcome={:?} depth={}",
                    id, node.outcome, node.depth
                );
                std::process::exit(1);
            }
        }

        self.tree = new_tree;
        self.expanded_by_hash.clear();
    }

    fn estimate_memory(&self) -> usize {
        let node_size = std::mem::size_of::<ProofNode>();
        let nodes_mem = self.tree.nodes.capacity() * node_size;
        let index_overhead = self.tree.index.capacity()
            * (std::mem::size_of::<String>() + std::mem::size_of::<usize>() + 8);
        let pending_overhead = self.pending.len() * std::mem::size_of::<Vec<NodeProven>>()
            + self.pending_event_count * std::mem::size_of::<NodeProven>()
            + self.pending_path_bytes
            + self.pending_move_bytes;
        let total = nodes_mem + index_overhead + self.index_path_bytes + pending_overhead;
        (total as f64 * 1.5) as usize
    }

    fn stats(&self) -> ProofStats {
        let nodes = self.tree.nodes.len();
        let win_nodes = self
            .tree
            .nodes
            .iter()
            .filter(|n| n.outcome == Outcome::Win)
            .count();
        let loss_nodes = self
            .tree
            .nodes
            .iter()
            .filter(|n| n.outcome == Outcome::Loss)
            .count();
        let root_depth = self.tree.nodes.first().map(|n| n.depth).unwrap_or(0);
        ProofStats {
            nodes,
            win_nodes,
            loss_nodes,
            root_depth,
        }
    }
}

#[cfg(test)]
mod tests;
