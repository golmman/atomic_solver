# Report 11: Phase 3 Stage 2a — advisory-dn censor-signal validation at larger n + the pre-registered certificate soundness contract

Initiative: `solve`. Executes `plan11.md` (Stage 2a, registered **S**,
measurement-only). Runs of 2026-10-04, strictly sequential (SMOKE →
R1′ → R2′ → contingent R3′ → merge → analysis). Harness:
`measurements/plan11/sweep11.py` (Python 3 stdlib, command table in
`measurements/plan11/README.md`), environment `env.json` (4-CPU quota,
8 GiB cgroup, oom_kill 0 throughout; product binary byte-identical to
plan9's registered `1b70b32f46d9218e…` hash — no rebuild needed).
**No solver, lib, or campaign-code changes** — the sweep is the plan9/plan10
campaign machinery driven by a new driver; the merge pass drives the
existing binaries.

## TL;DR

- **M2 gate verdict: SUBSTRATE_EMPTY — Stage 2 (A6b early-censor
  certificates) halts.** The stratified corpus (H 24 / M 12 / L 12 primary
  + U 8 control) produced **P = 0 positives at the full warm ladder**
  (256M / 1G / 2G child-evals per leaf per rung): all 56 leaves censored at
  every rung, **100% cap-pinned at all three budgets**, in 182.3 G
  child-evals ≈ 6.6 G nodes (2.81 h ladder wall). No AUC/θ/S(θ) is
  computable — the advisory-dn channel has no positives to validate. The
  plan10 n=2 substrate is not validated at larger n; it is *emptied*: the
  certificate policy's predicted-positive mass is empty at
  plan11's budgets. plan12 (the certificate implementation) is not
  draftable; per the registered bands this is a valid completion —
  Stage 2 closes with the sharpened reopener record, and the pre-registered
  certificate design + path-scoped soundness contract are preserved in
  `campaign_architecture.md` §11 A6b as the halt record.
- **The A6b lever's premise re-confirmed at scale:** the ≈100% re-burn of
  warm re-queues (plan9 §4, plan10 §3) holds at 256M, 1G and 2G on the
  certificate mechanism's actual target population (previously-censored
  warm lines). Budget escalation on this corpus converts nothing —
  consistent with plan10's ~1 conversion per ≈ 169 G projection.
- **Covariate finding for any revisit:** in the warm regime (the
  certificate-consumption regime) the committed-dn stratification
  partially flattens — R3′ pre-rung medians H 69,806 vs M 29,258 /
  L 24,897 / U 31,608 (M/L/U overlap). The advisory channel does not
  preserve the committed-dn ordering the strata selection relied on.
- Machinery: 0 job errors, 0 missing results, 0 verify failures (A2 never
  fired — vacuously: zero decisive results), 0 deviations, 0 oom kills,
  restore-probe 0 mismatches at every warm boundary, SMOKE clean, R3′
  fired by the registered rule (1.14 h elapsed + 2.03 h projected ≤ 4.5 h).

## 1. What ran

| stage | budget/leaf | jobs | wall | child evals | outcome |
| --- | --- | --- | --- | --- | --- |
| SMOKE | 100k × 2 phases | 8 + 8 | 5.0 s | 1.60 M | 0 win / 16 draw; close/restore protocol clean |
| R1′ (fresh) | 256M | 56 | 794.3 s | 14.34 G | 0 win / 56 draw |
| R2′ (warm) | 1G | 56 | 3120.5 s | 56.00 G | 0 win / 56 draw |
| R3′ (warm, contingent) | 2G | 56 | 6218.2 s | 112.00 G | 0 win / 56 draw |
| merge pass | — | 0 decisive | — | — | 0 re-verified; close = identity of plan10's merged close |
| **total** | | **168** | **2.81 h** | **182.34 G** | |

Machinery per rung: exit codes 0/0/0/0, TT-dump sets complete (~111 MB
each), job-line cross-checks true, restore integrity `file == table` with
`probe_checked=101 probe_mismatched=0` at every warm relaunch (R2′/R3′),
peak tree-RSS 0.89 GB per rung (watermarks 7.0/3.5 GiB never approached),
oom_kill delta 0. Observed campaign rate 667–677k nodes/s (κ_obs 26.7–26.9
vs locked 27.5 — this environment again runs faster than the locked
constants; all projection arithmetic used the locked values).

## 2. The M2 corpus and the registered covariate regimes

