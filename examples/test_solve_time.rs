use atomic_solver::position::Position;
use atomic_solver::search::dfpn::Search;

fn main() {
    let fen = "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26";
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    let start = std::time::Instant::now();
    let outcome = search.solve_outcome(&mut pos);
    println!(
        "outcome={outcome:?} elapsed={:.3}s",
        start.elapsed().as_secs_f64()
    );
}
