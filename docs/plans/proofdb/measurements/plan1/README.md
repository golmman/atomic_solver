# plan1 measurements — seed DB build (2026-09-28)

Merger MVP seed run over the `solve` plan4 artifact (95 validated proof-tree
shards), per `../../plan1.md`. All gates G1–G4 pass; census numbers are in
`../report1.md`.

## Command table

| file | command |
| --- | --- |
| `env.json` | captured at run time (git rev, cgroup limits, date) |
| `build_seed_manifest.py` | driver: builds the conforming manifest from `docs/plans/solve/measurements/plan4/artifact/index.json` (reconstructs missing startpos paths for `ladder_*`/`sib_*` entries; replay-verifies every path). Deterministic (sorted summary keys); two runs byte-identical. |
| `manifest_seed.json` | `python3 build_seed_manifest.py` (in this dir; sha256 `92e49fcb54227c29…`) |
| `proofdb_seed.db` | `target/release/examples/proofdb_merge --manifest manifest_seed.json --shard-dir docs/plans/solve/measurements/plan4/artifact --db proofdb_seed.db --dump nodes_seed.txt --sample-lines 10` (wall ≈ 15 s) |
| `nodes_seed.txt` | same run (canonical dump, byte-determinism artifact) |

## Reproduced results

- census: 95/95 shards merged; shard tree nodes 12,592 = 11,905 new + 687
  deduped; merged 12,081 = 1 root + 11,905 overlay + 175 open ancestors;
  89 open nodes upgraded; 95 skeleton re-validations, 0 conflicts.
- merged nodes: 11,994 proven (5,996 win / 5,667 loss exact at terminals,
  331 loss bound) + 87 open.
- G3 determinism: two runs — byte-identical DB and dump.
- G4 spot-checks: `c2c3 g7g5` reads `win, depth_bound=7, bound`;
  `c2c3 g7g5 d1a4` (2...g5 3.Qa4 line) reads `loss, bound 6`.
- DB integrity: 0 bad parent links, 0 orphans over all 12,081 rows.
