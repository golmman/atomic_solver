# Report: Threshold-Cut-Frame Pricing — Phase 0 Diagnostic Spike (Plan 6)

Spike executed 2026-09-18 per `plan6.md`. The temporary `CONV6_SPIKE`
instrumentation (a `spike6.rs` module, hook sites in `core.rs` /
`children.rs` / `mod.rs`, and a temporary `examples/solve_stats.rs` runner)
was applied, measured, and **fully reverted** — `src/` and `examples/` are
byte-identical to the pre-spike tree (`git diff` empty on both paths;
post-revert m22 first-outcome stdout md5 `ea72f7ea…`, matching report9's
recorded hash) and the fast gate (`make test`) is green. Raw artifacts under
`docs/plans/conversion/measurements/plan6/`.

## Verdict

**GO — but not for any of the three pre-registered candidate classes.** The
diagnostics cleared the go bar through a fourth class the spike surfaced
(class D below): a **partial-sum sweep short-circuit** at summed-bound
threshold cuts, with a measured first-order addressable surface of
**74.2% of stress first-outcome child evals** (73.0% default mode, present
on every control). Classes A/B/C all failed their probes:

- **(A) chunk-boundary resumption** — surface measured (19.8% FO / 34.7%
  default ceiling) but its mechanism requires in-run per-node resumption
  state, i.e. a new cache surface: it fails go-bar 2's plan1-lane clause
  ("no reuse widening"). Measured, not permitted.
- **(B) threshold-growth shaping** — the surface is *empty*: ε is inert on
  this class. 93.5% of AND cut frames (81.0% OR) exit with
  `epsilon_ceil(second) − best = 1`, and the second/best ratio sits in
  [1, 1.25) for 97.8% of AND cuts — the schedule already grows thresholds by
  the minimum increment; there is no large ε step to reshape.
- **(C) per-frame work allocation** — the premise (mass concentrated in a
  few repeatedly re-entered deep frames) is refuted: the hottest position in
  the stress FO run burned 7,722 own evals; the top-20 positions together
  account for ~0.04% of child evals. The churn is broad and shallow, not
  concentrated.
- **(D) partial-sum sweep short-circuit** (discovered by the gap histograms)
  — threshold-cut frames burn 89–98% of their own evals in their *initial
  sweep*, and the summed bound overshoots the threshold (gap 4–63) that a
  prefix of the sweep already crosses. Stopping the sweep at the crossing
  point — the summed-bound analogue of the existing decisive-child early
  exit in `evaluate_all_children` — would, under the same trajectory, have
  skipped 185.0M of 249.5M evals (FO). This passes all four go bars on
  paper and is handed to `plan7.md`.

## Step 0 — baselines at HEAD

Re-verified before any instrumentation, generous `--timeout`, 128 MB TT,
default ε (0.125), reference container (aarch64, 4 cores):

| case | mode | child evals | nodes | line | vs inherited |
| --- | --- | --- | --- | --- | --- |
| stress `4r2k/…w - - 0 21` | first-outcome | **249,480,478** | 13,907,467 | 477 plies | exact match (post-plan9) |
| stress | default | **338,094,183** | 19,943,731 | 129 plies, `cap-cut` | exact match |
| m22_white | first-outcome | **14,156,269** | 858,117 | 95 plies, ~2.6 s | exact match |
| m24_white | first-outcome | **406,737** | 22,739 | 15 plies | exact match (report9) |

All inherited cross-references stand; the report9 *ratios* re-verified below
are literal. One plan correction: **`plan6.md`'s "m24" FEN
(`…p5Pp/5p1P…w - - 1 23`) is actually `m23_white`** per
`tests/fixtures/move_order_positions.txt` (the real m24_white is the
`…6Pp/p4p1P…1R2R2K` dynamics sentinel). Both were measured; m23 serves as an
extra small-class point (9,673,403 evals / 553,100 nodes / 33-ply win at
HEAD).

## Instrumentation (all reverted)

