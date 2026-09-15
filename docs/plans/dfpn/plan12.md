# Plan 12: KQvK ladder — repetition-semantics resolution + repetition-contaminated work-mass reduction

Initiative: `dfpn` backlog #5 (opened 2026-09-14 from the `egtb` plan1
cross-validation). Prerequisite reading: `initiative.md` (plan5→plan7 twin
history, plan10/plan11 no-gos, plan9 repetition cache), `docs/plans/egtb/report1.md`
("Post-report addendum"), `research_repetition_cache.md`,
`research_ghi.md` §9.

## Problem

`egtb` plan1's cross-validation found KQvK positions the solver cannot
prove. The difficulty ladder (played out from the defect root; 600 s
budget, default mode):

| FEN | optimal (egtb table) | nodes to first outcome |
| --- | -------------------- | ---------------------- |
| `8/2K5/k7/8/8/8/8/4Q3 w - - 0 1` (root) | win in 15 | > 755,986,909 (unproven) |
| `8/1k1K4/8/8/8/8/8/4Q3 w - - 2 2` | win in 13 | 42,682,820 |
| `8/8/2k1K3/8/8/8/8/4Q3 w - - 4 3` | win in 11 | 3,874,308 |
| `8/5K2/8/3k4/8/8/8/4Q3 w - - 6 4` | win in 9 | < 201,482 |

~20× node growth per 2 plies of mate depth — exponential in DTM. Two more
facts pin the class:

- **The unbounded bootstrap is not the culprit.** A depth-limited
  `search_depth(&mut pos, 15)` (bounded AND/OR, 300 s) also fails:
  1,087,258,624 nodes, `ExitReason::Timeout`, no proof.
- **The transposition upper bound is tiny.** A depth-15 proof of a ≤16-ply
  mate with full transposition reuse visits at most ~#positions × depth ≈
  500k × 15 ≈ 7.5M nodes. The solver visits ~150× that.

**Hypothesis (to be confirmed, not assumed):** repetition-dependent results
are deliberately not TT-cached (the first-player-loss GHI shortcut,
post-plan7), and in KQvK the defender's hide-adjacent strategy (kings do
not attack, cannot capture, and are blast-immune — see
`docs/plans/egtb/report1.md` ruleset findings) makes most subtrees
repetition-contaminated. The plan9 repetition cache intercepts draws keyed
by (position hash, ancestor-set context hash); on KQvK the ancestor-context
working set may be far larger than the 1.6k entries measured on the 32-men
stress case, collapsing its hit rate — leaving the search to re-prove
draw chains in every path context.

## Why this is not plan10/plan11 again

Those plans attacked *cross-clock reuse of solved facts* and died on a
DF-PN destabilization (solved-child bounds folding into unsolved parents'
thresholds). This plan attacks a different mass: **re-proved draw chains
and repetition-suppressed stores on shuffle-dominated small spaces**. The
m22_white control failure mode (root dn explosion via folds) has no
obvious analogue here — but the plan's gates explicitly re-test it,
because plan11 proved that "the stress win and the destabilization are one
mechanism" is a real pattern in this codebase.

## Phase 0 — semantics resolution + work-mass diagnostics (no behavior change)

Temporary env-gated instrumentation (`DFPN12_SPIKE=1`), reverted after
measuring, per the initiative's working agreement. Three tasks, in order.

### T0 — reproduce the ladder

Baseline child-evals/nodes/wall for the four ladder FENs (600 s, default
mode) plus the bounded `search_depth(·, 15)` run (300 s). Numbers to
reproduce: the table above; 1,087,258,624 nodes for the bounded run.
Record exit reasons.

### T1 — resolve the semantics question (go/no-go for the whole plan)

