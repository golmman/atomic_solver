# report14 — plan14 (item #23: seeded move-order tie-break noise channel)

Date: 2026-10-11. Initiative: `research`. Evidence:
[`measurements/plan14/`](measurements/plan14/) (Phase-0 probe only),
[`gate_methodology.md`](gate_methodology.md) (stands as pinned v1.0).

## Gate result: **H0 — closed measured-out at the Phase-0 kill gate**

The pre-registered D4 kill gate fired: the fraction of `sort_moves` calls
whose **top-2 score is tied** is far below the 10 % threshold on every one
of the four gate cases (0 of 4; the gate needs ≥ 2):

| gate case | sort_moves calls | top-2 tie % | evals (probe budget) |
| --- | --- | --- | --- |
| m20_white | 1,008,426 | **6.19 %** | 20,000,035 (budget-censored) |
| dec10 | 606,762 | **4.55 %** | 4,262,128 (full solve) |
| m22_white | 857,953 | **1.54 %** | 14,156,269 (full solve) |
| dec13 | 164,869 | **1.05 %** | 3,822,602 (full solve) |

A tie-break channel perturbs the trajectory only where the composite
ordering score ties; on the eval-sensitive cases — precisely where plan12
found the salt channel basin-limited — such ties are rare (1–6 % of nodes)
mid-search. Per the pre-registered D8 rules, the verdict is **H0**: item
#23 closes measured-out, the `--seed` product knob is **not** built, the
D6 rollout and D7 basin/correlation analysis were **not** run, and
`gate_methodology.md` stands unchanged at pinned v1.0. No HALT occurred
(no soundness invariant was ever at risk — no product code was touched).

## Deliverables

1. **plan14 executed through Phase 0 and Phase 3 only.** No `src/` changes
   remain: the temporary, env-gated probe instrumentation
   (`ATOMIC_TIE_PROBE=1`, one counters module plus three hook lines in
   `history.rs`/`dfpn/mod.rs`/`main.rs`) was added, measured, reverted, and
   `git diff --exit-code` passes before and after (tree clean at
   `41a82e2`, rebuilt and re-verified after the revert).
2. **Probe corpus and design (pre-registered D4).** Quick benchmark suite
   (dec01–dec46 + m23–m29 from `tests/fixtures`) plus m22_white and
   m20_white, each as
   `atomic_solver --fen <FEN> --tt-size 128 --first-outcome --outcome-only
   --budget 20000000 --timeout 120` under `ATOMIC_TIE_PROBE=1`, ≤ 3
   concurrent processes. Per `sort_moves` call the probe recorded: move
   count, top tie-group size of the composite score, total moves in
   tie-groups, top-2 tie flag, max top tie-group size, and a top-tie-size
   histogram (`state/probe.json`).
3. **Probe hygiene (clean).** dec15 and dec10 at the plan12 1 B budget
   reproduce the plan12 salt-0 child-eval counts **exactly**, with the
   probe disabled *and* enabled (dec15 1,077,420; dec10 4,262,128): the
   counters never perturb the trajectory, and cross-session trajectory
   identity holds again.
4. **Tie-prevalence numbers** (full table in
   [`measurements/plan14/README.md`](measurements/plan14/README.md)):
   - Aggregate tie mass is large but **deep**: 33–81 % of moves per call
     sit in some tie-group, confirming the opening-session read-only
     `move_order_debug` picture.
   - But the tie-group **at the top score** is a single move on 94–99 % of
     calls for the deep cases (top-2 tie rate 1.05–6.19 % on the gate
     cases); max top tie-group sizes are 5–7 there.
   - The eval-sensitive / low-diversity cases have the *lowest* top-2 tie
     rates (dec13 1.05 %, dec14 1.75 %, m22 1.54 %, m20 6.19 %, dec10
     4.55 %). High top-2 tie rates (10–30 %) occur only on small
     tactically-forced positions (dec22 29.6 %, m26_white 21.3 %, dec34
     20.6 %, dec27 24.0 %) — all salt-invariant, single-basin cases where
     no noise is needed.