Counter-only, env-gated (`CONV6_SPIKE=1`; `CONV6_SPIKE_OFF=1` kill switch),
zero production behavior change. Temporary `src/search/dfpn/spike6.rs`:

1. **Frame-exit classification** at all five exits of the `core.rs` loop
   (solved / threshold-cut / no-best-child / work-cut / time-cut), per side
   (OR/AND): frame counts, own-eval sums, depth buckets. Every
   `evaluate_child` call is attributed to the innermost loop-active frame;
   early-return frames (terminal/leaf/path-rep/TT-resolved/cache-hit) consume
   no evals and are not loop frames.
2. **Per-position lifetime map** keyed by `tt_key` (unbounded by design —
   spike-RAM only): entry count, own-eval totals (overall and per exit
   class), an FNV best-move-stability signature, first/later-chunk eval
   split, top-20 dump. Peak ≈ 10.6M entries (stress default); with a
   64-byte entry plus hashmap overhead this is ≲1 GB — documented as
   spike-RAM, not a search-CLI contract change.
3. **Chunk-boundary waste** (`bounded_search`): per-round chunk lists
   (evals/chunk, decisive flag), plus the count of re-evaluations whose
   `(pn, dn)` returned unchanged (the `explored`-marking path).
4. **ε-sensitivity at cut exits**: which threshold fired, `bound − threshold`
   gaps (log-histograms, INF sentinel), the `epsilon_ceil(second) − best`
   ε-step the next re-entry prices, the parent-clamp flag, second/best
   ratio, and the class-D partial-sum saving (below).
5. **OR/AND split** of every counter.

**Attribution correctness (delta-0 check)**: the per-frame own-eval
partition summed to the exact `child_evals` total on *every* run (delta 0),
as did the per-position map — the report9 method, reproduced.

**Neutrality**: with the spike active, all four cases reproduced their
baselines bit-for-bit (same nodes, evals, PV, chunk log) — counters are
side-effect-free reads of state already computed.

## Anatomy of threshold-cut frames

### Exit-class eval partition — stress FO (249,480,478 total)

| side | class | frames | own evals |
| --- | --- | --- | --- |
| OR | solved | 212,388 | 1,442,084 |
| OR | threshold-cut | 2,618,204 | **98,419,058** |
| OR | no-best-child | 14,653 | 1,014,300 |
| OR | work-cut | 29 | 2,238 |
| AND | solved | 141,087 | 212,509 |
| AND | threshold-cut | 10,901,797 | **148,347,134** |
| AND | no-best-child | 11,696 | 42,581 |
| AND | work-cut | 23 | 574 |

Cut share: OR **97.6%** of OR own evals, AND **99.83%** of AND own evals —
**98.91% of all child evals sit in threshold-cut frames** (246,766,192),
and 97.3% of all loop frames are cut frames. The OR/AND own-eval totals
(100,877,680 / 148,602,798) reproduce report9 exactly. Default mode is
congurant (OR cut 130,249,787 of OR own; AND cut 200,579,696 of AND own;
97.8% overall).

### The mass is first-entry sweeps, not re-entry churn

Sweep vs re-eval attribution (stress FO): OR sweep 89,815,520 (89.0% of OR
own), OR re-eval 11,062,160; AND sweep 145,757,552 (**98.1%** of AND own),
AND re-eval 2,845,246. A threshold-cut frame never descends (the cut fires
before the descent), so cut-frame own evals are almost purely its initial
child sweep — 13.6 evals per AND cut frame on average.

Churn-vs-frontier (per-position map, stress FO; 10,140,752 positions,
13,899,877 entries = every loop frame exactly once):

