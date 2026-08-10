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
//! first and see the dynamic bonuses. Pass `--and` to show the defender
//! (AND-node) scoring profile. Pass `--config <FILE>` to load custom scorer
//! parameters.
//!
//! Default position is the `m19` regression FEN. Use `--name <case>` to inspect
//! one of the move-order benchmark positions from `tests/fixtures/move_order_positions.txt`.
//!
//! Usage:
//!     cargo run --example move_order_debug
//!     cargo run --example move_order_debug -- --name m25_white
//!     cargo run --example move_order_debug -- --name m25_white --and
//!     cargo run --example move_order_debug -- --config /path/to/scorer.toml
//!     cargo run --example move_order_debug -- --solve "<fen>"
//!     cargo run --example move_order_debug -- "<fen>"

mod common;

use atomic_solver::config;
use atomic_solver::notation::move_to_uci;
use atomic_solver::position::Position;
use atomic_solver::search::dfpn::Search;
use atomic_solver::search::ordering::StaticAtomicScorer;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut solve_first = false;
    let mut is_or_node = true;
    let mut name: Option<String> = None;
    let mut fen: Option<String> = None;
    let mut config_path: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--solve" => {
                solve_first = true;
                i += 1;
            }
            "--and" => {
                is_or_node = false;
                i += 1;
            }
            "--name" if i + 1 < args.len() => {
                name = Some(args[i + 1].clone());
                i += 2;
            }
            "--fen" if i + 1 < args.len() => {
                fen = Some(args[i + 1].clone());
                i += 2;
            }
            "--config" if i + 1 < args.len() => {
                config_path = Some(args[i + 1].clone());
                i += 2;
            }
            _ if name.is_none() && fen.is_none() => {
                fen = Some(args[i].clone());
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    let fen = if let Some(name) = name {
        common::move_order_case(&name)
            .unwrap_or_else(|| panic!("unknown benchmark name '{name}'"))
            .fen
    } else {
        fen.unwrap_or_else(|| common::M19_FEN.to_string())
    };

    let config_path = config_path.or_else(|| std::env::var("SCORER_CONFIG").ok());

    let pos = if fen == "startpos" {
        Position::new()
    } else {
        Position::from_fen(&fen).unwrap()
    };

    let scorer = match config_path {
        Some(path) => {
            let params = config::load_scorer_config(&path).expect("valid scorer config");
            StaticAtomicScorer::from_params(params)
        }
        None => StaticAtomicScorer::default(),
    };

    let mut search = Search::new(64);
    search.set_scorer(scorer);
    if solve_first {
        search.set_timeout(5);
        let mut p = pos.clone();
        search.solve(&mut p);
    }

    let breakdown = search.move_order_breakdown(&pos, is_or_node);
    println!("fen: {}", pos.fen());
    println!("move  static  history  killer  total");
    for (m, static_score, history, killer, total) in breakdown {
        let uci = move_to_uci(m);
        println!("{uci}  {static_score:7} {history:7} {killer:6} {total:7}");
    }
}
