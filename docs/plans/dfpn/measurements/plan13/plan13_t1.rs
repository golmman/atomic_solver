//! TEMPORARY plan13 Phase 0 T1 instrument — spike code, reverted after
//! measuring. Source archived under `docs/plans/dfpn/measurements/plan13/`.
//!
//! plan13 Phase 0 architecture bake-off (table-free): three bounded pre-phase
//! candidates on the KQvK ladder, all sharing the solver's terminal
//! classification and real `atomic_movegen` movegen, all enforcing the
//! solver's path-repetition semantics (a position recurring on the current
//! line is a Draw for the attacker), and all emitting WIN claims only with a
//! replay-verifiable certificate: a full attacking strategy whose complete
//! line enumeration is cycle-free and mates within the bound.
//!
//! Candidates (plan13 §T1):
//! - **R — region-closure fixpoint.** Forward BFS closure within a position
//!   budget, AND/OR fixpoint with exact DTM ranks over the closure
//!   (plan12 T1a machinery, table-free), rank-decreasing strategy extraction.
//!   The horizon is the certificate bound: every certified line must mate
//!   within `h` plies, so `h = dtm - 1` correctly yields UNKNOWN.
//! - **L — layered certificate fixpoint.** Iterated depth layers
//!   d = 1..=horizon with a persistent fact set. WIN facts are stored
//!   certificate-first (chosen move + board set of the strategy subtree) and
//!   are therefore context-free: valid under any path disjoint from the
//!   fact's board set — no subset tests (the plan's hypothesis under test).
//!   Oversized board sets and NOT_WIN facts fall back to the plan12 T1b
//!   subset-transfer lemma (valid under ancestor supersets).
//! - **P — PN bounded prover.** Proof-number/disproof-number search to a
//!   fixed depth horizon, repetition cut on the path, subset-transfer memo.
//!   PN's least-work-first pricing replaces both DF-PN threshold arithmetic
//!   and the table guidance plan12 T1b leaned on.
//!
//! Soundness notes shared by all candidates:
//! - Monotonicity lemma (plan12 T1b): the repetition-semantics game value
//!   for the attacker is ≤ the board-only value (repetition cuts only
//!   replace outcomes with draws). Hence a board-only Draw (R's fixpoint
//!   remainder) is a genuine repetition-semantics Draw, and no candidate can
//!   claim a WIN the board-only game refutes.
//! - Every WIN claim is downgraded to UNKNOWN unless its certificate's full
//!   line enumeration replays cycle-free (no board repeats on any line) and
//!   mates within the bound. This is the defense-in-depth gate (plan13 R3):
//!   a cyclic position cannot yield a cycle-free certificate.
//! - Depth ≤ 17 with root halfmove clocks ≤ 6 keeps the halfmove clock < 100
//!   on every line, so rule50 cannot fire and the clock is excluded from the
//!   state (documented limitation, same as plan12 T1b).
//! - KQvK invariant: no capture is legally possible (a king capturing an
//!   adjacent-protected piece explodes both kings; the queen can never be
//!   captured by the weak king), so `occupied == 3` on every reachable
//!   position and the packed key is always defined.
//! - The egtb q-table (`--qtable`) is used ONLY as a validation oracle (R
//!   region cross-check, `--oracle` sampling); it is never search input.
//!
//! Usage:
//!   plan13_t1 <fen> [--cand all|R|L|P] [--horizons 14,15,17]
//!             [--eval-cap N] [--wall-cap S] [--region-budget N]
//!             [--node-cap N] [--qtable PATH]
//!   plan13_t1 --oracle N --qtable PATH [--shard I] [--shards K]
//!             [--oracle-h H] [--oracle-eval-cap N] [--oracle-wall-cap S]

use atomic_movegen::board::{Board, StateInfo};
use atomic_movegen::movegen::generate_legal;
use atomic_movegen::types::{Color, Move, MoveList, PieceType};
use std::collections::HashMap;

// ---------- common 3-man infrastructure (plan12 T1 pattern) ----------

const UNDECIDED: u8 = 0;
const DRAW: u8 = 1;
const WIN: u8 = 2;
const LOSS: u8 = 3;

const T_DRAW: u8 = 1;
const T_WIN0: u8 = 2; // side to move is mated
const T_LOSS: u8 = 3; // side to move has no king (cannot occur from legal roots)

const P_UNKNOWN: u8 = 0;
const P_NOT_WIN: u8 = 1;
const P_WIN: u8 = 2;

fn strong_color(b: &Board) -> Color {
    if b.pieces_color_pt(Color::White, PieceType::Queen).is_empty() {
        Color::Black
    } else {
        Color::White
    }
}

/// Packed position key == q-table index (see `examples/egtb_gen3/table.rs`).
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

fn cheb(a: u32, b: u32) -> u32 {
    (a % 8).abs_diff(b % 8).max((a / 8).abs_diff(b / 8))
}

/// Sorted-vector subset test (plan12 T1b).
fn is_subset(a: &[u32], b: &[u32]) -> bool {
    a.iter().all(|x| b.binary_search(x).is_ok())
}

// ---------- counters / caps ----------

struct Ctx {
    evals: u64,
    eval_cap: u64,
    deadline: Option<std::time::Instant>,
    aborted: bool,
    abort_reason: &'static str,
    cut_by_rep: u64,
    nodes: u64,
}

impl Ctx {
    fn new(eval_cap: u64, wall_cap: f64) -> Self {
        Ctx {
            evals: 0,
            eval_cap,
            deadline: if wall_cap > 0.0 && wall_cap.is_finite() && wall_cap < 1.0e9 {
                Some(std::time::Instant::now() + std::time::Duration::from_secs_f64(wall_cap))
            } else {
                None
            },
            aborted: false,
            abort_reason: "",
            cut_by_rep: 0,
            nodes: 0,
        }
    }
    #[inline]
    fn tick(&mut self) {
        if self.evals > self.eval_cap {
            self.abort("eval-cap");
        }
        if !self.aborted && let Some(d) = self.deadline && std::time::Instant::now() >= d {
            self.abort("wall-cap");
        }
    }
    fn abort(&mut self, why: &'static str) {
        if !self.aborted {
            self.aborted = true;
            self.abort_reason = why;
        }
    }
}

/// One child evaluation: clone + do_move + packed key. KQvK invariant: no
/// capture is legally reachable, so the child always has 3 men.
fn child_board(b: &Board, m: Move, ctx: &mut Ctx) -> (Board, u32) {
    ctx.evals += 1;
    ctx.tick();
    let mut c = b.clone();
    let mut st = StateInfo::new();
    c.do_move(m, &mut st);
    debug_assert_eq!(c.occupied().count(), 3, "KQvK capture invariant violated");
    let k = key_of(&c);
    (c, k)
}

// ---------- certificate replay (shared verifier, plan13 G-B) ----------

#[derive(Clone)]
struct CertStats {
    lines: u64,
    mates: u64,
    repeats: u64,
    budget_exits: u64,
    bad_strategy: u64,
    nodes: u64,
    max_plies: u32,
}

/// Replay-verify a strategy (`strat`: attacker-node key -> certified child
/// key) from `root` under the path-repetition semantics: at attacker nodes
/// play the certified move, at defender nodes branch over ALL legal replies;
/// every line must end in mate within `bound` plies with no board repeating
/// anywhere on the line. Any defect fails the whole certificate.
fn replay_cert(root: &Board, strat: &HashMap<u32, u32>, bound: u32) -> (bool, CertStats) {
    let mut st = CertStats {
        lines: 0,
        mates: 0,
        repeats: 0,
        budget_exits: 0,
        bad_strategy: 0,
        nodes: 0,
        max_plies: 0,
    };
    let mut path = vec![key_of(root)];
    let ok = replay_walk(root, bound, 0, &mut path, strat, &mut st);
    (ok, st)
}

