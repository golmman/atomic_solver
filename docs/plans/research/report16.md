# report16 — plan16 (item #18, Phase 1: gated shaped-variant rollout of `mob-or` and `mobcap8`)

Date: 2026-10-10. Initiative: `research`. Evidence:
[`measurements/plan16/`](measurements/plan16/) (drivers + parsed state +
`env.json`; raw transcripts uncommitted, regenerable via `driver.py`).
Instrumentation: a temporary env-gated init hook (`ATOMIC_INIT=mob-or |
mobcap8`, init only, no counters) at the existing `(1, 1)` fallback site in
`src/search/dfpn/children.rs`, plus a temporary D2(d) helper
(`examples/init_trace_check.rs`); **no product change remains** — both were
added, measured, and reverted (`git diff --exit-code` clean; release build
re-verified; D2(a) salt-0 anchors re-run exact after the revert).

## Verdict: **both arms REJECT** — item #18 closes measured-out

Pinned gate v1.0 (D3/D4/D6) applied verbatim to 396 fresh paired runs
(3 arms × 22 cases × 6 salts; baseline arm run first and identity-checked
before the candidate arms):

| arm | lever verdict | reject drivers |
| --- | --- | --- |
| `mob-or` | **reject** | regressions: dec02 (1.78×), dec05 (1.26×), dec06 (2.15×), dec10 (median 1.32×, ratios 1.07–1.76); censored-loss: **m23_white** (candidate censored on salts 1/3/5 where baseline solves, 8–12 M there), **dec01** (catastrophic: 66–128 M vs 5.5–6.9 M baseline on 4 uncensored pairs, censored at 1 B on 2 more) |
| `mobcap8` | **reject** | regressions: dec05 (2.40×), dec10 (median 1.48×, ratios 1.20–1.98) |

No defer was issued on any case, so the D4 salt-extension rule never fires;
the verdicts stand as terminal. Per the pre-registered gates: **the #18
shaped-variant family closes measured-out** — the two shapes plan15
nominated fail at gate grade on the full corpus; the Phase-0 nomination
stands as below-noise-floor hypothesis-ranking data.

## What replicated and what the full corpus exposed

The plan15 nomination **replicated**: both arms hold hard-class direction
wins exactly as measured in Phase 0 — stress **censored-win** for both
(baseline censored at 2.5 B on salts 2/3/4; candidate ratios 0.08–0.40 on
the uncensored pairs), **m22_white win** for both (mob-or median 0.67×,
mobcap8 median 0.71×, 6/6 paired salts each) — plus dec13 (0.10×),
dec14 (0.07–0.10×), and most decisive controls at 0.04–0.34×.

But 22 cases × 6 salts exposed regressions plan15's 4-case single-salt grid
could not see:

1. **dec01 is a catastrophic class for `mob-or`** (basins=6, the strongest
   multi-draw case in the corpus). The OR-side `(n, 1)` init turns a ~6 M
   solve into a ≥ 1 B-censored search on 2 salts and a 11–55× blowup on the
   other four. `mobcap8` *wins* the same case (0.45×) — the conservative
   AND-side cap absorbs the damage.
2. **m23_white is a censored-loss for `mob-or`** (hard class, basins=5):
   candidate censored at 1 B on 3 salts where baseline solves in 9–12 M —
   a veto-class failure on the m-family itself, not a control-class
   footnote.
3. **dec05 regresses for both** (1.26× / 2.40×) and **dec02 for `mob-or`**
   (1.78×, salt-invariant) — quiet-case regressions outside the noise band.
4. **dec10 regresses for both** at gate grade (6 paired salts each; see D6
   below) — the plan15 single-draw caveat (1.21× / 1.35×) was real, not
   noise: the paired salt set puts both arms' medians at/above the quiet
   band edge (mob-or 1.319× vs band top 1.316; mobcap8 1.478×), each with a
   salt outlier at 1.76× / 1.98×.

The mechanism reading from plan15 stands unchanged: the OR-side `(n, 1)`
init carries the wins (stress/dec13/dec14 class) *and* the tail risk
(dec01, dec02, dec06 for the OR-only shape); the capped-cap shape
(`mobcap8`) trades stress depth (0.40× vs 0.29×) for robustness (it wins
dec01/m23 where `mob-or` collapses) but still cannot clear dec05/dec10.

## D6 — the dec10 caveat decision (pre-registered, applied verbatim)

