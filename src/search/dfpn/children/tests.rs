use super::*;
use crate::position::Position;
use atomic_movegen::board::StateInfo;
use atomic_movegen::types::Square;

#[test]
fn evaluate_child_terminal_win_for_parent() {
    // White rook can capture the black commoner on e8.
    let mut search = Search::new(1);
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
    let mv = Move::make_move(Square::E1, Square::E8);
    let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);

    assert_eq!(
        info.outcome,
        Some(Outcome::Loss),
        "capturing the commoner ends the game"
    );
    assert_eq!(
        info.pn, 0,
        "terminal winning child has pn 0 at the AND side"
    );
    assert_eq!(
        info.dn, INF,
        "terminal winning child has dn INF at the AND side"
    );
}

#[test]
fn evaluate_child_repetition_is_draw() {
    let mut search = Search::new(1);
    // Build a non-terminal position whose move leads to a position that is
    // already on the search path by pushing the child's repetition key before
    // evaluating it.
    let mut pos = Position::from_fen("4k3/8/8/4K3/8/8/8/4R3 w - - 0 1").unwrap();

    // Move the white king from e5 to d5; it is quiet and non-terminal.
    let mv = Move::make_move(Square::E5, Square::D5);
    pos.do_move(mv);
    let child_rep_key = pos.repetition_key();
    pos.undo_move(mv);
    search.path_push(child_rep_key);

    let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);

    assert_eq!(
        info.outcome,
        Some(Outcome::Draw),
        "repeating position is a draw"
    );
    assert!(info.repetition_seen, "repetition flag should be set");
}

#[test]
fn evaluate_child_rule50_expired_repetition_is_not_repetition_seen() {
    // The repetition key ignores the halfmove clock, so a rule50-expired
    // child can repeat a position on the search path. `outcome()` ran
    // before the repetition check in the original code, so such a child
    // was always reported as a rule50 terminal draw with
    // `repetition_seen = false`; the reordered terminal check must keep
    // that semantics (the GHI draw-suppression and selection tie-breaks
    // depend on the flag).
    let mut search = Search::new(1);
    let mut pos = Position::from_fen("4k3/8/8/4K3/8/8/8/4R3 w - - 100 1").unwrap();

    let mv = Move::make_move(Square::E5, Square::D5);
    pos.do_move(mv);
    let child_rep_key = pos.repetition_key();
    pos.undo_move(mv);
    search.path_push(child_rep_key);

    let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);

    assert_eq!(info.outcome, Some(Outcome::Draw), "rule50 draw");
    assert_eq!(info.depth, 0, "rule50 terminal is depth 0");
    assert!(
        !info.repetition_seen,
        "rule50 terminal outranks the repetition check, flag must stay false"
    );
}

#[test]
fn evaluate_child_rule50_checkmate_is_loss() {
    // A rule50-expired child that is also checkmate is a Loss: the
    // moves-empty branch outranks rule50, so the fast path must not
    // conclude a Draw before movegen. White plays Qb3-b2, delivering
    // mate to the black commoner on a1 with the halfmove clock reaching
    // 100 (same terminal position as the `fifty_move_checkmate_is_loss`
    // position test, reached as a child).
    let mut search = Search::new(1);
    let mut pos = Position::from_fen("7K/8/8/8/8/1Q6/8/k7 w - - 99 1").unwrap();
    let mv = Move::make_move(Square::B3, Square::B2);
    let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);
    assert_eq!(
        info.outcome,
        Some(Outcome::Loss),
        "rule50 checkmate must stay a Loss"
    );
    assert_eq!(info.depth, 0);
    assert!(!info.repetition_seen);
}

#[test]
fn evaluate_child_uses_solved_tt_entry() {
    let mut search = Search::new(1);
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
    // g1h1 is a quiet rook move to a non-terminal child.
    let mv = Move::make_move(Square::G1, Square::H1);
    pos.do_move(mv);
    let child_key = pos.hash();
    pos.undo_move(mv);

    // Store a solved Loss for the child position. The remaining-depth guard
    // requires remaining_depth == u32::MAX for an unbounded max_depth.
    search.tt.store(
        child_key,
        Move::NONE,
        u8::MAX,
        0,
        Some(Outcome::Loss),
        INF,
        0,
        1,
        u32::MAX,
    );

    // Force the TT lookup without any legal-move generation at the child.
    let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);
    assert_eq!(
        info.outcome,
        Some(Outcome::Loss),
        "TT-solved Loss should be reused"
    );
    assert_eq!(info.depth, 1, "TT depth should be preserved");
}

