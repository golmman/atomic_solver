mod common;

use std::process::Command;

use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;
use common::{cli_bin, pv_from_uci};

fn parse_expected(token: &str) -> Option<Outcome> {
    match token.to_lowercase().as_str() {
        "win" => Some(Outcome::Win),
        "loss" => Some(Outcome::Loss),
        "draw" => Some(Outcome::Draw),
        _ => None,
    }
}

fn parse_max_pv(token: &str) -> Option<u32> {
    token.parse().ok()
}

fn run_cli(fen: &str, timeout: u64) -> (Outcome, Vec<String>) {
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

    let outcome = stdout
        .lines()
        .find(|l| l.starts_with("outcome: "))
        .and_then(|l| {
            l.strip_prefix("outcome: ")?
                .split_whitespace()
                .next()
                .and_then(parse_expected)
        })
        .unwrap_or_else(|| panic!("missing outcome line for {fen}:\n{stdout}"));

    let pv = stdout
        .lines()
        .find(|l| l.starts_with("pv: "))
        .map(|l| {
            l.strip_prefix("pv: ")
                .unwrap()
                .split_whitespace()
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();

    (outcome, pv)
}

/// Solve every position in `tests/fixtures/positions.txt` via the CLI, assert
/// the expected outcome and PV length bound, and validate decisive PVs with
/// `Search::validate_pv`.
#[test]
#[cfg_attr(debug_assertions, ignore = "slow corpus; run with --ignored")]
fn corpus_solves_and_validates() {
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
        let max_pv = parts
            .next()
            .and_then(parse_max_pv)
            .unwrap_or_else(|| panic!("corpus line should have max_pv_plies: {line}"));

        let (outcome, pv) = run_cli(fen, 60);
        assert_eq!(
            outcome, expected,
            "expected {expected:?} for {fen}, got {outcome:?}"
        );

        if outcome != Outcome::Draw {
            let pos = Position::from_fen(fen).unwrap();
            // Terminal positions are decisive with no PV; non-terminal ones must
            // return a non-empty, length-bounded PV that validates.
            if pos.outcome().is_some() {
                assert!(
                    pv.is_empty(),
                    "terminal {fen} should not produce a PV, got {pv:?}"
                );
            } else {
                assert!(
                    !pv.is_empty() && pv.len() <= max_pv as usize,
                    "PV for {fen} should have length 1..={max_pv}, got {}",
                    pv.len()
                );
                let moves = pv_from_uci(&pos, &pv);
                assert!(
                    Search::validate_pv(&moves, &pos, outcome, None),
                    "PV validation failed for {fen}: {pv:?}"
                );
            }
        }

        solved += 1;
    }

    assert!(solved >= 5, "corpus should contain several positions");
}
