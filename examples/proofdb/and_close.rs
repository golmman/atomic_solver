//! The `and-close` policy (plan8 D1, normative in plan8 §2): the
//! completion-gradient harvest. The job set is the tree's actual
//! completion work — the **missing replies** (unvisited legal replies, i.e.
//! child paths) of every active undecided open row — replacing the
//! `breadth-pns` fresh-cascade whose number-1 pool structurally starves
//! exactly this work (plan8 §1).
//!
//! **Job set.** For every undecided open row that is *active* (the plan4
//! decisions 9/10 exclusions verbatim: no proven ancestor, not
//! implied-decided — the classification is read off [`super::pns::Pns`],
//! so this policy adds no second decision logic), its unvisited legal
//! replies: no DB row (an unvisited child or a known-open ledger record).
//! Stored rows — open or proven — are not jobs.
//!
//! **Orders** ([`AndCloseOrder`], `--and-close-order`):
//! - `completion` (the standing order): rows ranked by
//!   `(missing_count asc, ply asc, path asc)`, a row's replies in
//!   path-lexicographic order — the near-closure head first;
//! - `fresh`: only never-visited replies (no ledger record, or record at
//!   `passes_failed = 0`), ordered by `(reply ply asc, path asc)` — the
//!   unmeasured tail, shallow-first.
//!
//! **Budgets.** Per-reply geometric ladder read from the ledger with the
//! **strict-growth floor** (plan9 §2, superseding plan8 §2's monotone floor
//! for this policy): the k-th visit (`k = passes_failed + 1`) runs at
//! `max(2^(k-1) × base, 2 × work_done)` — a revisit at budget ≤ work_done
//! is a byte-identical deterministic repeat (a fresh empty-TT run
//! reproduces the original censored result) and must be impossible, so the
//! floor doubles accumulated work instead of matching it. Base = the CLI
//! `--budget-evals`.
//!
//! **Censor behavior** (the decision-11 deviation, normative for this
//! policy): a censored reply bumps its own ledger record and saves
//! atomically; it does **not** expose its children — exposure has no
//! consumer under `and-close` (the pns queue and pick-up state derive
//! fresh nodes by movegen anyway). The driver hook lives in
//! [`driver`]. An `and-close` session's ledger growth therefore equals
//! its censor count (the H1 no-exposure check).
//!
//! Census records carry `pass` and `work_before`; `number`/`kind` stay
//! null (pns-specific fields). Tests run in `tests/proofdb.rs` (which
//! includes this module verbatim; cargo does not run unit tests inside
//! example targets).
//!
//! File-size justification: ~10.1 KB — the order/ladder policy, the
//! sequence builder over the classified tree, and the census extraction
//! share one indexing scheme; tests are split out (`tests/proofdb.rs`).

use super::harvest::{Job, JobClass};
use super::pns::{Child, Kind, Pns};

pub mod census;
pub mod driver;
pub use census::{AndCloseCensus, describe_gradient};

/// The reply order (`--and-close-order`, plan8 §2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AndCloseOrder {
    /// Rows ranked by `(missing_count asc, ply asc, path asc)`; a row's
    /// replies in path-lexicographic order (the standing order).
    #[default]
    Completion,
    /// Only never-visited replies (no ledger record, or record at
    /// `passes_failed = 0`), ordered by `(reply ply asc, path asc)`.
    Fresh,
}

impl AndCloseOrder {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completion => "completion",
            Self::Fresh => "fresh",
        }
    }

    /// Parse the `--and-close-order` value.
    ///
    /// # Errors
    /// An unknown order name.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "completion" => Ok(Self::Completion),
            "fresh" => Ok(Self::Fresh),
            other => Err(format!(
                "unknown and-close order {other:?} (expected completion | fresh)"
            )),
        }
    }
}

/// Per-reply budget: the geometric ladder with the strict-growth floor
/// (plan9 §2, superseding the monotone floor of plan8 §2 for this policy).
/// The k-th visit of a reply (`k = passes_failed + 1`) runs at
/// `max(2^(k-1) × base, 2 × work_done)`: a revisit at budget ≤ work_done
/// is a byte-identical deterministic repeat (a fresh empty-TT run capped by
/// cumulative child-evals reproduces the original censored result — plan8
/// H5 proved the replay) and must be impossible, so the floor doubles the
/// accumulated work instead of matching it. Fresh replies (pass 0, work 0)
/// stay at the base; deep-censored replies climb geometrically in
/// cumulative work (1B → 2B → 6B → 12B …). Base = the CLI
/// `--budget-evals`.
#[must_use]
pub fn ladder_budget(passes_failed: u32, work_done: u64, base: u64) -> u64 {
    let k = u64::from(passes_failed).min(63);
    (1u64 << k)
        .saturating_mul(base)
        .max(work_done.saturating_mul(2))
}

