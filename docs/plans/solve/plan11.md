# Plan 11: Phase 3, Stage 2a — advisory-dn censor-signal validation at larger n + the pre-registered certificate soundness contract

Initiative: `solve`. Executes **Phase 3, Stage 2 (A6b), first half** of the
2026-10-03 owner decision (recorded in `initiative.md` Status): Stage 2
(budget-aware early-censor certificates, worker-side) is gated on
"larger-n validation of the advisory-dn signal (currently n=2) + the
path-scoped soundness argument" (report10 §11; `campaign_architecture.md`
§11 A7). This plan delivers exactly that gate material as **Stage 2a**:

- **M2** — the larger-n censor-label corpus: a stratified sample of the
  frozen frontier's censored body, rung-laddered at deep budgets, giving
  the advisory-dn channel more positives (or measuring their absence).
- **the certificate soundness contract** — the pre-registered, path-scoped
  argument that plan12 must carry verbatim into the implementation plan.

Stage 2a is registered as **S** (measurement-only): the harness is the
existing measured-sound plan9/plan10 campaign machinery driven by a new
driver, with **no solver, lib, or campaign-code changes** (the only new
file is the driver under `measurements/plan11/`, Python 3 stdlib, per the
plan8–10 precedent). **The certificate implementation itself is *not* in
this plan**: it is plan12 (Stage 2b), gated on this plan's M2 verdict —
the stage-internal gate order follows the initiative Status ("gated on
larger-n validation … + the path-scoped soundness argument").

Split rationale (recorded at drafting): the full Stage 2 (implementation +
verifier + SMOKE + warm-resume arms at 1 h each) cannot fit one 6 h
session alongside the corpus sweep; the M2 verdict changes *what* plan12
builds (GO: predicate-consuming skip policy; NO-GO/MARGINAL: nothing —
Stage 2 halts), so drafting plan12 before M2 is not meaningful.

Per repo convention the final task is `report11.md`.

## 1. Background (self-contained)

**The reopener record.** plan6 (A5) falsified the registered
scheduler/budget family. plan9 (A6) built the constraint-4
checkpoint/resume machinery and measured ρ = 0.0 (Δfacts 30/0/0/0);
the discriminator fired the *resume-mechanics* mode with a recorded
resolution caveat (at 8M everything pins at cap, so a warm discount
could only manifest below cap). plan10 (Stage 0) measured the full 698-leaf
frontier at an 8M/32M/128M warm ladder: **ΔC = 2 → CENSORING GO** —
budget escalation is refuted as a conversion mechanism (2 conversions in
110.9 G child-evals), censoring is ≈ 100% cap-pinned at every rung, and
the cold-control arm **measured a warm-state discount below cap**
(warm marginal / cold spend 0.72/0.86; one conversion unreachable cold at
its resolving budget) — exactly the regime the A6 caveat recorded as
unresolvable. The gate therefore selected **Stage 2 (A6b early-censor
certificates)**, with a measured but thin substrate: pre-rung advisory
root_dn separated the rung-resolved leaves from the censored mass at
**AUC 0.87 (R2) / 0.99 (R3)**, combined-signal AUC 0.99/1.00 — on
**n = 1 positive per rung**. The binding qualification (report10 §4):
"any certificate design must validate the signal at larger n and carry
the pre-registered path-scoped soundness argument". Stage 1 (A5.4) is not
gate-selected; the M1 sub-reading (all 38 conversions on never-searched
leaves; the dispatcher's 104-leaf sample produced zero at 4× budget) is
its measured motivation and is **out of scope here** (non-goal §6).

**Mechanism under validation.** The A6b lever: a warm-resumed worker
re-queues censored leaves at the same budget and re-burns ≈ 100% of the
cap for zero facts (plan9 §4; plan10 §3). An early-censor certificate
would let the campaign *avoid* the re-burn: the worker's restored advisory
bounds are used as a predicate at job start; when the predicate says this
(job, budget, lineage) will re-censor, the job is skipped or cut short and
a certificate is recorded, and the master's spend-aware re-queue policy
redirects the saved budget to escalation/coverage. The predicate's
direction per report10 M3: **conversions sit in the upper tail of
pre-run advisory root_dn (low pn)** — so the certificate classifies the
*low-signal mass* as expected-censor (skip/park) while the *upper tail*
is escalated. Whether that separation survives beyond n = 2 is what M2
measures. Note the plan10 corpus covariate regime: its rungs were warm
(--tt-load across rungs), so pre-rung advisory was the prior rung's
post-run bound. This plan's rungs start from a *fresh* worker set (the
plan10 TT dumps were deleted at close, by convention), so R1′ runs cold;
the warm rungs R2′/R3′ restore the prior rung's post-run advisory and are
the **primary regime** for certificate purposes (a certificate is consumed
warm). R1′ is analyzed with the committed (plan9-frozen) advisory as its
covariate and registered as the secondary regime.

**Why a sample, not the full frontier.** The frontier converts ~1 leaf
per rung at the plan10 budgets (M6: next doubling ≈ 1 conversion per
≈ 169 G evals). A full-frontier deep ladder is unaffordable (660 × 1 G =
660 G evals ≈ 24 G nodes ≈ ≥ 12 h). A stratified sample concentrates the
run budget where the hypothesis under test predicts positives (the upper
tail), making the *falsification* direction affordable too: if the
upper tail produces no positives, the n=2 substrate is falsified at
affordable cost.

## 2. The M2 corpus sweep harness (driver-only; no code changes)

Driver: `measurements/plan11/sweep11.py` (Python 3 stdlib only), following
the plan10 `sweep.py` conventions (driver-written job files, bare
`campaign_worker` processes, master bypassed, per-rung close = barrier →
`stop` file → bounded wait for exits + complete TT-dump set, warm relaunch
with `--tt-load`, memory sampler, oom watch). Command table in
`measurements/plan11/README.md`.

**Sampling frame.** The 660 `Open` leaves of plan10's committed merged
close (`../plan10/state/master_state_merge.json`: 728 leaves / 68 Won /
660 Open / 27 children, `children_resolved` 0/27). Covariates per leaf:
committed advisory `dn`/`pn` and `work` from plan9's frozen S1 close
(`../plan9/state/master_state_s1.json`, per the leaf-identity rule
path = child move + reply move), plus plan10's ledger cumulative spend
(`../plan10/state/ledger.json`). The 104 *touched* open leaves
(work > 0, committed advisory dn ∈ [1,113 … 146,201]) are the certificate
mechanism's actual target population (a certificate is only ever consumed
on a previously-censored, warm line); the 594 untouched leaves (dn = 1
build prior) are a different class (the cheap-class/coverage story).

