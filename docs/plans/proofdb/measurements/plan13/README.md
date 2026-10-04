# plan13 measurements — the website handoff package (2026-10-02)

Executes `../../plan13.md` (item 5: website handoff + user manual).
Docs + measurement artifacts only — no `src/`/`examples/`/`tests/` changes.

**Deliverable:** a single directory the owner can forward to the external
website project. The DB was **rebuilt this session** from the committed
manifest + shard set by `proofdb_merge` (not copied from `data/`); the
digest equals the plan12 pin, confirming the byte-identical rebuild
guarantee end-to-end.

## Provenance

| file | content |
| --- | --- |
| `handoff/proofdb.db` | the rebuilt standing DB, sha256 `0d929f4c…` (= plan12 pin), 55,703 nodes (proven 55,628 / open 75), schema v1 |
| `handoff/HANDOFF.md` | the consumer-facing one-pager: contract pointer, digest + census, read-only consumption statement, merge-census provenance |
| `handoff/global_proof_store.md` | verbatim copy of `docs/spec/global_proof_store.md` incl. the new §7 example consumer queries (diff-verified identical) |
| `env.json` | environment + rev + input/output state |

Input state: committed manifest `e91ad57b…` (262 entries) + shard dir —
the only durable layer, re-verified this session. Merge census (verbatim,
handoff build): shards merged 262/262; overlay new 55360; dedupes 866;
open-ancestor insertions 342; open nodes upgraded 268; merged 55,703
(proven 55,628, open 75); skeleton re-validations 262.

## Session runs (transcript sources for the manual's pinned outputs)

All runs re-executed this session in `/tmp/proofdb_quickstart` per the
runbook §2 quickstart; raw transcripts are regenerable and not committed:

1. **Merge** (standing set): census + digest exactly the plan12 pins.
2. **Smoke harvest** (2 jobs, 200k evals): 2 censored `job:` lines
   (`"outcome":"censored"`, `pass`/`work_before` present, `number`/`kind`
   absent), `manifest: unchanged (262 entries, digest e91ad57b…`,
   `harvest: policy and-close stop=max-jobs jobs 2 decisive 0 censored 2
   evals 400037 new_shards 0 wall 3.2s`, exit 0.
3. **Flip**: `flip_analysis: open_rows 75 flips 0 verified 0 root Null
   (fixpoint 1 rounds)`; JSON `{"flips": [], "flips_total": 0, …,
   "open_rows": 75, "root_bound": null, "root_implied": null}`.
4. **Ledger union** (N=1 over the committed plan10 snapshot):
   `union: input … (8446 records): new_paths 8446, sole_pass_upgrades 0` →
   `union: 1 inputs -> 8446 records` →
   `ledger_union: work_union.json -> 8446 records`.
5. **Extra capture runs** for the manual's output-grammar sections:
   merge `--sample-lines 2` (two `sample i: <uci-path> (N plies, end: …)`
   lines); `inspect_pt`/`pt_keys` over shard `h_013861d97dce5d48.bin`
   (§5 provenance examples). The decisive `job:` line in the manual is the
   pinned record from `../plan10/census_armA.json` (a real executed run;
   re-running it was out of plan13's no-growth scope).

## Read-only DB probe (Python stdlib sqlite3)

- `meta`: all 5 rows present, `schema_version = 1`, `node_count = 55703`,
  `built_from` = full manifest digest, `root_fen` = startpos.
- Root row: `id=0, parent_id=NULL` (no parent ✓); 7 stored children, all
  open — the standing undecided root. **Note:** the plan's "≥1 proven
  child" probe is not satisfied at the root's direct children (and never
  was: the root is undecided); the accurate statement is 55,628 proven
  nodes deeper in the tree (27,768 win + 27,860 loss; depth_status
  bound 30,795 / exact 24,833; max ply 48; 24,833 rule-terminal rows).
- Recursive-CTE path reconstruction over node 55416 →
  `e2e4 h7h6 d1h5 e7e5 h5f7` (the same line the merger's `--sample-lines`
  walk found — independent cross-check); all §7 spec queries verified
  against the DB.

## Hygiene

- Handoff artifacts: DB (validated, digest-pinned) + two markdown files —
  all committable per the measurement conventions; no transcripts, no
  `*.tt` snapshots, no scripts (the driver was the shell + Python here).
- `git add --dry-run` checked: only `docs/proofdb_pipeline.md`,
  `docs/spec/global_proof_store.md`, and `docs/plans/proofdb/
  measurements/plan13/**` (plus the initiative/index/report edits).
