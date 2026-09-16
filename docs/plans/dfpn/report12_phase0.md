# Report 12 — Phase 0 (Session A): KQvK ladder — semantics resolution + work-mass diagnostics

Plan: `plan12.md` (backlog #5). Date: 2026-09-15. Raw logs and spike sources
under `docs/plans/dfpn/measurements/plan12/`. Session B (Phase 1) was **not**
started; per the plan's execution protocol this report ends with the G1/G2
outcomes and an explicit arm recommendation for the user checkpoint.

## TL;DR

- **G1: GO.** The ladder wins are genuine under the solver's path-repetition
  semantics. T1a (exact-DTM rank-decreasing strategy, exhaustive line
  enumeration) and T1b (independent repetition-aware bounded prover) agree on
  **all four** ladder positions: WIN, exact DTM 15/13/11/9, cycle-free.
- **G2: FAIL on the plan's operationalization** (bounded `search_depth(·,15)`
  run: attributed mass **8.1%** of the ~2.83B-child-eval work gap, gate ≥
  50%). The plan's hypothesis — repetition-contaminated subtrees defeating
  TT/cache reuse — is **measured dead** on this class: the repetition
  machinery is almost perfectly inert (0% path-repetition frame exits, ≤
  0.66% plan9-cache hit rate, ~300 suppressed stores, class-1 mass ≈ 0.00%,
  clock-fragmentation events = 0).
