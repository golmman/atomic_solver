# Report 8 — Salt-seeded process portfolio at gate grade (backlog #5)

Executes `plan8.md` (pre-registered 2026-10-11, before any race run).
Measurements: `measurements/plan8/`. Zero product-code changes: every racer
*is* the unmodified solver at a different `--salt`; the sequential path was
never touched (stage 0 was an identity check, per plan).

## Verdict: **ADOPT** (mechanical, against the pre-registered rule)

adopt ⇔ H1 ∧ H3 ∧ H4 — all three pass. H2 (gated dec10 clause) passes too;
there is no contention anomaly. H4 is the soundness gate; a violation would
have meant halt-and-diagnose, not a lever verdict — nothing to diagnose.

- **H1 (conversion, the adopt trigger) PASS.** All three m20_white races had
  ≥ 1 uncensored decisive racer (three of them: s5/s3/s2 all decisive);
  `W_first` median **95.89 s** (gate ≤ 160 s). The shipped salt-0 run is
  budget-censored at 2 B evals — fresh solos: 313.5/319.7/312.6 s, all
  `draw`-censored, no answer. The portfolio converts the shipped timeout
  into a **proven win at 791,401,553 child evals** (0.40× of the burnt cap)
  in every race.
- **H2 PASS (dec10).** `W_first` median 0.522 s vs fresh baseline median
  0.680 s = **0.768×** (gate ≤ 0.80×). The m23_white clause is recorded, not
  gated (not in the raced subset): evals prediction 0.976× from plan12.
- **H3 (no wall regression) PASS.** stress 1.046× / m22_white 1.063× /
  dec13 1.054× — all ≤ 1.10×. The measured 4-way contention tax on the
  winning racer is ≤ 6.3 % wall (m20 winner: 95.89 s vs 93.7 s solo =
  **1.023×** — essentially nil).
- **H4 (fidelity, hard gate) PASS.**
  (a) Stage-0 identity: 5/5 exact plan12 anchor reproductions (m22 s0/s2,
  dec10 s0/s2, stress s0 — evals + outcome).
  (b) In-race: **63/63** decisive racer runs reproduce their plan12 (or
  stage-2 rem12) anchor `evals` + outcome exactly — determinism survives
  4-way concurrency bit-for-bit.
  (c) Zero decisive-outcome conflicts across racers in any race.
  (d) Censoring discipline held: m20's salt-0 racer recorded
  `censored_budget` ×3 (never a proven draw); stress s2/s3 racers recorded
  `wall_killed` ×3 (killed at first-decisive + 240 s grace per plan), evals
  excluded from verdicts.

Gate adaptation (declared in the plan) applied as registered: fresh
interleaved solo salt-0 baselines + the frozen plan12 six-draw distribution
as reference; verdict categories mapped 1:1 onto the pinned rule. Diversity
clause: m20_white basin count 4 (racer-set coverage 3/4 uncensored basins),
stress 3 (coverage 2/3), dec10 4 (4/4), m22 6 (4/6), dec13 **2** (low-
diversity — cannot support an adopt, only veto; it does not veto), rem12 6
(out of sample, 4/6). The adopt verdict rests on m20_white's censored-win —
a structural censored-vs-decisive contrast on a hard-class case with 4
basins, not a within-noise ratio.

## Prediction vs measurement

The stage-1 frozen pre-computation (`state/derived_evals.json`, computed
before any race) held exactly:

| case | predicted (evals space) | measured evals_first | measured W_first ratio | match |
| --- | --- | --- | --- | --- |
| m20_white | censored-win, min racer 0.791 B (s5) | 791,401,553 every race | 0.31× vs censored baseline | exact |
| dec10 | 0.685× (s2) | 2,918,636 every race (0.685×) | 0.768× | exact evals; wall tracked |
| m22_white | 1.000× (salt-0 is min) | 14,156,269 (salt-0 first) | 1.063× | exact |
| stress | 1.000× (salt-0 is min) | 249,480,478 (salt-0 first) | 1.046× | exact |
| dec13 | 1.000× | 3,822,602 (salt-0 first) | 1.054× | exact |
| rem12 | out of sample (stage-2: 6 basins) | 59,573,726 (s3, 0.856×) | **0.889×** | out-of-sample wall win |

Wall tracked evals everywhere: the min-evals racer finished first in all 18
races. Out-of-sample rem12 confirms the in-sample dec10 pattern — a fresh
case never used for racer selection produced a 0.89× wall win from a
salt-basin minimum. The pointwise-dominance prediction is confirmed: the
portfolio never regressed in evals space on any case (evals_first = min
over racers ≤ salt-0 everywhere by construction).

## Cost accounting

Per race, Σ racer core-seconds (medians): m20_white **747**, stress 606,
rem12 36, m22 9, dec10 3, dec13 3 — vs the sequential baseline's
356 core-s for the whole round's six solos (~3.9× CPU for the portfolio
family on the raced subset). Breakdown of the big two:

- **m20_white**: 747 core-s buys the first answer this case has ever produced
  (the shipped run's 313 core-s yields *nothing* — 2 B-eval censored
  timeout). The "wasted" compute is R0's 320 core-s (the shipped computation
  itself, run concurrently) plus R1's 219 core-s (a second uncensored proof
  that would have been the answer without s5). W_first ≈ 96 s wall vs
  ≥ 313 s to *still have no answer*.
- **stress**: 606 core-s, of which 543 are the two grace-killed s2/s3 racers
  (2.5 B-budget-censored trajectories that would never finish inside any
  useful wall). This is the honest price of the take-first-decisive shape:
  racers that cannot win still burn their slice until grace expires.
  A shorter grace (or killing remaining racers when the answer is already
  `win`) would reclaim most of it — a ship-shape knob, not a plan change.

Not-raced cases (dec01/dec14 + the 14 salt-invariant cases) contribute no
race cost: their portfolio result is the pre-computed table — racers are the
same computation (1 basin) or salt-0-anchored (1.000×).

## Deviations

None against the deviation policy (no mid-campaign mechanism, racer-set, or
cap changes). Recorded findings:

1. **H2/m23 wording vs raced table**: plan8's H2 names m23_white, but the
   fixed-parameters raced table (correctly, per the ≤ 1 s contention-noise
   argument) omits it. Handled as the table implies: the m23 clause is
   recorded from the pre-computation (0.976×), marked not-gated in
   `races8.json`. No race was added post hoc.
2. **Ambient load**: loadavg 0.45–0.65 at the pilot, 1.1–2.4 during the
   campaign (the `pi` agent process plus accumulated zombie entries from
   earlier sessions; no user jobs launched). Fidelity held bit-exact
   regardless and per-round W_first spread was ≤ 0.7 % on m20 and ≤ 2 % on
   the small cases — well under the ±20 % extension trigger (never fired).
3. **Fresh salt-0 baselines reproduced plan12 salt-0 evals exactly on all
   six cases** (2,000,000,027 / 249,480,478 / 4,262,128 / 14,156,269 /
   3,822,602; rem12 was new) — a third independent confirmation of the
   salt-0 identity invariant, across sessions.
4. The pilot's four racers already demonstrated H4b before the campaign
   (all four salts matched their plan12 anchors in one concurrent race).

## Ship-shape decision space (recorded, not implemented)

The lever is GO; *how* it reaches operators is an owner decision. Options:

1. **Documented operator pattern** (zero code): "for deep solves, launch
   3–4 background solvers at `--salt 0/2/3/5` with the case budget, take the
   first decisive answer, kill the rest." Cheapest, no maintenance, but
   error-prone by hand (grace/kill bookkeeping, evals parsing) and invisible
   to the tooling.
2. **`examples/portfolio` driver** (S effort): a small example binary (or
   shell/Python driver under `examples/`) that launches the pre-registered
   racer set, implements the grace/kill + take-first-decisive logic, and
   prints the winning racer's line. Product solver untouched; RAM contract
   = N × TT must be documented (4 × 128 MB here). Note: plan7 answered
   "do-not-ship" for a *config-portfolio* driver — that answer died with the
   plan7 NO-GO; this is a different, now-proven lever.
3. **Product flag** (M effort): e.g. `--race K` inside the CLI. Gains a
   supported surface and docs, at the cost of product surface for something
   that is semantically just K separate `Search` runs; the RAM restatement
   and the nondeterminism-which-proof caveat would need main.rs docs. No
   `src/search` change is needed — the flag would only fork processes.

Trade-off summary: option 2 captures the entire measured win at S effort
with zero product risk; option 3 only pays if the operator population is
wide enough to justify CLI surface; option 1 is fine for the current
single-operator reality. None of the three touches the soundness contracts
(per-process soundness is inherited; nondeterminism stays confined to *which*
valid proof wins).

## Unresolved / not done

- The 14 salt-invariant cases + dec01/dec14 were not raced (per plan):
  their portfolio behavior is the pre-computed table, not a wall
  measurement. A hypothetical contention tax on them is bounded by the
  raced small-case evidence (≤ 6.3 %).
- m23_white never raced (finding 1).
- The grace cost on stress (543 core-s) suggests a smarter kill policy
  (stop remaining racers once the root is proven, not after 240 s) — only
  relevant if a ship shape (option 2/3) is built; it is a driver policy,
  not a solver property.
- Backlog #6 (cooperative portfolio) and #7 (any-worker-can-finish SPDFPN)
  read this result under the now-confirmed draw-luck regime; their
  pre-registrations are separate plans.

## Tools / effort

- Drivers: `derive_portfolio8.py`, `campaign8.py`, `analyze_portfolio8.py`
  (all in `measurements/plan8/`; pattern follows plan7's harness).
- Total session wall: stage 0–2 ≈ 7 min, pilot instant, campaign ≈ 50 min
  (3 rounds × ~975 s), analysis immediate.
- Environment: commit `deaee91`, clean tree, release build (no-op), 4 CPUs,
  4 × 128 MB TT racers = 512 MB (RAM contract documented in `env.json`).
