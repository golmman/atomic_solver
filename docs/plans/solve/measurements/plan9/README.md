# Plan 9 measurements — multi-session checkpoint-resume accumulation (ρ)

Item 8, stage 3 of 3. Executes `docs/plans/solve/plan9.md`. Driver:
`resume.py` (Python 3 stdlib only), adapted from plan8's `campaign.py`.

## Command table

| command | purpose |
| --- | --- |
| `python3 resume.py env` | record `env.json` (binary SHA-256s incl. the registered product-binary identity check vs plan8's `1b70b32f46d9218e`) |
| `python3 resume.py smoke` | SMOKE arm: 5 min fresh → checkpoint → 5 min warm-resume; machinery validation only, not a datapoint |
| `python3 resume.py arm S1` | fresh 3600 s session (seed 0) + checkpoint at close |
| `python3 resume.py arm S2` | warm resume from S1 (state v2 + 4 worker TTs), 3600 s |
| `python3 resume.py arm S3` | warm resume from S2 (second boundary), 3600 s |
| `python3 resume.py arm RC` | cold resume from S1 (fresh workers, no TT restore) in a seed-0 copy of S1's session dir, 3600 s |
| `python3 resume.py analyze` | ρ per arm, plan7 §4 verdict bands + plan9 §4 degenerate rules, failure-mode attribution; writes `analysis.json` |
| `python3 resume.py status` | progress summary |

Arms run strictly sequential, nothing else concurrently, stdin from
DEVNULL. Session dirs under `/tmp` (`plan9_chain`, `plan9_smoke`,
`plan9_rc`). Raw captures under `logs/` (gitignored); committed record =
`resume.py`, this README, `env.json`, `state/*.json` (incl. the versioned
`master_state_s<k>.json` checkpoints), and `analysis.json`.

## Recorded shape (plan8 incumbent V1, unchanged)

`--slice 4000000 --max-slice 8000000`, feedback on, retention on, no
`--abandon`, 4 workers, `--tt-mb 128 --pt-mb 512`. Session budgets
3600 s (SMOKE 2 × 300 s). Job ids namespaced per session seed
(`w{w}_{seed}_{n}`) so resumed sessions cannot collide with earlier
result files in the shared chain directory.

## Binary identity

- Product binary must be byte-identical to plan8's record
  (`1b70b32f46d9218e…`); checked by `env`.
- Campaign binaries are superseded by the plan9 §2 resume machinery
  (expected difference, recorded); pre-plan9 hashes recorded in
  `env.json` under `pre_plan9_campaign_binaries` and
  `state/pre_plan9_binaries.txt`.