#[test]
fn evaluate_all_children_stops_at_winning_child() {
    let mut search = Search::new(1);
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
    let mut moves = MoveList::new();
    let mut state = StateInfo::new();
    pos.legal_moves_with_state(&mut moves, &mut state);

    // Sort so the winning capture e1e8 is tried first.
    search.sort_moves(&pos, &state, &mut moves, Move::NONE, true);
    let mut children = Vec::new();
    search.evaluate_all_children(&mut pos, &moves, u32::MAX, true, &mut children);

    assert_eq!(children.len(), moves.len());
    assert!(
        children[0].outcome == Some(Outcome::Loss),
        "first child should be the winning capture"
    );

    // Remaining children were filled with dummy unexplored entries.
    for c in &children[1..] {
        assert_eq!(c.pn, INF);
        assert_eq!(c.dn, 0);
        assert_eq!(c.outcome, None);
        assert!(!c.explored);
    }
}

#[test]
fn evaluate_child_checkmate_child_classifies_loss_without_movegen() {
    // White plays Qb3-b2, delivering mate to the black commoner on a1.
    // The boolean existence query finds no legal move and the checkers
    // bit classifies the child as checkmate (Loss) — same terminal
    // position as the `fifty_move_checkmate_is_loss` position test,
    // reached as a child.
    let mut search = Search::new(1);
    let mut pos = Position::from_fen("7K/8/8/8/8/1Q6/8/k7 w - - 0 1").unwrap();
    let mv = Move::make_move(Square::B3, Square::B2);
    let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);
    assert_eq!(info.outcome, Some(Outcome::Loss), "checkmate is a Loss");
    assert_eq!(info.depth, 0);
    assert!(!info.repetition_seen);
}

#[test]
fn evaluate_child_stalemate_child_classifies_draw_without_movegen() {
    // Black plays Qc3-c2, stalemat-ing the white commoner on a1: no
    // legal move and empty checkers must classify as stalemate (Draw).
    let mut search = Search::new(1);
    let mut pos = Position::from_fen("7k/8/8/8/8/2q5/8/K7 b - - 0 1").unwrap();
    let mv = Move::make_move(Square::C3, Square::C2);
    let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);
    assert_eq!(info.outcome, Some(Outcome::Draw), "stalemate is a Draw");
    assert_eq!(info.depth, 0);
    assert!(!info.repetition_seen);
}

#[test]
fn evaluate_child_two_commoners_is_draw() {
    // A K-vs-K child (occupied == 2) with legal moves and rule50 < 100
    // must be classified as a Draw via the board-static occupied == 2
    // branch, which follows the moves-empty branch in
    // `outcome_from_state` — guarding the precedence added with the
    // existence-query terminal check. White plays a1-a2; the child is
    // black to move with the two adjacent commoners.
    let mut search = Search::new(1);
    let mut pos = Position::from_fen("8/8/8/8/8/8/1k6/K7 w - - 0 1").unwrap();
    let mv = Move::make_move(Square::A1, Square::A2);
    let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);
    assert_eq!(
        info.outcome,
        Some(Outcome::Draw),
        "K vs K with legal moves is a draw"
    );
    assert_eq!(info.depth, 0);
    assert!(!info.repetition_seen);
}

#[test]
fn evaluate_child_unsolved_tt_bounds_used_when_non_degenerate() {
    let mut search = Search::new(1);
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
    // g1h1 is a quiet rook move to a non-terminal child.
    let mv = Move::make_move(Square::G1, Square::H1);
    pos.do_move(mv);
    let child_key = pos.hash();
    pos.undo_move(mv);

    // Store non-degenerate unsolved bounds with enough remaining depth.
    // max_depth=11 gives child_max_depth=10, so remaining_depth=10 is usable.
    search
        .tt
        .store(child_key, Move::NONE, u8::MAX, 1, None, 5, 5, 0, 10);

    let info = search.evaluate_child(&mut pos, mv, 11, true);
    assert_eq!(
        info.outcome, None,
        "unsolved bounds should not report an outcome"
    );
    assert_eq!(info.pn, 5);
    assert_eq!(info.dn, 5);
}