The `egtb` spike called the root a "solver defect" based on a depth-16
prover that **ignores path-repetition semantics**. Under the solver's
rules, any position recurring on the current path is a Draw — and a
depth-bounded proof that ignores this can hide cycles: if the attacker's
strategy can be steered back to an ancestor position, the defender forces
a draw and the "win" is fake. This must be settled before any fix is
attempted:

- **T1a — strategy-cycle check.** Load the generated 3-man q-table
  (`docs/plans/egtb/measurements/plan1/egtb3-q.bin`, layout in
  `examples/egtb_gen3/table.rs`; regenerate with `egtb_gen3 --material q`
  if missing). Extract the attacker strategy "at every Win node play a
  winning child (child = Loss); at every Loss node the defender moves".
  Build the reachable state graph from the ladder roots under this
  strategy and detect cycles (DFS). If the greedy strategy cycles, search
  over winning-child choices (prefer larger child dtm = more progressive)
  for a cycle-free winning strategy. A cycle-free winning strategy from
  the root ⇒ the position is a genuine Win under path-repetition
  semantics.
- **T1b — repetition-aware bounded prover.** Cross-check T1a with the
  independent prover from `egtb_gen3/prove.rs` extended by a path-repetition
  cut (state = position + ancestor set along the current line; memo keyed
  by (fen, depth, order-independent ancestor-set hash)). Depth 15–17.
  Feasibility risk: the ancestor-set component can blow up the memo; cap
  it and report the cap. T1a and T1b must agree.

**Gate G1 (go):** a cycle-free winning strategy exists for the root (and
ideally all four ladder positions). Then the solver's failure is a genuine
search deficiency and the plan proceeds to T2/Phase 1.

