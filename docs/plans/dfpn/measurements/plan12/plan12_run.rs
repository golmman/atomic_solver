//! TEMPORARY plan12 Phase 0 measurement runner (T0/T2) — spike code, reverted
//! after measuring. Source is archived under
//! `docs/plans/dfpn/measurements/plan12/` together with the raw logs.
//!
//! Runs one FEN either in default solve mode or as a bounded
//! `search_depth(depth)` run, and prints the deterministic metrics
//! (child evals, nodes, exit reason, wall, PV length) plus, when
//! `DFPN12_SPIKE=1`, the spike counters printed by `Search::spike12_summary`.

use atomic_solver::notation::move_to_uci;
use atomic_solver::position::Position;
use atomic_solver::search::dfpn::Search;

fn main() {
    let mut fen: Option<String> = None;
    let mut timeout = 600u64;
    let mut depth: Option<u32> = None;
    let mut first_outcome = false;
    let mut tt_mb = 128usize;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--timeout" => timeout = args.next().unwrap().parse().unwrap(),
            "--depth" => depth = Some(args.next().unwrap().parse().unwrap()),
            "--first-outcome" => first_outcome = true,
            "--tt-mb" => tt_mb = args.next().unwrap().parse().unwrap(),
            _ => fen = Some(a),
        }
    }
    let fen = fen.expect("a FEN argument is required");
    let mut pos = Position::from_fen(&fen).unwrap();
    let mut search = Search::new(tt_mb);
    search.set_timeout(timeout);
    if first_outcome {
        search.set_first_outcome_only(true);
    }
    let t0 = std::time::Instant::now();
    let (outcome, pv) = match depth {
        Some(d) => {
            let (o, pv, _n) = search.search_depth(&mut pos, d);
            (o, pv)
        }
        None => {
            let (o, pv, _n) = search.solve(&mut pos);
            (o, pv)
        }
    };
    let wall = t0.elapsed();
    let pv_uci: Vec<String> = pv.iter().take(40).map(|m| move_to_uci(*m)).collect();
    println!(
        "plan12_run result: fen=\"{fen}\" mode={} outcome={outcome:?} exit={} nodes={} child_evals={} fo_child_evals={} refine_rounds={} refine_evals={} wall={:.2}s pv_len={} pv_status={:?}",
        if depth.is_some() {
            format!("depth-{}", depth.unwrap())
        } else if first_outcome {
            "first-outcome".to_string()
        } else {
            "default".to_string()
        },
        search.exit_reason(),
        search.nodes(),
        search.child_evaluations(),
        search.first_outcome_evaluations(),
        search.refinement_rounds(),
        search.refinement_evaluations(),
        wall.as_secs_f64(),
        pv.len(),
        search.pv_status(),
    );
    println!("plan12_run pv: {}", pv_uci.join(" "));
    if let Some(summary) = search.spike12_summary() {
        print!("{summary}");
    }
}
