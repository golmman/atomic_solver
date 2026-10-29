# Interop Plan 1: Differential proof checking against provari

Initiative: `interop`, backlog **item 1**. Drafted 2026-10-29; this is a
measurement/validation plan, not a search-semantics or proof-format change.

Prerequisite reading: [`initiative.md`](initiative.md), `AGENTS.md`,
[`docs/spec/proof_tree_dump.md`](../../spec/proof_tree_dump.md), and
`src/proof_tree/{validate.rs,binary.rs,node.rs}`.

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

The plan is deliberately narrow: **root-`Win` trees only** (JSONL assumes the
attacker is the side to move), one external checker, a reproducible sample,
and triage. Root-`Loss` coverage and the compact `.pvr` format are backlog
item 2.

## External oracle: pin and isolate

- Repository: `https://github.com/aiorla/provari`
- Commit/tag at drafting: `48686bc6c786d6ea70568f0b2ad4002cf0520206`
  (`v0.1.0`)
- License: GPL-3.0-or-later. **Do not copy code and do not add it to
  `Cargo.toml`.** Clone/build in `/tmp` (or an ignored tools directory); only
  the driver and parsed results are committed.
- Build without bundled network/model:

  ```sh
  git clone https://github.com/aiorla/provari /tmp/provari-oracle
  git -C /tmp/provari-oracle checkout 48686bc6c786d6ea70568f0b2ad4002cf0520206
  cargo build --release --manifest-path /tmp/provari-oracle/Cargo.toml
  /tmp/provari-oracle/target/release/provari --version
  ```

  The checker does not need the NNUE network or model. If the build is
  impossible in the container (network/toolchain/glibc), record the blocker
  and stop as `INCONCLUSIVE`; do not substitute an untrusted checker.

Plan-session probe (2026-10-29): the pinned source built in `/tmp/provari`
with `cargo build --release` in ~25 s and reports `provari 0.1.0`. A
hand-written JSONL proof for the mate-in-1 fixture
`k7/8/1Q6/8/8/8/8/7K w - - 0 1` / `b6b7` was accepted
(`valid:true`, `won_positions:1`, exit 0), confirming the reader path and the
root-`Win` attacker assumption. That probe did not exercise the exporter, and
is recorded in
[`measurements/plan1/plan_session/README.md`](measurements/plan1/plan_session/README.md).

## JSONL contract (provari's reader, verified from `src/check.rs`)

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
position being proved; that clock is part of the checker's move key.

## Exporter deliverable

Add a small, tested proof-tree export module and a thin example:

- `src/proof_tree/export.rs`
  - `pub fn write_provari_jsonl<W: Write>(tree: &ProofTree, writer: &mut W)
    -> Result<usize, ExportError>` (or an equivalently named API).
  - It first runs `validate_proof_tree(tree)`; an invalid tree is an
    `ExportError`, never a silently written export.
  - It rejects a root whose outcome is not `Win` (root-`Loss` is backlog #2).
  - It replays the tree from `tree.root_fen` with `Position`, applies/undoes
    moves, and walks:
    - `Win` node: emit `current_fen` + the single proving child's move, then
      descend into that child;
    - `Loss` node: descend into every child, emitting nothing;
    - terminal node or node with no child: stop.
  - Use an explicit DFS stack with `Enter`/`Exit` frames; proof trees can be
  deep and must not recurse.
  - The header is `{"root":"<tree.root_fen>"}`. Write JSON directly; FEN and
    UCI strings are JSON-safe by construction (assert no `"`/`\`). No new
    dependency is needed.
  - Return the number of emitted attacker lines, for reporting.
- `examples/provari_export.rs`
  - Usage: `provari_export [--out PATH] <proof_tree.bin>`.
  - Reads the tree with `ProofTree::from_bin`, calls the export function, and
    writes to stdout or `--out`.
  - Prints a one-line summary to stderr (`positions: N`, `nodes: M`,
    `root: win`).
- Register the module in `src/proof_tree/mod.rs` (public, so the example can
  use it); update the `src/proof_tree/` description and Examples list in
  `AGENTS.md` in the same change.

### Unit tests (in `export.rs`)

1. The existing hand-built mate-in-1 fixture
   (`k7/8/1Q6/8/8/8/8/7K w - - 0 1`, `b6b7`) produces the header plus one
   line; replaying the output independently yields the same move.
2. A root-`Loss` tree returns `ExportError::RootNotWin` and writes no header
   (check the writer buffer is empty).
3. An invalid tree returns `ExportError::Invalid` with the validator's
   defects (e.g. a missing reply under a `Loss` node).
4. A loaded-from-binary tree with `hash == 0` exports successfully (the
   validator skips hash checks; replay still works).

The tests stay in the default gate (`make test`); no slow tests are needed.

## Sample

Use the committed proof-tree shards as the first corpus:

```sh
ls docs/plans/proofdb/shards/*.bin
```

At drafting there are 263 files. The driver must:

1. read each file and require the `ATOMTREE` magic; skip/report anything
   else;
2. run the exporter (which validates first);
3. record root outcome, node count, emitted line count, and either an export
   error class or the JSONL path.

If a shard is root-`Loss`, record it as skipped by plan1 scope. If the sample
contains very large trees, apply provari's own `--seconds` limit per proof and
classify a timeout as `unchecked`, not as a defect. The sample list and its
SHA-256 manifest are committed under
`docs/plans/interop/measurements/plan1/`; the exported `.jsonl` files
themselves are regenerable and are not committed.

## Run and triage

For each exported file:

```sh
/tmp/provari-oracle/target/release/provari check proof.jsonl --seconds 60
```

Provari prints one JSON line per proof (`valid: true|false|null`) and exits 1
unless all are valid. Capture stdout/stderr in the measurement directory's
ignored logs, parse the verdicts into a committed JSON/CSV table, and classify:

| class | condition | action |
|---|---|---|
| `accepted` | `valid: true` | record; no action |
| `convention-repeat` | `valid: false`, error reports a repeatable position | inspect the tree's place-cycle; if real, record as a convention difference (provari's no-repeat rule is stronger than our clock-inclusive cycle check), not a defect |
| `unchecked` | `valid: null` (time/size/depth/tablebase limit) | record the limit; do not count as failure |
| `export-bug` | provari rejects a line/FEN/move that our exporter produced, or our validator rejects a tree the export path accepted | fix the exporter in this plan; re-run |
| `suspect` | otherwise (`valid: false`, no explainable convention/export cause) | stop and minimize; this is the plan's finding |

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
3. `report1.md` states **CLEAN** (no `suspect`/`export-bug`) or **FIND** (at
   least one), with the minimized evidence and a recommendation for backlog
   #2/#3 or a defect-fix plan.

There is no accept-rate threshold: one unexplained rejection is the signal.
`INCONCLUSIVE` is reserved for an oracle build/run blocker, not for a proof
that could not be checked within provari's limits (that is `unchecked`).

## Gate

- `cargo fmt`
- `cargo clippy --all-targets`
- `make test` (release, fast tier)
- `cargo test --release --example provari_export` if any example-local tests
  are added beyond the module tests

No search/TT/proof-construction code changes, so `make test-full` is not
required. If an unexpected product-code change becomes necessary, stop and
re-scope the plan first.

## Report task

Write `docs/plans/interop/report1.md` with:

- provari commit/tag, build command, `--version`, host/container notes;
- exporter commit path and unit-test result;
- sample manifest and counts (scanned / not a tree / invalid / root-`Loss`
  skipped / root-`Win` exported);
- verdict table and per-class counts; all `suspect`/`export-bug` evidence;
- gate results;
- next-step recommendation: proceed to #2 (root-`Loss`/compact), #3
  (shards), or a defect-fix plan in the owning initiative.

Then update `initiative.md`'s history/backlog and the `docs/plans/README.md`
row only if the initiative's status or next lever changed.

## Risks and caveats

- **Convention mismatch:** provari rejects any repeated position; our
  validator only rejects a repeated full key (clock included). A single
  same-board/different-clock repeat can therefore be `valid: false` in
  provari while our tree is valid. Triage as `convention-repeat`; do not
  weaken our validator on this evidence alone.
- **No tablebases:** our trees are fully expanded to rule terminals, so
  provari should not need tables. If it reports a missing tablebase verdict,
  inspect for a non-terminal leaf (a real proof-tree defect).
- **FEN normalization:** provari follows Lichess' “drop impossible
  castling/ep, allow extra material” parsing. Our generated FENs should be
  legal; a parser disagreement is a `suspect` worth minimizing.
- **Historic sample:** the shards are existing artifacts, not a fresh solve.
  The report must say what was validated and not generalize beyond the
  sample.