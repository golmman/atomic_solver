//! Play a specific move on a position and then solve the resulting position.
//!
//! This is useful for checking a particular variation, e.g. to see why the
//! solver returns a different result after a forcing move.
//!
//! Default: the `m19` regression FEN with the move `d6f8`.
//!
//! Usage:
//!     cargo run --example `play_and_solve`
//!     cargo run --example `play_and_solve` -- "<fen>" <from> <to> [q|r|b|n]

mod common;

use atomic_solver::notation::move_to_uci;
use atomic_solver::position::Position;
use atomic_solver::search::dfpn::Search;

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
        (
            common::M19_FEN.to_string(),
            "d6".to_string(),
            "f8".to_string(),
            None,
        )
    };

    let pos = Position::from_fen(&fen).unwrap();
    let mv = common::parse_move(&pos, &from_str, &to_str, promo_str.as_deref())
        .unwrap_or_else(|| panic!("no legal move from {from_str} to {to_str}"));

    let mut pos = pos;
    pos.do_move(mv);
    let mut search = Search::new(256);
    search.set_timeout(5);
    let (outcome, pv, nodes) = search.solve(&mut pos);
    let uci = move_to_uci(mv);
    eprintln!("after {uci}: outcome: {outcome:?} nodes: {nodes}");
    for m in pv {
        eprintln!("{}", move_to_uci(m));
    }
}
