//! Offline proof-tree reconstruction from the root FEN plus a TT snapshot.
//!
//! This file is slightly larger than the 10 KiB guideline because the format
//! documentation (resolution order and soundness argument) and the public
//! surface (stats/config/output types, seeding, clock-scan, classification,
//! and the signature oracle) form one contract that the walker implements;
//! splitting the docs from the types they describe would obscure it.
//!
//! This is the decoupled proof pipeline's builder: it consumes the bounded
//! artifact written by [`crate::tt_snapshot::write_tt_snapshot`] and rebuilds
//! a proof tree without re-running the original search. The walk is top-down
//! from the root FEN: static terminal classification, snapshot lookups, hole
//! classification, and hole filling via prefix-aware local solves seeded with
//! the snapshot. [`crate::proof_event::ProofEvent`]s are synthesized into the
//! regular proof-tree
//! worker ([`crate::proof_tree::ProofTreeWorkerHandle`]), so dummy-parent
//! traversal, canonical finalization, and the binary dump are exactly the
//! live pipeline's.
//!
//! # Node resolution order (per needed node)
//!
//! 1. static terminal (no TT dependency), via `Position::outcome_from_state`;
//! 2. exact snapshot hit (full Zobrist key incl. halfmove clock);
//! 3. clock-scan (board key XOR `rule50_key(c)` for `c in 0..=100`): a
//!    Win/Loss record is adopted (`clock_hit`); a Draw record is *not*
//!    adopted (it may be a clock-dependent rule50 draw) and becomes a hole
//!    (`clock_miss_draw`);
//! 4. repetition context: the node's board key is already on the walk
//!    prefix, so the live search scored a first-player-loss GHI draw and
//!    never cached it as solved — by the proof-path soundness argument this
//!    cannot be a child of a node in a valid proof; counted as `repetition`
//!    and treated as an anomaly;
//! 5. absent → hole, filled by a bounded local solve
//!    ([`Search::search_depth_with_prefix`]) that preserves the walk's
//!    repetition history.
//!
//! # Soundness of the walk
//!
//! A proof-tree node is a unique move path from the root, and the halfmove
//! clock is a function of that path, so exact-key lookups along proof paths
//! hit whenever the snapshot entry physically survived. A Win node descends
//! exactly one child (its `best_move`); a Loss node expands *all* legal
//! replies. Every non-terminal child of a Win node must be Loss; every child
//! of a Loss node must be Win. A snapshot Draw (or repetition-context) child
//! contradicts the parent's proof and aborts the walk (correctness first).
//!
//! Reconstruction is deterministic: fills are budget-bounded by child
//! evaluations only, never wall clock.

mod walker;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};

use crate::position::Outcome;
use crate::proof_tree::ProofTree;
use crate::search::dfpn::Search;
use crate::tt_snapshot::SolvedRecord;
use crate::zobrist::rule50_key;

/// Snapshot-lookup classification of one needed proof node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoleClass {
    /// Exact full-Zobrist-key hit in the snapshot map.
    Hit,
    /// Statically terminal (no snapshot dependency).
    Terminal,
    /// Clock-scan hit adopting a Win/Loss record under a different clock.
    ClockHit,
    /// Clock-scan hit on a Draw record: not adopted (may be clock-dependent),
    /// becomes a fill hole.
    ClockMissDraw,
    /// Repetition context: the live search scored an uncached GHI draw here.
    /// Anomaly inside a valid proof.
    Repetition,
    /// No snapshot entry found; becomes a fill hole.
    Absent,
    /// A hole successfully filled by a bounded local solve.
    Filled,
    /// A hole that stayed Draw at the fill depth cap; walk aborts.
    Unfillable,
    /// Proof-structure contradiction; walk aborts.
    Anomaly,
}

/// Counters for the reconstruction walk, one per [`HoleClass`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReconstructStats {
    pub hit: u64,
    pub terminal: u64,
    pub clock_hit: u64,
    pub clock_miss_draw: u64,
    pub repetition: u64,
    pub absent: u64,
    pub filled: u64,
    pub unfillable: u64,
    pub anomalies: u64,
    /// Duplicate snapshot keys collapsed by first-wins map building.
    pub duplicate_keys: u64,
}

