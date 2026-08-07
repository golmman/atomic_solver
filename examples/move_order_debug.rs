//! Print the move-ordering breakdown for all legal moves in a position.
//!
//! For each move the example shows:
//!   * the static atomic scorer value,
//!   * the history-table bonus,
//!   * the killer-move bonus,
//!   * the combined total that `sort_moves` actually uses.
//!
//! By default the breakdown is shown *before* any search, so history and killer
//! bonuses are zero. Pass `--solve` as the first argument to run a short solve
//! first and see the dynamic bonuses.
//!
//! Default position is the `m19` regression FEN.
//!
//! Usage:
//!     cargo run --example move_order_debug
//!     cargo run --example move_order_debug -- --solve "<fen>"
//!     cargo run --example move_order_debug -- "<fen>"

mod common;

use atomic_solver::notation::move_to_uci;
use atomic_solver::position::Position;
use atomic_solver::search::dfpn::Search;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut solve_first = false;
    let mut fen: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--solve" => {
                solve_first = true;
                i += 1;
            }
            "--fen" if i + 1 < args.len() => {
                fen = Some(args[i + 1].clone());
                i += 2;
            }
            _ if fen.is_none() => {
                fen = Some(args[i].clone());
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    let fen = fen.unwrap_or_else(|| common::M19_FEN.to_string());
    let pos = if fen == "startpos" {
        Position::new()
    } else {
        Position::from_fen(&fen).unwrap()
    };

    let mut search = Search::new(64);
    if solve_first {
        search.set_timeout(5);
        let mut p = pos.clone();
        search.solve(&mut p);
    }

    let breakdown = search.move_order_breakdown(&pos);
    println!("fen: {}", pos.fen());
    println!("move  static  history  killer  total");
    for (m, static_score, history, killer, total) in breakdown {
        let uci = move_to_uci(m);
        println!("{uci}  {static_score:7} {history:7} {killer:6} {total:7}");
    }
}
