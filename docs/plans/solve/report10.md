# Report 10: Phase 3 Stage 0 — frontier budget-completion sweep (scheduling-vs-censoring diagnostic)

Initiative: `solve`. Executes `plan10.md` (the 2026-10-03 owner decision's
Stage 0). Runs of 2026-10-03 21:58–2026-10-04 00:20 UTC, strictly
sequential. Harness: `measurements/plan10/sweep.py` (Python 3 stdlib,
command table in `measurements/plan10/README.md`), environment `env.json`
(4-CPU quota, 8 GiB cgroup, oom_kill 0 throughout; product binary
byte-identical to plan9's record, §5). **No solver, lib, or campaign-code
changes** — the sweep is the plan9 campaign machinery driven in a new way
(driver-written jobs, master bypassed); the merge pass drives the existing
binaries.

## TL;DR

- **Stage-0 gate: CENSORING GO (ΔC = 2, with an M3 substrate — n=2,
  heavily qualified).** Budget escalation on the full frontier is
  measured nearly inert: C(8M) = 36, C(32M) = 37, C(128M) = 38 of 698
  open leaves (ΔC = 2 ≤ 5), with censoring 100%/100%/99.85% cap-pinned
  at the three rungs. Per the pre-registered bands this selects
  **Stage 2 (A6b early-censor certificates)** — and unlike plan9, the
  certificate substrate is no longer empty: the pre-rung advisory root_dn
  separates rung-resolved from rung-censored (AUC 0.87/0.99), and the
  cold-control arm **measured a warm-state discount below cap** —
  the exact regime report9 recorded as unresolvable.
- **The dispatcher-biased frontier record is superseded (A7):** all 38
  conversions are on never-searched leaves; the 104 dispatcher-touched
  leaves produced **zero** conversions at 4× budget. The incumbent
  selection policy is measured to be the binding constraint on the cheap
  class — C(8M) = 36 facts sit in its blind spot at a median cost of
  ~1.3k evals (min 0). This is Stage-1 (A5.4) design input **in addition
  to** the gate verdict, not instead of it: the cheap class is a
  *coverage* problem, the rest of the frontier is a *censoring* problem.
- **M4 = 0: no root-level conversion.** `children_resolved` stays 0/27;
  c1g5's two open leaves absorbed 268M/260M cumulative evals (historical +
  sweep) and still censor. The 2026-09-25 frozen-frontier record stands.
- Machinery: 0 job errors, 0 verify failures (A2 never fired), 0
  deviations, 0 oom kills, clean restore/dump at every rung boundary.

## 1. What ran

Rung ladder over the 698 `Open` leaves of plan9's frozen S1 close
(sanity-checked 728/30/698/27 from the committed state), driver-written
jobs (`w{w}_{rung}_{n}`, round-robin over 4 workers, stable per-leaf
worker affinity), master bypassed; per-rung close = barrier → STOP →
bounded wait for exits + complete TT-dump set; warm relaunch with
`--tt-load` (restore probed: 2.3–2.4M solved + 1.7–1.9M unsolved records,
0 probe mismatches, file == table counts at every boundary).

| stage | budget/leaf | jobs | wall | nodes | child evals | outcome |
| --- | --- | --- | --- | --- | --- | --- |
| R1 | 8M | 698 | 316.7 s | 201.6M | 5.30 G | 36 win / 662 draw |
| R2 | 32M | 662 | 1256.9 s | 798.7M | 21.17 G | 1 win / 661 draw |
| R3 | 128M | 661 | 4947.4 s | 3177.4M | 84.47 G | 1 win / 660 draw |
| R4 (contingent) | — | — | — | — | — | **not fired** |
| cold control | 32M/128M | 2 | 16.5 s | 0.55M | 40.0M | 1 win / 1 draw |
| merge pass | — | 38 re-drained | 60.0 s | — | — | 0 verify failures |

Total measured wall 1.83 h (registered worst case 2.2 h). Observed
campaign rate 636–642k nodes/s (m2 ≈ 3.21 vs the locked 2.82; κ_obs ≈
26.3–26.6 vs locked 27.5 — this environment runs measurably faster than
the plan8 constants; all §7 arithmetic used the locked, conservative
values). Peak tree-RSS 1.06/1.43/1.43 GB per rung — far under the
watermarks. **R4 decision (recorded in `state/r4_decision.json`):**
survivors 660, elapsed 1.83 h, projected 6.11 h; 1.83 + 6.11 > 5.0 h ⇒
not fired.

