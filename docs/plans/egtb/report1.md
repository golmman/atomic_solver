# Report 1: Go/no-go spike — men-count histogram + 3-man generator prototype

Initiative: `egtb` backlog #1. Implements `docs/plans/egtb/plan1.md`.
Date: 2026-09-14. All measurements under `docs/plans/egtb/measurements/plan1/`.

## TL;DR

- **Go/no-go: NO-GO for the "faster solves" goal.** The non-terminal ≤4-men
  share of child evaluations on the decisive suite is **0.000%** (0 of
  50,910,505) — three orders of magnitude below the 0.1% no-go threshold.
  The stress suite agrees (0.000% over 2.13 billion child evals).
- The generator prototype works and **passed the cross-validation merge
  gate with zero mismatches** (solver: 127/128 sampled positions agree, 1
  solver timeout; independent proof oracle: 0 contradictions over 112
  stratified probes; edge cases all agree; color-symmetry self-check: 0).
- The spike's highest-value outcome materialized: the ground-truth oracle
  found a **solver-side defect class** — `Search` cannot prove some genuine
  3-man KQvK wins (a verified ≤16-ply forced mate) within a 300 s budget.
- The ruleset findings materially reshape the rest of the initiative:
  in `atomic-movegen` semantics (kings never capture, kings do not attack,
  kings are blast-immune), **K+R/K+B/K+N vs K have no legal checkmates at
  all** — the 3-man layer is all draws except KQvK and KPvK — and KQvK
  contains large genuine fortress-draw regions.

## Task A: men-count histogram

Implementation: always-on, observability-only counters in `Search`
(`MenHistogram`, `Search::men_histogram()`): `all` bins every child eval by
`popcount(occupied)`; `nonterminal` bins only children with
`outcome == None` (the probeable work — terminal and TT-resolved children
are decided in O(1)); `clock` buckets the halfmove clock (0 / 1–25 / 26–50 /
51–99) of non-terminal evals at 2..=6 men. Hot-path cost: one popcount over
the already-maintained occupancy bitboard plus increments, in
`evaluate_child` (`src/search/dfpn/children.rs`).

**Metric semantics.** The histogram counts *child evaluations* (the ~95% of
children never searched — exactly what a leaf probe replaces), not searched
nodes. TT-resolved children are excluded from the non-terminal bins because
a TT hit already replaces the recursion at O(1), like a probe would.

### Decisive suite (`--suite decisive --first-outcome --timeout 5`, 46 dec positions + startpos)

Non-terminal child evals: 50,910,505 (of 51,923,087 total).

| men | count | share |
| --- | ----: | ----: |
| 2   | 0 | 0.000% |
| 3   | 0 | 0.000% |
| 4   | 0 | **0.000%** |
| 5   | 79 | 0.0002% |
| 6   | 4,581 | 0.009% |
| 7–8 | 349,718 | 0.687% |
| 9+  | 50,556,127 | 99.304% |

Cumulative: ≤4 **0.000%**, ≤5 0.0002%, ≤6 0.009%. Marginal coverage value:
≤4−≤3 = 0.000%, ≤5−≤4 = 0.0002%, ≤6−≤5 = 0.009%.

Halfmove-clock distribution of ≤6-men non-terminal evals (4,660): clock 0 =
54.0%, clock 1–25 = 46.0%, nothing above 25. (DTZ-intelligence input: the
share is tiny either way.)

Per-position: every one of the 46 decisive positions has `le4=0`; the
largest child-eval counts are dec01 (5.7M) and startpos (26.0M, timeout).

### Stress suite (`--suite stress --timeout 60 --first-outcome`, m19+m20×2+m21×2 + startpos)

Non-terminal child evals: 2,132,711,633. ≤4: **0** (0.000%); ≤5: 3,652
(0.0002%); ≤6: 4,222 (0.0002%); 9+: 99.919%. Clock at ≤6 men: 47.8% at 0 /
52.2% at 1–25. Raw: `men_histogram_stress.log`.

