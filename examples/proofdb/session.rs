//! The harvest session (plan2 D2): the retained `Search` plus the job
//! pipeline (replay → budgeted prefix search → classify → shard export →
//! manifest entry). Split from the `proofdb_harvest` CLI for the 10 KB
//! file-size convention; the CLI drives this type from `main`.
//!
//! Session shape (campaign-worker precedent): one `Search` instance with a
//! private TT retained across jobs; `first_outcome_only` is on (PV
//! shortening is wasted work for shard production). Retention makes the
//! census of a session order-dependent (recorded per job); soundness is
//! unaffected — every fact crosses the boundary only through reconstruction
//! + replay validation.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use atomic_solver::position::Outcome;
use atomic_solver::reconstruct::ReconstructConfig;
use atomic_solver::search::dfpn::Search;

use super::harvest::{Job, classify, make_entry, replay_job_path};
use super::shard_export::export_validated_shard;
use super::{ShardEntry, write_manifest};

/// Abort the session (non-zero exit) on any defect: rejected, never patched.
pub fn fail(msg: &str) -> ! {
    eprintln!("proofdb_harvest: ABORT: {msg}");
    std::process::exit(2);
}

/// One census record, emitted as a `job: {…}` JSON line (the census input).
pub struct JobRecord {
    pub path: String,
    pub policy: &'static str,
    pub class: &'static str,
    pub parent_bound: Option<u32>,
    pub tier: &'static str,
    pub budget: u64,
    pub child_evals: u64,
    pub wall_s: f64,
    pub outcome: String,
    pub exit_reason: String,
    pub tag: Option<String>,
    pub shard_nodes: Option<usize>,
}

impl JobRecord {
    pub fn emit(&self) {
        println!(
            "job: {}",
            serde_json::json!({
                "path": self.path, "policy": self.policy, "class": self.class,
                "parent_bound": self.parent_bound, "tier": self.tier,
                "budget": self.budget, "child_evals": self.child_evals,
                "wall_s": self.wall_s, "outcome": self.outcome,
                "exit_reason": self.exit_reason, "tag": self.tag,
                "shard_nodes": self.shard_nodes,
            })
        );
    }
}

/// The harvest session: the retained `Search` plus the mutable state the
/// job pipeline threads through (manifest entries, collision sets, counts).
pub struct Session {
    search: Search,
    export_cfg: ReconstructConfig,
    shard_dir: PathBuf,
    entries: Vec<ShardEntry>,
    known_tags: HashSet<String>,
    known_paths: HashSet<String>,
    /// The coverage policy name (census field).
    policy: &'static str,
    /// Shard entries appended this session.
    pub new_shards: usize,
}

impl Session {
    /// Assemble the session: fresh `Search` (safety-net wall timeout; the
    /// child-eval budget is binding), export config, and the manifest state
    /// the collision asserts work against.
    pub fn new(
        tt_mb: usize,
        shard_dir: PathBuf,
        seed_entries: Vec<ShardEntry>,
        policy: &'static str,
    ) -> Self {
        let mut search = Search::new(tt_mb);
        search.set_timeout(3600);
        search.set_first_outcome_only(true);
        let export_cfg = ReconstructConfig {
            tt_mb,
            pt_size_mb: 256,
            fill_base: 8,
            fill_depth_cap: 32,
            fill_attempt_budget: 2_000_000,
            fill_total_budget: 100_000_000,
        };
        let known_tags = seed_entries.iter().map(|e| e.tag.clone()).collect();
        let known_paths = seed_entries.iter().map(|e| e.moves.join(" ")).collect();
        Self {
            search,
            export_cfg,
            shard_dir,
            entries: seed_entries,
            known_tags,
            known_paths,
            policy,
            new_shards: 0,
        }
    }

    /// Does `path` already have a shard (decided during this session)?
    pub fn has_path(&self, path: &str) -> bool {
        self.known_paths.contains(path)
    }

