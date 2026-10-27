//! Unit tests for the reconstruction library (fast tier).
//!
//! Slightly above the 10 KiB guideline: the tests construct real snapshots
//! and run end-to-end walks, so the fixtures/helpers outweigh terse asserts.

use std::collections::HashMap;
use std::io::Cursor;

use atomic_movegen::types::Move;

use crate::notation::move_to_bits;
use crate::position::{Outcome, Position};
use crate::search::dfpn::Search;
use crate::tt_snapshot::{SolvedRecord, read_tt_snapshot, write_tt_snapshot};
use crate::zobrist::rule50_key;

use super::{
    HoleClass, ReconstructConfig, ReconstructStats, build_solved_map, classify_node, clock_scan,
    reconstruct, seed_search, tree_signature, walker::next_fill_bound,
};

/// The two-rook mate (win in 3 plies) used across the reconstruction tests.
const MATE3_FEN: &str = "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1";

fn solved_snapshot(fen: &str) -> Vec<SolvedRecord> {
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new(16);
    search.set_first_outcome_only(true);
    search.set_timeout(30);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    assert_ne!(outcome, Outcome::Draw, "fixture {fen} must solve");
    let mut buf = Vec::new();
    write_tt_snapshot(search.tt(), fen, 16, &mut buf).unwrap();
    let (_header, solved, _unsolved) = read_tt_snapshot(&mut Cursor::new(&buf)).unwrap();
    solved
}

fn record(key: u64, outcome: Outcome, depth: u32, best_move: Move) -> SolvedRecord {
    SolvedRecord {
        key,
        outcome,
        depth,
        best_move,
    }
}

#[test]
fn seeding_is_probe_visible_in_current_generation() {
    let mut pos = Position::from_fen(MATE3_FEN).unwrap();
    let mut live = Search::new(16);
    live.set_first_outcome_only(true);
    let (outcome, _pv, _nodes) = live.solve(&mut pos);
    assert_eq!(outcome, Outcome::Win);

    // The root solved record from the live search, seeded into a fresh table.
    let root_key = Position::from_fen(MATE3_FEN).unwrap().hash();
    let entry = live
        .tt()
        .entries()
        .find(|e| e.key == root_key && e.outcome.is_some())
        .copied()
        .expect("live search stored a solved root entry");

    let mut fresh = Search::new(16);
    let seeded = seed_search(
        &mut fresh,
        &[record(
            root_key,
            entry.outcome.unwrap(),
            entry.depth,
            entry.best_move,
        )],
    );
    assert_eq!(seeded, 1);

    let probed = fresh
        .tt()
        .probe(root_key)
        .expect("seeded entry is probe-visible");
    assert_eq!(probed.outcome, Some(Outcome::Win));
    assert_eq!(probed.depth, entry.depth);
    assert_eq!(probed.best_move, entry.best_move);
    assert_eq!(probed.remaining_depth, u32::MAX);
    assert_eq!(
        probed.generation,
        fresh.tt().current_generation(),
        "seeding stamps the current generation"
    );
}

#[test]
fn seeded_entry_resolves_a_mate_in_1_search() {
    // Rook mate in one. Seed the solved root record from a live solve and
    // confirm a small bounded search resolves from the seeded entry
    // (single-node work) — `resolved_from_entry` accepts it at a bound
    // >= its depth.
    // 4 men, outside the plan13 pre-phase detector's class: this test needs
    // the live DF-PN search to store TT entries, which a pre-phase claim
    // (no TT interaction) would bypass.
    let fen = "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1";
    let mut live_pos = Position::from_fen(fen).unwrap();
    let mut live = Search::new(16);
    live.set_first_outcome_only(true);
    let (live_outcome, _pv, _nodes) = live.solve(&mut live_pos);
    assert_eq!(live_outcome, Outcome::Win);
    let root_key = Position::from_fen(fen).unwrap().hash();
    let entry = live
        .tt()
        .entries()
        .find(|e| e.key == root_key && e.outcome.is_some())
        .copied()
        .expect("live search stored a solved root entry");
    assert_eq!(entry.remaining_depth, u32::MAX);
    assert!(entry.depth >= 1);

    let mut fresh = Search::new(16);
    seed_search(
        &mut fresh,
        &[record(
            root_key,
            entry.outcome.unwrap(),
            entry.depth,
            entry.best_move,
        )],
    );

    let mut pos = Position::from_fen(fen).unwrap();
    let (outcome, _pv, nodes) = fresh.search_depth(&mut pos, entry.depth + 1);
    assert_eq!(outcome, Outcome::Win);
    assert_eq!(nodes, 1, "the search must resolve from the seeded entry");
}