/// The extraction output: the ordered job sequence, the census, and the
/// completion-gradient head (rows in completion order; reported for both
/// orders — it describes the tree's completion work).
#[derive(Debug)]
pub struct AndCloseBuild {
    pub jobs: Vec<Job>,
    pub census: AndCloseCensus,
    /// `(path, missing_count)` in completion order (the gradient head
    /// input; rows with 0 missing replies included — their completion work
    /// sits in their stored open children, so they generate no jobs).
    pub gradient: Vec<(String, usize)>,
}

/// One active open row's missing replies (the intermediate extraction).
struct RowWork {
    path: Vec<String>,
    ply: usize,
    /// `(uci, ledger-censored, passes_failed, work_done)` in path-lex
    /// order (movegen order is normalized by the sort below).
    replies: Vec<(String, bool, u32, u64)>,
}

/// Build the `and-close` job sequence over the classified tree (`pns` from
/// [`super::pns::Pns::build`]), the reply order, and the base budget.
/// Read-only over the tree except for the lazy [`Pns::expand`] movegen of
/// active rows (idempotent); the ledger is read, never written.
///
/// # Errors
/// Anything the tree replay/movegen rejects (a DB defect — abort).
pub fn build_sequence(
    pns: &mut Pns,
    order: AndCloseOrder,
    base: u64,
) -> Result<AndCloseBuild, String> {
    let mut census = AndCloseCensus::default();
    let mut rows: Vec<RowWork> = Vec::new();
    for i in 0..pns.nodes.len() {
        if pns.nodes[i].kind != Kind::OpenRow {
            continue;
        }
        census.rows_open += 1;
        if !pns.nodes[i].active {
            census.excluded[0] += 1;
            continue;
        }
        let (pn, _) = pns.numbers(i);
        if pn == 0 {
            census.excluded[1] += 1;
            continue;
        }
        if pn == super::pns::INF {
            census.excluded[2] += 1;
            continue;
        }
        census.active_rows += 1;
        let path = pns.nodes[i].path.clone();
        let ply = pns.nodes[i].ply;
        pns.expand(i)?;
        let mut replies = Vec::new();
        for (uci, child) in &pns.nodes[i].children {
            let (censored, passes, work) = match child {
                Child::Row(_) => continue, // stored rows are not jobs
                Child::Unvisited => (false, 0, 0),
                Child::LedgerNode(j) => {
                    let p = pns.nodes[*j].passes;
                    (
                        p > 0,
                        p,
                        pns.ledger
                            .get(&pns.nodes[*j].key)
                            .map_or(0, |e| e.work_done),
                    )
                }
            };
            if censored {
                census.replies_censored += 1;
            } else {
                census.replies_fresh += 1;
            }
            replies.push((uci.clone(), censored, passes, work));
        }
        replies.sort_by(|a, b| a.0.cmp(&b.0));
        rows.push(RowWork { path, ply, replies });
    }
    // Completion gradient: (missing_count asc, ply asc, path asc).
    rows.sort_by(|a, b| {
        a.replies
            .len()
            .cmp(&b.replies.len())
            .then_with(|| a.ply.cmp(&b.ply))
            .then_with(|| a.path.join(" ").cmp(&b.path.join(" ")))
    });
    let gradient: Vec<(String, usize)> = rows
        .iter()
        .map(|r| (r.path.join(" "), r.replies.len()))
        .collect();
    let jobs = match order {
        AndCloseOrder::Completion => rows
            .iter()
            .flat_map(|r| {
                r.replies.iter().map(move |(uci, _, passes, work)| {
                    let mut path = r.path.clone();
                    path.push(uci.clone());
                    Job {
                        budget: ladder_budget(*passes, *work, base),
                        path,
                        ply: r.ply + 1,
                        class: JobClass::OpenChild,
                        parent_bound: None,
                    }
                })
            })
            .collect(),
        AndCloseOrder::Fresh => {
            let mut jobs: Vec<Job> = rows
                .iter()
                .flat_map(|r| {
                    r.replies
                        .iter()
                        .filter_map(move |(uci, censored, passes, work)| {
                            if *censored {
                                return None;
                            }
                            let mut path = r.path.clone();
                            path.push(uci.clone());
                            Some(Job {
                                budget: ladder_budget(*passes, *work, base),
                                path,
                                ply: r.ply + 1,
                                class: JobClass::OpenChild,
                                parent_bound: None,
                            })
                        })
                })
                .collect();
            // (reply ply asc, path asc).
            jobs.sort_by(|a, b| {
                a.ply
                    .cmp(&b.ply)
                    .then_with(|| a.path.join(" ").cmp(&b.path.join(" ")))
            });
            jobs
        }
    };
    Ok(AndCloseBuild {
        jobs,
        census,
        gradient,
    })
}
