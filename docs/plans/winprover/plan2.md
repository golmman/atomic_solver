# Winprover Plan 2: differential proof-checker oracle

Initiative: `winprover`, backlog **item 2**. Drafted 2026-11-02. This is a
**validation/evidence** plan: it adds a proof-tree exporter and measurement
tooling, but no search, TT, proof-construction, or proof-format change.

Prerequisite reading: [`initiative.md`](initiative.md), [`bench.md`](bench.md),
[`AGENTS.md`](../../../AGENTS.md),
[`docs/spec/proof_tree_dump.md`](../../spec/proof_tree_dump.md), and the
retired draft [`interop/plan1.md`](../interop/plan1.md). The old draft is the
design source; this plan re-cuts it against the current `ATOMTREE` v1 binary
format and the current committed shard set.

## Why this plan exists

Every proof check in this repository replays with our own rules
implementation: `proof_tree::validate_proof_tree`, `proofdb_verify`, and the
reconstruction validator all use `atomic-movegen` through `Position`. A
mistake in that shared layer would be invisible to all of them.

Provari is an independent Atomic prover with an independent checker
(`src/check.rs` in provari): it uses `shakmaty` rules, shares no code with its
search, and regenerates every defender reply from an attacker-only proof. If
we export a proof tree in its JSON-lines format and it accepts, that is
evidence from a second rules implementation.

The initiative ranks this item second because it **gates trust in item 3**:
the attacker-only front-end will produce new proof shapes, and those shapes
must be independently checkable before they are trusted. The exporter is also
reusable in later campaigns.

The plan is deliberately narrow: **root-`Win` trees only** (JSONL assumes the
attacker is the side to move), one external checker, a reproducible sample,
and triage. Root-`Loss` coverage and the compact `.pvr` format are horizon
item 8.

## Plan-session probe (2026-11-02, read-only)

- The pinned oracle source is present at `/tmp/winprover-oracle` and reports
  `provari 0.1.0, with its network and model built in`. The checker does not
  need the model; a plain build is sufficient.
- `docs/plans/proofdb/shards/` contains **262** `*.bin` files (plus
  `manifest.json`). Parsing the `ATOMTREE` headers gives **168 root-`Win`**
  and **94 root-`Loss`** trees; no root-`Draw` trees. Node counts range from
  2 to 4,762 (56,226 nodes total); root depths are at most 21; file sizes are
  80–28,642 bytes.
- The JSONL contract in the retired draft is still accurate against the
  pinned source: first non-empty line is `{"root":"<FEN>"}` (no `provari`
  field, or provari routes to the compact reader); each following line is
  `{"fen":"<FEN>","move":"<UCI>"}`; the checker loads all moves first, then
  walks from the root, looks up attacker moves by `(place, halfmove_clock)`,
  generates all defender replies itself, and rejects any cycle among places.
- The current binary format is `ATOMTREE` v1: header magic/version/root FEN/
  root outcome/root depth, then 6-byte `(parent_id, move_code)` records.
  Loaded trees carry `hash == 0`, so the validator skips hash checks; replay,
  terminal, depth, and completeness checks still run.

## Scope and non-goals

In scope:

- add `src/proof_tree/export.rs` with a tested `write_provari_jsonl` API;
- add `examples/provari_export.rs`;
- register the module and update `AGENTS.md`;
- add a measurement driver under `measurements/plan2/`;
- export every root-`Win` shard, run `provari check`, classify every verdict;
- write `report2.md` and update the initiative/README status.

Out of scope (do not do these here):

- no root-`Loss` export, no compact `.pvr` format (horizon item 8);
- no search, TT, proof-construction, proof-tree dump-format, or proofdb DB
  change;
- no new corpus, no fresh solve, no tablebases, no NNUE;
- no default-flag decision, no ledger row (this plan does not touch the win
  path);
- no GPL code import, no `Cargo.toml` dependency, no committed provari
  binary/source/model.

## Oracle setup (scratch only, never committed)