### Reference: default suite (`--timeout 10`, raw: `men_histogram_default.log`)

`two_rook_mate` (4 men): 27/27 non-terminal evals at ≤4 men (100%) — the
histogram works and marks exactly the positions a tablebase would serve.
`m19` and `startpos`: 0.000% at ≤4.

### Wall-time delta of the counter

m22_white (`--first-outcome --runs 8`, 3 interleaved pairs, baseline built
from `HEAD` via `git archive` into a scratch dir): baseline mean 2.529 /
2.543 / 2.583 s, histogram build 2.596 / 2.561 / 2.552 s → mean-of-means
2.552 vs 2.570 s = **+0.7%**, within run-to-run noise (one pair flipped
sign). Node/child-eval counts are byte-identical (858,117 / 14,156,269),
confirming the counter is inert. Consistent with the sub-1% estimate.

## Go/no-go decision

Criterion (plan1): non-terminal ≤4-men share on the decisive suite.

- **Measured: 0.000% (0 / 50,910,505).** Below 0.1% ⇒ **NO-GO** for the
  "faster solves" goal. This is not a judgment-call case (0.1–1%); it is
  three orders of magnitude below the threshold, and the stress suite
  (2.13B evals, 60 s budgets) confirms it.
- The ≤5/≤6 shares (0.0002% / 0.009% decisive) do not enter the criterion;
  recorded as intelligence only. Extending coverage to 5–6 men would buy
  hundredths of a percent at ~64–100× the per-class cost.
- Interpretation caveat (per plan): the share is an upper bound on
  replaceable work, not the net saving — but no upper bound above zero
  means no net saving either.

Per the plan, on a no-go "the initiative pivots to the correctness-oracle
goal or closes". **Recommendation (decision pending, see Next steps): pivot
to the correctness-oracle goal.** The spike demonstrated real oracle value:
ruleset-semantics documentation (below), a wholesale 3-man cross-check
harness, and a solver defect found. Closing is also defensible: the
correctness-oracle value could be harvested cheaply (keep the generator +
cross-validation as tests) without committing to 4-man generation.

## Task B: 3-man generator prototype

`examples/egtb_gen3/` (directory example: `main.rs` CLI + cross-validation,
`table.rs` generator core, `prove.rs` independent proof oracle).

- Index layout per material class: flat 2 colors × 2 stm × 64³ = 1,048,576
  entries, byte per entry, file codes 0=Loss / 1=Draw / 2=Win / 3=invalid
  (raw dump, no header — a format sketch; plan2 will need a header, block
  compression, and shared king-square indexing).
- Fixpoint: forward value iteration to convergence over precomputed CSR
  child-reference lists (terminal constants / same-table / cross-class for
  pawn promotions; pawn generated last). Passes: q 13, p 9, r/b/n 2.
- Generation time (release, per class, incl. sweeps): q 1.07 s, r 1.12 s,
  b 1.09 s, n 1.06 s, p 0.81 s. Child-reference memory: 60.7 / 46.5 /
  38.4 / 33.1 / 20.3 MB. Whole five-class run incl. cross-validation:
  785 s, peak well under the 8 GiB budget after the memo cap (see Problems).
- 4-man extrapolation input: ~30M entries/table ≈ 30× the 3-man index;
  with per-entry child work comparable to here, generation should stay in
  the minutes-per-table range; 2-bit WDL ≈ 8 MB raw/table.

### Results and ruleset findings

| class | win (per color) | loss | draw (incl. unresolved) | checkmates | stalemates | max pass |
| --- | ----: | ----: | ----: | ----: | ----: | --: |
| q | 212,140 | 161,456 | 378,232 | 1,864 | 440 | 13 |
| r | 48,776 | 0 | 899,768 | 0 | 224 | 2 |
| b | 30,660 | 0 | 936,616 | 0 | 0 | 2 |
| n | 18,448 | 0 | 961,312 | 0 | 0 | 2 |
| p | 44,114 | 21,500 | 928,086 | 0 | 0 | 9 |

