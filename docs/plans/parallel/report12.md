# Report 12 — backlog #7 groundwork: Phase-0 noise probe (stage 0); stages 1–3 vacuous

Executes `plan12.md`. Outcome up front: **the stage-0 Phase-0 noise probe
not-rejects H0 on both pre-registered cases → D1 fires NO-GO (mechanical);
backlog #7 is closed NO-GO and the initiative closes with it.** Stages 1
(selective re-land), 2 (per-phase work accounting), and 3 (revert) are
GO-branch-only by the plan's own scoping ("Stage 1 — selective re-land (only
on the GO branch; executed after the probe)"); on NO-GO **no code change was
made** — `src/` is untouched at commit `f88fa62`, so there is nothing to
revert (the plan's NO-GO bullet parenthetical "(re-land reverted in
stage 3)" is read as covering the hypothetical in which stage 1 had already
run; it had not). The tax pin and the accounting smoke were therefore never
built or measured; their non-results are recorded below as vacuous-by-branch,
per the plan's own risk note ("cut stage 2 …, never the stage 1 verification"
does not apply — stage 1 itself is branch-gated off).

## 1. What ran (read-only; no code changes)

- **Committed data** (no new runs): `measurements/plan6/state/campaign6_raw.jsonl`
  (8 rounds × N ∈ {1..4} × {m22, rem12, shuffle_win}, with per-run
  `helper_evals` / `coordinator_evals` / jobs / wall / outcome) over the
  research-plan12 corpus (`docs/plans/research/measurements/plan12/state/runs.json`,
  22 cases × 6 salts; `stress` = shuffle-win, `m22_white` = m22; rem12 absent).
  Consistency check that makes the comparison self-consistent: plan6's
  in-session sequential references equal the corpus salt-0 draws **exactly**
  (shuffle-win 249,480,478; m22 14,156,269) — same flags (FO + outcome-only),
  same era.
- **Fresh probes** (current binary, sha256_16 `c0a80db9124a9aca`, commit
  `f88fa62`, sequential only):
  - rem12 × salts {0..5} (`rem12_salts.py` → `state/rem12_salts.json`):
    all 6 draws decisive wins, 64.12–72.42 M evals, band [0.963, 1.088]
    (spread 1.129×) — tighter than the 1.31× the plan quoted from plan8's
    out-of-sample confirmation.
  - stress × salts {0, 1, 5} robustness (`stress_salts_fresh.py` →
    `state/stress_salts_fresh.json`): 287.9 M / 1138.1 M / 311.8 M — the
    loud salt-1 draw **persists on the current binary at 3.95×** (corpus
    era: 3.04×). The loud-draw phenomenon is not a stale-binary artifact.
- **Binary-drift declaration**: the current binary's salt-0 counts differ
  from the committed records (rem12 66,604,453 vs plan6-era 69,605,385,
  −4.3 %; stress 287,872,755 vs plan11-era 305,404,068, −5.7 %, vs corpus
  249,480,478, +15.4 %). This does not contaminate the verdict: the plan6
  campaign data is judged by the plan6-era corpus band (salt-0-identical
  denominators), and the fresh probes are normalized by their own salt-0
  draw. It *does* mean any future mechanism campaign would need fresh
  same-binary bands (plan13 would have re-baselined anyway, per its preview).

## 2. H0 verdict (pre-registered tests, mechanical)

**H0 (noise-floor):** the plan6 shuffle-win t4 bad-trajectory tail
(3/8 runs at 90–100 s, 5.67/5.93/6.33× eval inflation, 2 cap-hits + 1 slow
win) is an ordinary realization draw under the measured per-run parallel
overhead.

### Magnitude test (load-bearing)

Reject only if the observed max tail inflation exceeds **every**
`ratio_max(uncensored) × overhead` product by > 20 %. Overhead estimators
(all reported in `state/probe_noise.json`):

| estimator | shuffle-win t4 | product vs observed max 6.33× |
| --- | --- | --- |
| `inmax` — max inflation among non-tail t4 runs | 2.47× | 3.04 × 2.47 = **7.50 ≥ 6.33** |
| `decomp` — inflation / coordinator_ratio (median) | 3.82× | 3.04 × 3.82 = **11.61 ≥ 6.33** |
| `inmed` — median inflation among non-tail runs | 1.59× | 3.04 × 1.59 = 4.83 < 6.33 |

