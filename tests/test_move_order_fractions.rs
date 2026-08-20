use std::process::Command;
use std::sync::Mutex;

static RUN_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn move_order_fractions_small_fen_emits_table() {
    let _guard = RUN_LOCK.lock().unwrap();

    let manifest = env!("CARGO_MANIFEST_DIR");
    let output = Command::new("cargo")
        .args([
            "run",
            "--release",
            "--quiet",
            "--example",
            "move_order_fractions",
            "--",
            "--fen",
            "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1",
            "--timeout",
            "1",
        ])
        .current_dir(manifest)
        .output()
        .expect("failed to run move_order_fractions example");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "move_order_fractions exited with non-zero status: {stderr}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("or_nodes="),
        "stdout should contain an or_nodes= line:\n{stdout}"
    );
    assert!(
        stdout.contains("rank"),
        "stdout should contain a rank table:\n{stdout}"
    );

    // Each table block (per case and aggregate) must have the four buckets
    // 1/2/3/>3 whose pct column sums to ~100 within rounding.
    let mut block_rows = 0usize;
    let mut block_pct = 0.0f64;
    let mut blocks_checked = 0usize;
    for line in stdout.lines() {
        if line.starts_with("===") {
            if block_rows > 0 {
                assert!(
                    (block_pct - 100.0).abs() < 1.0,
                    "pct column should sum to ~100, got {block_pct} in:\n{stdout}"
                );
                blocks_checked += 1;
            }
            block_rows = 0;
            block_pct = 0.0;
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 4 && ["1", "2", "3", ">3"].contains(&fields[0]) {
            block_rows += 1;
            if let Ok(p) = fields[2].trim_end_matches('%').parse::<f64>() {
                block_pct += p;
            }
        }
    }
    if block_rows > 0 {
        assert!(
            (block_pct - 100.0).abs() < 1.0,
            "pct column should sum to ~100, got {block_pct} in:\n{stdout}"
        );
        blocks_checked += 1;
    }

    assert!(
        blocks_checked >= 1,
        "no complete rank table found:\n{stdout}"
    );
}
