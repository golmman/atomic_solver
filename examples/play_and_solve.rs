//! Play a specific move on a position and then solve the resulting position.
//!
//! This is useful for checking a particular variation, e.g. to see why the
//! solver returns a different result after a forcing move.
//!
//! Default: the m19 regression FEN with the move d6f8.
//!
//! Usage:
//!     cargo run --example play_and_solve
//!     cargo run --example play_and_solve -- "<fen>" <from> <to> [q|r|b|n]

use atomic_movegen::types::{Move, MoveList, MoveType, PieceType, Square, parse_sq};
use atomic_solver::notation::move_to_uci;
use atomic_solver::position::Position;
use atomic_solver::search::dfpn::Search;

fn parse_promotion(s: &str) -> Option<PieceType> {
    match s {
        "q" => Some(PieceType::Queen),
        "r" => Some(PieceType::Rook),
        "b" => Some(PieceType::Bishop),
        "n" => Some(PieceType::Knight),
        _ => None,
    }
}

fn find_move(
    pos: &Position,
    from: Square,
    to: Square,
    promotion: Option<PieceType>,
) -> Option<Move> {
    let mut moves = MoveList::new();
    pos.legal_moves(&mut moves);

    for i in 0..moves.len() {
        let m = moves[i];
        if m.from_sq() == from && m.to_sq() == to {
            match promotion {
                Some(pt) => {
                    if m.move_type() == MoveType::Promotion && m.promotion_type() == pt {
                        return Some(m);
                    }
                }
                None => return Some(m),
            }
        }
    }
    None
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (fen, from_str, to_str, promo_str) = if args.len() >= 4 {
        (
            args[1].clone(),
            args[2].clone(),
            args[3].clone(),
            args.get(4).cloned(),
        )
    } else {
        let default = "4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19".to_string();
        (default, "d6".to_string(), "f8".to_string(), None)
    };

    let mut pos = Position::from_fen(&fen).unwrap();
    let from = parse_sq(&from_str).unwrap();
    let to = parse_sq(&to_str).unwrap();
    let promotion = promo_str.as_deref().and_then(parse_promotion);

    let mv = find_move(&pos, from, to, promotion).unwrap_or_else(|| {
        panic!("no legal move from {} to {}", from_str, to_str);
    });

    pos.do_move(mv);
    let mut search = Search::new(256);
    search.set_timeout(5);
    let (outcome, pv, nodes) = search.solve(&mut pos);
    eprintln!(
        "after {}: outcome: {:?} nodes: {}",
        move_to_uci(mv),
        outcome,
        nodes
    );
    for m in pv {
        eprintln!("{}", atomic_solver::notation::move_to_uci(m));
    }
}
