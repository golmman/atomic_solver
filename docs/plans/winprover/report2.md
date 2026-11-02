# Winprover Report 2 — differential proof-checker oracle

Initiative: `winprover`, backlog **item 2**, executed as
[`plan2.md`](plan2.md). Validation/evidence work: it adds a proof-tree
exporter and measurement tooling, but no search, TT, proof-construction,
proof-format, or proofdb DB change. Date: **2026-11-02** (UTC).
Verdict: **CLEAN** — every sampled root-`Win` tree is accepted by the pinned
independent checker (168/168). Next: **item 3**, the win-only front-end
skeleton (with the item-4 sibling-simulation census in its plan session).

## Headline

- Added `src/proof_tree/export.rs`: `write_provari_jsonl` validates and
  replays a finalized tree, then writes provari's JSON-lines proof format.
  Four unit tests pass in the default gate; the walk is an explicit DFS stack
  and the output is byte-deterministic across two full runs.
- Added `examples/provari_export.rs` (`[--out PATH] <proof_tree.bin>`) and a
  provenance-carrying measurement driver
  (`measurements/plan2/run_check.py`).
- Checked **all 168** committed root-`Win` proof-tree shards against the
  pinned `provari check` (`aiorla/provari` `48686bc`, `--seconds 60`, no
  tablebases): **168 accepted, 0 non-accepts**. The 94 root-`Loss` shards are
  out of scope by plan.
- The acceptances are not vacuous: three corrupted copies of an accepted
  export are all rejected by the checker (missing attacker line, corrupted
  move, malformed FEN).
- No `convention-repeat`, `unchecked`, `export-bug`, or `suspect` in the
  sample, so there is no defect to minimize and nothing to change in search,
  proof construction, or the proof DB.
- **No ledger row** (item 2 does not touch the win path).

## 1. What was done

1. Confirmed the oracle: the pinned scratch clone at
   `/tmp/winprover-oracle` is at `48686bc6c786d6ea70568f0b2ad4002cf0520206`
   and reports `provari 0.1.0, with its network and model built in`. The
   checker is rules-only (shakmaty, no tablebases, no model), so no rebuild or
   artifact fetch was needed.
2. Re-read the current `ATOMTREE` v1 format and the retired
   [`interop/plan1.md`](../interop/plan1.md) draft, then implemented the
   exporter against the current binary reader (`ProofTree::from_bin`) and the
   validator (`validate_proof_tree`).
3. Added four unit tests: mate-in-1 export (header + one line, and the line
   replays to the same move), root-`Loss` rejection (buffer untouched),
   invalid-tree rejection with the validator's defects (buffer untouched),
   and a loaded-from-binary tree (`hash == 0`) exporting identically.
4. Added the example binary and registered the module
   (`pub mod export;` plus re-exports), and updated `AGENTS.md` (module
   description and Examples list) in the same change.
5. Wrote `run_check.py` (header parse + skip rules + export + one
   `provari check` per file + classification + manifest/results/CSV), smoke
   tested on three shards, then ran the full committed shard set.
6. Triaged the sample (all accepts), ran the negative controls and the
   determinism check, ran the gate, and wrote this report.

## 2. Provenance

| Item | Value |
| --- | --- |
| Date | 2026-11-02 (UTC) |
| Host | container `67db7a1089e2`; aarch64, 4 CPU; `Linux 7.1.13-402.asahi.fc44.aarch64+16k`, glibc 2.43 |
| Provari | `aiorla/provari` `48686bc6c786d6ea70568f0b2ad4002cf0520206` (`v0.1.0`) |
| Provari version | `provari 0.1.0, with its network and model built in` (pre-built bundled scratch clone; the checker does not read the model) |
| `atomic_solver` | plan2 working tree on top of `b7d002ca5473cd07aa574fe04cfefdb586dcb90c`, release build |
| Exporter | `target/release/examples/provari_export` |
| Sample | `docs/plans/proofdb/shards/`: 262 `*.bin` + `manifest.json` (`manifest.json` sha256 `e91ad57b44008fee65ab0b124fab51365c17d08f891777cde525cfecb61fb5fd`) |
| Sample census | 262 scanned; 168 root-`Win`, 94 root-`Loss`, 0 root-`Draw`, 0 non-`ATOMTREE`; 56,226 nodes (`root-Win` subset 33,020) |
| Budget | `provari check <proof> --seconds 60` per proof; no `--tablebases` |
| Export artifacts | JSONL in a `TemporaryDirectory`, not committed |

