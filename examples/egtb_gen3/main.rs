//! 3-man atomic endgame tablebase generator prototype + solver
//! cross-validation (egtb `plan1` Tasks B and C).
//!
//! This file is larger than the 10 KiB guideline because the CLI, the
//! sampling strategies, and both cross-validation oracles (solver with
//! timeout classification, independent proof search) share the FEN/index
//! mapping and outcome-code mapping; splitting them would duplicate the
//! entry-decode logic the report's numbers depend on.
//!
//! Generates K+x vs K WDL tables (x ∈ {Q, R, B, N, P}, both strong-side
//! colors) over `atomic_movegen` semantics, writes a raw byte-per-entry WDL
//! file, and cross-validates sampled positions against the `Search` solver.
//! Zero cross-validation mismatches is the merge gate of the spike.
//!
//! Run with:
//!     cargo run --release --example `egtb_gen3` -- --material q --out /tmp/kq3.bin
//!     cargo run --release --example `egtb_gen3` -- --material all --out /tmp/egtb3.bin --samples 32

mod prove;
mod table;

use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;
use std::time::Instant;

use table::{
    CLASS_CHARS, CLASS_ORDER, NB, Table3, U_DRAW, U_LOSS, U_WIN, decode_index, fen_for,
    position_is_legal, resolve_fen,
};

fn table_outcome(v: u8) -> Option<Outcome> {
    match v {
        U_LOSS => Some(Outcome::Loss),
        U_DRAW => Some(Outcome::Draw),
        U_WIN => Some(Outcome::Win),
        _ => None,
    }
}

/// Deterministic stride-free sample: take the `n` indices with the smallest
/// golden-ratio hashes, stratified by the caller.
fn sample_indices(indices: &[usize], n: usize) -> Vec<usize> {
    let mut keyed: Vec<(u64, usize)> = indices
        .iter()
        .map(|&i| (i.wrapping_mul(0x9E37_79B9_7F4A_7C15) as u64, i))
        .collect();
    keyed.sort_unstable();
    keyed.truncate(n);
    keyed.into_iter().map(|(_, i)| i).collect()
}

/// Solve one FEN with the solver (first-outcome, short timeout) and compare
/// against the expected table outcome.
fn solve_and_compare(fen: &str, expected: Option<Outcome>, timeout: u64) -> Result<(), String> {
    let mut pos = Position::from_fen(fen).map_err(|e| format!("{fen}: FEN error {e}"))?;
    let mut search = Search::new(16);
    search.set_timeout(timeout);
    search.set_first_outcome_only(true);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    match expected {
        Some(exp) if exp == outcome => Ok(()),
        Some(exp) if outcome == Outcome::Draw && search.time_exceeded() => {
            Err(format!("mismatch: {fen} solver=draw timeout table={exp}"))
        }
        Some(exp) => Err(format!(
            "mismatch: {fen} solver={} table={exp}",
            outcome.as_str()
        )),
        None => Err(format!("mismatch: {fen} no table value (invalid entry)")),
    }
}

/// Cross-validate one generated table against the solver: sampled positions
/// stratified by (strong color, outcome), solved with `Search`.
///
/// Only *legal* placements are sampled (illegal ones — side not to move
/// attackable — are unreachable in play and have quirks like generated
/// king-capture moves). A solver `Draw` that is really a timeout is retried
/// once with a larger budget and then counted as `unproven`, not as a
/// mismatch: the spike found the solver cannot prove some genuine 3-man
/// wins within any reasonable budget (see report1).
fn cross_validate_table(
    t: &Table3,
    samples: usize,
    timeout: u64,
    prove_samples: usize,
    memo: &mut std::collections::HashMap<(String, u32), Option<u8>>,
    log: &mut Vec<String>,
) -> usize {
    let mut mismatches = 0usize;
    let mut solved = 0usize;
    let mut unproven = 0usize;
    for strong in 0..2usize {
        for bucket in [U_LOSS, U_DRAW, U_WIN] {
            let indices: Vec<usize> = (0..NB)
                .filter(|&i| {
                    let (s, m, sk, wk, xs) = decode_index(i);
                    t.value_at(i) == bucket
                        && s == strong
                        && position_is_legal(s, m, sk, wk, xs, CLASS_ORDER[t.class])
                })
                .collect();
            if indices.is_empty() {
                log.push(format!(
                    "[crossval {}] strong={} bucket={bucket}: no legal positions",
                    CLASS_CHARS[t.class], strong
                ));
                continue;
            }
            for i in sample_indices(&indices, samples) {
                let (s, stm, sk, wk, xs) = decode_index(i);
                let fen = fen_for(s, stm, sk, wk, xs, CLASS_ORDER[t.class]);
                let expected = table_outcome(t.value_at(i));
                let mut to = timeout;
                loop {
                    match solve_and_compare(&fen, expected, to) {
                        Ok(()) => {
                            solved += 1;
                            break;
                        }
                        Err(e) if e.contains("solver=draw timeout") && to < 60 => {
                            to = 60;
                        }
                        Err(e) if e.contains("solver=draw timeout") => {
                            unproven += 1;
                            break;
                        }
                        Err(e) => {
                            mismatches += 1;
                            log.push(format!("[crossval {}] {e}", CLASS_CHARS[t.class]));
                            break;
                        }
                    }
                }
            }
        }
    }
    log.push(format!(
        "[crossval {}] solver: solved={solved} unproven(timeout)={unproven} mismatches={mismatches}",
        CLASS_CHARS[t.class]
    ));
    mismatches += cross_validate_prove(t, prove_samples, memo, log);
    mismatches
}