**Registered strata (deterministic; exact leaf lists computed by the
driver and committed as `state/strata.json`):**

| stratum | n | selection rule | role |
| --- | --- | --- | --- |
| H | 24 | top-24 open leaves by committed advisory dn (ties → higher work), **force-including the two c1g5 near-misses `f7f6`, `g8f6`** | the hypothesis's predicted-positive mass |
| M | 12 | touched-open leaves at the ~55th–75th percentile of the touched set's committed dn | mid-signal |
| L | 12 | touched-open leaves at the ~15th–35th percentile of the touched set's committed dn | low-signal (expected-censor mass) |
| U | 8 | untouched open leaves (work = 0), round-robin over children by static rank | cheap-class control — **analyzed separately, excluded from the primary gate** |

Primary corpus = H ∪ M ∪ L (48 leaves); U is a control (8 leaves).

**Ladder.** R1′ = 256M, R2′ = 1G (R1′ survivors), contingent
R3′ = 2G (R2′ survivors). Workers = 4, stable per-leaf worker affinity
across rungs, round-robin job naming (`w{w}_{rung}_{n}`), fresh workers at
R1′ (no `--tt-load`), warm relaunch at R2′/R3′. Contingent rule (fixed
before any run): fire R3′ iff `elapsed + survivors × 2G / κ / R_camp ≤
4.5 h`, with κ = 27.5 and R_camp = 558,529 nodes/s (the locked plan8
constants, conservative vs plan10's observed ≈ 3.2× — the R4-not-fired
lesson: projections use locked values). Decision recorded in
`state/r3_decision.json` either way.