Pinned oracle: `aiorla/provari` commit
`48686bc6c786d6ea70568f0b2ad4002cf0520206` (`v0.1.0`), GPL-3.0-or-later.
Do not copy code, do not add a dependency, do not vendor the binary or the
network/model files.

If `/tmp/winprover-oracle` is absent, rebuild it:

```sh
rm -rf /tmp/winprover-oracle
git clone https://github.com/aiorla/provari /tmp/winprover-oracle
git -C /tmp/winprover-oracle checkout 48686bc6c786d6ea70568f0b2ad4002cf0520206
cargo build --release --manifest-path /tmp/winprover-oracle/Cargo.toml
/tmp/winprover-oracle/target/release/provari --version
```

The checker does not need the bundled network/model; a plain build is enough.
If the clone or build fails (network/toolchain/glibc), record the blocker and
stop as **`INCONCLUSIVE`**. Do not substitute an untrusted checker.

## JSONL contract (verified from provari's `src/check.rs`)

First non-empty line: a JSON object with `root` (the root FEN). It must **not**
contain a `provari` field, or provari will route it to the compact reader.

Each following non-empty line: `{"fen": "<position FEN>", "move": "<UCI>"}`.
The checker loads all moves first, then walks from the root:

- at an attacker position it looks up `(place, halfmove_clock)`;
- if the position is already won by an immediate capture/mate, no stored move
  is needed;
- at a defender position it generates **all** legal replies itself;
- it rejects any cycle among the positions (places) the proof reaches.

Root FEN and every line FEN must carry the exact halfmove clock of the
position being proved; that clock is part of the checker's move key. JSONL
does not name tablebases; our trees are fully expanded to rule terminals, so
the checker should not need them. The checker's `moves` header field is not
validated for JSONL; we emit only `root`.

Provari's no-repetition rule is stronger than our clock-inclusive cycle check:
it compares places without the halfmove counter. A rejection for a repeated
place is a **convention difference**, not automatically a defect.

## Exporter deliverable

Add a small, tested proof-tree export module and a thin example:

- `src/proof_tree/export.rs`
  - `pub fn write_provari_jsonl<W: Write>(tree: &ProofTree, writer: &mut W)
    -> Result<usize, ExportError>`.
  - It first runs `validate_proof_tree(tree)`; an invalid tree is an
    `ExportError::Invalid`, never a silently written export.
  - It rejects a root whose outcome is not `Win` with
    `ExportError::RootNotWin(outcome)` (root-`Loss` is horizon item 8).
  - It replays the tree from `tree.root_fen` with `Position`, applies/undoes
    moves, and walks:
    - `Win` node: if terminal (`depth == 0` or no child), emit nothing; else
      emit `current_fen` + the single proving child's move, then descend into
      that child;
    - `Loss` node: if terminal, emit nothing; else descend into every child,
      emitting nothing;
    - `Draw`/unrealized node: unreachable after validation; return
      `ExportError::Malformed` defensively.
  - Use an explicit DFS stack with `Enter`/`Exit` frames; proof trees can be
    deep and must not recurse. Every non-root `Enter` plays its move and
    pushes an `Exit` that undoes it, including terminal nodes.
  - The header is `{"root":"<tree.root_fen>"}`. Write JSON directly; FEN and
    UCI strings are JSON-safe by construction, but use a small string writer
    that escapes `"`, `\`, and control characters. No new dependency is
    needed.
  - Return the number of emitted attacker lines, for reporting.
  - `ExportError` variants: `Invalid(Vec<TreeDefect>)`, `RootNotWin(Outcome)`,
    `Malformed(String)`, `Io(io::Error)`; implement `Display`, `Error`, and
    `From<io::Error>`.
- `examples/provari_export.rs`
  - Usage: `provari_export [--out PATH] <proof_tree.bin>`.
  - Reads the tree with `ProofTree::from_bin`, calls the export function, and
    writes to stdout or `--out`.
  - Prints a one-line summary to stderr (`positions: N`, `nodes: M`,
    `root: win`).
  - Exits non-zero on any export error.
- Register the module in `src/proof_tree/mod.rs` (`pub mod export;`); update
  the `src/proof_tree/` description and Examples list in `AGENTS.md` in the
  same change.

### Unit tests (in `export.rs`)

1. The existing hand-built mate-in-1 fixture
   (`k7/8/1Q6/8/8/8/8/7K w - - 0 1`, `b6b7`) produces the header plus one
   line; replaying the output independently yields the same move.
2. A valid root-`Loss` tree returns `ExportError::RootNotWin` and writes no
   header (check the writer buffer is empty). Use a checkmated root such as
   `7K/8/8/8/8/8/1Q6/k7 b - - 0 1`.
3. An invalid tree returns `ExportError::Invalid` with the validator's
   defects (e.g. a childless non-terminal `Win` root) and writes no header.
4. A loaded-from-binary tree with `hash == 0` exports successfully (the
   validator skips hash checks; replay still works).

The tests stay in the default gate (`make test`); no slow tests are needed.

## Sample

Use the committed proof-tree shards as the first corpus:

```sh
ls docs/plans/proofdb/shards/*.bin
```

At drafting there are 262 `.bin` files: 168 root-`Win`, 94 root-`Loss`. The
driver must:

1. scan `docs/plans/proofdb/shards/*.bin` (ignore `manifest.json` and any
   non-`.bin` file);
2. read each file and require the `ATOMTREE` magic; record anything else as
   `not-a-tree`;
3. parse the root outcome and node count from the header; record root-`Loss`
   as `root-loss-skipped` and root-`Draw` as `root-draw-skipped` (plan scope);
4. run the exporter (which validates first) for every root-`Win` tree;
5. record root outcome, node count, emitted line count, and either an export
   error class or the JSONL path;
6. run `provari check` on each exported file with `--seconds 60`; classify a
   timeout as `unchecked`, not as a defect.

The sample list and its SHA-256 manifest are committed under
`docs/plans/winprover/measurements/plan2/`; the exported `.jsonl` files
themselves are regenerable and are **not** committed. The sample is historic,
not a fresh solve: the report must say what was validated and not generalize
beyond the sample.

## Run and triage

For each exported file:

```sh
/tmp/winprover-oracle/target/release/provari check proof.jsonl --seconds 60
```

Provari prints one JSON line per proof (`valid: true|false|null`) and exits 1
unless all are valid. The driver captures stdout/stderr in memory, parses the
verdicts into a committed JSON/CSV table, and classifies:

| class | condition | action |
|---|---|---|
| `accepted` | `valid: true` | record; no action |
| `convention-repeat` | `valid: false`, error reports a repeatable position (`a line can repeat the position`) | inspect the tree's place-cycle; if real, record as a convention difference (provari's no-repeat rule is stronger than our clock-inclusive cycle check), not a defect |
| `unchecked` | `valid: null` (time/size/depth/tablebase limit) | record the limit; do not count as failure |
| `export-bug` | the exporter fails (I/O, malformed tree, root FEN parse), or provari rejects a line/FEN/move the exporter produced (`a bad FEN`, `a bad move`, `is not legal in`, `a bad line`, `a line without a position`, `a bad header`, `an empty proof`, `no root`) | fix the exporter in this plan; re-run |
| `suspect` | our validator rejects a committed shard, or provari rejects a proof for any other reason (`the proof has no move for`, `the game ends without the attacker winning`, `the 50-move limit is reached`, `the root is over or at the 50-move limit`, etc.) | stop and minimize; this is the plan's finding |

For every `suspect`, minimize to a small FEN/tree if possible and check the
case against at least one other oracle: our `validate_proof_tree`, a manual
replay, or `examples/egtb_gen3` for ≤3 men. Do **not** change search,
proof-tree construction, or the proof DB in this plan; report the defect and
propose the owning initiative's follow-up.

## Pre-registered completion criteria

The plan completes when all three hold:

1. the exporter is implemented, tested, and deterministic;
2. every sampled root-`Win` tree has a provari verdict, and every non-accept
   has a class and a reproducible reason;
3. `report2.md` states **CLEAN** (no `suspect`/`export-bug`) or **FIND** (at
   least one), with the minimized evidence and a recommendation for item 3,
   item 8, or a defect-fix plan.

There is no accept-rate threshold: one unexplained rejection is the signal.
`INCONCLUSIVE` is reserved for an oracle build/run blocker, not for a proof
that could not be checked within provari's limits (that is `unchecked`).

## Measurement driver

Add `docs/plans/winprover/measurements/plan2/run_check.py`:

- `sys.dont_write_bytecode = True`;
- arguments: `--shard-dir` (default `docs/plans/proofdb/shards`),
  `--exporter` (default `target/release/examples/provari_export`),
  `--provari` (default `/tmp/winprover-oracle/target/release/provari`),
  `--out-dir` (default `docs/plans/winprover/measurements/plan2`),
  `--seconds` (default 60), `--limit` (optional smoke-test cap);
- writes `sample_manifest.json` with one row per `.bin`: file, sha256, size,
  root outcome, root depth, node count, root FEN;
- writes `results.json` with the sample dispositions, per-tree verdicts, and
  per-class counts;
- writes `results.csv` with columns: file, tag, root_outcome, nodes,
  export_lines, class, valid, won_positions, highest_counter, error;
- uses a `tempfile.TemporaryDirectory` for exported JSONL and deletes it at
  the end; never writes raw transcripts or `__pycache__`;
- runs one `provari check` per exported file (isolation and simple mapping);
  a non-JSON output is recorded as `suspect` with a short error string.

## Tasks

1. **Read and confirm.** Re-read this plan, `initiative.md`, `bench.md`,
   `docs/spec/proof_tree_dump.md`, and the retired `interop/plan1.md`.
   Confirm the pinned commit, the GPL/no-import boundary, and the current
   `ATOMTREE` v1 format.
2. **Build/verify the oracle** per the setup section. Record
   `provari --version`, commit, and host/container notes in the provenance
   README. Do not commit the clone or binary.
3. **Implement `src/proof_tree/export.rs`** per the exporter section,
   including the four unit tests.
4. **Add `examples/provari_export.rs`** and register the module in
   `src/proof_tree/mod.rs`.
5. **Update `AGENTS.md`**: add the export module to the `src/proof_tree/`
   description and `provari_export` to the Examples list.
6. **Run the gate** (`cargo fmt`, `cargo clippy --all-targets`,
   `make test`, `cargo doc --no-deps`) and build the example.
7. **Write `run_check.py`** per the measurement-driver section.
8. **Smoke test** on 3 root-`Win` shards (`--limit 3`), inspect the manifest
   and results, then run the full sample.
9. **Triage** every non-accept per the class table; minimize any `suspect`.
10. **Write `report2.md`** (final task). Include: provenance and exact
    commands; exporter path and unit-test result; sample manifest and counts
    (scanned / not a tree / root-`Loss` skipped / root-`Win` exported /
    export errors); verdict table and per-class counts; all
    `suspect`/`export-bug` evidence; gate results; the CLEAN/FIND verdict;
    and the recommended next plan.
11. **Update `initiative.md`** (item 2 done with a one-line summary, item 3
    next, dated history entry) and the `docs/plans/README.md` winprover row.
    Append **no ledger row**.
12. **Clean up.** No `__pycache__`, no raw logs, no exported JSONL, no
    provari artifacts, no TT snapshots. Confirm with `git status --short` and
    `git add --dry-run` that only the intended files would be added.

## Acceptance

- The exporter is implemented, tested, deterministic, and rejects invalid
  trees and non-`Win` roots before writing anything.
- Every sampled root-`Win` tree has a provari verdict or a classified export
  error; every non-accept has a class and a reproducible reason.
- `report2.md` states CLEAN or FIND, with the evidence and a recommendation.
- No search, TT, proof-construction, proof-format, or proofdb DB changes.
- `make test` is green; `cargo fmt`, `cargo clippy --all-targets`, and
  `cargo doc --no-deps` are clean.
- `initiative.md` and the `docs/plans/README.md` row are updated; no ledger
  row was appended.
- `git status --short` shows only the intended source, example, doc, and
  measurement files; no provari binary/source/model, raw log, exported
  JSONL, or `__pycache__` is staged.

## Validation

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
make test
cargo doc --no-deps
cargo build --release --example provari_export
python3 - <<'PY'
import ast
from pathlib import Path
for p in [
    "docs/plans/winprover/measurements/plan2/run_check.py",
]:
    ast.parse(Path(p).read_text())
PY
```

Do not use `py_compile` (it writes `__pycache__`). Confirm every
`sample_manifest.json` and `results.json` parses with the standard library.

## Risks and mitigations

- **Oracle build/run fails.** Record `INCONCLUSIVE` and stop; do not
  substitute an untrusted checker.
- **Convention mismatch.** Provari rejects any repeated place; our validator
  only rejects a repeated full key (clock included). A single
  same-board/different-clock repeat can therefore be `valid: false` in
  provari while our tree is valid. Triage as `convention-repeat`; do not
  weaken our validator on this evidence alone.
- **No tablebases.** Our trees are fully expanded to rule terminals, so
  provari should not need tables. If it reports a missing tablebase verdict,
  inspect for a non-terminal leaf (a real proof-tree defect) and classify
  `unchecked` only if the limit is genuine.
- **FEN normalization.** Provari follows Lichess' “drop impossible
  castling/ep, allow extra material” parsing. Our generated FENs should be
  legal; a parser disagreement is a `suspect` worth minimizing.
- **Duplicate attacker positions.** The JSONL checker keys moves by
  `(place, halfmove)`; if the same position appears twice with different
  winning moves, the last line wins. Both are valid winning moves with their
  own emitted subtrees, so the proof remains checkable; record the occurrence
  if it shows up in triage.
- **Historic sample.** The shards are existing artifacts, not a fresh solve.
  The report must say what was validated and not generalize beyond the
  sample.
- **Large trees/timeouts.** The largest shard is ~4,762 nodes; `--seconds 60`
  is generous. A timeout is `unchecked`, not a defect.
- **Shard-set drift.** The driver records a SHA-256 manifest; if the shard
  set changes between drafting and execution, the manifest is the record.
  Do not silently re-pick cases.

## Report requirements

`report2.md` is the final task. It must include:

- the provenance table (date, host, provari commit/version, solver commit,
  shard dir, sample count, budgets);
- the exact commands used;
- the exporter path and unit-test result;
- the sample manifest and counts (scanned / not a tree / root-`Loss` skipped /
  root-`Draw` skipped / root-`Win` exported / export errors);
- the verdict table and per-class counts; all `suspect`/`export-bug`
  evidence, minimized where possible;
- the CLEAN/FIND verdict;
- gate results;
- the recommended next plan (expected: item 3, the win-only front-end
  skeleton, with the item-4 sibling-simulation census; if FIND, a
  defect-fix plan in the owning initiative first);
- explicit "not measured" items (root-`Loss`, compact `.pvr`, tablebases,
  NNUE, fresh solves, full `high200`, ledger row).

## Deliverables

- `src/proof_tree/export.rs` (new module, unit tests).
- `src/proof_tree/mod.rs` (`pub mod export;`).
- `examples/provari_export.rs` (new example).
- `AGENTS.md` (module description + Examples list).
- `docs/plans/winprover/measurements/plan2/README.md` (provenance).
- `docs/plans/winprover/measurements/plan2/run_check.py` (driver).
- `docs/plans/winprover/measurements/plan2/sample_manifest.json`.
- `docs/plans/winprover/measurements/plan2/results.json`.
- `docs/plans/winprover/measurements/plan2/results.csv`.
- `docs/plans/winprover/report2.md`.
- `docs/plans/winprover/initiative.md` and `docs/plans/README.md` status
  updates.