- **Recommendation: close plan12 as an evidence-based no-go** (the
  plan10/plan11 pattern). Arms A2 and D have no mass to cache; Arm C fails
  its own precondition ("T1b solves the ladder cheaply" — the root proof cost
  1.02B nodes, the same order as the solver's own 997M-node failure). The
  dominant mass (91.3% class-3 on the gate object) is **unsolved frontier
  churn**: 79% of dfpn frames are depth-0 leaves, 99% of child evals are
  unsolved, and the bounded search resolves *zero* frames via the TT — it
  never proves anything inside the horizon, so there is nothing to cache.
  The lever class for the ladder is threshold/horizon pricing, not
  repetition semantics.

## T0 — ladder reproduction (R7 bands)

Clean HEAD build (95295f0), temp runner (`plan12_run`, archived), 128 MB TT,
default ε. Default mode = `solve`, bounded run = `search_depth(15)`.

| FEN | outcome | exit | nodes | child evals | fo child evals | wall | last FO chunk boundary (addendum metric) |
| --- | ------ | ---- | ----: | ----------: | -------------: | ---: | ----: |
| root `8/2K5/k7/8/8/8/8/4Q3 w - - 0 1` | Draw | Timeout | 997,392,384 | 5,396,315,458 | – | 600.0 s | **755,986,909** (= addendum, bit-exact) |
| step1 `8/1k1K4/8/8/8/8/8/4Q3 w - - 2 2` | Win | Complete | 65,460,435 | 322,826,558 | 257,046,258 | 36.0 s | **42,682,820** (= addendum, bit-exact) |
| step2 `8/8/2k1K3/8/8/8/8/4Q3 w - - 4 3` | Win | Complete | 5,395,869 | 39,383,178 | 31,503,722 | 5.2 s | **3,874,308** (= addendum, bit-exact) |
| step3 `8/5K2/8/3k4/8/8/8/4Q3 w - - 6 4` | Win | Complete | 741,940 | 2,047,932 | 47,830 | 0.24 s | < 201,482 (= addendum) |
| bounded `search_depth(·,15)` (root) | Draw | Timeout | 1,105,641,472 | 2,832,550,718 | – | 300.0 s | 796,353,729 (addendum: 1,087,258,624 — band ✓) |

The addendum's "nodes to first outcome" numbers are the last work-chunk
boundary node counts of the first-outcome phase; all four reproduce
**bit-for-bit** on this host (the deterministic work schedule is
host-independent). Wall-clock-bounded totals form the R7 bands (this host is
~3% faster than the reference host on the bounded run). PVs: root none,
step1 19 plies, step2 10, step3 9 — all `pv_status: Unproven` (the exact
DTMs are 15/13/11/9, so none of the informational PVs is shortest; the
step-3 9-ply PV is length-correct but unproven).

## T1 — semantics resolution (G1)

Instruments: temp example `plan12_t1` (archived; source and full log in
`measurements/plan12/`). Both share no code with the solver; both use real
`atomic_movegen` movegen and the solver's terminal classification.

### T1a — strategy-cycle check (confirmatory, R6)

Retrograde AND/OR fixpoint with **exact DTM** over the region reachable from
each root (~420,532 positions per root), then extraction of the
rank-decreasing winning strategy (attacker plays a child with dtm exactly
d−1, the largest available) and exhaustive enumeration of every line under
it (defender plays all replies):

| root | table value | derived | exact dtm | table mismatches (region-wide) | ranked strategy: lines / mate leaves / cycles / budget exits | greedy probe |
| --- | --- | --- | ---: | ---: | --- | --- |
| root (dtm 15) | Win | WIN | 15 | **0** | 2,815 / 1,655 / 0 / 0 — cycle-free | **cycles at node 10** |
| step1 (dtm 13) | Win | WIN | 13 | **0** | 800 / 472 / 0 / 0 — cycle-free | **cycles at node 12** |
| step2 (dtm 11) | Win | WIN | 11 | **0** | 477 / 282 / 0 / 0 — cycle-free | **cycles at node 10** |
| step3 (dtm 9) | Win | WIN | 9 | **0** | 23 / 12 / 0 / 0 — cycle-free | **cycles at node 10** |

- The exact dtms **match the addendum's "optimal" column** (15/13/11/9).
- Every line under the rank-decreasing strategy mates within the root dtm
  with **no board repetition** (0 cycles, 0 budget exits on every root):
  strictly-decreasing dtm makes same-board recurrence impossible, so this is
  a constructive cycle-freedom certificate.
- The **naive greedy strategy (first table-winning child) cycles on every
  root** — the plan's cycle concern was real, and "prefer larger child dtm"
  (the plan's T1a tie-break) is exactly what repairs it.
- Region-wide cross-check: every decided region value agrees with the
  3-way-validated q-table (0 mismatches per root).

### T1b — repetition-aware bounded prover (primary G1 instrument, R6)

Independent AND/OR minimax with the solver's path-repetition cut (a position
on the current line is a Draw ⇒ `NOT_WIN`), memoized. As planned
(`plan12.md` T1b) the naive `(fen, depth, ancestor-set-hash)` memo **blew
up** (500M nodes at depth 9 with ~0 memo utility): ancestor-set diversity
defeats exact-key sharing, and UNKNOWN masses are not soundly shareable.
Two sound lemmas restored tractability (both documented in the archived
source):

1. **Subset transfer.** A WIN proven under ancestor set A stays valid under
   any A′ ⊆ A (fewer forbidden repeats); a NOT_WIN proven under A stays
   valid under any A′ ⊇ A (the defender keeps every escape). The memo stores
   proof ancestor-sets per (position, depth) and probes with subset tests.
2. **Monotonicity pruning.** The repetition-semantics game value for the
   attacker is ≤ the board-only value (repetition cuts only replace outcomes
   with draws), so a table-Draw child can never be a repetition-semantics
   win: at attacker nodes only table-Loss children are attempted (result
   UNKNOWN otherwise — never a claim). The table is used for ordering and
   this pruning only; **every WIN claim is still fully verified by the
   recursion** (real movegen + path-repetition cuts).

Results (node cap 500M default; root extended to 3B as documented effort):

| root | depth = dtm | depth = dtm+1 | depth = dtm+2 |
| --- | --- | --- | --- |
| root | **WIN** (1,024,541,116 nodes, 487 s) | UNKNOWN (cap) | UNKNOWN (cap) |
| step1 | **WIN** (267,181,782) | UNKNOWN (cap) | UNKNOWN (cap) |
| step2 | **WIN** (81,548,321) | **WIN** (426,013,500) | **WIN** (485,153,605) |
| step3 | **WIN** (30,242,510) | **WIN** (159,332,944) | **WIN** (153,653,412) |

**T1a and T1b agree on all four positions: the ladder wins exist under the
solver's path-repetition semantics.** The egtb addendum's caveat is resolved:
the egtb spike's "solver defect" conclusion was correct in substance — the
wins are genuine — and the "defect" is a search deficiency, not a semantics
mirage. (The egtb prover's own WIN claims were, strictly, unsound as win
*proofs* under repetition semantics — its memo can hide cycles; T1b's are
not.)

**Gate G1: GO.**

## T2 — work-mass diagnostics (G2)

Env-gated spike (`DFPN12_SPIKE=1`), R1 outermost-cause attribution, plus the
R3 clock-fragmentation counter. Runs: bounded-15 root (the G2 gate object),
root default, step1 default, m22_white FO (control), stress default
(control). Raw log: `measurements/plan12/t2_spike.log`.

| metric | bounded-15 root | root default | step1 default | m22 FO (control) | stress default (control) |
| --- | ---: | ---: | ---: | ---: | ---: |
| total child evals | 2,622,318,715 | 4,942,001,825 | 322,826,558 | 14,156,269 | 338,094,183 |
| dfpn frames | 1,023,451,136 | 913,080,320 | 65,460,435 | 858,117 | 19,943,731 |
| — depth-0 leaf frames | 812,017,426 (79.3%) | 0 | 16,450,959 (25.1%) | 0 | 285,108 (1.4%) |
| — path-repetition exits (item 3) | **0 (0.000%)** | **0** | **0** | **0** | **0** |
| — TT-resolved frames | **0** | **0** | **0** | **0** | 6 |
| — plan9 cache hits (item 2) | 0.11% | 0.51% | 0.66% | 0.02% | 0.16% |
| suppress stores / distinct positions (item 1) | 283 / 43 | 5,536 / 502 | 2,789 / 486 | 86 / 67 | 1,643 / 1,091 |
| multi-context positions / ctx-frag misses (item 2) | 28 / 1,053,675 | 314 / 3,545,042 | 292 / 121,746 | 13 / 22 | 286 / 2,257 |
| clock-frag misses (item 5) | **0** | **0**† | **0**† | **0** | **0**† |
| child evals resolved (item 4) | 0.84% | 2.19% | 3.40% | 1.13% | 0.83% |
| — of which repetition-draw children | 12,611,496 | 8,281,142 | 986,218 | 15,562 | 179,920 |
| evals at path depth 9–16 | 98.1% | 3.9% | 10.2% | 32.2% | 30.6% (61.3% at 5–8) |
| **class 1 (contaminated region)** | **509 (0.00%)** | 6,413 (0.00%) | 3,618 (0.00%) | 93 (0.00%) | 1,724 (0.00%) |
| **class 2 (fragmentation)** | **228,315,397 (8.71%)** | 3,326,565,170 (67.31%) | 78,826,578 (24.42%) | 1,395 (0.01%) | 676,885 (0.20%) |
| **class 3 (otherwise)** | 91.29% | 32.69% | 75.58% | 99.99% | 99.80% |

† runs where the (rep-key, context) registration map hit its 2M cap
(861.9M/41.2M/16.2M rejected registrations); late-run clock-fragmentation
detection is incomplete there. The bounded-15 root run never hit the cap
(1.19M pairs) and still recorded **0** clock-fragmentation events, so the
mechanism is absent where it would matter. Stress `stored_pairs` = 1,643
reproduces report9's measured working set exactly.

### Reading the data

1. **The plan's hypothesis is inverted.** Repetition-dependent results barely
   exist on this class: ~300–5,500 suppressed stores per run (vs 31,620 on
   the 32-men stress case), 0% path-repetition frame exits, ≤ 0.66% cache
   hits, class-1 mass ≈ 0.00%. The search is not "re-proving draw chains" —
   it **never proves anything**: 0 TT-resolved frames, 0.8–3.4% resolved
   child evals, 99%+ unsolved. The first-player-loss shortcut and the plan9
   cache are idle because their inputs (completed repetition-dependent
   proofs) never occur.
2. **The gate object's mass is frontier churn.** On the bounded-15 run, 79.3%
   of frames are depth-0 leaves and 98.1% of evals sit in the path-depth
   9–16 horizon band. The bounded search re-walks the horizon without
   ever collapsing a node — the churn is threshold/horizon dynamics (the
   lean9 finding: 99.7–99.9% of AND-frame mass in threshold-cut frames),
   not repetition caching.
3. **Item 5 (clock fragmentation) is exactly zero everywhere it could be
   measured.** Arm D's mechanism does not exist on this class.
4. **Item 2 (context fragmentation) is real but its lever is not.** 28–502
   distinct positions carry draws proven under multiple ancestor contexts,
   and on the *default* (unbounded) run the counterfactual class-2 mass is
   67.3%. But the only sound key reduction is R2's inert-ancestor lemma,
   whose key must over-approximate Reach(P) — and the 3-man position graph
   is (near-)strongly connected, so Reach(P) ≈ every ancestor and the key
   cannot collapse (R2's pre-registered "vacuous on exactly the target
   class" risk, confirmed). A *horizon-bounded* reachability filter would
   collapse contexts, but that is exactly Arm C's machinery (sound only
   inside a bounded proof), not a TT/cache key change.

### G2 computation

- Gate object: bounded `search_depth(·,15)` run. Work gap = 2,832,550,718
  (clean-run child evals) − ~7,500,000 (plan's transposition upper bound) ≈
  **2.825B child evals**.
- Attributed mass (class 1 + class 2) = 228,315,906 → **8.08% of the gap**
  (8.71% of the spike run's own total). Gate ≥ 50% → **FAIL**.
- Even under the charitable default-run reading (67.3% attributed), the gate
  also requires "a stated mechanism for caching it soundly": the only
  candidate mechanism (cross-context draw reuse) is unsound without the
  horizon bound (R2's lemma is vacuous here), and with the horizon bound it
  degenerates into Arm C, whose precondition fails (below). **G2: FAIL under
  every reading.**

## Arm assessment and recommendation

- **Arm A2 (context-key reduction, R2b): no-go.** Build condition technically
  met (fragmentation is context-driven, not clock-driven), but the gate
  fails on the gate object, and the lemma's over-approximation is vacuous on
  the 3-man graph — there is no key to shrink.
- **Arm D (repetition-key node component, R3a): no-go.** Its mechanism
  measured 0 events on every uncapped run.
- **Arm C (cycle-free bounded pre-phase, R5a): fails its precondition.** The
  plan conditions it on "T2 shows neither A nor B suffices **but T1b's
  repetition-aware prover solves the ladder cheaply**". T1b does solve the
  ladder, but not cheaply: the root proof costs 1.02B nodes / 487 s in a
  prover with table-guided ordering and subset-transfer memoization — the
  same order as the solver's own 997M-node / 600 s failure. Nothing in the
  data suggests a solver-integrated pre-phase would approach the plan's
  success criterion (root proven within 60 s and ≤ 20M child evals).

**Recommendation: close plan12 as an evidence-based no-go** (no Session B),
per the plan10/plan11 pattern. The plan's own no-go branch applies: "the mass
is diffuse or the dominant cost is none of these" — here the dominant cost is
frontier churn that no repetition-semantics change can touch. The KQvK
ladder itself stays open as a benchmark class; the measured lever direction
for a future plan is **bounded-search horizon/threshold pricing** (why the
bounded AND/OR search never resolves a node that a plain retrograde fixpoint
solves in ~6 s), not repetition handling.

## Soundness notes (spike only; nothing landed)

- All instrumentation was temporary and is **fully reverted**: `src/` is
  byte-identical to HEAD (verified via `git diff`/`git checkout`; the spike
  diff is archived as `measurements/plan12/spike_src.diff`).
- Post-revert verification: the rebuilt binary reproduces the T0 node counts
  exactly (65,460,435 / 5,395,869 / 741,940 on steps 1–3 via the search CLI)
  and `make test` passes.
- The spike was verified search-inert when enabled: with `DFPN12_SPIKE=1`
  the deterministic counters of step1, m22_white (858,117 / 14,156,269) and
  the stress case (338,094,183 child evals) match the recorded baselines
  exactly.
- R1 attribution implementation detail: fragmentation frames defer their
  attribution as "pending" values; a repetition-suppressed frame truncates
  its subtree's pendings and folds them into its class-1 mass
  (outermost-cause). Nested suppressed frames never double count.
- R8 honored in the T1 instruments: terminal classification is
  solver-equivalent (moves-empty with the checkers-bit split), illegal
  placements are unreachable by construction (the legal 3-man subgraph is
  closed), and inconclusive prover results are UNKNOWN, never proven Draws.

## Deviations from the plan

1. **T1b memo redesign** (documented above): the planned exact
   `(fen, depth, ancestor-set-hash)` key was built first and measured
   useless (500M nodes, zero effective sharing, cap thrashing); the shipped
   spike uses subset-transfer memoization + monotonicity-based attacker
   pruning, with both lemmas stated in the source. The table is ordering/
   pruning input only; WIN claims are fully recursion-verified.
2. **T1a realized as exact-DTM extraction + greedy probe** (R6's
   confirmatory reading). The plan's "search over winning-child choices"
   collapses to the rank-decreasing choice, which is provably cycle-free;
   the greedy probe documents that the naive choice indeed cycles.
3. **T0 metric identification**: the addendum's ladder numbers are
   last-chunk-boundary node counts, not exact first-outcome node counts;
   reproduced bit-for-bit on that metric and recorded additionally as exact
   child-eval totals (`fo_child_evals`) per R7.
4. **Host load**: the extended root T1b run (3B cap) overlapped the spike
   suite on a 4-core host; spike-run wall times and the spike root run's
   node totals drift accordingly (bounded-15 spike: 1.023B nodes vs 1.106B
   clean). Deterministic counters are unaffected; G2 was computed from the
   clean run's gap.
5. The plan's "T2 (only on G1 go)" ordering was kept; no arm was implemented
   despite G1 passing early (session protocol honored).

## Problems encountered

- `Board::outcome()` vs the solver's classification (report1 deviation 1)
  reconfirmed: both T1 instruments use the solver's classification.
- The monotonicity lemma's correctness depends on the q-table's WDL values;
  mitigation: the table is 3-way cross-validated (egtb plan1) *and* T1a
  re-derived all decided values over each root's reachable region with 0
  mismatches — the pruning never touches a claim's verification.
- T1b's depth-sensitivity: proving at depth = dtm succeeds while dtm+1/dtm+2
  often hit the node cap (poisoned-line verification cost grows with the
  horizon). Reported as measured; no fix attempted (spike scope).

## Missing tests

- No solver code changed, so no test gaps were introduced. The KQvK
  completeness gap itself remains untested in-suite (unchanged from the
  egtb plan1 state); a fixture addition is a plan-level decision that this
  no-go does not trigger.

## Next steps

1. **User checkpoint**: confirm the no-go closure (or override — Arm C
   remains *buildable* from the archived T1b machinery if its cost can be
   attacked first, e.g. via proof-number search instead of DF-PN thresholds).
2. On confirmation: mark backlog #5 closed (evidence-based no-go) in
   `initiative.md`, re-rank. Remaining measured lever for the deep-KQvK /
   bounded-proof class: horizon/threshold pricing of unsolved-frontier
   exploration (connects to lean9's AND-cut diagnostic and the plan10/11
   bound-folding hygiene note), or a PN-search-style bounded prover.
3. The archived T1 instruments (`plan12_t1.rs`) are reusable as an
   independent KQvK oracle if a future plan revisits the class.
4. `report12.md` (final, subsuming this Phase 0 report) is only written if
   the checkpoint overrides the no-go; otherwise this report closes the plan.
