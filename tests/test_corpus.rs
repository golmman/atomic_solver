mod common;

use std::process::Command;

use atomic_solver::position::Outcome;
use common::cli_bin;

fn parse_expected(token: &str) -> Option<Outcome> {
    token.parse::<Outcome>().ok()
}

fn run_cli(fen: &str, timeout: u64) -> Outcome {
    let output = Command::new(cli_bin())
        .args([
            "--fen",
            fen,
            "--timeout",
            &timeout.to_string(),
            "--outcome-only",
        ])
        .output()
        .expect("failed to run CLI binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "CLI failed for {fen}:\n{stdout}");

    stdout
        .lines()
        .find(|l| l.starts_with("outcome: "))
        .and_then(|l| {
            l.strip_prefix("outcome: ")?
                .split_whitespace()
                .next()
                .and_then(parse_expected)
        })
        .unwrap_or_else(|| panic!("missing outcome line for {fen}:\n{stdout}"))
}

/// Solve every position in `tests/fixtures/positions.txt` via the CLI and assert
/// the expected outcome.  The returned PV is informational and is not validated.
#[test]
#[cfg_attr(debug_assertions, ignore = "slow corpus; run with --ignored")]
fn corpus_solves_to_expected_outcome() {
    let corpus = include_str!("fixtures/positions.txt");
    let mut solved = 0;

    for line in corpus.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split(';');
        let fen = parts.next().expect("corpus line should have a FEN");
        let expected = parts
            .next()
            .and_then(parse_expected)
            .unwrap_or_else(|| panic!("corpus line should have an expected outcome: {line}"));

        let outcome = run_cli(fen, 60);
        assert_eq!(
            outcome, expected,
            "expected {expected:?} for {fen}, got {outcome:?}"
        );

        solved += 1;
    }

    assert!(solved >= 5, "corpus should contain several positions");
}
