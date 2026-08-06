//! In-memory proof tree and compact binary dump serializer.
//!
//! The proof tree records the nodes that belong to the final proof subtree.
//! It is intentionally separate from the transposition table so that it can be
//! dumped to a `.bin` file and inspected independently of the search state.
//!
//! This module is larger than 10 KiB because the proof-tree data model,
//! worker thread, and unit tests are closely related; the serialization format
//! and round-trip tests live in `binary.rs`.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use atomic_movegen::types::Move;

use crate::notation::move_to_uci;
use crate::position::Outcome;

pub mod binary;

/// A single node in the proof tree.
#[derive(Debug, Clone)]
pub struct ProofNode {
    pub parent: Option<usize>,
    pub mv: Move,
    pub outcome: Outcome,
    pub depth: u32,
    pub children: Vec<usize>,
}

/// Proof tree indexed by path string for event ordering.
#[derive(Debug, Clone)]
pub struct ProofTree {
    pub root_fen: String,
    pub nodes: Vec<ProofNode>,
    pub index: HashMap<String, usize>,
}

/// Event emitted by the search thread when a node on the proof subtree is
/// proven.
#[derive(Debug, Clone)]
pub struct NodeProven {
    pub path: String,
    pub mv: Move,
    pub outcome: Outcome,
    pub depth: u32,
}

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

impl ProofTree {
    /// Create a new proof tree with a single root node.
    pub fn new(root_fen: String, root_outcome: Outcome, root_depth: u32) -> Self {
        let mut index = HashMap::new();
        index.insert("root".to_string(), 0);
        Self {
            root_fen,
            nodes: vec![ProofNode {
                parent: None,
                mv: Move::NONE,
                outcome: root_outcome,
                depth: root_depth,
                children: Vec::new(),
            }],
            index,
        }
    }

    /// Add a child node under `parent` and return its id.
    pub fn add_node(&mut self, parent: usize, mv: Move, outcome: Outcome, depth: u32) -> usize {
        let parent_path = self.path_for(parent).to_string();
        let uci = move_to_uci(mv);
        let path = format!("{parent_path}.{uci}");
        let id = self.nodes.len();
        self.nodes[parent].children.push(id);
        self.nodes.push(ProofNode {
            parent: Some(parent),
            mv,
            outcome,
            depth,
            children: Vec::new(),
        });
        self.index.insert(path, id);
        id
    }

