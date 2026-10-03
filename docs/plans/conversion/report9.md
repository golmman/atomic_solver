# Report 9 — Ordering-guidance seed from Fairy-Stockfish (backlog #2, lever 2a)

**Verdict: NO-GO.** All three pre-registered go/no-go bars were evaluated;
bars (a) and (b) passed, bar (c) — the ≤ 50%-of-baseline projection — **failed
on direct measurement**, and with it the ordering-guidance lever closes.
Per the plan's pre-registered rule, **backlog #2 closes entirely** (the
verification lever 2b was already a measured no-go in `report2.md`), and the
engine-hybrid direction is marked exhausted in the initiative. Phase 0 only:
no production code was kept; the working tree is byte-identical to pre-plan9
(`git diff src/ tests/ examples/ Cargo.toml Cargo.lock` empty) and the fast
gate (`make test`) is green.

## What was run

Phase 0 per the plan: re-baseline at HEAD, engine query battery, coincidence
analysis, env-gated root-work attribution spike (`CONV9_SPIKE=1`, reverted),
plus one plan-deviating extension — an env-gated root-order promotion
(`CONV9_GUIDE=<uci>`, same spike module, also reverted) that converts bar (c)'s
analytic projection into a direct measurement (rationale below). Tools used:

- `target/release/atomic_solver` + `benchmark --json --runs 1` (the CLI does
  not expose `child_evals`; the JSON harness does) — step 0 baselines.
- `target/release/examples/move_order_debug` — root static-ordering probe.
- `libs/Fairy-Stockfish/src/stockfish` (the plan2 armv8 build; sha256
  `cf65ef2f…60309b` matched `report2.md`, no rebuild needed), driven over
  stdio UCI by a small Python script (committed as
  `measurements/plan9/sf_query.py`).
- Temporary spike `src/search/dfpn/spike_plan9.rs` + four marked hook sites
  (`evaluate_child` attribution, root-frame descent order, snapshot at the
  first decisive outcome, root-only guide promotion after `sort_moves`),
  fully deleted after measuring.