The pinned rule fired exactly as pre-registered: dec10 is a **regression**
for both arms (median ≥ 1.1 ∧ ≥ 75 % of ratios > 1 on 6/6 paired salts),
so it rejects — for `mob-or` alongside three other regressions and two
censored-losses; for `mobcap8` alongside dec05. The pre-registered
calibration question ("should a non-hard-class regression within the noise
band veto a hard-class win?") **was not triggered**: neither arm's sole
negative is dec10, and no recorded paired-ratio vector sits inside the
quiet band — mob-or's dec10 median (1.319×) sits 0.003 above the band top
(1.316) with a 1.76× outlier, mobcap8's at 1.478× with a 1.98× outlier.
Recorded as data with no rule change: **no methodology revision, no
W_LO/W_HI change** (standing revision rule).

## Per-arm verdict tables (D3; ratios paired by salt, basin counts from plan12)

Full per-salt censoring patterns and ratios:
[`measurements/plan16/state/verdicts.json`](measurements/plan16/state/verdicts.json).
Condensed (basins = baseline basin count; verdicts per the pinned rule):

<details><summary>mob-or</summary>

| case | basins | verdict | ratios |
| --- | --- | --- | --- |
| m20_white | 4 | unresolved_under_budget | (cand cens 4/6) |
| stress | 3 | censored-win | 0.285, 0.099, 0.300 |
| m22_white | 6 | win | 0.501–1.117, median 0.673 |
| m23_white | 5 | censored-loss | 0.535, 1.067, 0.449 |
| dec01 | 6 | censored-loss | 54.5, 10.3, 18.6, 11.1 |
| dec02 | 1 | regression | 1.781 (salt-invariant) |
| dec03 | 1 | win | 0.075 |
| dec04 | 1 | win | 0.222 |
| dec05 | 1 | regression | 1.262 |
| dec06 | 1 | regression | 2.15–2.37 |
| dec07 | 1 | win | 0.169 |
| dec08 | 1 | unclear | 1.000 |
| dec09 | 1 | win | 0.045 |
| dec10 | 4 | regression | 1.072–1.764, median 1.319 |
| dec11–dec18 | 1 | win | 0.036–0.288 |

</details>

<details><summary>mobcap8</summary>

| case | basins | verdict | ratios |
| --- | --- | --- | --- |
| m20_white | 4 | unresolved_under_budget | 0.254 (1 uncensored pair) |
| stress | 3 | censored-win | 0.396, 0.083, 0.342 |
| m22_white | 6 | win | 0.542–1.064, median 0.712 |
| m23_white | 5 | win | 0.471–0.652 |
| dec01 | 6 | win | 0.373–0.465 |
| dec02 | 1 | unclear | 0.976 |
| dec03 | 1 | win | 0.105 |
| dec04 | 1 | win | 0.337 |
| dec05 | 1 | regression | 2.402 |
| dec06 | 1 | win | 0.402 |
| dec07 | 1 | win | 0.207 |
| dec08 | 1 | unclear | 1.000 |
| dec09 | 1 | win | 0.045 |
| dec10 | 4 | regression | 1.201–1.976, median 1.478 |
| dec11–dec18 | 1 | win | 0.093–0.277 |

</details>

Notes: m20_white is **unresolved under budget** for both arms (candidate
censored on 4/6 salts at the pre-registered 2 B cap; > 3 of 6 ⇒ the pinned
under-budget rule applies and the case contributes nothing). The raw
censoring pattern is mixed and direction-inconsistent (mob-or: base censored
on salts 0/1, candidate on salts 2–5; mobcap8 interleaved), i.e. the m20
evidence is heavy-tailed and under-budgeted, not a measured win or loss.
Per the diversity clause, the many basins=1 wins are exact per-salt effects
(within-salt variance = 0) but cannot solely support an adopt — the rejects
rest on multi-basin cases (dec01 basins=6, m23 basins=5, dec10 basins=4).

## Paired-ratio dispersion (comparison rule's second real-arm calibration)

| arm | n | p10 | median | p75 | p90 | max | frac outside [0.9, 1.1] |
| --- | --- | --- | --- | --- | --- | --- | --- |
| mob-or | 118 | 0.039 | 0.169 | 1.000 | 1.781 | 54.51 | 93.2 % |
| mobcap8 | 124 | 0.093 | 0.277 | 0.542 | 1.000 | 2.40 | 89.5 % |

Data point for the methodology owner (recorded with a reason, per the
standing revision rule): the shaped-init levers are strongly **bimodal per
case** — a candidate arm is either a large win (0.04–0.4×) or a
multi-x regression on a given trajectory, with almost nothing in the
[0.9, 1.1] band (93 % / 90 % of pairs outside). The pinned rule's median +
75 %-agreement clauses handled this cleanly (no verdict hinged on the
band edges), but the observed lever behavior is far heavier-tailed than
the between-salt noise the W_LO/W_HI thresholds were calibrated against;
a future lever family with this signature might warrant a per-case
worst-regression veto discussion. No change proposed now: n = 2 lever
families measured, both rejected, no adopt ever hinged on the gap.

## Soundness invariants (all pass; no HALT)

- **(a) Pristine identity:** pre-hook build at HEAD reproduces plan12
  salt-0 counts exactly (stress 249,480,478; m22 14,156,269; m23 9,673,403;
  dec13 3,822,602; dec10 4,262,128; m20 censored at 2 B) — run before the
  hook was applied; re-run exact after the revert (g).
- **(b) Hook non-perturbation:** the hook build with `ATOMIC_INIT` unset
  reproduces plan12's recorded per-salt values exactly, all 132 baseline
  cells (evals and censoring status).
- **(c) plan15 anchors:** all 12 init-on cells reproduce plan15's archived
  `state/arms.json` values exactly (mob-or stress 71,148,597 / dec13
  486,175 / dec10 5,148,137 / m22 15,813,483 + 11,859,889 + 14,223,312;
  mobcap8 98,874,361 / 509,576 / 5,765,793 / 15,067,529 + 12,825,207 +
  13,387,712) — the init mapping, the `child_is_or` ↔ `(n, 1)` orientation,
  and the port fidelity are pinned (no lost anchors).
- **(d) Orientation trace:** `ATOMIC_INIT_TRACE` stderr lines on dec02
  match an independent helper (`examples/init_trace_check.rs`, temporary)
  move-by-move: the first 8 fresh inits are the OR-parent root children
  with child mobilities {7, 6, 6, 8, 10, 10, 10, 10} → mob-or `(n, 1)`,
  mobcap8 `(min(n,8), 1)`; AND-parent inits read `(1, 1)` under mob-or and
  `(1, min(n,8))` under mobcap8. The trace was removed before any
  measurement run.
- **(e) Outcomes:** zero decisive-outcome conflicts within any arm across
  salts; every uncensored run of every arm matches its fixture expected
  outcome.
- **(f) Censored sentinel:** no censored run is decisive (structural check
  in `parse.py`).
- **(g) Revert:** hook + helper deleted; `git diff --exit-code` clean;
  release build re-verified; (a) anchors re-run exact.

Rollout cost: ~2.2 h CPU-serial (base 0.70 h / mob-or 0.77 h / mobcap8
0.71 h wall at ≤ 3 concurrent processes); candidate arms faster than
baseline on the hard class (stress 0.29×/0.40× salt 0), slower on dec01/
dec05/dec10 — absorbed by the caps, no unexpected explosion.

## Problems encountered

- The anchor script's first attempt (a shell job-pool with exported
  associative arrays) silently mismatched case/arm arguments; redone in
  Python before any number was read. No measurement impact.
