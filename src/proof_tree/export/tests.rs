//! Unit tests for the provari JSON-lines proof-tree exporter.

use atomic_movegen::types::{Move, Square};

use super::{ExportError, write_provari_jsonl};
use crate::notation::{move_to_uci, uci_to_move};
use crate::position::{Outcome, Position};
use crate::proof_tree::{ProofTree, validate_proof_tree};

const MATE_FEN: &str = "k7/8/1Q6/8/8/8/8/7K w - - 0 1";
const MATED_FEN: &str = "k7/1Q6/8/8/8/8/8/7K b - - 1 1";

/// The hand-built mate-in-1 tree: root Win plays Qb7#, and the resulting
/// position is statically terminal (Loss for the side to move).
fn mate_in_one_tree() -> ProofTree {
    let pos = Position::from_fen(MATE_FEN).unwrap();
    let mate = uci_to_move("b6b7", &pos).expect("b6b7 must be legal");
    let mut after = pos.clone();
    after.do_move(mate);
    assert_eq!(after.outcome(), Some(Outcome::Loss), "b6b7 must be mate");

    let mut tree = ProofTree::new(MATE_FEN.to_string(), pos.hash(), Some(Outcome::Win), 1);
    tree.add_node(0, mate, after.hash(), Some(Outcome::Loss), 0);
    tree
}

/// The output is the header plus one attacker line, and replaying that line
/// on the root position reproduces the proof move.
#[test]
fn mate_in_one_exports_header_and_one_line() {
    let tree = mate_in_one_tree();
    assert_eq!(validate_proof_tree(&tree), Ok(()));

    let mut out = Vec::new();
    let lines = write_provari_jsonl(&tree, &mut out).expect("valid tree must export");
    assert_eq!(lines, 1);

    let text = String::from_utf8(out).expect("output must be UTF-8");
    // The line names the position the move is played in (the root).
    let expected =
        format!("{{\"root\":\"{MATE_FEN}\"}}\n{{\"fen\":\"{MATE_FEN}\",\"move\":\"b6b7\"}}\n");
    assert_eq!(text, expected);

    // Replay the emitted move on the root position independently.
    let pos = Position::from_fen(MATE_FEN).unwrap();
    let line = text.lines().nth(1).expect("one attacker line");
    let uci = line
        .split_once("\"move\":\"")
        .and_then(|(_, rest)| rest.split_once('"'))
        .map(|(uci, _)| uci)
        .expect("the line carries a move");
    let mv: Move = uci_to_move(uci, &pos).expect("emitted move must be legal");
    assert_eq!(
        move_to_uci(mv),
        move_to_uci(Move::make_move(Square::B6, Square::B7))
    );
}

/// A valid root-`Loss` tree is rejected before anything is written.
#[test]
fn root_loss_is_rejected_without_writing() {
    let tree = ProofTree::new(MATED_FEN.to_string(), 0, Some(Outcome::Loss), 0);
    assert_eq!(validate_proof_tree(&tree), Ok(()));

    let mut out = Vec::new();
    let err = write_provari_jsonl(&tree, &mut out).expect_err("root Loss must be rejected");
    assert!(
        matches!(err, ExportError::RootNotWin(Outcome::Loss)),
        "expected RootNotWin(Loss), got: {err:?}"
    );
    assert!(out.is_empty(), "a rejected export must write nothing");
}

/// An invalid tree is rejected with the validator's defects, and nothing is
/// written.
#[test]
fn invalid_tree_is_rejected_without_writing() {
    // A non-terminal Win root with no child violates WinNotSingle.
    let tree = ProofTree::new(MATE_FEN.to_string(), 0, Some(Outcome::Win), 2);
    let defects = validate_proof_tree(&tree).expect_err("childless Win root is invalid");

    let mut out = Vec::new();
    let err = write_provari_jsonl(&tree, &mut out).expect_err("invalid tree must be rejected");
    match err {
        ExportError::Invalid(found) => assert_eq!(found, defects),
        other => panic!("expected Invalid, got: {other:?}"),
    }
    assert!(out.is_empty(), "a rejected export must write nothing");
}

/// A tree loaded from the binary dump carries `hash == 0`; validation skips
/// hash checks and the replay-based export still works.
#[test]
fn loaded_from_bin_tree_exports() {
    let tree = mate_in_one_tree();
    let mut bytes = Vec::new();
    tree.to_bin(&mut bytes).expect("dump must serialize");
    let loaded = ProofTree::from_bin(&mut &bytes[..]).expect("dump must parse");
    assert!(loaded.nodes.iter().all(|node| node.hash == 0));

    let mut direct = Vec::new();
    let direct_lines = write_provari_jsonl(&tree, &mut direct).expect("direct export");
    let mut from_bin = Vec::new();
    let loaded_lines = write_provari_jsonl(&loaded, &mut from_bin).expect("loaded export");

    assert_eq!(direct_lines, 1);
    assert_eq!(loaded_lines, direct_lines);
    assert_eq!(from_bin, direct);
}