Case selection: stress = `make stress` FEN (= m21_white), m22 = m22_white,
dec controls **dec01/dec10/dec13** — the three hardest dec cases by plan8's
quick@0.125 `child_evals` (5.71M / 4.26M / 3.82M), consistent with the
`research` plan4 hard set. Engine walls matched the inherited figures
(0.86 s @1M, 6.8 s @10M); the 100M stress query was skipped per the plan
(10M already agrees with the solver's decisive root move).

## Step 0 — re-baseline at HEAD (all exact)

| case / mode | child_evals | nodes | wall | PV len | PV root move |
|---|---|---|---|---|---|
| stress FO | 249,480,478 | 13,907,467 | 53.1 s | 477 | **g3g4** |
| stress default | 338,094,183 | 19,943,731 | 70.5 s | 129 | g3g4 |
| m22 FO | 14,156,269 | 858,117 | 3.0 s | 95 | **g4h5** |
| dec01 FO | 5,713,706 | 284,313 | 1.2 s | 27 | f7g5 |
| dec10 FO | 4,262,128 | 617,048 | 0.7 s | 41 | c7c6 |
| dec13 FO | 3,822,602 | 164,880 | 0.9 s | 17 | d1f3 |

`pv_status: first-outcome` everywhere decisive; preflight defers on all six.

**The plan's premise needed correction before anything else.** The
plan-drafting probe's "solver's decisive root move `d6e5`" is the *static
scorer's* favorite, not the solver's own PV root move: at HEAD the solver's
stress baseline PV starts **g3g4** — the engine's own MultiPV-1 choice at
1M and 10M (report2). The probe itself reproduced exactly otherwise
(d6e5 rank 1 total 5180; g3g4 rank 22; b1b8 rank 25), but with plan8-drifted
totals (g3g4/b1b8 now 110 vs 1180/1050 at plan-drafting), which the plan
explicitly anticipated. m22 probe: g4g5 1 / d6e5 2 / e3f4 3 / g4h5 4.

## Step 1/2 — engine battery and coincidence table

Determinism verified: the stress@10M K=1 query was run twice with byte-identical
PVs. K=1 primary; one K=3 stress@10M run for the report2 cross-check.

| case | budget | K=1 root move | PV len | == solver decisive root move | static rank of that move | line-prefix overlap |
|---|---|---|---|---|---|---|
| stress | 1M  | g3g4 | 1  | **yes** | 22 | 1 ply |
| stress | 10M | g3g4 | 19 | **yes** | 22 | 1 ply (engine diverges at ply 2: h5g4 vs solver f5f4) |
| stress | 10M | (K=3) mpv1 d6e5, mpv2 g3g4, mpv3 b1b7 | 21/14/2 | mpv2 yes | — | — |
| m22 | 1M/10M | g4h5 | 1/15 | **yes** | 4 | 1 ply |
| dec01 | 1M/10M | f7g5 | 15 | **yes** | — | 3 plies |
| dec10 | 1M/10M | c7c6 | 23 | **yes** | — | 1 ply |
| dec13 | 1M/10M | d1f3 | 11 | **yes** | — | 3 plies |

The engine's MultiPV-1 root move coincides with the solver's decisive root
move on **all five cases at both budgets**. The recorded budget-instability
premise survives only at 100M (b1b8, report2) and inside the K=3 ladder —
both outside the mechanism's K=1 primary scope.

## Step 3 — root-work distribution (W)

Spike attribution: every `child_evals` increment attributed to its root
ancestor move; per root move the first root-sweep eval rank (the initial
root sweep's sorted order) and the first root-descent rank. Non-perturbation
verified twice: spike-off vs spike-on stdout md5-identical (stress FO
`cfc58bc4…`, m22 `8109ff0d…`), spike eval totals equal baseline evals
exactly.

**Unguided stress FO** (total 249,480,478):

- decisive root move **g3g4**, sweep rank **22** = descent rank **22**,
  21 root moves descended before it;
- **W = 83.57%** — 208.5M child evals spent under non-decisive root moves;
- largest sibling subtrees: c3e4 92.55M (37.3%), d6e5 20.55M (8.2%),
  c3d5 13.48M (5.4%), b1b8 11.06M (4.4%), d6f8 7.56M (3.0%);
- g3g4's own subtree: 40.98M (16.4%).

Default mode's first-decisive snapshot is identical (the default run reaches
the first decisive outcome at the same 249,480,478 evals; later snapshots are
refinement-round re-proofs and are not used). m22 spike: decisive g4h5,
sweep/descent rank 4, W = 26.87% — recorded as context; m22 is not the
target class.

## Step 4 — go/no-go on the pre-registered bars

- **(a) Coincidence — PASS.** On stress, K=1 at 1M and 10M has MultiPV-1 root
  move == the solver's decisive root move (g3g4), whose rank under the
  solver's own static ordering is 22 ≥ 2 — the seed would not be a no-op on
  paper. Dec controls impose no requirement (all five coincide anyway).
- **(b) Headroom — PASS on the letter.** W = 83.57% ≥ 10%. But see below:
  the guided measurement shows W is *not* recoverable, which is what bar (c)
  exists to test.
- **(c) Projection — FAIL, measured.** The plan's projection formula:
  *projected guided total = engine wall at the bar-(a) configuration + solve
  evals re-priced with the decisive root move promoted to rank 0, scaling W
  by the measured rank gap.* The analytic ceiling (perfect promotion, FO run
  stops at the first completed proof) is T·(1−W) = 41.0M evals (16.4%) +
  engine wall 0.86 s ⇒ ≈ 18% of baseline — well under the 50% bar — but only
  if g3g4's proof completes without sibling-subtree work, which the plan
  itself flagged as the potentially load-bearing unmeasured term. Rather than
  assume, I measured it: a spike extension (`CONV9_GUIDE=<uci>`) promotes one
  root move to the front of the root frame's sorted list (list-order
  promotion only, root frame only, ordering-only, reverted with the rest).

  Measured guided runs (FO `child_evals`):

  | case | guide move | unguided | guided | Δ | note |
  |---|---|---|---|---|---|
  | stress | g3g4 | 249,480,478 | **212,150,376** | **−15.0%** | wall 48.1 s vs 53.1 s; with engine +0.86 s ⇒ −7.9% total; PV 291, outcome win unchanged |
  | dec13 | d1f3 | 3,822,602 | 5,024,279 | **+31.4%** | proof completes through g4f5 instead |
  | dec01 | f7g5 | 5,713,706 | 5,713,706 | 0.0% | guide was already first |
  | dec10 | c7c6 | 4,262,128 | 3,212,311 | −24.6% | |
  | m22 | g4h5 | 14,156,269 | 22,336,876 | **+57.8%** | control from the report2 battery |

  Both clauses of bar (c) fail: guided stress-FO total is **85% of baseline**
  (bar: ≤ 50%), and dec13 regresses **+31.4%** (bar: no > 5% regression).
  The projection's load-bearing term is real: even with g3g4 descended
  *first*, the guided run still spends **80.9%** of its evals under sibling
  root moves — the decisive child's DF-PN subtree search is threshold-cut
  until sibling refutations have grown the thresholds and populated the
  TT/repetition cache, so the "wasted pre-decisive root work" is largely
  load-bearing, not waste. W measures where the evals sit, not which evals a
  reorder can remove.