    /// The decisive-outcome pipeline: export → validate → write shard →
    /// entry. Aborts the session on any defect (decision 2: rejected, never
    /// patched).
    fn produce_shard(&mut self, job: &Job, sub_fen: &str, outcome: Outcome) -> (String, usize) {
        let tree = export_validated_shard(&self.search, sub_fen, &self.export_cfg)
            .unwrap_or_else(|e| fail(&format!("job {}: export rejected: {e}", job.path.join(" "))));
        let entry = make_entry(job, sub_fen, outcome);
        if !self.known_tags.insert(entry.tag.clone()) {
            fail(&format!(
                "new shard tag {} collides with a known tag",
                entry.tag
            ));
        }
        if !self.known_paths.insert(entry.moves.join(" ")) {
            fail(&format!(
                "job {} already has a shard (path collision)",
                job.path.join(" ")
            ));
        }
        let bytes = {
            let mut buf = Vec::new();
            tree.to_bin(&mut buf)
                .unwrap_or_else(|e| fail(&format!("shard serialize: {e}")));
            buf
        };
        let path = self.shard_dir.join(&entry.file);
        std::fs::write(&path, &bytes)
            .unwrap_or_else(|e| fail(&format!("cannot write {}: {e}", path.display())));
        self.entries.push(entry.clone());
        self.new_shards += 1;
        (entry.tag, tree.nodes.len())
    }

    /// Run one job at its policy-resolved screen budget: replay the
    /// startpos→node path, search under the child-eval budget, classify,
    /// and produce the shard if decisive.
    pub fn run_job(&mut self, job: &Job) -> JobRecord {
        self.run_job_with_budget(job, "screen", job.budget)
    }

    /// Run one job at an explicit budget (the heavy tier's re-run of a
    /// censored screen job at the heavy budget).
    pub fn run_job_with_budget(&mut self, job: &Job, tier: &'static str, budget: u64) -> JobRecord {
        let (mut pos, prefix) =
            replay_job_path(&job.path).unwrap_or_else(|e| fail(&format!("job replay: {e}")));
        let sub_fen = pos.fen();
        self.search.set_child_eval_budget(budget);
        let t0 = Instant::now();
        let (outcome, _depth, _nodes) =
            self.search
                .search_depth_with_prefix(&mut pos, u32::MAX, &prefix);
        let wall_s = t0.elapsed().as_secs_f64();
        let child_evals = self.search.child_evaluations();
        let exit_reason = self.search.exit_reason().to_string();
        let path_str = job.path.join(" ");
        let Some(decisive) = classify(outcome) else {
            return JobRecord {
                path: path_str,
                policy: self.policy,
                class: job.class.as_str(),
                parent_bound: job.parent_bound,
                tier,
                budget,
                child_evals,
                wall_s,
                outcome: "censored".to_string(),
                exit_reason,
                tag: None,
                shard_nodes: None,
            };
        };
        let (tag, shard_nodes) = self.produce_shard(job, &sub_fen, decisive);
        eprintln!(
            "harvest: job {} -> {} ({tier}, {} evals, shard {tag}, {shard_nodes} nodes)",
            path_str,
            decisive.as_str(),
            child_evals,
        );
        JobRecord {
            path: path_str,
            policy: self.policy,
            class: job.class.as_str(),
            parent_bound: job.parent_bound,
            tier,
            budget,
            child_evals,
            wall_s,
            outcome: decisive.as_str().to_string(),
            exit_reason,
            tag: Some(tag),
            shard_nodes: Some(shard_nodes),
        }
    }

    /// Rewrite the manifest (sorted, deterministic) only when the batch grew
    /// the shard set; an unchanged set keeps its bytes (and digest).
    pub fn rewrite_manifest(&self, manifest_path: &Path, old_digest: &str) {
        if self.new_shards > 0 {
            let digest = write_manifest(manifest_path, &self.entries).unwrap_or_else(|e| fail(&e));
            println!("manifest: {} entries, digest {digest}", self.entries.len());
        } else {
            println!(
                "manifest: unchanged ({} entries, digest {old_digest})",
                self.entries.len()
            );
        }
    }
}
