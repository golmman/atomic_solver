# Lean Plan 8 — Report: #8 cheaper static scoring (bit-identical)

Implements backlog lever **#8** exactly as scoped by `plan8.md`: a
compute-cost-only change to `StaticAtomicScorer` with **bit-identical
scores** before vs after. Solver-side only; `atomic-movegen` untouched.
The companion tasks (initiative update, post-plan8 profile) are done; no
AGENTS.md change (no user-visible surface moved).

## Result summary

- Wall (interleaved A/B, median of 5 rounds, aarch64 container host):
  **−3.0% m22** first-outcome (3.72 s → 3.61 s), **−2.2% shuffle-win**
  first-outcome (68.59 s → 67.05 s); corroborated across 59 quick-suite
  positions (52 faster / 4 slower / 3 flat, suite total **−3.0%**) and m22
  default mode (**−3.1%**).
- Drift protocol fully green: quick suite 59/59 cases bit-identical
  (`child_evals`, `nodes`, `outcome`), m22 and shuffle-win first-outcome
  stdout byte-identical, lean3 default-mode golden byte-identical,
  move-order suite green, `make test` green.
- New permanent witness: `tests/test_lean8.rs` — map property test plus a
  full differential scorer test against a verbatim pre-plan8 reference.

## Phase 0 — attribution spike (measured, then reverted)

Temporary `AtomicU64` counters in `sort_moves`, `score_with_context`,
`capture_net_value`, and `nearest_commoner_map`, dumped via
`ATOMIC_SOLVER_SPIKE=1` (raw outputs: `measurements/plan8/spike_counters_*.err`;
perf cross-checks: `m22_first_outcome_instrumented_leaves.txt`,
`m22_first_outcome_pre_plan8_leaves.txt`). The instrumentation was reverted
completely; the shipped diff contains none of it (verified by grep).

### Counter totals

| counter | m22_white (14.27M moves scored, ~3.6 s) | shuffle-win (244.01M moves scored, ~62.5 s) |
| --- | --- | --- |
| `sort_moves` invocations | 857,953 | 13,899,877 |
| threat block: `attacks_from`, sliders | 6,685,083 | 114,882,190 |
| threat block: `attacks_from`, static movers | 6,273,300 | 111,859,519 |
| threat fires → `attackers_to` | 164,621 (1.15%) | 2,161,063 (0.89%) |
| rook block entries | 5,641,637 | 97,412,396 |
| rook first `rook_attacks` (alignment scan) | 5,641,637, hit 99,634 (1.8%) | 97,412,396, hit 994,019 (1.0%) |
| rook second semi-open-file scan | 3,236,670, hit 272,069 (8.4%) | 60,076,703, hit 3,037,843 (5.1%) |
| `capture_net_value` calls / blast squares | 1,281,248 / 6,766,163 (mean 5.28) | 16,971,230 / 88,807,527 (mean 5.23) |
| `nearest_commoner_map` calls × k | 857,953 × **k=1** → 54.9M pair ops | 13,899,877 × **k=1** → 889.6M pair ops |

Key facts: enemy-commoner count is **always k=1** on both validation cases
(the enemy king is a commoner); `attackers_to` fires on ~1% of scored moves;
rook-block sliding scans miss 92–99% of the time.

### Verdict

Projected addressable total (slider scans ~1.9%, rook scans ~2.5%, NCM
~2–3%, static-mover rewrite ~0.3%, aSEE ~0.2–0.9%) was **~5–7% wall on both
cases** — above the ~2% demotion bar, so #8 proceeded to implementation. The
per-fragment projections overestimated the in-situ effect (work overlaps
with memory stalls), which the post-implementation wall measurement below
corrects: the realized total is −2–3%.

## What was implemented vs skipped

| sub-lever | decision | evidence |
| --- | --- | --- |
| `nearest_commoner_map` replacement (phase 1) | **implemented as const Chebyshev table**, not BFS | see below |
| threat block restructure: static movers (pawn/knight/commoner) attack from static tables without the occupancy rewrite | implemented | sub-lever plan8 §2.2 |
| threat block, sliders: exact queen-ray pre-filter | implemented (spike-discovered addition) | a slider's attack set ⊆ queen rays through `to`; if no enemy commoner is aligned, the scan is provably skippable |
| rook block: exact alignment pre-filter (file/rank) before the first `rook_attacks` | implemented (spike-discovered addition) | a rook can attack `commoner_sq` only if aligned; 92–99% miss |
| rook block: exact enemy-back-rank-on-file pre-filter before the semi-open scan | implemented (spike-discovered addition) | `rook_attacks_semi & enemy_back_rank_pieces` cannot hit when that mask is empty |
| bitboard `capture_net_value` | implemented | plan §2.4; popcount×value sums are commutative-exact vs the per-square walk |
| blast-mask hoist (plan §2.1) | **skipped** | blocks 1 and 5 are on mutually exclusive return paths (captures return in block 3); the "hoist" saves one OR per capture, < 0.1% |
| empty-enemy fast path (plan §2.3) | **skipped** | `them_commoners` is never empty in a legal search position (the enemy king is a commoner; counters show k=1 on every node of both validation cases); projection ≈ 0 wall |
| file-size split | implemented | new `src/search/ordering/context.rs` holds `ScoreContext`, `nearest_commoner_map`, the const tables, and the Chebyshev/queen-ray helpers; `ordering.rs` back to 17.7 KB |

