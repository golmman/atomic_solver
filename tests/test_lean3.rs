//! Lean plan 3 regression tests.
//!
//! The default-mode trajectory golden is the automated proxy called out as
//! missing by the lean plan 2 report: default-mode refinement divergence is
//! not visible in the quick suite or the first-outcome stdout, so a stored
//! golden of the m22_white default-mode run is asserted verbatim. The golden
//! was derived from the drift-verified post-plan3 binary (quick suite 59/59
//! identical `child_evals`, m22 first-outcome stdout byte-identical, m22
//! default-mode chunk `work_done`/`nodes` sequence bit-identical; see
//! `docs/plans/lean/measurements/plan3/`).

mod common;

use std::process::{Command, Stdio};

use common::cli_bin;

#[test]
#[ignore = "slow: ~20 s wall"]
fn m22_default_mode_trajectory_matches_golden() {
    let output = Command::new(cli_bin())
        .args([
            "--fen",
            "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22",
            "--timeout",
            "20",
            "--outcome-only",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("failed to run CLI binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "CLI failed: {stdout}");

    let golden = include_str!("fixtures/m22_default_stdout_golden.txt");
    assert_eq!(
        stdout, golden,
        "m22_white default-mode trajectory diverged from the stored golden"
    );
}
