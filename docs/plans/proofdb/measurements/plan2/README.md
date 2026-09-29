# plan2 measurements — first harvest batch (2026-09-29)

Harvest loop (item 2) per `../../plan2.md`: standing shard directory
(`../../shards/`, the durable layer) grown by one batch of jobs over the seed
DB's 87 open frontier nodes; merger re-run after the batch.

## Command table

| file | command |
| --- | --- |
| `env.json` | captured at run time (git rev, cgroup limits, date) |
| `build_standing_manifest.py` | driver: builds the standing manifest (`../../shards/manifest.json`) from plan1's `manifest_seed.json` (required fields only, deterministic bytes; two runs byte-identical, sha256 `089bcc1d1bb4a912…`) |
| `proofdb_input.db`, `nodes_input.txt` | the batch's input DB: `target/release/examples/proofdb_merge --manifest ../../shards/manifest.json --shard-dir ../../shards --db proofdb_input.db --dump nodes_input.txt` (sanity merge; `nodes` content byte-identical to plan1's `proofdb_seed.db`; kept as the exact batch input for the H3 diff) |
| `proofdb_grown.db`, `nodes_grown.txt` | `target/release/examples/proofdb_merge --manifest ../../shards/manifest.json --shard-dir ../../shards --db proofdb_grown.db --dump nodes_grown.txt --sample-lines 10` after the batch (two runs byte-identical: db sha256 `949d6e59908fccbb…`, dump `f787c2f92719e4ca…`) |
| `census.json` | per-job records (path, tier, budget, child-evals, wall, outcome/exit_reason) parsed from the harvest run's `job:` stdout lines, plus the merge arithmetic and the root-job correction note. Raw transcripts are not committed (measurement conventions); this file is the record. |
| `sample_lines.txt` | the merger's `--sample-lines 10` output over the grown DB (H4 informational replays) |

## Reproduced results

- batch: 87 screen jobs at 4M child-evals each → 12 decisive (all wins,
  10–27 plies), 75 censored; heavy tier: 5 deepest censored re-run at 40M →
  0 decisive. Session wall ≈ 106 s.
- merge: 107 shards, 12,655 nodes = 12,580 proven + 75 open (87→75 open; 12
  frontier nodes upgraded, 101 open upgrades total incl. cross-route);
  skeleton re-validations 107/107, conflicts 0.
- root-job correction: the batch's root job (path `""`) hit a harvest-tool
  false-repetition defect (startpos key seeded into its own prefix → instant
  `Draw`, 0 evals; harmlessly censored). Fixed
  (`harvest::replay_job_path`) and re-run standalone: censored
  `BudgetExhausted` at 4M child-evals (0.79 s). See `census.json`.
