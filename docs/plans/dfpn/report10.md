# Report: Clock-Budget-Aware Solved-Entry Reuse — Phase 0 No-Go (Plan 10)

## Summary

Implemented the Phase 0 sizing spike of `docs/plans/dfpn/plan10.md` (temporary
instrumentation, since reverted). The lever is **sound** and delivers a large
win on its target (stress case first-outcome child evals **−37.7%**, default
mode **−41.6%**), but it **fails the go/no-go**: the benefit and a severe
DF-PN destabilization are the *same mechanism*, and the plan's designated
no-adoption control (`m22_white`) collapses from a 3.1 s win to a 120 s
timeout. 15 of 59 quick-suite cases regress (up to +185%). A controlled
experiment (flooring the folded bounds of adopted children) rescues the
destabilization but simultaneously eliminates the entire stress-case win —
so there is no shippable variant on this design axis. **Backlog #2 is closed
as a measured negative result; all instrumentation was reverted; the tree is
byte-identical to the post-plan9 state (baseline reproduced exactly).** The
next ranked lever is backlog #3.

## What was built (all reverted)

Temporary, non-`cfg` spike instrumentation — the cross-clock index in its
production shape plus counters:

- **Index**: `HashMap<u64, (Move, Outcome, u32, u16, u64)>` on `Search` —
  rep_key (board-only `repetition_key`) → (best_move, outcome, proven depth,
  stored clock, stored subtree work). The clock/work components existed only
  for the spike's delta histogram and savings upper bound; the production
  payload would store `(best_move, outcome, depth)`.
- **Store site**: the frame-exit TT store in `core.rs` `dfpn`, only for
  `store_outcome == Some(Win | Loss)` (draws — including GHI-suppressed ones —
  and unsolved bounds never enter; terminal-branch stores excluded per plan).
  Persisted across `begin_run` (mirrors the TT; note the live search never
  calls `tt.clear()`/`new_generation`, so "TT lifetime" = the `Search`
  lifetime).
- **Probe site 1** (`dfpn` node entry, after the plan9 repetition-cache probe,
  before `sort_moves`): on a hit satisfying `rule50 + d ≤ 100`, `d ≤
  max_depth`, and the one-ply `best_move_repeats_path` guard, emit
  `NodeProven` and return the outcome.
- **Probe site 2** (`evaluate_child`, when the exact-key `resolved` is
  `None`): same adoption rule with `d ≤ child_max_depth`; adopted children
  produce a solved `ChildInfo` (`repetition_seen: false`) exactly like an
  exact-key resolved hit.
- Env-var kill switches for differential experiments
  (`PLAN10_SPIKE_OFF`, `PLAN10_SPIKE_ONLY=win|loss`,
  `PLAN10_SPIKE_WHERE=or|and`, `PLAN10_SPIKE_FLOOR=1`,
  `PLAN10_SPIKE_DUMP_SAMPLE=n`, `PLAN10_SPIKE_ROOT=1`) and a temporary
  `examples/solve_stats.rs` runner. All removed.

A methodological note: the plan prescribed the reconstruct-style 101-clock
TT scan (`board_key ^ rule50_key(c)`) inside the instrumented build. This
spike instead populated a direct rep-key map at the store site (O(1) probe),
which counts exactly the same quantity — "a same-board solved Win/Loss entry
satisfying the adoption rule" — while costing one hash lookup instead of 101,
keeping the 2–6 minute measurement runs tractable. Because production would
use the same store sites, the spike numbers are the production numbers, not
an extrapolation.

## Measurements

Baselines were re-verified bit-for-bit with the same binary
(`PLAN10_SPIKE_OFF=1`) immediately before measuring:

- Stress case first-outcome: **13,907,467 nodes / 249,480,478 child evals /
  53.8 s**, first line 477 plies — matches report9 exactly.
- Stress default mode: **19,943,731 nodes / 338,094,183 child evals / 72.3 s**,
  refined PV 129 `cap-cut` — matches report9 exactly.
- `m22_white` control: **win in 3.1 s (858,117 nodes / 14,156,269 evals)**.

All runs: 128 MB TT, default ε, reference host.

### Stress case `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`

| Mode | Metric | Baseline | With index | Δ |
|------|--------|----------|------------|---|
| first-outcome | child evals (primary) | 249,480,478 | 155,394,450 | **−37.7%** |
| first-outcome | nodes | 13,907,467 | 8,544,142 | −38.6% |
| first-outcome | wall time | 53.8 s | 37.2 s | −31% |
| first-outcome | first line | 477 plies | 303 plies | shorter |
| default | child evals | 338,094,183 | 197,745,729 | **−41.6%** |
| default | nodes | 19,943,731 | 11,660,155 | −41.5% |
| default | refined PV | 129 `cap-cut` | 49 | shorter |

