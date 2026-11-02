//! Front-end accounting probe for `docs/plans/winprover/measurements/plan4`.
//!
//! Runs the opt-in win front-end alone on one FEN and prints one JSON line
//! with its child-eval/node counts, whether it proved a win, and the sibling
//! replay counters. Used to separate the front-end's work from the general
//! search in the plan4 bench run (the CLI only reports the combined counters).
//!
//! Build (the release crate is LTO, so the probe needs `-C lto=thin`):
//!
//! ```sh
//! rustc --edition 2024 -O -C panic=abort -C lto=thin probe.rs -o /tmp/probe \
//!   -L target/release/deps \
//!   --extern atomic_solver="$(ls -t target/release/deps/libatomic_solver-*.rlib | head -1)"
//! ```
//!
//! Usage: `probe <FEN> <budget> <simulation:on|off>`

fn main() {
    let mut args = std::env::args().skip(1);
    let fen = args.next().expect("usage: probe <FEN> <budget> <on|off>");
    let budget: u64 = args
        .next()
        .expect("usage: probe <FEN> <budget> <on|off>")
        .parse()
        .expect("budget must be an integer");
    let simulation = match args.next().as_deref() {
        Some("on") => true,
        Some("off") => false,
        _ => panic!("usage: probe <FEN> <budget> <on|off>"),
    };

    let pos = atomic_solver::position::Position::from_fen(&fen).expect("valid FEN");
    let scorer = atomic_solver::search::ordering::StaticAtomicScorer::default();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let mut front =
        atomic_solver::search::winprover::WinProver::new(scorer, budget, deadline, None);
    front.set_simulation(simulation);
    let started = std::time::Instant::now();
    let proof = front.solve(&pos);
    println!(
        "{{\"simulation\":{},\"evals\":{},\"nodes\":{},\"proved\":{},\"depth\":{},\"attempts\":{},\"successes\":{},\"sim_evals\":{},\"wall_s\":{:.3}}}",
        simulation,
        front.child_evaluations(),
        front.nodes(),
        proof.is_some(),
        proof.map_or(0, |p| p.depth),
        front.simulation_attempts(),
        front.simulation_successes(),
        front.simulation_evaluations(),
        started.elapsed().as_secs_f64()
    );
}
