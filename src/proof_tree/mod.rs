//! In-memory proof tree and PostgreSQL `ltree` dump serializer.
//!
//! The proof tree records the nodes that belong to the final proof subtree.
//! It is intentionally separate from the transposition table so that it can be
//! dumped to a `.sql` file and inspected independently of the search state.

use std::collections::HashMap;
use std::io::{self, Write};

use crate::position::Outcome;

/// A single node in the proof tree.
#[derive(Debug)]
pub struct ProofNode {
    pub parent: Option<usize>,
    pub uci_move: String,
    pub outcome: Outcome,
    pub depth: u32,
    pub children: Vec<usize>,
}

/// Proof tree indexed by `ltree` path.
#[derive(Debug)]
pub struct ProofTree {
    pub root_fen: String,
    pub nodes: Vec<ProofNode>,
    pub index: HashMap<String, usize>,
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
}
