# Report 1: Reach-vs-depth spike + leaf men-count profile — gate verdict RETHINK

Implements `docs/plans/solve/plan1.md` (backlog #1). Date: 2026-09-22.
All measurements under `measurements/plan1/`; protocols, deviations, and
pre-registered formulas are documented in `measurements/plan1/README.md`.

## TL;DR

- **Pre-registered gate verdict: RETHINK** — the campaign as scoped is not
  defensible on this evidence. The reach arm fired cleanly: **7 of 8
  self-play lines' last recordable ladder position sits before ply 30**
  (lines terminate at ply 6–38, none reach ply 44), and no depth range
  beyond ply 6 has ≥50% uncensored cost data — the growth curve the plan
  wanted cannot be estimated.
- The work landscape behind that verdict is bimodal: **52 of 75 cost runs
  (ply 1–30, ~30-men quiet positions) sit at a censoring plateau of
  20.5–46.6M dfpn nodes at the 120 s cap** (median 22.3M, ≈186k nodes/s),
  while every uncensored point except one is a **cheap tactical proof**
  (1–4,749 nodes; several opening mainlines are *forced wins in 5–9
  plies*, proven in ≤5k nodes). The single deep uncensored point —
  d4d5 p30, win in 29 — needed 23.2M nodes / 101.8 s, i.e. it sits exactly
  at the cap: the boundary between "tactical shot" and "unprovable within
  any budget we can afford per position".
- **The leaf men-count profile kills the EGTB-anchoring premise at
  generable depths.** Over 507.0M non-terminal child-eval sites (the
  probe-replaceable population, egtb-plan1 semantics) on the three
  instrumented searches: **≤5 men 0.000001%, ≤6 men 0.000009%**; median
  19 men; **≥95% coverage requires 23 men**. Neither 5 nor 6 men
  "suffices" — the answer the plan forwarded to the `egtb` reopening
  decision is that *no generable table depth reaches this population*.
- **First verify/find datapoint: 1.18 wall ratio at the deep boundary**
  (d4d5 p30: find 101.8 s vs `reconstruct_pt --validate` 120.3 s, both
  `validate: ok`). Verification wall is *not* a small constant on top of
  find wall — the reconstruct walker is movegen-heavy per node
  (32,942 walker nodes in 120 s). The initiative's combined metric
  (constraint 3) must price this in from the start.
- 1 GB-TT addendum at the deep point: **−22.6% nodes** (23.23M → 17.98M)
  for 8× the table — real but modest TT-size sensitivity; RAM is not the
  binding constraint at this frontier.
- A genuinely new positive finding the plan did not anticipate:
  **atomic opening mainlines are razor-sharp on this solver's semantics** —
  1.e4 e5 (win in 9), 1.e4 c5 (win in 5), 1.Nf3 d5 (win in 5) are proven
  in ≤4,749 nodes, and every ladder that survived past ply 20 ended in a
  forced-mate PV steered by the solver itself. The campaign's hardness is
  not uniformly spread over depth; it concentrates in the *quiet defense
  systems* (the censored plateau), which the ladder's own steering cannot
  enter.

## 1. What was run

1. Random-playout control: 200 uniform-random atomic games from startpos
   (`random_playouts.json`, with per-ply men trajectories). Game length:
   median 73, p90 322, p95 339, max 531 plies; 161/200 end in extinction
   Loss, 32 Draw, 7 by the (never hit at cap 1000) — none capped. Final
   men-count spread 2–32 (median 16).
2. Eight self-play ladders (startpos, 1.e4, 1.d4, 1.Nf3, 1.e4 e5,
   1.e4 c5, 1.d4 d5, 1.Nf3 d5), 8 s bounded steer solves, 2-ply advances
   (PV move pair when steerable, else `list_legal`-first-move fallback).
   All 8 lines ended in terminal positions at ply 6–38
   (`state/ladder_*.json`); per-line PV-steered reach: 10 / 27 / 27 / 25 /
   10 / 6 / 38 / 6.
3. Cost runs (`--timeout 120 --first-outcome`, non-outcome-only,
   sequential): 79 runs = 75 fit-eligible (ply ≥ 2) + 4 informational
   seed positions; 48 censored (timeout ⇒ lower bounds only), 27
   uncensored. TT snapshot byte size recorded for every run
   (108 B–146 MB; 128 MB TT fills to ~95% occupancy on censored runs).
4. 1 GB-TT addendum at d4d5 p30.
5. Temporary men-count instrumentation (egtb-plan1 method, re-derived;
   `MenHistogram` was removed post-egtb-report) on d4d5 p30 / d4d5 p32 /
   e4e5 p2 — the three deepest uncensored positions with non-trivial
   work (deviation documented in the README). Node counts and stdout
   byte-identical to the clean cost runs; sources reverted; `git diff
   src/` clean; post-revert m22 drift capture byte-identical.
6. `reconstruct_pt --validate` on the 5 deepest uncensored positions
   (d4d5 p30–p38): all `validate: ok`. Snapshot files were pruned
   before commit as byte-reproducible derived artifacts (see README;
   regeneration ≈ 4 min).
7. Pre-registered fit + gate (`fit.py` → `fit.json`).

Environment (recorded honestly): the sandbox exposes **4 cores and an
8 GiB cgroup memory limit** (plan assumed ≤~10 cores / 32 GB); release
build at git `34a19c0`, defaults everywhere (128 MB TT, ε=0.125,
default refine-cap). All runs sequential; the plan's ≤2-concurrent
allowance was used only briefly by the random-playout batch while cost
runs 1–2 were in flight (nodes unaffected — work proxy; wall recorded
for context only).

## 2. The growth curve: what the data supports

**The pre-registered fit is degenerate, and that is the finding.** The
pooled ≥50%-uncensored window spans plies 2–38 but contains only 26
uncensored points among 75 (all of them the mate-line residues:
e4c5 p2–6, e4e5 p2–10, nf3d5 p2–6, d4d5 p30–38, plus the p10–27
tactical shots). Within that set, work *decreases* with depth — pooled
b = 0.94/ply, per-line b = 0.14–0.34/ply — because as a forced mate
approaches, first-outcome cost collapses to 1 node. A per-ply growth
factor of the campaign-relevant kind (hard-position work vs depth) is
**not estimable from this ladder**: there is no depth range with
genuinely ≥50% uncensored data beyond ply 6.

What the cost runs do establish, as lower bounds and boundary points:

| population | n | work |
| --- | --- | --- |
| Censored quiet positions, ply 1–30 | 48 | 20.5–46.6M nodes at the 120 s cap (median 22.3M; ≈186k dfpn-nodes/s) |
| Uncensored tactical proofs, ply 2–38 | 26 | 1–23,228k nodes; all but one ≤ 4,749 |
| d4d5 p30 (win in 29, 25 men) | 1 | 23,228,233 nodes / 101.8 s — sits at the cap |

The plateau is flat in *depth* (no visible ply trend from 1 to 30 —
everything quiet censors at similar node counts) but the distribution's
edge is sharp: the same d4d5 line is censored at p2–p28 and uncensored
at p30 with a 29-ply proof. Depth in plies is the wrong axis for work;
the axis is **distance-to-tactical-resolution**. Positions where a
forcing line exists are cheap at any ply; positions without one are all
≥ 2×10^7 nodes at 120 s, regardless of ply.

Projection honesty: with b unestimable, the pre-registered projection
W = b^(D_liq − d_e) cannot be defended (fit.json's formal W = 1.0 and
D_eff = 0 are artifacts of the degenerate fit, not evidence). The gate
verdict therefore rests on the reach arm alone — which fired without
ambiguity.

## 3. Gate verdict: RETHINK (pre-registered reach arm)

The plan's RETHINK arm: *"W > 10^17, or the ladder censors before ply 30
on ≥ half the lines (reach too shallow to project)"*.

- Ladder terminal depths (estimated, last record + ~2 plies): 8, 8, 12,
  16, 27, 29, 29, 40 — **7 of 8 lines below ply 30**.
- Mechanism: PV-steering requires a proof; quiet middlegame positions
  (~30 men, no contact) are unprovable at 8 s *and* at 120 s, so the
  ladder drifts on fallback moves until a tactical shot appears
  (ply 22–32) or the line rule50-draws/mates out. The reach the plan
  wanted to measure ("how deep can startpos-reachable mainlines be
  solved") is bounded by the *steering* budget, and under every steering
  budget we tried, reach << 44.

Per the plan: *"the campaign as scoped is not defensible; initiative
pivots (e.g., anchor-only subgoal: deep EGTBs as the deliverable)."*
Two caveats the decision should weigh (both are data, not gate
arithmetic):

1. **The named pivot target is undermined by the same measurement.**
   Deep EGTBs as *search anchors* presuppose that proofs descend into
   low-men territory — they do not (§4: 507M leaf sites, median 19 men,
   ≤6 men 0.000009%). An "anchor-only" campaign would generate tables
   that the frontier searches never probe. If the initiative pivots, the
   pivot needs a new rationale (e.g., EGTBs as a *correctness oracle*,
   the egtb-report1 role) or a new anchoring theory (frontier-push from
   solved low-material positions upward, which is a different mechanism
   than leaf probing and is NOT measured here).
2. **The sharpness finding reshapes the sizing question.** The campaign
   does not face a uniform ply-44 frontier: three of eight opening
   systems collapse into forced wins ≤ 9 plies, and d4d5's quiet
   structure dies at ply 30 to a 29-ply forced win that costs 102 s.
   The width that matters is "refute the best defense to each tactical
   shot", and the count of such defense systems — not ply depth — is
   the sizing variable a re-scoped plan should estimate.

## 4. Leaf men-count profile (d_e for the egtb reopening)

Pooled over the three instrumented searches (507,025,251 non-terminal
child-eval sites; per-run splits in `state/menhist_summary.json`):

| men | cumulative share of non-terminal sites |
| --- | --- |
| ≤4 | 0 (no sites at all) |
| ≤5 | 0.000001% |
| ≤6 | 0.000009% |
| ≤9 | 0.006% |
| ≤12 | 0.376% |
| ≤16 | 12.6% |
| ≤19 (median) | 52.5% |
| ≤21 | 85.6% |
| ≤23 | 95.0% ← d_e (men) |
| ≤25 | 99.98% |

- The egtb plan1 result (≤4 men 0.000% on product-suite positions)
  generalizes to startpos-frontier proofs, and gets *worse* with scope:
  even ≤6 men covers 0.000009% here. **The plan's literal question
  ("does 5 suffice or is 6 required?") has the answer: neither — the
  leaf population is centered at ~19 men.**
- Plies-to-liquidation horizon from the random-playout trajectories:
  ≥95% of games sit at ≤5 men only within the last ~157 plies and at
  ≤6 men within ~196 plies of their terminal — i.e., in unguided play
  the low-men phase is a thin tail at the very end (and best-play proofs
  barely reach it at all within 120 s: the d4d5 p30 proof tree's sites
  span 5–25 men with mass at 13–21).
- Consequence for backlog #2: a 4→5→6-men depth push is **not justified
  as an anchor for startpos-frontier searches** on this evidence. The
  egtb reopening plan, if any, should be scoped around the oracle role
  or around a bottom-up frontier-push mechanism that probes tables by
  construction (searching from the solved end outward), not around leaf
  probing inside top-down searches.

## 5. Verify/find ratio (first datapoint for constraint 3)

| position | find (wall / nodes) | verify (wall / walker nodes) | validate | wall ratio |
| --- | --- | --- | --- | --- |
| d4d5 p30 | 101.8 s / 23,228,233 | 120.3 s / 32,942 | ok | **1.18** |
| d4d5 p32 | 0.028 s / 900 | 0.014 s / 68 | ok | 0.50 |
| d4d5 p34–p38 | 0.02 s / 6–9 | 0.012 s / 2–6 | ok | ~0.6 |

The deep point's ratio ≈ 1.18 is the first evidence that the combined
metric (wall(find) + wall(verify)) roughly *doubles* the effective cost
exactly where the campaign lives: the reconstruct walker re-derives
every proof-tree node with full movegen (≈274 walker nodes/s), so
verification wall scales with proof-tree size, not with TT hit rates.
Design consequence for any campaign plan: artifact size is a first-order
cost (constraint 3's "designs that inflate the proof tree pay twice"
is measured, not hypothetical). The trivial points show the ratio
approaches ~0.5–0.6 for tiny trees (fixed walker overhead dominates
differently); more datapoints across tree sizes are needed for a curve.

## 6. TT-size sensitivity (addendum)

d4d5 p30 at `--tt-size 1024`: **17,983,230 nodes (−22.6%) vs 128 MB**
(23,228,233), 503 MB snapshot, `Complete` well inside the 120 s cap.
An 8× table buys 1.29× work at this frontier — consistent with the
m21/m22 regime's "tree ≫ TT" characterization
(`research/structural_floor.md` §8): the plateau positions are not TT-
starved, and RAM is not the binding constraint on reach.

## 7. Problems encountered

- **Ladder protocol ambiguity (resolved twice, documented):** the plan's
  step-cap stop rule is unexecutable as written for unprovable shallow
  seeds (half the ladders would die at ply ≤ 1), and my first revision
  (stop after 4 consecutive capped steps) truncated every line at
  ply 6–10 because quiet fallback lines never re-enter the solver's
  reach. Final protocol: lines run to ply 44 / terminal; per-step steer
  status recorded; reach = deepest PV-steered ply. All in the README.
- **`outcome` parsing bug** (my harness, not the solver): `outcome: win
  length: 29` was stored verbatim and broke verify candidate selection;
  fixed and the stored JSONs normalized.
- The 1 GB addendum and instrumentation runs had to be sequenced after
  the cost batch (a mid-batch binary swap would have changed the
  instrument under measurement).
- The egtb plan1 `MenHistogram` instrumentation no longer exists (removed
  post-report as dead-goal debt); it was re-derived from the report's
  description in ~30 lines of temporary counters. Node-count and stdout
  byte-identity between clean and instrumented runs proves inertness.
- Compute reality: 48 censored cost runs cost ~107 min of the ~2.5 h
  batch; the 4-core/8 GiB cgroup (vs the plan's assumed envelope) did
  not bind, but a ply-44 ladder with richer lines would not fit this
  sandbox's patience at 120 s/run sequentially.

## 8. Deviations from the plan

1. Ladder stop rule revised twice (README documents both revisions and
   the reasons); per-line reach is reported as deepest PV-steered ply.
2. Instrumentation positions: three deepest uncensored **with non-trivial
   work** instead of the literal three deepest (1–9-node searches);
   documented above.
3. Verify positions: five deepest (not two) for the same reason; the
   meaningful ratio datapoints are p30/p32.
4. The pre-registered pooled fit window definition ("plies with ≥50%
   pooled uncensored fraction, lowest..highest") produced a degenerate
   window; the formal b/W are reported in `fit.json` but explicitly not
   interpreted. This is a failure of my pre-registration wording, fixed
   on the record by §2's population table instead of a post-hoc fit.

## 9. Missing tests

- No landed code, so no product tests are affected. The harness
  (`spike.py`/`fit.py`) is measurement-only; its correctness rests on the
  recorded raw logs (byte-replayable lines) and the inertness checks.
- If the egtb reopening ever revives an in-tree men histogram, the
  egtb report's missing-test note (two_rook_mate → 100% ≤4 smoke test)
  still applies.

## 10. Next steps

1. **Decision needed (user):** apply the RETHINK verdict to the `solve`
   initiative. Options, with the evidence each would need:
   - **Pivot to sharpness-first scoping:** re-estimate campaign width as
     "number of best-defense systems to tactical shots" (the §3 caveat);
     cheap to explore further with targeted solves (no ladder needed).
   - **Pivot to bottom-up frontier-push:** needs a new plan defining the
     push mechanism (solve-the-value-frontier-at-ply-k from the
     low-material end) — note §4's caveat that leaf probing inside
     top-down searches is measured dead, so this is an architecture
     change, not a table generation task.
   - **Close with the artifact:** the spike's bimodal work landscape,
     leaf profile, verify/find ratio, and TT sensitivity are the sized
     inputs any future revival starts from.
2. Forward to `egtb` (reopening decision): d_e(men) = 23 at ≥95%
   coverage; ≤5/≤6 shares 0.000001%/0.000009% — a 4→5→6 depth push is
   not justified as a top-down search anchor; scope any revival around
   the oracle role or a bottom-up mechanism.
3. Forward to `parallel`/`proof` (constraint 3): verify/find wall ratio
   1.18 at the deep boundary; proof-tree artifact size is a first-order
   campaign cost.
4. File the sharpness finding (1.e4 e5 win-in-9 etc.) as intelligence
   for any startpos-value narrative; the positions and lines are
   byte-replayable from `state/ladder_*.json`.
5. Update `docs/plans/README.md` status index when the pivot/close
   decision lands (not done here — the decision is pending).

Per repo convention, this report is the plan's final task.