| entries per position | positions | own evals | cut evals |
| --- | --- | --- | --- |
| 1 | 8,202,088 (80.9%) | **138,559,219 (55.5%)** | 138,193,999 |
| 2 | 1,352,771 | 51,414,259 | 50,756,118 |
| 3–4 | 424,531 | 30,154,061 | 29,796,723 |
| 5–8 | 105,907 | 12,820,681 | 12,556,109 |
| 9–16 | 36,831 | 7,748,293 | 7,440,526 |
| 17–32 | 14,076 | 5,345,363 | 4,984,860 |
| 33–64 | 3,547 | 2,295,496 | 2,005,227 |
| 65–128 | 848 | 857,042 | 759,692 |
| 129+ | 153 | 286,064 | 272,938 |

Reading: **55.5% of all child evals are spent at positions entered exactly
once** — one mandatory sweep each. The re-entry share (44.5%) is a broad
shallow tail, not a concentrated churn core: hottest position 7,722 evals,
top-20 ≈ 0.04% of the run. The best move was identical across all entries of
a position in 94.0% of positions (9,531,197) — re-entries mostly re-descend
the same child, but each entry still does fresh frontier work. Zero-progress
re-entries (bounds unchanged, the `explored`-marking path) are only
501,014 evals = **0.20%**.

### Chunk-boundary waste (candidate A)

Stress FO ran 9 geometric (×2) chunks; the decisive final chunk consumed
121,980,393 evals (48.9%), the eight bootstrap chunks 127,500,085 (51.1%).
Evals spent at positions first visited in an *earlier* chunk — the direct
ceiling for "re-walking structure the previous chunk had already reached" —
measured **49,489,390 = 19.8%** (FO) and **117,298,812 = 34.7%** (default;
34 refinement rounds, the capped final round alone 62.4M over 7 chunks).
m22: 10.4%. This surface is real but its mechanism requires carrying
per-node resumption (skeleton/frontier) state across chunks within a run —
a new in-run cache surface, which go-bar 2 forbids (plan1 lane). Recorded
as measured-but-not-permitted.

### ε-sensitivity at cut exits (candidate B)

| metric (cut frames) | OR (n=2,618,204) | AND (n=10,901,797) |
| --- | --- | --- |
| fired threshold | dn-only 90.6%, pn-only 9.3%, both 0.1% | pn-only 96.7%, dn-only 3.2%, both 0.2% |
| `epsilon_ceil(second) − best` = 1 | **81.0%** | **93.5%** |
| second/best ratio in [1, 1.25) | 88.1% | **97.8%** |
| parent clamps (`ε(second) ≥ th`) | 32.1% | 6.7% |
| exiting gap `bound − threshold` (fired side) | dn: 16–63 in 80.4% | pn: 8–15 in 48.0%, 16–63 in 22.8%, 4–7 in 17.4% |

The 1+ε mechanism adds essentially nothing on this class: the second-best
child is statistically tied with the best, so each re-entry prices a
threshold one unit above the achieved bound. There is no "large step" for a
reshaped schedule to smooth; the fired-side gap (4–63 overshoot) is the
summed bound blowing *past* the threshold, not an ε artifact. (B) is closed
at the diagnostic level: the schedule already grows by the minimum
increment.

### Class D — the partial-sum early-cut surface (new)

The gap histograms expose a structure none of A/B/C targets: a cut frame's
summed bound (AND: Σ child pn vs `th_pn`; OR: Σ child dn vs `th_dn`)
*crosses its threshold at a prefix of the sweep* — the frame evaluates all
~13–20 children anyway, then exits. The spike added a counter measuring,
per cut frame under the same trajectory, how many trailing children a
crossing-stopped sweep would have skipped:

| case (mode) | total child evals | OR saving | AND saving | **total (share)** |
| --- | --- | --- | --- | --- |
| stress FO | 249,480,478 | 66,811,189 | 118,207,971 | **185,019,160 (74.2%)** |
| stress default | 338,094,183 | 88,926,602 | 158,157,137 | **247,083,739 (73.0%)** |
| m22_white (FO) | 14,156,269 | 3,383,398 | 6,884,623 | **10,268,021 (72.5%)** |
| m23_white (FO) | 9,673,403 | 3,343,364 | 3,770,286 | 7,113,650 (73.5%) |
| m24_white (FO) | 406,737 | 113,516 | 187,272 | 300,788 (73.9%) |

