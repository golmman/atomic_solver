use std::process::Command;
use std::sync::Mutex;

static RUN_LOCK: Mutex<()> = Mutex::new(());

const TINY_FEN: &str = "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1";

#[test]
fn work_proxy_ablation_tiny_fen_round_trip() {
    let _guard = RUN_LOCK.lock().unwrap();
    let output = Command::new("cargo")
        .args([
            "run",
            "--release",
            "--quiet",
            "--example",
            "work_proxy_ablation",
            "--",
            "--fen",
            TINY_FEN,
            "--timeout",
            "2",
            "--tt-size",
            "16",
            "--pt-size",
            "16",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("failed to run work_proxy_ablation");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "work_proxy_ablation exited non-zero: {stderr}"
    );
    // The per-case summary line must carry the pair-flip metric, with a
    // plausible percentage (0.0% on a tree with no AND nodes).
    assert!(
        stderr.contains("pair_flip="),
        "stderr should carry a per-case summary with pair_flip: {stderr}"
    );
    assert!(
        stderr.contains("coverage="),
        "stderr should carry per-case coverage: {stderr}"
    );
    assert!(
        stderr.contains("tt_agree="),
        "stderr should carry the TT cross-check tt_agree: {stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("aggregate"),
        "stdout should carry the aggregate table row: {stdout}"
    );
}

#[test]
fn work_proxy_ablation_help_exits_zero() {
    let _guard = RUN_LOCK.lock().unwrap();
    let output = Command::new("cargo")
        .args([
            "run",
            "--release",
            "--quiet",
            "--example",
            "work_proxy_ablation",
            "--",
            "-h",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("failed to run work_proxy_ablation -h");
    assert!(
        output.status.success(),
        "work_proxy_ablation -h should exit 0"
    );
}

#[test]
fn work_proxy_ablation_unknown_option_exits_one() {
    let _guard = RUN_LOCK.lock().unwrap();
    let output = Command::new("cargo")
        .args([
            "run",
            "--release",
            "--quiet",
            "--example",
            "work_proxy_ablation",
            "--",
            "--bogus",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("failed to run work_proxy_ablation with unknown option");
    assert!(!output.status.success(), "unknown option should exit 1");
}
