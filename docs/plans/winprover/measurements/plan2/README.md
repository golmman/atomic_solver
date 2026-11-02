# plan2 measurements — provenance

Measurement record behind [`../../report2.md`](../../report2.md): every
committed root-`Win` proof-tree shard exported to provari's JSON-lines format
and checked by the pinned independent oracle. Captured **2026-11-02** (UTC).

## Provenance

| Item | Value |
| --- | --- |
| Date | 2026-11-02 (UTC) |
| Host | container `67db7a1089e2`; aarch64, 4 CPU; `Linux 7.1.13-402.asahi.fc44.aarch64+16k`; glibc 2.43 |
| Runs | sequential, one `provari check` process per exported tree |
| Provari | `aiorla/provari` commit `48686bc6c786d6ea70568f0b2ad4002cf0520206` (`v0.1.0`) |
| Provari version | `provari 0.1.0, with its network and model built in` (already-built bundled scratch clone; the checker does not need the model) |
| Mode artifacts | not used, not fetched again for this plan (the checker is rules-only) |
| Provari binary | `/tmp/winprover-oracle/target/release/provari` (scratch clone; **not** committed) |
| `atomic_solver` | plan2 working tree on top of commit `b7d002ca5473cd07aa574fe04cfefdb586dcb90c` ("add winprover plan2") |
| Exporter | `target/release/examples/provari_export` (`cargo build --release --example provari_export`) |
| Sample | `docs/plans/proofdb/shards/`: 262 `*.bin` shards + `manifest.json`; shard manifest sha256 `e91ad57b44008fee65ab0b124fab51365c17d08f891777cde525cfecb61fb5fd` |
| Sample census | 262 scanned; 168 root-`Win`, 94 root-`Loss`, 0 root-`Draw`, 0 non-`ATOMTREE`; 56,226 nodes total (`root-Win` subset 33,020 nodes) |
| Budget | `provari check --seconds 60` per proof; no tablebases, no model |
| Exported JSONL | regenerable, written to a `TemporaryDirectory`, **not** committed |

## Files

- [`run_check.py`](run_check.py) — the driver: parses each `ATOMTREE` v1
  header, skips non-`Win` roots, exports with `provari_export`, runs one
  `provari check` per file, classifies the verdict, and writes the three
  result files.
- [`sample_manifest.json`](sample_manifest.json) — one row per `.bin`: file,
  sha256, size, root outcome, root depth, node count, root FEN (262 rows).
- [`results.json`](results.json) — sample dispositions, per-class counts, and
  per-tree verdicts (168 exported trees).
- [`results.csv`](results.csv) — the same per-tree verdicts as a table:
  `file, tag, root_outcome, nodes, export_lines, class, valid, won_positions,
  highest_counter, error`.
- `README.md` — this provenance record.

No raw transcripts, no exported JSONL, no provari artifacts, no
`__pycache__` (`sys.dont_write_bytecode`), no TT snapshots.

## Commands

Oracle (already present in the scratch directory; not rebuilt for this plan):

```sh
/tmp/winprover-oracle/target/release/provari --version
# provari 0.1.0, with its network and model built in
git -C /tmp/winprover-oracle rev-parse HEAD
# 48686bc6c786d6ea70568f0b2ad4002cf0520206
```

Exporter build and measurement (from the repo root):

```sh
cargo build --release --example provari_export

# smoke test: first three shards, inspect the manifest/results, then full run
python3 docs/plans/winprover/measurements/plan2/run_check.py --limit 3
python3 docs/plans/winprover/measurements/plan2/run_check.py
```

Full-sample runtime: ~0.5 s (the largest proof is 4,762 nodes and the oracle
accepts it well inside the 60 s per-proof limit).

## Result summary

- 168/168 root-`Win` trees exported (0 export errors) and **168/168 accepted**
  by provari; no `convention-repeat`, `unchecked`, `export-bug`, or `suspect`.
- The 94 root-`Loss` shards are out of scope (JSON-lines assumes the attacker
  is the side to move); no root-`Draw` shard exists.
- Negative controls (three corrupted copies of one accepted export) are all
  rejected by provari, so the acceptances are not vacuous.
- Exporter output is deterministic across two full runs (identical sha256 over
  all 262 per-shard outputs/errors).
- 25 accepted trees emit more attacker lines than provari counts won positions
  (their FENs differ only in the (half)move clock fields); on an
  en-passant-insensitive `(board, turn, castling, clock)` key those duplicate
  lines never carry a different move, so provari's last-line-wins map cannot
  change the proof.