The two spike-discovered pre-filters are inside the phase-0 decision rule
("rank the candidate sub-levers by projected wall saving") — they are the
two largest projections (rook scans ~2.5%, slider scans ~1.9%) and are
exactness-proven in code comments.

### Phase 1: why the BFS was rejected (measured)

The plan prescribed a multi-source BFS. It was implemented first and
measured, as the plan requires:

- Micro-benchmark (5M calls, release; m22 root board): brute-force 48.5 ns,
  **BFS 175.9 ns** at k=1 (3.6× slower), const table **0.7 ns**.
- In-situ m22 first-outcome: BFS **+8% wall** (3.86–3.99 s vs 3.5–3.7 s
  baseline) — a regression beyond noise, which the plan's own validation
  rule turns into stop-and-bisect.

Root cause: at k=1 (the only case that occurs on real searches here), BFS
pays 8 neighbor expansions + queue/visited bookkeeping per square, while
brute force pays one cheap ALU chain per pair. The shipped replacement is a
const-evaluated `CHEBYSHEV[a][b]` table (4 KiB `.rodata`): k=1 is a single
64-byte row copy, k≥2 min-combines rows (vectorizable). Output is identical
for every input; the deviation is documented in the code comment and here.

## Test evidence (BFS-equivalence and differential)

`tests/test_lean8.rs` (fast tier), all green:

- `bfs_matches_brute_force_on_random_boards` — 512 seeded random boards
  (k = 0–8 commoners, sparse-to-mid density, both colors scanned) comparing
  the shipped map against the brute-force O(64×k) reference kept in the
  test file.
- `empty_commoners_maps_to_max`, `single_commoner_layers_are_chebyshev`,
  `multi_source_bfs_takes_the_minimum` — the dedicated edge cases from the
  plan.
- `scorer_matches_pre_plan8_reference` — the full pre-plan8 scoring path
  transcribed verbatim (old per-square aSEE loop, old threat block with
  unconditional occupancy rewrite, old unconditional rook scans, old
  per-pair Chebyshev arithmetic) and compared per move against the current
  scorer over 256 random boards plus 12 pinned positions (all m20–m22-class
  FENs from the ordering pins), every legal move, both OR and AND profiles.
  This is the primary bit-identical witness for all phase-2 sub-levers.
- The exact-score pins in `src/search/ordering/tests.rs` pass unchanged
  (24/24).

## Drift evidence

| check | result |
| --- | --- |
| quick suite `--json --first-outcome` (59 cases) | **bit-identical** per case (`child_evals`, `nodes`, `outcome`); raw: `measurements/plan8/quick_{before,after}.json` |
| m22 first-outcome stdout | **byte-identical** md5 `ea72f7ea…` |
| shuffle-win first-outcome stdout | **byte-identical** md5 `ffd3d015…` |
| lean3 default-mode golden (`test_lean3 --include-ignored`) | byte-identical, green |
| move-order suite (`test_move_order`, fast tier) | green |
| `make test` (fast gate) | green |
| `test_decisive_remaining --include-ignored` | fails **identically before and after** — pre-existing, see below |

Scope check: the final diff touches only `src/search/ordering.rs` (+
`ordering/context.rs`), `tests/test_lean8.rs`, and `docs/plans/lean/`;
no phase-0 instrumentation remains.

## Wall measurement

Interleaved A/B (base/new binaries alternating within the same session to
cancel host drift; median of 5 rounds each; aarch64 container):

- m22 first-outcome `--timeout 30 --outcome-only`: base 3.72 s → plan8
  **3.61 s** (**−3.0%**).
- shuffle-win first-outcome `--timeout 100 --outcome-only`: base 68.59 s →
  plan8 **67.05 s** (−2.2%).

Realized savings land inside the plan's −2–4% target on m22 and at its lower
edge on the shuffle-win case. Per-round wins on m22 were consistent
(new ≤ base in 4/5 rounds); the shuffle-win case is noisier (−5.6% to +3.4%
per round).