    /// Serialize the tree to the compact binary adjacency format.
    pub fn to_bin<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        binary::write_proof_tree(self, writer)
    }

    /// Load a tree from the compact binary adjacency format.
    pub fn from_bin<R: Read>(reader: &mut R) -> io::Result<Self> {
        binary::read_proof_tree(reader)
    }

    fn path_for(&self, id: usize) -> &str {
        self.index
            .iter()
            .find(|&(_, &v)| v == id)
            .map(|(k, _)| k.as_str())
            .unwrap_or("root")
    }

    /// Return true if the node is a terminal leaf (proven at depth 0).
    pub fn is_terminal(&self, node_id: usize) -> bool {
        self.nodes.get(node_id).is_some_and(|n| n.depth == 0)
    }

    /// Extract a principal variation from the proof tree.
    ///
    /// * `Win` (OR) nodes pick the proven winning child with the smallest depth.
    /// * `Loss` (AND) nodes pick the defender reply with the largest depth.
    /// * The walk stops at a terminal node.
    pub fn extract_ppv(&self) -> Vec<Move> {
        let mut pv = Vec::new();
        let mut id = 0usize;
        while !self.is_terminal(id) {
            let node = &self.nodes[id];
            let children = node.children.iter().copied().filter(|&c| {
                let child = &self.nodes[c];
                match node.outcome {
                    Outcome::Win => child.outcome == Outcome::Loss,
                    Outcome::Loss => child.outcome == Outcome::Win,
                    Outcome::Draw => false,
                }
            });
            let next = match node.outcome {
                Outcome::Win => children.min_by_key(|&c| self.nodes[c].depth),
                Outcome::Loss => children.max_by_key(|&c| self.nodes[c].depth),
                Outcome::Draw => None,
            };
            let Some(next_id) = next else {
                break;
            };
            pv.push(self.nodes[next_id].mv);
            id = next_id;
        }
        pv
    }

    /// Validate that `pv` is a path from the root to a terminal node in the
    /// proof tree.  This does not replay the moves on a chess board; it only
    /// checks that the sequence of moves exists in the tree.
    pub fn validate_ppv(&self, pv: &[Move]) -> bool {
        let mut id = 0usize;
        for mv in pv {
            if self.is_terminal(id) {
                return false;
            }
            let node = &self.nodes[id];
            let Some(&next_id) = node.children.iter().find(|&&c| self.nodes[c].mv == *mv) else {
                return false;
            };
            id = next_id;
        }
        self.is_terminal(id)
    }
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
    /// Spawn a worker thread and return the channel sender and join handle.
    pub fn spawn(
        root_fen: String,
        pt_size_mb: usize,
        memory_limited: Arc<AtomicBool>,
    ) -> (Sender<ProofMessage>, std::thread::JoinHandle<()>) {
        let (tx, rx) = channel();
        let tree = ProofTree::new(root_fen, Outcome::Draw, 0);
        let index_path_bytes = tree.index.keys().map(|k| k.len()).sum();
        let mut worker = Self {
            tree,
            pending: HashMap::new(),
            budget: pt_size_mb.saturating_mul(1024 * 1024),
            memory_limited,
            index_path_bytes,
            pending_path_bytes: 0,
            pending_event_count: 0,
        };
        let handle = std::thread::spawn(move || worker.run(rx));
        (tx, handle)
    }

    fn run(&mut self, rx: Receiver<ProofMessage>) {
        for msg in rx {
            match msg {
                ProofMessage::Clear => {
                    self.tree = ProofTree::new(self.tree.root_fen.clone(), Outcome::Draw, 0);
                    self.pending.clear();
                    self.index_path_bytes = self.tree.index.keys().map(|k| k.len()).sum();
                    self.pending_path_bytes = 0;
                    self.pending_event_count = 0;
                }
                ProofMessage::NodeProven(event) => self.process_event(event),
                ProofMessage::GetStats(tx) => {
                    let _ = tx.send(ProofResponse::Stats(self.stats()));
                }
                ProofMessage::GetTree(tx) => {
                    let _ = tx.send(ProofResponse::Tree(self.tree.clone()));
                }
            }
        }
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
mod tests {
    use atomic_movegen::types::{Move, Square};

    use super::*;

    #[test]
    fn add_node_reconstructs_path() {
        let mut tree = ProofTree::new("fen".to_string(), Outcome::Win, 2);
        let child = tree.add_node(0, Move::make_move(Square::E2, Square::E4), Outcome::Loss, 1);
        let grandchild = tree.add_node(
            child,
            Move::make_move(Square::E7, Square::E5),
            Outcome::Win,
            0,
        );
        assert_eq!(tree.index["root"], 0);
        assert_eq!(tree.index["root.e2e4"], child);
        assert_eq!(tree.index["root.e2e4.e7e5"], grandchild);
    }

    #[test]
    fn to_bin_round_trips_small_tree() {
        let mut tree = ProofTree::new(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
            Outcome::Win,
            2,
        );
        let child = tree.add_node(0, Move::make_move(Square::E2, Square::E4), Outcome::Loss, 1);
        tree.add_node(
            child,
            Move::make_move(Square::E7, Square::E5),
            Outcome::Win,
            0,
        );

        let mut buf = Vec::new();
        tree.to_bin(&mut buf).unwrap();

        let loaded = ProofTree::from_bin(&mut &buf[..]).unwrap();
        assert_eq!(loaded.nodes.len(), 3);
        assert_eq!(loaded.nodes[0].outcome, Outcome::Win);
        assert_eq!(loaded.nodes[0].depth, 2);
        assert_eq!(loaded.nodes[1].outcome, Outcome::Loss);
        assert_eq!(loaded.nodes[1].depth, 1);
        assert_eq!(loaded.nodes[2].outcome, Outcome::Win);
        assert_eq!(loaded.nodes[2].depth, 0);
        assert_eq!(loaded.nodes[1].mv, Move::make_move(Square::E2, Square::E4));
        assert_eq!(loaded.nodes[2].mv, Move::make_move(Square::E7, Square::E5));

        let ppv = loaded.extract_ppv();
        assert_eq!(
            ppv,
            vec![
                Move::make_move(Square::E2, Square::E4),
                Move::make_move(Square::E7, Square::E5),
            ]
        );
        assert!(loaded.validate_ppv(&ppv));
    }

    #[test]
    fn worker_handles_out_of_order_events() {
        let (tx, handle) =
            ProofTreeWorker::spawn("fen".to_string(), 256, Arc::new(AtomicBool::new(false)));
        tx.send(ProofMessage::NodeProven(NodeProven {
            path: "root.e2e4.e7e5".to_string(),
            mv: Move::make_move(Square::E7, Square::E5),
            outcome: Outcome::Win,
            depth: 0,
        }))
        .unwrap();
        tx.send(ProofMessage::NodeProven(NodeProven {
            path: "root.e2e4".to_string(),
            mv: Move::make_move(Square::E2, Square::E4),
            outcome: Outcome::Loss,
            depth: 1,
        }))
        .unwrap();
        tx.send(ProofMessage::NodeProven(NodeProven {
            path: "root".to_string(),
            mv: Move::NONE,
            outcome: Outcome::Win,
            depth: 2,
        }))
        .unwrap();

        let (reply_tx, reply_rx) = channel();
        tx.send(ProofMessage::GetStats(reply_tx)).unwrap();
        let ProofResponse::Stats(stats) = reply_rx.recv().unwrap() else {
            panic!("expected Stats response");
        };
        assert_eq!(stats.nodes, 3);
        assert_eq!(stats.win_nodes, 2);
        assert_eq!(stats.loss_nodes, 1);
        assert_eq!(stats.root_depth, 2);

        let (reply_tx2, reply_rx2) = channel();
        tx.send(ProofMessage::GetTree(reply_tx2)).unwrap();
        let ProofResponse::Tree(tree) = reply_rx2.recv().unwrap() else {
            panic!("expected Tree response");
        };
        assert_eq!(tree.nodes.len(), 3);
        assert_eq!(tree.nodes[0].children, vec![1]);
        assert_eq!(tree.nodes[1].children, vec![2]);

        drop(tx);
        handle.join().unwrap();
    }

    #[test]
    fn worker_replaces_win_child_with_shortest_loss() {
        let (tx, handle) =
            ProofTreeWorker::spawn("fen".to_string(), 256, Arc::new(AtomicBool::new(false)));
        tx.send(ProofMessage::NodeProven(NodeProven {
            path: "root".to_string(),
            mv: Move::NONE,
            outcome: Outcome::Win,
            depth: 5,
        }))
        .unwrap();
        tx.send(ProofMessage::NodeProven(NodeProven {
            path: "root.e2e4".to_string(),
            mv: Move::make_move(Square::E2, Square::E4),
            outcome: Outcome::Loss,
            depth: 4,
        }))
        .unwrap();
        tx.send(ProofMessage::NodeProven(NodeProven {
            path: "root.d2d4".to_string(),
            mv: Move::make_move(Square::D2, Square::D4),
            outcome: Outcome::Loss,
            depth: 2,
        }))
        .unwrap();
        // A deeper duplicate of the selected child must be ignored, not appended.
        tx.send(ProofMessage::NodeProven(NodeProven {
            path: "root.d2d4".to_string(),
            mv: Move::make_move(Square::D2, Square::D4),
            outcome: Outcome::Loss,
            depth: 6,
        }))
        .unwrap();

        let (reply_tx, reply_rx) = channel();
        tx.send(ProofMessage::GetTree(reply_tx)).unwrap();
        let ProofResponse::Tree(tree) = reply_rx.recv().unwrap() else {
            panic!("expected Tree response");
        };
        assert_eq!(tree.nodes[0].children.len(), 1);
        assert_eq!(
            tree.nodes[tree.nodes[0].children[0]].mv,
            Move::make_move(Square::D2, Square::D4)
        );
        assert_eq!(tree.nodes[tree.nodes[0].children[0]].depth, 2);

        drop(tx);
        handle.join().unwrap();
    }

    #[test]
    fn worker_loss_parent_keeps_all_distinct_win_children() {
        let (tx, handle) =
            ProofTreeWorker::spawn("fen".to_string(), 256, Arc::new(AtomicBool::new(false)));
        tx.send(ProofMessage::NodeProven(NodeProven {
            path: "root".to_string(),
            mv: Move::NONE,
            outcome: Outcome::Loss,
            depth: 5,
        }))
        .unwrap();
        tx.send(ProofMessage::NodeProven(NodeProven {
            path: "root.e2e4".to_string(),
            mv: Move::make_move(Square::E2, Square::E4),
            outcome: Outcome::Win,
            depth: 4,
        }))
        .unwrap();
        tx.send(ProofMessage::NodeProven(NodeProven {
            path: "root.d2d4".to_string(),
            mv: Move::make_move(Square::D2, Square::D4),
            outcome: Outcome::Win,
            depth: 2,
        }))
        .unwrap();

        let (reply_tx, reply_rx) = channel();
        tx.send(ProofMessage::GetTree(reply_tx)).unwrap();
        let ProofResponse::Tree(tree) = reply_rx.recv().unwrap() else {
            panic!("expected Tree response");
        };
        assert_eq!(tree.nodes[0].children.len(), 2);

        // A duplicate with a shorter depth updates the existing child.
        tx.send(ProofMessage::NodeProven(NodeProven {
            path: "root.e2e4".to_string(),
            mv: Move::make_move(Square::E2, Square::E4),
            outcome: Outcome::Win,
            depth: 1,
        }))
        .unwrap();

        let (reply_tx2, reply_rx2) = channel();
        tx.send(ProofMessage::GetTree(reply_tx2)).unwrap();
        let ProofResponse::Tree(tree2) = reply_rx2.recv().unwrap() else {
            panic!("expected Tree response");
        };
        assert_eq!(tree2.nodes[0].children.len(), 2);
        let e2e4_id = tree2.index["root.e2e4"];
        assert_eq!(tree2.nodes[e2e4_id].depth, 1);

        drop(tx);
        handle.join().unwrap();
    }

    #[test]
    fn worker_sets_memory_limited_flag() {
        let flag = Arc::new(AtomicBool::new(false));
        let (tx, handle) = ProofTreeWorker::spawn("fen".to_string(), 0, Arc::clone(&flag));
        tx.send(ProofMessage::NodeProven(NodeProven {
            path: "root".to_string(),
            mv: Move::NONE,
            outcome: Outcome::Win,
            depth: 0,
        }))
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(
            flag.load(Ordering::Acquire),
            "memory flag should be set for zero budget"
        );
        drop(tx);
        handle.join().unwrap();
    }
}
