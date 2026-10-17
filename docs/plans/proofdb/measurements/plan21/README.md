# plan21 measurements — the OR-node sibling-skip probes (2026-10-16)

Executes the `../../plan21.md` §5 task 6 probes: the standing-layer
census probes (or-closed structure before/after the skip) and the
§3.2 reference-boot re-run. All runs are regenerable from this README;
no solver transcripts are committed.

## Or-closed structure probe (`p21-probe.py`, `p21-results.jsonl`)

Read-only parent-chain reconstruction over two layers:

| layer | nodes | loss rows (odd ply) | or-closed parents | nodes at-or-under one | censor records | dropped by the static rule |
| ----- | ----- | ------------------- | ----------------- | --------------------- | -------------- | -------------------------- |
| committed fixture (`docs/plans/proofdb/shards` → temp merge) | 55,703 | 23,977 | 23,885 | 50,258 (90.2%) | 0 (fresh copy) | — |
| dev shard set (`data/proofdb/shards`, 52 shards → temp merge, legacy `work.json` censor rows imported) | 4,314 | 2,154 | 2,154 | 4,308 | 368 | **0** |
| standing layer (`data/proofdb/proofdb.db`, restored after the session — see report21 §6 on the file swap) | 43,092 | 6,306 | 6,306 | 12,612 (29.3%) | 3,617 | **0** |

On both live layers the censor frontier sits entirely outside the
or-closed subtrees (pre-plan21 sessions harvested the territory under
them before any or-close could fire), so the plan21 static drop does not
bite on them today — it bites going forward, as soon as a session
censors inside territory a later merge proves.

## §3.1 quickstart re-pin (dev container, release build)

```
frontier-walk: records 0 (lineage 0, ancestor 0, or_closed 0, out_of_scope 0, complete 0) — root open; base budget 200000; scope global
job: {"budget":200000,"child_evals":200000,...,"path":"",...,"outcome":"censored","pass":1,"work_before":0}
job: {"budget":200000,"child_evals":200015,...,"path":"a2a3",...,"outcome":"censored","pass":1,"work_before":0}
harvest: policy frontier-walk stop=max-jobs jobs 2 decisive 0 censored 2 evals 400015 new_shards 0 or_closes 0 wall 0.1s
```

The child-eval overshoot is **200015** here vs the manual's previous
reference-host pin 200021 (report20); verified identical against the
pristine HEAD build — a host/build dependence of the chunk-overshoot
accounting, unrelated to plan21 (the smoke session has no or-closed
structure and no decisive job). Recorded as a report21 finding; the
manual pins the container values and flags the host-dependence.

## §3.2 reference boot under the skip (dev container)

Protocol as in the plan16 gate: root-only layer, `--budget-evals 4000000
--max-total-evals 2000000000 --tt-mb 1024`, then a merge.

```
harvest: policy frontier-walk stop=budget jobs 549 decisive 49 censored 500 evals 2000051685 new_shards 49 or_closes 0 wall 414.3s
census: … merged nodes 4030 (proven 4024, open 6); …
db: … (nodes 4030), manifest built_from 64301f268490ff71
flip_analysis: open_rows 6 flips 0 verified 0 root Null (fixpoint 1 rounds)
probe: 2,012 or-closed parents; 4,024/4,030 nodes at-or-under one; 500 censor records, 0 dropped
```

**Isolated comparison (`p21-boot-comparison.jsonl`):** the pre-plan21
HEAD build run on the same container reproduces the boot **byte-
identically** (same censor rows, all 49 shards, same merged DB, same
manifest digest) — the skip never fires on a fresh layer: all 49
decisive boot jobs are wins at odd-ply (black-to-move) nodes, i.e.
refutations, so `or_closes 0` and the static `or_closed` set is empty.
The plan21-critical behavior is therefore invisible in the boot and the
manual's boot pin re-pinned purely for the pre-plan21 drift (the plan16
reference-host pin 553/53/6,706 predates the plan20 chunk-accounting
change and this container).