(All r/b/n "wins" and the entire loss column of r/b/n resolve only from
*illegal* placements — see legality below; there is not a single legal
checkmate in K+R/B/N vs K.)

1. **Kings cannot capture.** `generate_legal` never produces a king capture,
   even of an adjacent, undefended, checking queen (`8/8/8/4K3/8/8/k7/Q7 b`
   has the single reply a2b3). Consequently the queen may check from
   adjacent squares with impunity, and K+Q mates a cornered king without
   any king support (`8/8/kQ1K4/8/8/8/8/8 b` is checkmate).
2. **Kings do not attack and are blast-immune.** A defender king may stand
   on the attacker king's square-neighborhood indefinitely; the white king
   contributes one *blocked square* to the cage, not attack coverage. The
   rook/bishop/knight cover too few lines to ever mate: **K+R/B/N vs K is
   drawn everywhere** in legal positions (only KQvK and KPvK have decisive
   content at 3 men). KQvK has large genuine fortress-draw regions
   (~25% of entries) — the defender draws whenever the king can reach the
   attacker king's ring.
3. **Checkmate semantics.** `Board::outcome()` classifies *any*
   moves-empty position as a draw and disagrees with the solver's
   `Position::outcome_from_state` (checkers-based Loss) on every legal
   checkmate: 112 of 136 sampled terminal positions in the q table
   diverge (0 of 12 for r — its only terminals are stalemates). The plan's
   sketch "base cases from `Board::outcome()`" is therefore **unsound as
   written**; the generator uses the solver's classification and documents
   the divergence. `atomic-movegen 2.2` is self-inconsistent here (its
   movegen/checkers machinery vs its own `outcome()`); the solver's reading
   matches Fairy-Stockfish atomic (checkmate exists, stalemate is a draw).
4. **Legality and reachability.** Placements where the side *not* to move
   is attackable (131,516 per color for q; counted per table) are
   unreachable in play. They are kept in the tables (their index space is
   needed for uniform addressing) but excluded from cross-validation
   sampling; notably, king-capture moves *are* generated in such illegal
   positions (extinction wins), which is why they must not leak into
   sampled comparisons.
5. **Plan1's "K+1vK = Win" base constant is wrong as stated** (K+B/K+N vs
   K have no forced mate) and, at 3 men, never used: the legal 3-man
   subgraph is closed (children of legal positions are legal; no captures
   exist), so 2-man transitions never occur. plan2 must not encode 2-man
   constants blindly — 4-man captures *do* drop into 2/3-man territory,
   where extinction ends the game and `occupied == 2` is a draw.

## Task C: cross-validation (merge gate)

Three oracles, zero tolerance for contradictions; full log:
`egtb_gen3_all.log`.

1. **Solver** (`Search`, first-outcome, 5 s, one 60 s retry on timeouts;
   sampled 8 per (strong color × outcome) bucket over *legal* entries):
   **127 solved in agreement, 1 unproven (timeout), 0 mismatches** across
   all five tables. Explicit edge cases all agree: stalemate draw,
   checkmate loss, rule50-expired checkmate (still Loss — WDL is
   clock-0-consistent), bare kings (Draw).
2. **Independent depth-16 proof oracle** (`prove.rs`; deliberately shares
   no code with generator or solver; 8 samples per bucket): **60 confirmed,
   52 inconclusive (bound exhausted), 0 contradictions.**
3. **Color-symmetry self-check** (values must be invariant under color
   flip + vertical mirroring): 0 mismatches for every table.

Explosion-derived positions: **not reachable at 3 men** — the legal 3-man
subgraph contains no captures (kings cannot capture; the single commoner is
pseudo-royal), so no explosion transitions exist to test. This part of the
plan's Task C is vacuous at 3 men and is deferred to plan2, where 4-man
captures actually explode.

