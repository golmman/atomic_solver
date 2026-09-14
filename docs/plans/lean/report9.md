# Lean Report 9 — #5 AND-side ordering: phase-0 measurement spike

Spike executed 2026-09-15 per `plan9.md`. **No `src/` changes landed**; the
temporary `LEAN9_SPIKE` instrumentation (spike module + hook sites in
`core.rs` / `children.rs` / `mod.rs` / `main.rs`) was applied, measured, and
fully reverted — `src/` is byte-identical to the pre-spike tree (`git diff`
empty) and the post-revert m22 first-outcome stdout is byte-identical to the
pre-spike run. Raw artifacts under `measurements/plan9/`.

## Decision

**Backlog #5 (AND-side ordering signal, non-NN): NO-GO, closed.** Both
no-go clauses of the pinned decision rule fire:

1. **Q1 — refuter rank is already 0.** The median rank of the winning child
   in the final sorted move order at refuted AND frames is **0** on all
   three cases (m22: 8,249/8,258 at rank 0; shuffle: 112,578/112,608;
   m24: 23/23). Rank 0–1 covers **100.0%** of refuted frames everywhere —
   at the pinned ≥ 50% working bar this is not "a material fraction", it is
   all of them.
2. **Q2 — the recoverable mass is ~0, not ~5–20%.** Pre-refuter AND-frame
   eval mass is **0.00%** of total child evals on all cases (strict
   frame-local attribution), **≤ 0.02%** under the generous nested-subtree
   attribution. The backlog's "~5–20% evals" estimate is refuted with
   numbers.

The M4/M5 signal probes are moot against an empty surface, but they
*explain* the emptiness (see Q3): the refuter is almost always a
terminal-classified reply (extinction capture / mate) that the existing
static scorer already ranks first.

No successor plan; `initiative.md` backlog row updated to closed. The
initiative itself stays open (this is a backlog-item closure, not an
initiative pivot), so `docs/plans/README.md` is unchanged.

## Measured surface structure (why the surface is empty)

Per-frame own-eval accounting (every `evaluate_child` call belongs to
exactly one `dfpn` frame's sweep or re-eval slice; the partition summed to
the exact `child_evals` total on every run — delta 0 — which doubles as the
attribution correctness check):

| case (first-outcome) | total child evals | OR own | AND own | refuted AND own | proven-Loss AND own | cut AND own |
| --- | --- | --- | --- | --- | --- | --- |
| m22_white (30 s) | 14,156,269 | 5,510,656 | 8,645,613 | 8,271 | 11,447 | 8,625,570 |
| shuffle-win (100 s) | 249,480,478 | 100,877,680 | 148,602,798 | 112,722 | 96,408 | 148,390,289 |
| m24_white (20 s) | 406,737 | 172,130 | 234,607 | 23 | 686 | 233,896 |

The OR/AND split reproduces plan7 phase 1 exactly (5,510,656 / 8,645,613
and 100,877,680 / 148,602,798) — a cross-check on the new accounting. The
decisive fact: **99.7–99.9% of AND-frame own evals sit in threshold-cut
frames** (frames that exhaust their DF-PN thresholds without an outcome),
not in refuted frames. Reordering cannot salvage a cut frame — it never
reaches a refutation exit; the work is unsolved-subtree exploration priced
in by the disproof thresholds. The refutation exits themselves are nearly
free: the sweep finds the refuter immediately (rank 0) because it is a
statically-detectable terminal (Q3 below), so pre-refuter work is ~0 by
construction.

## M1 (Q4) — OR-frame decisive-child concentration

Winning-child attributed work / total work across OR frames proven `Win`,
plus rank of the winning child in the final sorted order:

| case | OR-Win frames | concentration | rank 0–1 share |
| --- | --- | --- | --- |
| m22_white | 26,026 | 80.8% | 97.3% |
| shuffle-win | 212,308 | 35.8% | 97.4% |
| m24_white | 606 | 26.9% | 97.7% |

- **Rank-wise the Q4 check passes**: the winning child is already ranked
  0–1 in ~97% of OR-Win frames — existing ordering is effectively
  rank-optimal at the frame level, consistent with the nn campaign's
  conclusion that OR ordering is at its floor.
- The work-share **concentration figures are far below the nn 90.6%**
  (m22 80.8%, shuffle 35.8%). This is a metric-population difference, not a
  reopened headroom: the `nn` figure attributed per-node work over the
  finalize-attributed proof replay, while M1 uses fresh in-search counters
  over *all* expanded OR-Win frames — the bulk of OR work lives in
  threshold-cut frames (130,560/156,596 on m22; 2,632,886/2,845,274 on
  shuffle) whose exploration is priced by DF-PN thresholds, not by ordering
  quality. The `nn`-era non-goal (no reopening of OR-side ordering) stands;
  nothing in this spike reopens it.

## M2 (Q1) — refuter rank at refuted AND frames

| case | refuted frames | rank 0 | rank 1 | rank 2–3 | rank 4–7 | rank 8+ | in initial sweep | by recursion |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| m22_white | 8,258 | 8,249 | 5 | 2 | 2 | 0 | 8,258 | 0 |
| shuffle-win | 112,608 | 112,578 | 22 | 4 | 4 | 0 | 112,608 | 0 |
| m24_white | 23 | 23 | 0 | 0 | 0 | 0 | 23 | 0 |

Two structural findings beyond the rank itself:

- **The refuter is always found in the initial sweep** (0/120,889 frames
  resolved by selection-loop recursion): refuted AND frames never need the
  re-evaluation path — the disproof exit fires during the first pass.
