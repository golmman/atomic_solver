# Initiative: `lean` — reducing wall time and nodes searched

## Status

Active, maintained **agile**: the backlog is a living document re-ranked
after every profile, plans are single-lever and sized to one session, and
nothing larger than one lever is planned before a fresh profile justifies
it. Plans 1–3 are done (`report1.md`–`report3.md`); the post-plan3 profile
(2026-09-08) is the current baseline for ranking.

## Motivation

The `nn` branch (archived, `docs/plans/nn/report8.md` on that branch)
settled the move-ordering question: the headroom is slim. The oracle-floor
measurement put the best possible per-node ordering improvement at
**0.509x evals work-weighted / 1.029x unweighted**, and the decomposition
showed **90.6% of OR-node work is already spent on the decisive child**
(recoverable ≤ 9.4% locally). The learned-ordering PoC was closed
permanently. Anything that only ranks the winning OR child earlier — NN
round 2, win-length-aware rankers — cannot beat that ceiling and is out
of scope.

### Post-plan3 profile (m22_white, first-outcome, 2026-09-08)

After plan3 (−46% wall), the leaf-attributed pie is much smaller and
re-ranked:

| cluster | leaves | share |
| --- | --- | --- |
| child-eval loop | `evaluate_child` (incl. inlined existence-query fragments) | 38.8% |
| existence check | `populate_state` 12.0% + `has_legal_move_with_state` 4.1% | ~16% |
| move make/unmake | `do_move` 11.2% + `undo_move` 2.6% | 13.8% |
| frame overhead | `dfpn` | 11.7% |
| static scoring | `score_with_context` | 5.4% |
| full movegen | `Board::legal` 4.0% + `generate_legal_with_state` 2.4% | 6.4% |
| clock sampling | `vdso` (`Instant::now`) | 2.4% |

Quantified notes that override older backlog guesses:

- **The existence check dominates movegen, and its cost is the per-child
  `populate_state` prefix.** `evaluate_child` populates a `StateInfo` for
  every non-fast-path child (~19 per searched node); the child frame's own
  populate at its `dfpn` entry is only ~1/20 of populate calls. The
  "hand the populated StateInfo to the recursion" idea from report3
  therefore removes only ~0.6% wall — the real levers are making the
  existence query need less than a full populate, or skipping the populate
  entirely for children already known non-terminal from the TT.
- **`populate_state` computes `checkers` + `pinned` + two popcounts.**
  The existence query needs `checkers` and `pinned` for legality, so the
  computation is mostly irreducible — the win must come from *calling it
  less*, not from a slimmer populate.
- Ordering output and search decisions must remain **bit-identical**
  unless a plan explicitly says otherwise and validates the change against
  the move-order benchmark suite.

### Spike 2026-09-09 — #12(a) TT non-terminal memo ceiling