#[test]
fn evaluate_child_degenerate_tt_bounds_fall_back_to_neutral() {
    let mut search = Search::new(1);
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
    // g1h1 is a quiet rook move to a non-terminal child.
    let mv = Move::make_move(Square::G1, Square::H1);
    pos.do_move(mv);
    let child_key = pos.hash();
    pos.undo_move(mv);

    // pn == 0 is degenerate and should not be reused as an unsolved bound.
    search
        .tt
        .store(child_key, Move::NONE, u8::MAX, 1, None, 0, 5, 0, 10);

    let info = search.evaluate_child(&mut pos, mv, 11, true);
    assert_eq!(info.outcome, None);
    assert_eq!(info.pn, 1);
    assert_eq!(info.dn, 1);
}

#[test]
fn dfpn_entry_generates_into_pooled_slot() {
    // The frame-local slot lifecycle: each `dfpn` entry generates its own
    // legal moves into the pool slot at its depth and returns it at frame
    // exit. After a bounded search, the root slot (depth 1, no prefix)
    // holds the root position's legal-move list, a searched child's slot
    // (depth 2) is filled, and index 0 (never a frame depth) stays empty
    // — no parent-level pre-fill happens anywhere.
    let fen = "r7/1Rp4k/4P2B/6p1/3P2P1/p6p/P6K/8 b - - 0 35";
    let mut search = Search::new(1);
    let mut pos = Position::from_fen(fen).unwrap();
    let (outcome, _, _) = search.search_depth(&mut pos, 2);
    assert_eq!(outcome, Outcome::Draw, "depth-2 bound is not decisive");

    let mut fresh = MoveList::new();
    pos.legal_moves(&mut fresh);
    assert!(
        search.precompute_pool.len() >= 3,
        "root and child frames allocated their slots"
    );
    assert_eq!(
        search.precompute_pool[0].moves.len(),
        0,
        "depth 0 is never a frame depth"
    );
    let root_slot = &search.precompute_pool[1].moves;
    assert_eq!(
        root_slot.len(),
        fresh.len(),
        "the root frame's slot holds the root position's legal moves"
    );
    assert!(
        fresh
            .as_slice()
            .iter()
            .all(|m| root_slot.as_slice().contains(m)),
        "the root frame's slot covers every legal move (order differs: the frame sorts in place)"
    );
    assert!(
        !search.precompute_pool[2].moves.is_empty(),
        "a searched child's frame filled its own slot"
    );
}

#[test]
fn dfpn_frame_local_slots_deterministic() {
    // Behavioral check: the frame-local slot plumbing must not perturb
    // the search — solving the same position twice with fresh `Search`
    // objects yields identical work counters (and catches pool state
    // leaking through the slot lifecycle).
    let fen = "r7/1Rp4k/4P2B/6p1/3P2P1/p6p/P6K/8 b - - 0 35";
    let mut counts = Vec::new();
    for _ in 0..2 {
        let mut search = Search::new(1);
        let mut pos = Position::from_fen(fen).unwrap();
        let (outcome, pv, _) = search.solve(&mut pos);
        counts.push((outcome, pv.len(), search.child_evaluations()));
    }
    assert_eq!(counts[0], counts[1], "identical solve, identical counts");
    assert_eq!(counts[0].0, Outcome::Loss, "dec44 is a loss for black");
}

#[test]
fn precompute_pool_stays_within_active_depth_bound() {
    // Memory bound: with one slot per active depth (branching no longer
    // multiplies the pool, as in the plan2-era per-child slots), the
    // pooled movegen storage after a full solve must stay far below the
    // few-MB budget that plan2 documented for m22-scale searches. Note
    // the first-outcome phase searches unbounded depth, so the pool
    // reflects the deepest recursion ever reached, not the last chunk's
    // `max_depth_reached`.
    let fen = "r7/1Rp4k/4P2B/6p1/3P2P1/p6p/P6K/8 b - - 0 35";
    let mut search = Search::new(1);
    let mut pos = Position::from_fen(fen).unwrap();
    let _ = search.solve(&mut pos);
    let pool_len = search.precompute_pool.len();
    let pool_bytes = pool_len * std::mem::size_of::<ChildPrecompute>();
    assert!(
        pool_bytes < 1_000_000,
        "pooled movegen storage must stay well under the old per-child budget, got {pool_bytes} bytes"
    );
}

// ---- performance plan3: the TT pre-probe ladder ------------------------

/// Fixture for the pre-probe tests: a quiet, non-terminal child whose move
/// is a plain rook step.
const PREPROBE_FEN: &str = "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1";