5. **Item #23 and the report12 caveat resolved with data.** The standing
   caveat "salt-channel independence from tie-breaking unmeasured" is now
   a *measured* negative for the pre-registered mechanism: a top-anchored
   tie-break cannot be a productive noise channel on this corpus, so
   independence from tie-breaking is vacuous at gate grade — the salt
   channel remains the gate's only calibrated channel.

## Why the channel is structurally silent (finding)

The mechanism rationale (D1 motivation) assumed ties would be reachable at
the point where ordering decisions bind. The probe shows the opposite
regime: where history/killer bonuses are unpopulated the solver is deep in
tactic-forced subtrees (ordering barely matters and DF-PN thresholds make
the top move irrelevant), and where ordering binds — the eval-sensitive
deep cases — the dynamic bonuses plus static score separate the top-2 in
> 93 % of nodes. The root tie-blocks that motivated the plan were real but
not decision-relevant. This also explains why salt 5 could not add a
clean fifth channel: both candidate perturbation mechanisms are quiet on
exactly the same cases, and the per-case basin ceiling (plan12 D7) is a
property of the case's trajectory structure, not of the channel count.

**Caveat for any successor idea (not pre-registered, not explored):** the
probe measured the *top* tie-group only. Deep tie mass is large, so a
channel that reorders *all* tie-groups (e.g. a hash perturbation of the
whole ordering, or a seeded stable-shuffle within tie-groups at every
depth) would have mechanical room to act — but that is a different
mechanism touching DF-PN selection indirectly at every level, and it would
need its own plan with its own pre-registration (and likely a much larger
divergence risk per draw). The pre-registered scope of #23 (top-anchored
secondary sort key) is measured out.

## Problems encountered

- None in execution. The probe fired on the first pass; the kill gate
  evaluation is unambiguous (max 6.19 % vs 10 % threshold).
- One cosmetic note: the temporary module needed `pub` (not
  `pub(crate)`) visibility so the binary's `main.rs` could call the dump
  hook across the lib/bin boundary; irrelevant after the revert.

## Missing tests

- None to add: no product code changed (the seed-0 identity, per-seed
  determinism, and outcome-consistency tests named in plan14 Phase 1 are
  moot — the `--seed` option does not exist). The existing suite is
  untouched; `make test` was not required for a no-code-change session,
  though the release build was re-verified after the revert.

## Next steps

- Item #23 is closed; the #17 noise-channel family is settled at two
  channels measured: the salt (calibrated, pinned v1.0) and the top-anchored
  tie-break (measured-out, this report). Any further channel idea starts
  from a new pre-registered plan with a Phase-0 probe — this session's
  experience shows the probe-to-kill-gate pattern is cheap and decisive.
- The open research backlog is unchanged: #19 re-scores (plan13 drafted;
  its ε arms remain the recommended next execution), #18 leaf-init family
  (m22-collapse diagnosis first), #20 salted restarts, #21 in-context
  child results, #22 fringe.

SESSION COMPLETE
- plan14 Phase 0 executed per pre-registration; kill gate → H0; probe
  instrumentation reverted (`git diff --exit-code` clean);
  measurements/plan14/ archived (probe_driver.py, env.json, state/probe.json,
  README.md); report14.md written; initiative.md #23 row + history updated;
  docs/plans/README.md research row updated (caveat resolution).
- Gate: H0 (item #23 closed measured-out; gate methodology stands at v1.0).
Follow-up options:
  1. Execute plan13 (item #19 ε re-score arms) — the next open pre-registered
     plan in the queue; runs the pinned gate on real lever arms.
  2. Draft a fresh plan for a whole-ordering tie/noise channel only if the
     owner wants to pursue the deep-tie-mass finding above — explicitly not
     item #23, needs its own Phase-0 probe and kill gate.