Passive counters on m22_white first-outcome (temporary instrumentation,
reverted after measuring): of 37.2M existence checks, 5.9M (15.9%) hit a
TT entry with `outcome == None`. Re-run at `--tt-size 512`: total work
nearly halved (19.2M checks — TT hits change the trajectory) but the
skippable rate stayed flat at 15.5%, so 84% of existence checks are
genuinely first-visits, not TT churn. Ceiling: ~0.155 × 16% ≈ **2.5%
wall** — below the bar for a dedicated plan given the invariant it would
introduce ("outcome-None TT entry ⇒ position had legal moves", which
holds today: terminal stores always carry `Some`, leaf bounds store
`None` only after the terminal check passed). #12 is demoted; a fused
upstream API (#12b) remains bounded by the same 16% and is also
demoted.

### Spike 2026-09-09 — #14 make/unmake sizing

Fresh m22_white first-outcome profile (`perf record --call-graph dwarf`):
`Position::do_move` is its own leaf at 12.0%, `undo_move` 2.4%. A
micro-benchmark over m22's root position (5M round-trips, release build):

- raw `Board::do_move` + `undo_move`: **7.6 ns/round-trip** — incremental
  XOR hash, minimal bitboard churn; the blast loop and castling-rights
  cascade only run on captures. Nothing removable without heroics.
- `Position::do_move` + `undo_move`: **11.9 ns** — the wrapper adds
  ~4.3 ns: a fresh 64-byte `StateInfo::new()` zeroing (only the undo
  fields are ever read back; `checkers`/`pinned` are pure waste here) plus
  the 64-byte undo-stack push/pop copy.

Paid once per evaluated child (~37M on m22), the wrapper fat is a
**~3% wall ceiling**; killing the push/pop copy alone is ~1%. An upstream
make/unmake round is **not justified**. Note `StateInfo` is only 64 B
(packed `captured: [(Square, Piece); 9]`), not the ~200 B assumed earlier.
Killing the zeroing without `unsafe` needs an upstream constructor
(e.g. a documented `undo_scratch()` that leaves stale bytes, sound because
`do_move` writes every field `undo_move` reads).

### Research round 2026-09-09 — TT capacity (`research_tt_capacity.md`)

The #12 spike's 512 MB observation was sized properly: at the 64 MB
default the m22 first-outcome search fills the table (~1M live entries
for 2.4M nodes) and does **2.6× the work and 2.0× the wall** of a 128 MB
table; the quick suite is insensitive (±2%); above 128 MB the regime is
chaotic and larger tables lose wall to cache misses. Entry slimming is
blocked by `INF = 1 << 60` (u64 pn/dn) and cannot close a 2× capacity
gap anyway. New backlog item **#16** (default bump 64 → 128 MB), ranked
first: S effort, behavior-changing, validated on the move-order suite.

## Goal

Reduce solver wall time and nodes/child-evals on the benchmark suites
without changing search semantics: identical outcomes, and
(default-mode) identical proven lines.

Priorities follow AGENTS.md: correctness first, then performance,
memory, maintainability.

## Working agreement (agile cadence)

1. **Profile before planning.** Every plan starts from the freshest
   profile; potentials in the backlog are estimates to be re-derived, not
   commitments.
2. **One lever per plan.** A plan that needs a second lever gets split;
   large items (parallelism) get a design spike first, then staged
   implementation plans.
3. **Drift protocol is the gate.** `benchmark --suite quick --json
   --first-outcome` bit-identical per case; m22 first-outcome stdout
   byte-identical; m22 default-mode chunk trajectory bit-identical when
   the lever touches anything the refinement path reads.
4. **Re-rank after every plan.** The post-plan report updates the
   backlog's potentials and this profile section.

## Backlog (re-ranked post-plan3; confidence-weighted)

| # | Item | Mechanism | Potential | Affects | Effort | Status |
|---|------|-----------|-----------|---------|--------|--------|
| 16 | TT capacity default | Default `--tt-size` 64 MB is capacity-starved on hard searches (m22 first-outcome: 2.6× work, 2.0× wall at 128 MB; quick suite insensitive ±2%). See `research_tt_capacity.md` | ~2× wall on m22-class hard cases at default settings | wall + behavior (drift protocol N/A; move-order suite is the validator) | S + re-baseline | **done (plan4)**: default now 128 MB; m22 first-outcome −49% wall, default-mode −74%; see `report4.md` |
| 2 | Parallel search | Lazy-SMP-style parallel sibling children or parallel refinement roots over the shared TT | 2–8× wall on multicore | wall | L–XL | open — the only other multiplicative lever; needs a determinism design spike for the `child_eval_budget` contract first |
| 12 | Existence-check cost | (a) solver-side: skip `populate_state` + `has_legal_move` when the child TT entry already proves the position non-terminal (`outcome == None` ⇒ it was expanded); (b) upstream: fused populate+existence API | ceiling measured **~2.5% wall** (spike 2026-09-09, see below) | wall | S (a) / M (b) | **spiked, demoted** — only worth folding into a micro-wins bundle |
| 14 | Upstream make/unmake cost | `do_move`+`undo_move` 13.8% — **spiked 2026-09-09 (below)**: upstream core ~7.6 ns/round-trip and inherently tight; solver wrapper ~4.3 ns (64 B `StateInfo` zeroing + undo-stack push/pop), ~3% wall ceiling | ~1–3% wall (wrapper slimming only) | wall | S | **spiked, re-scoped** — no upstream round; wrapper slimming is a micro-wins candidate |
| 8 | Cheaper static scoring | `StaticAtomicScorer` does 2–5 sliding-attack scans per quiet move; precomputed/incremental attacks | ~3–5% wall (was 5–15% pre-plan3; `score_with_context` now 5.4%) | wall | M | open |
| 13 | Clock sampling | `time_exceeded()` calls `Instant::now()` at every `dfpn` entry; sample every N nodes (budget mode is eval-count-based and unaffected) | ~2% wall | wall | S | open — good agile warm-up |
| 15 | `has_legal_move` playout cross-check | Random-playout property test: `Position::has_legal_move` vs `legal_moves_with_state` + `outcome_from_state` (report3 "missing tests") | correctness hardening, no speed | correctness | S | open |
| 5 | AND-side ordering signal (non-NN) | Counter-moves, AND-specific history, TT `work` feedback — disproving work concentrates in 1–2 replies per AND node (median max child-share 52.9%) | ~5–20% evals, regression risk (oracle hurt m24_white 2.1×) | nodes | M | open |
| 7 | Lazy/staged child evaluation | Min-heap: evaluate children in rank order as needed instead of all on first iteration | ~3–10% evals | nodes | M | open |
| 9 | 2–3-man atomic endgame tablebases | Leaf probes in shallow-material positions | huge where covered, negligible elsewhere | nodes | M–L | open |
| 10 | History/killer constant re-tuning | Never re-tuned after the GHI/twin removal; side-aware killers | ~0–5% evals | nodes | S–M | open |

Done: #1, #1a, #4 (**plan1**); #3, #6 (**plan2**); #11 (**plan3**, 46%
wall on m22 first-outcome). The plan3-era upstream idea of *handing the
populated StateInfo to the recursion* is demoted: it saves ~1/20 of
populate calls (~0.6% wall) and is folded into #12(b) only if a fused API
falls out for free. The plan2-era per-child slot pre-fill stays retired
(plan3 made the existence query strictly cheaper than generation for the
~95% of children never searched).

