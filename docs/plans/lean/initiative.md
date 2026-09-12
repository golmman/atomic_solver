# Initiative: `lean` — reducing wall time and nodes searched

## Status

Active, maintained **agile**: the backlog is a living document re-ranked
after every profile, plans are single-lever and sized to one session, and
nothing larger than one lever is planned before a fresh profile justifies
it. Plans 1–6 are done (`report1.md`–`report6.md`); the post-plan6 profile
(2026-09-12) is the current baseline for ranking.

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

### Post-plan6 profile (m22_white, first-outcome, default 128 MB TT, 2026-09-12)

After plan6 (micro-wins bundle: #14 wrapper slimming, #13 clock sampling,
#12a TT non-terminal skip) the existence-check cluster collapsed and the
pie re-ranked (leaf attribution, raw table under `measurements/plan6/`;
post-plan4 shares in parentheses; wall −7.1% m22, −5.9% shuffle-win):

| cluster | leaves | share post-plan6 (post-plan4) |
| --- | --- | --- |
| child-eval loop | `evaluate_child` (incl. inlined scans) | 26.9% (26.6%) |
| move make/unmake | `do_move` 29.4% + `undo_move` 2.5% | 31.9% (26.9%) |
| frame overhead | `dfpn` | 15.5% (9.9%) |
| static scoring | `score_with_context` | 5.5% (4.3%) |
| existence check | `populate_state` 2.1% + `has_legal_move_with_state` 5.4% | 7.5% (20.7%) |
| full movegen | `Board::legal` 3.1% + `generate_legal_with_state` 0.8% | 3.9% (4.0%) |
| clock sampling | `vdso` (`Instant::now`) | <0.1% top-leaf (~2.2%) |
| move sort | `sort_moves` sort leaves | ~2.6% (~2.2%) |

Notes (numbers only, no promotions without spikes):

- **#12a over-delivered relative to its ~2.5% ceiling**: `populate_state`
  dropped 17.8% → 2.1% because the skip removes the 64-byte zeroing *and*
  the populate/existence work on the ~15% of checks that hit `None`
  entries — the cluster fell from 20.7% to 7.5% of a smaller pie.
- **#13 removed clock sampling from the leaf table** (<0.1%; was ~2.2%).
- **#14's win shows as a smaller absolute `do_move`/`undo_move`** (the
  wrapper fat is gone; what remains is the raw upstream core, 29.4% of a
  −7% smaller pie).
- Make/unmake is now the top cluster again; it is the measured-tight
  upstream core (~7.6 ns/round-trip) — #14's solver-side fat cannot be
  re-cut. Further moves there need an upstream round (out of scope) or
  fewer calls (algorithmic, `docs/plans/dfpn/initiative.md`).

### Post-plan4 profile (m22_white, first-outcome, default 128 MB TT, 2026-09-09)

After plan4 (−49% wall, −62% first-outcome work) the pie is smaller again
and the shares re-ranked (`perf record -e cpu-clock -g`, leaf
attribution; raw output under `measurements/plan5/`; post-plan3 shares
in parentheses):

| cluster | leaves | share post-plan4 (post-plan3) |
| --- | --- | --- |
| move make/unmake | `do_move` 25.0% + `undo_move` 1.9% | 26.9% (13.8%) |
| child-eval loop | `evaluate_child` (incl. inlined existence-query fragments) | 26.6% (38.8%) |
| existence check | `populate_state` 17.8% + `has_legal_move_with_state` 2.9% | 20.7% (~16%) |
| frame overhead | `dfpn` | 9.9% (11.7%) |
| static scoring | `score_with_context` | 4.3% (5.4%) |
| full movegen | `Board::legal` 2.2% + `generate_legal_with_state` 1.8% | 4.0% (6.4%) |
| clock sampling | `vdso` (`Instant::now`) | ~2.2% (2.4%) |
| move sort | `sort_moves` sort leaves | ~2.2% (folded into frame overhead) |

Quantified notes:

- **Fixed per-child costs now dominate relatively.** Per child eval,
  make/unmake roughly doubled (~31 ns → ~59 ns) and `populate_state`
  nearly doubled (~18 ns → ~39 ns), while `has_legal_move_with_state`
  stayed flat (~7 ns). This is consistent with cache pressure from the
  8× larger TT (zobrist/board working sets evicted by random TT probes);
  a `--tt-size 64` control run reproduces the post-plan3-like shares
  (make/unmake 18.6%, `populate_state` 10.8%).
