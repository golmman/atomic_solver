# Report 8 — campaign arms at 4 workers on d4d5-p2 (item 8, stage 2/3)

Executes `plan8.md` (item 8, stage 2 of 3) in one session, 2026-09-25.
Harness: `measurements/plan8/campaign.py` (black-box driver over the
unmodified release binaries; binary SHA-256s match the plan6 record
exactly — product path and campaign code unchanged). **No code
changes.** All four registered arms ran strictly sequential on the
otherwise idle sandbox, stdin from DEVNULL, raw captures under
`logs/`, state JSONs under `state/`, environment in `env.json`
(4-CPU quota, 8 GiB memory.max — identical to plan3/plan7).

## 1. Gate verdict: **FEASIBLE, GO-band** (R_camp = 558,529 nodes/s, m2 = 2.82)

All arms censored (prior ≈ 0 for COMPLETED held); no memory incident
anywhere; the §3 gate fires **GO-band**:

| arm | budget | Σ worker nodes | wall (s) | aggregate rate (nodes/s) | **m2** | κ | max-RSS (master / worst worker) |
| --- | --- | --- | --- | --- | --- | --- | --- |
| SM | 600 s | 415,312,189 | 600.2 | 691,975 | **3.4908** | 27.6 | 19 MB / 591 MB |
| S600 (`--mode seq`) | 600 s | 118,788,096 | 600.0 | 197,971 | 0.999 (rate vs R_seq) | **27.5** | 227 MB |
| C4 (headline) | 14400 s | 8,043,331,647 | 14400.9 | 558,529 | **2.8176** | 27.52 | 19 MB / 519 MB |
| C1 | 3600 s | 2,215,391,189 | 3600.1 | 615,361 | **3.1043** | 27.6 | 19 MB / 595 MB |

- **m2 ≥ 1 → GO-band**: the campaign beats sequential node-throughput
  on this root at sizing scale. R_best switches to
  **R_camp = m2 · R_seq = 558,529 nodes/s** (2.82× R_seq); plan7 §4's
  horizon-table campaign rows fill at R_camp. The A5.4 prior (campaign
  < 1× sequential in throughput terms) is **falsified on this root at
  sizing scale** — consistent with the §1 economics argument: on a
  censored quiet root the sequential side churns a capacity-capped TT
  while campaign workers solve shallow leaf jobs in private 128 MB TTs
  that never approach capacity (worker RSS ≈ 450–600 MB flat, 1 h → 4 h).
- **DECAY modifier not fired**: m2(C4)/m2(C1) = 0.9076 > 0.8
  (registered threshold). R_camp is priced at the C4 value (the
  conservative choice anyway).
- **Floor extension**: N_floor′ = max(2.87 G, Σ nodes at censor of C4)
  = **8.043 G nodes** (the prior m2 ≈ 1.79 predicted ≈ 5.1 G; the
  measured 2.82 beats it).
- **Memory check (the stage's own local gate): PASS** with ≈ 7×
  margin. Peak tree-RSS (10 s sampler, master + 4 workers) ≈ 0.97 GiB
  in every campaign arm vs the 7.0 GiB abort watermark; per-process
  ru_maxrss: master 19–20 MB, workers 453–609 MB vs the 3.5 GiB single
  process bound; `oom_kill` delta 0 in all arms; no worker vanished
  (`report6` §5.1's OOM precedent not reproduced at 4 workers — the
  V3 OOM there was proof-tree-export accumulation on finishable
  positions, absent here: only 30 leaf wins total, 0 children resolved).
- The registered CONFw2 2-worker confirmation was **not run** (its
  trigger, an INFEASIBLE signal, never fired).

Interpretation: **4 workers buy 2.82× sequential node-throughput on the
quiet root, memory-checked, rate-stable over the stage-1 4× span.**
Per-worker efficiency declines mildly with session length (SM 0.873×
R_seq → C1 0.776× → C4 0.704×) — jobs get harder as the cheap-leaf
frontier drains — but stays far enough above the DECAY band. This is
the stage-2 input to the item-8 combination rule: R_best switches from
R_seq to R_camp, conditional on the registered V1 shape.

## 2. The conversion fixed once (κ both sides)

- Sequential side (S600): κ = **27.5** child evals/node
  (3,266,511,211 evals / 118,788,096 nodes).
- Campaign side: SM 27.6, C4 27.52, C1 27.6 — **agreement within 0.4%**
  across sides and budgets. The d4d5-p2 root's κ ≈ **27.5** (plan5's
  job-level prior ≈ 28 confirmed; the sequential-shuffle 17.9 was
  class-dependent, as registered).
