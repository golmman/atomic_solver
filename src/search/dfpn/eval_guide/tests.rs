//! Unit tests for the hand evaluation, the step, and the EF-PN scale.

use super::*;
use atomic_movegen::board::Board;

fn board(fen: &str) -> Board {
    Board::from_fen(fen).unwrap()
}

const WHITE_COMMONER: Color = Color::White;
const BLACK_COMMONER: Color = Color::Black;

// ---------- evaluation, material and step ----------

/// White knights far from both commoners contribute no attack term and are
/// not beside their own commoner (which would be a fuse for Black): the
/// evaluation is exactly their material value.
#[test]
fn material_only_scores_by_value_difference() {
    let b = board("7k/8/8/8/4N3/8/8/K7 w - - 0 1");
    assert_eq!(
        evaluation(&b, WHITE_COMMONER),
        VALUES[PieceType::Knight as usize]
    );
    assert_eq!(
        evaluation(&b, BLACK_COMMONER),
        -VALUES[PieceType::Knight as usize]
    );

    // Two knights add up; no attack term leaks in.
    let b = board("7k/8/8/8/2N1N3/8/8/K7 w - - 0 1");
    assert_eq!(
        evaluation(&b, WHITE_COMMONER),
        2 * VALUES[PieceType::Knight as usize]
    );
    assert_eq!(step(&b, WHITE_COMMONER), 2, "clearly ahead");
    assert_eq!(step(&b, BLACK_COMMONER), 0, "clearly behind");
}

/// The step is exactly `1` for a balanced position and stays monotone as the
/// score rises through a material ladder.
#[test]
fn step_is_balanced_and_monotone() {
    let balanced = board("7k/8/8/8/8/8/8/K7 w - - 0 1");
    assert_eq!(evaluation(&balanced, WHITE_COMMONER), 0);
    assert_eq!(step(&balanced, WHITE_COMMONER), 1);

    // Material ladder: (FEN, expected step for White).
    let ladder = [
        ("7k/8/8/8/8/8/8/K7 w - - 0 1", 1),     // 0
        ("7k/8/8/8/8/8/4P3/K7 w - - 0 1", 1),   // +305
        ("7k/8/8/8/4N3/8/8/K7 w - - 0 1", 2),   // +545
        ("7k/8/8/8/2N1N3/8/8/K7 w - - 0 1", 2), // +1090
    ];
    let mut observed: Vec<(i32, u8)> = ladder
        .iter()
        .map(|(fen, _)| {
            let b = board(fen);
            (evaluation(&b, WHITE_COMMONER), step(&b, WHITE_COMMONER))
        })
        .collect();
    for ((fen, expected), (score, got)) in ladder.iter().zip(&observed) {
        assert_eq!(*got, *expected, "{fen} scored {score}");
    }
    observed.sort_by_key(|(score, _)| *score);
    let steps: Vec<u8> = observed.iter().map(|(_, step)| *step).collect();
    let mut sorted = steps.clone();
    sorted.sort_unstable();
    assert_eq!(steps, sorted, "step must be monotone in the score");
}

/// The step is computed for the OR side: the same board yields complementary
/// steps for the two colours (the sign-convention pin).
#[test]
fn step_is_computed_for_the_or_side() {
    for fen in [
        "7k/8/8/8/8/8/8/K7 w - - 0 1",
        "7k/8/8/8/8/8/4P3/K7 w - - 0 1",
        "7k/8/8/8/4N3/8/8/K7 w - - 0 1",
        "r1b1kbnr/ppp3pp/n4q2/3pppN1/1P1PP3/8/P1P2PPP/RNBQKB1R w KQkq - 0 1",
    ] {
        let b = board(fen);
        let white = step(&b, WHITE_COMMONER);
        let black = step(&b, BLACK_COMMONER);
        assert_eq!(white + black, 2, "{fen}: steps {white}/{black}");
        assert_eq!(white, 2 - black);
    }
}

// ---------- attack terms ----------

fn white_terms(fen: &str) -> Terms {
    terms(&board(fen), WHITE_COMMONER)
}