Per-frame saving distribution (frames): AND concentrated at 8–15 skipped
children (5.28M frames) and 16–31 (2.41M); OR at 16–31 (1.11M) and 32+
(1.01M). The surface is remarkably stable (72.5–74.2%) across every case
and mode — including the m22 control, satisfying go-bar 4.

This is the summed-bound analogue of the *existing* decisive-child early
exit in `evaluate_all_children` (which already stops the sweep once a child
classifies as a parent-win): stop once the partial sum of the running
bound reaches the threshold the frame was given, and return the cut with
the partial sum as the stored (weaker) bound.

## Go/no-go verdict, bar by bar

1. **≥10% measured addressable surface (stress FO)** — **MET** by class D:
   74.2% (185.0M/249.5M), measured directly by the counters on the stress
   run, not extrapolated; default mode 73.0%. (A also measured ≥10% — 19.8%
   — but fails bar 2; B and C measured empty/refuted.)
2. **plan10 hazard test on paper** — class D **passes**: the crossing
   condition consults only the frame's own children bounds and its own
   thresholds; thresholds/bounds remain functions of (bounds, thresholds, ε)
   alone. No solved fact ever enters unsolved parents' arithmetic (the early
   cut returns an *unsolved* bound, never an outcome); no new cache, key,
   context, or adoption rule (all state is frame-local and discarded at
   frame exit); no repetition-semantics change. Full argument in the design
   brief below.
3. **Eval-count determinism** — **MET**: the crossing index depends only on
   deterministic child bounds (no wall-clock input); the mechanism only
   *reduces* evals consumed, leaving `child_eval_budget` /
   `ExitReason::BudgetExhausted` semantics untouched.
4. **m22 control shows the same surface** — **MET**: 72.5% (slightly above
   the stress share).

**Verdict: GO.** `plan7.md` opens for exactly one mechanism — the
partial-sum sweep short-circuit — drafted from this data. The
realized (post-redirect) saving can land well below the first-order
ceiling: weaker stored bounds may cause additional re-descents, and the
traversal changes wholesale. That gap is precisely what plan7's phases must
measure; the 74% ceiling leaves room for a large loss and still clears the
initiative's ≥5–10% lever bar.

## Mechanism design brief (input to `plan7.md`)

**Mechanism.** In `evaluate_all_children`, pass the frame's thresholds in
and stop the sweep as soon as the running summed bound crosses the
corresponding threshold: at AND frames Σ `child.pn` (saturating) vs
`th_pn`; at OR frames Σ `child.dn` vs `th_dn`. The frame then exits through
the existing threshold-cut path with the partial table: stored `pn`/`dn` =
the partial sums (`pn.max(1)`/`dn.max(1)` clamp unchanged), best/second
child and `best_move` from the partial table. The re-evaluation loop needs
no change (it operates after the sweep; its unchanged-marking path still
applies).

**Soundness argument (the hazard test, written out).**

- *No false decisive outcomes.* Proofs arise only from
  `selection.solved_outcome`, which is computed from children `outcome`
  fields (actual solved facts), never from bounds. An early-cut frame
  returns `Outcome::Draw` (unsolved) exactly like today's threshold cut;
  `suppress_draw`/repetition-cache stores are unaffected (they fire only on
  solved draws).
- *Bounds stay valid lower estimates.* A partial sum of child bounds is a
  valid underestimate of the frame's true summed bound (Σ over a subset ≤ Σ
  over all); DF-PN stores lower estimates for unsolved nodes and grows them
  monotonically across re-entries, so convergence semantics are preserved.
  The one asymmetry — an AND frame's stored `dn` (min over the evaluated
  subset) can overestimate the true min — affects only guidance, not
  provability, for the same reason (no outcome is derived from bounds).
