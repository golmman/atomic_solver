//! Harvest job-mechanics unit tests (plan2, plan22): censored
//! classification, tag determinism, sidecar shape, and the replay-prefix
//! contract. Included by `harvest.rs`; also runs via `tests/proofdb.rs`.

use super::*;
use atomic_solver::position::Position;
use atomic_solver::search::dfpn::ExitReason;

#[test]
fn draw_classification_distinguishes_proof_from_censor() {
    // A completed draw is a proof (a `drawn` scheduling row); any other
    // Draw is a fact of absence (a censor).
    assert_eq!(
        classify(Outcome::Draw, ExitReason::Complete),
        JobVerdict::ProvenDraw
    );
    assert_eq!(
        classify(Outcome::Draw, ExitReason::BudgetExhausted),
        JobVerdict::Censored
    );
    assert_eq!(
        classify(Outcome::Draw, ExitReason::Timeout),
        JobVerdict::Censored
    );
    assert_eq!(
        classify(Outcome::Win, ExitReason::Complete),
        JobVerdict::Decisive(Outcome::Win)
    );
    assert_eq!(
        classify(Outcome::Loss, ExitReason::BudgetExhausted),
        JobVerdict::Decisive(Outcome::Loss)
    );
}

#[test]
fn quick_draw_is_a_complete_proof_not_a_censor() {
    // The `make quick_draw` position: the solver proves the draw in a few
    // hundred child-evals and exits Complete. That must classify as a
    // proven draw, not as a censored budget exhaustion.
    let mut pos = Position::from_fen("7k/5p1P/5P2/2p5/1NPB4/8/8/K7 w - - 0 1").unwrap();
    let mut search = atomic_solver::search::dfpn::Search::new(4);
    search.set_first_outcome_only(true);
    let (outcome, _, _) = search.search_depth(&mut pos, u32::MAX);
    assert_eq!(outcome, Outcome::Draw);
    assert_eq!(search.exit_reason(), ExitReason::Complete);
    assert_eq!(
        classify(outcome, search.exit_reason()),
        JobVerdict::ProvenDraw
    );
}

#[test]
fn tag_is_deterministic_hex() {
    let a = vec!["e2e4".to_string(), "e7e5".to_string()];
    let b = vec!["e2e4".to_string(), "e7e5".to_string()];
    let t = tag_for_path(&a);
    assert_eq!(t, tag_for_path(&b), "deterministic");
    assert_eq!(t.len(), 18, "h_ + 16 hex chars");
    assert!(t.starts_with("h_"));
    assert!(t[2..].chars().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(tag_for_path(&[]), tag_for_path(&a));
}

#[test]
fn entry_carries_path_fen_outcome() {
    let job = Job {
        path: vec!["e2e4".into()],
        ply: 1,
        class: JobClass::OpenChild,
        budget: 1_000_000,
    };
    let e = make_sidecar(&job, "FEN", Outcome::Loss);
    assert_eq!(e.tag, tag_for_path(&job.path));
    assert_eq!(e.file, format!("{}.bin", e.tag));
    assert_eq!(e.moves, job.path);
    assert_eq!(e.fen, "FEN");
    assert_eq!(e.outcome, Outcome::Loss);
    assert!(e.sha256.is_empty(), "the digest is filled in by the caller");
}

#[test]
fn replay_prefix_excludes_the_node_itself() {
    // Empty path (root job): no prefix at all — the startpos must not sit
    // twice on the search's own path (false repetition, see fn docs).
    let (pos, prefix) = replay_job_path(&[]).unwrap();
    assert!(prefix.is_empty());
    assert_eq!(pos.fen(), Position::STARTPOS_FEN);

    let (pos, prefix) = replay_job_path(&["e2e4".into(), "e7e5".into()]).unwrap();
    assert_eq!(prefix.len(), 2, "startpos key + key after e2e4");
    assert_ne!(prefix[0], prefix[1]);
    assert_eq!(
        pos.fen(),
        "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2"
    );
    // The node's own key is not in its prefix (the search pushes it).
    assert!(!prefix.contains(&pos.repetition_key()));
}

#[test]
fn replay_rejects_illegal_db_move() {
    assert!(replay_job_path(&["e2e5".into()]).is_err());
}