/// A knight attacks one ring square holding an enemy piece: `targets` is 1;
/// a second such piece makes it 2, and a third cannot raise it further.
#[test]
fn targets_cap_at_two() {
    // A knight on c6 attacks d8 and e7 beside the black commoner e8.
    let none = white_terms("4k3/8/2N5/8/8/8/8/K7 w - - 0 1");
    assert_eq!(none.targets, 0);
    assert_eq!(none.fuses, 0);
    assert_eq!(none.pressure, 2, "Nc6 attacks d8 and e7");

    let one = white_terms("3rk3/8/2N5/8/8/8/8/K7 w - - 0 1");
    assert_eq!(one.targets, 1);
    assert_eq!(one.fuses, 1);

    let two = white_terms("3rkr2/4n3/2N5/8/8/8/8/K2R1R2 w - - 0 1");
    assert_eq!(two.targets, 2);
    assert_eq!(two.fuses, 3, "three non-commoner pieces beside e8");
    // The scoring maps the capped count to TARGET/TARGETS, not to a sum.
    assert_eq!(
        Terms {
            targets: 1,
            ..Terms::default()
        }
        .score(),
        TARGET
    );
    assert_eq!(
        Terms {
            targets: 2,
            ..Terms::default()
        }
        .score(),
        TARGETS
    );
    assert_eq!(
        Terms {
            targets: 7,
            ..Terms::default()
        }
        .score(),
        TARGETS
    );
}

/// A fuse with no attacker adds only `FUSE`: the enemy piece beside its
/// commoner is counted, but no target and no pressure.
#[test]
fn fuse_without_attacker_adds_only_fuse() {
    let bare = white_terms("4k3/8/8/8/8/8/8/K7 w - - 0 1");
    assert_eq!(bare.score(), 0);
    let fused = white_terms("4k3/4r3/8/8/8/8/8/K7 w - - 0 1");
    assert_eq!(fused.targets, 0);
    assert_eq!(fused.fuses, 1);
    assert_eq!(fused.pressure, 0);
    assert_eq!(fused.checkers, 0);
    assert_eq!(fused.score(), FUSE);
}

/// Adjacent (touching) commoners can be neither checked nor exploded, so the
/// attack terms are dropped and only material remains.
#[test]
fn touching_commoners_drop_attack_terms() {
    let fused = white_terms("8/8/8/8/8/8/4k3/4KR2 w - - 0 1");
    assert_eq!(fused.material, VALUES[PieceType::Rook as usize]);
    assert_eq!(fused.targets, 0);
    assert_eq!(fused.fuses, 0);
    assert_eq!(fused.pressure, 0);
    assert_eq!(fused.checkers, 0);
    assert_eq!(fused.score(), VALUES[PieceType::Rook as usize]);
}

/// A multi-commoner position uses the union of the rings, stays finite, and
/// is deterministic.
#[test]
fn multi_commoner_position_uses_the_union_of_rings() {
    let fen = "kr4rk/8/8/8/8/8/8/KR4R1 w - - 0 1";
    let first = white_terms(fen);
    let second = white_terms(fen);
    assert_eq!(first, second, "evaluation must be deterministic");
    assert_eq!(first.targets, 2, "b8 and g8 are targets");
    assert_eq!(first.fuses, 2, "one piece beside each black commoner");
    assert!(evaluation(&board(fen), WHITE_COMMONER).abs() < i32::MAX);
}

/// The pawn reverse-attack direction: a white pawn on c6 attacks d7, and a
/// d7 pawn checks the black commoner on e8 — so the pawn is a checker. With
/// the wrong colour the reverse set falls off the board and the term vanishes.
#[test]
fn pawn_checker_uses_the_reverse_attack_direction() {
    let checker = white_terms("4k3/8/2P5/8/8/8/8/K7 w - - 0 1");
    assert_eq!(checker.checkers, 1);
    let not_checker = white_terms("4k3/8/1P6/8/8/8/8/K7 w - - 0 1");
    assert_eq!(not_checker.checkers, 0);
}