All three bars conjunctive → **NO-GO**.

## Why this closes the lever (and the direction)

The mechanism's economics fail in both directions at once. On the target
class the best measured case is −15% evals / −8% wall — below any adoption
bar this initiative has used, and before paying the engine query or building
the plan10 mechanism. On adjacent classes the same seed regresses +31% to
+58%, i.e. the guide is not even value-neutral where the engine happens to
agree with the solver. And the direction's ceiling is now measured, not
argued: a root-order seed cannot recover W because W is structurally
load-bearing under DF-PN threshold dynamics. Together with report2's 2b
no-go (verification) and the lean oracle-floor bound on interior OR-ordering,
the engine-hybrid direction is exhausted at the diagnostic level.

## Deviations from the plan

1. **CONV9_GUIDE spike extension** (the material one, argued above): the plan
   asked for an analytic projection from step-3 numbers with the unmeasured
   term "marked as such and failing the bar if load-bearing". Since
   load-bearing-ness was directly measurable with a four-line, root-only,
   ordering-only spike hook inside the already-established spike module, the
   projection was measured instead of projected. This stays inside the
   one-lever scope (same lever, same mechanism family, no production code
   kept) and makes the no-go decision strictly stronger.
2. Solver's decisive root move is g3g4, not "d6e5-class" as the plan's
   premise recorded (see step 0). The plan's own instruction to re-run the
   probe at execution time is what surfaced this; the coincidence analysis
   was evaluated against the measured baseline PVs, as the plan defines.
3. move_order_debug probe totals drifted vs the plan-drafting probe (ranks
   1/22/25 stable, totals 5180/1180/1050 → 5180/110/110 post-plan8) —
   anticipated by the plan; ranks are what bar (a) consumes.

## Problems encountered

- Root-frame detection at the descent site: `path_push` happens *before* the
  selection loop, so the root frame is the frame with `move_stack` empty, not
  `path_stack` empty (the first spike revision silently recorded no
  descents; caught via `decisive_rank_descent=0` contradicting the summary
  counters, fixed, and re-verified for non-perturbation).
- `solve` in default mode re-decides in every refinement round, so the
  snapshot hook fires once per round; all reporting uses the *first* snapshot
  (= first decisive outcome, the FO cut).
- No 100M engine query was spent (plan-conditional: 10M agrees), keeping the
  engine battery at ~70 s wall total.

## Unresolved parts / missing tests

- None on the code side: nothing was kept, `src/` is byte-identical to HEAD,
  and the repetition soundness gate is vacuously satisfied (the binary is
  bit-identical to the one plan8's gates validated; no search-facing code
  changed).
- The analytic rank-gap scaling formula the plan sketched is moot with the
  direct measurement; it is recorded here only as the failed bar's
  statement.

## Next steps

- **Backlog #2 closed** (2b: `report2.md`; 2a: this report). The initiative's
  remaining open item is #5(e) (Gao 2021 mining) only; #4 lives in the
  `parallel` initiative. A sensible close: mine (e), then close the
  initiative with the deep-conversion lever set exhausted (repetition/cache
  semantics measured out in `dfpn`, ordering measured out here and in
  `lean`, pricing in plan6/plan7, ε in plan8).
- `plan10.md` will not be opened; the Phase 1 sketch in plan9 is void.
- Raw artifacts: `docs/plans/conversion/measurements/plan9/` (parsed JSONs,
  probe tables, UCI query driver; transcripts not committed per convention).
