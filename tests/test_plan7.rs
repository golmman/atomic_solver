//! Integration test for the plan7 default workflow
//! (`docs/plans/proof/plan7.md`): the search CLI no longer builds proof
//! trees — proof construction moves to `--tt-dump-path` + offline
//! reconstruction.
//!
//! Fast tier, end-to-end: run the CLI binary on the two-rook mate fixture
//! with `--tt-dump-path`, assert the snapshot line and the absence of the
//! removed proof-tree stdout lines, then reconstruct the proof tree from the
//! snapshot with the library API (same pattern as `tests/reconstruct.rs`) and
//! require `validate: ok`. This is the replacement gate for what the
//! solver's `pt_validate:` pre-exit hook used to guarantee.

mod common;

use std::process::{Command, Stdio};

use atomic_solver::position::Outcome;
use atomic_solver::proof_tree::validate_proof_tree;
use atomic_solver::reconstruct::{ReconstructConfig, reconstruct};
use atomic_solver::tt_snapshot::read_tt_snapshot;
use common::cli_bin;
use std::io::BufReader;

const MATE_FEN: &str = "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1";

#[test]
fn plan7_solve_then_reconstruct_is_the_default_workflow() {
    let snapshot_path = "target/tt_snapshot_test_plan7.bin";
    let _ = std::fs::remove_file(snapshot_path);

    // 1. Decisive run with the opt-in TT snapshot.
    let output = Command::new(cli_bin())
        .args([
            "--fen",
            MATE_FEN,
            "--timeout",
            "2",
            "--tt-dump-path",
            snapshot_path,
        ])
        .stdin(Stdio::null())
        .output()
        .expect("failed to run CLI binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "CLI failed: {stdout}");
    assert!(
        stdout.contains("outcome: win"),
        "expected a win outcome:\n{stdout}"
    );
    assert!(
        stdout.contains(&format!("tt_snapshot: {snapshot_path} ")),
        "expected the tt_snapshot line in stdout:\n{stdout}"
    );
    // The proof-tree stdout lines are gone from the search CLI.
    for removed in ["proof_tree:", "proof_tree_dump:", "pt_validate:"] {
        assert!(
            !stdout.contains(removed),
            "solver stdout must not contain '{removed}' anymore:\n{stdout}"
        );
    }

    // 2. Offline reconstruction from the snapshot (the plan5 tool's library
    //    path, exactly what `reconstruct_pt --snapshot` drives).
    let file = std::fs::File::open(snapshot_path).expect("snapshot file should exist");
    let (header, solved, _unsolved) =
        read_tt_snapshot(&mut BufReader::new(file)).expect("snapshot should parse");
    assert_eq!(header.root_fen, MATE_FEN);

    let output = reconstruct(MATE_FEN, &solved, &ReconstructConfig::default());
    assert!(
        output.error.is_none(),
        "reconstruction failed: {:?}",
        output.error
    );
    assert_eq!(output.root_outcome, Some(Outcome::Win));

    // 3. The reconstructed tree validates (`validate: ok`) and round-trips
    //    through the binary dump format.
    let tree = output
        .tree
        .expect("successful reconstruction carries a tree");
    assert!(
        validate_proof_tree(&tree).is_ok(),
        "reconstructed proof tree must validate (validate: ok)"
    );

    let dump_path = "target/proof_tree_test_plan7.bin";
    {
        let file = std::fs::File::create(dump_path).expect("create dump file");
        tree.to_bin(&mut std::io::BufWriter::new(file))
            .expect("dump write");
    }
    let reloaded = std::fs::File::open(dump_path)
        .map(|f| atomic_solver::proof_tree::ProofTree::from_bin(&mut BufReader::new(f)))
        .expect("open dump file")
        .expect("dump should parse");
    assert!(
        reloaded.nodes.len() == tree.nodes.len() && reloaded.nodes.len() > 1,
        "reloaded dump should carry the same non-trivial node set"
    );

    let _ = std::fs::remove_file(snapshot_path);
    let _ = std::fs::remove_file(dump_path);
}
