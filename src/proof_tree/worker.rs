//! Background worker that collects `ProofEvent` messages and maintains the
//! in-memory proof tree.
//!
//! This module is intentionally larger than 10 KiB because the worker state
//! machine, the public `ProofTreeWorkerHandle`, the query protocol, the
//! link-list child bookkeeping, and the global child-move index are tightly
//! coupled; splitting them would add cross-module boilerplate without
//! improving readability. Worker-specific tests live in `worker/tests.rs` to
//! keep this file under the 20 KiB soft module-size limit.
//!
//! The worker builds the tree by traversing each event's `Vec<Move>` path from
//! the root. Missing ancestors are created as dummy nodes with `outcome: None`
//! and are realized when their own `NodeProven` event arrives. The final
//! `finalize()` pass removes any remaining dummy subtrees and rebuilds the
//! canonical proven tree.

use std::collections::{HashMap, HashSet};
use std::io;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Duration;

use atomic_movegen::types::Move;

use crate::position::Outcome;
use crate::proof_event::{NodeProven, ProofEvent};

use super::binary::move_to_bits;
use super::{ProofNode, ProofTree};

/// Control messages sent to the proof-tree worker.
#[derive(Debug)]
enum ProofTreeWorkerMessage {
    GetStats(Sender<ProofResponse>),
    GetTree(Sender<ProofResponse>),
    Finalize,
    DumpToBin {
        path: String,
        tx: Sender<io::Result<()>>,
    },
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

    /// Serialize the finalized proof tree to a compact binary dump file.
    pub fn dump_to_bin<P: AsRef<std::path::Path>>(&self, path: P) -> io::Result<()> {
        let (tx, rx) = channel();
        self.query_tx
            .send(ProofTreeWorkerMessage::DumpToBin {
                path: path.as_ref().to_string_lossy().into_owned(),
                tx,
            })
            .expect("worker thread alive");
        rx.recv().expect("worker response")
    }
}

/// Background worker that collects `ProofEvent` messages and maintains the
/// in-memory proof tree.
pub(crate) struct ProofTreeWorker {
    tree: ProofTree,
    /// Global `(parent_id << 32) | move_bits -> child_id` index used to
    /// traverse event paths in O(1) per ply without scanning a child list.
    child_index: HashMap<u64, u32>,
    /// Expanded (terminal or internal) nodes indexed by `(hash, outcome)` for
    /// the final canonicalization pass. One canonical id is kept per key.
    expanded_by_hash: HashMap<(u64, Outcome), usize>,
    budget: usize,
    memory_limited: Arc<AtomicBool>,
}

impl ProofTreeWorker {
    /// Build a worker for the given memory budget (in bytes).
    pub(crate) fn new(root_fen: String, budget: usize, memory_limited: Arc<AtomicBool>) -> Self {
        Self {
            tree: ProofTree::new(root_fen, 0, None, 0),
            child_index: HashMap::new(),
            expanded_by_hash: HashMap::new(),
            budget,
            memory_limited,
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
            ProofTreeWorkerMessage::DumpToBin { path, tx } => {
                let result =
                    std::fs::File::create(&path).and_then(|mut file| self.tree.to_bin(&mut file));
                let _ = tx.send(result);
            }
        }
    }

    fn clear(&mut self) {
        self.tree = ProofTree::new(self.tree.root_fen.clone(), 0, None, 0);
        self.child_index.clear();
        self.expanded_by_hash.clear();
    }

    fn process_event(&mut self, event: NodeProven) {
        if self.memory_limited.load(Ordering::Acquire) {
            return;
        }

        let id = self.find_or_create_node(&event.path);
        let was_dummy = self.tree.nodes[id as usize].outcome.is_none();
        self.apply_event(id as usize, &event);
        if was_dummy {
            self.reconcile_children(id as usize);
        }

        if !event.path.is_empty() {
            let parent_path = &event.path[..event.path.len() - 1];
            let parent_id = self.find_or_create_node(parent_path);
            if self.tree.nodes[parent_id as usize].outcome.is_some() {
                self.reconcile_children(parent_id as usize);
            }
        }

        if self.estimate_memory() > self.budget {
            self.memory_limited.store(true, Ordering::Release);
        }
    }

    /// Locate the node for `path`, creating dummy ancestors as needed.
    fn find_or_create_node(&mut self, path: &[Move]) -> u32 {
        let mut id = 0u32;
        for &mv in path {
            let key = ((id as u64) << 32) | (move_to_bits(mv) as u64);
            if let Some(&child_id) = self.child_index.get(&key) {
                id = child_id;
                continue;
            }
            let new_id = self.tree.add_node(id as usize, mv, 0, None, 0) as u32;
            self.child_index.insert(key, new_id);
            id = new_id;
        }
        id
    }

    /// Apply a `NodeProven` event to a real or dummy node.
    fn apply_event(&mut self, id: usize, event: &NodeProven) {
        let node = &mut self.tree.nodes[id];
        node.mv = event.mv;
        node.hash = event.hash;
        match node.outcome {
            None => {
                node.outcome = Some(event.outcome);
                node.depth = event.depth;
            }
            Some(_) if event.depth < node.depth => {
                node.depth = event.depth;
            }
            _ => {}
        }
    }

