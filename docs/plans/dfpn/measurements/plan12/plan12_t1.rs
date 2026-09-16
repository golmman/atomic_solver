//! TEMPORARY plan12 Phase 0 T1 instrument — spike code, reverted after
//! measuring. Source archived under `docs/plans/dfpn/measurements/plan12/`.
//!
//! T1a — strategy-cycle check (R6: confirmatory): loads the 3-man q-table,
//! recomputes *exact* DTM over the region reachable from a ladder root under
//! real `atomic_movegen` movegen (retrograde AND/OR fixpoint, clock-0
//! semantics), extracts the rank-decreasing winning strategy (attacker plays a
//! child with dtm exactly d−1 = the largest child dtm available) and
//! enumerates *every* line under it, checking cycle-freedom (no board repeats
//! on a line) and termination (mate within the root dtm). Also probes the
//! naive greedy (first table-winning child, any dtm) for cycles.
//!
//! The packed position key doubles as the table index:
//! `key = (((strong*2 + stm)*64 + sk)*64 + wk)*64 + xs` with `sk` the strong
//! king, `wk` the weak king, `xs` the queen (same layout as
//! `examples/egtb_gen3/table.rs`), so region-vs-table consistency is checked
//! entry by entry for free.
//!
//! T1b — repetition-aware bounded prover (R6: primary G1 instrument):
//! independent AND/OR minimax over real movegen with the solver's
//! path-repetition semantics: a position already on the current line is a
//! Draw (`NOT_WIN`), so any winning strategy this prover certifies is
//! cycle-free by construction. Memo keyed by
//! `(position key, remaining depth, order-independent ancestor-set hash)`,
//! capped. Values: WIN (attacker mates within depth), NOT_WIN (defender
//! forces a draw/repetition within depth), UNKNOWN. Depth ≤ 17 with root
//! clocks ≤ 6 keeps the halfmove clock < 100, so rule50 cannot fire and the
//! clock is excluded from the state (documented limitation).
//!
//! Usage: `plan12_t1 <qtable.bin> <fen> [--t1b-depths 15,16,17]
//!         [--greedy-node-cap N] [--memo-cap N] [--no-t1a] [--no-t1b]`

use atomic_movegen::board::{Board, StateInfo};
use atomic_movegen::movegen::generate_legal;
use atomic_movegen::types::{Color, Move, MoveList, PieceType};
use std::collections::HashMap;

const UNDECIDED: u8 = 0;
const DRAW: u8 = 1;
const WIN: u8 = 2;
const LOSS: u8 = 3;

const T_DRAW: u8 = 1;
const T_WIN0: u8 = 2; // attacker has already won (defender mated)
const T_LOSS: u8 = 3; // attacker already lost (cannot occur from legal roots)

fn strong_color(b: &Board) -> Color {
    if b.pieces_color_pt(Color::White, PieceType::Queen).is_empty() {
        Color::Black
    } else {
        Color::White
    }
}

/// Packed position key == table index.
fn key_of(b: &Board) -> u32 {
    let strong = strong_color(b);
    let sk = b.commoners(strong).lsb() as u32;
    let wk = b.commoners(strong.flip()).lsb() as u32;
    let xs = b.pieces_color_pt(strong, PieceType::Queen).lsb() as u32;
    let stm = b.side_to_move() as usize as u32;
    let s = strong as usize as u32;
    (s << 19) | (stm << 18) | (sk << 12) | (wk << 6) | xs
}