impl ReconstructStats {
    /// The counter for `class`.
    #[must_use]
    pub fn count(&self, class: HoleClass) -> u64 {
        match class {
            HoleClass::Hit => self.hit,
            HoleClass::Terminal => self.terminal,
            HoleClass::ClockHit => self.clock_hit,
            HoleClass::ClockMissDraw => self.clock_miss_draw,
            HoleClass::Repetition => self.repetition,
            HoleClass::Absent => self.absent,
            HoleClass::Filled => self.filled,
            HoleClass::Unfillable => self.unfillable,
            HoleClass::Anomaly => self.anomalies,
        }
    }

    /// Total number of proof nodes the walk needed to resolve. Each node is
    /// counted exactly once by its lookup class (`filled` nodes are already
    /// counted under `clock_miss_draw`/`absent`).
    #[must_use]
    pub fn total(&self) -> u64 {
        self.hit
            + self.terminal
            + self.clock_hit
            + self.clock_miss_draw
            + self.repetition
            + self.absent
    }

    /// The hole-rate breakdown line printed by the CLI:
    /// `holes: hit=<n> terminal=<n> ... anomalies=<n>`.
    #[must_use]
    pub fn holes_line(&self) -> String {
        format!(
            "holes: hit={} terminal={} clock_hit={} clock_miss_draw={} repetition={} absent={} filled={} unfillable={} anomalies={}",
            self.hit,
            self.terminal,
            self.clock_hit,
            self.clock_miss_draw,
            self.repetition,
            self.absent,
            self.filled,
            self.unfillable,
            self.anomalies
        )
    }
}

/// Budgets and sizes for a reconstruction run.
#[derive(Clone, Debug)]
pub struct ReconstructConfig {
    /// Fill-search transposition-table size in megabytes. Should be at least
    /// the snapshot's origin TT size so every seeded record fits.
    pub tt_mb: usize,
    /// Proof-tree worker memory budget in megabytes (as `--pt-size`).
    pub pt_size_mb: usize,
    /// Fill bound for the root hole (no parent depth available).
    pub fill_base: u32,
    /// Fill bound ceiling; a hole still Draw at this bound is unfillable.
    pub fill_depth_cap: u32,
    /// Per-attempt child-eval budget for one fill search.
    pub fill_attempt_budget: u64,
    /// Global child-eval budget summed over all fills; `0` = unbounded.
    pub fill_total_budget: u64,
}

impl Default for ReconstructConfig {
    fn default() -> Self {
        Self {
            tt_mb: 128,
            pt_size_mb: 256,
            fill_base: 8,
            fill_depth_cap: 32,
            fill_attempt_budget: 10_000_000,
            fill_total_budget: 0,
        }
    }
}

/// Result of one reconstruction run.
#[derive(Debug)]
pub struct ReconstructOutput {
    /// Lookup/fill/anomaly counters.
    pub stats: ReconstructStats,
    /// Child evaluations spent by all fill searches combined.
    pub fill_evals: u64,
    /// Proven root outcome, `Some` iff the walk resolved the root.
    pub root_outcome: Option<Outcome>,
    /// The finalized proof tree, `Some` iff the walk completed successfully.
    pub tree: Option<ProofTree>,
    /// Failure reason, `None` on success. A failed walk never finalizes the
    /// worker, so no dump is produced.
    pub error: Option<String>,
}

/// Build the exact-lookup map from the snapshot's solved records.
///
/// Records are in native bucket order; the snapshot is treated as a set keyed
/// by `key`. Duplicate keys (possible across generations) collapse
/// first-wins and are counted.
#[must_use]
pub fn build_solved_map(solved: &[SolvedRecord]) -> (HashMap<u64, SolvedRecord>, u64) {
    let mut map: HashMap<u64, SolvedRecord> = HashMap::with_capacity(solved.len());
    let mut duplicates: u64 = 0;
    for record in solved {
        match map.entry(record.key) {
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert(*record);
            }
            std::collections::hash_map::Entry::Occupied(_) => duplicates += 1,
        }
    }
    (map, duplicates)
}

