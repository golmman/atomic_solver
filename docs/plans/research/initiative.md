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

Status notes: #1 **answered (plan1)** — >98% of child evals are unsolved, ~80% of frame evals sit in threshold-cut frames (`report1.md`); #2 **answered (plan2)** — the outlier tail is one position family (m20–m23), decisive-suite outliers structurally diverse, no pre-flight misses at the roots (`report2.md`); #3 **claimed by plan3** (subgame material-mass diagnostic, `plan3.md`).
| 4 | What is the cumulative cost of the `dfpn` frame overhead cluster (~8–15% of wall time in `lean` profiles) at the child-eval granularity? | Frame overhead is not child-evals; separating it lets us count "real" search work vs. control flow. | `lean` |

### Literature targets

Papers and sources to mine, with the question each answers:

| # | Source / Topic | Question for the solver | Status |
|---|----------------|--------------------------|--------|
| 5 | **Selective search / child-level early termination in DF-PN** — Are there published ways to stop evaluating a child once its bound crosses a "hopeless" threshold, without breaking the soundness of the parent proof? | Related to the closed `conversion` #6 threshold-cut-frame pricing, but from the child-granularity side rather than the frame-granularity side. | open |
| 6 | **Machine-learned node priors for PNS** — Can a tiny NN or logistic model predict `pn`/`dn` from board features, and has this been shown to reduce nodes in *any* PNS solver? | The `nn` branch (archived under `lean`) closed as oracle-floor; this is a lighter "prior" view (frontier prediction, not full ordering). | open |
| 7 | **Pattern databases / mating-net recognizers in chess/shogi** — Are there small, exact recognizers for forced-mate or forced-extinction patterns that can return decisive outcomes without search? | Could shrink the search tree for common atomic-chess tactical motifs. | open |
| 8 | **Alternative search algorithms: PDS, PN², or bounded variants** — Do Nagai's PDS or PN² have better repetition / deep-conversion scaling than DF-PN+ in recent solver competitions or publications? | The solver is committed to DF-PN+; a measured comparison on the stress case would quantify the lock-in cost. | open |
| 9 | **Job-level / massively parallel PNS (Saffidine 2011, Čížek 2025)** — Already tracked in `conversion` #4 / `lean` #2; this initiative mines only if a new parallel design surfaces that changes the node-count story (not wall time alone). | Hand-off to `conversion` #4 if actionable. | open |

### POC candidates

Rough ideas waiting for a literature justification or a sizing spike. Each
remains *candidate* until a plan scopes it.

| # | Candidate | What it tests | Sizing question | Likely owner |
|---|-----------|---------------|-----------------|--------------|
| 10 | **Node-classification profiler** — Instrument `evaluate_child` and `dfpn` to tag every eval as `OR-decisive`, `AND-refute`, `threshold-cut`, `TT-hit`, `preflight-hit`, `path-rep-draw`. Aggregate per-suite. | Whether the current "unknown" work mass is actually concentrated in one category. | < 1 session of temporary instrumentation | `research` (feeds #1) — **done (plan1)** |
| 11 | **Dynamic epsilon scheduling** — Vary ε or the refinement cap based on depth or rule50 clock, rather than globally. | Deep conversions may need coarser thresholds early and finer later. | 1 session, flag-gated | `conversion` |
| 12 | **TT eviction priority** — Protect entries from the current PV or high-work subtrees instead of uniform replacement. | Does the current flat replacement discard useful solved entries prematurely? | 1 session, instrumentation only | `lean` |
| 13 | **Frontier-node prediction prior** — Hand-craft or train a fast board-feature regression to predict `pn/dn` ratio for unexpanded children, using it as a pre-sort key before any search. | A lightweight alternative to the full `nn` branch; can be validated by oracle-floor comparison. | 1–2 sessions (data generation + micro-eval) | `lean` or `conversion` |

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

Per repo convention, every plan ends with the task of writing its
`report<N>.md` in this directory.