fn preprobe_fixture() -> (Position, Move) {
    let pos = Position::from_fen(PREPROBE_FEN).unwrap();
    let mv = Move::make_move(Square::G1, Square::H1);
    (pos, mv)
}

/// `ChildInfo` has no `PartialEq`; compare the production fields one by
/// one so a divergence names its field.
fn assert_same_child_info(fast: &ChildInfo, played: &ChildInfo, ctx: &str) {
    assert_eq!(fast.mv, played.mv, "{ctx}: mv");
    assert_eq!(fast.pn, played.pn, "{ctx}: pn");
    assert_eq!(fast.dn, played.dn, "{ctx}: dn");
    assert_eq!(fast.outcome, played.outcome, "{ctx}: outcome");
    assert_eq!(fast.depth, played.depth, "{ctx}: depth");
    assert_eq!(
        fast.repetition_seen, played.repetition_seen,
        "{ctx}: repetition_seen"
    );
    assert_eq!(fast.explored, played.explored, "{ctx}: explored");
}

#[test]
fn preprobe_resolves_live_unsolved_entry_without_make() {
    // A live `outcome == None` entry decides the child from the keys alone
    // (no make/unmake, no existence query). The reused bounds and the
    // neutral consequence of a degenerate entry must both match the
    // make/unmake path for the identical entry.
    let (mut pos, mv) = preprobe_fixture();
    let fen_before = pos.fen();
    let (child_key, _, _) = pos.keys_after(mv);

    let mut search = Search::new(1);
    // `max_depth = 11` ⇒ `child_max_depth = 10`, so `remaining_depth = 10`
    // is usable and the bounds are non-degenerate.
    search
        .tt
        .store(child_key, Move::NONE, u8::MAX, 1, None, 5, 5, 0, 10);

    let fast = search.evaluate_child(&mut pos, mv, 11, true);
    assert_eq!(
        search.preprobe_skips(),
        1,
        "the pre-probe must decide a live unsolved child"
    );
    assert_eq!((fast.pn, fast.dn), (5, 5), "stored bounds are reused");
    assert_eq!(fast.outcome, None);
    assert_eq!(fast.depth, 0);
    assert!(!fast.repetition_seen && !fast.explored);
    assert_eq!(pos.fen(), fen_before, "the child move is not played");

    let entry = *search.tt.probe(child_key).expect("entry stays live");
    let played =
        search.evaluate_child_played(&mut pos, mv, 11, true, false, child_key, Some(&entry));
    assert_same_child_info(&fast, &played, "live unsolved reuse");
    assert_eq!((played.pn, played.dn), (5, 5), "played path reuses too");
    assert_eq!(pos.fen(), fen_before);
}

#[test]
fn preprobe_resolves_solved_entry_and_emits_the_same_proof_event() {
    // Identical solved payloads through the two decision paths must emit
    // the identical `NodeProven` event: the pre-probe (`best_move == NONE`,
    // decided before the make) and the shipped guarded branch (a
    // `best_move` hint whose one-ply guard does not fire, decided on the
    // played child — the pre-probe cannot evaluate that guard without the
    // child board, so it deliberately falls back).
    let (pos, mv) = preprobe_fixture();
    let (child_key, _, _) = pos.keys_after(mv);
    let hint = {
        let mut child = pos.clone();
        child.do_move(mv);
        child.legal_moves_vec()[0]
    };
    assert_ne!(hint, Move::NONE);

    let mut streams = Vec::new();
    let mut skips = Vec::new();
    for best_move in [Move::NONE, hint] {
        let mut search = Search::new(1);
        search.tt.store(
            child_key,
            best_move,
            u8::MAX,
            0,
            Some(Outcome::Loss),
            INF,
            0,
            1,
            u32::MAX,
        );
        let (tx, rx) = std::sync::mpsc::channel();
        search.set_proof_event_sender(Some(tx));
        let mut child_pos = Position::from_fen(PREPROBE_FEN).unwrap();
        let info = search.evaluate_child(&mut child_pos, mv, u32::MAX, true);
        assert_eq!(info.outcome, Some(Outcome::Loss));
        assert_eq!(info.depth, 1, "the entry's proven depth is preserved");
        skips.push(search.preprobe_skips());
        drop(search);
        streams.push(rx.iter().collect::<Vec<ProofEvent>>());
    }

    assert_eq!(skips, vec![1, 0], "only the hintless entry is a skip");
    assert_eq!(streams[0].len(), 1, "one proven child ⇒ one event");
    assert_eq!(
        format!("{:?}", streams[0]),
        format!("{:?}", streams[1]),
        "the pre-probe emits the make path's event"
    );
    // `ProofEvent` has exactly one variant, so "the search never emits a
    // reset" is enforced by the type system rather than by a match arm.
    let ProofEvent::NodeProven(event) = &streams[0][0];
    assert_eq!(event.path, vec![mv]);
    assert_eq!(event.hash, child_key, "event hash is the child key");
    assert_eq!(event.outcome, Outcome::Loss);
    assert_eq!(event.depth, 1);
}