The sample is **historic**, not a fresh solve: the shards are the committed
proofdb transport artifacts. This report validates those trees; it does not
generalize to proof shapes the current solver has not produced.

## 3. Exact commands

Oracle (already present; nothing committed):

```sh
/tmp/winprover-oracle/target/release/provari --version
# provari 0.1.0, with its network and model built in
git -C /tmp/winprover-oracle rev-parse HEAD
# 48686bc6c786d6ea70568f0b2ad4002cf0520206
```

Exporter build and measurement (repo root):

```sh
cargo build --release --example provari_export

# smoke: first three shards
python3 docs/plans/winprover/measurements/plan2/run_check.py --limit 3
# {"scanned": 3, "not_a_tree": 0, "root_loss_skipped": 1, "root_draw_skipped": 0,
#  "root_win": 2, "root_win_exported": 2, "export_errors": 0}
# {"accepted": 2, "root-loss-skipped": 1}

# full committed sample
python3 docs/plans/winprover/measurements/plan2/run_check.py
# {"scanned": 262, "not_a_tree": 0, "root_loss_skipped": 94, "root_draw_skipped": 0,
#  "root_win": 168, "root_win_exported": 168, "export_errors": 0}
# {"accepted": 168, "root-loss-skipped": 94}
```

Gate (see §8) plus the plan's Python validation (`ast.parse`, no
`py_compile`), the two-JSON parse check, and the per-shard manual check:

```sh
target/release/examples/provari_export --out /tmp/wpexp/test.jsonl \
  docs/plans/proofdb/shards/h_013861d97dce5d48.bin
# positions: 34, nodes: 68, root: win
/tmp/winprover-oracle/target/release/provari check /tmp/wpexp/test.jsonl --seconds 60
# {"highest_counter":0,"moves":null,"proof":"...","valid":true,"won_positions":34}
```

## 4. Exporter

**Path:** `src/proof_tree/export.rs` (+ `export/tests.rs`, so the module file
stays under the size guideline; the tests remain part of the `export` module
and run in the default gate). Public API:
`write_provari_jsonl<W: Write>(&ProofTree, &mut W) -> Result<usize, ExportError>`,
`ExportError::{Invalid(Vec<TreeDefect>), RootNotWin(Outcome), Malformed(String), Io(io::Error)}`
with `Display`, `Error`, and `From<io::Error>`.

Behavior (all normative in the module header): it runs
`validate_proof_tree` first; an invalid tree is `Invalid`, a non-`Win` root is
`RootNotWin`, and neither writes anything. It then replays the tree from
`tree.root_fen` with `Position` on an explicit DFS stack
(`Enter`/`Exit`): a non-terminal `Win` node emits
`{"fen":"<position>","move":"<proving child move>"}` (the position the move is
played *in*, never the child position) and descends into its single child; a
`Loss` node emits nothing and descends into every child; a terminal
(`depth == 0`) node emits nothing and is not descended into. The header is
`{"root":"<root FEN>"}` with no `provari` field (a `provari` field would route
the file to the compact reader). Strings are JSON-escaped by a small writer,
so no new dependency is needed. It returns the number of attacker lines
emitted.

**Unit-test result:** 4/4 pass (`cargo test --release export`):
`mate_in_one_exports_header_and_one_line`,
`root_loss_is_rejected_without_writing`,
`invalid_tree_is_rejected_without_writing`, `loaded_from_bin_tree_exports`.

**Example:** `examples/provari_export.rs`
(`provari_export [--out PATH] <proof_tree.bin>`), writes only on success,
exits non-zero on any error, and prints
`positions: N, nodes: M, root: win|loss|draw` to stderr.