- The hook helper initially landed inside `children.rs`'s test module;
  compile caught it immediately (moved to top level before any run).
- `parse.py` required `state/` to exist; created manually (plan13's script
  had the same implicit assumption).
- No plan15-anchor drift: the product CLI reproduces the probe-example
  counts bit-exactly, confirming plan15's probe was trajectory-neutral.

## Missing tests

- None to add: no product code changed (report15 precedent applies;
  `make test` not required for a no-code-change session; the release build
  plus the salt-0 trajectory identity — a stronger check — were run twice).
- The D7 hand-off test list (init-mapping unit tests, outcome-consistency
  regressions) is **not** triggered: no adopt, no productization.

## Next steps

1. **#18 family record:** update the #18 row to close the family
   measured-out (Phase-0 nomination + Phase-1 rejection at gate grade; the
   diagnosis stands, the shapes fail). The later #18 stages (blast /
   king-zone threat features, clock-borrowed initialization) remain
   *possible future work* only if the owner reopens the family with a new
   mechanism hypothesis — the current evidence says mobility-shaped leaf
   init is not a robust lever.
2. Alternative (the drafted-next queue): **#19 remaining closures** —
   clock-budget solved-entry reuse (`dfpn` #2), `lean` plan10
   history/killer arms, TT-eviction arm V2 — each a one-goal plan under the
   pinned gate.
3. Lower priority: #20 salted TT-keeping restarts (distribution data now
   much richer after two 396-run rollouts), #21 in-context child results
   (dec10 class), #22 fringe.

SESSION COMPLETE
- plan16 executed end-to-end: D2(a)–(g) all pass, 396/396 runs, 12/12
  anchors exact; verdicts mob-or REJECT / mobcap8 REJECT (D3+D6 verbatim,
  no defer, D4 not fired); measurements/plan16/ written (driver, parse,
  verdicts, env.json, README.md, state/); hook + helper reverted clean
  (`git diff --exit-code`), release build re-verified, anchors re-run
  exact; report16.md written; initiative.md #18 row updated (family closed
  measured-out)
Follow-up options:
  1. "Execute docs/plans/research/plan17.md" — draft + execute the next
     #19 re-score arm (clock-budget solved-entry reuse, `dfpn` #2): the
     highest-value remaining unjudged closure, one-goal sized, same gate.
  2. Alternative: close the `research` initiative session here and record
     the #18 family closure in the owner's queue first — no new mechanism
     hypothesis for the init family is on the table, so a plan17 for #18
     is not indicated.
