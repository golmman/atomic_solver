# Plan 10 measurements — Phase 3 Stage 0: frontier budget-completion sweep

Executes `docs/plans/solve/plan10.md`. Driver: `sweep.py` (Python 3 stdlib
only), following plan7–9 driver conventions. The sweep **bypasses the
master's dispatcher by design** (§2.2): the driver writes job files directly
into the session's `jobs/` dir and runs bare `campaign_worker` processes; a
final merge pass (`campaign_master --resume`) re-verifies every decisive
result under the global context (A2 discipline) before it counts toward the
decision metric.

## Command table

| command | purpose |
| --- | --- |
| `python3 sweep.py env` | record `env.json` (binary SHA-256s incl. the registered product-binary identity check vs plan9's `1b70b32f46d9218e`) |
| `python3 sweep.py rung R1` | rung R1: 8M child-evals per job on the 698 open leaves (fresh workers, no TT restore) |
| `python3 sweep.py rung R2` | rung R2: 32M per job on R1's survivors (warm: `--tt-load` of R1's dumps) |
| `python3 sweep.py rung R3` | rung R3: 128M per job on R2's survivors (warm), then the contingent-R4 projection rule is evaluated and recorded |
| `python3 sweep.py r4` | contingent R4 (512M on R3 survivors): fired iff `elapsed + K·(512M/κ)/R_camp ≤ 5.0 h` |
| `python3 sweep.py cold` | conditional cold-control arm: fired iff ΔC > 0; ≤ 30 leaves resolved at R2/R3 re-run cold at their resolving rung's budget |
| `python3 sweep.py ladder` | R1 → R2 → R3 (+ contingent R4 + cold control), resumable per rung |
| `python3 sweep.py merge` | A2 merge pass: decisive results → fresh session seeded with plan9's frozen master state → `campaign_master --resume --job-seed 10 --max-wall 60 ...` re-drain (verify failure = machinery defect, abort) |
| `python3 sweep.py analyze` | M1–M6 + the pre-registered Stage-0 gate + `state/ledger.json` / `ledger.csv` → `analysis.json` |
| `python3 sweep.py status` | progress summary |

Rungs run strictly sequential, nothing else concurrently, stdin from
DEVNULL. Session dirs under `/tmp`: `plan10_sweep`, `plan10_cold`,
`plan10_merge`. TT dumps (≈480 MB/set) kept only for the latest rung,
deleted at close. Raw captures under `logs/` (gitignored); committed record
= `sweep.py`, this README, `env.json`, `state/*.json` (rung records,
r4/cold/merge records, merged master state, ledger, analysis).

## Registered shape

- Frontier: the 698 `Open` leaves of plan9's fresh S1 close
  (`../plan9/state/master_state_s1.json`; sanity-checked: 728 leaves /
  30 Won / 698 Open / 27 children).
- Jobs: `w{w}_{rung}_{n}.json`, round-robin over 4 workers, stable per-leaf
  worker affinity across rungs (warm cumulative semantics: a leaf's
  retained TT carries its own prior rungs' state).
- Budgets: R1 = 8M, R2 = 32M, R3 = 128M child-evals per job (R4 = 512M
  contingent). Per-job marginals are exact (`begin_run()` resets counters).
- Close protocol per rung: barrier (every rung job has a result) → STOP →
  bounded wait (300 s) for worker exits + complete TT-dump set.
- Memory gate (plan8 §2.3): tree-RSS sampler, abort at 7.0 GiB tree / 3.5
  GiB process; oom_kill delta watched.

## Binary identity

- Product binary must be byte-identical to plan9's record
  (`1b70b32f46d9218e…`); checked by `env` (reproduced with
  `CARGO_PROFILE_RELEASE_LTO=thin` — see `env.json` for the recorded
  plain-release-hash divergence, source unchanged since plan9's commit).
- Campaign binaries: sources git-identical to plan9's close commit
  `c6a30b0`; hashes differ from plan9's record (plan9's exact build context
  not reproducible from the commit + intervening proofdb workspace deps
  change cargo's fingerprint metadata). Recorded in `env.json`; behavior
  risk carried by the A2 merge-pass verification.