Verdict: **not rejected** — under the two identification-consistent
estimators the observed tail sits 16–45 % *below* the noise-model bound. The
`inmed` reading would nominally reject, but it is internally inconsistent:
with overhead 1.59 the good t4 runs (inflation 1.28–2.47) would need
realization ratios 0.81–1.55 while the bad tails would need 3.6–4.0 — above
the max uncensored draw (3.04) — and it contradicts the cross-case overhead
level (rem12/m22 t4 both measure ≈ 3.8, below). The plan pre-registered
"max measured overhead", which selects the inmax/decomp readings.

**rem12** (the sharper test): no tails exist (0/8 cap-hits at every N; max
wall 17.8 s < 1.5× seq). Median t4 inflation 3.03× vs the pre-registered
threshold 1.31 × median overhead = 5.01× (decomp estimator; 4.16× with the
fresh band max 1.088) → **not exceeded** → H0 holds: rem12's inflation is
the known mechanism overhead, not realization luck.

### Frequency test (veto-only, weak power stated)

- shuffle-win: sequential tail rate (ratio ≥ 2×) = 1/3 uncensored draws;
  the 3 budget-censored draws are tails a fortiori (ratio ≥ 10.0×) → 4/6
  over all draws. Parallel t4 tail rate 3/8 = 0.375 sits **inside** [1/3, 2/3]
  — no veto (it is below the all-draws rate). Censoring reported, not imputed.
- rem12: 0/8 parallel vs 0/6 sequential — consistent, nothing to veto.
- Limit stated per plan: with 6 draws the frequency test can only veto; it
  did not.

### Decomposition (method 3 — the plan13 design input that now goes unused)

The strongest finding of the probe, for the record: defining
`overhead_decomp = inflation / (coordinator_evals / seq_salt0)`, the t4 work
multiplier is **remarkably constant at ≈ 3.8× on every one of the 24 t4 runs
across all three cases** (m22 3.56–3.90, rem12 3.78–3.92, shuffle-win
3.67–3.86). Run-to-run total-work variance is almost entirely
coordinator-side: the coordinator's realized work spans 0.33–1.70× salt0 on
shuffle-win (0.66–1.00 on rem12), while helpers scale with it. The bad-tail
excess vs good-run medians is ~72 % helper-side (+3.1 salt0-units) and ~28 %
coordinator-side (+1.2); the bad-tail coordinator draws (1.51–1.70×) are
plausible mid-band realizations (stress draws reach 3.04× uncensored and
≥ 10× censored), though unobserved among only 3 uncensored stress draws.
Identification caveat (recorded in the JSON): `coordinator_evals` is itself
perturbation-contaminated — good runs' coordinator ratios (0.33–0.66) fall
below the sequential band minimum, impossible for a pure realization — so
"coordinator realization" conflates the salt draw with helper-TT pollution.
The magnitude verdict does not rest on this identification (it holds under
the estimator-free `inmax` bound too). This decomposition also retro-explains
plan6's differing I4 medians (2.22 / 3.03 / 3.54) as coordinator-realization
differences, not mechanism differences.

### H0 conclusion

**H0 holds** (magnitude on both pre-registered cases; frequency does not
veto): the "trim the bad tail" premise of #7 is dead — the min-selection
any-worker-can-finish would add is already harvested by the adopted salt
portfolio at zero `src/` risk. Per the plan: *"#7 closes NO-GO on the probe
alone."*

## 3. D1 outcome

**NO-GO (mechanical).** GO required H0 rejected at magnitude on ≥ 1 of
{shuffle-win, rem12} plus the stage-1 tax pin plus the stage-2 smoke; the
first conjunct failed on both cases, so the gate never reached stages 1–2.
The borderline clause (owner decision on mixed cases / within-20 % results)
does not fire: both pre-registered cases agree, and the shuffle-win margin
is 16 % *below* the most conservative noise product (not within 20 % of
exceeding it).

**Out-of-scope finding (m22, plan6's control — reported, not gated):** m22's
parallel tails **do** exceed every noise product — t3 r3 at 11.48× and t4 r2
at 33.86× (cap-hit, draw) vs max product 1.671 × 4.24 = 7.08× (observed max
exceeds it by 378 %), with sequential tail frequency 0/6 vs parallel 2/8.
This is a genuine parallelism-specific inflation component — the
perturbation / repetition-stall signature (job counts 10–50× typical:
119,815 vs 8–16 K on the t4 cap-hit run). It is **consistent with plan6's own
NO-GO record** (I4 = 3.54× and cap-hit 1/8 on the control case) and does not
rescue #7's premise: S4(m22) = 1.01× — parallelism is pointless on the case
class where the pathology lives, and the ship shape was "long solves only".
No in-repo action follows; the record is here and in `probe_noise.json`.

## 4. Vacuous-by-branch results

- **Stage 1 (selective re-land) + tax pin**: not executed (GO-only). The
  named conflict surfaces (lean 4-slot TT, `with_mb` rounding, facts hooks,
  `--facts-*` CLI) were never touched; no re-land identity, `make test`, or
  tax-pin measurement exists. The plan's risk ("budget half the session for
  stage 1") was never spent.
- **Stage 2 (per-phase accounting) + accounting smoke**: not executed
  (GO-only; it instruments the re-landed build). report6's "first diagnostic"
  remains unbuilt — moot for this initiative, which is now closed.
- **Stage 3 (revert)**: vacuous — nothing was landed.

## 5. Problems encountered

- Plan-text inconsistency: stage 3's header says "(NO-GO branch or tax-pin
  miss)" while stage 1 says "only on the GO branch". Resolved by the literal
  GO-only reading of stage 1 (D1 precedes it); recorded here rather than
  silently normalized.