    /// Validate and select children for a newly realized parent.
    fn reconcile_children(&mut self, parent_id: usize) {
        let Some(parent_outcome) = self.tree.nodes[parent_id].outcome else {
            return;
        };

        let mut old_children = Vec::new();
        let mut child = self.tree.nodes[parent_id].first_child;
        while let Some(c) = child {
            let id = c.get();
            old_children.push(id);
            child = self.tree.nodes[id as usize].next_sibling;
        }

        let kept: Vec<u32> = match parent_outcome {
            Outcome::Win => {
                let mut best: Option<u32> = None;
                let mut best_depth = u32::MAX;
                for child_id in &old_children {
                    let child = &self.tree.nodes[*child_id as usize];
                    if child.outcome == Some(Outcome::Loss) && child.depth < best_depth {
                        best_depth = child.depth;
                        best = Some(*child_id);
                    }
                }
                best.into_iter().collect()
            }
            Outcome::Loss => old_children
                .iter()
                .copied()
                .filter(|&c| self.tree.nodes[c as usize].outcome == Some(Outcome::Win))
                .collect(),
            Outcome::Draw => Vec::new(),
        };

        // Remove pruned children from the child index and, recursively, their
        // subtrees so the map size stays honest for memory accounting.
        let kept_set: HashSet<u32> = kept.iter().copied().collect();
        for child_id in &old_children {
            if !kept_set.contains(child_id) {
                let key = ((parent_id as u64) << 32)
                    | (move_to_bits(self.tree.nodes[*child_id as usize].mv) as u64);
                self.child_index.remove(&key);
                self.remove_child_index_subtree(*child_id);
            }
        }

        // Rebuild the parent's child list from the kept ids.
        self.tree.nodes[parent_id].first_child = None;
        for child_id in kept.iter().rev().copied() {
            let old_first = self.tree.nodes[parent_id].first_child;
            self.tree.nodes[child_id as usize].next_sibling = old_first;
            self.tree.nodes[parent_id].first_child = NonZeroU32::new(child_id);
        }
    }

