//! In-memory proof tree and PostgreSQL `ltree` dump serializer.
//!
//! The proof tree records the nodes that belong to the final proof subtree.
//! It is intentionally separate from the transposition table so that it can be
//! dumped to a `.sql` file and inspected independently of the search state.

use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use crate::position::Outcome;

/// A single node in the proof tree.
#[derive(Debug, Clone)]
pub struct ProofNode {
    pub parent: Option<usize>,
    pub uci_move: String,
    pub outcome: Outcome,
    pub depth: u32,
    pub children: Vec<usize>,
}

/// Proof tree indexed by `ltree` path.
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
    pub uci_move: String,
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
                uci_move: String::new(),
                outcome: root_outcome,
                depth: root_depth,
                children: Vec::new(),
            }],
            index,
        }
    }

    /// Add a child node under `parent` and return its id.
    pub fn add_node(
        &mut self,
        parent: usize,
        uci_move: String,
        outcome: Outcome,
        depth: u32,
    ) -> usize {
        let parent_path = self.path_for(parent).to_string();
        let label = sanitize_label(&uci_move);
        let path = format!("{parent_path}.{label}");
        let id = self.nodes.len();
        self.nodes[parent].children.push(id);
        self.nodes.push(ProofNode {
            parent: Some(parent),
            uci_move,
            outcome,
            depth,
            children: Vec::new(),
        });
        self.index.insert(path, id);
        id
    }

    /// Serialize the tree to a PostgreSQL `.sql` dump.
    pub fn to_sql<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writeln!(writer, "CREATE EXTENSION IF NOT EXISTS ltree;")?;
        writeln!(writer)?;
        writeln!(
            writer,
            "CREATE TABLE proof_meta (key text PRIMARY KEY, value text);"
        )?;
        writeln!(writer, "CREATE TABLE proof_nodes (")?;
        writeln!(writer, "    path ltree PRIMARY KEY,")?;
        writeln!(writer, "    parent_path ltree,")?;
        writeln!(writer, "    uci_move text,")?;
        writeln!(
            writer,
            "    outcome text CHECK (outcome IN ('Win', 'Loss')),"
        )?;
        writeln!(writer, "    depth int,")?;
        writeln!(writer, "    terminal boolean")?;
        writeln!(writer, ");")?;
        writeln!(
            writer,
            "CREATE INDEX idx_proof_nodes_parent ON proof_nodes USING btree (parent_path);"
        )?;
        writeln!(
            writer,
            "CREATE INDEX idx_proof_nodes_path ON proof_nodes USING gist (path);"
        )?;
        writeln!(writer)?;
        let fen = self.root_fen.replace('\'', "''");
        writeln!(
            writer,
            "INSERT INTO proof_meta (key, value) VALUES ('root_fen', '{fen}');"
        )?;
        writeln!(writer)?;
        writeln!(
            writer,
            "COPY proof_nodes (path, parent_path, uci_move, outcome, depth, terminal) FROM STDIN;"
        )?;
        self.write_node_sql(writer, 0, "root", "\\N")?;
        writeln!(writer, "\\.")?;
        Ok(())
    }

    fn write_node_sql<W: Write>(
        &self,
        writer: &mut W,
        id: usize,
        path: &str,
        parent_path: &str,
    ) -> io::Result<()> {
        let node = &self.nodes[id];
        let outcome = match node.outcome {
            Outcome::Win => "Win",
            Outcome::Loss => "Loss",
            Outcome::Draw => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "ProofTree SQL supports only Win/Loss nodes",
                ));
            }
        };
        let terminal = if node.depth == 0 { "true" } else { "false" };
        writeln!(
            writer,
            "{path}\t{parent_path}\t{}\t{outcome}\t{}\t{terminal}",
            node.uci_move, node.depth
        )?;
        for &child in &node.children {
            let child_path = format!("{path}.{}", sanitize_label(&self.nodes[child].uci_move));
            self.write_node_sql(writer, child, &child_path, path)?;
        }
        Ok(())
    }

    fn path_for(&self, id: usize) -> &str {
        self.index
            .iter()
            .find(|&(_, &v)| v == id)
            .map(|(k, _)| k.as_str())
            .unwrap_or("root")
    }
}

/// Background worker that collects `NodeProven` events and maintains the
/// in-memory proof tree.
pub struct ProofTreeWorker {
    tree: ProofTree,
    pending: HashMap<String, Vec<NodeProven>>,
    budget: usize,
    memory_limited: Arc<AtomicBool>,
}