- *No plan10 folding.* The mechanism never consults a solved outcome, the
  TT's outcome field, the repetition cache, or any cross-path index at the
  cut; the pathology of plan10/11 (solved (0, INF)/(INF, 0) facts folded
  into unsolved Σ/min arithmetic) is structurally unreachable.
- *No reuse widening (plan1 lane).* No new cache, key, context, or adoption
  rule: everything is frame-local, recomputed per entry, and discarded at
  exit.
- *Budget/determinism contract.* The crossing test is a pure function of
  deterministic child bounds; the mechanism strictly reduces evals
  consumed per frame, so `child_eval_budget`, refinement caps, and
  `ExitReason::BudgetExhausted` behave as before. No wall clock enters any
  decision; no `ProofEvent`s are added (Draw cut frames emit none today).

**Known risks for plan7 to measure.**

1. *First-order ceiling vs realized saving.* The 74% figure holds the
   trajectory fixed. Weaker stored bounds may cause parents to re-descend
   more often (the child looks cheaper than it is), partially or fully
   offsetting the sweep savings; conversely the freed budget lets later
   chunks go deeper. Plan7 must measure the realized net on the primary
   metric, with the m22 control and the quick suite as guards.
2. *Interaction with re-entry pricing.* A re-entered early-cut frame
   resumes from its stored partial bound; the crossing test must use
   thresholds the parent re-prices (unchanged logic), and the
   `explored`-marking path gains importance (bounds may repeat for more
   re-entries than before).
3. *PV quality.* Informational PVs come from TT best-move chains; weaker
   bounds may lengthen first lines (as plan1's +0.1% run showed lines can
   shorten or lengthen). PV is informational only; no contract change.

## Follow-ups

- Backlog #6 stays open, re-scoped: the mechanism plan is `plan7.md`
  (partial-sum sweep short-circuit), drafted from this report.
- Candidate B (threshold-growth shaping) is closed on this evidence; the
  closure should spare any future plan the EWS/MOPNS-style threshold
  re-derivation on this position class.
- Candidate A's 19.8%/34.7% ceiling is recorded for completeness; pursuing
  it would require reopening the reuse-widening lane (plan1's closed
  surface) and is not recommended.

## Tools / examples used

- Temporary `examples/solve_stats.rs` runner (outcome/nodes/child_evals/
  wall + full spike summary; `--fen/--timeout/--first-outcome`) — deleted.
- Temporary env-gated `src/search/dfpn/spike6.rs` + hooks in `core.rs`
  (exit classification, cut anatomy, class-D counter), `children.rs`
  (eval attribution), `mod.rs` (chunk/round accounting) — reverted via
  `git restore`; tree verified byte-identical to HEAD; `make test` green.
- Raw artifacts: `docs/plans/conversion/measurements/plan6/*.txt` (one file per run,
  spike-on and spike-off), incl. `git status` verification.

## Problems encountered

1. `plan6.md`'s "m24" FEN was `m23_white` (fixture mismatch — see Step 0);
   both positions were measured and the fixture names used in this report.
2. The first stress spike run initially appeared to abort under a shell
   loop; cause was word-splitting of the FEN in the loop variable, not the
   solver (re-run individually, clean).
3. The class-D counter was added mid-spike (after the first three
   instrumented runs) when the gap histograms revealed the crossing
   structure; all five instrumented runs were then re-measured with the
   final instrumentation. The numbers in this report are all from the final
   build; decision-identity was re-verified on every run.

## Missing tests

None added — with the spike reverted there is no feature surface to test.
The identity anchors (baseline reproduction spike-off, decision-identity
spike-on, delta-0 attribution) live in this report and the raw artifacts
only.

## Next steps

- Draft `plan7.md` from the design brief above (done in this session; the
  plan defers implementation to its own session per the one-lever rule).
- If plan7 measures a realized ≥10% net saving with clean controls, the
  class-D mechanism is the first structural lever since the `lean` plan9
  oracle-floor result; if it fails, backlog #6 closes with both the
  anatomy tables (this report) and the realized-saving measurement
  (report7) as closing evidence.
