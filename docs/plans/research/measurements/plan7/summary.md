# plan7 measurement summary — TT eviction/turnover (backlog #12)

Raw per-run outputs: `phase0_<case>_<tt>.{out,err}` (Phase 0 counters;
`_rerun` = 32 MB stress confirmation run), `phase1_<arm>_<case>.{out,err}`
(Phase 1 replacement-policy arms), `quick_{clean,instrumented,postrevert}.json`
(drift-gate proof), `quick_{v1,v2}.json` (Phase 1 outcome-flip check).

## Setup

- Cases: stress (m21_white), m22_white, dec13, dec10 (FENs in plan7.md).
- All runs `--first-outcome --outcome-only`, release build, default ε=0.125,
  default refine-cap; TT 128 MB (default) and 32 MB (pressure point).
- Timeouts: stress 600 s, m22/dec13/dec10 120 s (32 MB stress rerun 600 s;
  post-revert stress reproduction 300 s).
- `child_evals` via env-gated stderr dump (`RESEARCH7_DUMP=1`, plan1/plan3/plan4
  pattern); counters via `ATOMIC_TT_STATS=1`; Phase 1 arms via
  `ATOMIC_TT_REPL=v1|v2`. All instrumentation reverted before plan close
  (`git diff --exit-code` verified).
- Reference host: this container (4 cores).

## Counter design (throwaway)

Counters in `TranspositionTable` (atomic, relaxed; decision-neutral):

- store classes, cross-tabulated by new-entry class (solved/unsolved):
  `update_existing` (key hit in a live slot), `insert_free_slot`,
  `insert_stale_slot` (dead in-run — `new_generation()` is never called by the
  search; all-zero as predicted), `evict_live_unsolved_victim`,
  **`evict_live_solved_victim`** (the harmful class);
- probe classes: `hit`, `miss`, and `miss_evicted` — a miss whose key appears
  in a 2^21-slot direct-mapped shadow history of evicted keys (exact-key
  compare; collisions under-count, never over-count beyond stale remnants;
  measurement-only, not part of the shipped table);
- occupancy: end-of-run full/half/empty buckets, live entries, live solved.

Phase 1 arms (env-gated `match` on `ATOMIC_TT_REPL`, no layout changes):
**V1** solved-slot immunity — a new *unsolved* store never evicts a live
*solved* slot (store dropped; counted as `v1_dropped`); **V2** steeper work
priority — unsolved-vs-unsolved victim scoring gains a `remaining_depth`
tiebreak before `generation` (deep high-work subtrees protected over shallow
churn).

## Verification trail

- Clean-HEAD quick suite (`benchmark --suite quick --json --first-outcome
  --timeout 5`): aggregate `total_child_evals` = 38,974,090, 59/59 solved
  (`quick_clean.json`).
- Same suite with full instrumentation: per-case `status/outcome/nodes/
  child_evals/pv_len` bit-identical (`quick_instrumented.json`).
- Phase 0 stress 128 MB reproduces the post-plan9 baseline exactly:
  249,480,478 child evals; win length 477, `pv_status: first-outcome`.
- Post-revert: quick suite bit-identical again (`quick_postrevert.json`);
  stress run reproduces win 477 with the identical 8-chunk `work_done`
  fingerprint (`/tmp` fingerprint diff, not archived) — trajectory unchanged
  by the reverted instrumentation.
- Hygiene at plan close: `cargo fmt --check` clean, `cargo clippy --release
  --all-targets` clean, `make test` green, `git diff --exit-code` clean.

## Phase 0 run tables (128 MB default)

`ev_uns←X` = live-unsolved victim evicted by new class X; `ev_solved←uns` =
live-**solved** victim evicted by a new *unsolved* store (harmful transition
(a)). LS% = (ev_solved←solved + ev_solved←uns) / stores. miss_ev% = misses
whose key was evicted earlier / all probes.

