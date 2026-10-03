# Plan 9: Ordering-guidance seed from Fairy-Stockfish (backlog #2, lever 2a)

Initiative: `conversion` backlog #2, scoped to the **ordering-guidance lever
(2a) only**. The verification lever (2b) is closed — measured no-go
(`report2.md`); this plan must not reopen it (no verification loop, no
PPV machinery in the mechanism). Prerequisite reading: `report2.md` (the
engine protocol and the measured cooperative-horizon failure this lever
must not inherit), `lean/initiative.md` Motivation (the oracle-floor
measurement this lever is ranked against), `conversion/report4.md`
(the measured-empty AND-side ordering surface), and `conversion/initiative.md`
backlog #2 row (the recorded weak premise this plan exists to test).

## Goal

Measure whether a **one-shot ordering seed** from Fairy-Stockfish can reduce
`child_evals` / wall time on the deep-conversion position class: query the
engine once before the solve, seed the candidate line's moves into the
ordering layer (history/killer-class bonuses only), then run the unmodified
solver. Zero soundness risk by construction (ordering only — no engine value
ever enters the TT, thresholds, or any decisive claim), but the *economics*
are unproven and the recorded premise is weak.

This is a **Phase-0-only plan** (plan2/plan4/plan6 precedent): a
zero-production-code measurement spike with a hard, pre-registered go/no-go.
On a go, the mechanism opens as `plan10.md`; on a no-go, backlog #2 closes
entirely and the engine-hybrid direction is retired.

## The premise, stated sharply (and one fresh measurement)

Backlog #2 records that the engine's stress root move is unstable across
budgets (`g3g4`@MultiPV-1/10M vs `b1b8`@MultiPV-1/100M vs `d6e5`@MultiPV-3/10M)
and diverges from the solver's own `d6e5`-class PVs. A probe run during plan
drafting (read-only, existing binary, 2026-09-28) makes the weakness concrete:

```
$ target/release/examples/move_order_debug '<STRESS_FEN>'
rank 1  d6e5   total 5180   ← the solver's decisive-class root move, already first
rank 22 g3g4   total 1180   ← engine's MultiPV-1/10M root move
rank 25 b1b8   total 1050   ← engine's MultiPV-1/100M root move
```

