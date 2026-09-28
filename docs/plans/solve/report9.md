# Report 9 — multi-session checkpoint-resume accumulation and the item-8 verdict (item 8, stage 3/3)

Executes `plan9.md` (item 8, stage 3 of 3) in one session, 2026-09-25.
Harness: `measurements/plan9/resume.py` (black-box driver, Python 3
stdlib only, adapted from plan8's `campaign.py`). All five registered
runs (SMOKE, S1, S2, S3, RC) completed strictly sequential on the
otherwise idle sandbox, stdin from DEVNULL, raw captures under `logs/`
(gitignored), state JSONs + versioned master-state checkpoints under
`state/`, environment in `env.json`. **Compute ≈ 4 h 55 m of the 6 h
stage cap** (SMOKE 10 m + 4 × 1 h arms + ≈ 25 m close/analysis
overhead); no §7 contingency fired; no arm was interrupted.

## 1. The machinery (§2) — built, gated, SMOKE-validated

All changes confined to `examples/campaign*`, as registered:

- **Master state v2 + `--resume`** (`examples/campaign/state.rs`):
  additive dump fields (`version`, per-leaf `depth`/`last_worker`,
  per-child `depth`/`synthesized`); `--resume` overlays the v2 state
  over a fresh `build_children` (statuses, advisory pn/dn, work, slices,
  synthesized flags), drops all job locks, clears stale job files,
  re-emits previously synthesized child facts once at startup so the
  **fresh proof tree rebuilds by construction**, and re-drains the
  durable `results/` from an empty `processed` list (re-verified,
  re-merged; counters restart at 0). Two implementation findings beyond
  the plan text, both recorded here as the faithful reading of §2:
  (a) in-flight **straggler results of session k−1** (locks dropped at
  resume) would be unmergeable by job id, so the drain gained a
  path-based fallback (`find_loc_by_path` over the echoed 2-ply job
  path) — stragglers merge into session k exactly per the registered
  accounting rule; (b) re-drained historical results must not re-add
  per-leaf `work` (restored already), so the accumulation is gated on a
  same-session lock match.
- **`--job-seed`** (§2.2): job ids `w{w}_{seed}_{n}`; unit test pins
  cross-seed uniqueness and the worker-prefix/parse compatibility.
- **Worker `--tt-dump` / `--tt-load`** (§2.3): dump on the observed stop
  condition (STOP file or `--max-runtime`) before exit; restore at
  startup into the fresh table via `tt_mut().store` (solved: outcome +
  depth + best_move; unsolved: bounds + work; `best_child` unset =
  registered fidelity limitation). Restore logs a machine-parseable
  `restore:` line; a missing/corrupt snapshot degrades the worker to
  cold-start with a `restore: DEGRADED` line (the §7 contingency —
  never needed).
- **Close protocol** (§2.4) is driver-owned: master writes STOP →
  workers finish their in-flight job, dump their TT, exit on their own →
  driver waits bounded (300 s) for exits + the complete TT file set →
  versions `master_state_s<k>.json` into the committed `state/`.

Verification: `make test` green (fast gate; campaign tests extended:
state v2 dump→load round-trip, job-seed namespacing, path-locator,
worker TT snapshot round-trip, missing-snapshot degradation); clippy/fmt
clean. **Product binary byte-identity check passed** — SHA-256
`1b70b32f46d9218e…`, exactly plan8's record. Campaign binaries differ
(expected, registered); their pre-plan9 hashes are recorded in
`env.json` / `state/pre_plan9_binaries.txt` with an observation: they
already differed from plan8's `env.json` entries before any plan9 edit
(clean tree; the examples in `target/` predate plan9 and plan8 recorded
different hashes — likely a rebuild between the env capture and the
run; the registered identity anchor, the product binary, matches
exactly, and S1's fresh-session numbers corroborate plan8 C1 at −7%
rate, §3).

**SMOKE (not a datapoint) passed all seven registered checks** on the
first run — no machinery defect, no re-run needed: state v2 fields
present, 4/4 TT files complete (≈ 122 MB each), every worker restored
(file counts = table counts, probe 101/101 clean), re-drain evidenced
(jobs_completed 708 → 1340), verify_failures/job_errors 0 both
sessions, facts monotone (30 → 30).

## 2. Arms × metrics (the registered table)

Shape: plan8's incumbent V1 unchanged (`--slice 4000000 --max-slice
8000000`, feedback on, retention on, no `--abandon`, 4 workers,
`--tt-mb 128 --pt-mb 512`). Root: d4d5 p2. Host: same sandbox class
(4-CPU quota, 8 GiB memory.max; hostname differs from plan8's capture —
new container instance; identical cgroup limits recorded in `env.json`).

| metric | S1 (fresh) | S2 (warm ← S1) | S3 (warm ← S2) | RC (cold ← S1) |
| --- | --- | --- | --- | --- |
| seed / resume | 0 / no | 1 / v2 + 4 TTs | 2 / v2 + 4 TTs | 9 / v2, no TTs |
| wall (s) | 3607.0 | 3608.1 | 3608.2 | 3601.6 |
| marginal nodes (a) | 2.061 G | 1.803 G | 1.549 G | 1.798 G |
| marginal rate (nodes/s) | 572,497 | 500,769 | 430,319 | 499,532 |
| marginal m2 (b) | 2.888 | 2.526 | 2.171 | 2.520 |
| jobs completed (c) | 7,189 | 13,365 | 18,666 | 13,405 |
| jobs dispatched in-session | 7,193 | 6,176 | 5,301 | 6,216 |
| jobs/h (in-session) | 7,179 | 6,162 | 5,288 | 6,212 |
| κ (marginal) | 27.59 | 27.5 | 27.5 | 27.5 |
| children resolved | 0/27 | 0/27 | 0/27 | 0/27 |
| leaves open / won / lost | 698/30/0 | 698/30/0 | 698/30/0 | 698/30/0 |
| **facts at close** | **30** | **30** | **30** | **30** |
| **Δfacts vs prior close** | **30** | **0** | **0** | **0** |
| job_errors / verify_failures | 0 / 0 | 0 / 0 | 0 / 0 | 0 / 0 |
| oom_kill delta | 0 | 0 | 0 | 0 |
| peak tree-RSS | 0.96 GiB | 0.90 GiB | 0.90 GiB | 0.90 GiB |
| worst worker ru_maxrss | 819 MB | 346 MB | 344 MB | 226 MB |
| restored records (solved+unsolved) | — | 2.34M+1.86M | 2.43M+1.76M | — (cold) |
| restore probe (checked / mismatched) | — | 101 / 0 | 101 / 0 | — |
| TT checkpoint set at close | 4/4 (~452 MB) | 4/4 (~460 MB) | 4/4 (~456 MB) | — |

(a) Marginals by subtraction of prior closes per the registered rule
(the summary counters of a resumed session include the re-drained prior
results; e.g. S2's raw counter 3.864 G − S1's 2.061 G). RC's base is
S1's close. (b) vs R_seq = 198,229 (report7). (c) A resumed session's
`jobs_completed` counts every drained result including the re-drain —
the in-session dispatch line is the session's own new jobs.

## 3. Primary metrics: ρ = 0 at both boundaries and under cold resume

- **Δfacts(S1) = 30** — the denominator is alive and exactly corroborates
  plan8 C1's fresh-hour harvest (30); the halt-and-investigate
  degenerate rule did not fire. S1's close state (30 wins, 698 open
  leaves, 0/27 children) reproduces plan8 §3's frozen-frontier prior at
  1 h.
- **Δfacts(S2) = 0, Δfacts(S3) = 0** — ρ_2 = ρ_3 = 0.0. The full
  accumulated state (master proof state + 4 worker TTs carrying 2.3–2.4M
  solved records each, restore verified clean) converted into **zero**
  new verified facts across two consecutive warm hours.
- **Δfacts(RC) = 0** — ρ_cold = 0.0. The attribution control behaves
  identically to the warm arms at the facts level.
- **Headline ρ = (Δfacts(S2) + Δfacts(S3)) / (2·Δfacts(S1)) = 0.0.**

## 4. Pre-registered gate: **NEGATIVE** (ρ = 0.0 < 0.4, m2 = 2.8176 ≥ 1 locked)

Per plan7 §4 bands fired at plan9: m2 is in GO band, so the verdict
hinges on ρ alone, and ρ measured zero. **The item-8 verdict is
NEGATIVE: `solve` goes dormant per the status rule** (plan6 Not-GO +
item-8 negative; no open items, reopeners only). The zero-numerator
degenerate rule applied as pre-registered: ρ = 0 is a *measured* value —
the denominator exists (the fresh hour harvests the cheap class) and the
numerator is measured zero. The limitation demonstrated is this root's
frozen frontier plus absent conversion at job-scale budgets — **not**
that session-boundary accumulation was falsified in the abstract.

**Failure-mode attribution (§3.4 substrate metric):** the pre-registered
discriminator fires the **resume-mechanics mode** — no warm discount:
on the 12 leaves common to all four sessions' job sets, warm and fresh
per-job work-to-censor are equal (ratio 1.00; both sides ≈ 100% pinned
at the 8M-eval budget; median advisory root_dn 37k–48k ≪ 8M). With a
**recorded resolution caveat**: the fresh S1 jobs on those same leaves
are already 99.7% budget-pinned, so a discount could only manifest
below-cap and the metric cannot resolve one in this regime; the positive
integrity signals (clean restore, clean re-drain, monotone state, zero
verify failures) show the substrate *survives* the boundary while
nothing *converts*. The frozen-frontier degeneracy is separately
confirmed on the fresh side (S1 = 30 facts then flat, plan8 §3 at 1 h).
Both readings agree on the verdict and on the reopener: the open
mechanism questions are the A5.4 coverage race (master-side targeting of
the proof-bearing child) and budget-aware early-censor certificates —
neither is session-structure work, and none was done here (non-goal).
Recorded as **A6** in `campaign_architecture.md` §11.

## 5. Secondary metrics (observational)

- **Restore integrity** (§3.3): every warm worker restored 2.33–2.43M
  solved + 1.76–1.86M unsolved records; table counts equal file counts
  (no eviction); 100-record probes at 0 mismatches in all 8 warm
  session-worker cells. Registered realization note: the plan's
  "replay 100 sampled solved keys through a fresh `Position`" is not
  implementable as literally stated — Zobrist keys are one-way, no
  position can be derived from a record. The realized spot-check is the
  store→probe round-trip on 100 evenly sampled solved records (outcome +
  depth + best_move exact), backed by two stronger soundness nets: the
  master's global replay verification on every re-drained and new
  decisive fact (A2 tripwire — 0 failures), and the artifact validator.
- **Continuity/health** (§3.5): job_errors 0 and verify_failures 0 in
  every arm (the re-drain re-verified ~7.2k prior results per resumed
  session with zero failures — the resume path never produced a
  unsound fact); oom_kill delta 0; peak tree-RSS 0.90–0.96 GiB vs the
  7.0 GiB abort watermark; workers ≤ 819 MB ru_maxrss vs the 3.5 GiB
  bound. Warm sessions hold *less* RSS than S1 (the restored table
  replaces the cheap-class fill; jobs are harder, tables less full).
- **Work-to-censor** (§3.4): see §4 — warm ≈ fresh, both budget-pinned.
- **Marginal rate decline across the chain** (observational, no
  registered gate): m2 2.89 → 2.53 → 2.17 over S1 → S2 → S3. Steeper
  than plan8's 600 s → 4 h decline (3.49 → 2.82); consistent with the
  cheap-leaf frontier draining within the first minutes and every later
  job being a hard censor at full budget. Not a DECAY-gate datum (that
  stage-2 gate is closed); recorded for any future campaign pricing.

## 6. Deliverables

- **The plan7 §4 horizon table, completed**: single-session rows stand
  from report8 §4 (R_camp = 558,529 nodes/s; T(10^14) ≈ 5.7 years at 4
  workers). **The accumulated rows are struck with the measured reason**:
  ρ = 0 — multi-session accumulation does not multiply the affordable
  horizon on this root; the horizon numbers remain the single-session
  constants. W ≈ 27.5 × nodes (κ fixed, report8 §2).
- **The item-8 verdict: NEGATIVE → `solve` dormant** per the status
  rule; `initiative.md` and `docs/plans/README.md` updated at this
  close.
- **`campaign_architecture.md` §11 amendment A6** (attribution + reopener
  record).
- **The checkpoint/resume machinery remains in `examples/`** as the
  campaign's constraint-4 implementation regardless of verdict: master
  state v2 + `--resume` + `--job-seed`, worker `--tt-dump`/`--tt-load`,
  driver close protocol — measured sound end-to-end (SMOKE 7/7, 8/8
  clean restore cells, 0 verify failures over ~28k re-drained + new
  results, 0 oom, memory profile flat).

## 7. Deviations, problems, tools

- **No registered deviations in arms, budgets, order, or gates.** No
  arm interrupted; no contingency fired; SMOKE needed no re-run.
- **Substrate-metric refinement (analysis-side only, no re-run)**: the
  registered mean work-to-censor comparison was additionally computed
  restricted to the 12 leaves common to all four sessions and with
  INF-scale root_dn values excluded (the unrestricted mean was polluted
  by DF-PN's INF-scale advisory numbers and the early cheap class);
  medians and cap-pinned fractions added. The verdict-bearing
  discriminator (warm/fresh ratio) is reported under both readings
  (unrestricted 1.011, restricted 1.00 — same mode).
- **`find_loc_by_path` + work-gating** are the two implementation-level
  findings beyond the plan text (§1a/b above); both are resume-path
  faithful to the registered accounting rule and covered by unit tests.
- **Environment**: new container instance vs plan8's capture (hostname,
  kernel differ; cgroup limits identical and recorded). S1's fresh rate
  −7% vs plan8 C1 (572k vs 615k nodes/s) — host noise class; the
  facts-level denominators agree exactly.
- **Pre-plan9 campaign binary hashes** differed from plan8's `env.json`
  record before any plan9 edit (observation recorded in `env.json`;
  product binary — the registered anchor — matches exactly).
- Tools: Python 3 stdlib only (`os.wait4`, `/proc/<pid>/statm` sampler,
  cgroup v2 `memory.events`); release binaries; `inspect_pt --validate`
  ready for the COMPLETED branch (never fired — all arms censored).

## 8. Cleanup (measurement conventions)

- `/tmp/plan9_{smoke,chain,rc}` session dirs removed (including all
  checkpoint worker TT files ≈ 1.4 GB and the RC copy); `/tmp` back to
  9.4 MB used.
- `logs/` raw captures are gitignored; the committed record is
  `resume.py`, `README.md`, `env.json`, `state/*.json` (arm records,
  SMOKE record, versioned `master_state_{smoke_s0,smoke_s1,s1,s2,s3}.json`,
  `pre_plan9_binaries.txt`), and `analysis.json`.
- `git add --dry-run` confirms only the commit list above (plus the
  campaign code changes and the docs updates).

## 9. Next steps

The initiative's status disposition changes at this close: **`solve` →
dormant** per the plan6/plan7 status rule (no open items; reopeners
only). The reopener record (A5.4 + A6) names the two mechanism
directions that would need new pre-registration before any compute: the
coverage race (master-side targeting) and budget-aware early-censor
certificates; a resource step-change (the 8-CPU/16 GB/12 h envelope)
does not reopen the item by itself — it re-prices T(W) at the same ρ = 0
session economics.

SESSION COMPLETE
- plan9.md executed: §2 machinery implemented (examples/-side only; product binary byte-identical, `make test` green, clippy/fmt clean), SMOKE 7/7, arms S1/S2/S3/RC run strictly sequentially with clean health counters; Δfacts = 30/0/0/0 → **ρ = 0.0 → item-8 verdict NEGATIVE (resume-mechanics mode per the pre-registered discriminator, with the budget-cap resolution caveat)**; `analysis.json`, `report9.md`, A6 amendment written; `initiative.md` and the `docs/plans/README.md` solve row updated (solve → dormant); session dirs and checkpoint TT files cleaned.
Follow-up options:
  1. Close-out session: review A6/A5 reopener notes and decide whether either mechanism direction (coverage race, early-censor certificates) earns a fresh pre-registered plan or stays a recorded reopener; no compute by default.
  2. Alternative: archive/summary pass — distill the initiative's measured record (plans 1–9) into the user-facing decision memo (value-of-startpos campaign economics at every measured operating point), since the ladder is complete and dormant.