/// Seed a search's transposition table from the snapshot's solved records.
///
/// Replicates the live search's solved-entry store exactly
/// (`remaining_depth = u32::MAX`, `pn`/`dn` from the outcome, stored under
/// the current generation), so `probe` and `resolved_from_entry` see the
/// seeded entries just like live ones. Records are seeded in snapshot native
/// order, first-wins on duplicate keys, so seeding is deterministic.
///
/// Returns the number of records seeded (duplicates skipped).
pub fn seed_search(search: &mut Search, solved: &[SolvedRecord]) -> usize {
    let mut seen: HashSet<u64> = HashSet::with_capacity(solved.len());
    let mut seeded = 0usize;
    for record in solved {
        if !seen.insert(record.key) {
            continue;
        }
        let (pn, dn) = record.outcome.to_pn_dn();
        search.tt_mut().store(
            record.key,
            record.best_move,
            u8::MAX,
            0,
            Some(record.outcome),
            pn,
            dn,
            record.depth,
            u32::MAX,
        );
        seeded += 1;
    }
    seeded
}

/// Clock-scan: find a solved record stored under the board key XOR any
/// `rule50_key(c)`, `c in 0..=100`. Only reached on exact-key misses, so a
/// hit is always a different clock than the walk's own.
#[must_use]
pub fn clock_scan(map: &HashMap<u64, SolvedRecord>, board_key: u64) -> Option<&SolvedRecord> {
    (0u16..=100).find_map(|c| map.get(&(board_key ^ rule50_key(c))))
}

/// Walk-internal classification of one needed node (see the module docs for
/// the resolution order). `rep_on_prefix` must be computed against the
/// positions *before* the node, matching the live search's
/// `path_contains` check at frame entry.
pub(crate) enum Resolution {
    Hit(SolvedRecord),
    ClockHit(SolvedRecord),
    ClockMissDraw,
    Repetition,
    Absent,
}

pub(crate) fn classify_node(
    map: &HashMap<u64, SolvedRecord>,
    hash: u64,
    board_key: u64,
    rep_on_prefix: bool,
) -> Resolution {
    if let Some(record) = map.get(&hash) {
        return Resolution::Hit(*record);
    }
    if let Some(record) = clock_scan(map, board_key) {
        return if record.outcome == Outcome::Draw {
            Resolution::ClockMissDraw
        } else {
            Resolution::ClockHit(*record)
        };
    }
    if rep_on_prefix {
        return Resolution::Repetition;
    }
    Resolution::Absent
}

/// Structure signature of a proof tree: `path → (outcome, depth)` for every
/// node. Two builds are isomorphic iff their signatures are equal. Node
/// creation order does not matter; the comparison is structural. Moves are
/// keyed by their 16-bit `move_to_bits` encoding (the dump format's move
/// code); `Move` itself is not `Hash`.
#[must_use]
pub fn tree_signature(tree: &ProofTree) -> HashMap<Vec<u16>, (Option<Outcome>, u32)> {
    let mut signature = HashMap::new();
    let mut on_path: HashSet<u64> = HashSet::new();
    dfs_signature(tree, 0, Vec::new(), &mut on_path, &mut signature);
    signature
}

fn dfs_signature(
    tree: &ProofTree,
    id: usize,
    path: Vec<u16>,
    on_path: &mut HashSet<u64>,
    signature: &mut HashMap<Vec<u16>, (Option<Outcome>, u32)>,
) {
    let node = &tree.nodes[id];
    signature.insert(path.clone(), (node.outcome, node.depth));
    // Cycle guard (defense in depth only). Trees loaded from a binary dump
    // carry `hash: 0` on every node and are acyclic by construction
    // (`parent_id < child id`), so the guard only applies to real hashes.
    let guard_active = node.hash != 0;
    if guard_active && !on_path.insert(node.hash) {
        return;
    }
    for child in tree.children(id) {
        let mut child_path = path.clone();
        child_path.push(crate::notation::move_to_bits(tree.nodes[child].mv));
        dfs_signature(tree, child, child_path, on_path, signature);
    }
    if guard_active {
        on_path.remove(&node.hash);
    }
}

/// Reconstruct a proof tree from the root FEN plus the snapshot's solved
/// records. See the module docs for the algorithm; on any failure the output
/// carries the error and the stats collected so far (no tree, no finalize).
pub fn reconstruct(
    root_fen: &str,
    solved: &[SolvedRecord],
    config: &ReconstructConfig,
) -> ReconstructOutput {
    walker::run(root_fen, solved, config)
}