// ---------- the scale ----------

fn scale(is_or_node: bool, step: u8, mode: EvalPnMode, pn: u64, dn: u64) -> (u64, u64) {
    ChildScale::for_parent(is_or_node, step, mode).map_or((pn, dn), |scale| scale.apply(pn, dn))
}

#[test]
fn scale_table_matches_provari() {
    // OR ("attacking") parents multiply only.
    assert_eq!(scale(true, 0, EvalPnMode::Both, 1, 1), (11, 1));
    assert_eq!(scale(true, 1, EvalPnMode::Both, 1, 1), (6, 11));
    assert_eq!(scale(true, 2, EvalPnMode::Both, 1, 1), (1, 21));
    assert_eq!(scale(true, 0, EvalPnMode::Or, 1, 1), (11, 1));
    assert_eq!(scale(true, 1, EvalPnMode::Or, 1, 1), (6, 11));
    assert_eq!(scale(true, 2, EvalPnMode::Or, 1, 1), (1, 21));

    // AND ("defending") parents add to the proof number and multiply the
    // disproof number.
    assert_eq!(scale(false, 0, EvalPnMode::Both, 1, 1), (21, 1));
    assert_eq!(scale(false, 1, EvalPnMode::Both, 1, 1), (11, 6));
    assert_eq!(scale(false, 2, EvalPnMode::Both, 1, 1), (1, 11));

    // A plan2 class number composes on the OR branch; the AND branch adds.
    assert_eq!(scale(true, 0, EvalPnMode::Both, 9, 3), (99, 3));
    assert_eq!(scale(false, 0, EvalPnMode::Both, 9, 3), (29, 3));
    assert_eq!(scale(false, 1, EvalPnMode::Both, 9, 3), (19, 18));
}

#[test]
fn scale_identity_cases() {
    // Off is inert for both node kinds.
    assert!(ChildScale::for_parent(true, 1, EvalPnMode::Off).is_none());
    assert!(ChildScale::for_parent(false, 1, EvalPnMode::Off).is_none());
    // `or` leaves AND-parent children neutral.
    assert!(ChildScale::for_parent(false, 0, EvalPnMode::Or).is_none());
    assert!(ChildScale::for_parent(false, 1, EvalPnMode::Or).is_none());
    assert!(ChildScale::for_parent(false, 2, EvalPnMode::Or).is_none());
    assert!(ChildScale::for_parent(true, 2, EvalPnMode::Or).is_some());

    assert!(!EvalPnMode::Off.is_enabled());
    assert!(EvalPnMode::Both.is_enabled());
    assert!(EvalPnMode::Or.is_enabled());
    assert_eq!(EvalPnMode::default(), EvalPnMode::Off);
}

#[test]
fn scale_never_returns_zero_and_never_overflows() {
    for mode in [EvalPnMode::Both, EvalPnMode::Or] {
        for is_or_node in [true, false] {
            for step in 0..=2 {
                let scale = ChildScale::for_parent(is_or_node, step, mode);
                let Some(scale) = scale else { continue };
                let zero = scale.apply(0, 0);
                assert!(
                    zero.0 >= 1 && zero.1 >= 1,
                    "{mode:?} {is_or_node} {step}: {zero:?}"
                );
                let max = scale.apply(u64::MAX, u64::MAX);
                assert_eq!(max, (INF, INF), "{mode:?} {is_or_node} {step}");
                let class = scale.apply(15, 15);
                assert!(class.0 >= 1 && class.0 <= INF);
                assert!(class.1 >= 1 && class.1 <= INF);
            }
        }
    }
}

#[test]
fn step_input_is_clamped() {
    // A step outside 0..=2 must not underflow the `2 - step` arithmetic.
    let high = ChildScale::for_parent(true, 9, EvalPnMode::Both).unwrap();
    assert_eq!(high.apply(1, 1), (1, 21));
    let low = ChildScale::for_parent(false, 9, EvalPnMode::Both).unwrap();
    assert_eq!(low.apply(1, 1), (1, 11));
}