On the full sample the exporter's per-tree line counts always equal half the
node count (every `Win` node is non-terminal in these trees, and their
`Win`/`Loss` counts are equal), so `export_lines = nodes / 2` for all 168
trees (16,510 lines total, 4,762 nodes / 2,381 lines at the largest).

## 5. Sample and dispositions

`measurements/plan2/sample_manifest.json` records one row per `.bin`
(file, sha256, size, root outcome, root depth, node count, root FEN). Its
aggregate matches the plan-session probe exactly (262 / 168 / 94, 56,226
nodes, largest 4,762 nodes at 28,642 bytes, sizes 80–28,642 bytes), so the
shard set did not drift between drafting and execution.

| Disposition | Count |
| --- | --- |
| `.bin` shards scanned | 262 |
| not an `ATOMTREE` v1 tree | 0 |
| root-`Loss` skipped (plan scope) | 94 |
| root-`Draw` skipped | 0 |
| root-`Win` exported | 168 |
| export errors (`Invalid`/`Malformed`/I/O) | 0 |

Root-`Win` subset: 33,020 nodes (min 2, max 4,762); root depths 1–21 as
stored in the header; root halfmove clocks 0–4.

## 6. Verdicts and classification

Every exported file gets exactly one `provari check --seconds 60`; the driver
parses the single JSON verdict line and classifies it per the plan's table.

| Class | Count | Notes |
| --- | --- | --- |
| `accepted` (`valid: true`) | **168** | the whole exported root-`Win` sample |
| `convention-repeat` (`valid: false`, repeated place) | 0 | — |
| `unchecked` (`valid: null`) | 0 | no time/size/depth/tablebase limit hit |
| `export-bug` | 0 | no rejected line/FEN/move |
| `suspect` | 0 | no unexplained rejection |
| root-`Loss` skipped | 94 | plan scope, not a verdict |
| not a tree | 0 | — |

Accepted-tree details (`measurements/plan2/results.csv`): 16,510 exported
attacker lines; 14,614 distinct attacker positions counted as won by the
checker; `highest_counter` (provari's report of the root's own 50-move
counter) 0 on 104 trees, 1 on 51, 2 on 10, 3 on 2, 4 on 1.

**No `suspect` / `export-bug` evidence exists to minimize.** Because there is
none, no cross-oracle (manual replay / `egtb_gen3`) minimization was needed
beyond the negative controls below.

### Duplicate attacker lines (recorded occurrence, not a defect)

25 of the 168 accepted trees emit more attacker lines than the checker counts
won positions (`export_lines != won_positions`); the differences are 1 line
(small trees) up to 740 lines (the 4,762-node tree). The exported FENs differ
only in their clock fields, which the JSON-lines checker keys on
`(place, halfmove_clock)`. Re-deriving the keys from the exported lines with
an en-passant-insensitive `(board, turn, castling, clock)` key gives 1,896
duplicate lines across those 25 trees and **zero cases where the duplicates
disagree on the winning move**, so provari's "last line wins" map cannot flip
any of these proofs. This is the duplicate-position risk the plan flagged;
it occurred benignly.

### Negative controls (the checker is not vacuous)

Three corrupted copies of one accepted export (`h_013861d97dce5d48`, 34
attacker lines) were checked:

| Control | Corruption | Verdict |
| --- | --- | --- |
| missing attacker line | dropped the root move line | `valid: false`, `the proof has no move for <root FEN>` |
| corrupted move | replaced the first move with a legal but wrong rook move | `valid: false`, `the proof has no move for <later FEN>` |
| malformed FEN | replaced the side-to-move field of line 4 | `valid: false`, `a bad FEN …: invalid turn part in fen` |

So an acceptance means the checker replayed our attacker moves and every
defender reply; a silently dropped or altered line is caught.

### Determinism

Two full runs of the exporter over all 262 shards produce byte-identical
outputs (sha256 over all per-shard exports and error texts
`bcadc13f49d4a32153ba8bcba3c99b96ddceda1f8ed7cf51bf27c3d3c94ca3f7` for both
runs), and the driver's committed files are regenerable.

## 7. Interpretation

