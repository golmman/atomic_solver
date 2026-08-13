use std::process::Command;
use std::sync::Mutex;

static RUN_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn benchmark_quick_emits_valid_json() {
    let _guard = RUN_LOCK.lock().unwrap();

    let manifest = env!("CARGO_MANIFEST_DIR");
    let temp = std::env::temp_dir().join("atomic_solver_benchmark_quick_test.json");
    let _ = std::fs::remove_file(&temp);

    let output = Command::new("cargo")
        .args([
            "run",
            "--release",
            "--quiet",
            "--example",
            "benchmark",
            "--",
            "--config",
            "config.toml",
            "--suite",
            "quick",
            "--json",
            "--first-outcome",
            "--timeout",
            "1",
            "--runs",
            "1",
            "--output-file",
        ])
        .arg(temp.to_str().unwrap())
        .current_dir(manifest)
        .output()
        .expect("failed to run benchmark example");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "benchmark exited with non-zero status: {stderr}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should contain valid JSON");

    assert_eq!(json["suite"], "quick");
    assert_eq!(json["mode"], "first-outcome");
    assert_eq!(json["timeout"], 1);
    assert_eq!(json["runs"], 1);
    assert!(json["epsilon"].as_f64().is_some());
    assert!(json["tt_size"].as_u64().is_some());
    assert!(json["config_path"].as_str().is_some());

    let results = json["results"]
        .as_array()
        .expect("results should be an array");
    assert!(!results.is_empty());
    for r in results {
        assert!(r["name"].as_str().is_some());
        let status = r["status"].as_str().expect("status should be a string");
        assert!(["ok", "timeout", "wrong", "unknown"].contains(&status));
        assert!(r["outcome"].as_str().is_some());
        assert!(r["nodes"].as_u64().is_some());
        assert!(r["child_evals"].as_u64().is_some());
        assert!(r["time_mean"].as_f64().is_some());
        assert!(r["time_min"].as_f64().is_some());
        assert!(r["time_max"].as_f64().is_some());
        assert!(r["pv_len"].as_u64().is_some());
        assert!(r["timeout"].is_boolean());
        assert!(r["wrong"].is_boolean());
    }

    let aggregates = json["aggregates"]
        .as_object()
        .expect("aggregates should be an object");
    assert!(aggregates["total_nodes"].as_u64().is_some());
    assert!(aggregates["total_child_evals"].as_u64().is_some());
    assert!(aggregates["total_time"].as_f64().is_some());
    assert!(aggregates["solved"].as_u64().is_some());
    assert!(aggregates["timeouts"].as_u64().is_some());
    assert!(aggregates["wrong"].as_u64().is_some());
    assert!(aggregates["mean_pv_len"].as_f64().is_some());

    let file_contents = std::fs::read_to_string(&temp).expect("output file should exist");
    let file_json: serde_json::Value =
        serde_json::from_str(&file_contents).expect("output file should contain valid JSON");
    assert_eq!(json, file_json, "output-file JSON should match stdout JSON");

    let _ = std::fs::remove_file(&temp);
}
