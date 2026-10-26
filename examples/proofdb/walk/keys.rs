//! The `frontier-walk` scope-relative key algebra and drop classification
//! (plan16 D2, plan19 scope rules, plan21 D3): path-key plies, scope
//! membership, the relative/absolute conversions, and the drop/handled
//! rules the frontier construction, admission, and census share. Peeled off
//! `walk.rs` by the plan21 D9 file-size split (the census/`RootState` types
//! live in [`super::census`]).

use std::collections::HashMap;
use std::collections::HashSet;

use atomic_solver::position::Outcome;

use super::super::db::DbContent;
use super::super::ledger::Ledger;

/// Why a ledger record is not a walk candidate (D2's drops; a record can
/// carry several reasons — the shallowest decisive prefix wins).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Drop {
    /// The record's own path is a proven DB row (decision 6).
    Lineage,
    /// A proven DB row on a proper prefix (decision 9; the empty prefix
    /// included — a proven root row drops everything).
    Ancestor,
    /// At-or-under an in-scope or-closed white-to-move parent (plan21 D3):
    /// a proven `Loss` DB-row child already proves the parent — one
    /// winning move per OR node, the sibling corridors need no work.
    OrClosed,
}

/// The D2 drop rule for one key: `Some(Lineage)` iff the key's own path
/// is a proven row, `Some(Ancestor)` iff a proper prefix (the empty
/// prefix included) is, `Some(OrClosed)` iff a prefix (self included) is
/// an or-closed parent; the shallowest decisive prefix decides.
#[must_use]
pub(super) fn drop_reason(
    key: &str,
    proven: &HashSet<String>,
    or_closed: &HashSet<String>,
) -> Option<Drop> {
    let parts: Vec<&str> = if key.is_empty() {
        Vec::new()
    } else {
        key.split(' ').collect()
    };
    for i in 0..=parts.len() {
        let prefix = parts[..i].join(" ");
        if proven.contains(&prefix) {
            return Some(if i == parts.len() {
                Drop::Lineage
            } else {
                Drop::Ancestor
            });
        }
        if or_closed.contains(&prefix) {
            return Some(Drop::OrClosed);
        }
    }
    None
}

/// Is `key` at-or-under any key of `closed` (a closed ancestor-or-self —
/// the defensive admission check's input)?
#[must_use]
pub(super) fn under_closed(key: &str, closed: &HashSet<String>) -> bool {
    let parts: Vec<&str> = if key.is_empty() {
        Vec::new()
    } else {
        key.split(' ').collect()
    };
    (0..=parts.len()).any(|i| closed.contains(&parts[..i].join(" ")))
}

/// Plies of a space-joined path key (0 for the root).
pub(super) fn ply_of(key: &str) -> usize {
    if key.is_empty() {
        0
    } else {
        key.split(' ').count()
    }
}

/// The path vector of a key (empty for the root).
pub(super) fn path_of(key: &str) -> Vec<String> {
    if key.is_empty() {
        Vec::new()
    } else {
        key.split(' ').map(str::to_string).collect()
    }
}

/// Is `key` within the session scope? `""` (global) admits everything;
/// otherwise the key must equal the scope or extend it by a full move
/// token (`scope + " " + …`) — a key that merely shares a string prefix is
/// out of scope.
#[must_use]
pub(super) fn in_scope(scope: &str, key: &str) -> bool {
    scope.is_empty() || key == scope || key.strip_prefix(scope).is_some_and(|r| r.starts_with(' '))
}

/// The scope-relative path of an in-scope key: the prefix stripped, the
/// scope root itself maps to `""` (the walk-relative root).
#[must_use]
pub(super) fn rel_of<'a>(scope: &str, key: &'a str) -> &'a str {
    if scope.is_empty() {
        key
    } else if key == scope {
        ""
    } else {
        &key[scope.len() + 1..]
    }
}

/// The absolute key of a scope-relative path (the inverse of [`rel_of`]).
#[must_use]
pub(super) fn join_scope(scope: &str, rel: &str) -> String {
    if scope.is_empty() {
        rel.to_string()
    } else if rel.is_empty() {
        scope.to_string()
    } else {
        format!("{scope} {rel}")
    }
}

/// The handled rule (D3): a stored DB row (any outcome), a manifest ∪
/// in-session shard path, a proven-draw scheduling record, or a ledger
/// record with `work_done ≥ B`.
#[must_use]
pub(super) fn is_handled(
    key: &str,
    rows: &HashMap<String, Option<Outcome>>,
    ledger: &Ledger,
    base: u64,
    has_shard: impl Fn(&str) -> bool,
) -> bool {
    rows.contains_key(key)
        || has_shard(key)
        || ledger.is_drawn(key)
        || ledger.get(key).is_some_and(|e| e.work_done >= base)
}

/// All DB-row paths keyed by outcome — the expansion handled-rule's row
/// map, built once per batch (and reused by the census root state).
#[must_use]
pub(super) fn rows_map(db: &DbContent) -> HashMap<String, Option<Outcome>> {
    db.rows
        .iter()
        .map(|r| (r.path.join(" "), r.outcome))
        .collect()
}
