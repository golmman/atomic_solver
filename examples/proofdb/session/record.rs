//! The census record ([`JobRecord`], plan2 D2): one `job: {…}` JSON line
//! per visit — the census input the assemble drivers parse. Split from
//! `session.rs` for the 10 KB file-size convention; the session type and
//! the job pipeline remain there.
//!
//! The `pass`/`work_before` pair is the frontier-walk census (plan16 D3:
//! the ledger visit rung and the `work_done` before the visit); `kind`
//! (`search` | `expand`) distinguishes the walk's two visit kinds.

/// One census record, emitted as a `job: {…}` JSON line (the census input).
/// The `pass`/`work_before` pair is the frontier-walk census (plan16 D3);
/// `kind` (`search` | `expand`) distinguishes the walk's two visit kinds.
pub struct JobRecord {
    pub path: String,
    pub policy: &'static str,
    pub class: &'static str,
    pub tier: &'static str,
    pub budget: u64,
    pub child_evals: u64,
    pub wall_s: f64,
    pub outcome: String,
    pub exit_reason: String,
    pub tag: Option<String>,
    pub shard_nodes: Option<usize>,
    /// The walk census: 1-based visit rung (`passes_failed + 1`).
    pub pass: Option<u32>,
    /// The walk census: ledger `work_done` before this visit.
    pub work_before: Option<u64>,
    /// The walk census: visit kind (`search` | `expand`).
    pub kind: Option<&'static str>,
}

impl JobRecord {
    pub fn emit(&self) {
        let mut o = serde_json::json!({
                "path": self.path, "policy": self.policy, "class": self.class,
                "tier": self.tier,
                "budget": self.budget, "child_evals": self.child_evals,
                "wall_s": self.wall_s, "outcome": self.outcome,
                "exit_reason": self.exit_reason, "tag": self.tag,
                "shard_nodes": self.shard_nodes,
        });
        if let Some(pass) = self.pass {
            o["pass"] = serde_json::json!(pass);
        }
        if let Some(work_before) = self.work_before {
            o["work_before"] = serde_json::json!(work_before);
        }
        if let Some(kind) = self.kind {
            o["kind"] = serde_json::json!(kind);
        }
        println!("job: {o}");
    }
}
