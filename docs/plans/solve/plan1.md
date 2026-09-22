# Plan 1: Reach-vs-depth spike + leaf men-count profile (the go/rethink gate)

Initiative: `solve` backlog #1. This plan produces the two numbers every
later stage is sized by, before any hardware or engine investment:

1. **The growth curve**: how first-outcome solve cost (work proxy)
   scales with game depth on startpos-reachable mainlines — the
   pre-registered go/rethink gate for the whole initiative.
2. **The leaf men-count distribution**: at what men-count do deep
   searches' child-eval sites sit — the sizing input for the EGTB depth
   push (backlog #2; reopens `egtb` with the campaign's requirements).

Deliverables: `measurements/plan1/` (raw captures + README command
table), the fitted growth model with its pre-registered gate verdict,
and `report1.md`.

**No `src/` changes land.** One part of the spike uses *temporary*
men-count instrumentation following the lean plan7 spike discipline
(apply → measure → fully revert → drift-verify byte-identical). The
instrumentation method is the one `egtb/report1.md` used for its
child-eval-site men-count measurement (its ≤4-men 0.000% result is the
product-suite datapoint this plan extends to deep startpos-frontier
positions); if the exact snippet is not recoverable from that report,
re-derive it there.

## Hardware envelope

This sandbox: ≤ ~10 cores, 32 GB RAM. The spike is sequential (the
metric is work, not wall — one solver process at a time; ≤2 concurrent
runs permitted for the bulk ladder only, where wall is not recorded).
Defaults everywhere: 128 MB TT, ε=0.125, default refine-cap, release
build. 128 MB TT is a controlled variable for comparability with the
m21/m22 baselines (`research/structural_floor.md` header); one deep
position is re-run at 1 GB TT as an addendum (TT-size sensitivity).

## Positions under test

**Self-play mainline ladder** (deterministic, reproducible): 8 seed
FENs — startpos and the positions after 1.e4, 1.d4, 1.Nf3, 1.e4 e5,
1.e4 c5, 1.d4 d5, 1.Nf3 d5 (all via `replay`). From each seed, extend
the line in 2-ply steps: at each step run one bounded solve
(`--timeout 8 --first-outcome`, non-outcome-only pass, stdin=DEVNULL)
and play its **PV's first move** (`pv:` line, `replay` to advance);
fallback when no PV is printed: the first move of `list_legal` order.
Every line is recorded move-by-move in a JSON so any position is
byte-exactly replayable. Ladder runs to **ply 44** or until the step
solve hits the step cap, whichever first.

**Depth ladder cost runs** (the growth curve): at each ladder position
(ply 2…44), one solve per position: `--timeout 120 --first-outcome`
non-outcome-only pass. Recorded per position: outcome, wall, `pre_exit:
nodes=N` (the documented work proxy — `child_evals` is not CLI-visible;
state this in the README), PV length, TT snapshot size
(`--tt-dump-path`), and whether the run was **censored** (timeout ⇒
unproven; censored points enter the analysis as lower bounds only).
One pass per position (the non-outcome-only pass yields outcome, nodes
and wall together; the plan2 double-pass split is not needed here).

**Random-playout control** (cheap breadth): ~200 uniformly random
atomic games from startpos (all legal moves equally likely). Recorded:
game length distribution (plies to terminal) and final men-count
distribution. Purpose: the ruleset's natural liquidation depth, and a
breadth control against the mainline bias of the ladder.

## Temporary instrumentation (men-count at child-eval sites)

On the **3 deepest uncensored ladder positions** only: with the
temporary counters applied, record the histogram of
`position.piece_count()` (whatever egtb plan1 measured) at child-eval
sites, split by side-to-move, plus the men-count at search exit. Output
to stderr; capture; **revert; verify `git diff` on `src/` clean; run
the m22 first-outcome drift capture — byte-identical stdout required**
(the lean protocol). Deliverable: for each position, the empirical
distribution P(men = m) over child-eval sites.

## Pre-registered gate (fixed before fitting — do not tune after seeing data)

Let b = fitted per-ply growth factor of median uncensored solve work
(least squares on log(nodes) vs depth, pooled across lines over the
depth range with ≥50% uncensored data; per-line b reported too). Let
D_liq = the mainline liquidation depth indicated by the ladder +
random-playout evidence (depth at which lines reach terminal/EGTB-class
positions), and let d_e = the EGTB anchor depth covering ≥95% of
instrumented leaf sites (the backlog-#2 requirement). Effective
remaining proof depth D_eff = D_liq − d_e. Projected campaign work
W ≈ b^D_eff nodes (crude by design; the gate is on the order of
magnitude).

- **GO** (backlog #2/#3 unblock): W ≤ 10^15 nodes. Calibration: the
  checkers solve took ~10^14 node touches over 18 years on 1990s–2000s
  hardware; the campaign gets 10²–10⁴× more throughput, so 10^15 is the
  largest projection we can defend.
- **MARGINAL**: 10^15 < W ≤ 10^17 — proceed only with a named plan for
  a ≥100× work reduction (deeper anchoring, better pruning class);
  the initiative stays open with the note as state of record.
- **RETHINK**: W > 10^17, or the ladder censors before ply 30 on ≥half
  the lines (reach too shallow to project) — the campaign as scoped is
  not defensible; initiative pivots (e.g., anchor-only subgoal: deep
  EGTBs as the deliverable).

Independent of the gate, the men-count histogram sets d_e and is
forwarded to the `egtb` reopening plan (4-men is certainly insufficient
for startpos leaves if the egtb plan1 result generalizes; the question
this plan answers is whether 5 men suffices or 6 is required).

## Tasks

1. Write the spike harness (Python stdlib, black-box CLI driver:
   ladder generator, cost recorder, random-playout runner) under
   `measurements/plan1/`; README command table following the
   `parallel/measurements/plan2/` layout.
2. Run the random-playout control batch (~200 games).
3. Run the 8 self-play ladders to ply 44 / step-cap; capture raw
   stdout/stderr per run.
4. TT snapshot size recording at every ladder position; 1 GB-TT addendum
   at the deepest uncensored position.
5. Temporary men-count instrumentation on the 3 deepest uncensored
   positions; revert; drift-verify (byte-identical m22 first-outcome
   capture; `git status` docs/measurements-only).
6. Fit the growth model, compute d_e and W, apply the pre-registered
   gate. Verify-side datapoint: on the 2 deepest uncensored positions,
   run `reconstruct_pt --snapshot … --validate` and record wall(verify),
   validated node count, and the verify/find ratio (first datapoint for
   the combined metric of constraint 3).
7. Write `report1.md`: growth curve + gate verdict, d_e and the EGTB
   depth requirement forwarded to `egtb`, censoring analysis, verify/find
   ratio, problems, missing tests, next steps (plan 2 = `egtb` reopening
   plan and/or plan 3 = distributed prototype per the verdict).

## Non-goals

- No EGTB generation (backlog #2), no distributed prototype (backlog
  #3), no GPU work, no solver optimization.
- No landed `src/`/`examples/` changes; instrumentation is applied and
  fully reverted within task 5, with the drift capture as proof.
- No product-solver benchmark or suite changes; the existing baselines
  (m21 stress, m22) are reused as calibration points, not re-measured.
- No claim about the startpos value itself.

Per repo convention, the final task of this plan is writing its
`report1.md` in this directory.