## 2. M1 — completion curve

| B | decided | won | lost |
| --- | --- | --- | --- |
| 8M (R1) | 36 | 36 | 0 |
| 32M (R2) | 37 | 37 | 0 |
| 128M (R3) | 38 | 38 | 0 |

- ΔC = C(128M) − C(8M) = **2**.
- **Sub-reading: C(8M) on the 594 untouched leaves = 36/36 of the R1
  harvest.** The touched-104 conversion count is 0 at every rung. The
  cheap class is entirely in the dispatcher's blind spot.
- Cost structure of the harvest (`state/ledger.json`): 6 conversions cost
  ≤ 955 evals (min: `c1h6 g7g5` = 0 evals, an immediate terminal), median
  R1 win ≈ 1.3k evals; the two R2/R3 conversions cost 23M and 60M warm
  marginal (cumulative sweep 31M / 100M). No LOST results anywhere —
  every conversion is a refutation of a black reply.

## 3. M2 — censor-depth distribution (marginal per-job child evals)

| rung | n | cap-pinned | median | deciles (d10…d90) |
| --- | --- | --- | --- | --- |
| R1 | 662 | 1.000 | 8,000,014 | 8.00M…8.00M |
| R2 | 661 | 1.000 | 32,000,014 | 32.0M…32.0M |
| R3 | 660 | 0.9985 | 128,000,013 | 128.0M…128.0M |

The report9 "≈ 100% pinned at 8M" datum re-measured at every rung: work-to-censor
is budget-pinned at all three scales. Budget escalation extracts almost
nothing from the censoring frontier — it buys exactly 2 conversions in
110.9 G child-evals (M6).

## 4. M3 — advisory-signal predictivity (A6b substrate)

For leaves entering a rung (warm bounds = prior rung's post-run advisory;
work = historical + cumulative sweep), label = resolved at that rung:

| signal | R2 (entered 662, resolved 1) | R3 (entered 661, resolved 1) |
| --- | --- | --- |
| root_pn | AUC 0.027, Youden 0.027 | AUC 0.091, Youden 0.091 |
| root_dn | **AUC 0.873, Youden 0.873** (thr 9,626) | **AUC 0.992, Youden 0.992** (thr 44,073) |
| cumulative work | AUC 0.000, Youden 0.000 | AUC 0.000, Youden 0.000 |
| combined (sign-corrected rank mean) | AUC 0.994, Youden 0.994 | AUC 1.000, Youden 1.000 |

The two conversions sat in the extreme upper tail of pre-rung advisory
root_dn (9,626 with most censored leaves ≤ 9,626 at R2; 44,073 vs ≤
44,073 for all but 5 at R3); root_pn separated *inversely* (resolved
leaves had low pn); cumulative work does not separate.

**Qualification (binding for any Stage-2 design): n = 1 positive per
rung.** With a single positive, best-Youden trivially achieves near-perfect
separation for any signal concentrated in a tail; AUC 0.87–0.99 on n=2
total positives is a *measured substrate candidate*, not a validated
discriminator. The honest statement: the advisory-dn channel showed a
consistent, strong, directionally-sensible signal on the only two
conversions the frontier produced; a Stage-2 POC must validate it at
larger n before certificates are trusted. This is why the gate verdict's
substrate clause is reported with the n=2 caveat rather than claimed as
proven.

**Cold-control arm (fired, ΔC = 2 > 0; sample = the 2 R2/R3 conversions —
the stratified-sampler ceiling):**

| leaf | resolving rung (warm marginal) | cold re-run at same budget |
| --- | --- | --- |
| b1d2 e8d7 | R2, 22,989,099 evals | **draw** — censors at 32M (32,000,006 evals) |
| b1a3 e8d7 | R3, 60,174,090 evals | win, 69,902,397 evals |