fn replay_walk(
    b: &Board,
    bound: u32,
    plies: u32,
    path: &mut Vec<u32>,
    strat: &HashMap<u32, u32>,
    st: &mut CertStats,
) -> bool {
    st.nodes += 1;
    if let Some(t) = classify_terminal(b) {
        if t == T_WIN0 {
            st.lines += 1;
            st.mates += 1;
            st.max_plies = st.max_plies.max(plies);
            return true;
        }
        if std::env::var("PLAN13_DEBUG").is_ok() {
            eprintln!("replay: draw terminal at plies={plies} fen={}", b.fen());
        }
        st.bad_strategy += 1; // draw terminal under a winning strategy
        return false;
    }
    if plies >= bound {
        if std::env::var("PLAN13_DEBUG").is_ok() {
            eprintln!("replay: budget exit at plies={plies} fen={}", b.fen());
        }
        st.budget_exits += 1;
        return false;
    }
    let attacker = b.side_to_move() == strong_color(b);
    let mut moves = MoveList::new();
    generate_legal(b, &mut moves);
    let mut si = StateInfo::new();
    if attacker {
        let Some(&ck) = strat.get(&key_of(b)) else {
            st.bad_strategy += 1;
            return false;
        };
        let mut found: Option<Board> = None;
        for i in 0..moves.len() {
            let mut c = b.clone();
            c.do_move(moves[i], &mut si);
            if key_of(&c) == ck {
                found = Some(c);
                break;
            }
        }
        let Some(c) = found else {
            if std::env::var("PLAN13_DEBUG").is_ok() {
                eprintln!("replay: certified move missing at plies={plies} fen={}", b.fen());
            }
            st.bad_strategy += 1; // certified move not legal here
            return false;
        };
        if path.contains(&ck) {
            st.repeats += 1;
            return false;
        }
        path.push(ck);
        let ok = replay_walk(&c, bound, plies + 1, path, strat, st);
        path.pop();
        ok
    } else {
        // Defender node: every reply must be covered; no short-circuit on
        // success, but stop at the first failing line (verdict is false
        // regardless).
        let mut ok = true;
        for i in 0..moves.len() {
            let mut c = b.clone();
            c.do_move(moves[i], &mut si);
            let ck = key_of(&c);
            if path.contains(&ck) {
                st.repeats += 1;
                return false;
            }
            path.push(ck);
            ok = replay_walk(&c, bound, plies + 1, path, strat, st);
            path.pop();
            if !ok {
                return false;
            }
        }
        st.lines += 1;
        ok
    }
}

// ---------- candidate R: region-closure fixpoint ----------

struct ROutcome {
    horizon: u32,
    result: &'static str,
    evals: u64,
    scans: u64,
    nodes: u64,
    region: usize,
    wall: f64,
    dtm: u32,
    cert: Option<CertStats>,
    cert_wall: f64,
    table_mismatches: usize,
    reason: &'static str,
}

