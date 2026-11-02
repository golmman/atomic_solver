//! Front-end accounting probe for `docs/plans/winprover/measurements/plan3`.
//!
//! Runs the opt-in win front-end alone on one FEN and prints one JSON line
//! with its child-eval/node counts and whether it proved a win. Used to
//! separate the front-end's work from the general search in the plan3 bench
//! run (the CLI only reports the combined counters).
//!
//! Build (the release crate is LTO, so the probe needs `-C lto=thin`):
//!
//! ```sh
//! rustc --edition 2024 -O -C panic=abort -C lto=thin probe.rs -o /tmp/probe \
//!   -L target/release/deps \
//!   --extern atomic_solver="$(ls -t target/release/deps/libatomic_solver-*.rlib | head -1)"
//! ```
//!
//! Usage: `probe <FEN> <budget>`

fn main() {
    let mut args = std::env::args().skip(1);
    let fen = args.next().expect("usage: probe <FEN> <budget>");
    let budget: u64 = args
        .next()
        .expect("usage: probe <FEN> <budget>")
        .parse()
        .expect("budget must be an integer");

    let pos = atomic_solver::position::Position::from_fen(&fen).expect("valid FEN");
    let scorer = atomic_solver::search::ordering::StaticAtomicScorer::default();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let mut front =
        atomic_solver::search::winprover::WinProver::new(scorer, budget, deadline, None);
    let started = std::time::Instant::now();
    let proof = front.solve(&pos);
    println!(
        "{{\"evals\":{},\"nodes\":{},\"proved\":{},\"depth\":{},\"wall_s\":{:.3}}}",
        front.child_evaluations(),
        front.nodes(),
        proof.is_some(),
        proof.map_or(0, |p| p.depth),
        started.elapsed().as_secs_f64()
    );
}