| case (128 MB) | child_evals | stores | upd_solv | upd_unsolv | free s+u | ev_uns←solv | ev_uns←uns | ev_solv←solv | ev_solv←uns | LS % | probe hit | miss_ev | miss_ev % | full % | live / solved |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| stress | 249,480,478 | 13,899,877 | 46,625 | 2,220,102 | 109,642 + 3,971,471 | 195,287 | 7,336,496 | 607 | 19,647 | **0.146** | 43,458,158 | 3,175,882 | 1.21 | **95.4** | 4,081,113 / 331,906 |
| m22 | 14,156,269 | 857,953 | 5,588 | 135,615 | 31,251 + 672,218 | 584 | 12,664 | 1 | 32 | 0.004 | 2,392,897 | 4,626 | 0.03 | 4.6 | 703,469 / 37,391 |
| dec13 | 3,822,602 | 164,869 | 2,281 | 11,755 | 26,031 + 124,658 | 24 | 113 | 0 | 7 | 0.004 | 327,388 | 60 | 0.002 | 0.25 | 150,689 / 28,329 |
| dec10 | 4,262,128 | 606,762 | 2,793 | 513,675 | 6,750 + 83,517 | 1 | 26 | 0 | 0 | 0.000 | 2,243,746 | 14 | 0.000 | 0.09 | 90,267 / 9,544 |

## Phase 0 run tables (32 MB pressure)

| case (32 MB) | child_evals | Δ vs 128 MB | stores | ev_solv←uns | LS % | miss_ev | miss_ev % | full % | verdict |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| stress | 3,332,774,166 (rerun 3,332,302,190) | **timeout, unsolved at 600 s** | 658,910,292 | 197,473 | 0.031 | 24,565,969 | 0.61 | **100.0** | table saturated; no win at 13× the 128 MB evals |
| m22 | 18,903,940 | +33.5% | 1,112,152 | 860 | 0.080 | 98,812 | 0.49 | 52.4 | slower, still solved |
| dec13 | 4,462,921 | +16.7% | 192,842 | 112 | 0.071 | 547 | 0.012 | 4.5 | slower, still solved |
| dec10 | 12,120,831 | +184.4% | 2,157,230 | 4 | 0.000 | 587 | 0.005 | 1.7 | slower, still solved |

The 32 MB stress catastrophe reproduces deterministically (identical chunk
fingerprint up to the wall-clock cut; the rerun's child_evals differ only by
where the 600 s timeout lands inside the last chunk).

## Phase 0 gate evaluation (pre-registered in plan7.md)

- **Condition A** (live-solved evictions < 0.1% of stores AND occupancy
  < ~80% full at 128 MB): **fails** — stress LS = 0.146% and occupancy 95.4%
  full; neither conjunct holds.
- **Condition B** (churn nonzero at 32 MB but node counts match 128 MB within
  noise): **fails** — 32 MB stress does not solve at all within 600 s /
  3.33 B evals (vs 249 M at 128 MB); the 2026-09-11 TT-size invariance claim
  does not transfer to the current (post-plan9) solver.

→ Proceed to Phase 1 per the plan, with the churn profile as sizing basis.

## Phase 1 arm table (128 MB, first-outcome child evals)

| arm | stress (249,480,478) | m22 (14,156,269) | dec13 (3,822,602) | dec10 (4,262,128) | quick outcomes |
|---|---:|---:|---:|---:|---|
| V1 solved-slot immunity | **timeout 600 s** (3,947,370,089) | 495,419,099 (**+3400%**) | 3,841,160 (+0.5%) | 4,262,128 (0.0%) | 59/59 preserved, evals bit-identical (never fires) |
| V2 steeper work priority | 344,866,966 (**+38.2%**) | 17,507,076 (+23.7%) | 3,822,602 (0.0%) | 4,262,128 (0.0%) | 59/59 preserved, evals bit-identical (never fires) |

Mechanics of the failures:

- V1 dropped 14,963,941 stores on stress (1,177,343 on m22) — three orders of
  magnitude more than the ~20 K evictions it prevents. The dropped unsolved
  bounds are re-searched from scratch; on stress the arm never reaches a
  decisive outcome (worse than even the 32 MB capacity collapse). Protecting
  live-solved slots *by discarding unsolved stores* is strictly harmful: the
  unsolved bounds carry the search.
- V2 changes the victim choice among unsolved slots; both stress (+38.2%) and
  m22 (+23.7%) regress. Notably V2's stress trajectory finds a *much shorter*
  first win (97 plies vs 477) but pays more evals to get there — the depth
  tiebreak reshapes the trajectory without reducing its cost. The existing
  `(live, solved, work, generation)` priority is already at a local optimum
  for this search.