- The refuter is **statically classified at eval time** (M5): extinction
  blast-captures dominate (m22 5,407/8,258 = 65%; shuffle 106,764/112,608 =
  94.8%; remainder mostly moves-empty mate terminals, 11–85 TT-resolved,
  0 repetition/rule50).

## M3 (Q2) — recoverable pre-refuter mass

Both documented attribution variants (see plan9.md M3):

| case | pre-refuter own evals (strict) | % of total | refuter own evals | generous pre-refuter (work − refuter-attributed descents) |
| --- | --- | --- | --- | --- |
| m22_white | 0 | 0.00% | 8,271 (0.06%) | 3,278 (0.02%) |
| shuffle-win | 0 | 0.00% | 112,722 (0.05%) | 19,504 (0.01%) |
| m24_white | 0 | 0.00% | 23 (0.01%) | 0 (0.00%) |

Even counting the refuter's own eval as recoverable (it is not — a proof
must evaluate the refuter) the whole refuted-frame class is ≤ 0.06% of
child evals. The proven-Loss class (all replies evaluated) is similarly
tiny (0.04–0.17%). **Ceiling measured: two orders of magnitude below the
~5% bar.**

## M4 (Q3a) — online counter-move probe

Probe-before-update at every refuted AND frame, keyed by the entering
attacker move (last `move_stack` entry), last-writer-wins table:

| case | probes | table hit | miss (entry, other move) | no entry | hit rate (of probed with entry) | hit rank 0–1 | top-refuter share over all refutations |
| --- | --- | --- | --- | --- | --- | --- | --- |
| m22_white | 8,258 | 5,618 | 2,333 | 307 | 70.7% | 100% | 58.5% (285/307 attackers) |
| shuffle-win | 112,608 | 76,062 | 35,898 | 648 | 67.9% | 100% | 53.8% (465/648) |
| m24_white | 23 | 18 | 0 | 5 | 100.0% | 100% | 100% (5/5) |

The classic counter-move heuristic would have worked reasonably *as a
signal* (~68–71% hit rate, hits at rank 0-1, top refuter covering ~54–59%
of refutations per attacker move). But with 0.00% recoverable mass there is
nothing for it to save; recorded here so a future plan does not re-derive
the probe.

## M5 (Q3b/c) — resolution source and existing-signal coverage

- **Resolution source** (M5): refuters are terminal-classified children —
  extinction (commoner blast / last-commoner capture) or moves-empty mate;
  TT-resolved hits are negligible (≤ 0.1%), recursion-resolved refuters do
  not occur. No `TtEntry` field or new table could surface them earlier
  than the terminal check already does.
- **Static-score rank** (M5b): at refuted AND frames the refuter sits at
  static-score rank 0–1 in **100.0%** of frames on all cases (m22:
  8,247+5; shuffle: 112,571+30+7+9; m24: 23+0) — identical to the final
  sorted rank. The existing AND-scaled static profile alone produces the
  rank-0 refuter; history/killers contribute nothing that matters.

## Instrumentation neutrality (revert contract evidence)

Same binary, `LEAN9_SPIKE=base` (ungated behavior, triple printed) vs
`LEAN9_SPIKE=1` (counters active):

| case | nodes | child_evals | stdout vs pre-spike |
| --- | --- | --- | --- |
| m22_white | 858,117 = 858,117 | 14,156,269 = 14,156,269 | byte-identical |
| m24_white | 22,739 = 22,739 | 406,737 = 406,737 | byte-identical |
| shuffle-win | 13,907,467 = 13,907,467 | 249,480,478 = 249,480,478 | byte-identical |

The m22/shuffle triples also match the conversion-plan4 recorded baselines
exactly. The m24 dynamics sentinel stayed at its 15-ply win with unchanged
nodes — no perturbation. Post-revert: `git diff -- src/` empty, m22
first-outcome stdout md5 `ea72f7ea…` (matches pre-spike and report8),
`make test` green. The spike touched only counters/probe maps — no
`sort_moves`, selection, TT, or history/killer logic.

## Method notes and caveats

- **Attribution approximation (documented per plan9.md M3)**: the strict
  metric counts frame-local own evals (sweep evals before the winning
  index + non-refuter re-evals); because every `evaluate_child` call
  belongs to exactly one frame's slice, the strict partition is exact
  (delta 0 vs `child_evals` on all runs). The generous variant adds
  nested-subtree descent work attributed to non-refuter replies and may
  double-count with nested frames' own accounting; it is reported as the
  ceiling-side number.
- **M1 vs the `nn` 90.6% figure**: different populations and work
  attribution (fresh in-search counters over all expanded frames here vs
  finalize-attributed per-node replay on the archived `nn` branch). The
  discrepancy does not reopen OR-side ordering: the rank-level finding
  (winning child already at rank 0–1 in ~97% of OR-Win frames) is the
  ordering-quality statement, and it is as strong at OR as at AND.
- Runs were `--first-outcome` per the plan's command table, so the
  counters cover the first-outcome phase only (no refinement rounds).

## Follow-ups

- Backlog #5 is **closed** in `initiative.md` with these numbers; the
  AND-side ordering lane is empty.
- The one genuinely open AND-side observation — 99.8% of AND own evals in
  threshold-cut frames — is a DF-PN *threshold dynamics* property, not an
  ordering property; it belongs to the `dfpn` initiative's algorithmic
  items, not to a move-ordering signal.