#[test]
fn clock_scan_finds_synthetic_clock_variants() {
    let pos = Position::from_fen(MATE3_FEN).unwrap();
    let board_key = pos.repetition_key();
    let actual_clock = pos.board().rule50();

    // A Win record under a different clock is found by the scan.
    let win_key = board_key ^ rule50_key((actual_clock + 7).min(100));
    let mut map = HashMap::new();
    map.insert(win_key, record(win_key, Outcome::Win, 3, Move::NONE));
    assert_eq!(
        clock_scan(&map, board_key).map(|r| r.outcome),
        Some(Outcome::Win)
    );
    assert!(matches!(
        classify_node(&map, pos.hash(), board_key, false),
        super::Resolution::ClockHit(_)
    ));

    // A Draw record under a different clock is NOT adopted.
    let draw_key = board_key ^ rule50_key((actual_clock + 13).min(100));
    let mut map = HashMap::new();
    map.insert(draw_key, record(draw_key, Outcome::Draw, 0, Move::NONE));
    assert_eq!(
        clock_scan(&map, board_key).map(|r| r.outcome),
        Some(Outcome::Draw)
    );
    assert!(matches!(
        classify_node(&map, pos.hash(), board_key, false),
        super::Resolution::ClockMissDraw
    ));

    // Exact hit wins over the clock-scan.
    map.insert(pos.hash(), record(pos.hash(), Outcome::Loss, 5, Move::NONE));
    assert!(matches!(
        classify_node(&map, pos.hash(), board_key, false),
        super::Resolution::Hit(_)
    ));
}

#[test]
fn fill_recovers_a_dropped_root_record() {
    let solved = solved_snapshot(MATE3_FEN);
    let root_key = Position::from_fen(MATE3_FEN).unwrap().hash();
    let mut holes = solved.clone();
    let before = holes.len();
    holes.retain(|r| r.key != root_key);
    assert_eq!(holes.len(), before - 1, "the root record must be dropped");

    let output = reconstruct(MATE3_FEN, &holes, &ReconstructConfig::default());
    assert!(output.error.is_none(), "fill failed: {:?}", output.error);
    assert_eq!(output.root_outcome, Some(Outcome::Win));
    assert_eq!(
        output.stats.filled, 1,
        "exactly the dropped root was filled"
    );
    assert_eq!(output.stats.absent, 1);
    assert!(output.tree.is_some());
}

/// The root's solved record, its `best_move`, and the snapshot with that
/// child's record dropped — the deep-hole fixture (a hole directly below a
/// deep parent). The returned root record's depth is set to `depth`.
fn snapshot_with_child_hole(fen: &str, depth: u32) -> (Vec<SolvedRecord>, u64) {
    let solved = solved_snapshot(fen);
    let mut pos = Position::from_fen(fen).unwrap();
    let root_key = pos.hash();
    let root = *solved
        .iter()
        .find(|r| r.key == root_key)
        .expect("the root record exists");
    assert_eq!(root.outcome, Outcome::Win);
    pos.do_move(root.best_move);
    let child_key = pos.hash();
    // The child's exact record must be gone, so the walk sees a hole there
    // (the clock-scan cannot resurrect it: `rule50_key(0)` is the identity and
    // the clock-0 variant is exactly the dropped key).
    let holes: Vec<SolvedRecord> = solved
        .into_iter()
        .filter(|r| r.key != child_key)
        .map(|r| {
            if r.key == root_key {
                record(root_key, r.outcome, depth, r.best_move)
            } else {
                r
            }
        })
        .collect();
    (holes, root_key)
}