/// R — region-closure fixpoint (plan13 T1-R). Forward BFS closure within
/// `region_budget`, AND/OR fixpoint with exact ranks (plan12 T1a, table-free),
/// rank-decreasing strategy replayed per horizon bound. The fixpoint runs
/// once; each horizon only changes the certificate bound. Root board-DRAW is
/// a genuine repetition-semantics Draw by the monotonicity lemma.
fn run_r(
    fen: &str,
    horizons: &[u32],
    region_budget: usize,
    ctx: &mut Ctx,
    qtable: Option<&[u8]>,
) -> Vec<ROutcome> {
    let t0 = std::time::Instant::now();
    let root = Board::from_fen(fen).unwrap();
    let root_key = key_of(&root);

    // Forward BFS closure (child evals counted).
    let mut keys: Vec<u32> = vec![root_key];
    let mut index: HashMap<u32, u32> = HashMap::new();
    index.insert(root_key, 0);
    let mut children_idx: Vec<Vec<u32>> = vec![Vec::new()];
    let mut terminal: Vec<Option<u8>> = vec![None];
    let mut attacker: Vec<bool> = vec![
        root.side_to_move() == strong_color(&root),
    ];
    let mut qi = 0usize;
    while qi < keys.len() {
        let idx = qi;
        qi += 1;
        let b = board_from_key(keys[idx]);
        if let Some(t) = classify_terminal(&b) {
            terminal[idx] = Some(t);
            continue;
        }
        let mut moves = MoveList::new();
        generate_legal(&b, &mut moves);
        let mut si = StateInfo::new();
        for i in 0..moves.len() {
            let (c, ck) = child_board(&b, moves[i], ctx);
            let cid = *index.entry(ck).or_insert_with(|| {
                keys.push(ck);
                children_idx.push(Vec::new());
                terminal.push(classify_terminal(&c));
                attacker.push(c.side_to_move() == strong_color(&c));
                (keys.len() - 1) as u32
            });
            children_idx[idx].push(cid);
        }
        if ctx.aborted {
            break;
        }
        if keys.len() > region_budget {
            ctx.abort("region-budget");
            break;
        }
    }
    let region = keys.len();

    let mut outcomes: Vec<ROutcome> = Vec::new();
    if ctx.aborted {
        for &h in horizons {
            outcomes.push(ROutcome {
                horizon: h,
                result: "UNKNOWN",
                evals: ctx.evals,
                scans: 0,
                nodes: ctx.nodes,
                region,
                wall: t0.elapsed().as_secs_f64(),
                dtm: 0,
                cert: None,
                cert_wall: 0.0,
                table_mismatches: 0,
                reason: ctx.abort_reason,
            });
        }
        return outcomes;
    }

    // AND/OR fixpoint with exact distances (plan12 T1a, delta-free passes).
    let mut val: Vec<u8> = Vec::with_capacity(region);
    let mut dist: Vec<u32> = vec![0; region];
    for idx in 0..region {
        match terminal[idx] {
            Some(t) => val.push(t),
            None => val.push(UNDECIDED),
        }
    }
    let mut scans: u64 = 0;
    loop {
        let mut changed = false;
        for k in 0..region {
            if val[k] != UNDECIDED {
                continue;
            }
            if attacker[k] {
                for &c in &children_idx[k] {
                    scans += 1;
                    if val[c as usize] == WIN {
                        val[k] = WIN;
                        dist[k] = dist[c as usize] + 1;
                        changed = true;
                        break;
                    }
                }
            } else {
                let mut maxd = 0u32;
                let mut all_win = true;
                for &c in &children_idx[k] {
                    scans += 1;
                    match val[c as usize] {
                        WIN => maxd = maxd.max(dist[c as usize]),
                        DRAW | LOSS => {
                            val[k] = DRAW;
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
                if all_win && !children_idx[k].is_empty() {
                    val[k] = WIN;
                    dist[k] = maxd + 1;
                    changed = true;
                }
            }
        }
        if !changed || ctx.aborted {
            break;
        }
    }
    // Remaining undecided positions are genuine draws (monotonicity lemma).
    for v in val.iter_mut() {
        if *v == UNDECIDED {
            *v = DRAW;
        }
    }
    // Exact-distance relaxation (attacker: 1 + min child dist; defender:
    // 1 + max child dist).
    loop {
        let mut changed = false;
        for k in 0..region {
            if val[k] != WIN {
                continue;
            }
            let d = if attacker[k] {
                children_idx[k]
                    .iter()
                    .filter(|&&c| val[c as usize] == WIN)
                    .map(|&c| dist[c as usize])
                    .min()
            } else {
                children_idx[k].iter().map(|&c| dist[c as usize]).max()
            };
            if let Some(d) = d && dist[k] != d + 1 {
                dist[k] = d + 1;
                changed = true;
            }
            scans += children_idx[k].len() as u64;
        }
        if !changed || ctx.aborted {
            break;
        }
    }

    // Table cross-check (oracle only).
    let mut table_mismatches = 0usize;
    if let Some(table) = qtable {
        for k in 0..region {
            let t = table[keys[k] as usize];
            let expect = match (val[k], attacker[k]) {
                (WIN, true) => 2u8,
                (WIN, false) => 0u8,
                (LOSS, true) => 0u8,
                (LOSS, false) => 2u8,
                _ => 1u8,
            };
            if t != 3 && t != expect {
                table_mismatches += 1;
            }
        }
    }

    let fix_wall = t0.elapsed().as_secs_f64();
    let root_val = val[0];
    let root_dtm = dist[0];

    for &h in horizons {
        if root_val != WIN {
            outcomes.push(ROutcome {
                horizon: h,
                result: if root_val == DRAW { "DRAW" } else { "UNKNOWN" },
                evals: ctx.evals,
                scans,
                nodes: ctx.nodes,
                region,
                wall: fix_wall,
                dtm: root_dtm,
                cert: None,
                cert_wall: 0.0,
                table_mismatches,
                reason: "",
            });
            continue;
        }
        // Rank-decreasing strategy replay under bound h.
        let tc0 = std::time::Instant::now();
        let mut st = CertStats {
            lines: 0,
            mates: 0,
            repeats: 0,
            budget_exits: 0,
            bad_strategy: 0,
            nodes: 0,
            max_plies: 0,
        };
        let ok = replay_r_walk(
            0,
            root_dtm,
            h,
            0,
            &mut vec![root_key],
            &val,
            &dist,
            &children_idx,
            &attacker,
            &terminal,
            &keys,
            &mut st,
        );
        let cert_wall = tc0.elapsed().as_secs_f64();
        outcomes.push(ROutcome {
            horizon: h,
            result: if ok { "WIN_CERTIFIED" } else { "UNKNOWN" },
            evals: ctx.evals,
            scans,
            nodes: ctx.nodes,
            region,
            wall: fix_wall,
            dtm: root_dtm,
            cert: Some(st),
            cert_wall,
            table_mismatches,
            reason: if ok { "" } else { "cert-fail" },
        });
    }
    outcomes
}

/// Rank-decreasing strategy replay (plan12 T1a walk, bound = line length).
#[allow(clippy::too_many_arguments)]
fn replay_r_walk(
    idx: u32,
    rank: u32,
    bound: u32,
    plies: u32,
    path: &mut Vec<u32>,
    val: &[u8],
    dist: &[u32],
    children_idx: &[Vec<u32>],
    attacker: &[bool],
    terminal: &[Option<u8>],
    keys: &[u32],
    st: &mut CertStats,
) -> bool {
    st.nodes += 1;
    if let Some(t) = terminal[idx as usize] {
        if t == T_WIN0 {
            st.lines += 1;
            st.mates += 1;
            st.max_plies = st.max_plies.max(plies);
            return true;
        }
        st.bad_strategy += 1;
        return false;
    }
    if plies >= bound {
        st.budget_exits += 1;
        return false;
    }
    if attacker[idx as usize] {
        // Rank-decreasing choice: child attacker-won with rank exactly
        // rank-1 (the most progressive winning move).
        let mut chosen: Option<u32> = None;
        for &c in &children_idx[idx as usize] {
            if val[c as usize] == WIN && dist[c as usize] == rank - 1 {
                chosen = Some(c);
                break;
            }
        }
        let Some(c) = chosen else {
            st.bad_strategy += 1;
            return false;
        };
        if path.contains(&keys[c as usize]) {
            st.repeats += 1;
            return false;
        }
        path.push(keys[c as usize]);
        let ok = replay_r_walk(c, rank - 1, bound, plies + 1, path, val, dist, children_idx, attacker, terminal, keys, st);
        path.pop();
        ok
    } else {
        let mut ok = true;
        for &c in &children_idx[idx as usize] {
            if val[c as usize] != WIN || dist[c as usize] >= rank {
                st.bad_strategy += 1;
                return false;
            }
            if path.contains(&keys[c as usize]) {
                st.repeats += 1;
                return false;
            }
            path.push(keys[c as usize]);
            ok = replay_r_walk(c, dist[c as usize], bound, plies + 1, path, val, dist, children_idx, attacker, terminal, keys, st);
            path.pop();
            if !ok {
                return false;
            }
        }
        st.lines += 1;
        ok
    }
}

// ---------- candidate L: layered certificate fixpoint ----------

/// Board-set cap for context-free WIN facts; larger strategy subtrees fall
/// back to the T1b subset-transfer (path-bound) encoding.
const BOARD_CAP: usize = 4096;
const NW_PER_KEY_CAP: usize = 4;
const NW_GLOBAL_CAP: usize = 4_000_000;

struct WinFact {
    depth: u32,
    chosen: u32, // child key; u32::MAX for defender (all-replies) facts
    boards: Option<Vec<u32>>, // sorted, includes self; None => path-bound
    proof_set: Vec<u32>,      // sorted ancestor keys (path-bound kind only)
}

struct LSearch {
    ctx: Ctx,
    win: HashMap<u32, WinFact>,
    nw: HashMap<u32, Vec<(u32, Vec<u32>)>>,
    nw_total: usize,
    fact_hits: u64,
    nw_hits: u64,
    facts_stored: u64,
    pathbound_facts: u64,
    nw_stores: u64,
}

impl LSearch {
    fn l_dfs(&mut self, key: u32, rem: u32, path: &[u32]) -> u8 {
        self.ctx.nodes += 1;
        if self.ctx.nodes % 1024 == 0 {
            self.ctx.tick();
            if self.ctx.aborted {
                return P_UNKNOWN;
            }
        }
        let b = board_from_key(key);
        if let Some(t) = classify_terminal(&b) {
            return if t == T_WIN0 { P_WIN } else { P_NOT_WIN };
        }
        let mut path_sorted: Vec<u32> = path.to_vec();
        path_sorted.sort_unstable();
        path_sorted.dedup();
        // WIN fact probe (context-free board-set or path-bound).
        if let Some(f) = self.win.get(&key) {
            if f.depth <= rem {
                let ok = match &f.boards {
                    Some(bs) => path_sorted
                        .iter()
                        .all(|k| bs.binary_search(k).is_err()),
                    None => is_subset(&path_sorted, &f.proof_set),
                };
                if ok {
                    self.fact_hits += 1;
                    return P_WIN;
                }
            }
        }
        // NOT_WIN probe (T1b subset-transfer lemma: valid under ancestor
        // supersets and depths <= the proven depth).
        if let Some(list) = self.nw.get(&key) {
            for (d, set) in list {
                if *d >= rem && is_subset(set, &path_sorted) {
                    self.nw_hits += 1;
                    return P_NOT_WIN;
                }
            }
        }
        if rem == 0 {
            return P_UNKNOWN;
        }
        let attacker = b.side_to_move() == strong_color(&b);
        let mut moves = MoveList::new();
        generate_legal(&b, &mut moves);
        // Child generation + static ordering. Repetition children resolve
        // free (rank 0). Attacker: queen then king approach to the weak
        // king. Defender: maximize the weak king's distance from the
        // attacker queen. Squares come from the packed key (strong-relative
        // bit layout).
        let mut kids: Vec<(u32, u8, u32, u32)> = Vec::with_capacity(moves.len()); // (key, is_rep, qd, kd)
        {
            let mut si = StateInfo::new();
            for i in 0..moves.len() {
                let (c, ck) = child_board(&b, moves[i], &mut self.ctx);
                drop(c);
                let is_rep = u8::from(path.contains(&ck) || ck == key);
                let q = ck & 63;
                let wk = (ck >> 6) & 63;
                let sk = (ck >> 12) & 63;
                kids.push((ck, is_rep, cheb(q, wk), cheb(sk, wk)));
            }
        }
        if attacker {
            kids.sort_by_key(|&(_, rep, qd, kd)| (rep, qd, kd));
        } else {
            kids.sort_by_key(|&(_, rep, qd, _)| (rep, std::cmp::Reverse(qd)));
        }
        let mut child_path: Vec<u32> = path.to_vec();
        child_path.push(key);
        let mut result = if attacker { P_NOT_WIN } else { P_WIN };
        let mut any_unknown = false;
        let mut win_child: u32 = u32::MAX;
        for &(ck, _, _, _) in &kids {
            let val = if child_path.contains(&ck) {
                self.ctx.cut_by_rep += 1;
                P_NOT_WIN
            } else {
                self.l_dfs(ck, rem - 1, &child_path)
            };
            if self.ctx.aborted {
                return P_UNKNOWN;
            }
            if attacker {
                if val == P_WIN {
                    result = P_WIN;
                    win_child = ck;
                    break;
                }
                any_unknown |= val == P_UNKNOWN;
            } else {
                if val == P_NOT_WIN {
                    result = P_NOT_WIN;
                    break;
                }
                any_unknown |= val == P_UNKNOWN;
            }
        }
        // Optimistic sides cannot be claimed over unknown children.
        if any_unknown && result == if attacker { P_NOT_WIN } else { P_WIN } {
            return P_UNKNOWN;
        }
        if result == P_WIN {
            self.store_win_fact(key, rem, win_child, attacker, &path_sorted);
        } else if result == P_NOT_WIN && !any_unknown {
            self.store_nw_fact(key, rem, path_sorted);
        }
        result
    }

    fn store_win_fact(
        &mut self,
        key: u32,
        rem: u32,
        chosen: u32,
        attacker: bool,
        path_sorted: &[u32],
    ) {
        // Compose the strategy board set from the covering child fact(s).
        let child_boards: Option<Vec<u32>> = if attacker {
            self.win.get(&chosen).and_then(|f| f.boards.clone())
        } else {
            // Defender fact: union over ALL legal children's facts.
            let b = board_from_key(key);
            let mut moves = MoveList::new();
            generate_legal(&b, &mut moves);
            let mut si = StateInfo::new();
            let mut acc: Option<Vec<u32>> = Some(Vec::new());
            for i in 0..moves.len() {
                let mut c = b.clone();
                c.do_move(moves[i], &mut si);
                let ck = key_of(&c);
                let cb = self.win.get(&ck).and_then(|f| f.boards.clone());
                match (acc.take(), cb) {
                    (Some(mut a), Some(cb2)) => {
                        a.extend(cb2);
                        acc = Some(a);
                    }
                    _ => {
                        acc = None;
                        break;
                    }
                }
            }
            acc
        };
        let fact = match child_boards {
            Some(cb) if cb.len() + 1 <= BOARD_CAP => {
                let mut boards = cb;
                boards.push(key);
                boards.sort_unstable();
                boards.dedup();
                if boards.len() <= BOARD_CAP {
                    WinFact {
                        depth: rem,
                        chosen: if attacker { chosen } else { u32::MAX },
                        boards: Some(boards),
                        proof_set: Vec::new(),
                    }
                } else {
                    self.pathbound_facts += 1;
                    WinFact {
                        depth: rem,
                        chosen: if attacker { chosen } else { u32::MAX },
                        boards: None,
                        proof_set: path_sorted.to_vec(),
                    }
                }
            }
            _ => {
                self.pathbound_facts += 1;
                WinFact {
                    depth: rem,
                    chosen: if attacker { chosen } else { u32::MAX },
                    boards: None,
                    proof_set: path_sorted.to_vec(),
                }
            }
        };
        self.win.insert(key, fact);
        self.facts_stored += 1;
    }

    fn store_nw_fact(&mut self, key: u32, rem: u32, path_sorted: Vec<u32>) {
        if self.nw_total >= NW_GLOBAL_CAP {
            return;
        }
        let list = self.nw.entry(key).or_default();
        list.push((rem, path_sorted));
        self.nw_total += 1;
        self.nw_stores += 1;
        if list.len() > NW_PER_KEY_CAP {
            // Keep the largest depths (they transfer to the most remaining
            // budgets); drop the smallest-depth entry.
            list.sort_by_key(|&(d, _)| std::cmp::Reverse(d));
            list.pop();
            self.nw_total -= 1;
        }
    }

    /// Extract the strategy from the fact set (attacker nodes: chosen move;
    /// defender nodes: all replies covered by their own facts).
    fn extract(&self, key: u32, strat: &mut HashMap<u32, u32>) -> bool {
        let Some(f) = self.win.get(&key) else {
            return false;
        };
        let b = board_from_key(key);
        if classify_terminal(&b).is_some() {
            return true;
        }
        let attacker = b.side_to_move() == strong_color(&b);
        if attacker {
            if f.chosen == u32::MAX {
                return false;
            }
            strat.insert(key, f.chosen);
            self.extract(f.chosen, strat)
        } else {
            let mut moves = MoveList::new();
            generate_legal(&b, &mut moves);
            let mut si = StateInfo::new();
            for i in 0..moves.len() {
                let mut c = b.clone();
                c.do_move(moves[i], &mut si);
                if !self.extract(key_of(&c), strat) {
                    return false;
                }
            }
            true
        }
    }
}

struct LHorizonResult {
    result: &'static str,
    evals: u64,
    wall: f64,
    cert: Option<CertStats>,
    cert_wall: f64,
    fact_depth: u32,
    reason: &'static str,
}

/// L — layered certificate fixpoint (plan13 T1-L): layers 1..=hmax with a
/// persistent fact set; the root result is snapshotted at each requested
/// horizon (cumulative evals through that layer).
fn run_l(fen: &str, horizons: &[u32], ctx_caps: (u64, f64)) -> Vec<(u32, LHorizonResult)> {
    let hmax = horizons.iter().copied().max().unwrap();
    let root = Board::from_fen(fen).unwrap();
    let root_key = key_of(&root);
    let t0 = std::time::Instant::now();
    let mut ls = LSearch {
        ctx: Ctx::new(ctx_caps.0, ctx_caps.1),
        win: HashMap::new(),
        nw: HashMap::new(),
        nw_total: 0,
        fact_hits: 0,
        nw_hits: 0,
        facts_stored: 0,
        pathbound_facts: 0,
        nw_stores: 0,
    };
    let mut recorded: HashMap<u32, LHorizonResult> = HashMap::new();
    for d in 1..=hmax {
        let evals_before = ls.ctx.evals;
        let wall_before = t0.elapsed().as_secs_f64();
        let root_val = ls.l_dfs(root_key, d, &[]);
        let layer_evals = ls.ctx.evals - evals_before;
        let wall_now = t0.elapsed().as_secs_f64();
        println!(
            "plan13_t1 layer: d={d} root_val={root_val} layer_evals={layer_evals} total_evals={} facts={} nw={} fact_hits={} nw_hits={} pathbound={} wall={wall_now:.2}s",
            ls.ctx.evals, ls.win.len(), ls.nw.len(), ls.fact_hits, ls.nw_hits, ls.pathbound_facts,
        );
        if ls.ctx.aborted {
            for &h in horizons {
                recorded.entry(h).or_insert(LHorizonResult {
                    result: "UNKNOWN",
                    evals: ls.ctx.evals,
                    wall: wall_now,
                    cert: None,
                    cert_wall: 0.0,
                    fact_depth: 0,
                    reason: ls.ctx.abort_reason,
                });
            }
            break;
        }
        if root_val == P_WIN {
            let fact_depth = ls.win.get(&root_key).map(|f| f.depth).unwrap_or(0);
            let mut strat: HashMap<u32, u32> = HashMap::new();
            let tc0 = std::time::Instant::now();
            let extracted = ls.extract(root_key, &mut strat);
            let (ok, cert) = if extracted {
                replay_cert(&root, &strat, hmax)
            } else {
                (
                    false,
                    CertStats {
                        lines: 0,
                        mates: 0,
                        repeats: 0,
                        budget_exits: 0,
                        bad_strategy: 0,
                        nodes: 0,
                        max_plies: 0,
                    },
                )
            };
            let cert_wall = tc0.elapsed().as_secs_f64();
            let res = if ok { "WIN_CERTIFIED" } else { "CLAIM_UNCERTIFIED" };
            for &h in horizons {
                if h >= d {
                    recorded.insert(
                        h,
                        LHorizonResult {
                            result: res,
                            evals: ls.ctx.evals,
                            wall: t0.elapsed().as_secs_f64(),
                            cert: Some(cert.clone()),
                            cert_wall,
                            fact_depth,
                            reason: if ok { "" } else { "extract-or-replay-fail" },
                        },
                    );
                }
            }
            break;
        }
        if root_val == P_NOT_WIN {
            recorded.insert(
                d,
                LHorizonResult {
                    result: "NOT_WIN",
                    evals: ls.ctx.evals,
                    wall: wall_now,
                    cert: None,
                    cert_wall: 0.0,
                    fact_depth: 0,
                    reason: "",
                },
            );
        }
        for &h in horizons {
            if h == d {
                recorded.entry(h).or_insert(LHorizonResult {
                    result: "UNKNOWN",
                    evals: ls.ctx.evals,
                    wall: wall_now,
                    cert: None,
                    cert_wall: 0.0,
                    fact_depth: 0,
                    reason: "layer-unknown",
                });
            }
        }
    }
    let mut out: Vec<(u32, LHorizonResult)> = Vec::new();
    for &h in horizons {
        let r = recorded.remove(&h).unwrap_or(LHorizonResult {
            result: "UNKNOWN",
            evals: ls.ctx.evals,
            wall: t0.elapsed().as_secs_f64(),
            cert: None,
            cert_wall: 0.0,
            fact_depth: 0,
            reason: "layer-not-reached",
        });
        out.push((h, r));
    }
    out
}


// ---------- candidate P: PN bounded prover ----------

const NO_NODE: u32 = u32::MAX;
const INF: u64 = u64::MAX / 4;

struct PChild {
    key: u32,
    node: u32,
    pn: u64,
    dn: u64,
}

struct PNode {
    key: u32,
    depth: u32,
    parent: u32,
    is_or: bool,
    expanded: bool,
    dead: bool,
    proven: u8,
    chosen: u32, // winning child key for WIN-proven nodes (strategy extraction)
    pn: u64,
    dn: u64,
    children: Vec<PChild>,
}

struct PSearch {
    nodes: Vec<PNode>,
    memo_win: HashMap<(u32, u32), (Vec<u32>, u32)>, // (key, depth) -> (proof set, chosen child)
    memo_nw: HashMap<(u32, u32), Vec<u32>>,
    ctx: Ctx,
    win_hits: u64,
    nw_hits: u64,
    memo_stores: u64,
    dead_count: u64,
}

impl PSearch {
    fn ancestors(&self, mut idx: u32) -> Vec<u32> {
        let mut v = Vec::new();
        while idx != NO_NODE {
            v.push(self.nodes[idx as usize].key);
            idx = self.nodes[idx as usize].parent;
        }
        v
    }

    /// Evaluate/expand a leaf node.
    fn expand(&mut self, idx: u32) {
        let (key, depth) = {
            let n = &self.nodes[idx as usize];
            (n.key, n.depth)
        };
        let b = board_from_key(key);
        if let Some(t) = classify_terminal(&b) {
            let n = &mut self.nodes[idx as usize];
            n.expanded = true;
            if t == T_WIN0 {
                n.proven = P_WIN;
                n.pn = 0;
                n.dn = INF;
            } else {
                n.proven = P_NOT_WIN;
                n.pn = INF;
                n.dn = 0;
            }
            return;
        }
        let mut path_sorted = self.ancestors(idx);
        path_sorted.sort_unstable();
        path_sorted.dedup();
        // Subset-transfer memo probes (T1b lemmas, restated):
        // WIN proven under set A is valid under any A' ⊆ A (fewer forbidden
        // repeats); NOT_WIN proven under A is valid under any A' ⊇ A (the
        // defender keeps every escape).
        if let Some((set, chosen)) = self.memo_win.get(&(key, depth)) {
            if is_subset(&path_sorted, set) {
                let n = &mut self.nodes[idx as usize];
                n.expanded = true;
                n.proven = P_WIN;
                n.chosen = *chosen;
                n.pn = 0;
                n.dn = INF;
                self.win_hits += 1;
                return;
            }
        }
        if let Some(set) = self.memo_nw.get(&(key, depth))
            && is_subset(set, &path_sorted)
        {
            let n = &mut self.nodes[idx as usize];
            n.expanded = true;
            n.proven = P_NOT_WIN;
            n.pn = INF;
            n.dn = 0;
            self.nw_hits += 1;
            return;
        }
        if depth == 0 {
            let n = &mut self.nodes[idx as usize];
            n.expanded = true;
            n.dead = true; // horizon leaf: unknown, never expandable
            return;
        }
        let mut moves = MoveList::new();
        generate_legal(&b, &mut moves);
        let mut si = StateInfo::new();
        let mut children: Vec<PChild> = Vec::with_capacity(moves.len());
        for i in 0..moves.len() {
            let (c, ck) = child_board(&b, moves[i], &mut self.ctx);
            let (pn, dn) = if path_sorted.contains(&ck) {
                self.ctx.cut_by_rep += 1;
                (INF, 0) // repetition cut: NOT_WIN for the attacker
            } else {
                match classify_terminal(&c) {
                    Some(T_WIN0) => (0, INF),
                    Some(_) => (INF, 0),
                    None => {
                        let cdepth = depth - 1;
                        if cdepth == 0 {
                            (1, 1)
                        } else {
                            let mut cpath = path_sorted.clone();
                            cpath.push(ck);
                            cpath.sort_unstable();
                            cpath.dedup();
                            if let Some((set, _chosen)) = self.memo_win.get(&(ck, cdepth))
                                && is_subset(&cpath, set)
                            {
                                self.win_hits += 1;
                                (0, INF)
                            } else if let Some(set) = self.memo_nw.get(&(ck, cdepth))
                                && is_subset(set, &cpath)
                            {
                                self.nw_hits += 1;
                                (INF, 0)
                            } else {
                                (1, 1)
                            }
                        }
                    }
                }
            };
            children.push(PChild {
                key: ck,
                node: NO_NODE,
                pn,
                dn,
            });
            if self.ctx.aborted {
                break;
            }
        }
        let is_or = self.nodes[idx as usize].is_or;
        let (pn, dn) = aggregate(&children, is_or);
        let n = &mut self.nodes[idx as usize];
        n.expanded = true;
        n.children = children;
        n.pn = pn;
        n.dn = dn;
        if n.pn == 0 {
            n.proven = P_WIN;
            let chosen = n.children.iter().find(|c| c.pn == 0).map(|c| c.key).unwrap_or(u32::MAX);
            n.chosen = chosen;
            self.memo_win.insert((key, depth), (path_sorted, chosen));
            self.memo_stores += 1;
        } else if n.dn == 0 {
            n.proven = P_NOT_WIN;
            self.memo_nw.insert((key, depth), path_sorted);
            self.memo_stores += 1;
        }
    }

    /// Recompute numbers up to the root; record proven transitions. The
    /// child entry push happens for every visited node (including nodes
    /// already proven at expansion time), otherwise the parent keeps stale
    /// (1,1) numbers and PN selection spins on a proven child forever.
    fn backup(&mut self, mut idx: u32) {
        loop {
            {
                let n = &self.nodes[idx as usize];
                if n.proven == P_UNKNOWN && n.expanded && !n.children.is_empty() {
                    let (pn, dn) = aggregate(&n.children, n.is_or);
                    let (key, depth, is_win, chosen) = {
                        let n = &self.nodes[idx as usize];
                        (
                            n.key,
                            n.depth,
                            pn == 0,
                            n.children.iter().find(|c| c.pn == 0).map(|c| c.key).unwrap_or(u32::MAX),
                        )
                    };
                    let mut path_sorted = self.ancestors(idx);
                    path_sorted.sort_unstable();
                    path_sorted.dedup();
                    let n = &mut self.nodes[idx as usize];
                    n.pn = pn;
                    n.dn = dn;
                    if is_win {
                        n.proven = P_WIN;
                        n.chosen = chosen;
                        self.memo_win.insert((key, depth), (path_sorted, chosen));
                        self.memo_stores += 1;
                    } else if dn == 0 {
                        n.proven = P_NOT_WIN;
                        self.memo_nw.insert((key, depth), path_sorted);
                        self.memo_stores += 1;
                    }
                }
            }
            // Push the (possibly changed) numbers into the parent's child entry.
            let parent = self.nodes[idx as usize].parent;
            if parent == NO_NODE {
                break;
            }
            let (pn, dn) = (self.nodes[idx as usize].pn, self.nodes[idx as usize].dn);
            if let Some(pn_node) = self.nodes[parent as usize]
                .children
                .iter_mut()
                .find(|c| c.node == idx)
            {
                pn_node.pn = pn;
                pn_node.dn = dn;
            }
            idx = parent;
        }
    }
}

fn sel_key(is_or: bool, ch: &PChild) -> (u64, u64) {
    if is_or {
        (ch.pn, ch.dn)
    } else {
        (ch.dn, ch.pn)
    }
}

fn aggregate(children: &[PChild], is_or: bool) -> (u64, u64) {
    if is_or {
        let pn = children.iter().map(|c| c.pn).min().unwrap_or(INF);
        let dn = children.iter().map(|c| c.dn).fold(0u64, |a, b| a.saturating_add(b).min(INF));
        (pn, dn)
    } else {
        let pn = children.iter().map(|c| c.pn).fold(0u64, |a, b| a.saturating_add(b).min(INF));
        let dn = children.iter().map(|c| c.dn).min().unwrap_or(INF);
        (pn, dn)
    }
}

struct POutcome {
    horizon: u32,
    result: &'static str,
    evals: u64,
    nodes: u64,
    wall: f64,
    cert: Option<CertStats>,
    cert_wall: f64,
    win_hits: u64,
    nw_hits: u64,
    memo_stores: u64,
    cut_by_rep: u64,
    reason: &'static str,
}

/// P — PN bounded prover (plan13 T1-P) at one fixed horizon. Standard PN
/// tree search: descend the most-proving line (OR: min pn, AND: min dn),
/// expand the leaf (evaluating all children), back the numbers up. Dead
/// nodes (horizon leaves, exhausted subtrees) are skipped by selection.
fn run_p(fen: &str, horizon: u32, eval_cap: u64, wall_cap: f64, node_cap: usize) -> POutcome {
    let t0 = std::time::Instant::now();
    let root = Board::from_fen(fen).unwrap();
    let root_key = key_of(&root);
    let mut ps = PSearch {
        nodes: Vec::new(),
        memo_win: HashMap::new(),
        memo_nw: HashMap::new(),
        ctx: Ctx::new(eval_cap, wall_cap),
        win_hits: 0,
        nw_hits: 0,
        memo_stores: 0,
        dead_count: 0,
    };
    // Root node.
    ps.nodes.push(PNode {
        key: root_key,
        depth: horizon,
        parent: NO_NODE,
        is_or: root.side_to_move() == strong_color(&root),
        expanded: false,
        dead: false,
        proven: P_UNKNOWN,
        chosen: u32::MAX,
        pn: 1,
        dn: 1,
        children: Vec::new(),
    });
    let result = 'search: loop {
        if ps.nodes[0].proven != P_UNKNOWN {
            break 'search ps.nodes[0].proven;
        }
        ps.ctx.tick();
        if ps.ctx.aborted {
            break 'search P_UNKNOWN;
        }
        if ps.nodes.len() > node_cap {
            ps.ctx.abort("node-cap");
            break 'search P_UNKNOWN;
        }
        if ps.nodes[0].dead {
            break 'search P_UNKNOWN;
        }
        // Stall guard: every iteration must expand a node or dead-mark one.
        let progress_key = (ps.nodes.len(), ps.dead_count);
        // Descend to an unexpanded leaf (or a dead end).
        let mut cur = 0usize;
        let mut action = DescendAction::Expand;
        'descend: loop {
            let n = &ps.nodes[cur];
            if n.proven != P_UNKNOWN {
                // Proven on a previous pass; numbers already backed up.
                action = DescendAction::Continue;
                break 'descend;
            }
            if !n.expanded {
                action = DescendAction::Expand;
                break 'descend;
            }
            // Select the most-proving child among live, unproven children.
            let is_or = n.is_or;
            let mut best: Option<usize> = None;
            for (i, ch) in n.children.iter().enumerate() {
                if ch.node != NO_NODE && ps.nodes[ch.node as usize].dead {
                    continue;
                }
                if ch.pn == 0 || ch.dn == 0 {
                    continue; // proven children never improve selection
                }
                let better = match best {
                    None => true,
                    Some(b) => sel_key(is_or, ch) < sel_key(is_or, &n.children[b]),
                };
                if better {
                    best = Some(i);
                }
            }
            match best {
                Some(i) => {
                    let (ck, cdepth, node) = {
                        let ch = &ps.nodes[cur].children[i];
                        (ch.key, ps.nodes[cur].depth - 1, ch.node)
                    };
                    cur = if node == NO_NODE {
                        let ci = ps.nodes.len() as u32;
                        let b = board_from_key(ck);
                        ps.nodes.push(PNode {
                            key: ck,
                            depth: cdepth,
                            parent: cur as u32,
                            is_or: b.side_to_move() == strong_color(&b),
                            expanded: false,
                            dead: false,
                            proven: P_UNKNOWN,
                            chosen: u32::MAX,
                            pn: 1,
                            dn: 1,
                            children: Vec::new(),
                        });
                        ps.nodes[cur].children[i].node = ci;
                        ci as usize
                    } else {
                        node as usize
                    };
                }
                None => {
                    // All children dead/hopeless: this subtree is a dead end.
                    ps.nodes[cur].dead = true;
                    ps.dead_count += 1;
                    ps.backup(cur as u32);
                    action = DescendAction::Continue;
                    break 'descend;
                }
            }
        }
        match action {
            DescendAction::Continue => {
                // No progress this pass: proven node reached mid-descent.
                // This can only be a transient inconsistency; guard against
                // a spin by comparing against the pre-pass progress key.
                let now = (ps.nodes.len(), ps.dead_count);
                if now == progress_key {
                    ps.ctx.abort("pn-stall");
                    break 'search P_UNKNOWN;
                }
                continue 'search;
            }
            DescendAction::Expand => {
                ps.expand(cur as u32);
                if ps.ctx.aborted {
                    break 'search P_UNKNOWN;
                }
                ps.backup(cur as u32);
            }
        }
    };
    let wall = t0.elapsed().as_secs_f64();
    let (result, reason) = match result {
        P_WIN => ("WIN_PENDING_CERT", ""),
        P_NOT_WIN => ("NOT_WIN", ""),
        _ => ("UNKNOWN", ps.ctx.abort_reason),
    };
    let mut outcome = POutcome {
        horizon,
        result,
        evals: ps.ctx.evals,
        nodes: ps.nodes.len() as u64,
        wall,
        cert: None,
        cert_wall: 0.0,
        win_hits: ps.win_hits,
        nw_hits: ps.nw_hits,
        memo_stores: ps.memo_stores,
        cut_by_rep: ps.ctx.cut_by_rep,
        reason,
    };
    if result == "WIN_PENDING_CERT" {
        // Strategy extraction: in-tree first, memo chains for proven leaves;
        // the replay verifier is the arbiter.
        let mut strat: HashMap<u32, u32> = HashMap::new();
        let tc0 = std::time::Instant::now();
        let extracted = p_extract(&ps, 0, &mut strat);
        let (ok, cert) = if extracted {
            replay_cert(&root, &strat, horizon)
        } else {
            (false, CertStats {
                lines: 0,
                mates: 0,
                repeats: 0,
                budget_exits: 0,
                bad_strategy: 0,
                nodes: 0,
                max_plies: 0,
            })
        };
        outcome.cert_wall = tc0.elapsed().as_secs_f64();
        outcome.result = if ok {
            "WIN_CERTIFIED"
        } else {
            "CLAIM_UNCERTIFIED"
        };
        outcome.reason = if ok { "" } else { "extract-or-replay-fail" };
        outcome.cert = Some(cert);
    }
    outcome
}

enum DescendAction {
    Continue,
    Expand,
}

/// Strategy extraction for P: at attacker nodes pick the proven child (in
/// tree) or follow the memo chain (proven leaves); at defender nodes cover
/// every reply. Replay is the arbiter — any gap fails the claim.
fn p_extract(ps: &PSearch, idx: u32, strat: &mut HashMap<u32, u32>) -> bool {
    let n = &ps.nodes[idx as usize];
    let b = board_from_key(n.key);
    if classify_terminal(&b).is_some() {
        return true;
    }
    let dbg = std::env::var("PLAN13_DEBUG").is_ok();
    if n.is_or {
        let chosen = if !n.children.is_empty() {
            n.children.iter().find(|c| c.pn == 0).map(|c| c.key)
        } else {
            ps.memo_win
                .get(&(n.key, n.depth))
                .map(|&(_, chosen)| chosen)
        };
        let Some(ck) = chosen else {
            return false;
        };
        if dbg {
            eprintln!("extract OR key={:#x} d={} chosen={ck:#x}", n.key, n.depth);
        }
        strat.insert(n.key, ck);
        // Continue: in-tree child if present, else memo chain.
        if let Some(c) = n.children.iter().find(|c| c.key == ck && c.node != NO_NODE) {
            p_extract(ps, c.node, strat)
        } else {
            let cdepth = n.depth - 1;
            p_extract_memo(ps, ck, cdepth, strat)
        }
    } else {
        for ch in &n.children {
            if ch.pn != 0 {
                return false;
            }
            let ok = if ch.node != NO_NODE {
                p_extract(ps, ch.node, strat)
            } else {
                p_extract_memo(ps, ch.key, n.depth - 1, strat)
            };
            if !ok {
                return false;
            }
        }
        true
    }
}

fn p_extract_memo(ps: &PSearch, key: u32, depth: u32, strat: &mut HashMap<u32, u32>) -> bool {
    let b = board_from_key(key);
    if classify_terminal(&b).is_some() {
        return true;
    }
    if depth == 0 {
        return false;
    }
    let attacker = b.side_to_move() == strong_color(&b);
    if attacker {
        let Some(chosen) = ps.memo_win.get(&(key, depth)).map(|&(_, c)| c) else {
            return false;
        };
        strat.insert(key, chosen);
        p_extract_memo(ps, chosen, depth - 1, strat)
    } else {
        // Defender node: every legal reply must be covered by its own
        // proven fact (memo chains were created for every pn==0 child at
        // the defender node's proof).
        let mut moves = MoveList::new();
        generate_legal(&b, &mut moves);
        let mut si = StateInfo::new();
        for i in 0..moves.len() {
            let mut c = b.clone();
            c.do_move(moves[i], &mut si);
            if !p_extract_memo(ps, key_of(&c), depth - 1, strat) {
                return false;
            }
        }
        true
    }
}

// ---------- oracle sampling (plan13 G-B) ----------

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

fn oracle(
    n_target: usize,
    qtable: &[u8],
    shard: usize,
    shards: usize,
    region_budget: usize,
    oracle_h: u32,
    eval_cap: u64,
    wall_cap: f64,
) {
    // Sample legal attacker-to-move KQvK placements; classify via the table
    // (validation oracle only). Side-not-to-move must not be in check.
    let mut rng = Rng(0x9E3779B97F4A7C15 ^ (shard as u64 + 1));
    let mut seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut wins: Vec<(u32, Board)> = Vec::new();
    let mut draws: Vec<(u32, Board)> = Vec::new();
    let mut attempts: u64 = 0;
    let half = n_target / 2;
    while (wins.len() < half || draws.len() < half) && attempts < 20_000_000 {
        attempts += 1;
        let strong = rng.below(2) as usize;
        let sk = rng.below(64) as usize;
        let wk = rng.below(64) as usize;
        let xs = rng.below(64) as usize;
        if sk == wk || sk == xs || wk == xs {
            continue;
        }
        let strong_white = strong == 0;
        let mut grid = [b'.'; 64];
        grid[sk] = if strong_white { b'K' } else { b'k' };
        grid[wk] = if strong_white { b'k' } else { b'K' };
        grid[xs] = if strong_white { b'Q' } else { b'q' };
        let fen = grid_fen(&grid, strong == 0); // attacker (strong) to move
        let b = Board::from_fen(&fen).unwrap();
        // Legality: side NOT to move (defender) must not be in check.
        let twin_fen = grid_fen(&grid, strong == 1);
        let twin = Board::from_fen(&twin_fen).unwrap();
        if !twin.checkers().is_empty() {
            continue;
        }
        let key = key_of(&b);
        if !seen.insert(key) {
            continue;
        }
        let t = qtable[key as usize];
        if t == 2 && wins.len() < half {
            wins.push((key, b));
        } else if t == 1 && draws.len() < half {
            draws.push((key, b));
        }
    }
    println!(
        "plan13_t1 oracle: shard={shard}/{shards} samples win={} draw={} attempts={attempts}",
        wins.len(),
        draws.len(),
    );
    let mut contradictions: u64 = 0;
    let mut downgraded: u64 = 0;
    let mut win_cert = 0u64;
    let mut draw_claims = 0u64;
    let mut unknowns = 0u64;
    for (i, (key, b)) in wins.iter().chain(draws.iter()).enumerate() {
        let is_win = i < wins.len();
        let fen = b.fen();
        // R with the region budget.
        let mut ctx = Ctx::new(u64::MAX, f64::MAX);
        let r = run_r(&fen, &[oracle_h], region_budget, &mut ctx, Some(qtable));
        let r0 = &r[0];
        let mut dtm_known: Option<u32> = None;
        if r0.result == "WIN_CERTIFIED" {
            win_cert += 1;
            dtm_known = Some(r0.dtm);
            if !is_win {
                contradictions += 1;
                println!("plan13_t1 oracle CONTRADICTION: R WIN on draw sample {fen}");
            }
        } else if r0.result == "DRAW" {
            draw_claims += 1;
            if is_win {
                contradictions += 1;
                println!("plan13_t1 oracle CONTRADICTION: R DRAW on win sample {fen}");
            }
        } else {
            unknowns += 1;
            if r0.table_mismatches > 0 {
                contradictions += r0.table_mismatches as u64;
                println!(
                    "plan13_t1 oracle CONTRADICTION: R region/table mismatches={} at {fen}",
                    r0.table_mismatches
                );
            }
        }
        // L and P: horizon = oracle_h, or the exact dtm when R derived one
        // <= oracle_h (bounds the NOT_WIN soundness check: a NOT_WIN claim at
        // h >= dtm would be a contradiction).
        let (lh, lh_is_dtm) = match dtm_known {
            Some(d) if d <= oracle_h => (d, true),
            _ => (oracle_h, false),
        };
        for (cand, res) in [
            ("L", {
                let out = run_l(&fen, &[lh], (eval_cap, wall_cap));
                out[0].1.result.to_string()
            }),
            ("P", {
                let out = run_p(&fen, lh, eval_cap, wall_cap, 1_000_000);
                out.result.to_string()
            }),
        ] {
            println!(
                "plan13_t1 oracle sample: shard={shard} {i} fen=\"{fen}\" kind={} R={} lh={lh} cand={cand} result={res}",
                if is_win { "win" } else { "draw" },
                r0.result,
            );
            if res == "WIN_CERTIFIED" {
                if !is_win {
                    contradictions += 1;
                    println!("plan13_t1 oracle CONTRADICTION: {cand} WIN on draw sample {fen}");
                }
            } else if res == "CLAIM_UNCERTIFIED" {
                // A claim whose certificate failed replay is downgraded to
                // UNKNOWN (safe), but tracked: extraction defects matter for
                // certificate yield.
                downgraded += 1;
                println!("plan13_t1 oracle DOWNGRADED: {cand} uncertified claim at {fen}");
            } else if res == "NOT_WIN" && is_win && lh_is_dtm {
                contradictions += 1;
                println!(
                    "plan13_t1 oracle CONTRADICTION: {cand} NOT_WIN at h=dtm on win sample {fen}"
                );
            } else {
                unknowns += 1;
            }
        }
        if (i + 1) % 20 == 0 {
            println!(
                "plan13_t1 oracle progress: shard={shard} {}/{} contradictions={contradictions}",
                i + 1,
                wins.len() + draws.len()
            );
        }
    }
    println!(
        "plan13_t1 oracle summary: shard={shard}/{shards} samples={} contradictions={contradictions} downgraded={downgraded} R_win_cert={win_cert} R_draw_claims={draw_claims} unknown_or_bounded={unknowns}",
        wins.len() + draws.len(),
    );
}

fn grid_fen(grid: &[u8; 64], white_to_move: bool) -> String {
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
    s.push(if white_to_move { 'w' } else { 'b' });
    s.push_str(" - - 0 1");
    s
}

fn load_table(path: &str) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    assert_eq!(bytes.len(), 1 << 20, "unexpected table size");
    bytes
}