**SMOKE** (not a datapoint): 8 leaves × 100k evals, one rung, full
close/restore protocol, before the ladder.

## 3. Certificate design + path-scoped soundness contract (pre-registered)

This section is the stage-2 gate material the initiative Status demands.
It is written **before** any run and before plan12; plan12 must carry it
verbatim (amendments only as registered deviations), and the report
amends `campaign_architecture.md` §11 with it (new sub-item **A6b**).

**Certificate (v1) definition.** A worker-produced record attached to a
*censored* result for job (global path P, budget B):

```
path_digest       SHA-256 of the canonical global path (replay-verifiable)
budget_evals      B
pre_advisory      (pn, dn) restored from the worker's TT at job start
post_advisory     (pn, dn) at job close
evals_spent       actual child evals consumed by the job
lineage_digest    SHA-256 of the worker's TT dump at the session boundary
                  the job ran in (ties the claim to the exact TT lineage)
session_id, worker_id, job_seed
outcome           Censored   (decisive results never carry certificates —
                             they go through the existing A2 fact pipeline,
                             unchanged)
```

**What a certificate may license — and nothing else.** The master's
spend-aware re-queue policy may skip re-queuing leaf L at budget ≤ B on
lineage (worker w, session lineage ℓ) when a certificate at
(L, B, w, ℓ) exists and the plan12-validated predicate holds. It may not:
change L's status class (L stays `Open`), compose into proof state, gate
any A2-verified fact, suppress escalated budgets (certified leaves are
first in the escalated queue, so suppression never blocks the ladder), or
apply across workers / across session lineages.

**What a certificate may not claim.** Anything about L's game-theoretic
value; anything about budgets > B; anything about a lineage other than its
recorded one; anything about a different path.

**Soundness argument (path-scoped provenance).**

1. *Composition surface unchanged.* Certificates gate **work allocation
   only**. The artifact pipeline (per-fact A2 verification at merge; final
   replay validation) never sees a certificate, and no new fact class
   exists. The campaign's constraint 1 (nothing unverified reaches the
   artifact) is untouched by construction.
2. *Path scope.* The claim is indexed by the exact job path P (the
   campaign root → leaf line, clock and repetition context included); the
   master re-derives P by replay (existing A2 machinery) and rejects on
   mismatch. No repetition-dependent result crosses any boundary — none
   is produced at all (a censored result's "draw-bounded-by-budget" status
   is already a non-fact under the standing contract).
3. *Lineage scope.* The prediction "warm re-run at budget B re-censors" is
   conditioned on the worker's TT state at lineage ℓ. Session-boundary TT
   dumps are content-addressed; a consumed certificate must match the
   consuming session's restored dump digest. Cross-job TT pollution
   *inside* ℓ is deliberately **included** in the claim's scope — the
   claim is about the lineage as a whole, which is precisely what a warm
   re-queue actually runs on — and that inclusion is why the scope must
   not be widened (a certificate consumed on a different worker or after
   an unrecorded TT evolution is out of contract and must be rejected).
4. *GHI.* Worker journals keep the standing contract
   (`dfpn/research_ghi_journal.md`); certificates cache no search result;
   the skip is a scheduling decision. The GHI hazard class is exactly the
   plan9 machinery's, which is measured sound (SMOKE 7/7, restore
   integrity 0 mismatches, re-drain 0 verify failures).
5. *Fail-open, always.* Any verification failure, missing field, digest
   mismatch, or unknown-certificate condition must fail **open**: the leaf
   is re-queued normally. Suppression may never be the default.
6. *Yield risk (not soundness) — false censor.* Suppressing a re-queue
   that would have converted loses a fact. Handled by: (i) the M2 gate
   (this plan) before any implementation; (ii) plan12's mandatory on-line
   probe — every 16th certificate consumption is *not* honored and runs at
   full budget, measuring the online false-censor rate, with auto-disable
   if it exceeds 1/16; (iii) escalation priority (above).