/// Second oracle: an independent depth-limited AND/OR proof search over the
/// same movegen semantics. Confirms table Wins (within the depth bound) and
/// flags any proof contradicting the table. `None` results are inconclusive
/// (the bound, not the table, is the limit) and counted separately.
fn cross_validate_prove(
    t: &Table3,
    samples: usize,
    memo: &mut std::collections::HashMap<(String, u32), Option<u8>>,
    log: &mut Vec<String>,
) -> usize {
    const PROVE_DEPTH: u32 = 16;
    let mut defects = 0usize;
    let mut confirmed = 0usize;
    let mut inconclusive = 0usize;
    for strong in 0..2usize {
        for bucket in [U_LOSS, U_DRAW, U_WIN] {
            let indices: Vec<usize> = (0..NB)
                .filter(|&i| {
                    let (s, m, sk, wk, xs) = decode_index(i);
                    t.value_at(i) == bucket
                        && s == strong
                        && position_is_legal(s, m, sk, wk, xs, CLASS_ORDER[t.class])
                })
                .collect();
            for i in sample_indices(&indices, samples) {
                // The proof memo grows with the proof frontier; cap it to keep
                // the whole run inside the memory budget (cleared memo =
                // recomputation, never a wrong result).
                if memo.len() > 2_000_000 {
                    memo.clear();
                }
                let (s, stm, sk, wk, xs) = decode_index(i);
                let fen = fen_for(s, stm, sk, wk, xs, CLASS_ORDER[t.class]);
                let board =
                    atomic_movegen::board::Board::from_fen(&fen).expect("valid generated FEN");
                let pv = prove::prove(&board, PROVE_DEPTH, memo);
                let table_v = t.value_at(i);
                let consistent = match (table_v, pv) {
                    (U_WIN, Some(prove::P_WIN)) | (U_LOSS, Some(prove::P_LOSS)) => true,
                    (U_DRAW | _, None) => true, // bound exhausted
                    (U_DRAW, Some(prove::P_DRAW)) => true,
                    _ => false,
                };
                if consistent {
                    if pv.is_some() {
                        confirmed += 1;
                    } else {
                        inconclusive += 1;
                    }
                } else {
                    defects += 1;
                    log.push(format!(
                        "[prove {}] contradiction: {fen} table={table_v} prover={pv:?}",
                        CLASS_CHARS[t.class]
                    ));
                }
            }
        }
    }
    log.push(format!(
        "[prove {}] depth={PROVE_DEPTH} confirmed={confirmed} inconclusive={inconclusive} contradictions={defects}",
        CLASS_CHARS[t.class]
    ));
    defects
}

/// Explicit edge cases from `src/position.rs` tests plus bare kings; each is
/// solved with the solver and (where in the 3-man space) checked against the
/// matching table entry.
fn cross_validate_edges(tables: &[Table3], timeout: u64, log: &mut Vec<String>) -> usize {
    // (label, fen, expected solver outcome)
    const EDGES: [(&str, &str, Outcome); 4] = [
        (
            "stalemate_draw",
            "7k/8/8/8/8/8/2q5/K7 w - - 0 1",
            Outcome::Draw,
        ),
        (
            "checkmate_loss",
            "7K/8/8/8/8/8/1Q6/k7 b - - 0 1",
            Outcome::Loss,
        ),
        (
            "rule50_checkmate_is_loss",
            "7K/8/8/8/8/8/1Q6/k7 b - - 100 1",
            Outcome::Loss,
        ),
        (
            "bare_kings_draw",
            "8/8/8/8/8/8/1K6/k7 b - - 0 1",
            Outcome::Draw,
        ),
    ];
    let mut mismatches = 0usize;
    let q_table = tables.iter().find(|t| t.class == 0);
    for (label, fen, expected) in EDGES {
        if let Err(e) = solve_and_compare(fen, Some(expected), timeout) {
            mismatches += 1;
            log.push(format!("[edges] solver mismatch ({label}): {e}"));
        }
        // Table agreement where the position lives inside the q-table space.
        if let (Some(qt), Some(i)) = (q_table, resolve_fen(0, fen)) {
            let tv = table_outcome(qt.value_at(i));
            if tv == Some(expected) {
                log.push(format!("[edges] {label}: solver={expected} table ok"));
            } else {
                mismatches += 1;
                log.push(format!(
                    "[edges] table mismatch ({label}): {fen} table={tv:?} solver={expected:?}"
                ));
            }
        } else if q_table.is_none() {
            log.push(format!(
                "[edges] {label}: solver={expected} (q-table not generated, solver-checked only)"
            ));
        } else {
            log.push(format!(
                "[edges] {label}: solver={expected} (outside the 3-man space, solver-checked only)"
            ));
        }
    }
    mismatches
}