Re-run this probe at execution time (the binary may have drifted since
plan8's scoring changes); if it reproduces, the headroom question is already
half-answered: where the engine **disagrees**, seeding it first demotes the
solver's rank-0 choice; where the engine **agrees** (`d6e5`@MultiPV-3/10M),
seeding is a no-op. The lever can only win in the window where the engine
picks the decisive root move that the solver currently ranks *below rank 0*
— and then only by as much work as the solver wastes before reaching it.
Phase 0 measures exactly that window.

## Inherited facts (from report2 — do not re-derive)

- Submodule `libs/Fairy-Stockfish` pinned `226c7f18c854372d5612be2a7d7f14449ae5a239`
  (fairy_sf_14-322). The container is **aarch64**: build with
  `make -C libs/Fairy-Stockfish/src build ARCH=armv8 COMP=gcc -j4`
  (`x86-64*` targets fail). The binary is `src/stockfish`, **not**
  `fairy-stockfish`. Classical eval only (NNUE embedding off); NNUE stays out
  of scope — report2 found the 2b failure structural, not eval-strength-bound.
- UCI protocol (identical for every query):
  `setoption name UCI_Variant value atomic`, `Threads value 1`,
  `Hash value 256`, `MultiPV value <K>`, `position fen <FEN>`,
  `go nodes <N>`; parse the last `info … multipv k … pv …` line per rank;
  stop at `bestmove`. At fixed nodes/Threads=1/Hash the output is
  deterministic — verify once by running the stress query twice.
- Inherited engine walls: 0.8 s @1M, 6.6 s @10M, 68.8 s @100M (stress).
- Current HEAD baseline (post-plan6 step 0, exact match to post-plan9):
  stress FO 249,480,478 child evals / 13,907,467 nodes / ~53.8 s wall
  (reference-host figure; this container is ~10% slower — compare
  child_evals, re-measure wall at step 0).

## Scope decisions (recorded, one-lever-per-plan)

1. **Ordering only.** The seed touches `sort_moves` inputs (history /
   killer-class state) at solve start. No TT writes, no threshold
   arithmetic, no proof events, no outcome-dependent behavior. The plan10
   hazard test passes trivially (no solved facts involved).
2. **One engine query per run, before the solve.** The search never spawns
   or depends on an external process (plan2's recorded scope decision is
   inherited verbatim): the engine client lives in an `examples/sf_guide`
   binary if the mechanism is built; the search CLI would only gain an
   opt-in `--guide-moves "<uci>"` flag taking a pre-fetched line.
3. **Single line, K=1 primary.** No MultiPV ladder in the mechanism
   (report2: the ladder only adds cost when candidates fail). If bar (a)
   below can only be met at K=3, that is recorded as a finding and the
   mechanism design may reconsider — but the *bar* is evaluated on the
   recorded K ∈ {1, 3} query data either way.
4. **No verification fallback loop.** If a seeded run fails to solve within
   budget, the fallback is simply the unguided solve (flag-off behavior);
   no PPV verification step is added.
5. **Position-mapping rule.** A seed line is only applied over its legal
   prefix: moves are replayed from the root FEN; at the first illegal move
   the seed stops (engine and solver movegen agree per report2's perft
   sanity check, but the guard is mandatory — the seed is data, not proof).

## Phase 0 — measurement spike (no production code changes)

### Step 0 — re-baseline at HEAD

- stress FO and default mode (`--timeout 120` each), m22 FO
  (`--timeout 30`), plus three dec controls (pick the hardest dec cases the
  suite metadata suggests; `--timeout 30`). Record `child_evals` (the
  deterministic metric), nodes, wall. Generous timeouts so no run is
  resource-cut; the numbers become this plan's comparison table.
- Re-run the `move_order_debug` root probe (stress + m22) and record the
  root ranks of the solver's baseline-PV first move and, later, the engine's
  candidates.

### Step 1 — engine query battery (no solver code)

Rebuild the engine if `libs/Fairy-Stockfish/src/stockfish` is absent; record
the build line and sha256. Query stress + m22 + the three dec controls at
nodes ∈ {1M, 10M} (add 100M on stress only if the 10M line still disagrees
with the solver's decisive root move — 100M is 68.8 s of engine wall).
K=1 primary; one K=3 run on stress at 10M for the coincidence table
(report2 already recorded it; re-confirm cheaply). Determinism check: run
the stress@10M query twice, PVs must match. Output: per (case, budget) the
full PV and engine wall time.

### Step 2 — coincidence analysis (no solver code)

For every (case, budget) line from step 1:

- Does the engine's MultiPV-1 root move equal the solver's decisive root
  move (from the step-0 baseline PV / outcome)? The stress case is the
  target class; the dec controls qualify the easy class.
- Root rank of the engine's root move under the solver's own static
  ordering (step 0 probe).
- Line-prefix overlap: length of the common prefix between the engine line
  and the solver's baseline PV line (both as UCI move sequences from the
  root FEN).

### Step 3 — root-work distribution spike (env-gated, reverted after)

Temporary, `CONV9_SPIKE=1`-gated instrumentation only: attribute all child
evals in a solve to the root-frame iteration (root move) they occur under,
recording per-root-move cumulative evals and first-expansion order. Run on
stress FO and default mode only. Deliverables:

- **W** = share of total child evals spent under root moves explored before
  the decisive root move's first proof completes (the "wasted pre-decisive
  root work" — the entire theoretical prize of root-ordering guidance).
- The decisive root move's rank in the first root sweep and the count of
  root moves expanded before it.

Verify non-perturbation by reproducing the step-0 baselines bit-for-bit
(spike off = byte-identical stdout; spike on = identical evals/PV, counters
additive only). Then fully revert; the working tree must be byte-identical
to pre-plan9 (plan4/plan6 contract).

### Step 4 — pre-registered go/no-go

All three bars are conjunctive; any fail = no-go, backlog #2 closes
entirely (both levers retired), engine-hybrid marked as exhausted in the
initiative.

- **(a) Coincidence.** On the stress case, some engine configuration
  (K ∈ {1,3}, nodes ∈ {1M,10M}) has MultiPV-1 root move == the solver's
  decisive root move, **and** that move's rank under the solver's own
  ordering is ≥ 2 (otherwise the seed is a no-op and there is nothing to
  win). On the dec controls, no requirement — the easy class was already
  shown engine-cost-bound by report2.
- **(b) Headroom.** W ≥ 10% of stress-FO child evals (the initiative's #7
  GO bar). The ceiling of *any* ordering seed is bounded by W plus the
  lean oracle floor's ≤ 9.4% OR-side recoverable share; if W is thin, the
  mechanism cannot pay its own engine-query cost.
- **(c) Projection.** Using step-3 numbers: projected guided stress-FO
  total (engine wall at the bar-(a) configuration + solve evals re-priced
  with the decisive root move promoted to rank 0, scaling W by the measured
  rank gap) ≤ 50% of the unguided baseline — the same bar plan2 used — and
  no dec control projected to regress > 5%. State the projection formula in
  the report so it is auditable; a projection that only works with
  unmeasured cross-node history effects (the seed also fires at interior
  nodes, where its sign is unknown) must be marked as such and fails the
  bar if the unmeasured term is load-bearing.

### Step 5 — closeout

Revert all spike code; `make test` green; write `report9.md` (decision,
tables, projection formula, problems encountered); update the backlog #2 row
and the initiative timeline. On a go, additionally draft nothing in this
session beyond the report's next-steps note — plan10 opens as its own
session with the mechanism design below as its starting point.

## Phase 1 sketch (contingent — plan10's charter, not executed here)

If Phase 0 goes:

- `Search::seed_guide_line(&[Move])`: replay the UCI line from the root FEN,
  validate legality per the position-mapping rule, and install an ordering
  seed. Design axis to settle in plan10: a dedicated guide table (ranked
  between `SCORE_KILLER` = 50 000 and the history cap 10 000) versus
  overwriting history entries / pre-filling killer slots at the line's ply
  depths. Global history seeding fires at unrelated nodes too (sign
  unknown — this is the load-bearing unmeasured term of bar (c)).
- CLI: opt-in `--guide-moves "<uci>"`; empty/absent = byte-identical to
  pre-plan10 (drift protocol: quick-suite outcomes and evals unchanged with
  the flag off; with the flag on, quick-suite *outcomes* unchanged and no
  new timeouts — the plan8 ε lesson shows ordering changes can flip
  win→draw via 5 s budget timeouts, so this gate is real, not ceremonial).
- `examples/sf_guide`: the engine client (UCI protocol above) printing the
  line for `--guide-moves`; the solver CLI never spawns processes.
- Validation: two-sided per working agreement #3 (drift protocol + the
  repetition soundness gate, `cargo test --release --test test_repetition
  -- --include-ignored` — ordering-only, but run it anyway; the cyclic rook
  must never claim a win). Unit tests for seed legality-truncation and
  flag parsing. Stress-case measurement against the step-0 baseline with
  the adoption bar: guided wall (engine + solve) ≤ 50% of unguided.

## Success metric for Phase 0

Not a code change: the decision itself. The report must state (i) the
coincidence table, (ii) W, (iii) the projection with its formula and
honest error bars, and (iv) the resulting backlog disposition. A negative
result cleanly closing backlog #2 is a valid completion.

## Estimated effort

S–M: step 0 ~30 min of solver runs, step 1 ~15–90 min of engine wall
depending on whether 100M is needed, steps 2–4 are analysis, step 3 is the
only code touch (small, env-gated, reverted). One session.