**Verifier (plan12 deliverable, specified here).** A campaign-side
function checking a certificate against the master state: (a) path replay
legality from the campaign root (A2 discipline, reused); (b) recorded
`pre_advisory` equals the master state's per-leaf advisory record for that
leaf; (c) digest checks (path, lineage); (d) outcome class = Censored and
`evals_spent ≤ budget_evals`; (e) session/worker/seed present and
well-formed. Any failure → rejected, fail-open. **Registered
corrupted-certificate test:** the implementation must include a test
injecting at least three tampering classes (altered advisory dn, altered
path/digest, altered budget or spent-evals overclaim) and asserting
rejection for each; a certificate accepted by all checks but *not*
re-derivable from the recorded lineage must also be rejected (lineage
digest mismatch test).

**Plan12 POC gates (inherited verbatim from the 2026-10-03 owner
decision, initiative Status):** "the replay verifier accepts every
produced certificate and rejects a deliberately corrupted one;
warm-resume Δfacts > 0 (POC-level ρ > 0)." Standing constraints also
inherited: campaign code stays `examples/`-side; product binary
byte-identity re-checked; POC-level gates only (full verdict bands stay
de-registered until a later verdict stage).

## 4. Metrics and the pre-registered M2 gate (fixed before any run)

Per rung r ∈ {R1′, R2′, R3′} and each primary leaf, record:
outcome (`Win` / `Censored`; any `Lost` is registered as a decided label
and is a machinery-noteworthy event), `evals_spent` (marginal,
`begin_run()`-reset), pre-rung advisory (pn, dn), post-rung advisory.

**Covariate regime matching (binding).** R1′ covariate = committed
advisory dn from plan9's frozen S1 close (fresh regime; secondary
analysis). R2′/R3′ covariate = prior rung's post-run advisory dn
(warm regime; **primary analysis** — it matches the certificate
consumption regime). Cumulative work is registered as a *control*
covariate (plan10 M3 measured AUC 0.000 — it must not separate; if it
does, the covariate pipeline is defective and the gate verdict is void
until the defect is fixed and the affected rung re-analyzed).

**Signal metrics.** Per warm rung with ≥ 1 positive: AUC and Youden
threshold of pre-rung advisory dn (positives = upper tail), plus the
sign-corrected combined rank-mean (dn ascending-rank + pn descending-rank)
as secondary. Pooled AUC over warm rungs by rank-attachment.

**θ procedure.** Pool the primary positives by their regime-matching
pre-rung advisory dn. θ = the minimum pre-rung dn among R1′ positives if
R1′ produced any (fresh-regime threshold, then R2′/R3′ are held-out);
otherwise θ = the minimum pre-rung dn among R2′ positives with R3′ (if
run) as held-out; if no held-out rung exists, the separation analysis is
in-sample-only and the report must carry that caveat (a GO verdict under
this branch additionally requires pooled AUC ≥ 0.85).

**Skipable mass.** S(θ) = fraction of R2′ primary entrants with pre-rung
dn ≤ θ. (θ captures the expected-censor mass a certificate policy would
park; the upper tail above θ is what escalation targets.)

**M2 gate bands:**

- **M2-GO (substrate validated; plan12 draftable):** P ≥ 3 positives
  ∧ zero held-out false negatives ∧ pooled warm-regime AUC ≥ 0.80
  ∧ S(θ) ≥ 0.40 ∧ fewer than 2 directional inversions.
  *Directional inversion* = a positive whose covariate sits in stratum L's
  registered percentile range (or below it) — i.e., the signal's claimed
  direction fails where it should hold.
- **M2-NO-GO (substrate falsified at larger n; Stage 2 halts):** P ≥ 3
  ∧ (pooled warm-regime AUC < 0.60 ∨ ≥ 2 directional inversions ∨ a
  held-out false negative).
- **M2-MARGINAL (unvalidated; no plan12 draft):** 0 < P < 3, or P ≥ 3
  with AUC ∈ [0.60, 0.80) and no inversions/FN. Judgment call is *not*
  left open: the registered action is halt + sharpened reopener record
  (the corpus is the honest statement that the substrate cannot be
  validated at affordable n on this frontier).
- **Substrate-empty halt:** P = 0 at the full ladder including fired
  R3′ → same as M2-MARGINAL with the stronger wording: the advisory-dn
  channel produced no positives to validate at all.