Warm marginal / cold spend: 0.72 and 0.86 (median 0.79). **A warm-state
discount measurably manifests below cap** — including the strong form:
the b1d2 conversion is *unreachable cold* at its resolving budget. This
prices the report9 budget-cap resolution caveat: at 8M everything pins
(the report9 regime), but at the budgets where conversion happens, warm
state is load-bearing. Method note: the cold arm ran with
`--retention off` (per-job fresh `Search`), which makes each cold job an
exact cold measurement — stronger than the plan's "fresh workers, no TT
restore" wording (a shared worker session would have polluted later cold
jobs with earlier ones' TT state); recorded here as the registered
implementation reading.

## 5. M4 — root-level conversion

Merged close (`state/master_state_merge.json`, A2-verified): leaves_won
68 (30 plan9 + 38 sweep), leaves_lost 0, leaves_open 660,
**children_resolved 0/27**, children_refuted 0. Per-child "open wins
needed": e2e4 10, e2e3 8, d1d3 10, c2c4 12, d1d2 10 … c1g5 2. The
`c1g5` near-miss record (`f7f6`, `g8f6` — the two leaf-wins plan10 §1
identified) did not convert: their cumulative spend reached 268M / 260M
evals (historical 100M/92M + sweep 168M each) and both censor at 128M
warm. The 0/27 frozen-frontier record stands; the sweep produced the
first root-level conversion *attempt data* but no conversion.

## 6. M5 — cumulative-spend ledger / M6 — frontier pricing

Ledger: `state/ledger.json` + `ledger.csv` (728 rows: path, untouched
flag, historical work = max over the committed plan9 closes, per-rung
marginals, cumulative sweep spend, resolving rung, outcome, merged
status). Historical-work limitation as registered: plan8 states carry
aggregates only; S1/S2/S3/RC closes are cumulative per-leaf and are
maxed.

| metric | value |
| --- | --- |
| total sweep spend | 110.95 G child evals ≈ 4.03 G nodes (est. κ 27.5) |
| per rung | R1 5.30 G / R2 21.17 G / R3 84.47 G evals |
| spend split | untouched leaves 93.5 G / touched 17.5 G |
| conversions per G eval | R1 6.8 / R2 0.05 / R3 0.012 |
| geometric projection (naive) | next doubling (256M × 660): ≈ 1 conversion for ≈ 169 G evals |

M6 pricing: the frontier's marginal cost per fact at the measured ladder
is ~2.9 G evals per additional conversion and rising geometrically; the
36-fact cheap harvest, by contrast, cost 0.19 G evals (3.6% of R1's
spend). Budget escalation is measured to be the wrong lever for
everything but the tail — the harvest is where the facts are cheap.

## 7. Registered cross-checks

- **plan9-common 12 leaves** (recovered from the committed closes as the
  work-grew-in-S2∧S3∧RC set, exactly the affinity-locked re-queue
  targets): sweep R1 median marginal 8,000,019 evals, cap-pinned 1.0 —
  vs plan9 fresh 7,989,618.7 / 0.997. Dispatcher-independent replication
  of plan9's fresh censoring. ✓
- **No Won-leaf jobs:** 0 violations (the 30 already-Won leaves were
  excluded by construction and never targeted). ✓
- **job_errors 0** across all rungs, the cold arm, and the merge; 0
  missing results; 0 deviations. ✓

## 8. Gate verdict

```
ΔC = C(128M) − C(8M) = 38 − 36 = 2  →  CENSORING band (ΔC ≤ 5)
M3 advisory separation present (root_dn AUC 0.87/0.99; combined 0.99/1.00)
  → verdict CENSORING_GO, substrate qualified n=2
M4 = 0 (headline structural finding of absence)
M1 sub-reading: 36-fact dispatcher-blind-spot harvest (Stage-1 input)
```

**Stage 0 decision: CENSORING GO.** Budget escalation is refuted on this
frontier as a conversion mechanism; the registered next lever is Stage 2
(A6b budget-aware early-censor certificates, worker-side), now with a
measured — though n=2 — substrate (advisory-dn separation + below-cap
warm discount). Stage 1 (A5.4 coverage-race targeting) is *not* selected
by the gate bands, but the M1 sub-reading is a measured argument for
folding cheap-dispatch targeting into any Stage-1-adjacent design: the
dispatcher's 15% sample is measurably the wrong 15% for the cheap class.

## 9. Deviations and problems encountered