    fn remove_child_index_subtree(&mut self, root: u32) {
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let children: Vec<usize> = self.tree.children(id as usize).collect();
            for child_id in children {
                let key = ((id as u64) << 32) | (move_to_bits(self.tree.nodes[child_id].mv) as u64);
                self.child_index.remove(&key);
                stack.push(child_id as u32);
            }
        }
    }

    /// Rebuild the `(hash, outcome) -> best node id` index used to
    /// canonicalise transpositions during finalisation. Only terminal nodes
    /// and internal nodes with children are considered "expanded".
    fn build_expanded_index(&mut self) {
        self.expanded_by_hash.clear();
        for (id, node) in self.tree.nodes.iter().enumerate() {
            let Some(outcome) = node.outcome else {
                continue;
            };
            let expanded = node.depth == 0 || node.first_child.is_some();
            if !expanded {
                continue;
            }
            let implied = self.implied_depth(id);
            let consistent = node.depth == implied;
            let better = match self.expanded_by_hash.get(&(node.hash, outcome)) {
                None => true,
                Some(&other) => {
                    let other_node = &self.tree.nodes[other];
                    let other_implied = self.implied_depth(other);
                    let other_consistent = other_node.depth == other_implied;
                    if consistent != other_consistent {
                        consistent
                    } else {
                        node.depth < other_node.depth
                    }
                }
            };
            if better {
                self.expanded_by_hash.insert((node.hash, outcome), id);
            }
        }
    }

    fn implied_depth(&self, id: usize) -> u32 {
        let node = &self.tree.nodes[id];
        if node.depth == 0 {
            0
        } else {
            let child_depths: Vec<u32> = self
                .tree
                .children(id)
                .map(|c| self.tree.nodes[c].depth)
                .collect();
            match node.outcome {
                Some(Outcome::Win) => child_depths
                    .iter()
                    .min()
                    .copied()
                    .unwrap_or(0)
                    .saturating_add(1),
                Some(Outcome::Loss) => child_depths
                    .iter()
                    .max()
                    .copied()
                    .unwrap_or(0)
                    .saturating_add(1),
                _ => 0,
            }
        }
    }

    /// Finalize the proof tree after search has stopped.
    ///
    /// This drains any remaining events, selects a canonical expanded node for
    /// every `(hash, outcome)` key, and rebuilds a complete proof tree by
    /// copying canonical subtrees onto unexpanded transpositions. Dummy nodes
    /// that were never realized are skipped during the rebuild. If the final
    /// tree contains an unexpanded internal node, the process logs an error and
    /// exits.
    fn finalize_tree(&mut self, event_rx: Option<&Receiver<ProofEvent>>) {
        // Drain any remaining events.
        if let Some(rx) = event_rx {
            while let Ok(event) = rx.try_recv() {
                self.handle_event(event);
            }
        }

        if self.tree.nodes[0].outcome.is_none() {
            eprintln!("error: proof-tree root was never realized");
            std::process::exit(1);
        }

        self.build_expanded_index();

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
        let mut path_hashes: HashSet<u64> = HashSet::new();
        let mut path_hash_stack: Vec<u64> = Vec::new();
        actions.push(Action::Enter(0, 0, Move::NONE));

        while let Some(action) = actions.pop() {
            match action {
                Action::Enter(raw_id, parent_new_id, edge_mv) => {
                    let node = &old_tree.nodes[raw_id];
                    if node.outcome.is_none() {
                        continue;
                    }
                    let Some(outcome) = node.outcome else {
                        continue;
                    };
                    let eff_id = self
                        .expanded_by_hash
                        .get(&(node.hash, outcome))
                        .copied()
                        .unwrap_or(raw_id);
                    let eff_node = &old_tree.nodes[eff_id];
                    let hash = eff_node.hash;
                    let eff_outcome = eff_node.outcome;
                    let depth = eff_node.depth;

                    let is_root = edge_mv == Move::NONE && parent_new_id == 0;

                    // Cycle guard: if this hash already appears on the current
                    // path, create a leaf and stop expanding this branch.
                    if !is_root && path_hashes.contains(&hash) {
                        let Some(leaf_outcome) = eff_outcome else {
                            continue;
                        };
                        let _ = new_tree.add_node(
                            parent_new_id,
                            edge_mv,
                            hash,
                            Some(leaf_outcome),
                            depth,
                        );
                        continue;
                    }

                    let new_id = if is_root {
                        new_tree.nodes[0].hash = hash;
                        new_tree.nodes[0].outcome = eff_outcome;
                        new_tree.nodes[0].depth = depth;
                        0
                    } else {
                        path_hashes.insert(hash);
                        path_hash_stack.push(hash);
                        new_tree.add_node(parent_new_id, edge_mv, hash, eff_outcome, depth)
                    };

                    actions.push(Action::Exit);
                    let eff_children: Vec<usize> = old_tree.children(eff_id).collect();
                    for &child_id in eff_children.iter().rev() {
                        let child_mv = old_tree.nodes[child_id].mv;
                        actions.push(Action::Enter(child_id, new_id, child_mv));
                    }
                }
                Action::Exit => {
                    if let Some(h) = path_hash_stack.pop() {
                        path_hashes.remove(&h);
                    }
                }
            }
        }

        // Recompute proven depths from the leaves up.
        for i in (0..new_tree.nodes.len()).rev() {
            if new_tree.nodes[i].first_child.is_none() {
                new_tree.nodes[i].depth = 0;
            } else {
                let child_depths: Vec<u32> = new_tree
                    .children(i)
                    .map(|c| new_tree.nodes[c].depth)
                    .collect();
                new_tree.nodes[i].depth = match new_tree.nodes[i].outcome {
                    Some(Outcome::Win) => child_depths
                        .iter()
                        .min()
                        .copied()
                        .unwrap_or(0)
                        .saturating_add(1),
                    Some(Outcome::Loss) => child_depths
                        .iter()
                        .max()
                        .copied()
                        .unwrap_or(0)
                        .saturating_add(1),
                    _ => 0,
                };
            }
        }

        // Any non-terminal node without children violates the assumption that
        // every proven node has an expanded twin.
        for (id, node) in new_tree.nodes.iter().enumerate() {
            if node.outcome.is_some_and(|o| o != Outcome::Draw)
                && node.depth != 0
                && node.first_child.is_none()
            {
                eprintln!(
                    "error: proof-tree finalization left unexpanded internal node id={} outcome={:?} depth={}",
                    id, node.outcome, node.depth
                );
                std::process::exit(1);
            }
        }

        self.tree = new_tree;
        self.expanded_by_hash.clear();

        // Rebuild the global child-move index so it matches the final tree.
        self.child_index.clear();
        for (i, node) in self.tree.nodes.iter().enumerate() {
            if let Some(p) = node.parent {
                let parent_id = p.get() as usize - 1;
                let key = ((parent_id as u64) << 32) | (move_to_bits(node.mv) as u64);
                self.child_index.insert(key, i as u32);
            }
        }
    }

    fn estimate_memory(&self) -> usize {
        let node_size = std::mem::size_of::<ProofNode>();
        let nodes_mem = self.tree.nodes.capacity() * node_size;

        // HashMap<u64, u32>: entry size ~ key/value pair + control byte.
        let child_index_entry = std::mem::size_of::<(u64, u32)>() + 1;
        let child_index_mem = std::mem::size_of::<HashMap<u64, u32>>()
            + self.child_index.capacity() * child_index_entry;

        // expanded_by_hash only exists briefly during finalization.
        let total = nodes_mem + child_index_mem;
        (total as f64 * 1.1) as usize
    }

    fn stats(&self) -> ProofStats {
        let nodes = self.tree.nodes.len();
        let win_nodes = self
            .tree
            .nodes
            .iter()
            .filter(|n| n.outcome == Some(Outcome::Win))
            .count();
        let loss_nodes = self
            .tree
            .nodes
            .iter()
            .filter(|n| n.outcome == Some(Outcome::Loss))
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