- W can therefore be quoted in child evals as ≈ 27.5 × nodes for
  continuity with the plan5/plan6 record.

**S600 censoring note (registered-unit caveat).** The `--mode seq`
summary hardcodes `exit: "done"` and a censored search returns the
`Draw` default, so the summary alone cannot distinguish "proved draw"
from "censored". Censoring is certain by monotonicity: plan7's L1
censored at 3600 s with 709.7M nodes on the identical binary/FEN; S600
did 118.8M nodes in 600 s — strictly less work, so the first decisive
outcome cannot have been found. Its 197,970 nodes/s (−0.13% vs R_seq =
198,229) independently matches the stage-1 certified rate.

## 3. Job health and the master proof state (plan9 substrate)

Job-level (from worker stderr job lines, all arms):

- jobs/h: SM 9,081 → C1 7,725 → C4 6,937; mean job wall 1.58 → 1.83 →
  1.85 s. 4 workers do **not** change the job mix vs plan5's 2-worker
  row qualitatively — but the slice ladder is **structurally
  non-binding on this root**: every completed job in every arm is
  < 1M nodes (27,748/27,748 in C4), so the 4M→8M budget shape never
  binds; jobs resolve (censored-draw) far below the slice.
- job_errors 0, verify_failures 0 in all arms (SOUND clean).
- job outcomes: 30 `win` jobs in every campaign arm — the same 30 leaf
  proofs appear at 600 s and never grow; everything else is `draw`.