**No-go:** no cycle-free winning strategy is found up to a documented
search effort. Then the position is (probably) genuinely unprovable under
the solver's repetition semantics — the "defect" dissolves into semantics:
file a correction against `docs/plans/egtb/report1.md`'s addendum, close
this plan with no code changes, and re-rank (remaining lever for the
class: #3).

### T2 — work-mass diagnostics (only on G1 go)

Instrument the root (and the depth-15 bounded run) with temporary
counters:

1. TT store suppression: how many solved results are suppressed by
   repetition dependence (first-player-loss shortcut) per run, and how
   many distinct positions they cover.
2. plan9 repetition cache: hit rate, distinct (hash, context) keys,
   context-set size distribution. Compare against the 32-men stress case's
   1.6k-entry working set.
3. Path-repetition cut frequency: share of `dfpn` frames exiting via
   `path_contains`.
4. Child-eval mass by (OR/AND × resolved-unsolved) and the depth
   distribution of evaluated children.

**Gate G2:** a dominant, cacheable mass exists — operationalized as: at
least one of (1)–(3) accounts for ≥ 50% of the work gap between the
measured ~1.1B nodes and the ~7.5M transposition upper bound, with a
stated mechanism for caching it soundly. If the mass is diffuse or the
dominant cost is none of these, close the plan with the diagnostics
recorded (evidence-based no-go, the plan10/plan11 pattern).

## Phase 1 — implementation (only on G1 ∧ G2, one selected arm)

Phase 0's data selects the arm; the sketches below are pre-registered so
the spike can be compared against them. Soundness sketches are
deliberately conservative: the initiative's lesson (plan5→plan7) is that a
lever that cannot state why it never returns a false decisive outcome is
not implementable.

- **Arm A — context-key reduction for the plan9 repetition cache.** If T2
  shows the cache miss rate is driven by context-set fragmentation (many
  distinct ancestor-set hashes covering the same positions), shrink the
  context key to the cycle-relevant core: the ancestor positions that can
  actually be revisited within the remaining proof horizon (e.g. positions
  on the current path whose repetition the *defender* can force — kings
  and slow pieces only; pawn pushes reset the clock and are
  cycle-breaking). Soundness sketch: two paths with the same
  cycle-relevant ancestor set admit the same repetition threats, so a draw
  proof cached under one is valid under the other; a Win is never cached
  here (plan9 stores draws only), so no false decisive can enter.
  Risk: "cycle-relevant" is easy to get subtly wrong; the T2 context-set
  size distribution tells us whether the reduction is even needed.
- **Arm B — repetition-suppressed store accounting.** If T2 shows the
  dominant mass is TT solved results suppressed by the first-player-loss
  shortcut, extend the plan9 cache to also accept *disproof-side* facts
  (repetition-dependent Draws proven against specific ancestor contexts,
  which plan9 already handles) and measure whether the remaining
  suppression is actually load-bearing. Soundness: no new claim classes —
  this arm only moves existing plan9-proven facts to a better key.
- **Arm C — cycle-free bounded proofs.** If T2 shows neither A nor B
  suffices but T1b's repetition-aware prover solves the ladder cheaply,
  add a depth-limited, repetition-aware proof pre-phase to `solve`
  (bounded by a static small-space detector: occupied-popcount threshold,
  not a material-class table). Soundness: the pre-phase proves Win only
  with explicit cycle-freedom on every line (T1b machinery); anything else
  returns "unknown" and defers to the normal search. This is the largest
  lever and the largest risk; it must not run on ordinary positions (the
  detector gates it), and its results must be provable by the existing
  replay validator semantics.

Whichever arm is selected: implement, then run the full validation below.
If the selected arm fails its gates, close the plan as a measured no-go
with the arm's numbers (the plan10/plan11 pattern) — do not iterate into a
second arm inside the same session.

## Goal / success criteria

- Ladder root: proven Win within 60 s and ≤ 20M child evals (from
  >756M/unproven); the other three ladder steps improve ≥ 10× in
  child-evals to first outcome.
- No regression: `m22_white` first-outcome within 1.1× baseline child
  evals; stress case (`4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - -
  0 21`) first-outcome within 1.05×; quick suite drift per the protocol.
- Soundness: no false decisive anywhere (gates below).

## Validation

1. `cargo fmt`, `cargo clippy --all-targets`, `cargo test` (fast gate).
2. `cargo test --release --test test_repetition -- --include-ignored` —
   the report7 cyclic-rook safe-area regressions must pass unchanged.
3. Drift protocol: `benchmark --suite quick --json --first-outcome`
   byte-identical before vs. after except on the ladder positions (not in
   the quick suite — so byte-identical, full stop, unless the selected arm
   legitimately changes a quick case; then the plan's delta list applies
   and the move-order suite is the regression check).
4. KQvK oracle check: with the Phase 1 binary, sample ≥ 200 KQvK positions
   stratified by the 3-man q-table outcome and verify the solver never
   returns a decisive outcome contradicting the table, and every proven
   Win survives the replay validator's structural rules on its PV.
5. Deterministic budget contract: `set_child_eval_budget` /
   `ExitReason::BudgetExhausted` semantics unchanged (unit tests pass
   unchanged).
6. `make test-full` before closing (search change ⇒ full tier per
   AGENTS.md).

## Constraints

- No re-proposal of the plan5 twin/simulation machinery (initiative
  non-goal); any reuse-shape change must state its relation to report7's
  soundness holes and research_ghi §9.
- rule50 stays in the Zobrist key.
- Phase 0 instrumentation is temporary and reverted before Phase 1 lands
  (two separate builds; the Phase 1 diff must be clean of spike code).
- Files under the AGENTS.md sizing rules; new modules go in
  `src/search/dfpn/`.

## Final task

Write `docs/plans/dfpn/report12.md`: T0–T2 diagnostics with raw numbers,
the G1/G2 gate outcomes, the selected arm and its measured effect on the
ladder and controls, soundness argument as implemented, deviations,
problems, missing tests, and next steps (re-rank the backlog; if the
ladder is fixed, propose adding the root to `tests/fixtures/` as a
regression case — a fixture-file change is a plan-level decision). Update
the initiative's History and backlog row.
