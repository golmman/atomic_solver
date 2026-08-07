//! Background worker that collects `NodeProven` events and maintains the
//! in-memory proof tree.
//!
//! Worker-specific tests live in `worker/tests.rs` to keep this file under the
//! 20 KiB soft module-size limit.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};

use crate::position::Outcome;
use crate::proof_tree::{NodeProven, ProofNode, ProofTree};

/// Messages sent to the proof-tree worker.
#[derive(Debug)]
pub enum ProofMessage {
    Clear,
    NodeProven(NodeProven),
    GetStats(Sender<ProofResponse>),
    GetTree(Sender<ProofResponse>),
}

/// Replies from the proof-tree worker.
#[derive(Debug)]
pub enum ProofResponse {
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

/// Background worker that collects `NodeProven` events and maintains the
/// in-memory proof tree.
pub struct ProofTreeWorker {
    tree: ProofTree,
    pending: HashMap<String, Vec<NodeProven>>,
    budget: usize,
    memory_limited: Arc<AtomicBool>,
    // Memory-accounting totals updated incrementally so `estimate_memory` is O(1).
    index_path_bytes: usize,
    pending_path_bytes: usize,
    pending_event_count: usize,
}

impl ProofTreeWorker {
    /// Build a worker for the given memory budget (in bytes).
    ///
    /// This constructor does not spawn a thread, so unit tests can drive the
    /// worker directly via [`ProofTreeWorker::handle_message`].
    pub(crate) fn new(root_fen: String, budget: usize, memory_limited: Arc<AtomicBool>) -> Self {
        let tree = ProofTree::new(root_fen, Outcome::Draw, 0);
        let index_path_bytes = tree.index.keys().map(|k| k.len()).sum();
        Self {
            tree,
            pending: HashMap::new(),
            budget,
            memory_limited,
            index_path_bytes,
            pending_path_bytes: 0,
            pending_event_count: 0,
        }
    }

    /// Spawn a worker thread and return the channel sender and join handle.
    pub fn spawn(
        root_fen: String,
        pt_size_mb: usize,
        memory_limited: Arc<AtomicBool>,
    ) -> (Sender<ProofMessage>, std::thread::JoinHandle<()>) {
        let (tx, rx) = channel();
        let mut worker = Self::new(
            root_fen,
            pt_size_mb.saturating_mul(1024 * 1024),
            memory_limited,
        );
        let handle = std::thread::spawn(move || {
            for msg in rx {
                let _ = worker.handle_message(msg);
            }
        });
        (tx, handle)
    }

    /// Handle a single proof-tree message.
    ///
    /// Returns `Some(response)` for `GetStats`/`GetTree`, `None` otherwise.
    pub(crate) fn handle_message(&mut self, msg: ProofMessage) -> Option<ProofResponse> {
        match msg {
            ProofMessage::Clear => {
                self.clear();
                None
            }
            ProofMessage::NodeProven(event) => {
                self.process_event(event);
                None
            }
            ProofMessage::GetStats(tx) => {
                let stats = self.stats();
                let response = ProofResponse::Stats(stats);
                let _ = tx.send(response);
                Some(ProofResponse::Stats(stats))
            }
            ProofMessage::GetTree(tx) => {
                let tree = self.tree.clone();
                let _ = tx.send(ProofResponse::Tree(tree.clone()));
                Some(ProofResponse::Tree(tree))
            }
        }
    }

    fn clear(&mut self) {
        self.tree = ProofTree::new(self.tree.root_fen.clone(), Outcome::Draw, 0);
        self.pending.clear();
        self.index_path_bytes = self.tree.index.keys().map(|k| k.len()).sum();
        self.pending_path_bytes = 0;
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
        if event.path == "root" {
            self.tree.nodes[0].mv = event.mv;
            self.tree.nodes[0].outcome = event.outcome;
            self.tree.nodes[0].depth = event.depth;
            self.process_pending("root");
            return;
        }

        if let Some((parent_path, _)) = event.path.rsplit_once('.') {
            if let Some(&parent_id) = self.tree.index.get(parent_path)
                && self.tree.nodes[parent_id].outcome != Outcome::Draw
            {
                self.attach_child(parent_id, event);
            } else {
                self.pending_path_bytes += event.path.len();
                self.pending_event_count += 1;
                self.pending
                    .entry(parent_path.to_string())
                    .or_default()
                    .push(event);
            }
        }
    }

    fn attach_child(&mut self, parent_id: usize, event: NodeProven) {
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
                let same_path = self.tree.index.get(&event.path) == Some(&existing_id);
                if same_path {
                    if event.depth < self.tree.nodes[existing_id].depth {
                        self.tree.nodes[existing_id].depth = event.depth;
                    }
                    return;
                }
                if event.depth >= self.tree.nodes[existing_id].depth {
                    return;
                }
            }
            self.tree.nodes[parent_id].children.clear();
        }

        let id = if let Some(&id) = self.tree.index.get(&event.path) {
            self.tree.nodes[id].mv = event.mv;
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
                outcome: event.outcome,
                depth: event.depth,
                children: Vec::new(),
            });
            self.index_path_bytes += event.path.len();
            let path = event.path.clone();
            self.tree.index.insert(event.path, id);
            if !self.tree.nodes[parent_id].children.contains(&id) {
                self.tree.nodes[parent_id].children.push(id);
            }
            self.process_pending(&path);
            return;
        };

        if !self.tree.nodes[parent_id].children.contains(&id) {
            self.tree.nodes[parent_id].children.push(id);
        }

        self.process_pending(&event.path);
    }

    fn process_pending(&mut self, path: &str) {
        if let Some(children) = self.pending.remove(path)
            && let Some(&parent_id) = self.tree.index.get(path)
        {
            for child in &children {
                self.pending_path_bytes -= child.path.len();
                self.pending_event_count -= 1;
            }
            for child in children {
                self.attach_child(parent_id, child);
            }
        }
    }

    fn estimate_memory(&self) -> usize {
        let node_size = std::mem::size_of::<ProofNode>();
        let nodes_mem = self.tree.nodes.capacity() * node_size;
        let index_overhead = self.tree.index.capacity()
            * (std::mem::size_of::<String>() + std::mem::size_of::<usize>() + 8);
        let pending_overhead = self.pending.len() * std::mem::size_of::<Vec<NodeProven>>()
            + self.pending_event_count * std::mem::size_of::<NodeProven>()
            + self.pending_path_bytes;
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
