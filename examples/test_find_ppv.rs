use atomic_solver::position::Position;
use atomic_solver::search::dfpn::Search;

fn main() {
    let fen = "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26";
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    let outcome = search.solve_outcome(&mut pos);
    println!("solve_outcome={outcome:?}");
    let start = std::time::Instant::now();
    let ppv = search.find_ppv(&mut pos, outcome);
    println!(
        "find_ppv elapsed={:.3}s len={}",
        start.elapsed().as_secs_f64(),
        ppv.as_ref().map_or(0, |pv| pv.len())
    );
    if let Some(pv) = ppv {
        println!(
            "pv: {}",
            pv.iter().map(|m| m.to_uci()).collect::<Vec<_>>().join(" ")
        );
    }
}