#[test]
fn preprobe_falls_back_on_path_repetition() {
    // A path repetition is a Draw no matter what the table holds (the
    // shipped order puts the repetition branch before the TT); the ladder
    // must keep that, and the repetition branch is not a pre-probe skip.
    let mut search = Search::new(1);
    let mut pos = Position::from_fen("4k3/8/8/4K3/8/8/8/4R3 w - - 0 1").unwrap();
    let mv = Move::make_move(Square::E5, Square::D5);
    let (child_key, child_rep_key, _) = pos.keys_after(mv);
    search.tt.store(
        child_key,
        Move::NONE,
        u8::MAX,
        0,
        Some(Outcome::Win),
        0,
        INF,
        1,
        u32::MAX,
    );
    search.path_push(child_rep_key);

    let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);
    assert_eq!(
        info.outcome,
        Some(Outcome::Draw),
        "the repetition branch outranks the live solved entry"
    );
    assert!(info.repetition_seen);
    assert_eq!(info.depth, 0);
    assert_eq!(search.preprobe_skips(), 0);
}

#[test]
fn preprobe_falls_back_when_rule50_expired() {
    // A rule50-expired child is concluded by the shipped rule50 branch
    // (existence query), never by the pre-probe: its key's rule50 component
    // is clamped, so a live entry for it cannot exist. Per the shipped
    // order, a repetition of the same board must not fire either.
    let mut search = Search::new(1);
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 100 1").unwrap();
    let mv = Move::make_move(Square::G1, Square::H1);
    let (child_key, child_rep_key, rule50_after) = pos.keys_after(mv);
    assert!(rule50_after >= 100, "fixture must expire the clock");
    search.tt.store(
        child_key,
        Move::NONE,
        u8::MAX,
        0,
        Some(Outcome::Win),
        0,
        INF,
        1,
        u32::MAX,
    );
    search.path_push(child_rep_key);

    let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);
    assert_eq!(info.outcome, Some(Outcome::Draw), "rule50 draw");
    assert_eq!(info.depth, 0);
    assert!(
        !info.repetition_seen,
        "rule50 outranks the repetition check"
    );
    assert_eq!(search.preprobe_skips(), 0);
}

#[test]
fn preprobe_degenerate_bounds_fall_back_to_neutral() {
    // `pn == 0` bounds and suppressed (`remaining_depth == u32::MAX`)
    // entries must both reuse neutral (1, 1), exactly like the played path.
    for (pn, dn, remaining_depth) in [(0u64, 5u64, 10u32), (5, 5, u32::MAX)] {
        let (mut pos, mv) = preprobe_fixture();
        let (child_key, _, _) = pos.keys_after(mv);
        let mut search = Search::new(1);
        search.tt.store(
            child_key,
            Move::NONE,
            u8::MAX,
            1,
            None,
            pn,
            dn,
            0,
            remaining_depth,
        );

        let fast = search.evaluate_child(&mut pos, mv, 11, true);
        assert_eq!(
            search.preprobe_skips(),
            1,
            "a live unsolved entry is decided by the pre-probe"
        );
        assert_eq!((fast.pn, fast.dn), (1, 1), "degenerate bounds ⇒ neutral");
        assert_eq!(fast.outcome, None);

        let entry = *search.tt.probe(child_key).expect("entry stays live");
        let played =
            search.evaluate_child_played(&mut pos, mv, 11, true, false, child_key, Some(&entry));
        assert_same_child_info(&fast, &played, "degenerate entry");
    }
}

