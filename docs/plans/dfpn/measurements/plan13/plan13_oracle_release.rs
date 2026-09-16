//! TEMPORARY plan13 Session B validation instrument — the Phase 0 G-B oracle
//! protocol re-run on the release binary. Not committed: deleted after the
//! run and archived under `docs/plans/dfpn/measurements/plan13/`.
//!
//! Samples legal attacker-to-move KQvK placements (side NOT to move must not
//! be in check — plan12 R8 hygiene), classifies them via the 3-man q-table
//! (`egtb3-q.bin`, validation oracle only), and runs the release `Search`
//! (pre-phase enabled) at bound h=6 with a 3M eval budget per sample.
//! Contradictions: a WIN claim on a draw sample, a DRAW claim on a win
//! sample, an uncertified WIN PV, or a region/table mismatch is impossible
//! to observe here (the pre-phase is table-free) — so the checks are:
//! claim-vs-table consistency and PV replay validity.

use atomic_movegen::board::Board;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn grid_fen(grid: &[u8; 64], white_to_move: bool) -> String {
    let mut s = String::with_capacity(80);
    for r in (0..8).rev() {
        let mut run = 0;
        for f in 0..8 {
            let c = grid[r * 8 + f];
            if c == b'.' {
                run += 1;
            } else {
                if run > 0 {
                    s.push_str(&run.to_string());
                    run = 0;
                }
                s.push(c as char);
            }
        }
        if run > 0 {
            s.push_str(&run.to_string());
        }
        if r > 0 {
            s.push('/');
        }
    }
    s.push(' ');
    s.push(if white_to_move { 'w' } else { 'b' });
    s.push_str(" - - 0 1");
    s
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n_target: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(240);
    let table_path = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "docs/plans/egtb/measurements/plan1/egtb3-q.bin".to_string());
    let table = std::fs::read(&table_path).expect("q-table");
    assert_eq!(table.len(), 1 << 20, "unexpected table size");

    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut seen = std::collections::HashSet::new();
    let mut wins: Vec<(u32, Board)> = Vec::new();
    let mut draws: Vec<(u32, Board)> = Vec::new();
    let half = n_target / 2;
    let mut attempts = 0u64;

    // Spike key_of: (s<<19)|(stm<<18)|(sk<<12)|(wk<<6)|xs, strong = queen side.
    let key_of = |grid: &[u8; 64], strong_white: bool| -> u32 {
        // sk = STRONG-side king (by color, not by case), wk = weak king.
        let (strong_king_char, weak_king_char, queen_char) = if strong_white {
            (b'K', b'k', b'Q')
        } else {
            (b'k', b'K', b'q')
        };
        let mut sk = 0usize;
        let mut wk = 0usize;
        let mut xs = 0usize;
        for i in 0..64 {
            if grid[i] == strong_king_char {
                sk = i;
            } else if grid[i] == weak_king_char {
                wk = i;
            } else if grid[i] == queen_char {
                xs = i;
            }
        }
        let s = u32::from(!strong_white);
        let stm = s; // attacker (strong) to move
        (s << 19) | (stm << 18) | ((sk as u32) << 12) | ((wk as u32) << 6) | xs as u32
    };

    while (wins.len() < half || draws.len() < half) && attempts < 20_000_000 {
        attempts += 1;
        let strong_white = rng.below(2) == 0;
        let sk = rng.below(64) as usize;
        let wk = rng.below(64) as usize;
        let xs = rng.below(64) as usize;
        if sk == wk || sk == xs || wk == xs {
            continue;
        }
        let mut grid = [b'.'; 64];
        grid[sk] = if strong_white { b'K' } else { b'k' };
        grid[wk] = if strong_white { b'k' } else { b'K' };
        grid[xs] = if strong_white { b'Q' } else { b'q' };
        // Legality: the side NOT to move (defender) must not be in check.
        let defender_fen = grid_fen(&grid, !strong_white);
        let defender = Board::from_fen(&defender_fen).unwrap();
        if !defender.checkers().is_empty() {
            continue;
        }
        let key = key_of(&grid, strong_white);
        if !seen.insert(key) {
            continue;
        }
        let t = table[key as usize];
        let fen = grid_fen(&grid, strong_white);
        if t == 2 && wins.len() < half {
            wins.push((key, Board::from_fen(&fen).unwrap()));
        } else if t == 1 && draws.len() < half {
            draws.push((key, Board::from_fen(&fen).unwrap()));
        }
    }

    println!(
        "plan13_oracle: samples win={} draw={} attempts={attempts}",
        wins.len(),
        draws.len()
    );

    let mut contradictions = 0u64;
    let mut win_claims = 0u64;
    let mut draw_claims = 0u64;
    let mut deferred = 0u64;
    // Phase 0's R oracle runs were bounded only by the region budget (evals
    // unbounded); the production REGION_BUDGET (1M) covers every 3-man
    // component, so no eval cap is applied here either.
    let eval_budget: u64 = u64::MAX;
    let bound = 6u32;

    for (i, (key, board)) in wins.iter().chain(draws.iter()).enumerate() {
        let is_win = i < wins.len();
        let fen = board.fen();
        let mut pos = Position::from_fen(&fen).unwrap();
        let mut search = Search::new(2);
        search.set_child_eval_budget(eval_budget);
        let (outcome, pv, _nodes) = search.search_depth(&mut pos, bound);
        let report = search.preflight_report().unwrap();

        if report.decided {
            match report.outcome {
                Some(Outcome::Win) => {
                    win_claims += 1;
                    if !is_win {
                        contradictions += 1;
                        println!("CONTRADICTION: WIN claim on draw sample {fen}");
                    }
                    if outcome != Outcome::Win
                        || pv.len() as u32 != report.rank
                        || !search.validate_pv(&pv, &pos, Outcome::Win, Some(report.rank))
                    {
                        contradictions += 1;
                        println!("CONTRADICTION: invalid WIN PV at {fen}");
                    }
                }
                Some(Outcome::Draw) => {
                    draw_claims += 1;
                    if is_win {
                        contradictions += 1;
                        println!("CONTRADICTION: DRAW claim on win sample {fen}");
                    }
                }
                Some(Outcome::Loss) => {
                    contradictions += 1;
                    println!("CONTRADICTION: LOSS claim at attacker-to-move {fen}");
                }
                None => unreachable!("decided report carries an outcome"),
            }
        } else {
            deferred += 1;
            // Deferral reasons must be sound: `bound` (dtm > 6) or
            // `region-budget` — never budget/stop/cert-fail on these
            // unbounded runs.
            if matches!(report.reason, "cert-fail" | "eval-budget" | "stop" | "internal" | "rank-guard") {
                contradictions += 1;
                println!("CONTRADICTION: unsound deferral reason {} at {fen}", report.reason);
            }
        }
        let _ = key;
        if (i + 1) % 40 == 0 {
            println!(
                "plan13_oracle progress: {}/{} contradictions={contradictions}",
                i + 1,
                wins.len() + draws.len()
            );
        }
    }

    println!(
        "plan13_oracle summary: samples={} contradictions={contradictions} win_claims={win_claims} draw_claims={draw_claims} deferred={deferred}",
        wins.len() + draws.len()
    );
    if contradictions > 0 {
        std::process::exit(1);
    }
}