- This is evidence from a second rules implementation: provari's checker uses
  `shakmaty` rules, shares no code with its prover, and regenerates every
  defender reply itself. Our validator, `proofdb_verify`, and the
  reconstruction validator all share `atomic-movegen`/`Position`, so this
  closes the correlated-rules blind spot for **these proof shapes**.
- The sample exercises real branching: 168 trees, 33,020 nodes, up to 4,762
  nodes and 2,381 attacker lines per tree, root halfmove clocks 0–4, and
  root-`Loss` (attacker-to-move-loss) shards excluded by construction.
- No evidence of a rules discrepancy on this sample: not in terminal claims,
  not in legal-reply coverage, not in FEN parsing, not in repetition
  conventions. Both directions were exercised: the mate-in-1 fixture proves the
  export is minimal, and the negative controls prove acceptance is meaningful.
- Limitation: only root-`Win` trees (the JSON-lines attacker assumption).
  Root-`Loss` proofs and the compact `.pvr` path remain horizon item 8; the
  shard set is historic, and nothing here validates proof shapes the current
  solver has not produced.

## 8. Gate

| Check | Result |
| --- | --- |
| `cargo fmt --check` | clean |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `make test` (release fast tier) | green (4 new unit tests pass; all previously ignored slow tests still ignored) |
| `make test-full` (release, `--include-ignored`) | green — 37/37 test groups ok, 872 passed, 0 failed, 0 ignored (run because the change adds a module under `src/proof_tree/`) |
| `cargo doc --no-deps` | clean |
| `cargo build --release --example provari_export` | clean |
| Python `ast.parse` of `run_check.py`, both output JSONs parse | clean |

`make test-full` was **also** run (the plan did not require it, but the new
module lives under `src/proof_tree/`, which `AGENTS.md` lists as requiring the
full tier): 872 tests passed, 0 failed, 0 ignored across all 37 test groups,
including the 60 s regression/stress suites. No search, TT, move-ordering,
proof-construction, or proof-format behaviour changed; the export module is
additive.

## 9. Not measured (explicit)

- Root-`Loss` / root-`Draw` proof export and the compact `.pvr` format
  (horizon item 8).
- Tablebases (none used; our trees are expanded to rule terminals) and NNUE /
  the stretch `--guide model` column (irrelevant to checking).
- Fresh solves, new corpus cases, full `high200`, and any wall/eval
  measurement on the win path.
- No ledger row was appended to [`bench.md`](bench.md) (item 2 does not touch
  the win path); `results.json` / `bench-v0` untouched.

## 10. Recommended plan

**Item 3 — win-only front-end skeleton** (`plan3.md`), whose plan session
folds in the read-only item-4 sibling-simulation census. Evidence:

- Item 2's gate for item 3 is now satisfied: the exporter and the
  independent checker are in place, deterministic, and exercised on every
  committed root-`Win` shape. New attacker-only proof shapes can be exported
  and independently checked as part of item 3's acceptance rather than after
  the fact.
- The exporter is reusable: `provari_export` works on any
  `ATOMTREE` v1 root-`Win` dump, including one produced by
  `reconstruct_pt` from a future front-end run.
- The sample found no rules discrepancy, so no defect-fix plan in an owning
  initiative is needed; the ranking's next item stands.

Alternative if item 3 is not funded: the evidence thread has one cheap,
independent extension (item 8's root-`Loss` export) — but it does not unblock
the search levers, so it is not the recommended next session.

## 11. Deliverables

- `src/proof_tree/export.rs` + `src/proof_tree/export/tests.rs` (new module,
  4 unit tests), `src/proof_tree/mod.rs` registration/re-exports.
- `examples/provari_export.rs` (new example).
- `AGENTS.md`: `src/proof_tree/` description and Examples list.
- `measurements/plan2/`: `README.md` (provenance), `run_check.py` (driver),
  `sample_manifest.json`, `results.json`, `results.csv`.
- This report, plus the `initiative.md` item-2 status and the
  `docs/plans/README.md` winprover row.
- No `__pycache__`, no raw transcripts, no exported JSONL, no provari
  binary/source/model in the tree.
