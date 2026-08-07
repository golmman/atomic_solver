mod common;

use std::process::{Command, Stdio};

use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;
use common::{cli_bin, pv_from_uci};

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
    assert!(stdout.contains("pv: e1e8"), "expected the winning PV");
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
fn cli_first_outcome_validates_and_dumps_proof_tree() {
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
        stdout.contains("ppv_valid: true"),
        "expected the proof principal variation to validate:\n{stdout}"
    );
}

fn parse_outcome(line: &str) -> Option<Outcome> {
    line.strip_prefix("outcome: ")?
        .split_whitespace()
        .next()
        .and_then(|s| match s {
            "win" => Some(Outcome::Win),
            "loss" => Some(Outcome::Loss),
            "draw" => Some(Outcome::Draw),
            _ => None,
        })
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

/// Run the CLI on a decisive FEN, parse the returned PV, and validate it with
/// `Search::validate_pv`. This exercises the full public CLI surface and the
/// binary proof-tree path.
#[test]
fn cli_pv_validates_for_decisive_position() {
    let fen = "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1";
    let output = Command::new(cli_bin())
        .args(["--fen", fen, "--timeout", "1", "--outcome-only"])
        .output()
        .expect("failed to run CLI binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "CLI failed: {stdout}");

    let outcome = stdout
        .lines()
        .find(|l| l.starts_with("outcome: "))
        .and_then(parse_outcome)
        .expect("expected a decisive outcome");

    let pv_line = stdout
        .lines()
        .find(|l| l.starts_with("pv: "))
        .expect("expected a pv line");
    let pv_uci: Vec<String> = pv_line
        .strip_prefix("pv: ")
        .unwrap()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    let pos = Position::from_fen(fen).unwrap();
    let pv = pv_from_uci(&pos, &pv_uci);
    assert!(Search::validate_pv(&pv, &pos, outcome, None));
}
