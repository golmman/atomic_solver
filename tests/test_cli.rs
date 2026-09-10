mod common;

use std::process::{Command, Stdio};

use atomic_solver::position::Outcome;
use atomic_solver::tt_snapshot::read_tt_snapshot;
use common::cli_bin;

const MATE_FEN: &str = "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1";

#[test]
fn cli_help_lists_options_and_exits_cleanly() {
    let output = Command::new(cli_bin())
        .arg("--help")
        .output()
        .expect("failed to run CLI binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "--help should exit cleanly");
    assert!(stdout.contains("--fen"), "help should mention --fen");
    assert!(
        stdout.contains("--timeout"),
        "help should mention --timeout"
    );
    assert!(
        stdout.contains("--epsilon"),
        "help should mention --epsilon"
    );
    assert!(
        stdout.contains("--dump-path"),
        "help should mention --dump-path"
    );
    assert!(
        stdout.contains("--tt-dump-path"),
        "help should mention --tt-dump-path"
    );
}

#[test]
fn cli_outcome_only_does_not_print_pre_exit_summary() {
    let output = Command::new(cli_bin())
        .args([
            "--fen",
            "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1",
            "--timeout",
            "1",
            "--outcome-only",
        ])
        .output()
        .expect("failed to run CLI binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "CLI failed: {stdout}");
    assert!(stdout.contains("outcome: win"), "expected a win outcome");
    assert!(stdout.contains("pv:"), "expected a PV line");
    assert!(
        !stdout.contains("pre_exit:"),
        "--outcome-only should not print a pre_exit summary"
    );
}

#[test]
fn cli_dump_path_writes_proof_tree_dump() {
    let dump_path = "target/proof_tree_test_cli.bin";
    let _ = std::fs::remove_file(dump_path);

    let output = Command::new(cli_bin())
        .args([
            "--fen",
            "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1",
            "--timeout",
            "1",
            "--dump-path",
            dump_path,
        ])
        .stdin(Stdio::null())
        .output()
        .expect("failed to run CLI binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "CLI failed: {stdout}");
    assert!(
        stdout.contains(&format!("proof_tree_dump: {dump_path}")),
        "expected proof_tree_dump line in stdout:\n{stdout}"
    );

    let metadata = std::fs::metadata(dump_path).expect("dump file should exist");
    assert!(metadata.len() > 0, "proof tree dump should not be empty");
    let _ = std::fs::remove_file(dump_path);
}

#[test]
fn cli_first_outcome_dumps_proof_tree() {
    let output = Command::new(cli_bin())
        .args([
            "--fen",
            "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1",
            "--timeout",
            "1",
            "--first-outcome",
        ])
        .stdin(Stdio::null())
        .output()
        .expect("failed to run CLI binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "CLI failed: {stdout}");
    assert!(stdout.contains("outcome: win"), "expected a win outcome");
    assert!(
        stdout.contains("proof_tree_dump"),
        "expected a proof_tree_dump line:\n{stdout}"
    );
    assert!(
        !stdout.contains("ppv_valid:"),
        "CLI should not print ppv_valid"
    );
}

fn parse_outcome(line: &str) -> Option<Outcome> {
    line.strip_prefix("outcome: ")?
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<Outcome>().ok())
}

#[test]
fn cli_solves_default_start_position_without_arguments() {
    // The default position is the standard start; with a tiny timeout the solver
    // should either time out cleanly or (if it gets lucky) report a draw.
    let output = Command::new(cli_bin())
        .args(["--timeout", "1", "--outcome-only"])
        .output()
        .expect("failed to run CLI binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "CLI failed: {stdout}");
    let has_timeout = stdout.lines().any(|l| l == "timeout");
    let outcome = stdout
        .lines()
        .find(|l| l.starts_with("outcome: "))
        .and_then(parse_outcome);
    assert!(
        has_timeout || outcome == Some(Outcome::Draw),
        "start position should time out to a draw:\n{stdout}"
    );
}

/// The CLI prints an outcome and an informational PV for a decisive position.
#[test]
fn cli_prints_pv_for_decisive_position() {
    let output = Command::new(cli_bin())
        .args([
            "--fen",
            "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1",
            "--timeout",
            "1",
            "--outcome-only",
        ])
        .output()
        .expect("failed to run CLI binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "CLI failed: {stdout}");
    assert!(stdout.contains("outcome:"), "expected an outcome line");
    assert!(stdout.contains("pv:"), "expected a PV line");
    assert!(
        !stdout.contains("ppv_valid:"),
        "CLI should not print ppv_valid"
    );
}

/// The Re8# back-rank mate is a 1-move win: refinement cannot shorten it, so
/// the PV is proven shortest.
#[test]
fn cli_prints_pv_status_proven_shortest_for_mate_in_one() {
    let output = Command::new(cli_bin())
        .args([
            "--fen",
            "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1",
            "--timeout",
            "1",
            "--outcome-only",
        ])
        .output()
        .expect("failed to run CLI binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "CLI failed: {stdout}");
    assert!(stdout.contains("outcome: win"), "expected a win outcome");
    assert!(
        stdout.contains("pv_status: proven-shortest"),
        "expected a proven-shortest pv_status line:\n{stdout}"
    );
}

/// `--first-outcome` on the promotion fixture skips refinement: the PV came
/// from the first-outcome phase and its length is informational.
#[test]
fn cli_prints_pv_status_first_outcome() {
    let output = Command::new(cli_bin())
        .args([
            "--fen",
            "4k3/PP6/8/8/8/8/8/4K3 w - - 0 1",
            "--timeout",
            "1",
            "--first-outcome",
            "--outcome-only",
        ])
        .output()
        .expect("failed to run CLI binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "CLI failed: {stdout}");
    assert!(stdout.contains("outcome: win"), "expected a win outcome");
    assert!(
        stdout.contains("pv_status: first-outcome"),
        "expected a first-outcome pv_status line:\n{stdout}"
    );
}

/// A decisive outcome always prints a `pv_status:` line; a draw never does.
#[test]
fn cli_draw_prints_no_pv_status() {
    // Bare kings are a terminal draw.
    let output = Command::new(cli_bin())
        .args([
            "--fen",
            "4k3/8/8/8/8/8/8/4K3 w - - 0 1",
            "--timeout",
            "1",
            "--outcome-only",
        ])
        .output()
        .expect("failed to run CLI binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "CLI failed: {stdout}");
    assert!(stdout.contains("outcome: draw"), "expected a draw outcome");
    assert!(
        !stdout.contains("pv_status:"),
        "a draw must not print a pv_status line:\n{stdout}"
    );
}

/// `--tt-dump-path` writes a binary TT snapshot after the search; the snapshot
/// line is printed and the file parses with `solved_count >= 1` and a size
/// bounded by the TT RAM (default `--tt-size` = 128 MB).
#[test]
fn cli_tt_dump_path_writes_parsable_snapshot() {
    let dump_path = "target/tt_snapshot_test_cli.bin";
    let _ = std::fs::remove_file(dump_path);

    let output = Command::new(cli_bin())
        .args([
            "--fen",
            MATE_FEN,
            "--timeout",
            "1",
            "--tt-dump-path",
            dump_path,
        ])
        .stdin(Stdio::null())
        .output()
        .expect("failed to run CLI binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "CLI failed: {stdout}");
    assert!(
        stdout.contains(&format!("tt_snapshot: {dump_path} ")),
        "expected tt_snapshot line in stdout:\n{stdout}"
    );

    let bytes = std::fs::read(dump_path).expect("snapshot file should exist");
    let (header, solved, _unsolved) =
        read_tt_snapshot(&mut std::io::Cursor::new(&bytes)).expect("snapshot should parse");
    assert_eq!(header.root_fen, MATE_FEN);
    assert!(header.solved_count >= 1, "expected solved entries");
    assert_eq!(header.solved_count as usize, solved.len());
    assert!(
        bytes.len() as u64 <= header.tt_size_mb as u64 * 1_048_576,
        "snapshot size {} must stay within the TT bound",
        bytes.len()
    );
    let _ = std::fs::remove_file(dump_path);
}

/// The explicit `--tt-dump-path` opt-in overrides the `--outcome-only`
/// no-artifacts default: the snapshot is still written.
#[test]
fn cli_tt_dump_path_overrides_outcome_only() {
    let dump_path = "target/tt_snapshot_test_cli_outcome_only.bin";
    let _ = std::fs::remove_file(dump_path);

    let output = Command::new(cli_bin())
        .args([
            "--fen",
            MATE_FEN,
            "--timeout",
            "1",
            "--outcome-only",
            "--tt-dump-path",
            dump_path,
        ])
        .output()
        .expect("failed to run CLI binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "CLI failed: {stdout}");
    assert!(
        stdout.contains(&format!("tt_snapshot: {dump_path} ")),
        "expected tt_snapshot line despite --outcome-only:\n{stdout}"
    );
    let bytes = std::fs::read(dump_path).expect("snapshot file should exist");
    let (header, _solved, _unsolved) =
        read_tt_snapshot(&mut std::io::Cursor::new(&bytes)).expect("snapshot should parse");
    assert!(header.solved_count >= 1);
    let _ = std::fs::remove_file(dump_path);
}

/// Without `--tt-dump-path` no snapshot line is printed (opt-in flag).
#[test]
fn cli_no_tt_dump_by_default() {
    let output = Command::new(cli_bin())
        .args(["--fen", MATE_FEN, "--timeout", "1", "--outcome-only"])
        .output()
        .expect("failed to run CLI binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "CLI failed: {stdout}");
    assert!(
        !stdout.contains("tt_snapshot:"),
        "default run must not print a tt_snapshot line:\n{stdout}"
    );
}
