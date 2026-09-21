# Initiative: `research` — reducing node count to first decisive outcome

## Status

Active, opened 2026-09-19. Research-oriented: the backlog is a living
ledger of open questions, literature targets, and POC candidates. Plans are
scoped to one discovery or measurement at a time. Nothing is implemented into
`src/` permanently from this initiative; proven ideas are handed off to
`lean`, `conversion`, or a new dedicated initiative for production.

## Motivation

The solver's performance initiatives (`lean`, `dfpn`, `conversion`) have
cycled through the locally-known lever set:

- `lean` — micro-optimizations and ordering heuristics (wall-time / node
  reductions with a bit-identical drift gate). Open items remain: #7 lazy/staged
  child evaluation, #10 history/killer constant re-tuning.
- `dfpn` — search-semantics changes for repetition-dominated positions (set
  dormant 2026-09-17 after plan13's pre-phase landed). Reopen triggers exist
  (a new measured diagnostic for deep repetition class; a soundness regression).
- `conversion` — deep tempo/progression conversion class. Open item: #4
  parallel search design spike (jointly with `lean` #2). Several items were
  closed as measured no-gos (EWS/MOPNS reading, threshold-cut-frame pricing).
- `egtb` — dormant; a future 4-man effort could reopen it.
- `proof` — dormant; TT checkpoint and deep-proof capacity are the reopen
  triggers.

Across all of these, first-outcome node counts on the stress case
(`4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`, post-plan9
249M child evals first-outcome) and the `m22_white` baseline (~14M child
evals) are not expected to drop from the currently-known backlog alone.
The honest next step is a *structured search outside the current
implementation*: identify what is still unsolved, scan recent literature for
techniques not yet evaluated, and run quick, instrumented POCs to separate
promising directions from dead ends before they become formal backlog items.

## Goal

Reduce `child_evals` to the first decisive outcome on hard positions by:

1. Maintaining an explicit inventory of unsolved problems in the current
   implementation (across all active and dormant initiatives);
2. Mining academic and practical literature for new algorithmic ideas
   applicable to atomic-chess DF-PN+ solving;
3. Running quick, temporary-instrumentation POCs to validate or reject those
   ideas with data;
4. Feeding proven ideas into the appropriate implementation initiative
   (`lean`, `conversion`, or a new one) as formal, sized backlog items.

Priorities follow AGENTS.md: correctness first, then performance, memory,
maintainability.

> **Re-scoped 2026-09-21 (pivot, not closure):** the node-count program is
> closed — every backlog row answered, closed, or pre-weakened; the ε
> surface has four closure legs (plan4/plan8); plan8 recommended closure.
> Instead of closing outright, the initiative pivots to **characterizing
> the structural floor**: one final cheap literature lever is mined
> (Deep df-pn 2017, #15/plan9), then the no-go record is consolidated into
> a single closing document stating what the solver is locked into and
> why, with evidence pointers (#16/plan10). The original goal text above
> is preserved for the record; hand-offs already made (ε=0.375 note,
> rescue-mass observation) are unaffected.

## Working agreement (research cadence)

1. **One discovery per plan.** A plan either inventories problems, mines one
   paper, or builds one POC for one idea. No multi-paper or multi-POC plans.
2. **POCs are throwaway.** They use temporary instrumentation, feature
   flags, or standalone scripts. Code is reverted or archived under
   `measurements/` before the plan closes; only the report and the decision
   (open / closed / handed off) survive.
3. **No drift risk from POCs.** Because POC code touching `src/` is reverted,
   there is no drift protocol for this initiative. If a plan claims a
   measurement from a temporary `src/` change, the revert is verified by
   `git diff --exit-code` or equivalent before the report is written.
4. **Feeding decisions.** A POC that succeeds (measured ≥ 10% node-count
   improvement on a hard case, or opens a new measurable surface, or
   identifies a sound algorithmic change) becomes a formal backlog item in the
   relevant implementation initiative. A POC that fails is closed with a
   no-go decision record and the reasoning archived.
5. **No competing with active backlogs.** This initiative does not implement
   production changes that belong in `lean` or `conversion`; it only
   researches and hands off.

## Backlog (opened 2026-09-19)

### Problem inventory

| # | Question | Motivation | Owner after hand-off |
|---|----------|------------|----------------------|
| 1 | What is the exact node-count breakdown of `m22_white` first-outcome into: OR decisive-child work, AND refutation work, threshold-cut churn, TT-resolved reuse, and pre-flight hits? | The `lean` plan9 spike gave a coarse partition; a sharper profiler would expose which surface is largest *unmeasured*. | `lean` or `conversion` |
| 2 | Which positions in the benchmark suites are "outlier hard" (`child_evals` >> median) and what structural features do they share? | Hard positions drive the metric; clustering them may reveal an unaddressed class. | `conversion` or new initiative |
| 3 | Is there a material-rich class where the pre-flight phase does not fire but the solver still spends > 10 M evals on a subgame? | The pre-flight detector (≤3 men, no pawns, no castling) is narrow; a wider recognizer class may exist. | `dfpn` (reopen) or `conversion` |

Status notes: #1 **answered (plan1)** — >98% of child evals are unsolved, ~80% of frame evals sit in threshold-cut frames (`report1.md`); #2 **answered (plan2)** — the outlier tail is one position family (m20–m23), decisive-suite outliers structurally diverse, no pre-flight misses at the roots (`report2.md`); #3 **answered (plan3, NO-GO)** — the searched tree does not simplify: zero `dfpn` frames below 6 men on m22/stress/dec13, 99.93–99.97% of frame-eval mass at ≥9 men, harvestable (≤5-men, pawnless, no-castling) frames exactly 0; the mid-search recognizer direction is closed for the outlier family (`report3.md`).
| 4 | What is the cumulative cost of the `dfpn` frame overhead cluster (~8–15% of wall time in `lean` profiles) at the child-eval granularity? | Frame overhead is not child-evals; separating it lets us count "real" search work vs. control flow. | `lean` |

### Literature targets

Papers and sources to mine, with the question each answers:

| # | Source / Topic | Question for the solver | Status |
|---|----------------|--------------------------|--------|
| 5 | **Selective search / child-level early termination in DF-PN** — Are there published ways to stop evaluating a child once its bound crosses a "hopeless" threshold, without breaking the soundness of the parent proof? | Related to the closed `conversion` #6 threshold-cut-frame pricing, but from the child-granularity side rather than the frame-granularity side. | closed |
| 5 (cont.) | Status note: **answered (plan5, CLOSED)** — mined Henderson 2010 (FDFPN child limit); the surveyed child-level surface collapses into threshold increments (implemented/closed), count-based child limits (structurally equivalent to the closed `conversion` #6/#7 partial-sum lever *and* inverted relative to the threshold-cut churn mass), correlation/heuristic pruning (unsound here without a domain equivalence proof or an evaluator — #6/#7 dependency), and loop-avoidance/terminal-detection (already covered). No class-(b) mechanism; backlog #5 closed with the mapping as the no-go record (`report5.md`, `research_child_termination.md`). | | |
| 6 | **Machine-learned node priors for PNS** — Can a tiny NN or logistic model predict `pn`/`dn` from board features, and has this been shown to reduce nodes in *any* PNS solver? | The `nn` branch (archived under `lean`) closed as oracle-floor; this is a lighter "prior" view (frontier prediction, not full ordering). | open |
| 7 | **Pattern databases / mating-net recognizers in chess/shogi** — Are there small, exact recognizers for forced-mate or forced-extinction patterns that can return decisive outcomes without search? | Could shrink the search tree for common atomic-chess tactical motifs. | open |
| 8 | **Alternative search algorithms: PDS, PN², or bounded variants** — Do Nagai's PDS or PN² have better repetition / deep-conversion scaling than DF-PN+ in recent solver competitions or publications? | The solver is committed to DF-PN+; a measured comparison on the stress case would quantify the lock-in cost. | closed |
| 8 (cont.) | Status note: **answered (plan6, CLOSED)** — mined van den Herik & Winands (PNS-variants chapter) with Pawlewicz & Lew 2007 §4 and the ICGA-2012 survey as corroborators: the only published direct df-pn-vs-PDS comparisons (Atari Go TT-sweep; 286 hard LOA) favor df-pn by 2.6–4.5×, largest in the tree-≫-TT regime matching the stress class; PN² is RAM-contract-fatal (best-first frontier outside the TT) with published memory-collapse; PDS-PN/DFPN-PN carry the same level-2 frontier plus a never-run df-pn comparison (the gap is recorded in the mined source itself); published PDS ignores GHI. No variant in class (b); backlog #8 closed with the 9-row head-to-head evidence table as the lock-in-cost record (`report6.md`, `research_alternative_algorithms.md`). | | |
| 9 | **Job-level / massively parallel PNS (Saffidine 2011, Čížek 2025)** — Already tracked in `conversion` #4 / `lean` #2; this initiative mines only if a new parallel design surfaces that changes the node-count story (not wall time alone). | Hand-off to `conversion` #4 if actionable. | open |
| 15 | **Seesaw-effect reducers: DeepPN (Ishitobi 2015) / Deep df-pn (Zhang 2017)** — Can Deep df-pn's depth-dependent unsolved-leaf pn/dn (`E^(D−depth)`) be mapped onto DF-PN+ in a path-independent, GHI-safe, RAM-bounded way, and does any of its Connect6 evidence transfer to the stress class? | The one unexamined mechanism class; both full texts in-repo (`docs/theory/deep-{pns-2015,dfpn-2017}/`), so a half-session desk exercise. Scope decision: DeepPN 2015 is **not** mined separately (best-first frontier family closed by #8/plan6; RAM = TT only fatal) — only the 2017 df-pn variant. | open — plan9 |

### POC candidates

Rough ideas waiting for a literature justification or a sizing spike. Each
remains *candidate* until a plan scopes it.

| # | Candidate | What it tests | Sizing question | Likely owner |
|---|-----------|---------------|-----------------|--------------|
| 10 | **Node-classification profiler** — Instrument `evaluate_child` and `dfpn` to tag every eval as `OR-decisive`, `AND-refute`, `threshold-cut`, `TT-hit`, `preflight-hit`, `path-rep-draw`. Aggregate per-suite. | Whether the current "unknown" work mass is actually concentrated in one category. | < 1 session of temporary instrumentation | `research` (feeds #1) — **done (plan1)** |
| 11 | **Dynamic epsilon scheduling** — Vary ε or the refinement cap based on depth or rule50 clock, rather than globally. | Deep conversions may need coarser thresholds early and finer later. | 1 session, flag-gated | `conversion` — **done (plan4, NO-GO for scheduling)**: no depth/clock schedule arm beats the global default on any case (best arm 0.0%; the firing arms regress up to +351% and time out m22); the churn mass's threshold response is trajectory chaos with no regional structure (report4.md). Spin-off finding: global ε=0.375 Pareto-improves all four cases (stress −8.7%, m22 −12.7%, dec13 +2.6%, dec10 −14.5%) — handed to `conversion` as a sized default-ε candidate; the 2026-09-11 "global ε inert" claim is formally refuted (ε=0.5 wins stress −37.3% but regresses controls). |
| 12 | **TT eviction priority** — Protect entries from the current PV or high-work subtrees instead of uniform replacement. | Does the current flat replacement discard useful solved entries prematurely? | 1 session, instrumentation only | `lean` — **done (plan7, CLOSED per H2)**: premise corrected twice — priority replacement `(live, solved, work, generation)` already exists in `insert_new` (the "uniform replacement" wording was stale), and the TT-size invariance study cited as pre-weakening does not transfer to the post-plan9 solver (32 MB stress unsolved at 3.33 B evals vs solved at 249 M on 128 MB, deterministic). Phase 0: harmful-class churn (new-unsolved evicting live-solved) 0–0.146% of stores at the 128 MB default, ≤1.21% of probes on evicted keys; Phase 1: both pre-registered policy arms fail — solved-slot immunity (V1) drops 15 M unsolved stores on stress and never terminates (unsolved bounds *are* the working set), steeper work priority (V2) +38.2% stress / +23.7% m22. The incumbent two-slot priority layout is a measured local optimum; eviction-policy work would need a layout change, fixed out of scope by RAM = TT only (`report7.md`, `measurements/plan7/`). |
| 13 | **Frontier-node prediction prior** — Hand-craft or train a fast board-feature regression to predict `pn/dn` ratio for unexpanded children, using it as a pre-sort key before any search. | A lightweight alternative to the full `nn` branch; can be validated by oracle-floor comparison. | 1–2 sessions (data generation + micro-eval) | `lean` or `conversion` |
| 14 | **Per-node confidence-conditioned ε** — At the threshold-computation site, pad more where node-local features (side × depth × static top-2 margin, clamp state) predict the cut child will resolve anyway, pad less where they predict a dead end; tests whether rescue-vs-waste of threshold padding is predictable from features observable at the node — the plan4-mandated *new mechanism* (node-local, not a path schedule). | Whether the rescue outcome of a threshold cut is separable by cheap node-local features, and whether conditioning converts that into a first-outcome `child_evals` win over the best global constant (ε=0.375). | Phase 0 separation study + Phase 1 arm POC, one session | `conversion` — **done (plan8, CLOSED per H2)**: no single feature separates (best ≥5%-mass lift 1.94× < 2×); composite buckets do (OR ∧ shallow, 2.19–2.73×), but the ε mix-shift shows the separation is trajectory-relative (absent at ε=0.125), and all three conditioned-ε arms fail catastrophically (OR pad-more stress +300%, AND pad-less timeout at 2.71 B evals). #14 closed with the separation + arm tables; the confidence feature is non-exploitable for node counts (`report8.md`, `measurements/plan8/`). |

## Closing deliverables (2026-09-21 re-scope)

With the node-count program closed (plan8), the initiative's remaining
purpose is to characterize the **structural floor**: one authoritative
document consolidating the no-go record.

| # | Deliverable | Content | Status |
|---|-------------|---------|--------|
| 16 | **`structural_floor.md`** — the consolidating no-go record | What the solver is locked into, and why, with evidence pointers: the DF-PN+ commitment and plan6's lock-in-cost evidence table; the 1+ε threshold mechanism and the four ε closure legs (no constant, no schedule, no regional structure, no node-local signal — plan4/plan8); GHI first-player-loss shortcut and path-independent TT; RAM = TT only and the best-first/PN² exclusion; ordering and TT-eviction local optima (`lean` plan9, plan7); the seesaw-thread closure (#15/plan9, if it closes); child-level termination surface (#5/plan5). Every claim links to its report or `research_*.md`. | open — plan10 (after plan9) |

## Non-goals

- **Direct production implementation.** Proven ideas are handed off; this
  initiative stays research-only.
- **Re-running closed levers** (plan10/11 cross-clock reuse,
  threshold-cut-frame pricing, EWS/MOPNS ordering, etc.) without a *new*
  hypothesis that the closed result does not already reject.
- **Changing the proof-tree layer, `ProofEvent` protocol, or optimizer
  interface contract** (`docs/spec/optimizer_interface.md`).
- **Wall-time-only optimizations** that do not change node counts. Those
  belong in `lean`.

## Measurement conventions

- POCs use the `m22_white` and stress-case baselines from
  `dfpn/initiative.md` and `lean/initiative.md`:
  - Stress case (`4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`):
    post-plan9 first-outcome **249,480,478 child evals** (128 MB TT,
    reference host).
  - `m22_white`: post-plan9 first-outcome **~14.2 M child evals**.
- New hard cases discovered by the outlier analysis (backlog #2) are
  added to a local fixture file under `measurements/`, not the shared
  benchmark suites, until adopted by an implementation initiative.
- Quick-suite drift is N/A for POCs (code is reverted), but any plan that
  temporarily touches `src/` and claims a non-behavior-changing measurement
  must show `benchmark --suite quick --json --first-outcome` bit-identical
  before and after instrumentation.
- `child_evals` is the primary efficiency metric; wall time is secondary.

## History

- **2026-09-19** — Initiative opened. Backlog seeded from the unsolved
  surfaces visible at the dormancy of `dfpn` (2026-09-17) and the current
  open items in `lean` (#7, #10) and `conversion` (#4). Next plan number
  is **plan1**.
- **2026-09-19** — plan1 (#10 node-classification profiler) done:
  >98% of child evals unsolved, ~80% of frame evals in threshold-cut
  frames (`report1.md`); answers problem #1's partition.
- **2026-09-19** — plan2 (#2 outlier cluster analysis) done: the outlier
  tail is the m20–m23 position family (20 men), decisive-suite outliers
  structurally diverse; recommended generic levers #11–#13
  (`report2.md`).
- **2026-09-19** — plan3 drafted (backlog #3, subgame material-mass
  diagnostic). Supersedes plan2's #11/#12/#13 recommendation for this
  slot: #13 is pre-weakened by `lean` plan9's ordering closure, #12 by
  the `dfpn` TT-invariance study; #11 stays the documented fallback.
  Plan measures whether first-outcome work descends into ≤5-men subgames
  where a plan13-style mid-search recognizer could fire.

- **2026-09-19** — plan3 done (backlog #3, subgame material-mass
  diagnostic): **NO-GO**. Zero `dfpn` frames below 6 men were ever entered
  and 99.93–99.97% of frame-eval mass sits at ≥9 men (m22, stress, dec13);
  harvestable (≤5-men, pawnless, no-castling) frames: 0. The mid-search
  recognizer direction is closed for the outlier family with data
  (`report3.md`). Next plan falls back to #11 (depth-scheduled ε POC),
  with the ε-invariance caveat documented in the plan and report.
- **2026-09-19** — plan4 drafted (backlog #11, scheduled-ε POC; the
  documented plan3 fallback). Phase 0 re-establishes the global-ε inert
  claim with a clean per-case first-outcome sweep (the 2026-09-11 sweep was
  suite-aggregate and dominated by timeout noise); Phase 1 measures four
  env-gated schedule arms (path depth, remaining depth, rule50 clock,
  linear) against the stress gate object with m22/dec13/dec10 controls.
  Gates: GO ≥10% stress win at control parity → `conversion` hand-off;
  NO-GO closes #11 and elevates literature target #5 to next plan.
- **2026-09-19** — plan4 done (backlog #11, scheduled-ε POC): **NO-GO for
  scheduling**. Phase 0 refuted the "global ε inert" claim (ε=0.5 wins the
  stress gate object −37.3% but regresses m22 +12.7% / dec13 +52.4%; ε=0.375
  Pareto-improves all four cases at −8.7% stress, under the 10% GO bar);
  Phase 1 found no depth/clock schedule beats the default (best arm 0.0%,
  two arms never fire, near-root coarsening explodes m22). H1 and H2 both
  rejected: the response is trajectory chaos, not regional structure
  (`report4.md`). #11 closed with data; ε=0.375 spin-off handed to
  `conversion` as a sized note. Next plan: literature target #5
  (child-level early termination in DF-PN).
- **2026-09-20** — plan5 drafted (literature target #5, child-level early
  termination in DF-PN; the plan4 successor). Mining plan per the working
  agreement: Phase 0 is a bounded survey that selects exactly one source
  (clusters: df-pn cutoff variants, partial/selective expansion, solver
  engineering; λ-search excluded as backlog #7 overlap, algorithm swaps as
  #8), Phase 1 extracts it to `research_child_termination.md` mapped to
  the concrete `dfpn` call sites, Phase 2 classifies every mechanism as
  implemented / sound-and-unmeasured / unsound-here / equivalent-to-a-
  closed-lever — with the `conversion` #6 deferral-asymmetry and the
  repetition/TT contracts as hard filters. Gates: OPEN (a class-(b)
  mechanism survives) → sized POC proposal as plan6 candidate; CLOSED →
  close #5 and elevate the next literature target (#6/#7/#8).

- **2026-09-20** — plan5 done (backlog #5, child-level early termination):
  **CLOSED (H0)**. Phase 0 survey (13 sources, `measurements/plan5/`)
  selected Henderson 2010's Focused DFPN child limit as the one qualifying,
  obtainable, unmined source; the extraction maps it onto the verified
  `dfpn` call sites and shows it is structurally equivalent to the closed
  partial-sum sweep lever (subset-Σ stores, deferral asymmetry = the
  source's own Observation 3) *and* inverted relative to the churn mass
  (delays the Σ threshold cuts that carry ~80% of descendant evals instead
  of accelerating them). Every surveyed mechanism classified (a)/(c)/(d)/
  out-of-scope — none lands in (b). Backlog #5 closed with the mapping as
  the no-go record; no POC. Next literature target: #6/#7 (both carry the
  known no-heuristic-component transfer blocker this mining sharpened);
  #8's algorithm-swap question remains separate (PDS's cutoff rule itself
  was classified subsumed here) (`report5.md`).

- **2026-09-20** — plan6 drafted (literature target #8, PDS/PN²
  algorithm-swap scaling; the plan5 successor). Mining plan per the
  working agreement: Phase 0 is a bounded survey over the
  PDS/PN²/hybrid/head-to-head clusters (availability caveat: Nagai 2002
  recorded unobtainable in plan5; the evidence core is expected to come
  from the Winands 2002 author copy, the PNS-variants chapter, and the
  Kishimoto 2012 survey), Phase 1 extracts one source to
  `research_alternative_algorithms.md` with a normalized head-to-head
  evidence table mapped to the `dfpn` call sites, Phase 2 classifies every
  variant as (a) structurally covered / (b) sound-and-unmeasured /
  (c) contract-breaking / (d) evidence-absent-or-against — with the GHI
  first-player-loss shortcut, TT path-independence, and the search CLI's
  RAM = TT only contract as hard filters (expected fatal for PN²'s
  best-first frontier). Gates: OPEN (a class-(b) variant with comparable-
  regime evidence) → sized POC proposal as plan7 candidate; CLOSED →
  close #8 with the evidence table as the lock-in-cost record. #7 was
  demoted (plan3's zero-harvestable-subgames data), #6 demoted (the
  sharpened no-heuristic-component blocker), leaving #8 as the only
  un-weakened open literature target.

- **2026-09-20** — plan6 done (literature target #8, PDS/PN² algorithm-swap
  scaling): **CLOSED (H0)**. Phase 0 obtained and vendored all three
  plan5-identified author-copy sources; selected the van den Herik & Winands
  PNS-variants chapter as the one qualifying source (PDS threshold rules,
  NegaPDS/PDS-PN pseudo-code, PN² construction, LOA head-to-head tables, and
  §7's df-pn-vs-PDS ratios). The evidence table (9 normalized rows) favors
  df-pn in every measured regime — df-pn+1+ε is 4.17–4.46× faster than the
  PDS variants on hard LOA and dominates the Atari Go tree-≫-TT sweep that
  most resembles the stress class — while the comparisons that could overturn
  this (df-pn vs PDS-PN; any repetition-dominated domain) were never run and
  are recorded as gaps. Classification: PDS (d), PN² (c, RAM = TT only
  fatal), PDS-PN/DFPN-PN (c+d), 1+ε df-pn+ (a, implemented). Backlog #8
  closed with the evidence table as the lock-in-cost record; no POC. Next
  plan: remaining targets #6/#7 (both pre-weakened) or POC candidates
  #12/#13 per the CLOSED consequence (`report6.md`).

- **2026-09-20** — plan7 drafted (POC candidate #12, TT eviction/turnover
  measurement with a conditional replacement-priority POC; the plan6 CLOSED
  successor). Measurement-first per the working agreement: Phase 0 is
  counter-only turnover instrumentation (store/probe classes, occupancy) at
  128 MB default + 32 MB pressure on stress/m22/dec13/dec10, with a cheap
  kill gate; Phase 1 (only if harmful churn materializes) is an env-gated
  replacement-variant POC (solved-slot immunity, steeper work priority)
  against the pre-registered ≥10%-stress-FO gate. Premise correction
  documented: the candidate's "uniform replacement" premise is stale —
  `insert_new` already scores (live, solved, work, generation); the open
  question is whether new-unsolved stores evict live-solved entries (or
  high-work unsolved churn) and whether that costs child evals. #12's
  pre-weakening (TT-size invariance study) is addressed by converting the
  indirect inference into a direct eviction measurement.

- **2026-09-20** — plan7 done (POC candidate #12, TT eviction/turnover
  measurement + conditional replacement-priority POC): **CLOSED (H2)**.
  Phase 0 counter-only instrumentation (trajectory-neutral: stress baseline
  249,480,478 reproduced exactly, quick suite bit-identical before/after)
  found harmful-class churn (new-unsolved evicting live-solved) of 0–0.146%
  of stores at the 128 MB default and ≤1.21% of probes on evicted keys, with
  case-dependent occupancy (stress 95.4% full, controls <5%). The pre-registered
  kill gate failed (stress: 0.146% > 0.1% and 95.4% > 80%; 32 MB pressure
  diverges catastrophically), so Phase 1 ran: V1 solved-slot immunity
  (stress timeout, m22 +3400% — dropped unsolved bounds force re-search)
  and V2 steeper work priority (stress +38.2%, m22 +23.7%) both fail the
  ≥10% GO bar with 59/59 quick outcomes preserved; the incumbent priority
  replacement is a measured local optimum within the two-slot layout.
  Spin-off finding: the 2026-09-11 TT-size invariance claim does not
  transfer to the post-plan9 solver (32 MB stress unsolved at 3.33 B evals,
  deterministic) — recorded for future capacity work. #12 closed; no
  hand-off (`report7.md`, `measurements/plan7/`). Next plan: remaining
  targets #6/#7/#13 are all pre-weakened; the initiative should consider
  re-scope or closure of the node-count program (see report7 Next steps).

Per repo convention, every plan ends with the task of writing its
`report<N>.md` in this directory.

- **2026-09-21** — plan8 drafted (POC candidate **#14** opened as a new
  backlog row; the documented successor of plan7, admitted under plan4's
  escape clause because it tests a *new mechanism*: ε conditioned on
  node-local features read at the threshold-computation site, not a path
  schedule). Phase 0 is a counter-only, env-gated separation study —
  per-key rescue/dead labeling of threshold-cut frames with node-local
  features (side, depth, second/best ratio, ε(second)−best, clamp state,
  exiting gap, static top-2 margin) and a pre-registered kill gate (no
  ≥2× rescue-lift on any bucket covering ≥5% of stress-ε=0.375 cut-frame
  eval mass); Phase 1 (only if the gate fails) runs ≤3 env-gated
  conditioned-ε arms against the ε=0.375 baseline with the ≥10% stress /
  +5% controls / zero-quick-flips GO gate.
- **2026-09-21** — plan8 done (POC candidate #14, per-node
  confidence-conditioned ε): **CLOSED per H2**. Phase 0 (counter-only,
  trajectory-neutral: both stress baselines and all controls reproduced
  exactly, quick suite bit-identical, zero log overflow) found no single
  feature separating (best ≥5%-mass lift 1.94× < 2×; plan6's degenerate
  tie-region signal replicated), but composite buckets passed the gate
  (OR ∧ depth 1–4 at 2.42×/8.5% mass), so Phase 1 ran: all three arms
  failed — OR pad-more +300% stress / +76.7% m22, AND pad-less timeout at
  2.71 B evals, quick outcomes flipped via timeouts. Key diagnostic: the
  separation is trajectory-relative (the same buckets collapse to base
  rate at ε=0.125), so node-local confidence conditioning re-instantiates
  plan4's trajectory chaos at node granularity. #14 closed with both
  tables; the ε surface now has four closure legs (no constant, no
  schedule, no regional structure, no exploitable node-local signal)
  (`report8.md`, `measurements/plan8/`). **Recommendation: close the
  initiative's node-count program** per report7's next-steps — all
  backlog rows answered/closed/pre-weakened; note the shallow-OR rescue
  mass for the `conversion` #4 parallel-spike constraint discussion.
- **2026-09-21** — Initiative **re-scoped (pivot, not closure)** per the
  report8 recommendation discussion: the node-count program stays closed,
  but the initiative continues as "characterize the structural floor"
  (new Closing deliverables section, #16 — the consolidating no-go
  document, `structural_floor.md`, planned as plan10). One final cheap
  lever admitted before consolidation: literature target **#15** opened
  (seesaw-effect reducers; plan9 drafted — Deep df-pn 2017 desk mining,
  with DeepPN 2015 explicitly not mined separately).