Adoption statistics (first-outcome): site 1 = **0** adoptions (the exact-key
TT check and the plan9 repetition-cache probe intercept essentially every
node-level revisit); site 2 = **1,111,158** adoptions with a would-be savings
upper bound of 87,351,073 cumulative child evals (the realized saving,
94.1M, exceeds the bound because avoided descents also avoid cascaded work).
Default mode: site 1 = 1, site 2 = 1,975,249 (bound 420.6M).

The spike's delta histogram (stored-clock − probe-clock, first-outcome) is
concentrated near 0 (Δ0: 623,614; |Δ| ≤ 4: ~180k) with a thin tail to ±22 —
consistent with shuffle-driven clock drift; the `rule50 + d ≤ 100` budget
rule rarely binds at these deltas.

Working set (no eviction, so final size = peak): **169,978** entries (stress
first-outcome), 197,325 (stress default), 59,003 (m22 run) — below the
plan's `1 << 18` cap; the cap never bound (0 drops).

### The control case fails: `m22_white` `4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22`

With the index active the control does **not solve**: 120 s timeout,
111,693,824 nodes / 714,594,659 evals (vs baseline win in 3.1 s) — with
46.9M adoptions and a working set of 59k entries. Root-threshold telemetry
(`PLAN10_SPIKE_ROOT=1`) shows the instability: baseline root pn climbs
48 → 103 → 141 → 148 and solves in chunk 5; with the index, root pn climbs
48 → 176, then drops to 30 while **root dn explodes 9,316 → 503,529** —
DF-PN's disproof-number schedule destabilizes and the search wanders into
very deep shuffle lines (max_depth 206 vs 51 at comparable work).

### Quick-suite drift (`benchmark --suite quick --json --first-outcome`, 59 cases)

**All 59 outcomes unchanged, `wrong=false` everywhere** — the soundness
surface is clean. But 37 cases changed work: 22 improved (dec10 −74.1%,
m23_white −64.8%, m23_black −50.8%, m24_black −34.6% …), **15 regressed**
(dec43 +185.4%, dec06 +147.6%, dec36 +144.6%, dec01 +94.7%, dec40 +73.1%,
dec14 +28.4%, dec17 +7.3% …), and 22 were unchanged. Note the unpredictability:
the stress case (m21_white, −37.7%) and its sibling m22_white (timeout) are
the same position family with opposite outcomes.

### Differential experiments (stress first-outcome unless noted)

| Configuration | Stress | m22 control |
|---|---|---|
| full lever | 155.4M (win) | timeout (714.6M @ 120 s) |
| adopt-Loss only | 240.1M (win) | win 3.1 s / 13.4M (mild gain) |
| adopt-Win only | 145.5M (win) | timeout (168.0M @ 30 s) |
| Loss @ AND-children only | 206.1M (win) | timeout (68.9M @ 15 s) |
| Loss @ OR-children only | timeout (>310.5M) | — |
| Win @ AND-children only | timeout (>276.3M) | timeout (69.0M @ 15 s) |
| Win @ OR-children only | timeout (>271.7M) | timeout (92.1M @ 15 s) |
| only when child has no TT entry | timeout (>280.3M) | timeout (70.8M @ 15 s) |
| **folded bounds floored to ≥ 1** | **timeout (>258.5M) — benefit gone** | **win 5.2 s / 21.7M** |

Reading: every 2×2×2 slice is *worse* than the full combination on the
stress case (the adoptions are synergistic), no slice rescues m22, and the
gap-filling gate (adopt only when the child has no TT entry) is strictly
harmful — it reactivates node-entry adoption (276,764 adoptions carrying
128.9M would-be work), which destabilizes even the stress case.

## Why the go fails: the benefit and the pathology are one mechanism

1. **Soundness is not the problem.** The shift-lemma argument held up under
   attack, and it was verified empirically: 444/444 sampled adopted Win
   claims (depth ≤ 3, dumped via `PLAN10_SPIKE_DUMP_SAMPLE` from the
   pathological m22 run) were independently re-proven by fresh `Search`
   instances at the adopted clock; 59/59 quick-suite outcomes unchanged;
   the cyclic rook position stays a draw with the index active. Win/Loss
   proofs in this solver are repetition-robust by construction (repetition
   edges evaluate as Draw and can never support a decisive proof), so
   cross-path conservatism is genuinely inherited, not extended.
