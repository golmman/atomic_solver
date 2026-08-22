use std::collections::HashSet;
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;

static RUN_LOCK: Mutex<()> = Mutex::new(());
static NEXT_TMP: AtomicUsize = AtomicUsize::new(0);

const TINY_FEN: &str = "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1";
const REQUIRED_ROW_KEYS: [&str; 5] = ["fen", "outcome", "legal_moves", "subtree_size", "children"];

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "corpus_gen_{tag}_{}_{}",
        std::process::id(),
        NEXT_TMP.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn corpus_gen_solve_load_round_trip() {
    let _guard = RUN_LOCK.lock().unwrap();
    let manifest = env!("CARGO_MANIFEST_DIR");
    let tmp = temp_dir("round_trip");

    let solve = Command::new("cargo")
        .args([
            "run",
            "--release",
            "--quiet",
            "--example",
            "corpus_gen",
            "--",
            "solve",
            "--fen",
            TINY_FEN,
            "--timeout",
            "2",
            "--tt-size",
            "16",
            "--pt-size",
            "16",
            "--dump-dir",
        ])
        .arg(&tmp)
        .current_dir(manifest)
        .output()
        .expect("failed to run corpus_gen solve");

    let stderr = String::from_utf8_lossy(&solve.stderr);
    assert!(
        solve.status.success(),
        "corpus_gen solve exited with non-zero status: {stderr}"
    );
    assert!(
        tmp.join("fen.bin").exists(),
        "solve should write fen.bin:\n{stderr}"
    );
    assert!(
        tmp.join("manifest.json").exists(),
        "solve should write manifest.json:\n{stderr}"
    );

    let out_path = tmp.join("out.ndjson");
    let load = Command::new("cargo")
        .args([
            "run",
            "--release",
            "--quiet",
            "--example",
            "corpus_gen",
            "--",
            "load",
            "--dump-dir",
        ])
        .arg(&tmp)
        .args(["--output"])
        .arg(&out_path)
        .current_dir(manifest)
        .output()
        .expect("failed to run corpus_gen load");

    let stderr = String::from_utf8_lossy(&load.stderr);
    assert!(
        load.status.success(),
        "corpus_gen load exited with non-zero status: {stderr}"
    );

    let content = std::fs::read_to_string(&out_path).unwrap();
    let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(
        !lines.is_empty(),
        "loaded NDJSON should have at least 1 line"
    );

    let meta: serde_json::Value = serde_json::from_str(lines[0]).expect("meta line should parse");
    assert_eq!(meta["_meta"], "atomic-corpus/2");
    assert!(meta["rows"].is_u64());

    let mut hashes = HashSet::new();
    for line in lines.iter().skip(1) {
        let row: serde_json::Value = serde_json::from_str(line).expect("row should parse as JSON");
        for key in REQUIRED_ROW_KEYS {
            assert!(
                row.get(key).is_some(),
                "row missing required key {key}: {row}"
            );
        }
        let children = row["children"]
            .as_array()
            .expect("children should be an array");
        for child in children {
            assert!(
                child.get("work").is_some() && child["work"].is_u64(),
                "child missing u64 work: {child}"
            );
            if row["outcome"] == "loss" {
                assert!(
                    child["work"].as_u64().unwrap() > 0,
                    "AND-child work must be > 0: {child}"
                );
            }
        }
        assert!(
            hashes.insert(
                row["hash"]
                    .as_u64()
                    .expect("hash should be an unsigned integer")
            ),
            "hash not unique: {row}"
        );
        assert!(
            row["outcome"] == "win" || row["outcome"] == "loss",
            "outcome should be win or loss: {row}"
        );
    }
    assert_eq!(
        meta["rows"].as_u64().unwrap() as usize,
        lines.len() - 1,
        "meta rows count should match the emitted row lines"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn corpus_gen_help_exits_zero() {
    let _guard = RUN_LOCK.lock().unwrap();
    let output = Command::new("cargo")
        .args([
            "run",
            "--release",
            "--quiet",
            "--example",
            "corpus_gen",
            "--",
            "-h",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("failed to run corpus_gen -h");
    assert!(output.status.success(), "corpus_gen -h should exit 0");
}

#[test]
fn corpus_gen_unknown_subcommand_exits_one() {
    let _guard = RUN_LOCK.lock().unwrap();
    let output = Command::new("cargo")
        .args([
            "run",
            "--release",
            "--quiet",
            "--example",
            "corpus_gen",
            "--",
            "frobnicate",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("failed to run corpus_gen with unknown subcommand");
    assert!(!output.status.success(), "unknown subcommand should exit 1");
}

#[test]
fn corpus_gen_unknown_option_exits_one() {
    let _guard = RUN_LOCK.lock().unwrap();
    let output = Command::new("cargo")
        .args([
            "run",
            "--release",
            "--quiet",
            "--example",
            "corpus_gen",
            "--",
            "solve",
            "--bogus",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("failed to run corpus_gen with unknown option");
    assert!(!output.status.success(), "unknown option should exit 1");
}
