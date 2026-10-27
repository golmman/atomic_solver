//! The harvest session (plan2 D2, plan22 sidecar production): the retained
//! `Search` plus the job pipeline (replay → budgeted prefix search →
//! classify → shard export → bin + sidecar write). Split from the
//! `proofdb_harvest` CLI for the 10 KB file-size convention; the CLI drives
//! this type from `main`.
//!
//! Session shape (campaign-worker precedent): one `Search` instance with a
//! private TT retained across jobs; `first_outcome_only` is on (PV
//! shortening is wasted work for shard production). Retention makes the
//! census of a session order-dependent (recorded per job); soundness is
//! unaffected — every fact crosses the boundary only through reconstruction
//! + replay validation.
//!
//! Crash consistency (plan22 D1, superseding plan17's manifest dance): the
//! produce pipeline writes `<tag>.bin` first, then the sidecar
//! `<tag>.meta.json` (atomic tmp + rename). The write order is the commit
//! marker — a sidecar present ⇒ its bin is complete; a bin without a
//! sidecar is the interrupted job's orphan (deterministically overwritten
//! by the retry); a sidecar-write failure aborts the session with the bin
//! as the one orphan. An unexpected crash or ctrl-c therefore loses at most
//! the in-flight job's work. Censored jobs touch only the censor state (one
//! committed `censor`-table transaction per censor, plan20 — already
//! atomic); a proven draw touches only the `drawn` scheduling table.
//!
//! Justified per the file-size convention (~12 KB): the whole job pipeline
//! (replay → budgeted search → classify → shard export → durable bin +
//! sidecar write → census records) shares one `Session` state — splitting
//! it would fragment the produce ordering the crash-consistency contract
//! rests on; tests live in `tests/proofdb.rs` (cargo does not run unit
//! tests inside example targets) and `record.rs`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use atomic_solver::position::Outcome;
use atomic_solver::reconstruct::ReconstructConfig;
use atomic_solver::search::dfpn::Search;

use super::harvest::{Job, JobVerdict, classify, make_sidecar, replay_job_path, tag_for_path};
use super::shard_export::export_validated_shard;
use super::sidecar::{SidecarEntry, write_sidecar};

mod record;
pub use record::JobRecord;

/// Abort the session (non-zero exit) on any defect: rejected, never patched.
pub fn fail(msg: &str) -> ! {
    eprintln!("proofdb_harvest: ABORT: {msg}");
    std::process::exit(2);
}

/// The export pipeline's fill budgets (plan25 D4): operator knobs because a
/// budget-censored export fill aborts the session, and the escalation must not
/// need a rebuild. The defaults are the values previously hardcoded in
/// [`Session::new`], so a default invocation is behaviorally identical.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExportBudgets {
    /// Per-attempt child-eval budget of one fill search.
    pub attempt: u64,
    /// Child-eval budget summed over all fills of one export; `0` = unbounded.
    pub total: u64,
}

impl Default for ExportBudgets {
    fn default() -> Self {
        Self {
            attempt: super::harvest_args::DEFAULT_FILL_ATTEMPT_BUDGET,
            total: super::harvest_args::DEFAULT_FILL_TOTAL_BUDGET,
        }
    }
}

impl ExportBudgets {
    /// The reconstruction config these budgets drive at a given TT size. The
    /// fill base and the depth cap stay fixed: since plan25 the cap bounds
    /// **root** holes only (a non-root hole is bounded by its parent's proven
    /// depth), and a root hole fails earlier with "snapshot does not prove a
    /// decisive root".
    #[must_use]
    pub fn config(&self, tt_mb: usize) -> ReconstructConfig {
        ReconstructConfig {
            tt_mb,
            pt_size_mb: 256,
            fill_base: 8,
            fill_depth_cap: 32,
            fill_attempt_budget: self.attempt,
            fill_total_budget: self.total,
        }
    }
}

/// The harvest session: the retained `Search` plus the mutable state the
/// job pipeline threads through (collision sets, counts).
pub struct Session {
    search: Search,
    export_cfg: ReconstructConfig,
    shard_dir: PathBuf,
    known_tags: HashSet<String>,
    known_paths: HashSet<String>,
    /// The coverage policy name (census field).
    policy: &'static str,
    /// Shards produced this session.
    pub new_shards: usize,
}

impl Session {
    /// Assemble the session: fresh `Search` (safety-net wall timeout; the
    /// child-eval budget is binding), export config, and the collision
    /// sets seeded from the DB's `shards` rows (plan22 D6 — the standing
    /// layer's tags/paths; the manifest is gone). `seed_paths` holds the
    /// space-joined UCI paths of the standing shard rows.
    pub fn new(
        tt_mb: usize,
        shard_dir: PathBuf,
        seed_paths: HashSet<String>,
        policy: &'static str,
    ) -> Self {
        Self::new_with_export_budgets(
            tt_mb,
            shard_dir,
            seed_paths,
            policy,
            ExportBudgets::default(),
        )
    }