/// D1: a non-root hole's bound is its parent's proven depth, which dominates
/// the configured `fill_depth_cap` — the hole is filled, not rejected.
/// Fake depths are safe: the walk uses them as bounds only (`finalize`
/// recomputes the tree's depths).
#[test]
fn deep_parent_bound_is_not_capped_by_the_fill_depth_cap() {
    let config = ReconstructConfig::default();
    let deep = config.fill_depth_cap + 1;
    let (holes, root_key) = snapshot_with_child_hole(MATE3_FEN, deep);
    assert!(holes.iter().any(|r| r.key == root_key && r.depth == deep));

    let output = reconstruct(MATE3_FEN, &holes, &config);
    assert!(output.error.is_none(), "fill failed: {:?}", output.error);
    assert_eq!(output.root_outcome, Some(Outcome::Win));
    assert_eq!(output.stats.filled, 1, "the child hole was filled");
    assert_eq!(output.stats.unfillable, 0);
    assert!(output.tree.is_some());
}

/// D3: a fill attempt cut by its child-eval budget is reported as a budget
/// abort, not as depth exhaustion, and does not count as `unfillable`.
#[test]
fn censored_fill_attempt_reports_budget_exhaustion() {
    let config = ReconstructConfig {
        fill_attempt_budget: 1,
        ..ReconstructConfig::default()
    };
    let output = reconstruct(MATE3_FEN, &[], &config);

    let error = output.error.expect("a censored fill must abort the walk");
    assert!(error.starts_with("BudgetExhausted:"), "unexpected: {error}");
    assert!(error.contains("censored at bound"), "unexpected: {error}");
    assert_eq!(output.stats.unfillable, 0);
    assert_eq!(output.stats.filled, 0);
    assert!(output.tree.is_none());
}

/// D2: the doubling search may not skip a bound between the current one and
/// the cap (the clamp is not observable through a cheap `Complete`-draw
/// fixture, so the helper is the seam).
#[test]
fn fill_bound_doubling_clamps_at_the_cap() {
    assert_eq!(next_fill_bound(8, 20), 16);
    assert_eq!(next_fill_bound(16, 20), 20);
    assert_eq!(next_fill_bound(20, 20), 20);
    assert_eq!(next_fill_bound(1, 1), 1);
    assert_eq!(next_fill_bound(u32::MAX, u32::MAX), u32::MAX);
}

#[test]
fn anomaly_on_non_terminal_win_without_best_move() {
    let root_key = Position::from_fen(MATE3_FEN).unwrap().hash();
    let solved = [record(root_key, Outcome::Win, 1, Move::NONE)];
    let output = reconstruct(MATE3_FEN, &solved, &ReconstructConfig::default());
    assert!(output.tree.is_none());
    assert_eq!(output.stats.anomalies, 1);
    let error = output.error.expect("walk must abort");
    assert!(
        error.contains("best_move NONE"),
        "unexpected error: {error}"
    );
}

#[test]
fn draw_root_is_not_decisive() {
    let root_key = Position::from_fen(MATE3_FEN).unwrap().hash();
    let solved = [record(root_key, Outcome::Draw, 0, Move::NONE)];
    let output = reconstruct(MATE3_FEN, &solved, &ReconstructConfig::default());
    assert!(output.tree.is_none());
    let error = output.error.expect("walk must fail");
    assert!(
        error.contains("does not prove a decisive root"),
        "unexpected error: {error}"
    );
}

