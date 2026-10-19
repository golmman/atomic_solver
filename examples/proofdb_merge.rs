//! `proofdb_merge` — the single-writer fold of the startpos proof-line
//! database (proofdb initiative, plan22: the DB is the single source of
//! truth).
//!
//! The incremental fold (`proofdb/merge/fold.rs`, one transaction per
//! invocation): open the standing DB read-write (missing file → fresh
//! schema, everything folds; a pre-v2 file aborts with the rebuild hint),
//! load the merged tree from the `nodes` rows via a replay-validated DFS,
//! fold every *unmerged* shard — the current load pipeline verbatim
//! (validate gate, binary parse, replay-validate, sidecar sha cross-check,
//! startpos replay + hash fidelity), then `graft_path` + `overlay_subtree`
//! and the depth fixpoint — and write the delta. Tags already in the
//! `shards` table are skipped idempotently after a sha256 cross-check;
//! conflicts abort without writes (the transaction rolls back).
//!
//! Shard metadata (plan22 D1/D2): each shard is self-describing — the
//! harvest writes `<tag>.bin` then `<tag>.meta.json` (atomic tmp + rename).
//! A bin without a sidecar is the interrupted job's orphan (skipped with a
//! note; the retry overwrites determinically); a sidecar without a bin is a
//! durable-layer defect. The retired manifest (`--manifest`) remains a
//! legacy metadata source: when the file exists, its entries serve as
//! metadata for unmerged tags that lack a sidecar and the merge backfills
//! their sidecars; sidecar metadata wins when both exist; a used manifest
//! entry whose bin is missing aborts.
//!
//! Censor state (plan20 D5/D6, now vacuous in place): the fold never
//! touches the DB's `censor` rows; `--import-censor <work.json>` merges a
//! legacy pre-plan20 JSON ledger into the table under the per-field max
//! rule (idempotent; never decreases a stored value) inside the same
//! transaction.
//!
//! Output: a census on stdout; a canonical dump at `--dump` when requested
//! (and `--sample-lines` spot-checks). Generated state defaults wholly
//! under `data/proofdb/` (plan14). Any validation defect, cross-check
//! mismatch, or proven-outcome conflict aborts with a non-zero exit and
//! writes nothing — conflicts are never patched.
//!
//! File-size justification: ~13 KB — the argument parsing, the census
//! grammar, and the sample-line RNG are one self-contained CLI flow; the
//! fold itself (loader, delta writer, dir scan, shard pipeline) lives in
//! `proofdb/merge/fold.rs` and the sidecar codec in `proofdb/sidecar.rs`.

use std::path::PathBuf;

use atomic_solver::notation::move_to_uci;
use atomic_solver::position::Position;

mod proofdb;

use proofdb::manifest::read_manifest;
use proofdb::merge::fold::{self, FoldOutput};
use proofdb::schema::canonical_dump_with_ids;
use proofdb::{DEFAULT_DB, DEFAULT_MANIFEST, DEFAULT_SHARD_DIR, create_parent_dir};

/// Deterministic splitmix64 RNG for the `--sample-lines` spot-checks.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

fn sample_lines(tree: &proofdb::merge::PathTree, n: usize) {
    let mut rng = Rng(0x2026_0928);
    for k in 1..=n {
        let mut cur = 0usize;
        let mut uci: Vec<String> = Vec::new();
        while !tree.nodes[cur].children.is_empty() {
            let kids: Vec<usize> = tree.nodes[cur].children.values().copied().collect();
            cur = kids[(rng.next() % kids.len() as u64) as usize];
            uci.push(move_to_uci(
                tree.nodes[cur].mv.expect("non-root has a move"),
            ));
        }
        let end = match tree.nodes[cur].outcome {
            None => "open".to_string(),
            Some(o) => format!(
                "{} bound {} ({})",
                o.as_str(),
                tree.depth_bound(cur).expect("proven node has a depth"),
                tree.depth_status(cur)
                    .expect("proven node has a status")
                    .as_str(),
            ),
        };
        println!(
            "sample {k}: {} ({} plies, end: {end})",
            uci.join(" "),
            uci.len()
        );
    }
}

fn fail(msg: &str) -> ! {
    eprintln!("proofdb_merge: CONFLICT/DEFECT: {msg}");
    std::process::exit(2);
}

