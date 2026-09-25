# Plan8 measurements — campaign arms at 4 workers on d4d5-p2 (item 8, stage 2)

Stage-2 sizing measurement (plan8, executed 2026-09-25): the registered
campaign shape (plan6 winner **V1**: `--slice 4M --max-slice 8M`,
feedback on, retention on, no `--abandon`) at 4 workers on the
registered representative quiet root, under the plan7 §4 combination
rule. **No code changes** — black-box driver over the unmodified release
binaries (`campaign.py`, Python 3 stdlib only); binary SHA-256s in
`env.json` match the plan6 record exactly (solver `1b70b32f46d9218e`,
master `3d12f4555da007d4`, worker `6ac75a4cbd30b3cb`). Sandbox:
4-CPU cgroup quota, 8 GiB memory.max (identical to plan3/plan7).

## Registered arms (plan8 §2) and gate

| arm | budget | purpose |
| --- | --- | --- |
| SM | 600 s, 4 workers | harness validation, early 4-worker rate, memory profile |
| S600 | 600 s, `--mode seq` | sequential nodes↔child-evals conversion (κ_seq) |
| C4 | 14400 s, 4 workers | **headline**: R_camp, m2 at the stage-2 design point |
| C1 | 3600 s, 4 workers | rate-stability point over the stage-1 4× span |

Headline gate (plan8 §3): **FEASIBLE, GO-band** — m2(C4) = **2.8176**
(≥ 1), R_camp = **558,529 nodes/s**, DECAY modifier not fired
(m2(C4)/m2(C1) = 0.9076 > 0.8), floor extension N_floor′ =
**8,043,331,647 nodes** (C4 censor Σ > 2.87 G stage-1 floor), memory
check **passed** (peak tree-RSS ≈ 0.97 GiB vs the 7.0 GiB abort
watermark; max single process 595 MB vs 3.5 GiB; `oom_kill` delta 0 in
every arm). No arm fired an INFEASIBLE signal; the registered CONFw2
confirmation was therefore not run.

## Command table

| artifact | command |
|---|---|
| `env.json` | `python3 campaign.py env` |
| `state/arm_SM.json` | `python3 campaign.py arm SM` |
| `state/arm_S600.json` | `python3 campaign.py arm S600` |
| `state/arm_C4.json` | `python3 campaign.py arm C4` |
| `state/arm_C1.json` | `python3 campaign.py arm C1` |
| `state/arm_CONFw2_rep0.json` (registered, not run — no INFEASIBLE signal) | `python3 campaign.py confirm` |
| `analysis.json` | `python3 campaign.py analyze` |
| progress | `python3 campaign.py status` |

Arms are resumable per arm (state-file exists ⇒ skipped); strictly
sequential execution, stdin from DEVNULL, raw master/worker captures
under `logs/` (gitignored), session dirs under `/tmp/plan8_arms`
(removed at close).

## Data conventions

- **Unit**: dfpn node expansions. Campaign side = the master summary's
  `worker_nodes` accumulator (registered unit); per-job result JSONs
  are the cross-check (`cross_check` field). SM matched exactly; C4/C1
  show +0.004% deltas (the last in-flight jobs' results land on disk
  after the master freezes its counters at censor — bounded, explained).
- **κ (nodes↔child-evals)**: S600 (sequential) 27.5; SM 27.6; C4 27.52;
  C1 27.6 — both sides agree on this root ("fixed once" deliverable).
- Memory: per-process max-RSS via `os.wait4` ru_maxrss; tree-RSS
  sampler (Σ `/proc/<pid>/statm`, 10 s cadence) enforced the registered
  abort rule (7.0 GiB tree / 3.5 GiB single, 2 consecutive samples).
  Note: the 10 s sampler misses short export spikes; `ru_maxrss` is the
  authoritative per-process peak (workers ≈ 450–600 MB).
- `logs/` raw captures and session dirs are not committed (convention);
  the state JSONs (incl. per-worker job lines) are the record.