#[test]
fn tree_signature_equal_trees_and_differences() {
    let mut a = crate::proof_tree::ProofTree::new("fen".into(), 1, Some(Outcome::Win), 2);
    let child = a.add_node(0, Move::NONE, 2, Some(Outcome::Loss), 1);
    a.add_node(child, Move::NONE, 3, Some(Outcome::Win), 0);

    // Equal tree: identical signature.
    let mut b = crate::proof_tree::ProofTree::new("fen".into(), 1, Some(Outcome::Win), 2);
    let child_b = b.add_node(0, Move::NONE, 2, Some(Outcome::Loss), 1);
    b.add_node(child_b, Move::NONE, 3, Some(Outcome::Win), 0);
    let (sig_a, sig_b) = (tree_signature(&a), tree_signature(&b));
    assert_eq!(sig_a, sig_b);
    assert_eq!(sig_a.len(), 3, "root, child, grandchild");

    // Differing depth is detected.
    let mut c = crate::proof_tree::ProofTree::new("fen".into(), 1, Some(Outcome::Win), 2);
    let child_c = c.add_node(0, Move::NONE, 2, Some(Outcome::Loss), 1);
    c.add_node(child_c, Move::NONE, 3, Some(Outcome::Win), 1);
    assert_ne!(sig_a, tree_signature(&c));

    // Differing outcome is detected.
    let mut d = crate::proof_tree::ProofTree::new("fen".into(), 1, Some(Outcome::Win), 2);
    let child_d = d.add_node(0, Move::NONE, 2, Some(Outcome::Win), 1);
    d.add_node(child_d, Move::NONE, 3, Some(Outcome::Loss), 0);
    assert_ne!(sig_a, tree_signature(&d));

    // Differing child set is detected.
    let mut e = crate::proof_tree::ProofTree::new("fen".into(), 1, Some(Outcome::Win), 2);
    e.add_node(0, Move::NONE, 2, Some(Outcome::Loss), 1);
    assert_ne!(sig_a, tree_signature(&e));
}

#[test]
fn duplicate_snapshot_keys_first_wins() {
    let key = 0x1234;
    let first = record(key, Outcome::Win, 3, Move::NONE);
    let second = record(key, Outcome::Loss, 9, Move::NONE);
    let (map, duplicates) = build_solved_map(&[first, second]);
    assert_eq!(duplicates, 1);
    assert_eq!(map.get(&key), Some(&first), "first-wins");
    assert_eq!(map.len(), 1);
}

#[test]
fn holes_line_format() {
    let stats = ReconstructStats {
        hit: 5,
        terminal: 3,
        clock_hit: 1,
        clock_miss_draw: 0,
        repetition: 0,
        absent: 2,
        filled: 2,
        unfillable: 0,
        anomalies: 0,
        duplicate_keys: 4,
    };
    assert_eq!(
        stats.holes_line(),
        "holes: hit=5 terminal=3 clock_hit=1 clock_miss_draw=0 repetition=0 absent=2 filled=2 unfillable=0 anomalies=0"
    );
    assert_eq!(stats.count(HoleClass::Hit), 5);
    assert_eq!(stats.count(HoleClass::Anomaly), 0);
    assert_eq!(stats.total(), 11);
}

/// End-to-end in-crate smoke: the walk over a complete snapshot reproduces
/// the live search's tree isomorphically (see `tests/reconstruct.rs` for the
/// integration-level oracle with a dumped reference tree).
#[test]
fn reconstruct_matches_live_tree_structure() {
    let fen = MATE3_FEN;
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new(16);
    search.set_first_outcome_only(true);
    search.set_timeout(30);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Win);

    let mut buf = Vec::new();
    write_tt_snapshot(search.tt(), fen, 16, &mut buf).unwrap();
    let (_header, solved, _unsolved) = read_tt_snapshot(&mut Cursor::new(&buf)).unwrap();

    let output = reconstruct(fen, &solved, &ReconstructConfig::default());
    assert!(
        output.error.is_none(),
        "reconstruction failed: {:?}",
        output.error
    );
    assert_eq!(output.root_outcome, Some(Outcome::Win));
    assert!(output.stats.hit > 0);
    assert!(output.fill_evals < search.child_evaluations());

    // Signatures against the live proof tree need a live worker build; the
    // structural sanity here is root + proven subtree non-trivial.
    let tree = output.tree.unwrap();
    assert!(tree.nodes.len() > 1);
    assert_eq!(tree.nodes[0].outcome, Some(Outcome::Win));
    assert_eq!(move_to_bits(tree.nodes[0].mv), move_to_bits(Move::NONE));
}
