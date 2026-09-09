# Lean Report 5 — #15 `has_legal_move` playout cross-check (+ post-plan4 profile re-rank)

Implements `docs/plans/lean/plan5.md` (item #15 of the lean initiative).
Test-only plan: **no `src/` change**, so the bit-identical drift protocol
does not apply. The companion task (post-plan4 profile re-rank) is
measurement only and claims no backlog lever beyond what the plan
prescribed.

## Summary of changes

- New `tests/test_playout_crosscheck.rs` (name deliberately *not*
  `test_plan5.rs`, which is taken):
  - Inline splitmix64 PRNG (~20 lines, no `rand` dev-dependency);
    FNV-1a-style per-game seed derivation from the seed FEN + playout
    index, so every game is a deterministic stream.
  - Seeds: `Position::STARTPOS_FEN`, every FEN of
    `common::load_move_order_suite()`, `load_decisive_suite()`,
    `load_smoke_suite()` (81 seeds total), plus the two plan-specified
    endgame seeds (stalemate `7k/8/…/2q5/K7 w`, bare-kings
    `4k3/…/4K3 w`).
  - Playout driver: per seed, N random games, up to 120/200 plies; per
    ply one `StateInfo` populated once and reused for both the existence
    query and `generate_legal_with_state` (hot-path pattern); PRNG move
    picked from the legal list; `Position::do_move`; fresh
    `Position::from_fen` per game (no undo replay; the move list is the
    legality filter since `try_do_move` is `#[cfg(test)]`).
  - Assertions P1–P4 as specified; every message carries the seed-set
    label, seed FEN, playout index, ply, and live position FEN.
  - `common::assert_position_invariants` runs per ply (incremental
    Zobrist locked along playout lines) — kept, it did not double the
    wall (see timings).
- `docs/plans/lean/initiative.md`: #15 flipped to done; "Post-plan3
  profile" replaced by "Post-plan4 profile"; #14/#13 rows re-ranked;
  status line and history updated.
- Raw profile outputs: `docs/plans/lean/measurements/plan5/`
  (`m22_first_outcome_tt128_leaf.txt`, `m22_first_outcome_tt64_leaf.txt`).

## What the test locks (P1–P4)

- **P1** — `pos.has_legal_move(&state)` == `!moves.is_empty()` from
  `generate_legal_with_state`, both off one populated `StateInfo`.
- **P2** — the outcome reconstructed purely from fast-path signals
  (commoner-extinction bitboards → checkers bit → `rule50 >= 100` →
  `occupied == 2`) equals `pos.outcome_from_state(&state, &moves)`; plus
  a sharpening assert that a terminal-with-legal-moves can only fire a
  move-list-independent branch (extinction / rule50 / two-piece).
- **P3** — `pos.outcome()` (full movegen internally) equals the same
  classification.
- **P4** — aggregate counters must show ≥ 1 checkmate-terminal,
  ≥ 1 stalemate-draw-terminal, ≥ 1 extinction-terminal position.

## Tier sizing and timings

| tier | config | positions checked | wall (release) | wall (debug) |
| --- | --- | --- | --- | --- |
| fast (`playout_crosscheck_fast_tier`) | 2 playouts/seed, 120-ply cap | 12,787 | **0.01 s** | **0.09 s** |
| slow (`playout_crosscheck_slow_tier`, `#[ignore]`) | 32 playouts/seed, 200-ply cap | 274,989 | 0.12 s | — |

Both tiers are deterministic: release and debug runs produce identical
position/branch counts (PRNG is seeded from FEN + playout index only).

Branch-coverage counts (P4, well above the ≥ 1 bar):

| tier | checkmate | stalemate | extinction |
| --- | --- | --- | --- |
| fast | 10 | 8 | 59 |
| slow | 230 | 233 | 1,067 |

`make test` with the new test: 26/26 binaries green, sum of test runtime
**37.7 s** (the new test contributes 0.01 s; gate target < ~60 s holds).
`cargo fmt --check` clean; `cargo clippy --release --all-targets` clean.

The plan estimated the slow tier at "minutes"; it runs in 0.12 s because
random atomic playouts terminate quickly (extinction/checkmate median
around 30–40 plies), so the ply caps are rarely reached. The tier sizing
could be raised in a later plan without gate impact.

## Teeth check (not committed; `src/` reverted after each)

1. `Position::has_legal_move` inverted (`!has_legal_move_with_state`):
   fast tier fails immediately at the very first position,
   ```
   assertion `left == right` failed: P1 existence mismatch (seed set
   'fast', seed 'rnbqkbnr/… w KQkq - 0 1', playout 0, ply 0):
   has_legal_move=false, 20 legal moves
   ```
   → reverts cleanly, P1 has teeth.
2. Checkers branch dropped in `outcome_from_state` (no-moves ⇒ always
   `Draw`):
   ```
   assertion `left == right` failed: P2 classification mismatch (seed
   set 'fast', seed 'r5r1/5N1k/2p2p2/pp1p3p/3Pp3/2P1P3/P7/2bQ1R1K w - -
   0 30', playout 1, ply 15): fast-path=Some(Loss),
   outcome_from_state=Some(Draw)
   ```
   → P2/P3 have teeth. A misclassified checkmate-draw is caught on live
   playout positions, exactly the gap report3 flagged.

## Companion task: post-plan4 profile re-rank

`perf record -e cpu-clock -g` on m22_white first-outcome `--outcome-only`
(default 128 MB TT), leaf attribution. Run solves in ~2.55 s wall
(matches `report4.md`'s 2.67–2.78 s); 12,362 samples. Raw tables under
`measurements/plan5/`.

| cluster | leaves | post-plan4 | post-plan3 |
| --- | --- | --- | --- |
| move make/unmake | `do_move` 25.0% + `undo_move` 1.9% | **26.9%** | 13.8% |
| child-eval loop | `evaluate_child` | 26.6% | 38.8% |
| existence check | `populate_state` 17.8% + `has_legal_move_with_state` 2.9% | **20.7%** | ~16% |
| frame overhead | `dfpn` | 9.9% | 11.7% |
| static scoring | `score_with_context` | 4.3% | 5.4% |
| full movegen | `Board::legal` 2.2% + `generate_legal_with_state` 1.8% | 4.0% | 6.4% |
| clock sampling | `vdso` (+`clock_gettime` 0.05%) | ~2.2% | 2.4% |
| move sort | `sort_moves` sort leaves | ~2.2% | (folded) |

Interpretation (flagged as hypothesis): per child eval, make/unmake
roughly doubled (~31 ns → ~59 ns) and `populate_state` nearly doubled
(~18 ns → ~39 ns) while `has_legal_move_with_state` stayed flat (~7 ns).
Consistent with cache pressure from the 8× larger TT. A `--tt-size 64`
control run (`…tt64_leaf.txt`) reproduces the post-plan3-like shares
(make/unmake 18.6%, `populate_state` 10.8%, `undo_move` 8.5%). Per-eval
nanoseconds are derived from the profiled run's CPU time (perf overhead
included) — use for share comparison, not absolute costing.

### Re-rank verdicts (numbers only)

- **#14 make/unmake**: 13.8% → 26.9% relative; per-eval ~31 → ~59 ns;
  absolute ~1.15 s → ~0.83 s. Wrapper-fat ceiling unchanged in absolute
  terms (~4.3 ns × 14.2 M evals ≈ 0.06 s ≈ **2.4%** of the 2.55 s wall),
  but it is now measured against ~27% of the pie instead of ~14%.
  Verdict: stays a micro-wins item; **ranks first among micro-wins**.
  No promotion (needs no spike — it already had one — but no plan
  claimed it here).
- **#12a TT non-terminal skip**: existence cluster 16% → 20.7%;
  ceiling 0.155 × 20.7% ≈ **3.2%** wall (skippable rate from the
  512 MB spike, *not* re-measured at 128 MB). Verdict: stays demoted;
  micro-wins bundle at best.
- **#13 clock sampling**: 2.4% → ~2.2%. Verdict: unchanged; stays the
  agile warm-up candidate.

No item promoted without its own spike, per the plan.

## Deviations from the plan

1. The two plan-specified endgame seeds sufficed for P4: checkmate and
   extinction coverage both arose naturally from playouts (59 extinction
   terminals in the fast tier alone), so no extra forced-coverage FEN
   was added.
2. `common::assert_position_invariants` was kept (plan made it optional):
   it locks the incremental Zobrist along playout lines and did not
   double the fast-tier wall.
3. Beyond the plan's single profile run, a 64 MB control profile was
   taken to interpret the share shifts (see above). Measurement only.

## Problems encountered

- `benchmark --suite move-order --first-outcome --timeout 30` exceeds a
  5-minute shell timeout (five > 30 s cases dominate); the direct
  `atomic_solver` invocation on m22_white was used for wall confirmation
  instead. Not needed for the deliverable.
- A self-inflicted brace imbalance while applying a clippy collapse
  (`if let` chain) — caught by the compiler, fixed immediately.

## Missing tests

- The P4 guard does not require a `rule50 >= 100` terminal; that branch
  is only covered by `position.rs` unit tests, not by playout coverage.
  A rule50-heavy seed would close it.
- The extinction branch's *precedence* over the moves-empty branch
  (extinction while boxed-in) is exercised by playouts (counted), but no
  dedicated minimal fixture pins it in this test.
- Teeth checks covered P1 and P2/P3; the test's own fast-path
  reconstruction (`classify_from_fast_signals`) vs `outcome_from_state`
  divergence is what caught the checkers mutation — the extinction
  signal itself (commoner bitboards) is not teeth-checked here.

## Additional tools/examples used

- `perf record/report` (per AGENTS.md profiling section), `python3` for
  timing deltas and report arithmetic, direct `atomic_solver` runs for
  wall confirmation.

## Next steps

- **plan6 candidates** (per the refreshed backlog): #2 parallelism
  design spike (multiplicative), or the micro-wins bundle starting with
  #14 wrapper slimming (now the top-ranked micro-win).
- Cheap follow-up to this plan: raise the slow tier's playout count and
  add a rule50-heavy seed to the P4 guard.