#[test]
fn preprobe_best_move_guard_matches_make_path() {
    // A solved entry whose `best_move` would repeat the path must not be
    // reused. Its one-ply guard needs the *child* board (the entry's move
    // is played from the child), which no key-level computation provides,
    // so such children always take the played path — there the shipped
    // guard fires and the child degenerates to neutral bounds, while the
    // same entry without the path push resolves normally.
    let (mut pos, mv) = preprobe_fixture();
    let (child_key, _, _) = pos.keys_after(mv);
    let hint = {
        let mut child = pos.clone();
        child.do_move(mv);
        child.legal_moves_vec()[0]
    };
    let grandchild_rep_key = {
        let mut child = pos.clone();
        child.do_move(mv);
        child.do_move(hint);
        child.repetition_key()
    };
    let seed = |search: &mut Search| {
        search.tt.store(
            child_key,
            hint,
            u8::MAX,
            0,
            Some(Outcome::Loss),
            INF,
            0,
            1,
            u32::MAX,
        );
    };

    // The guard fires: neutral bounds, played path, no pre-probe skip.
    let mut search = Search::new(1);
    seed(&mut search);
    search.path_push(grandchild_rep_key);
    let guarded = search.evaluate_child(&mut pos, mv, u32::MAX, true);
    assert_eq!(guarded.outcome, None, "the guarded entry must not resolve");
    assert_eq!((guarded.pn, guarded.dn), (1, 1));
    assert_eq!(
        search.preprobe_skips(),
        0,
        "a hinted solved entry takes the played path"
    );
    let entry = *search.tt.probe(child_key).expect("entry stays live");
    let played =
        search.evaluate_child_played(&mut pos, mv, u32::MAX, true, false, child_key, Some(&entry));
    assert_same_child_info(&guarded, &played, "guarded entry");

    // The guard does not fire: the same entry resolves, still on the
    // played path (the pre-probe never takes a hinted entry).
    let mut search = Search::new(1);
    seed(&mut search);
    let resolved = search.evaluate_child(&mut pos, mv, u32::MAX, true);
    assert_eq!(resolved.outcome, Some(Outcome::Loss));
    assert_eq!(resolved.depth, 1);
    assert_eq!(search.preprobe_skips(), 0);
}

#[test]
fn preprobe_skip_counter_resets_per_run() {
    // The test-only census counter is per-run state, like the repetition
    // cache: `begin_run` clears it so a second run's count is its own.
    let (mut pos, mv) = preprobe_fixture();
    let (child_key, _, _) = pos.keys_after(mv);
    let mut search = Search::new(1);
    search
        .tt
        .store(child_key, Move::NONE, u8::MAX, 1, None, 5, 5, 0, 10);
    let _ = search.evaluate_child(&mut pos, mv, 11, true);
    assert_eq!(search.preprobe_skips(), 1);

    search.begin_run();
    assert_eq!(search.preprobe_skips(), 0, "a new run starts from zero");
}

// ---------- move-class starting proof numbers (provari plan2) ----------

/// A quiet white knight move to a square the black rook on e7 attacks:
/// non-terminal, no target, so the classifier would call it `HangingQuiet`.
const MOVE_CLASS_HANGING_FEN: &str = "4k3/4r3/8/8/8/8/8/K5N1 w - - 0 1";

#[test]
fn move_class_default_off_keeps_neutral_bounds() {
    let mut search = Search::new(1);
    assert!(!search.move_class_pn, "the heuristic must default off");
    let mut pos = Position::from_fen(MOVE_CLASS_HANGING_FEN).unwrap();
    let mv = Move::make_move(Square::G1, Square::E2);
    let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);
    assert_eq!(info.outcome, None);
    assert_eq!((info.pn, info.dn), (1, 1), "default off is inert");
}

#[test]
fn move_class_initializes_or_child_bounds() {
    let mut search = Search::new(1);
    search.set_move_class_pn(true);
    let mut pos = Position::from_fen(MOVE_CLASS_HANGING_FEN).unwrap();
    let mv = Move::make_move(Square::G1, Square::E2);
    let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);
    assert_eq!(info.outcome, None);
    assert_eq!((info.pn, info.dn), (15, 1));
}

#[test]
fn move_class_leaves_and_children_neutral() {
    let mut search = Search::new(1);
    search.set_move_class_pn(true);
    let mut pos = Position::from_fen(MOVE_CLASS_HANGING_FEN).unwrap();
    let mv = Move::make_move(Square::G1, Square::E2);
    // Parent is AND (`is_or_node = false`): the child is OR and keeps (1, 1).
    let info = search.evaluate_child(&mut pos, mv, u32::MAX, false);
    assert_eq!(info.outcome, None);
    assert_eq!((info.pn, info.dn), (1, 1));
}