- The plan's rem12 expectation "loud spread only 1.31×" was stale (plan8-era,
  different binary): the fresh band is [0.963, 1.088]. Direction unaffected
  (a tighter band weakens the noise explanation for rem12's inflation even
  less — the median test passes with more margin under the fresh band via
  the decomp estimator: threshold 4.16× vs median 3.03×).
- `/usr/bin/time` is absent in this container; the fresh-run drivers time
  with `time.monotonic()` (wall is not load-bearing; evals are deterministic).
- The decomposition estimator needed defining (the plan names `overhead_r`
  but not its estimator); three estimators are computed and reported, and
  the verdict is shown to be robust to the choice (§2). This is the one
  place where the pre-registration underdetermined the test; the report
  errs toward transparency rather than a single silent choice.

## 6. Unresolved parts / missing tests

- The noise model remains under-determined by design (6 salt draws per case,
  3 censored on stress): H0 is *not rejected*, which is weaker than
  *confirmed*. The record states the limit; no further measurement is
  proposed (the initiative is closed and the marginal information does not
  change any decision).
- The m22 short-case pathology (11.5×/33.9× tails) is diagnosed only at
  signature level (job-count blowup + cap-hit + outcome draw), not
  root-caused. It is consistent with the report5a repetition-region stall
  and plan6's cap-hit record. Any future owner interest in short-case
  parallelism would need a fresh plan with its own pre-registration; nothing
  in the current backlog motivates one (S4(m22) = 1.01×).
- No tests were added or changed (no code changes); `make test` was not run
  (nothing to gate — the working tree contains only new files under
  `docs/plans/parallel/measurements/plan12/` and doc edits).

## 7. Deliverables

- `measurements/plan12/`: `README.md` (provenance), `env.json`
  (binary-drift declarations, tmpfs note — the solver writes no files in
  these runs; transcripts under `/tmp/plan12_probe`), drivers
  (`rem12_salts.py`, `stress_salts_fresh.py`, `probe_noise.py`), state
  (`rem12_salts.json`, `stress_salts_fresh.json`, `probe_noise.json`).
- `initiative.md` — backlog #7 row closed NO-GO + status paragraph; no other
  rows touched.
- `docs/plans/README.md` — `parallel` row updated (close event: #7 NO-GO,
  initiative closed).
- `report12.md` — this file (final task).
- `src/` untouched; no proof-tree, benchmark, or tuner changes (non-goals
  respected).

## 8. Closure

```
SESSION COMPLETE
- plan12 executed: stage-0 Phase-0 noise probe only (read-only); H0 not
  rejected on both pre-registered cases (shuffle-win tails inside every
  ratio×overhead product; rem12 no tails, median test passed; frequency no
  veto); decomposition finding: t4 mechanism work multiplier ≈ 3.8× constant
  across all 24 t4 runs on all three cases; out-of-scope m22 finding
  recorded (11.5×/33.9× tails exceed the noise model — parallelism-specific,
  short-case class). D1 = NO-GO (mechanical) → backlog #7 closed NO-GO,
  initiative closed; stages 1–3 vacuous; plan13 never drafted; report12.md
  written; measurements/plan12/ committed; docs/plans/README.md row updated.
- Gate: not applicable (no code changes; src/ untouched at f88fa62).
Follow-up options:
  1. No follow-up — the parallel initiative is closed with every backlog
     item resolved; the salt portfolio (plan8/plan9) remains the family's
     adopted mechanism. Recommended: leave closed.
  2. (Owner-only, not recommended by the data) If the m22 short-case
     pathology ever becomes product-relevant, open a NEW initiative with a
     fresh pre-registration targeting the perturbation/repetition-stall
     signature (job-count blowup) — plan12's probe_noise.json is the ground
     truth to pre-register against. The bad-tail motivation that carried
     #7 is dead and must not be re-used.
```