### Breadth check beyond the two validation cases (follow-up measurement)

Because a single-position delta can be a noisy reading, the change was
additionally measured across the full quick suite (59 positions: dec01–dec46
+ m23–m29, `--first-outcome --runs 3`, two interleaved suite rounds per
binary, per-case median):

- **52 of 59 cases faster, 4 slower, 3 flat** (> ±0.5% threshold);
  suite total 9.2 s → 9.0 s (**−3.0%**; dec family −3.2%, m family −2.7%).
  Sub-100 ms cases swing in relative terms from microsecond noise; the
  aggregate and the heavy cases are the signal. All trajectories remain
  bit-identical.
- m22 **default mode** (first-outcome + PV refinement, which also runs
  `sort_moves`): base median 4.49 s → new 4.35 s (**−3.1%**), default-mode
  output identical modulo timestamps.
- m20 (`hardest`, not in the quick suite) does not solve within 300 s for
  either binary — identical `draw` at timeout, so there is no wall delta to
  read there; it is search-work-bound, which this lever deliberately does
  not touch.

Direction consistency across four independent measurements (m22
first-outcome −3.0%, m22 default −3.1%, shuffle-win −2.2%, 59-case suite
−3.0%) and a 52/4 win/loss split make noise an untenable explanation for
the m22 number.

## Post-plan8 profile (companion task 2)

m22 first-outcome, leaf attribution (`perf record -e cpu-clock
--no-children`), on the aarch64 container host — see the initiative's
"Post-plan8 profile" section for the table. Headline:
`score_with_context` leaf 3.30% → 3.02%; the removed scan work also
disappears from the inlined fragments inside the `dfpn`/`sort_moves` leaves.
**Host caveat:** this host splits `populate_state` fragments into a separate
`compute_checkers` leaf (17.2%), so its shares do not line up 1:1 with the
reference-host post-plan6 table; clusters must be compared, and a
reference-host re-baseline is needed before ranking further upstream work.

Re-rank conclusion: static scoring is **spent** — the lever is closed, the
remaining ~3% is the per-move integer math itself, not attack scans. The
next wall levers are #10 (history/killer re-tune; behavior-changing, needs
the move-order-suite validation and oracle-floor regression check) or the
`dfpn` algorithmic items (`docs/plans/dfpn/initiative.md` #1–#4) — noted
here per the plan, not planned.

## Problems encountered

1. **The plan's phase-1 BFS prescription regressed wall (+8% m22) and was
   replaced by the const Chebyshev table** — measured, documented above.
   Lesson: the O(64×k) scan it replaced was already cheap at the k=1 that
   actually occurs; "O(64) with a small queue" is not automatically cheaper
   than 64 ALU pairs.
2. **Pre-existing test failure found: `rem01` in
   `test_decisive_remaining` (solvable within 200M evals) fails at HEAD
   independent of plan8.** Bisect: passes at `3c2558b` (lean plan6), fails
   from `88ddbcb` (dfpn plan9) onward. dfpn plan9's repetition cache
   intentionally shifts traversal order on repetition-heavy positions
   (documented in its `report9.md`, outcomes unchanged on the quick suite);
   `rem01` is a repetition-heavy 49th-move endgame whose trajectory now
   exceeds the fixture's 200M-eval budget. Plan8's trajectories are
   bit-identical (the test fails with the same assertion, same result,
   before and after). Follow-up needed: re-categorize `rem01` per the test's
   own docstring (raise budget or move to unproven) — a separate,
   behavior-documenting change, deliberately not bundled here.
3. **Host wall variance (±5%)** made small deltas unreadable from single
   runs; all wall claims here rest on interleaved A/B medians. The plan6-era
   5.5% static-scoring share reads as 3.3% leaf on this host for the same
   binary — a reminder that shares are host-specific.

## Missing tests

- `queen_rays`/`CHEBYSHEV` const tables are covered indirectly (differential
  scorer test over random boards with sliders, map property test) but not
  exhaustively per square; an exhaustive 64×64 pin would be trivial if ever
  needed.
- The differential test uses sparse random boards plus pinned sharp
  positions; it does not walk real game trees (the end-to-end stdout/golden
  gates cover that).
- `rem01` re-categorization (see problems) is outstanding.

## Next steps

1. Re-categorize the `rem01` fixture entry (small standalone change; must
   reference the dfpn-plan9 drift report).
2. Next wall lever per the re-ranked backlog: #10 (constant re-tune,
   behavior-changing) — only with the full move-order-suite validation — or
   the `dfpn` algorithmic items, which now own the largest clusters
   (make/unmake, existence check, frame overhead).
3. Optional reference-host re-profile to realign the pie shares before
   planning further micro-work.

Per repo convention this report ends plan 8.