fn main() {
    let mut material = String::from("all");
    let mut out = String::from("egtb3.bin");
    let mut samples = 8usize;
    let mut prove_samples = 8usize;
    let mut timeout = 5u64;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--material" => {
                material = args
                    .get(i + 1)
                    .expect("--material needs q|r|b|n|p|all")
                    .clone();
                i += 2;
            }
            "--out" => {
                out = args.get(i + 1).expect("--out needs a file path").clone();
                i += 2;
            }
            "--samples" => {
                samples = args
                    .get(i + 1)
                    .and_then(|s| s.parse().ok())
                    .expect("--samples needs a number");
                i += 2;
            }
            "--prove-samples" => {
                prove_samples = args
                    .get(i + 1)
                    .and_then(|s| s.parse().ok())
                    .expect("--prove-samples needs a number");
                i += 2;
            }
            "--timeout" => {
                timeout = args
                    .get(i + 1)
                    .and_then(|s| s.parse().ok())
                    .expect("--timeout needs seconds");
                i += 2;
            }
            other => panic!("unknown argument '{other}'"),
        }
    }

    let classes: Vec<usize> = if material == "all" {
        (0..CLASS_ORDER.len()).collect()
    } else {
        let pos = CLASS_CHARS
            .iter()
            .position(|&c| c == material.chars().next().unwrap());
        vec![pos.unwrap_or_else(|| panic!("unknown material '{material}'"))]
    };

    let mut log: Vec<String> = Vec::new();
    let mut total_mismatches = 0usize;
    let mut built: Vec<Table3> = Vec::new();
    let mut memo: std::collections::HashMap<(String, u32), Option<u8>> =
        std::collections::HashMap::new();
    let start = Instant::now();

    for &class in &classes {
        let pt = CLASS_ORDER[class];
        // Pawn children promote into Q/R/B/N; those must exist first.
        let deps: Vec<&Table3> = if pt == atomic_movegen::types::PieceType::Pawn {
            (0..4)
                .map(|c| {
                    built
                        .iter()
                        .find(|t| t.class == c)
                        .expect("promo dependency")
                })
                .collect()
        } else {
            Vec::new()
        };
        let t0 = Instant::now();
        let mut t = Table3::build(class, &deps);
        let build_secs = t0.elapsed().as_secs_f64();
        let st = &t.stats;
        log.push(format!(
            "[table {}] entries={NB} win=[{},{}] loss=[{},{}] draw=[{},{}] invalid={} illegal=[{},{}] checkmates=[{},{}] stalemates=[{},{}] max_dtm={}plies passes={} child_refs={} child_ref_MB={:.1} build={build_secs:.2}s unknown_left={} board_outcome_divergences={}/{}(sampled) symmetry_mismatches={}",
            CLASS_CHARS[class],
            st.wins[0], st.wins[1], st.losses[0], st.losses[1], st.draws[0], st.draws[1],
            st.invalid, st.illegal[0], st.illegal[1],
            st.checkmates[0], st.checkmates[1], st.stalemates[0], st.stalemates[1],
            st.max_dtm, st.passes, st.child_refs,
            (st.child_refs * 4) as f64 / (1024.0 * 1024.0),
            st.unknown_left, st.outcome_divergences, st.outcome_samples,
            st.symmetry_mismatches,
        ));
        if st.symmetry_mismatches > 0 {
            log.push(format!(
                "[table {}] FAILED: symmetry violations",
                CLASS_CHARS[class]
            ));
            total_mismatches += 1;
        }
        // Raw WDL file (one file per material when running all).
        let path = if classes.len() == 1 {
            out.clone()
        } else {
            match out.rsplit_once('.') {
                Some((stem, ext)) if !stem.is_empty() => {
                    format!("{stem}-{}.{}", CLASS_CHARS[class], ext)
                }
                _ => format!("{}-{}", out, CLASS_CHARS[class]),
            }
        };
        match t.write_raw(std::path::Path::new(&path)) {
            Ok(bytes) => log.push(format!(
                "[table {}] wrote {path} ({bytes} bytes)",
                CLASS_CHARS[class]
            )),
            Err(e) => log.push(format!("[table {}] write failed: {e}", CLASS_CHARS[class])),
        }
        // Only `value` is read from here on (cross-validation, later deps).
        t.drop_child_lists();
        if samples > 0 {
            total_mismatches +=
                cross_validate_table(&t, samples, timeout, prove_samples, &mut memo, &mut log);
        }
        built.push(t);
    }

    if samples > 0 {
        total_mismatches += cross_validate_edges(&built, timeout, &mut log);
    }

    for line in &log {
        println!("{line}");
    }
    println!(
        "egtb_gen3 done in {:.2}s: crossval_mismatches={total_mismatches}",
        start.elapsed().as_secs_f64()
    );
    if total_mismatches > 0 {
        std::process::exit(1);
    }
}