Sampling frame: the 660 `Open` leaves of plan10's committed merged close
(sanity-checked 728/68/660/27; committed advisory identity-checked against
plan9's frozen S1 close — 0 divergences). Strata built deterministically
and committed as `state/strata.json` before the ladder: **H** = top-24 by
committed advisory dn (ties → higher work; `c1g5 g8f6` forced in —
`c1g5 f7f6` was already in the top-24), **M**/**L** = 12 leaves each at the
touched set's ~55–75th / ~15–35th committed-dn percentiles (band ranks
56..77 / 15..36 of 104), **U** = 8 untouched leaves, round-robin over
children by static rank. Primary corpus = H ∪ M ∪ L (48); U analyzed
separately, excluded from the gate.

Covariate regime matching (registered, binding): R1′ covariate = committed
advisory dn (fresh regime; secondary); R2′/R3′ covariate = the prior rung's
post-run advisory dn (warm regime; primary). Per-stratum covariate medians:

| stratum | committed dn (R1′ pre) | R2′ pre (warm) | R3′ pre (warm) |
| --- | --- | --- | --- |
| H | 28,503 [5,935…146,201] | 26,250 [6,975…137,627] | 69,806 [22,626…200,105] |
| M | 5,470 [4,740…6,960] | 15,900 [9,642…28,619] | 29,258 [1,120…37,840] |
| L | 2,664 [2,546…2,923] | 13,558 [7,873…24,213] | 24,897 [3,133…34,997] |
| U | 1 | 16,094 [10,220…83,351] | 31,608 [2,893…192,577] |

## 3. The M2 gate (pre-registered bands)

```
P = 0 positives at the full ladder including fired R3'
  → verdict SUBSTRATE_EMPTY (stronger M2-MARGINAL wording)
pooled warm-regime AUC: not computable (0 positives)
θ / S(θ): undefined (no positives)
directional inversions: 0 (no positives)   held-out FNs: 0
registered control covariate (cumulative work): not evaluable
  (0 positives — vacuous, not "passing"); no defect detected in the
  covariate pipeline itself (per-leaf covariates recorded in full in
  state/m2_corpus.csv)
Lost labels: 0 (no machinery-noteworthy events)
```

Per the registered bands: **Stage 2 halts**. The corpus is the honest
statement that the substrate cannot be validated at affordable n on this
frontier — not because the signal separates weakly, but because the signal
has nothing to separate: zero conversions anywhere in 182 G child-evals,
including in H (the hypothesis's predicted-positive mass) and in the two
force-included c1g5 near-misses.

What this does and does not falsify: it does not show the advisory-dn
direction is wrong (there are no positives to contradict it); it shows the
certificate policy has no predictable-positive mass to act on at
plan11's budgets, so a skip/park predicate could never be validated nor
worth building. Any certificate revival needs a conversion-rate step-change
first (a different frontier, root, or resource regime), not re-measurement
of this corpus.

## 4. U-stratum control (analyzed separately; gates nothing)

8 untouched leaves (committed work = 0, committed dn = 1): 0 conversions at
256M/1G/2G, 26.05 G child-evals total, 100% cap-pinned at every rung. The
cheap untouched class plan10 harvested at 8M is gone from the open frame —
the surviving untouched leaves are untouched *because* they are hard.

## 5. Merge pass (A2 discipline)

Registered shape: decisive results → `campaign_master --resume` over
plan10's merged state (`--job-seed 11`, `--max-wall 60`). The sweep
produced **zero decisive results**, so the pass's input set is empty and
the master was **not invoked** (recorded interpretation, see §7
deviation 4): in resume mode the master dispatches new jobs from t = 0, and
plan10's "results never arrive within the wall" slop assumption breaks with
an empty result set — running it would have added ~60 s of unregistered
master work to the committed close while verifying nothing. The committed
close `state/master_state_merge11.json` is the verified byte-identity of
plan10's merged close (`close_is_identity_of_seed: true`); Δfacts = 0,
matching the corpus (68 Won / 660 Open / children_resolved 0/27 unchanged).

## 6. Deliverables

1. **Soundness contract** — pre-registered in plan11 §3 before any run;
   with the SUBSTRATE_EMPTY verdict it is not carried into a plan12;
   amended verbatim into `campaign_architecture.md` §11 as **A6b** (the
   halt record).
2. **Driver** — `measurements/plan11/sweep11.py` + `README.md` (command
   table, registered shape, registered analysis readings, binary-identity
   section), plan10's machinery conventions throughout.
3. **Environment** — `env.json`; product binary byte-identity vs plan9's
   `1b70b32f46d9218e…` ✓ (already matched; no rebuild); campaign-binary
   hashes recorded (not byte-reproducible per plan10 §9.2; behavior risk
   carried by A2 — vacuously here).
4. **Strata** — `state/strata.json` committed before the ladder.
5. **SMOKE → R1′ → R2′ → R3′** — R3′ decision recorded in
   `state/r3_decision.json` (fired).
6. **Merge** — `state/merge.json`, `state/master_state_merge11.json`.
7. **Analysis** — `state/analysis.json` (per-rung labels, covariates,
   gate), `state/m2_corpus.csv` (168 rows: the full per-leaf per-rung
   label/covariate record), `state/ledger11.json` + `ledger11.csv`
   (plan10's 698-row ledger extended with plan11 marginals).
8. **Cleanup** — see §8.

## 7. Deviations and problems encountered

1. **Three driver bugs found and fixed before any registered run** (SMOKE
   caught one, code review the others): (a) SMOKE budgets keyed by rung
   tag raised `KeyError: 'S0'` — fixed with a `*` default; (b) R2′ would
   not have excluded R1′-resolved leaves (empty prior-tags tuple) — the
   bug never fired because R1′ resolved nothing, but it would have
   corrupted any rung construction with conversions; (c) `ladder` ran R3′
   unconditionally instead of only via the contingent rule. All fixed
   before the ladder; the committed driver is the fixed one.
2. **R3′ launch interrupted ~60 s in and recovered cleanly (not a
   datapoint):** a tooling timeout killed the driver mid-`r3` while R3′
   workers were running. Recovery: worker processes were killed with the
   process group (zombie entries only); R2′ TT dumps were preserved; zero
   R3′ result files existed (workers dump only at close); stale job/claim
   files are wiped by `prepare_rung` on re-run. R3′ re-launched clean via
   the same registered `r3` command; the committed `rung_R3.json` is the
   clean full run. No state pollution.
3. **R3′ fired** (rule: 1.135 h elapsed + 2.026 h projected ≤ 4.5 h,
   locked constants); recorded in `state/r3_decision.json` either way, as
   registered. Actual R3′ wall 1.73 h (observed rate ≈ 1.23× the locked
   projection).
4. **Merge pass as a registered no-op** (§5 above): with zero decisive
   results the master is not invoked; the close is committed as the
   verified identity of the seed. Reason recorded in `state/merge.json`
   (`master_reason`). This is an interpretation of the registered pass for
   the empty-input case, not a skip: the pass ran, copied 0 results, and
   verified the identity invariant.
5. **The SMOKE run is excluded from all metrics** (registered: not a
   datapoint); its 1.6 M child-evals are recorded separately in
   `analysis.json`.

## 8. Cleanup (measurement conventions)

Session dirs `/tmp/plan11_sweep`, `/tmp/plan11_smoke` (deleted at its own
close), `/tmp/plan11_merge` and all TT dumps deleted at close; raw captures
live under `logs/` (gitignored). Committed record: `sweep11.py`,
`README.md`, `env.json`, `state/*.json` (strata, rung_R1/R2/R3,
arm_SMOKE, r3_decision, merge, master_state_merge11, ledger11,
analysis) + `state/ledger11.csv`, `state/m2_corpus.csv`. No transcripts,
no `.tt` files committed.

## 9. Unresolved parts / missing tests / next steps

- The M2 substrate question is closed by measurement, not left open: the
  advisory-dn channel produced no positives at any registered budget; no
  predicate can be validated on this frontier at affordable n.
- Stage 1 (A5.4 coverage-race targeting) is the initiative's remaining
  registered direction (owner-decided active-POC state). Its design must
  not re-tread the falsified A5 scheduler/budget family (plan6/plan10).
  Honest pricing from this plan: the frontier body converts ~1 leaf per
  ≈ 169 G evals at deep budgets (plan10 M6) and the cheap class is already
  harvested (§4 here) — a Stage-1 POC's value hinges on master-side
  targeting of the proof-bearing child, which has no measured substrate
  either (plan6). The initiative may therefore deserve a dormancy decision
  rather than a Stage-1 draft; that is an owner call, not this plan's.
- The warm-regime covariate flattening (§2) is recorded for any future
  signal work: warm advisory dn partially loses the committed-dn ordering.
- No campaign-code or solver tests were touched (no code changes);
  `make test` not re-run per the plan's non-goals — the test surface was
  the sweep's own machinery checks (all green) and the A2 merge pass
  (vacuous).
- The certificate v1 + soundness contract is preserved in §11 A6b; a
  revival must re-validate its premises (the M2 gate would need a
  positive-bearing corpus that does not exist today).

## 10. SESSION COMPLETE block

Deliverables: plan11 executed end to end (strata → SMOKE → R1′ → R2′ →
contingent R3′ → merge → analysis), report11.md written,
`campaign_architecture.md` §11 A6b amendment written, `initiative.md`
Status close entry added, `docs/plans/README.md` solve row updated,
cleanup confirmed.

SESSION COMPLETE
- plan11 executed: M2 gate = SUBSTRATE_EMPTY (P = 0 positives; 182.3 G
  evals; machinery 0-defect throughout); report11.md written; §11 A6b
  amendment + initiative/index updates written; gate `make test` not
  re-run (no code changes, per plan non-goals); measurement record
  committed under measurements/plan11/.
Follow-up options:
  1. Owner decision session: read report11.md + initiative.md and decide
     Stage 2's halt formally — either draft plan12 as the Stage-1 (A5.4)
     coverage-race POC with honest pricing from §9, or re-close `solve`
     as DORMANT with the sharpened reopener record (recommended: the
     dormancy decision, since both remaining mechanisms now lack a
     measured substrate).
  2. Alternative: a cheap probe plan sizing the A5.4 substrate directly
     (does master-side targeting identify proof-bearing children at all?)
     before any Stage-1 draft — only worth it if the owner wants to keep
     the campaign line open.
