use std::process::Command;
use std::sync::Mutex;

static RUN_LOCK: Mutex<()> = Mutex::new(());

const FEN: &str = "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26";

fn run(fen: &str, moves: &str, timeout: u64) -> (bool, String, String) {
    let _guard = RUN_LOCK.lock().unwrap();
    let manifest = env!("CARGO_MANIFEST_DIR");
    let output = Command::new("cargo")
        .args([
            "run",
            "--release",
            "--quiet",
            "--example",
            "verify_ppv",
            "--",
        ])
        .arg("--fen")
        .arg(fen)
        .arg("--moves")
        .arg(moves)
        .arg("--timeout")
        .arg(timeout.to_string())
        .current_dir(manifest)
        .output()
        .expect("failed to run verify_ppv example");

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let verified = stdout.contains("is_ppv: true") && output.status.success();
    (verified, stdout, stderr)
}

#[test]
fn illegal_move_is_not_ppv() {
    let (verified, _stdout, stderr) = run(FEN, "a1a2", 60);
    assert!(!verified, "expected is_ppv: false, got stdout with true");
    assert!(
        stderr.contains("not legal"),
        "expected illegal move error: {stderr}"
    );
}

#[test]
fn legal_non_decisive_first_move_is_not_ppv() {
    let (verified, _stdout, stderr) = run(FEN, "b1b8", 60);
    assert!(!verified);
    assert!(
        stderr.contains("not decisive"),
        "expected non-decisive error: {stderr}"
    );
}

#[test]
fn non_decisive_final_is_not_ppv() {
    let (verified, _stdout, stderr) = run(FEN, "b1b8 g8h7", 60);
    assert!(!verified);
    assert!(
        stderr.contains("not decisive"),
        "expected non-decisive error: {stderr}"
    );
}

#[test]
fn long_line_is_valid_ppv() {
    // A longer winning continuation that follows the defender's longest
    // defenses at each step; the verifier still accepts it as a PPV.
    let line = "b1b8 g8h7 b8h8 h7g7 h8h7 g7g8 h7g7 g8h8 g7g8 h8h7 g8g6";
    let (verified, stdout, _stderr) = run(FEN, line, 60);
    assert!(verified, "expected is_ppv: true, got {stdout}");
}

#[test]
fn verified_ppv_one() {
    let line = "b1b8 g8f7 c3e2 c5c4 e2f4 c4c3 f4g6";
    let (verified, stdout, _stderr) = run(FEN, line, 60);
    assert!(verified, "expected is_ppv: true, got {stdout}");
}

#[test]
fn verified_ppv_two() {
    let line = "b1b8 g8f7 c3e2 c5c4 c2c3 f7e6 e2f4 e6f5 f4g6";
    let (verified, stdout, _stderr) = run(FEN, line, 60);
    assert!(verified, "expected is_ppv: true, got {stdout}");
}

#[test]
fn mate_in_one_is_ppv() {
    let fen = "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1";
    let (verified, stdout, _stderr) = run(fen, "e1e8", 60);
    assert!(verified, "expected is_ppv: true, got {stdout}");
}
