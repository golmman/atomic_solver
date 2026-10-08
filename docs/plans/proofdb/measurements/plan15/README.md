# plan15 measurements — the `descend` boot gates (2026-10-08)

Executes `../../plan15.md` tasks 2–5: the self-bootstrapping harvest boot
gate (task 2), the determinism gate (task 3), and the handoff smoke
(task 4), plus the census of the boot batches. The integration test
(task 5, `descend_boot_from_empty_layer`) lives in `tests/proofdb.rs` and
runs in the default gate; no separate artifact.

## Protocol (both boots identical)

```
fresh data/proofdb
printf '{"entries":[]}' > data/proofdb/shards/manifest.json
proofdb_merge                                  # root-only DB, built_from d801aa1fb7ddcc33
proofdb_harvest --policy descend               # bare defaults: base 4M, plies 2, tt-mb 128
proofdb_merge                                  # 4,314 nodes
proofdb_flip                                   # flips 0 verified 0
```

Boot B repeats the whole protocol in a second fresh layer (task 3).

## Provenance

| file | content |
| --- | --- |
| `env.json` | environment + rev + boot protocol + output-state digests + determinism flags |
| `boot_census.json` | boot A: session-start lines, summary line, per-ply split, and the 420 `job:` records (`wall_s` stripped — host-dependent) |
| `verdicts.json` | boot A vs boot B gate comparison (task 2/3 verdicts) |

Raw transcripts (`/tmp/bootA.out`, `/tmp/bootA.err`, …) are regenerable
byte-identically by the protocol above (modulo `wall_s`) and are not
committed, per the measurement-layout convention.

## Results (pinned)

- Census (both boots): `descend: candidates 420 (kept 420, covered 0) —
  ply 1: 20 kept, ply 2: 400 kept; base budget 4000000`
- Summary (boot A): `harvest: policy descend stop=exhausted jobs 420
  decisive 52 censored 368 evals 1474704787 new_shards 52 wall 361.9s`
  (boot B: identical counts, `evals 1474704787`, `wall 353.3s`)
- Yield: **52 decisive shards, all ply 2** (all `win`); ply 1: 20/20
  censored (the known cold-start depth). Decisive child-evals 0 (TT
  retention hits) to 2,504,216; shard nodes 38–334.
- Merged DB: 4,314 nodes (1 root + 4,256 overlay + 57 open-ancestor
  insertions), 6 open rows; `proofdb_flip`: `flips 0 verified 0`.
- Determinism (task 3): manifest bytes, ledger bytes, and all 52 shard
  files byte-identical across the two boots; job records identical
  (420/420 compared after `wall_s` strip).
- Handoff smoke (task 4): one `and-close` batch (`--budget-evals 200000
  --max-jobs 5`) on the grown layer — frontier `C1 6 / C2 69137 / C3 63`
  (vs root-only `C1 1 / C2 0 / C3 20`), `active rows 6` (the descend-opened
  ply-1 parents), jobs are C3 replies resuming at `pass 2` from the
  descend censor records (ladder `2 × work_done`); 5 censored, manifest
  unchanged. The layer's ledger was restored to the boot-B bytes after the
  smoke (the bumps were probe-only).
