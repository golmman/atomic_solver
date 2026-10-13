# plan16 measurements — the `frontier-walk` gates (2026-10-13)

Executes `../../plan16.md` tasks 2–4: the fresh-layer boot gate (task 2),
the determinism gate (task 3), and the grown-layer smoke (task 4). The
integration test (task 5, `walk_boot_from_empty_layer`) lives in
`tests/proofdb.rs` and runs in the default gate; no separate artifact.

## Boot protocol (both boots identical)

```
fresh layer
printf '{"entries":[]}' > data/proofdb/shards/manifest.json
proofdb_merge                                            # root-only DB, built_from d801aa1fb7ddcc33
proofdb_harvest --policy frontier-walk --max-total-evals 2000000000
proofdb_merge                                            # 6,706 nodes
proofdb_flip                                             # flips 0 verified 0
```

The base budget is the 4M default (`--budget-evals` 0). The 2G eval cap is
added because the walk does not self-retire (plan16 D6): on an undecided
layer it always has work, so an uncapped session does not terminate — the
plan's "(defaults)" boot command is understood as defaults-plus-a-cap (a
report finding, not a deviation from the policy semantics).

Boot B repeats the whole protocol in a second fresh layer (task 3).

## Boot yield (boot A)

`stop=budget jobs 553 decisive 53 censored 500 evals 2001311002 wall
216.2s`; merged DB 6,706 nodes (1 root + 6,647 overlay + 58
open-ancestor insertions); `flips 0 verified 0`. The visit mix: 1
search-kind visit (the virtual root, class `C1`) + 552 expansion visits
(class `C3`) — after the root censor, expansion replaces deepening
exactly as designed. 44 of the 53 decisive shards were zero-eval TT hits.
First decisive in walk order: fresh search #135 (`c2c3 g7g5`, win-in-a-line
at 6,990 evals), reached after ~620M cumulative child-evals (~2.5 min on
the reference host).

## Provenance

| file | content |
| --- | --- |
| `env.json` | environment + rev + protocols + output-state digests + determinism flags + the grown-layer smoke summary |
| `boot_census.json` | boot A: session-start lines, summary line, visit-kind split, waste measurement, and the 553 `job:` records (`wall_s` stripped — host-dependent) |

Raw transcripts (`/tmp/wsmoke/*`, `/tmp/wbootB/*`, `/tmp/wgrown/*`) are
regenerable from the protocols above and not committed.