2. **The pathology is threshold arithmetic.** A solved child folds
   `(0, INF)` or `(INF, 0)` into an unsolved parent's `Σ`/`min` bound
   computation (`select_from_children`). For unsolved parents this yields
   bound-0/INF-polluted values that distort DF-PN's threshold propagation
   (the observed root-dn explosion). This folding already exists for
   exact-key TT-resolved children — the lever multiplies its frequency
   ~50–100× and crosses the stability boundary on some positions.
3. **Flooring the folding fixes the pathology and kills the lever.** With
   adopted children's folded bounds floored to ≥ 1 (`PLAN10_SPIKE_FLOOR=1`),
   m22_white wins again — but the stress case returns to baseline-or-worse
   (258.5M, no win in 60 s). The aggressive folding *is* the win: it lets
   DF-PN treat cross-clock-refuted defender shuffles as free solved mass.
   There is no configuration measured that keeps the stress win without the
   instability.

Per the plan's go/no-go and the initiative's working agreements (a lever the
solver cannot ship on its designated control case is not implementable as
is), the item is closed as a **negative result**. All instrumentation was
reverted; `git status` is clean and the restored binary reproduces the
post-plan9 stress baseline exactly (identical chunk log, nodes, 477-ply
line).

## Tools/Examples Used

- Temporary `examples/solve_stats.rs` runner (outcome/nodes/child_evals/wall
  + spike counters + delta histogram) — deleted.
- Temporary env-var-switched instrumentation in `Search` (index, counters,
  differential gates, adopted-claim dumper, root-threshold telemetry) —
  reverted via `git restore`; tree verified byte-identical to HEAD.
- `benchmark --suite quick --json --first-outcome` before/after drift.
- Fresh `Search` re-solves to independently verify 444 sampled adopted claims.
- `tests/fixtures/move_order_positions.txt` (stress case = m21_white;
  m22_white control).

## Problems Encountered

1. The plan's Phase 0 anticipated only two failure modes ("adoptions rare"
   or "working set pathological"). The realized failure mode was a third:
   abundant, sound adoptions whose *threshold-folding side effect*
   destabilizes DF-PN on other positions. Future sizing spikes for
   value-reuse levers should instrument the parent's folded (pn, dn) at
   selection time, not just adoption counts — the adoption counters alone
   looked like a clear "go".
2. The site-1/site-2 asymmetry predicted by the plan ("the win concentrates
   at `evaluate_child`") was confirmed in the healthy configuration (site 1
   ≈ 0 adoptions: the exact-key check and the plan9 cache intercept
   node-level revisits), but site 1 becomes dominant and harmful in
   constrained variants (gap-gated run) — probe-site ordering is load-bearing
   in ways the plan did not spell out.
3. Shell-quirk incident (no repo impact): a `sed` verification script
   mis-parsed `first_outcome=true` as the outcome token, producing 444 false
   "mismatches"; fixed and re-run (0 mismatches).

## Unresolved Parts

- The lever is closed, not postponed: no measured variant keeps the
  stress-case win without the destabilization. Any revival needs a mechanism
  that decouples them (see Next Steps), not a re-tuning of the adoption rule.
- The 303-ply (first-outcome) and 49-ply (default) PVs produced by the
  instrumented stress runs were not replay-verified — moot after the revert.
- `AGENTS.md` was intentionally left unchanged: nothing landed, and its dfpn
  paragraph correctly describes the post-plan9 state.

## Missing Tests

- None added; with a no-go there is no feature surface to test. The
  soundness evidence (claim re-verification, outcome preservation, cyclic
  rook) lives in this report only, as the spike artifacts were reverted per
  plan.

## Next Steps

- **Backlog #3 (continue refinement after cap-cut) is the next ranked
  lever** and is unaffected by this result.
- Candidate new backlog item surfaced by this spike — **"solved-child bound
  folding hygiene"**: when an unsolved parent folds solved children into its
  pn/dn (`select_from_children`), the 0/INF contributions distort DF-PN
  threshold propagation; today this is rare (exact-key reuse only), but it
  is load-bearing (the floored variant changed stress-case behavior even
  with the index active). A principled study (floor vs. exclude vs. current)
  must be gated on the bit-identical drift protocol, since it changes the
  baseline search on every TT-resolved position — measured here only as a
  diagnostic, not a lever.
- The stress case's remaining headroom is unchanged from report9's analysis:
  cross-path reuse under a *different* ancestor set (backlog #4) and the
  refinement tail (#3). This spike adds a caution for #4: any mechanism that
  injects solved facts into DF-PN at volume must ship together with stable
  threshold semantics, and must gate on the m22_white control explicitly —
  it is a far more sensitive canary than the quick suite (its baseline is
  3.1 s; the lever turned it into a 120 s timeout while 44 of 59 quick cases
  stayed within ±30%).