Statuses reference plans under `docs/plans/lean/`; an item is *open*
until a plan claims it.

## Non-goals

- Reopening OR-side move-ordering quality in any form (NN or
  hand-written). The `nn` measurement bounds it; see Motivation.
- Changing the proof-tree layer, the `ProofEvent` protocol, or the
  optimizer interface contract (`docs/spec/optimizer_interface.md`)
  beyond what a plan explicitly specifies.
- Nondeterministic search for the sequential path: the deterministic
  `child_evals` budget (`Search::set_child_eval_budget`) and its
  `ExitReason::BudgetExhausted` semantics must keep working exactly as
  documented.

## Measurement conventions

- `child_evals` is the preferred efficiency metric (per the optimizer
  spec); wall time is secondary and machine-dependent.
- Every performance change must be validated with a **bit-identical
  drift check**: `benchmark --suite quick --json --first-outcome`
  before vs after must produce identical `child_evals` per case, unless
  the plan intentionally changes search behavior (e.g. the refinement
  cap; in that case compare with the cap disabled).
- Live measurements for reports use `m22_white`
  (`tests/fixtures/move_order_positions.txt`) as the dominant case;
  raw outputs go under `docs/plans/lean/measurements/`.
- Profiling follows the "Profiling in this container" section of
  `AGENTS.md` (`perf record -e cpu-clock -g`, leaf attribution).

## History

- **plan1** — accounting fix + quick wins #1 and #4 (done, `report1.md`).
- **plan2** — hot-path compute: #3 (+ #6, #8 share one profiling pass)
  (done, `report2.md`).
- **plan3** — upstream `has_legal_move` fast path (#11): two phases,
  movegen crate 2.2.0 then solver integration; −46% wall on m22
  first-outcome (done, `report3.md`).
- **plan4** — TT capacity default (#16): `--tt-size` 64 → 128 MB;
  behavior-changing, validated on the move-order suite; m22
  first-outcome −49% wall, default-mode −74% (done, `report4.md`).
- **From here on:** agile cadence per the working agreement above; the
  next plan is chosen from the backlog table, not from a fixed roadmap.
  Leading candidates: #2 parallelism design spike (the only remaining
  multiplicative lever) or the micro-wins bundle (#13, #12a, #14, #15).

Per repo convention, every plan ends with the task of writing its
`report<N>.md` in this directory.