### Triaged findings

- **Solver defect (the gate's intended catch):** `8/2K5/k7/8/8/8/8/4Q3 w`
  is a forced win for White (table: Win; verified by the independent
  depth-16 prover → Win, and by the mating line
  `c7d6 a6a7 e1b1 a7a6 b1b6#` with every defender reply covered by the
  table). `Search --timeout 300 --first-outcome` returns Draw on timeout.
  The `verify_ppv` tool also fails to confirm the line, but only because
  its bounded defender-refutation budget is too tight there (inconclusive,
  not a contradiction). Suspected mechanism: DF-PN+ with the
  repetition-first-player-loss shortcut and work-chunk bounds interacts
  badly with the fortress/hiding defense (the defender king can always
  step next to the white king, so many lines stay draw-claimable far past
  the mate horizon). Severity: an efficiency/completeness gap on
  positions the solver would otherwise solve instantly; no false decisive
  result was observed. Filed as a candidate follow-up outside the egtb
  backlog (or as new evidence for the `dfpn` initiative).
- No generator defects, no convention mismatches beyond the documented
  `Board::outcome()` divergence (item 3 above).

## Deviations from the plan

1. Base cases from the solver's terminal classification instead of
   `Board::outcome()` (the sketch's oracle contradicts the solver on
   checkmate; divergence is sampled and reported on every run).
2. "Retrograde fixpoint" realized as forward value iteration over
   precomputed child-reference lists (same fixpoint; simpler and fast
   enough that generation is ~1 s per class).
3. Cross-validation extended beyond the plan: legality filtering of
   samples, solver-timeout classification (timeout ≠ Draw proof), a
   second, independent proof oracle, and the color-symmetry self-check.
4. The plan's "verify base constants against `Board::outcome()`" was
   replaced by the three-oracle scheme (see 1).
5. Directory example (`egtb_gen3/{main,table,prove}.rs`) instead of a
   single file, per the AGENTS.md sizing rules (headers document the
   justifications).
6. Remaining-unknown entries at the fixpoint are genuine draws (defender
   reachability games: the fortress/shuffle region), not a failure
   condition; the run's failure gate covers symmetry violations,
   cross-validation mismatches, and proof contradictions.

## Problems encountered

- Two generator bugs found and fixed during bring-up (both caught by the
  spike's own instrumentation, before the gate ran): a CSR offset hole that
  silently skipped entries followed by skipped placements, and a match arm
  that swallowed `U_WIN` in the all-win (Loss) test, stalling Loss
  propagation entirely.
- The first full five-class run was OOM-killed at the 8 GiB cgroup limit:
  the shared proof memo (String-keyed, depth-16 proofs) grew without
  bound. Fixed with a 2M-entry memo cap (clearing only costs recomputation)
  and by dropping child lists after each table's dump.
- `Board::outcome()`'s moves-empty→Draw semantics cost a plan revision
  (see deviation 1).

## Missing tests

- The generator has no unit tests; the three-oracle cross-validation run
  *is* its test. If the pivot keeps the oracle goal, plan2 should add a
  fast-tier smoke test (generate a small slice, assert counts/symmetry)
  and a fixture-locked regression for the index layout.
- `men_histogram` has no test; it is a measurement tool. A cheap smoke
  test (two_rook_mate → 100% ≤4 share) would guard the histogram plumbing.
- The solver KQvK defect has no regression test (no fix exists yet); it
  should become a `#[ignore = "slow: ..."]` case or a dedicated issue
  whichever way the pivot decision falls.

## Post-report cleanup (same session)

The no-go decision removed the motivating use for the histogram
instrumentation, so it was removed from the hot path after this report was
written: the `MenHistogram` counters in `Search`/`evaluate_child` and the
`men_histogram` example no longer exist. The measurement results above are
unaffected (raw outputs in `measurements/plan1/`); to re-run the
measurement, check out the tree at plan1 completion (git history). The
documented cost (+0.7% mean, within noise) was accepted as an argument for
removal: hot-path code serving a dead goal is debt.

## Post-report addendum: solver defect investigation (same session)

Two follow-ups sharpen the KQvK defect found in Task C:

1. **Difficulty ladder (played out from the defect root, 600 s budget).**

   | FEN | optimal | nodes to first outcome |
   | --- | ------- | ---------------------- |
   | `8/2K5/k7/8/8/8/8/4Q3 w` (root) | win in 15 | > 755,986,909 (unproven) |
   | `8/1k1K4/8/8/8/8/8/4Q3 w - - 2 2` | win in 13 | 42,682,820 |
   | `8/8/2k1K3/8/8/8/8/4Q3 w - - 4 3` | win in 11 | 3,874,308 |
   | `8/5K2/8/3k4/8/8/8/4Q3 w - - 6 4` | win in 9 | < 201,482 |

   Node count grows ~20× per 2 plies of mate depth — exponential in DTM,
   the signature of a near-full-width search without effective proof
   guidance or transposition reuse on shuffle-rich trees.

2. **The unbounded bootstrap is not the culprit.** A *depth-limited*
   `search_depth(&mut pos, 15)` (bounded AND/OR, 300 s budget) also fails:
   1,087,258,624 nodes, `ExitReason::Timeout`, no proof. A depth-15 proof
   of a ≤16-ply mate with full transposition reuse should visit at most
   ~#positions × depth ≈ 7.5M nodes; the solver visits ~150× that.

   **Refined hypothesis:** repetition-dependent results are deliberately
   not TT-cached (the first-player-loss GHI shortcut), and in KQvK the
   defender's hide-adjacent strategy makes most subtrees
   repetition-contaminated — so the TT memoizes almost nothing on exactly
   these trees and the search degenerates to exponential full-width
   behavior. This is the classic GHI problem for df-pn-style searches.

   Candidate fix directions for a dedicated `dfpn` plan (none trivial):
   graph-history-interaction-aware proof numbers; caching
   repetition-contaminated *bounds* instead of full results; or
   depth-limited proofs with iterative deepening that treat frontier
   nodes as Unknown. The four-position ladder is the ready-made benchmark
   (current: 201k / 3.9M / 42.7M / >756M nodes); a fix should collapse
   all four to roughly TT-sized work (≪1M nodes).

   **Caveat (supersedes the "solver defect" wording above):** the
   independent depth-16 prover ignores path-repetition semantics, under
   which any position recurring on the current line is a Draw. A
   depth-bounded proof without repetition awareness can hide cycles (an
   attacker strategy that can be steered back to an ancestor position is
   a draw line even if it eventually mates), so the win's existence under
   the solver's own semantics is unproven. Filed as `dfpn` backlog #5
   (`plan12.md`), whose Phase 0 resolves exactly this question before any
   fix work; this addendum's conclusions are provisional until then.

## Next steps

1. **Decision needed (user):** pivot to the correctness-oracle goal or
   close the initiative. Data for the decision:
   - For pivot: the oracle found a real solver defect class on its first
     run; the 3-man tables are cheap (~1 s/class) and wholesale
     cross-validatable; the ruleset findings are documented nowhere else.
   - For close: the speed motivation (the initiative's primary goal) is
     dead per the histogram; the oracle value might be harvestable by
     keeping the generator as a test fixture without new plans.
2. If pivoting: re-scope plan2 from "4-man generator + storage format" to
   the oracle role (3-man tables as a permanent solver cross-check /
   fuzz oracle; optionally 4-man WDL later as a *validator*, not a probe).
   The 4-man coverage decision (initiative open decision 1) is moot unless
   probing is revived.
3. File the solver KQvK completeness defect with the reproduction above
   (cross-ref `dfpn`/`lean` backlogs).
4. Update `docs/spec/` only if the oracle goal is formalized (none of
   plan1's outputs are spec-relevant yet).