#[test]
fn move_class_does_not_overwrite_tt_bounds() {
    let mut search = Search::new(1);
    search.set_move_class_pn(true);
    let mut pos = Position::from_fen(MOVE_CLASS_HANGING_FEN).unwrap();
    let mv = Move::make_move(Square::G1, Square::E2);
    let (child_key, _, _) = pos.keys_after(mv);
    // A live outcome-less entry with non-neutral bounds, within the depth
    // guard. The pre-probe ladder reuses these bounds before classification.
    search
        .tt
        .store(child_key, Move::NONE, u8::MAX, 0, None, 5, 7, 0, 0);
    let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);
    assert_eq!(info.outcome, None);
    assert_eq!((info.pn, info.dn), (5, 7), "learned bounds must win");
}

#[test]
fn move_class_does_not_touch_solved_children() {
    let mut search = Search::new(1);
    search.set_move_class_pn(true);
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
    let mv = Move::make_move(Square::E1, Square::E8);
    let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);
    assert_eq!(info.outcome, Some(Outcome::Loss), "terminal capture");
    assert_eq!((info.pn, info.dn), (0, INF));
}

#[test]
fn move_class_check_child_uses_capped_evasions() {
    let mut search = Search::new(1);
    search.set_move_class_pn(true);
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4K2R w - - 0 1").unwrap();
    let mv = Move::make_move(Square::H1, Square::H8);
    let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);
    assert_eq!(info.outcome, None);
    assert_eq!((info.pn, info.dn), (3, 1), "Kd7/Ke7/Kf7");
}

/// With the flag on, a small position's child table must differ only in the
/// started `pn` values: every bound stays positive and finite, and AND
/// children keep the neutral `(1, 1)`.
#[test]
fn move_class_table_bounds_stay_positive_and_finite() {
    let mut search = Search::new(1);
    search.set_move_class_pn(true);
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/K5N1 w - - 0 1").unwrap();
    let mut moves = MoveList::new();
    let mut state = StateInfo::new();
    pos.legal_moves_with_state(&mut moves, &mut state);
    search.sort_moves(&pos, &state, &mut moves, Move::NONE, true);
    let mut children = Vec::new();
    search.evaluate_all_children(&mut pos, &moves, u32::MAX, true, &mut children);
    assert!(!children.is_empty());
    for child in &children {
        if child.outcome.is_none() {
            assert!(
                child.pn >= 1 && child.pn <= 15,
                "pn={} out of range",
                child.pn
            );
            assert_eq!(child.dn, 1);
        }
    }
}

/// The flag-off `evaluate_all_children` table is the shipped table: a fresh
/// search and one with the flag explicitly disabled produce identical rows,
/// and every unsolved row carries the neutral `(1, 1)`. This is the
/// regression pin for "default off leaves the child table unchanged".
#[test]
fn move_class_off_table_matches_shipped() {
    let mut default_search = Search::new(1);
    let mut disabled_search = Search::new(1);
    disabled_search.set_move_class_pn(false);
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/K5N1 w - - 0 1").unwrap();
    let mut moves = MoveList::new();
    let mut state = StateInfo::new();
    pos.legal_moves_with_state(&mut moves, &mut state);
    let sort = |search: &mut Search| {
        let mut sorted = MoveList::new();
        for i in 0..moves.len() {
            sorted.push(moves[i]);
        }
        search.sort_moves(&pos, &state, &mut sorted, Move::NONE, true);
        sorted
    };
    let default_moves = sort(&mut default_search);
    let disabled_moves = sort(&mut disabled_search);
    assert_eq!(default_moves.len(), disabled_moves.len());
    let mut a = Vec::new();
    let mut b = Vec::new();
    default_search.evaluate_all_children(&mut pos, &default_moves, u32::MAX, true, &mut a);
    disabled_search.evaluate_all_children(&mut pos, &disabled_moves, u32::MAX, true, &mut b);
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(x.mv, y.mv);
        assert_eq!(
            (x.pn, x.dn, x.outcome, x.depth),
            (y.pn, y.dn, y.outcome, y.depth)
        );
        if x.outcome.is_none() {
            assert_eq!(
                (x.pn, x.dn),
                (1, 1),
                "flag-off unsolved bounds stay neutral"
            );
        }
    }
}