- **The existence check no longer dominates make/unmake.** The plan3-era
  claim "the win must come from calling populate less" still holds, but
  the wrapper-fat micro-win on `Position::do_move` (#14) is now measured
  against ~27% of the pie instead of ~14% — its absolute ceiling is
  unchanged (~1–3% of wall), only the pie shrank.
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

### Spike 2026-09-12 — #17 path-membership sizing (plan6 phase 0)

Temporary counters on `Search` (reverted after measuring): on m22_white
first-outcome, `path_contains` is called 15.0M times (14.2M child evals +
0.83M `dfpn` entries) scanning 141.5M path-stack elements total (mean 9.4,
max 107); `best_move_repeats_path` runs 113k do/undo round-trips with 7
hits (0.006%). On the shuffle-win study position: 371.3M calls scanning
3.43G elements (mean 9.3, max 406); 2.04M repeat-guard calls, 40 hits
(0.002%). Calibration micro-benchmark (`slice::contains` on u64, miss
path, matching scan lengths): ~2.2 ns/call at mean length 9 (the
vectorized compare dominates short scans, ~0.24 ns/element) up to
~4.5 ns/call at length 40. Wall estimate: **~1.0% on m22, ≤ ~2.2% on the
shuffle-win case** (all 371M scans at the deep-path per-call cost) —
below the ~3% bar on both validation cases. The repeat-guard hit rates
(~0.01%) make `best_move_repeats_path` negligible. Verdict: **#17
demoted — spiked, below bar**; a hash-set path stack is not justified.

### Position study 2026-09-11 — deep shuffle win (`4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`)

A 60+ ply tempo/conversion win against a shuffling rook defense (sibling of
`m22_white`, pawns still on). First-outcome: **20,313,879 nodes / ~65 s**
(first line 405 plies); default mode **38,694,766 nodes / ~95 s**, PV 405 →
115 `cap-cut`. The decisive point is invariant across ε ∈ {0, 0.5} and TT ∈
{32 MB, 2 GB} — the cost is algorithmic, not tuning; the algorithmic items
spun out to `docs/plans/dfpn/initiative.md` (backlog #1–#4). Profile shares
match the post-plan4 m22 profile closely (`evaluate_child` 48.5% self,
`populate_state` 15.6%, make/unmake ~10.2%, full movegen ~5%), so the pie
ranking below carries over to this class. One
unprofiled cost is new to the backlog (#17): the O(depth) path-membership
scans, which this position exercises harder than any suite case (depth 51+,
41 root moves, cycle-heavy lines).

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
| 12 | Existence-check cost | (a) solver-side: skip `populate_state` + `has_legal_move` when the child TT entry already proves the position non-terminal (`outcome == None` ⇒ it was expanded); (b) upstream: fused populate+existence API | ceiling measured **~2.5% wall** (spike 2026-09-09, see below) | wall | S (a) / M (b) | **done (plan6, phase 3)**: skip implemented behind the outcome-None invariant; existence cluster 20.7% → 7.5% of the profile pie; see `report6.md` |
| 14 | Upstream make/unmake cost | `do_move`+`undo_move` **26.9% post-plan4** (13.8% post-plan3; per-eval ~31→~59 ns, TT cache pressure — see profile above); upstream core ~7.6 ns/round-trip and inherently tight; solver wrapper ~4.3 ns (64 B `StateInfo` zeroing + undo-stack push/pop) | ~1–3% wall (wrapper slimming only; absolute ceiling unchanged) | wall | S | **done (plan6, phase 1)**: `do_move_with_scratch`/`undo_move_with_scratch` over a pooled dirty slot; wrapper round-trip 12.2 → ~7.0 ns in the micro-benchmark; see `report6.md` |
| 17 | Path-membership cost | `path_contains` is an O(depth) linear scan over `path_stack` (`children.rs`, `core.rs`), called per child eval and per `dfpn` entry; `best_move_repeats_path` adds a full do/undo round-trip per TT-resolved hit | **spiked, below bar**: ~1.0% wall on m22, ≤ ~2.2% on the shuffle-win case (see spike 2026-09-12 above); repeat-guard hit rate ~0.01% | wall | S | **demoted (plan6, phase 0 spike)** — a hash-set path stack is not justified at these costs |
| 8 | Cheaper static scoring | `StaticAtomicScorer` does 2–5 sliding-attack scans per quiet move; precomputed/incremental attacks | ~3–5% wall (was 5–15% pre-plan3; `score_with_context` now 5.4%) | wall | M | open |
| 13 | Clock sampling | `time_exceeded()` calls `Instant::now()` at every `dfpn` entry; sample every N nodes (budget mode is eval-count-based and unaffected) | ~2% wall (2.2% post-plan4, unchanged) | wall | S | **done (plan6, phase 2)**: `Instant::now()` sampled every 4096 dfpn entries behind the unchanged `time_exceeded` call sites; clock leaves <0.1% in the post-plan6 profile; see `report6.md` |
| 15 | `has_legal_move` playout cross-check | Random-playout property test: `Position::has_legal_move` vs `legal_moves_with_state` + `outcome_from_state` (report3 "missing tests") | correctness hardening, no speed | correctness | S | **done (plan5)**: `tests/test_playout_crosscheck.rs`, P1–P4 green in both tiers; see `report5.md` |
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
- The deep shuffle win studied on 2026-09-11
  (`4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`, baseline in
  the Motivation section above) is the secondary hard case for micro-wins
  validation (#13, #12a, #14, #17): m22 no longer stresses the
  per-child fixed costs at depth. Algorithmic levers for it live in
  `docs/plans/dfpn/initiative.md`, not here.
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
- **plan5** — #15 `has_legal_move` playout cross-check: test-only
  property test (`tests/test_playout_crosscheck.rs`), no `src/` change;
  plus the post-plan4 profile re-rank above (done, `report5.md`).
- **From here on:** agile cadence per the working agreement above; the
  next plan is chosen from the backlog table, not from a fixed roadmap.
  Leading candidates: #2 parallelism design spike (the only remaining
  multiplicative lever) or the micro-wins bundle (#13, #12a, #14 — #14
  now the top candidate after the post-plan4 re-rank).
- **plan6** — micro-wins bundle: #14 scratch-slot wrapper slimming
  (anchor; wrapper round-trip 12.2 → ~7.0 ns), #13 clock sampling (4096-entry
  sampler), #12a TT non-terminal skip, plus the #17 sizing spike as
  phase 0 (verdict: below bar, demoted). Drift protocol fully green
  (quick suite bit-identical, m22/shuffle stdout byte-identical, lean3
  golden byte-identical); wall −7.1% m22, −5.9% shuffle-win first-outcome
  (done, `report6.md`).

Per repo convention, every plan ends with the task of writing its
`report<N>.md` in this directory.