fn board_from_key(key: u32) -> Board {
    let strong = (key >> 19) & 1;
    let stm = (key >> 18) & 1;
    let sk = ((key >> 12) & 63) as usize;
    let wk = ((key >> 6) & 63) as usize;
    let xs = (key & 63) as usize;
    let mut grid = [b'.'; 64];
    let strong_white = strong == 0;
    grid[sk] = if strong_white { b'K' } else { b'k' };
    grid[wk] = if strong_white { b'k' } else { b'K' };
    grid[xs] = if strong_white { b'Q' } else { b'q' };
    let mut s = String::with_capacity(80);
    for r in (0..8).rev() {
        let mut run = 0;
        for f in 0..8 {
            if grid[r * 8 + f] == b'.' {
                run += 1;
            } else {
                if run > 0 {
                    s.push_str(&run.to_string());
                    run = 0;
                }
                s.push(grid[r * 8 + f] as char);
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
    s.push(if stm == 0 { 'w' } else { 'b' });
    s.push_str(" - - 0 1");
    Board::from_fen(&s).unwrap()
}

/// Solver-equivalent terminal classification at clock 0.
fn classify_terminal(b: &Board) -> Option<u8> {
    let stm = b.side_to_move();
    let strong = strong_color(b);
    if b.commoners(stm).is_empty() {
        return Some(if stm == strong { T_LOSS } else { T_WIN0 });
    }
    let mut m = MoveList::new();
    generate_legal(b, &mut m);
    if m.is_empty() {
        return Some(if b.checkers().is_empty() {
            T_DRAW
        } else if stm == strong {
            T_LOSS
        } else {
            T_WIN0
        });
    }
    if b.occupied().count() == 2 {
        return Some(T_DRAW);
    }
    None
}

/// Children of a position as packed keys (real movegen).
fn child_keys(b: &Board) -> Vec<u32> {
    let mut moves = MoveList::new();
    generate_legal(b, &mut moves);
    let mut st = StateInfo::new();
    let mut out = Vec::with_capacity(moves.len());
    for i in 0..moves.len() {
        let mut c = b.clone();
        c.do_move(moves[i], &mut st);
        out.push(key_of(&c));
    }
    out
}

/// Exact AND/OR fixpoint over the region reachable from `root`.
/// Returns (value map, distance map, region size, mismatch count vs table).
fn t1a_region(
    root_key: u32,
    table: &[u8],
) -> (HashMap<u32, u8>, HashMap<u32, u32>, usize, usize) {
    // Forward BFS over the reachable region.
    let mut region: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut queue = vec![root_key];
    region.insert(root_key, Vec::new());
    while let Some(k) = queue.pop() {
        let b = board_from_key(k);
        if classify_terminal(&b).is_some() {
            continue;
        }
        let children = child_keys(&b);
        for c in children {
            if !region.contains_key(&c) {
                region.insert(c, Vec::new());
                queue.push(c);
            }
        }
        region.get_mut(&k).unwrap().clear();
        let b = board_from_key(k);
        region.get_mut(&k).unwrap().extend(child_keys(&b));
    }
    let size = region.len();

    // Layered AND/OR fixpoint with exact distances.
    let mut val: HashMap<u32, u8> = HashMap::with_capacity(size);
    let mut dist: HashMap<u32, u32> = HashMap::with_capacity(size);
    for &k in region.keys() {
        let b = board_from_key(k);
        if let Some(t) = classify_terminal(&b) {
            val.insert(k, t);
            dist.insert(k, 0);
        } else {
            val.insert(k, UNDECIDED);
        }
    }
    let keys: Vec<u32> = region.keys().copied().collect();
    loop {
        let mut changed = false;
        for &k in &keys {
            if val[&k] != UNDECIDED {
                continue;
            }
            let children = &region[&k];
            let b = board_from_key(k);
            let attacker = b.side_to_move() == strong_color(&b);
            if attacker {
                // OR node: the attacker wins if any child is attacker-won
                // (children of an attacker node are defender-to-move
                // positions; WIN there means the defender loses). A LOSS
                // child (attacker mated) is bad for the attacker and skipped.
                for &c in children {
                    if val[&c] == WIN {
                        val.insert(k, WIN);
                        dist.insert(k, dist[&c] + 1);
                        changed = true;
                        break;
                    }
                }
            } else {
                // AND node: the defender escapes if any child is a draw (or
                // an attacker-mated terminal, which cannot occur here); the
                // attacker wins (in `maxd + 1`) only if every child is
                // attacker-won.
                let mut maxd = 0u32;
                let mut all_win = true;
                for &c in children {
                    match val[&c] {
                        WIN => maxd = maxd.max(dist[&c]),
                        DRAW => {
                            val.insert(k, DRAW);
                            changed = true;
                            all_win = false;
                            break;
                        }
                        LOSS => {
                            // Attacker mated below a defender node: the
                            // defender would choose it. Cannot occur from a
                            // legal root at 3 men.
                            val.insert(k, DRAW);
                            changed = true;
                            all_win = false;
                            break;
                        }
                        _ => {
                            all_win = false;
                            break;
                        }
                    }
                }
                if all_win && !children.is_empty() {
                    val.insert(k, WIN);
                    dist.insert(k, maxd + 1);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    // Remaining undecided positions are genuine draws (fortress region).
    for (&k, v) in val.iter_mut() {
        if *v == UNDECIDED {
            *v = DRAW;
            dist.remove(&k);
        }
    }

    // Exact distances: relax dtm to the fixpoint (attacker: 1 + min child
    // dist; defender: 1 + max child dist). The decision loop above decides a
    // node on the first qualifying child, which can overestimate dist.
    loop {
        let mut changed = false;
        for &k in &keys {
            if val[&k] != WIN {
                continue;
            }
            let children = &region[&k];
            let b = board_from_key(k);
            let attacker = b.side_to_move() == strong_color(&b);
            let d = if attacker {
                children
                    .iter()
                    .filter(|&c| val[c] == WIN)
                    .map(|c| dist[c])
                    .min()
            } else {
                children.iter().map(|c| dist[c]).max()
            };
            if let Some(d) = d
                && dist[&k] != d + 1
            {
                dist.insert(k, d + 1);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    // TEMPORARY debug: value distribution + terminal classification counts.
    let mut counts = [0u64; 4];
    let mut term_counts = [0u64; 4];
    for &k in &keys {
        counts[val[&k] as usize] += 1;
        if classify_terminal(&board_from_key(k)).is_some() {
            term_counts[val[&k] as usize] += 1;
        }
    }
    eprintln!(
        "debug: val counts UND/DRAW/WIN/LOSS = {:?}, terminal = {:?}",
        counts, term_counts
    );

    // Cross-check every decided region value against the q-table. The table
    // is coded from the position's side-to-move perspective, so the mapping
    // depends on which side owns the queen.
    let mut mismatches = 0usize;
    for (&k, &v) in &val {
        let t = table[k as usize];
        let b = board_from_key(k);
        let attacker_stm = b.side_to_move() == strong_color(&b);
        let expect = match (v, attacker_stm) {
            (WIN, true) => 2u8,
            (WIN, false) => 0u8, // defender to move, defender loses
            (LOSS, true) => 0u8, // attacker to move, attacker loses
            (LOSS, false) => 2u8,
            _ => 1u8,
        };
        if t != 3 && t != expect {
            mismatches += 1;
            if mismatches <= 5 {
                eprintln!(
                    "T1a table mismatch at key {k} ({}): table={t} derived={v}",
                    board_from_key(k).fen()
                );
            }
        }
    }
    (val, dist, size, mismatches)
}

struct WalkStats {
    lines: u64,
    mate_leaves: u64,
    cycles: u64,
    budget_exhausted: u64,
    nodes: u64,
    max_plies: u32,
}

/// Enumerate every line under the rank-decreasing strategy (attacker plays a
/// child with dtm exactly `d_left - 1`; defender plays all moves). Every line
/// must end in mate without repeating a board.
fn walk_ranked(
    key: u32,
    d_left: u32,
    val: &HashMap<u32, u8>,
    dist: &HashMap<u32, u32>,
    path: &mut Vec<u32>,
    stats: &mut WalkStats,
) -> bool {
    stats.nodes += 1;
    let b = board_from_key(key);
    if let Some(t) = classify_terminal(&b) {
        if t == T_WIN0 {
            stats.lines += 1;
            stats.mate_leaves += 1;
            stats.max_plies = stats.max_plies.max(path.len() as u32);
            return true;
        }
        return false; // draw terminal under a winning strategy: failure
    }
    if d_left == 0 {
        stats.budget_exhausted += 1;
        return false;
    }
    let attacker = b.side_to_move() == strong_color(&b);
    let children = child_keys(&b);
    if attacker {
        // Rank-decreasing choice: child (defender to move) attacker-won with
        // dtm exactly d_left - 1 — the largest child dtm available, i.e. the
        // most progressive winning move.
        let mut chosen = None;
        for &c in &children {
            if val[&c] == WIN && dist[&c] == d_left - 1 {
                chosen = Some(c);
                break;
            }
        }
        let Some(c) = chosen else {
            stats.budget_exhausted += 1;
            return false;
        };
        if path.contains(&c) {
            stats.cycles += 1;
            return false;
        }
        path.push(c);
        let ok = walk_ranked(c, d_left - 1, val, dist, path, stats);
        path.pop();
        ok
    } else {
        // Defender node: every reply must be covered.
        let mut ok = true;
        for &c in &children {
            if !ok {
                break;
            }
            if path.contains(&c) {
                stats.cycles += 1;
                return false;
            }
            if val[&c] != WIN {
                stats.budget_exhausted += 1;
                return false;
            }
            let d_c = dist[&c];
            if d_c >= d_left {
                stats.budget_exhausted += 1;
                return false;
            }
            path.push(c);
            ok &= walk_ranked(c, d_c, val, dist, path, stats);
            path.pop();
        }
        if ok {
            stats.lines += 1;
        }
        ok
    }
}

/// Naive greedy probe: attacker plays the first table-winning child (any
/// dtm); detect cycles, bounded effort.
fn walk_greedy(
    key: u32,
    d_left: u32,
    val: &HashMap<u32, u8>,
    dist: &HashMap<u32, u32>,
    table: &[u8],
    path: &mut Vec<u32>,
    stats: &mut WalkStats,
    node_cap: u64,
) -> bool {
    stats.nodes += 1;
    if stats.nodes > node_cap {
        return false;
    }
    let b = board_from_key(key);
    if let Some(t) = classify_terminal(&b) {
        if t == T_WIN0 {
            stats.lines += 1;
            stats.mate_leaves += 1;
            stats.max_plies = stats.max_plies.max(path.len() as u32);
            return true;
        }
        return false;
    }
    if d_left == 0 {
        stats.budget_exhausted += 1;
        return false;
    }
    let attacker = b.side_to_move() == strong_color(&b);
    let children = child_keys(&b);
    if attacker {
        let mut chosen = None;
        for &c in &children {
            if table[c as usize] == 0 {
                // First table-Loss child (defender to move loses).
                chosen = Some(c);
                break;
            }
        }
        let Some(c) = chosen else {
            stats.budget_exhausted += 1;
            return false;
        };
        if path.contains(&c) {
            stats.cycles += 1;
            return false;
        }
        let d_c = dist.get(&c).copied().unwrap_or(0);
        path.push(c);
        let ok = walk_greedy(c, d_c.max(1), val, dist, table, path, stats, node_cap);
        path.pop();
        ok
    } else {
        let mut ok = true;
        for &c in &children {
            if !ok {
                break;
            }
            if path.contains(&c) {
                stats.cycles += 1;
                return false;
            }
            if val[&c] != WIN {
                stats.budget_exhausted += 1;
                return false;
            }
            let d_c = dist[&c];
            path.push(c);
            ok &= walk_greedy(c, d_c, val, dist, table, path, stats, node_cap);
            path.pop();
        }
        if ok {
            stats.lines += 1;
        }
        ok
    }
}

// ---------- T1b ----------

const P_UNKNOWN: u8 = 0;
const P_NOT_WIN: u8 = 1;
const P_WIN: u8 = 2;

/// Sorted-vector subset test.
fn is_subset(a: &[u32], b: &[u32]) -> bool {
    a.iter().all(|x| b.binary_search(x).is_ok())
}

/// Repetition-aware bounded prover (plan12 T1b, primary G1 instrument).
///
/// Values are from the attacker's perspective: WIN = attacker mates within
/// `depth`, NOT_WIN = defender forces a non-mate (draw/repetition), UNKNOWN =
/// inconclusive. A position on the current line is a Draw (solver's
/// path-repetition semantics), so a WIN claim is cycle-free by construction.
///
/// Memoization is one-sided-sound (subset transfer):
/// - a WIN proven under ancestor set `A` (whose lines repeat no member of
///   `A`) remains valid under any `A' ⊆ A` — fewer forbidden repeats — so a
///   probe hits when the current path is a subset of a stored proof's
///   ancestor set;
/// - a NOT_WIN proven under `A` remains valid under any `A' ⊇ A` — the
///   defender keeps every escape — so a probe hits when a stored set is a
///   subset of the current path.
///
/// UNKNOWN values are not memoized. The egtb q-table is used for move
/// ORDERING only (winning children first for the attacker, likely escapes
/// first for the defender); every claim is still fully verified by the
/// recursion, so the prover stays independent of the table for its verdicts.
struct Prover {
    memo_win: HashMap<(u32, u32), Vec<u32>>,
    memo_nw: HashMap<(u32, u32), Vec<u32>>,
    memo_cap: usize,
    memo_resets: u64,
    win_subset_hits: u64,
    nw_subset_hits: u64,
    nodes: u64,
    node_cap: u64,
    cut_by_rep: u64,
    strong: Color,
    table: Vec<u8>,
}

impl Prover {
    fn prove(&mut self, key: u32, depth: u32, path: &[u32]) -> u8 {
        self.nodes += 1;
        if self.nodes > self.node_cap {
            return P_UNKNOWN;
        }
        let b = board_from_key(key);
        if let Some(t) = classify_terminal(&b) {
            return match t {
                T_WIN0 => P_WIN,
                _ => P_NOT_WIN,
            };
        }
        if path.contains(&key) {
            self.cut_by_rep += 1;
            return P_NOT_WIN;
        }
        if depth == 0 {
            return P_UNKNOWN;
        }
        let mkey = (key, depth);
        // Sorted copy of the path for subset tests (paths are ≤ 17 entries).
        let mut path_sorted: Vec<u32> = path.to_vec();
        path_sorted.sort_unstable();
        path_sorted.dedup();
        if let Some(stored) = self.memo_win.get(&mkey)
            && is_subset(&path_sorted, stored)
        {
            self.win_subset_hits += 1;
            return P_WIN;
        }
        if let Some(stored) = self.memo_nw.get(&mkey)
            && is_subset(stored, &path_sorted)
        {
            self.nw_subset_hits += 1;
            return P_NOT_WIN;
        }

        let mut moves = MoveList::new();
        generate_legal(&b, &mut moves);
        let mut st = StateInfo::new();
        let attacker = b.side_to_move() == self.strong;
        // Child candidate collection. SOUND PRUNING (monotonicity lemma):
        // the repetition-semantics game value for the attacker is ≤ the
        // board-only game value (repetition cuts only replace outcomes with
        // draws), so a table-Draw child can never be a repetition-semantics
        // win. At attacker nodes we therefore only attempt table-Loss
        // children (the q-table's winning moves); if none verifies, the node
        // is UNKNOWN — never a claim. WIN claims themselves are still fully
        // verified by the recursion (real movegen + path-repetition cuts),
        // independent of the table.
        let mut order: Vec<(Move, u8)> = Vec::with_capacity(moves.len());
        for i in 0..moves.len() {
            let mut c = b.clone();
            c.do_move(moves[i], &mut st);
            let ck = key_of(&c);
            if attacker && self.table[ck as usize] != 0 {
                continue; // monotonicity: table-Draw child cannot be a win
            }
            // Order key: repetition-cut children are free; then checks; then
            // the rest. Defender nodes try likely escapes (table-Draw
            // children) first.
            let rank = if path.contains(&ck) {
                0u8
            } else if attacker {
                u8::from(!c.checkers().is_empty())
            } else {
                match self.table[ck as usize] {
                    1 => 0,
                    _ => u8::from(!c.checkers().is_empty()) + 1,
                }
            };
            order.push((moves[i], rank));
        }
        order.sort_by_key(|&(_, rank)| rank);
        if attacker && order.is_empty() {
            // No table-Loss children: the node cannot be a win (monotonicity).
            return P_UNKNOWN;
        }

        let mut child_path_buf: Vec<u32> = Vec::with_capacity(path.len() + 1);
        let mut any_unknown = false;
        let mut result: u8 = if attacker { P_NOT_WIN } else { P_WIN };
        for &(m, _) in &order {
            let mut c = b.clone();
            c.do_move(m, &mut st);
            let ck = key_of(&c);
            let child_val = if path.contains(&ck) {
                self.cut_by_rep += 1;
                P_NOT_WIN
            } else {
                child_path_buf.clear();
                child_path_buf.extend_from_slice(path);
                child_path_buf.push(key);
                self.prove(ck, depth - 1, &child_path_buf)
            };
            if attacker {
                match child_val {
                    P_WIN => {
                        result = P_WIN;
                        break;
                    }
                    P_UNKNOWN => any_unknown = true,
                    _ => {}
                }
            } else {
                match child_val {
                    P_NOT_WIN => {
                        result = P_NOT_WIN;
                        break;
                    }
                    P_UNKNOWN => any_unknown = true,
                    _ => {}
                }
            }
        }
        // The optimistic side cannot be claimed while children are unknown.
        if any_unknown && result == if attacker { P_NOT_WIN } else { P_WIN } {
            result = P_UNKNOWN;
        }
        // Store decided results with subset-transfer semantics.
        if result != P_UNKNOWN {
            if self.memo_win.len() + self.memo_nw.len() >= self.memo_cap {
                self.memo_win.clear();
                self.memo_nw.clear();
                self.memo_resets += 1;
            }
            if result == P_WIN {
                let entry = self.memo_win.entry(mkey).or_insert_with(|| path_sorted.clone());
                // Keep the smallest known proof set (subset of the old one).
                if is_subset(&path_sorted, entry) {
                    *entry = path_sorted;
                }
            } else {
                let entry = self.memo_nw.entry(mkey).or_insert_with(|| path_sorted.clone());
                // Keep the smallest stored set (transfers to the most paths).
                if is_subset(&path_sorted, entry) {
                    *entry = path_sorted;
                }
            }
        }
        result
    }
}

fn load_table(path: &str) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    assert_eq!(bytes.len(), 1 << 20, "unexpected table size");
    bytes
}

fn resolve_root(fen: &str, table: &[u8]) -> (u32, u8) {
    let b = Board::from_fen(fen).unwrap();
    assert_eq!(b.occupied().count(), 3, "not a 3-man position: {fen}");
    let key = key_of(&b);
    (key, table[key as usize])
}

fn main() {
    let mut args = std::env::args().skip(1);
    let table_path = args.next().expect("q-table path required");
    let fen = args.next().expect("root FEN required");
    let mut t1b_depths: Vec<u32> = vec![15, 16, 17];
    let mut greedy_node_cap: u64 = 20_000_000;
    let mut memo_cap: usize = 4_000_000;
    let mut node_cap: u64 = 500_000_000;
    let mut do_t1a = true;
    let mut do_t1b = true;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--t1b-depths" => {
                t1b_depths = args
                    .next()
                    .unwrap()
                    .split(',')
                    .map(|s| s.parse().unwrap())
                    .collect();
            }
            "--greedy-node-cap" => greedy_node_cap = args.next().unwrap().parse().unwrap(),
            "--memo-cap" => memo_cap = args.next().unwrap().parse().unwrap(),
            "--node-cap" => node_cap = args.next().unwrap().parse().unwrap(),
            "--no-t1a" => do_t1a = false,
            "--no-t1b" => do_t1b = false,
            _ => panic!("unknown arg {a}"),
        }
    }
    let table = load_table(&table_path);
    let (root_key, table_val) = resolve_root(&fen, &table);
    println!("plan12_t1: fen=\"{fen}\" root_key={root_key} table_value={table_val} (2=Win)");

    if do_t1a {
        let t0 = std::time::Instant::now();
        let (val, dist, size, mismatches) = t1a_region(root_key, &table);
        let root_val = val[&root_key];
        let root_d = dist.get(&root_key).copied().unwrap_or(0);
        println!(
            "plan12_t1 t1a: region={size} root_value={} dtm={root_d} mismatches_vs_table={mismatches} wall={:.2}s",
            if root_val == WIN { "WIN" } else { "NOT_WIN" },
            t0.elapsed().as_secs_f64(),
        );
        if root_val == WIN {
            // Rank-decreasing strategy: enumerate every line.
            let t0 = std::time::Instant::now();
            let mut stats = WalkStats {
                lines: 0,
                mate_leaves: 0,
                cycles: 0,
                budget_exhausted: 0,
                nodes: 0,
                max_plies: 0,
            };
            let mut path = vec![root_key];
            let ok = walk_ranked(root_key, root_d, &val, &dist, &mut path, &mut stats);
            println!(
                "plan12_t1 t1a ranked-strategy: cycle_free_all_lines={ok} lines={} mate_leaves={} cycles={} budget_exhausted={} nodes={} max_line_plies={} wall={:.2}s",
                stats.lines,
                stats.mate_leaves,
                stats.cycles,
                stats.budget_exhausted,
                stats.nodes,
                stats.max_plies,
                t0.elapsed().as_secs_f64(),
            );
            // Naive greedy probe (first table-winning child, any dtm).
            let t0 = std::time::Instant::now();
            let mut gstats = WalkStats {
                lines: 0,
                mate_leaves: 0,
                cycles: 0,
                budget_exhausted: 0,
                nodes: 0,
                max_plies: 0,
            };
            let mut gpath = vec![root_key];
            let gok = walk_greedy(
                root_key,
                root_d,
                &val,
                &dist,
                &table,
                &mut gpath,
                &mut gstats,
                greedy_node_cap,
            );
            let effort = if gstats.nodes > greedy_node_cap {
                " (effort cap hit)"
            } else {
                ""
            };
            println!(
                "plan12_t1 t1a greedy-probe: cycle_free_all_lines={gok}{effort} lines={} mate_leaves={} cycles={} budget_exhausted={} nodes={} max_line_plies={} wall={:.2}s",
                gstats.lines,
                gstats.mate_leaves,
                gstats.cycles,
                gstats.budget_exhausted,
                gstats.nodes,
                gstats.max_plies,
                t0.elapsed().as_secs_f64(),
            );
        }
    }

    if do_t1b {
        let b = Board::from_fen(&fen).unwrap();
        let strong = strong_color(&b);
        for &d in &t1b_depths {
            let t0 = std::time::Instant::now();
            let mut prover = Prover {
                memo_win: HashMap::with_capacity(memo_cap.min(1 << 22)),
                memo_nw: HashMap::with_capacity(memo_cap.min(1 << 22)),
                memo_cap,
                memo_resets: 0,
                win_subset_hits: 0,
                nw_subset_hits: 0,
                nodes: 0,
                node_cap,
                cut_by_rep: 0,
                strong,
                table: table.clone(),
            };
            let v = prover.prove(root_key, d, &[]);
            println!(
                "plan12_t1 t1b: depth={d} result={} nodes={} memo_win={} memo_nw={} win_subset_hits={} nw_subset_hits={} memo_resets={} repetition_cuts={} wall={:.2}s",
                match v {
                    P_WIN => "WIN",
                    P_NOT_WIN => "NOT_WIN",
                    _ => "UNKNOWN",
                },
                prover.nodes,
                prover.memo_win.len(),
                prover.memo_nw.len(),
                prover.win_subset_hits,
                prover.nw_subset_hits,
                prover.memo_resets,
                prover.cut_by_rep,
                t0.elapsed().as_secs_f64(),
            );
        }
    }
}
