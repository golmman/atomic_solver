//! Fidelity smoke tests for the `examples/portfolio` driver (plan9).
//!
//! Both tests race the plan8 campaign dec10 FEN; racers are deterministic
//! and concurrency-independent (plan8 H4b, 63/63), so the dec10 anchor
//! must reproduce *exactly* — this is a fidelity gate, not a tolerance
//! check.
//!
//! Resource note: the 4-racer race runs 4 × 128 MB TT = 512 MB peak RAM
//! (the documented driver RAM contract); wall ≈ 1–2 s per decisive race
//! and ≪ 1 s for the budget-censoring race.

use std::path::PathBuf;
use std::process::Command;

/// The plan12/plan8 campaign dec10 case, verbatim.
const DEC10_FEN: &str = "3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41";
/// The plan8 campaign dec10 winner anchor: salt 2 at exactly 2,918,636 evals.
const DEC10_WINNER_SALT: u64 = 2;
const DEC10_WINNER_EVALS: u64 = 2_918_636;

fn portfolio_bin() -> PathBuf {
    // Integration tests run from target/<profile>/deps/; examples live in
    // target/<profile>/examples/. Fail loudly — `cargo test` builds
    // examples, so absence is a gate defect, not a skip.
    let exe = std::env::current_exe().expect("current_exe");
    let profile_dir = exe
        .parent()
        .and_then(|deps| deps.parent())
        .expect("test binary profile dir");
    let path = profile_dir.join("examples").join("portfolio");
    assert!(
        path.is_file(),
        "portfolio driver missing at {}; `cargo test` must build examples",
        path.display()
    );
    path
}

fn run_portfolio(extra_args: &[&str]) -> (Vec<String>, i32) {
    let output = Command::new(portfolio_bin())
        .arg("--fen")
        .arg(DEC10_FEN)
        .args([
            "--solver",
            env!("CARGO_BIN_EXE_atomic_solver"),
            // Bounded race wall: the dec10 race settles in ~1–2 s.
            "--timeout",
            "60",
        ])
        .args(extra_args)
        .output()
        .expect("failed to run portfolio driver");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    (
        stdout.lines().map(str::to_string).collect(),
        output.status.code().expect("exit code"),
    )
}

/// The plan8 stop-when-proven discipline: a budget-censored racer is never
/// misreported as a proven draw.
#[test]
fn budget_censored_race_reports_none_and_censored_racers() {
    let (lines, code) = run_portfolio(&["--budget", "1000"]);
    assert_eq!(code, 1, "budget-censored race must exit 1: {lines:?}");
    let summary = lines
        .iter()
        .find(|l| l.starts_with("portfolio:"))
        .expect("summary line");
    assert_eq!(summary.trim_end(), "portfolio: outcome=none racers=4");
    let mut racer_lines = lines.iter().filter(|l| l.starts_with("racer:"));
    for salt in [0u64, 2, 3, 5] {
        let line = racer_lines.next().expect("four racer lines");
        assert!(
            line.starts_with(&format!("racer: salt={salt} ")),
            "racer lines sorted by salt: {line}"
        );
        assert!(
            line.contains("outcome=draw censored=budget"),
            "budget-censored racer never a proven draw: {line}"
        );
        assert!(line.contains("evals="), "evals field present: {line}");
    }
    assert_eq!(racer_lines.count(), 0, "exactly one line per racer");
}

/// The driver's fidelity gate: the 4-racer dec10 race must reproduce the
/// plan8 campaign anchor exactly (winner salt 2, 2,918,636 evals, win).
#[test]
fn dec10_race_reproduces_plan8_winner_anchor_exactly() {
    let (lines, code) = run_portfolio(&[]);
    assert_eq!(code, 0, "decisive race must exit 0: {lines:?}");
    let summary = lines
        .iter()
        .find(|l| l.starts_with("portfolio: outcome="))
        .expect("summary line");
    let expected_summary_prefix =
        format!("portfolio: outcome=win salt={DEC10_WINNER_SALT} evals={DEC10_WINNER_EVALS} wall=");
    assert!(
        summary.starts_with(&expected_summary_prefix),
        "plan8 dec10 anchor must reproduce exactly: {summary}"
    );
    assert!(
        summary.ends_with(" racers=4"),
        "default racer set has 4 members: {summary}"
    );
    // Winner's PV lines echoed verbatim after the summary.
    let summary_idx = lines.iter().position(|l| l == summary).unwrap();
    let tail = &lines[summary_idx + 1..];
    assert!(
        tail.first().is_some_and(|l| l.starts_with("pv: ")),
        "winner pv line echoed: {tail:?}"
    );
    assert!(
        tail.get(1).is_some_and(|l| l.starts_with("pv_status: ")),
        "winner pv_status line echoed: {tail:?}"
    );
    // Winner racer line agrees with the summary.
    let winner_line = lines
        .iter()
        .find(|l| l.starts_with(&format!("racer: salt={DEC10_WINNER_SALT} ")))
        .expect("winner racer line");
    assert!(
        winner_line.contains("outcome=win censored=no")
            && winner_line.contains(&format!("evals={DEC10_WINNER_EVALS}")),
        "winner racer line: {winner_line}"
    );
}
