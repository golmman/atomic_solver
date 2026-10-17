//! The `frontier-walk` census types (plan16 D5, peeled off `walk.rs` by the
//! plan21 D9 file-size split): the session-start root state, the census
//! struct, and the root-state classifier. The census wording shape is
//! pinned by report16; the `out_of_scope`/`scope` fields came with plan19's
//! scoped harvest and the `or_closed` field with plan21's OR-node sibling
//! skip — accepted format changes in both cases.

use super::super::db::DbContent;
use super::super::session::Session;

/// The session-start root state (the census's `root` field).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RootState {
    /// No root row in the DB (pathological — a DB always has the root).
    #[default]
    Fresh,
    /// The root is a stored open row (the fresh-layer bootstrap target).
    Open,
    /// The root is proven-stored (a proven DB row or a manifest/shard path).
    Proven,
}

impl RootState {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Open => "open",
            Self::Proven => "proven",
        }
    }
}

/// The session-start census (D5; the wording shape is pinned by report16 —
/// the `out_of_scope`/`scope` fields were added by plan19's scoped harvest,
/// the `or_closed` field by plan21's OR-node sibling skip, accepted format
/// changes in both cases).
#[derive(Debug, Default)]
pub struct WalkCensus {
    /// Ledger records at session start.
    pub records: usize,
    /// Records whose own path is a proven DB row (lineage drop).
    pub lineage: usize,
    /// Records behind a proven DB row proper prefix (shallowest reason
    /// wins; lineage wins only if no shallower prefix is proven).
    pub ancestor: usize,
    /// Records at-or-under an in-scope or-closed white-to-move parent
    /// (plan21 D3 — a proven `Loss` child row already proves the parent).
    pub or_closed: usize,
    /// Records outside the session scope (never candidates, never written;
    /// preserved in the shared ledger for later global sessions).
    pub out_of_scope: usize,
    /// Complete-marked candidates at emission time (0 at session start —
    /// the complete/decided sets are in-session state).
    pub complete: usize,
    pub root: RootState,
    /// The session's world: the scope key, or empty for a global session.
    pub scope: String,
}

impl WalkCensus {
    /// The one-line session-start census (D5).
    #[must_use]
    pub fn describe(&self, base: u64) -> String {
        format!(
            "frontier-walk: records {} (lineage {}, ancestor {}, or_closed {}, \
             out_of_scope {}, complete {}) — root {}; base budget {}; scope {}",
            self.records,
            self.lineage,
            self.ancestor,
            self.or_closed,
            self.out_of_scope,
            self.complete,
            self.root.as_str(),
            base,
            if self.scope.is_empty() {
                "global"
            } else {
                &self.scope
            },
        )
    }
}

/// The session-start root state for the census (D5): the state of the
/// scope root row (the game root for a global session) — proven-stored,
/// else a stored open row, else fresh (no row — pathological for the game
/// root, normal for a fresh scope).
#[must_use]
pub fn root_state(db: &DbContent, session: &Session, scope: &str) -> RootState {
    let root_row = db.rows.iter().find(|r| r.path.join(" ") == scope);
    if root_row.is_some_and(|r| r.outcome.is_some()) || session.has_path(scope) {
        RootState::Proven
    } else if root_row.is_some() {
        RootState::Open
    } else {
        RootState::Fresh
    }
}