Master proof-state counts at censor (observational ρ substrate for
plan9, not a verdict metric): **children 0/27 resolved, leaves_open
698, leaves_won 30, leaves_lost 0 — identical at 600 s and at 4 h.**
The proof state freezes after the early cheap-leaf phase; 4 h of
campaign work at 2.82× throughput resolves no additional root children
(plan5's 0/27 prior confirmed at 4-worker scale). The frontier is wide
(698 open leaves) and churning — plan9's checkpoint-resume question
(ρ, accumulation across sessions) is exactly the right next stage: the
campaign's throughput advantage is real, but converting it into proof
progress needs multi-session accumulation.

## 4. Stage-2 rows of the plan7 §4 horizon table

R_camp = m2 · R_seq = 2.8176 × 198,229 = **558,529 nodes/s** (GO-band,
no DECAY; conditional on the registered V1 shape). T(W) = W / R_camp:

| W (nodes) | sequential @ R_seq (report7) | **campaign @ 4 workers (R_camp = 558.5k/s, plan8)** | 8-worker envelope (12 h sessions) |
| --- | --- | --- | --- |
| 10^14 | 5.04e8 s ≈ 16.0 years | 1.79e8 s ≈ **5.7 years** | not measured in this sandbox (quota-oversubscribed); plan7 extrapolation ≈ 2× R_camp ≈ 1.12M nodes/s → ≈ 2.8 years, optimistic-linear, flagged |
| 10^15 | 5.04e9 s ≈ 160 years | 1.79e9 s ≈ **57 years** | ≈ 28 years (extrapolated) |
| 10^17 | 5.04e11 s ≈ 16,000 years | 1.79e11 s ≈ **5,700 years** | ≈ 2,800 years (extrapolated) |

Conversion note: W ≈ 27.5 × nodes in child evals (κ fixed above).
Caveat carried from plan8 §2: R_camp is conditional on the registered
shape; if a different recorded shape has a higher quiet-root rate, that
is a follow-up observation, not a registered comparison (no compute for
it). The multi-session accumulation factor ρ (plan9) is still required
before any POSITIVE verdict: m2 ≥ 1 alone does not make T(W)
affordable — even the W = 10^14 anchor remains > 5 years of
uninterrupted 4-worker campaign at the measured constant.

## 5. Updated stage-2 inputs of the item-8 combination rule (for plan9)

- **R_camp = 558,529 nodes/s** (m2 = 2.8176), 4 workers, V1 shape —
  R_best for campaign-priced rows.
- **m2 = 2.8176** (≥ 1: GO-band fired; A5.4 falsified in throughput
  terms on this root).
- **κ ≈ 27.5** (both sides, ±0.4%).
- **N_floor′ = 8.043 G nodes** (C4 censor; extends the 2.87 G stage-1
  floor).
- **Memory profile**: 4-worker campaign ≈ 1.0 GiB tree-RSS steady-state
  (workers ≈ 450–600 MB each, master 19 MB); 8 GiB cgroup has ≈ 8×
  headroom — plan9 can add checkpoint stores without a memory gate.
- **ρ substrate**: master proof state frozen at 0/27 children, 698 open
  leaves across all arms — the accumulation measurement (does ρ
  approach 1 across sessions?) is the open question; no mechanism work
  done here (non-goal).

## 6. Deviations, problems, tools

- **Arms wording deviation (pre-registered in plan8 §2)**: the
  initiative's stage-2 wording said "2 h / 4 h caps"; the executed arms
  are 1 h / 4 h (C1/C4) to fit SM + S600 + analysis inside the hard 6 h
  stage cap while keeping the stage-1 4× span. `initiative.md` is
  amended at this close.
- **ru_maxrss lost for 2 of 4 workers in C4 and C1** (workers that saw
  the master's STOP_FILE and exited cleanly were reaped by subprocess
  internals before the driver's `wait4`). Not a gate metric; the
  sampler's per-process peaks cover the missing workers (247/246 MB in
  C4), and the memory gate has ≈ 7× margin either way.
- **Master accumulator vs result-file cross-check**: SM exact;
  C4 +290k nodes (+0.0036%), C1 +588k (+0.027%) — the last in-flight
  jobs' result files land after the master freezes its counters at
  censor. The registered unit is the master accumulator; the delta is
  bounded by the in-flight window (≤ 4 jobs) and does not affect any
  verdict.
- A 60 s throwaway smoke run (harness validation before SM, not a
  datapoint, state discarded) validated the sampler, cross-check and
  job-line parsing; per §5.2 the harness needed no defect fix after SM.
- Tools: Python 3 stdlib only (`os.wait4` ru_maxrss, `/proc/<pid>/statm`
  sampler, cgroup v2 `memory.events` oom_kill counter); release
  binaries as built, unchanged since plan6 (hash match).
- Compute: SM 10 m + S600 10 m + C4 4 h + C1 1 h + ≈ 15 m
  analysis/report ≈ **5.7 h of the 6 h stage cap**. No arm was
  interrupted; no §7 contingency fired.

## 7. Cleanup

- `/tmp/plan8_arms` session dirs (including the smoke run) removed.
- No TT snapshots or proof-tree artifacts to prune (censored arms
  produce neither; workers' private TTs lived in the session dirs).
- `logs/` raw captures are gitignored; state JSONs + `analysis.json` +
  `env.json` + `campaign.py` + this report are the committed record.
- `docs/plans/README.md` solve row intentionally unchanged (the
  initiative's status did not change: stage 3 still pending; note: the
  row still carries plan7's "not yet executed" wording — stale but
  status-neutral; `initiative.md` History is authoritative and updated).

## 8. Next steps

Stage 3 (plan9) runs the multi-session checkpoint-resume accumulation
measurement and fires the item-8 verdict with the plan7 §4 bands:
POSITIVE needs ρ ≥ 0.7 ∧ m2 ≥ 1 — **m2 = 2.82 is now locked in GO
band**, so the verdict hinges on ρ alone. The campaign's frozen proof
state (§3) and the 8× memory headroom are the substrate plan9 needs.
SESSION COMPLETE
- plan8.md executed: harness `measurements/plan8/` (campaign.py, README, env.json), all four registered arms run strictly sequentially, gate **FEASIBLE / GO-band** (m2(C4) = 2.8176, R_camp = 558,529 nodes/s, no DECAY, N_floor′ = 8.043 G, memory check passed), `analysis.json` + `report8.md` written, sessions/logs cleaned. No index row change (initiative stays active; stage 3 of 3 pending); initiative.md amended (stage-2 arms wording, History).
Follow-up options:
  1. `Draft docs/plans/solve/plan9.md` — stage 3: multi-session checkpoint-resume accumulation (ρ) on the campaign master proof state; with m2 locked in GO band, the item-8 verdict hinges on ρ alone.
  2. Alternative: if plan9 needs a preamble (checkpoint/resume mechanism probe in the campaign master's state files), run it as a short read-only session first, per the plan9 drafting convention.