impl ProofTreeWorker {
    /// Spawn a worker thread and return the channel sender and join handle.
    pub fn spawn(
        root_fen: String,
        pt_size_mb: usize,
        memory_limited: Arc<AtomicBool>,
    ) -> (Sender<ProofMessage>, std::thread::JoinHandle<()>) {
        let (tx, rx) = channel();
        let mut worker = Self {
            tree: ProofTree::new(root_fen, Outcome::Draw, 0),
            pending: HashMap::new(),
            budget: pt_size_mb.saturating_mul(1024 * 1024),
            memory_limited,
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
        if self.estimate_memory() > self.budget {
            self.memory_limited.store(true, Ordering::Release);
            return;
        }
        self.insert_event(event);
    }

    fn insert_event(&mut self, event: NodeProven) {
        if event.path == "root" {
            self.tree.nodes[0].uci_move = event.uci_move;
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
            let keep = if let Some(&existing_id) = self.tree.nodes[parent_id].children.first() {
                event.depth < self.tree.nodes[existing_id].depth
            } else {
                true
            };
            if !keep {
                if let Some(&id) = self.tree.index.get(&event.path) {
                    self.tree.nodes[id].depth = event.depth;
                }
                return;
            }
            self.tree.nodes[parent_id].children.clear();
        }

        let id = if let Some(&id) = self.tree.index.get(&event.path) {
            self.tree.nodes[id].uci_move = event.uci_move;
            self.tree.nodes[id].outcome = event.outcome;
            self.tree.nodes[id].depth = event.depth;
            id
        } else {
            let id = self.tree.nodes.len();
            self.tree.nodes.push(ProofNode {
                parent: Some(parent_id),
                uci_move: event.uci_move,
                outcome: event.outcome,
                depth: event.depth,
                children: Vec::new(),
            });
            self.tree.index.insert(event.path.clone(), id);
            id
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
            for child in children {
                self.attach_child(parent_id, child);
            }
        }
    }

    fn estimate_memory(&self) -> usize {
        let node_size = std::mem::size_of::<ProofNode>();
        let nodes_mem = self.tree.nodes.len() * node_size;
        let strings_mem: usize = self
            .tree
            .nodes
            .iter()
            .map(|n| n.uci_move.capacity() + std::mem::size_of::<String>())
            .sum();
        let index_overhead = self.tree.index.capacity()
            * (std::mem::size_of::<String>() + std::mem::size_of::<usize>() + 8);
        let pending_overhead: usize = self
            .pending
            .iter()
            .map(|(k, v)| {
                k.capacity()
                    + std::mem::size_of::<String>()
                    + std::mem::size_of::<Vec<NodeProven>>()
                    + v.capacity() * std::mem::size_of::<NodeProven>()
                    + v.iter()
                        .map(|e| e.path.capacity() + e.uci_move.capacity())
                        .sum::<usize>()
            })
            .sum();
        let total = nodes_mem + strings_mem + index_overhead + pending_overhead;
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

/// Sanitize a UCI move into a valid `ltree` label.
///
/// Labels must consist of alphanumeric ASCII characters, underscores, or
/// hyphens, be at most 1000 bytes long, and not start with a digit.
fn sanitize_label(s: &str) -> String {
    let mut label: String = s
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if label.is_empty() {
        label.push('_');
    }
    if label.as_bytes()[0].is_ascii_digit() {
        label.insert(0, '_');
    }
    if label.len() > 1000 {
        label.truncate(1000);
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_label_lowercases_and_replaces_invalid_chars() {
        assert_eq!(sanitize_label("e7e8Q"), "e7e8q");
        assert_eq!(sanitize_label("E7E8Q"), "e7e8q");
        assert_eq!(sanitize_label("e1!g1"), "e1_g1");
        assert_eq!(sanitize_label("bad move"), "bad_move");
    }

    #[test]
    fn sanitize_label_handles_empty_and_leading_digit() {
        assert_eq!(sanitize_label(""), "_");
        assert_eq!(sanitize_label("0-0"), "_0-0");
    }

    #[test]
    fn add_node_reconstructs_ltree_path() {
        let mut tree = ProofTree::new("fen".to_string(), Outcome::Win, 2);
        let child = tree.add_node(0, "e2e4".to_string(), Outcome::Loss, 1);
        let grandchild = tree.add_node(child, "e7e5".to_string(), Outcome::Win, 0);
        assert_eq!(tree.index["root"], 0);
        assert_eq!(tree.index["root.e2e4"], child);
        assert_eq!(tree.index["root.e2e4.e7e5"], grandchild);
    }

    #[test]
    fn to_sql_serializes_small_tree() {
        let mut tree = ProofTree::new(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
            Outcome::Win,
            2,
        );
        let child = tree.add_node(0, "e2e4".to_string(), Outcome::Loss, 1);
        tree.add_node(child, "e7e5".to_string(), Outcome::Win, 0);

        let mut buf = Vec::new();
        tree.to_sql(&mut buf).unwrap();
        let sql = String::from_utf8(buf).unwrap();

        assert!(sql.contains("CREATE EXTENSION IF NOT EXISTS ltree;"));
        assert!(sql.contains("CREATE TABLE proof_meta"));
        assert!(sql.contains("CREATE TABLE proof_nodes"));
        assert!(sql.contains("CREATE INDEX idx_proof_nodes_parent"));
        assert!(sql.contains("CREATE INDEX idx_proof_nodes_path"));
        assert!(sql.contains("COPY proof_nodes"));
        assert!(sql.contains("\\."));
        assert!(sql.contains("root\t\\N\t\tWin\t2\tfalse"));
        assert!(sql.contains("root.e2e4\troot\te2e4\tLoss\t1\tfalse"));
        assert!(sql.contains("root.e2e4.e7e5\troot.e2e4\te7e5\tWin\t0\ttrue"));
    }

    #[test]
    fn to_sql_escapes_fen_single_quotes() {
        let tree = ProofTree::new("f'en with quote".to_string(), Outcome::Win, 0);
        let mut buf = Vec::new();
        tree.to_sql(&mut buf).unwrap();
        let sql = String::from_utf8(buf).unwrap();
        assert!(sql.contains("'f''en with quote'"));
    }

    #[test]
    fn worker_handles_out_of_order_events() {
        let (tx, handle) =
            ProofTreeWorker::spawn("fen".to_string(), 256, Arc::new(AtomicBool::new(false)));
        tx.send(ProofMessage::NodeProven(NodeProven {
            path: "root.e2e4.e7e5".to_string(),
            uci_move: "e7e5".to_string(),
            outcome: Outcome::Win,
            depth: 0,
        }))
        .unwrap();
        tx.send(ProofMessage::NodeProven(NodeProven {
            path: "root.e2e4".to_string(),
            uci_move: "e2e4".to_string(),
            outcome: Outcome::Loss,
            depth: 1,
        }))
        .unwrap();
        tx.send(ProofMessage::NodeProven(NodeProven {
            path: "root".to_string(),
            uci_move: String::new(),
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
            uci_move: String::new(),
            outcome: Outcome::Win,
            depth: 5,
        }))
        .unwrap();
        tx.send(ProofMessage::NodeProven(NodeProven {
            path: "root.e2e4".to_string(),
            uci_move: "e2e4".to_string(),
            outcome: Outcome::Loss,
            depth: 4,
        }))
        .unwrap();
        tx.send(ProofMessage::NodeProven(NodeProven {
            path: "root.d2d4".to_string(),
            uci_move: "d2d4".to_string(),
            outcome: Outcome::Loss,
            depth: 2,
        }))
        .unwrap();

        let (reply_tx, reply_rx) = channel();
        tx.send(ProofMessage::GetTree(reply_tx)).unwrap();
        let ProofResponse::Tree(tree) = reply_rx.recv().unwrap() else {
            panic!("expected Tree response");
        };
        assert_eq!(tree.nodes[0].children.len(), 1);
        assert_eq!(tree.nodes[tree.nodes[0].children[0]].uci_move, "d2d4");
        assert_eq!(tree.nodes[tree.nodes[0].children[0]].depth, 2);

        drop(tx);
        handle.join().unwrap();
    }

    #[test]
    fn worker_loss_parent_keeps_all_win_children() {
        let (tx, handle) =
            ProofTreeWorker::spawn("fen".to_string(), 256, Arc::new(AtomicBool::new(false)));
        tx.send(ProofMessage::NodeProven(NodeProven {
            path: "root".to_string(),
            uci_move: String::new(),
            outcome: Outcome::Loss,
            depth: 3,
        }))
        .unwrap();
        tx.send(ProofMessage::NodeProven(NodeProven {
            path: "root.e2e4".to_string(),
            uci_move: "e2e4".to_string(),
            outcome: Outcome::Win,
            depth: 2,
        }))
        .unwrap();
        tx.send(ProofMessage::NodeProven(NodeProven {
            path: "root.d2d4".to_string(),
            uci_move: "d2d4".to_string(),
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

        drop(tx);
        handle.join().unwrap();
    }

    #[test]
    fn worker_sets_memory_limited_flag() {
        let flag = Arc::new(AtomicBool::new(false));
        let (tx, handle) = ProofTreeWorker::spawn("fen".to_string(), 0, Arc::clone(&flag));
        tx.send(ProofMessage::NodeProven(NodeProven {
            path: "root".to_string(),
            uci_move: String::new(),
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