fn usage() -> ! {
    eprintln!(
        "usage: proofdb_merge [--manifest <index.json>] [--shard-dir <dir>] \
         [--db <out.db>] [--dump <nodes.txt>] [--sample-lines <n>] \
         [--import-censor <work.json>]  \
         (defaults: --shard-dir data/proofdb/shards --db data/proofdb/proofdb.db; \
         --manifest is a legacy metadata source only — since plan22 each shard \
         carries its own <tag>.meta.json sidecar, and the merge backfills \
         sidecars from manifest entries for unmerged tags that lack one)"
    );
    std::process::exit(1);
}

struct Args {
    manifest: PathBuf,
    shard_dir: PathBuf,
    db: PathBuf,
    dump: Option<PathBuf>,
    sample_lines: usize,
    import_censor: Option<PathBuf>,
}

fn parse_args() -> Args {
    let mut args = Args {
        manifest: PathBuf::from(DEFAULT_MANIFEST),
        shard_dir: PathBuf::from(DEFAULT_SHARD_DIR),
        db: PathBuf::from(DEFAULT_DB),
        dump: None,
        sample_lines: 0,
        import_censor: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut next = || it.next().unwrap_or_else(|| usage());
        match arg.as_str() {
            "--manifest" => args.manifest = PathBuf::from(next()),
            "--shard-dir" => args.shard_dir = PathBuf::from(next()),
            "--db" => args.db = PathBuf::from(next()),
            "--dump" => args.dump = Some(PathBuf::from(next())),
            "--import-censor" => args.import_censor = Some(PathBuf::from(next())),
            "--sample-lines" => {
                let v = next().parse::<usize>().unwrap_or_else(|_e| usage());
                args.sample_lines = v;
            }
            _ => usage(),
        }
    }
    args
}

fn main() {
    let args = parse_args();
    // The legacy manifest (D2) is optional metadata: read only if the file
    // exists. A production layer without a manifest folds purely from
    // sidecars.
    let manifest = if args.manifest.is_file() {
        Some(read_manifest(&args.manifest).unwrap_or_else(|e| fail(&e)))
    } else {
        None
    };
    create_parent_dir(&args.db).unwrap_or_else(|e| fail(&e));
    let mut conn = rusqlite::Connection::open(&args.db)
        .unwrap_or_else(|e| fail(&format!("cannot open DB {}: {e}", args.db.display())));
    let output: FoldOutput = fold::fold(
        &mut conn,
        &args.shard_dir,
        manifest.as_ref(),
        args.import_censor.as_deref(),
    )
    .unwrap_or_else(|e| fail(&e));
    let r = &output.report;
    if args.sample_lines > 0 {
        sample_lines(&output.tree, args.sample_lines);
    }
    if let Some(dump_path) = &args.dump {
        let dump = canonical_dump_with_ids(
            Position::STARTPOS_FEN,
            &r.fold_digest,
            &output.tree,
            &output.ids,
        );
        std::fs::write(dump_path, dump)
            .unwrap_or_else(|e| fail(&format!("cannot write dump: {e}")));
    }

    let dedupes = r
        .shard_nodes_sum
        .checked_sub(r.overlay_insertions)
        .expect("overlay insertions cannot exceed shard nodes");
    println!(
        "census: folded {} shards ({} already merged, {} orphans skipped); \
         shard tree nodes {}; overlay new {}; dedupes {}; \
         open-ancestor insertions {}; open nodes upgraded {}; \
         merged nodes {} (proven {}, open {}); skeleton re-validations {}",
        r.folded,
        r.already,
        r.orphans,
        r.shard_nodes_sum,
        r.overlay_insertions,
        dedupes,
        r.ancestor_insertions,
        r.open_upgraded,
        r.node_count,
        r.proven,
        r.open,
        r.skeleton_revalidations,
    );
    println!(
        "node arithmetic: this fold {} shard nodes = {} overlay-new + {} deduped \
         (+{} open ancestors); standing nodes {}",
        r.shard_nodes_sum, r.overlay_insertions, dedupes, r.ancestor_insertions, r.node_count,
    );
    if r.sidecars_backfilled > 0 {
        println!(
            "backfill: {} sidecars written from manifest metadata",
            r.sidecars_backfilled
        );
    }
    if r.imported_censor > 0 {
        println!(
            "censor: {} imported records (max rule, in place)",
            r.imported_censor
        );
    }
    println!(
        "db: {} (nodes {}), fold digest {}",
        args.db.display(),
        r.node_count,
        &r.fold_digest[..16],
    );
}
