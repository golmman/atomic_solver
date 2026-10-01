//! The census record ([`JobRecord`], plan2 D2): one `job: {…}` JSON line
//! per visit — the census input the assemble drivers parse. Split from
//! `session.rs` for the 10 KB file-size convention; the session type and
//! the job pipeline remain there.
//!
//! The `pass`/`number`/`work_before` triple is present only for the
//! breadth-PNS policy (plan4 D2); legacy policies emit without it. The
//! `kind` field (plan6 D1: `expand` | `rung`) is likewise PNS-only and
//! traces the selection mechanism's split.

/// One census record, emitted as a `job: {…}` JSON line (the census input).
/// The `pass`/`number`/`work_before` triple is present only for the
/// breadth-PNS policy (plan4 D2); legacy policies emit without it. The
/// `kind` field (plan6 D1: `expand` | `rung`) is likewise PNS-only and
/// traces the selection mechanism's split.
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
    /// PNS census: 1-based visit rung (`passes_failed + 1`).
    pub pass: Option<u32>,
    /// PNS census: effective number at selection time.
    pub number: Option<u64>,
    /// PNS census: ledger `work_done` before this visit.
    pub work_before: Option<u64>,
    /// PNS census: visit kind (plan6 D1; `None` = legacy policy).
    pub kind: Option<&'static str>,
}

impl JobRecord {
    #[allow(clippy::too_many_lines)]
    pub fn emit(&self) {
        let mut o = serde_json::json!({
                "path": self.path, "policy": self.policy, "class": self.class,
                "parent_bound": self.parent_bound, "tier": self.tier,
                "budget": self.budget, "child_evals": self.child_evals,
                "wall_s": self.wall_s, "outcome": self.outcome,
                "exit_reason": self.exit_reason, "tag": self.tag,
                "shard_nodes": self.shard_nodes,
        });
        if let (Some(pass), Some(number), Some(work_before)) =
            (self.pass, self.number, self.work_before)
        {
            o["pass"] = serde_json::json!(pass);
            o["number"] = serde_json::json!(number);
            o["work_before"] = serde_json::json!(work_before);
        }
        if let Some(kind) = self.kind {
            o["kind"] = serde_json::json!(kind);
        }
        println!("job: {o}");
    }
}