A negative verdict is a valid completion (per the session protocol); it
closes Stage 2 with the sharpened record and leaves the initiative in its
owner-decided active-POC state with Stage 1 (A5.4) as the remaining
registered direction.

**Merge pass (A2 discipline; bookkeeping, not a gate).** All decisive
results → `campaign_master --resume` over plan10's merged state
(`--job-seed 11`), every fact globally re-verified; verify failure =
machinery defect, abort and investigate. Δfacts recorded for the
initiative record (expected: U-stratum cheap conversions, possibly an
upper-tail conversion — the first since plan10).

## 5. Tasks

1. **Soundness contract** — §3 above is the deliverable; carried into
   plan12 verbatim; `campaign_architecture.md` §11 A6b amendment at the
   report.
2. **Driver** — `measurements/plan11/sweep11.py` + `README.md` (command
   table, registered shape, binary-identity section), reusing plan10's
   driver structure (close protocol, `stop` semantics — lowercase file,
   report10 §9.3 lesson — restore probing, memory sampler).
3. **Environment** — `env.json`; product binary byte-identity check vs
   plan9's registered `1b70b32f…` hash; if diverged, rebuild with
   `CARGO_PROFILE_RELEASE_LTO=thin` before anything runs (plan10 §9.1
   lesson); campaign-binary hashes recorded (byte-reproducibility is not
   achievable per plan10 §9.2 — behavior risk carried by the A2 merge).
4. **Strata build** — `state/strata.json` (leaf lists + covariates +
   selection provenance), committed *before* the ladder runs.
5. **SMOKE** → fix any machinery defect → **R1′** → **R2′** → contingent
   **R3′** decision (recorded either way). Rungs strictly sequential,
   nothing else on the box.
6. **Merge pass** (A2) over all decisive results → `state/merge.json`,
   `state/master_state_merge11.json`.
7. **Analysis** — `state/analysis.json` (per-rung labels, covariates,
   AUC/Youden, θ, S(θ), gate verdict, M2 corpus CSV) + `state/ledger11.json`
   (per-leaf spend bookkeeping, plan10 ledger extended).
8. **Cleanup** — session dirs `/tmp/plan11_*` and TT dumps deleted; raw
   captures under `logs/` (gitignored); commit list per conventions
   (driver, README, env.json, state/*.json — no transcripts, no `.tt`).
9. **`report11.md`** — verdict, deviations, cleanup confirmation,
   initiative.md history close entry; update the `docs/plans/README.md`
   `solve` row's Next-lever text if the verdict changes what is next.

## 6. Non-goals

- **No certificate implementation** (worker predicate, master re-queue
  policy, verifier, corrupted-certificate tests) — plan12, gated on M2.
- **No A5.4 / Stage-1 work** (dispatcher targeting, cheap-class harvest
  designs) — not gate-selected; the U stratum is a control, not a
  harvest mechanism, and its results gate nothing.
- **No solver, lib, or campaign-code changes**; no product-surface
  changes; `make test` is therefore not re-run (same convention as
  plan10) — the test surface is the sweep's own machinery checks.
- **No full verdict bands** (ρ ≥ 0.7 etc. stay de-registered; POC-level
  gates only, per the 2026-10-03 owner decision).
- **No cold-control arm** — plan10 priced the warm discount; M2's labels
  are per-rung with regime-matched covariates and need no second control.

## 7. Budget

Worst case (contingent fired): R1′ 56 × 256M = 14.3 G evals; R2′ ≤ 55 ×
1 G = 55 G; R3′ ≤ 54 × 2 G = 108 G → ≤ 178 G evals ≈ 6.5 G nodes
(κ = 27.5) ≈ 3.2 h at the locked R_camp, ≈ 2 h at plan10's observed rate;
plus merge + analysis overhead ⇒ **≤ 4 h of the 6 h cap**. R1′+R2′ alone
(2.5 G nodes) ≈ 1.2 h. The contingent projection rule (§2) protects the
cap; if it does not fire, the analysis proceeds on R1′/R2′ labels and the
smaller corpus is recorded honestly.