// ---------- main ----------

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut oracle_n: Option<usize> = None;
    let mut qtable_path: Option<String> = None;
    let mut shard = 0usize;
    let mut shards = 1usize;
    let mut cand = String::from("all");
    let mut horizons: Vec<u32> = Vec::new();
    let mut eval_cap: u64 = 400_000_000;
    let mut wall_cap: f64 = 180.0;
    let mut region_budget: usize = 500_000;
    let mut node_cap: usize = 3_000_000;
    let mut oracle_h: u32 = 6;
    let mut oracle_eval_cap: u64 = 3_000_000;
    let mut oracle_wall_cap: f64 = 8.0;
    let mut i = 0usize;
    let mut fen: Option<String> = None;
    while i < args.len() {
        match args[i].as_str() {
            "--oracle" => {
                oracle_n = Some(args[i + 1].parse().unwrap());
                i += 2;
            }
            "--qtable" => {
                qtable_path = Some(args[i + 1].clone());
                i += 2;
            }
            "--shard" => {
                shard = args[i + 1].parse().unwrap();
                i += 2;
            }
            "--shards" => {
                shards = args[i + 1].parse().unwrap();
                i += 2;
            }
            "--cand" => {
                cand = args[i + 1].clone();
                i += 2;
            }
            "--horizons" => {
                horizons = args[i + 1]
                    .split(',')
                    .map(|s| s.parse().unwrap())
                    .collect();
                i += 2;
            }
            "--eval-cap" => {
                eval_cap = args[i + 1].parse().unwrap();
                i += 2;
            }
            "--wall-cap" => {
                wall_cap = args[i + 1].parse().unwrap();
                i += 2;
            }
            "--region-budget" => {
                region_budget = args[i + 1].parse().unwrap();
                i += 2;
            }
            "--node-cap" => {
                node_cap = args[i + 1].parse().unwrap();
                i += 2;
            }
            "--oracle-h" => {
                oracle_h = args[i + 1].parse().unwrap();
                i += 2;
            }
            "--oracle-eval-cap" => {
                oracle_eval_cap = args[i + 1].parse().unwrap();
                i += 2;
            }
            "--oracle-wall-cap" => {
                oracle_wall_cap = args[i + 1].parse().unwrap();
                i += 2;
            }
            other => {
                fen = Some(other.to_string());
                i += 1;
                args.drain(..i);
                i = 0;
            }
        }
    }
    let qtable = qtable_path.as_deref().map(load_table);
    if let Some(n) = oracle_n {
        let table = qtable.expect("--qtable required for --oracle");
        oracle(
            n,
            &table,
            shard,
            shards,
            region_budget,
            oracle_h,
            oracle_eval_cap,
            oracle_wall_cap,
        );
        return;
    }
    let fen = fen.expect("a FEN argument is required");
    if horizons.is_empty() {
        horizons = vec![14, 15, 17];
    }
    println!("plan13_t1: fen=\"{fen}\" cand={cand} horizons={horizons:?} eval_cap={eval_cap} wall_cap={wall_cap}");
    if cand.contains('R') || cand == "all" {
        let t0 = std::time::Instant::now();
        let mut ctx = Ctx::new(eval_cap, wall_cap);
        let out = run_r(&fen, &horizons, region_budget, &mut ctx, qtable.as_deref());
        for o in &out {
            println!(
                "plan13_t1 cand=R fen=\"{fen}\" horizon={} result={} evals={} scans={} region={} dtm={} wall={:.2}s cert_lines={} cert_mates={} cert_repeats={} cert_exits={} cert_maxplies={} cert_wall={:.2}s table_mismatches={} reason={}",
                o.horizon, o.result, o.evals, o.scans, o.region, o.dtm, o.wall,
                o.cert.as_ref().map(|c| c.lines).unwrap_or(0),
                o.cert.as_ref().map(|c| c.mates).unwrap_or(0),
                o.cert.as_ref().map(|c| c.repeats).unwrap_or(0),
                o.cert.as_ref().map(|c| c.budget_exits).unwrap_or(0),
                o.cert.as_ref().map(|c| c.max_plies).unwrap_or(0),
                o.cert_wall, o.table_mismatches, o.reason,
            );
        }
        println!("plan13_t1 cand=R total wall={:.2}s", t0.elapsed().as_secs_f64());
    }
    if cand.contains('L') || cand == "all" {
        let t0 = std::time::Instant::now();
        let out = run_l(&fen, &horizons, (eval_cap, wall_cap));
        for (h, r) in &out {
            println!(
                "plan13_t1 cand=L fen=\"{fen}\" horizon={} result={} evals={} wall={:.2}s fact_depth={} cert_lines={} cert_mates={} cert_maxplies={} cert_wall={:.2}s reason={}",
                h, r.result, r.evals, r.wall, r.fact_depth,
                r.cert.as_ref().map(|c| c.lines).unwrap_or(0),
                r.cert.as_ref().map(|c| c.mates).unwrap_or(0),
                r.cert.as_ref().map(|c| c.max_plies).unwrap_or(0),
                r.cert_wall, r.reason,
            );
        }
        println!("plan13_t1 cand=L total wall={:.2}s", t0.elapsed().as_secs_f64());
    }
    if cand.contains('P') || cand == "all" {
        let t0 = std::time::Instant::now();
        for &h in &horizons {
            let o = run_p(&fen, h, eval_cap, wall_cap, node_cap);
            println!(
                "plan13_t1 cand=P fen=\"{fen}\" horizon={} result={} evals={} nodes={} wall={:.2}s win_hits={} nw_hits={} memo_stores={} cut_by_rep={} cert_lines={} cert_mates={} cert_repeats={} cert_exits={} cert_bad={} cert_maxplies={} cert_wall={:.2}s reason={}",
                o.horizon, o.result, o.evals, o.nodes, o.wall, o.win_hits, o.nw_hits,
                o.memo_stores, o.cut_by_rep,
                o.cert.as_ref().map(|c| c.lines).unwrap_or(0),
                o.cert.as_ref().map(|c| c.mates).unwrap_or(0),
                o.cert.as_ref().map(|c| c.repeats).unwrap_or(0),
                o.cert.as_ref().map(|c| c.budget_exits).unwrap_or(0),
                o.cert.as_ref().map(|c| c.bad_strategy).unwrap_or(0),
                o.cert.as_ref().map(|c| c.max_plies).unwrap_or(0),
                o.cert_wall, o.reason,
            );
        }
        println!("plan13_t1 cand=P total wall={:.2}s", t0.elapsed().as_secs_f64());
    }
}