1. **Product-binary hash divergence found and resolved before the run:**
   `target/release/atomic_solver` hashed `86635244…` ≠ plan9's
   registered `1b70b32f…`. Cause: an intervening `make test` had rebuilt
   the binary plain (no `CARGO_PROFILE_RELEASE_LTO=thin`); the source is
   unchanged since plan9's commit `c6a30b0` (git-verified: `src/` and
   `examples/campaign*` are bit-identical). Rebuilt with the LTO=thin
   profile → hash matches plan9's record byte-for-byte. Recorded in
   `env.json`.
2. **Campaign binaries not byte-reproducible:** neither the current tree
   nor an isolated `git archive c6a30b0` build (plain and LTO=thin)
   reproduces plan9's recorded campaign hashes — plan9's exact build
   context is not recoverable from the commit, and the intervening
   proofdb work added workspace dependencies that change cargo's
   fingerprint metadata for untouched sources. Mitigation: sources are
   git-identical and the product library is byte-identical; the behavior
   risk is carried by the merge pass's A2 re-verification (0 failures).
3. **Driver-fix re-run of R1 (SMOKE-style, not a datapoint):** the first
   R1 attempt was aborted after two driver bugs surfaced — (a) the close
   protocol wrote `STOP` while the worker's constant is lowercase
   `stop` (`examples/campaign/mod.rs:29`), so workers never self-closed
   and no TT dumps were written; (b) the crash-recovery clock ran from
   rung start instead of first-claim detection, firing 359 spurious
   recoveries. Both fixed; the polluted attempt's artifacts were deleted
   and R1 re-run clean (the committed `rung_R1.json` is the clean run).
   Lesson recorded: worker STOP semantics are case-sensitive; the plan9
   close protocol's "STOP" terminology means the file `stop`.
4. **R4 not fired** (rule recorded above). **Cold arm sample = 2**, not
   "up to 30": only 2 leaves resolved at R2/R3 — the sampler's ceiling
   is set by the frontier's inertia, a measurement outcome, not a
   shortfall.
5. **Environment note:** this host reports `nproc` 4 (plan9's env
   recorded 8 with a 4-CPU quota); measured campaign m2 ≈ 3.21 exceeds
   the locked 2.82 — the conservative locked constants were used for all
   §7 arithmetic (the R4 projection rule), so the not-fired decision is
   robust.

## 10. Cleanup (measurement conventions)

Session dirs `/tmp/plan10_sweep`, `/tmp/plan10_cold`, `/tmp/plan10_merge`
and all TT dumps deleted at close; raw captures live under `logs/`
(gitignored). Committed record: `sweep.py`, `make_book.py`, `README.md`,
`env.json`, `state/*.json` (rung_R1/R2/R3, r4_decision, arm_COLD, merge,
master_state_merge, ledger.json, ledger.csv, analysis.json), and
`docs/plans/solve/book.md` (generated, counts verified against report4
§1).

## 11. Unresolved parts / missing tests / next steps

- M3's substrate is n=2; Stage 2's first task must be a larger-n
  validation of the advisory-dn censor signal (the sweep's ledger gives
  the covariate per leaf; any cheap budgeted re-sweep supplies more
  positives only if the frontier converts — the alternative substrate is
  a *constructed* censor-label corpus from deeper ladders on a sample).
- The A6b certificate design still needs its pre-registered soundness
  argument (path-scoped provenance; per-job TT pollution must not cross
  the job boundary) — plan-level work, not started.
- Stage-1 (A5.4) targeting of the cheap class is supported by the M1
  sub-reading but was not gate-selected; if pursued, it must not re-tread
  the falsified A5 scheduler/budget family (§6 of plan10).
- No campaign-code or solver tests were touched (no code changes); the
  gate (`make test`) is therefore not re-run for this plan per its
  non-goals — the only "test" surface is the sweep's own machinery
  checks (all green) and the A2 merge verification.
- book.md's PVs are the solver's informational PVs (from the TT, not
  validated proofs) — the same caveat as report4; a validator pass over
  each book line would upgrade them but is out of this plan's scope.

## 12. SESSION COMPLETE block

See the session closure in the conversation record; deliverables:
plan10 executed (ladder + merge + analysis + book.md), report10.md
written, initiative + plans index updated at close, cleanup done.