    /// [`Session::new`] with explicit export fill budgets (the plan25 knobs);
    /// the default path is bit-identical to the pre-plan25 hardcoded values.
    pub fn new_with_export_budgets(
        tt_mb: usize,
        shard_dir: PathBuf,
        seed_paths: HashSet<String>,
        policy: &'static str,
        budgets: ExportBudgets,
    ) -> Self {
        let mut search = Search::new(tt_mb);
        search.set_timeout(3600);
        search.set_first_outcome_only(true);
        let export_cfg = budgets.config(tt_mb);
        let known_tags: HashSet<String> = seed_paths
            .iter()
            .map(|p| {
                let path: Vec<String> = if p.is_empty() {
                    Vec::new()
                } else {
                    p.split(' ').map(str::to_string).collect()
                };
                tag_for_path(&path)
            })
            .collect();
        Self {
            search,
            export_cfg,
            shard_dir,
            known_tags,
            known_paths: seed_paths,
            policy,
            new_shards: 0,
        }
    }

    /// Does `path` already have a shard (decided during this session or in
    /// the standing layer)?
    pub fn has_path(&self, path: &str) -> bool {
        self.known_paths.contains(path)
    }

    /// The decisive-outcome pipeline: export → validate → write bin →
    /// write sidecar. Aborts the session on any defect (decision 2:
    /// rejected, never patched); a sidecar-write failure leaves the bin as
    /// the one orphan (plan22 D1's crash window).
    fn produce_shard(&mut self, job: &Job, sub_fen: &str, outcome: Outcome) -> (String, usize) {
        let tree = export_validated_shard(&self.search, sub_fen, &self.export_cfg)
            .unwrap_or_else(|e| fail(&format!("job {}: export rejected: {e}", job.path.join(" "))));
        let side = make_sidecar(job, sub_fen, outcome);
        if !self.known_tags.insert(side.tag.clone()) {
            fail(&format!(
                "new shard tag {} collides with a known tag",
                side.tag
            ));
        }
        if !self.known_paths.insert(side.moves.join(" ")) {
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
        // The sidecar's sha256 is the bin digest (the corruption cross-check
        // and the fold's idempotence key) — set after serialization.
        let side = SidecarEntry {
            sha256: super::digest_hex(&bytes),
            ..side
        };
        // The default shard dir lives under data/proofdb/ — create it on the
        // first shard write (plan14 D3; reading never creates anything).
        std::fs::create_dir_all(&self.shard_dir)
            .unwrap_or_else(|e| fail(&format!("cannot create {}: {e}", self.shard_dir.display())));
        let bin = self.shard_dir.join(&side.file);
        std::fs::write(&bin, &bytes)
            .unwrap_or_else(|e| fail(&format!("cannot write {}: {e}", bin.display())));
        // D1: the sidecar is the commit marker — written only after the
        // bin is complete, so an interruption here leaves a bin-only orphan.
        write_sidecar(&self.shard_dir, &side).unwrap_or_else(|e| {
            fail(&format!(
                "sidecar write after job {}: {e} (the bin {} is the one orphan; \
                 the retry overwrites determinically)",
                job.path.join(" "),
                side.file
            ))
        });
        self.new_shards += 1;
        (side.tag, tree.nodes.len())
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
        let exit = self.search.exit_reason();
        let exit_reason = exit.to_string();
        let path_str = job.path.join(" ");
        let decisive = match classify(outcome, exit) {
            JobVerdict::Decisive(decisive) => decisive,
            verdict => {
                let outcome = match verdict {
                    JobVerdict::ProvenDraw => "draw",
                    JobVerdict::Censored => "censored",
                    JobVerdict::Decisive(_) => unreachable!("handled above"),
                };
                return JobRecord {
                    path: path_str,
                    policy: self.policy,
                    class: job.class.as_str(),
                    tier,
                    budget,
                    child_evals,
                    wall_s,
                    outcome: outcome.to_string(),
                    exit_reason,
                    tag: None,
                    shard_nodes: None,
                    pass: None,
                    work_before: None,
                    kind: None,
                };
            }
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
            tier,
            budget,
            child_evals,
            wall_s,
            outcome: decisive.as_str().to_string(),
            exit_reason,
            tag: Some(tag),
            shard_nodes: Some(shard_nodes),
            pass: None,
            work_before: None,
            kind: None,
        }
    }
}
