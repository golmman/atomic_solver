# Plan 2: `corpus_gen` example (Gate 1 corpus generation)

## Goal

Add `examples/corpus_gen.rs`, an example binary with two subcommands:

1. **`solve`** — solves the concept's Gate-1 suites (`quick` + `decisive`,
   i.e. `decisive` plus the `m23`..`m29` move-order cases) at fixed
   `--timeout/--epsilon/--tt-size` defaults and writes one `proof.bin` dump
   per case (via `ProofTreeWorkerHandle::dump_to_bin`) into `--dump-dir`,
   plus a JSON manifest recording per-case solve metadata.
2. **`load`** — replays every `.bin` (root FEN + move paths) using
   `ProofTree::from_bin`, materializes one row per expanded, non-leaf tree
   node with
   `{hash, source, fen, stm, outcome, depth, subtree_size, legal_moves,
   static_scores, children, first_decisive_rank, partial}`,
   deduplicates rows by Zobrist hash across the whole corpus, and
   serializes them as NDJSON for the external (Gate 2) trainer. The
   move-order suite is *not* part of the corpus: it is held out for
   evaluation (concept.md Gate 4).

This is Gate 1 of `docs/plans/nn/concept.md` and follows the Gate-0 "go"
recommendation in `docs/plans/nn/report1.md` (recoverable work-weighted
share 57–69% at OR nodes).

## Background

- **Data source.** The finalized in-memory proof tree is available after
  `ProofTreeWorker::finalize()`; `dump_to_bin` writes the compact adjacency
  `.bin` format specified in `docs/spec/proof_tree_dump.md`, and
  `ProofTree::from_bin` reloads it.
- **`.bin` semantics.** The dump stores only `parent_id` + 16-bit `move_code`
  records; per-node `outcome` and `depth` are derived on load (outcome by
  parity from `root_outcome`, depth by post-order). The loader in
  `src/proof_tree/binary.rs` returns `hash: 0` for every node, so the corpus
  loader must recompute hashes by replaying moves and calling
  `Position::hash()`.
- **Label semantics** (concept.md §5): OR (Win) nodes → the proven
  decisive child must rank above every other legal move
  (`first_decisive_rank`); Loss (AND) nodes → rank children by derived
  post-order `subtree_size`. Legal moves that were never expanded are
  censored and must never be treated as "cheap".
- **Precedent**: `examples/move_order_fractions.rs` already implements the
  static-rank computation, the DFS replay with a single mutable `Position`,
  the synthesize-a-Loss-root workaround for Draw roots, and the
  `timeout=yes` partial-tree marking; the corpus loader reuses the same
  patterns.
- The `quick` suite is `decisive` plus move-order cases `m23`+ (see
  `examples/benchmark.rs`, `quick_suite()`). The concept's "quick +
  decisive" therefore equals `quick` without re-solving `decisive`
  separately.
- `serde_json` is a dev-dependency; examples can use it.

## Decisions (pinned here, used verbatim by Gate 2)

- **Toolchain split**: Rust emits the corpus; an external trainer consumes
  it. The `.bin` dumps are the durable raw artifact; NDJSON is derived, so a
  changed schema only requires re-running `load` (no re-solving).
- **Suite defaults**: `--suite quick` (default), also
  `--suite decisive`; the corpus's `quick` is the union of concept's
  "quick + decisive". The move-order suite is held out.
- **Fixed settings**: the corpus is only meaningful with deterministic
  settings; defaults are `--timeout 10 --epsilon 0.125 --tt-size 64
  --pt-size 256` and every case in one `solve` run uses the same values.
  The `load` step needs no solver settings.
- **Row schema** (one JSON object per line; see Design §4 for the exact
  field table): `{hash, source, fen, stm, outcome, depth, subtree_size,
  legal_moves, static_scores, children, first_decisive_rank, partial}`.
- **Per-node static scores** are included so Gate 2 can compute the static
  baseline rank offline (e.g. residual training) without reimplementing
  movegen in the trainer; `first_decisive_rank` is the summary label the
  concept lists.
- **Deduplication**: key = `Position::hash()` (u64), keep the occurrence
  with the largest `subtree_size` (ties: first seen; processing order =
  sorted case names, deterministic). This merges positions that appear in
  more than one tree (transpositions across cases get one row).
- **Leaf (terminal, depth == 0) nodes are skipped by default**: they carry
  no ranking labels and inflate the corpus; `--include-leaves` re-enables
  them for trainer experiments.
- **Partial trees are kept but flagged**: a row is `"partial": true` when
  its case timed out or its root outcome was synthesized; the trainer may
  filter them.

## Scope

In scope:

- `examples/corpus_gen.rs` (solve + load subcommands, CLI, manifest,
  analysis, NDJSON writer).
- `tests/test_corpus_gen.rs` (integration test exercising the round trip on
  a tiny FEN, plus CLI error handling).
- `AGENTS.md` example-list entry.
- `docs/plans/nn/report2.md` (final report; the plan's final task).

Out of scope:

- Any `src/`, `Cargo.toml`, or fixture change.
- The `.bin` format (v1 stays).
- The external trainer (Gate 2) and inference (Gate 3).
- Real per-child `child_evals` work counters (subtree-size proxy per
  concept.md; optional ablation is separate).
- AND-node first_decisive_rank (concept.md §5 defines it at OR nodes only;
  AND ranking uses derived `children[].subtree_size`).

## Design

### 1. CLI

```
corpus_gen <SUBCOMMAND> [OPTIONS]

solve:
  --fen <FEN>          Solve a single position; case name "fen"
  --suite <NAME>       quick | decisive          (default: quick)
  --timeout <S>        Search budget in seconds   (default: 10)
  --epsilon <F>        DF-PN+ threshold           (default: 0.125)
  --tt-size <MB>       TT size                    (default: 64)
  --pt-size <MB>       Proof-tree memory budget   (default: 256)
  --dump-dir <DIR>     Output dir for .bin dumps + manifest
                       (default: data/trees; created if missing)
  -h, --help

load:
  --dump-dir <DIR>     Dir with *.bin (default: data/trees; reads manifest
                       if present; warns if not)
  --output <FILE>      NDJSON output file (default: stdout)
  --include-leaves     Emit depth-0 rows too
  -h, --help
```

- Unknown options/subcommands exit with an error; `--help` exits 0; no
  positional args except the subcommand. Same conventions as
  `move_order_fractions`.
- `--suite` in `solve` maps `quick` to `load_decisive_suite()`
  (`examples/common.rs`) + move-order cases with `m`-prefix number ≥ 23
  (duplicated from `benchmark.rs::quick_suite`, without refactoring
  benchmark), and `decisive` to `load_decisive_suite()` only.
- `--fen` bypasses suites; case name `"fen"`.

### 2. Solve subcommand

Per case, mirror `move_order_fractions::solve_and_measure`:

1. `Position::from_fen(fen)`; `Search::new(tt_size)`,
   `set_timeout`, `set_epsilon`; **no** `set_first_outcome_only` (the
   default refined search grows the proven tree, which is what the corpus
   wants).
2. `ProofTreeWorker::spawn(fen, pt_size, Arc::new(AtomicBool::new(false)))`;
   `search.set_proof_event_sender(handle.event_sender())`.
3. `search.solve_with_progress(&mut pos, |o, line| ...)` (progress to
   stderr).
4. If `outcome == Draw` (or timeout with unproven root): synthesize a Loss
   root via `ProofEvent::NodeProven(NodeProven::new(Vec::new(),
   pos.hash(), Outcome::Loss, 0))` — same workaround as
   `move_order_fractions` — and record `synthesized_root = true`.
5. `handle.finalize(); handle.dump_to_bin(<dump-dir>/<case>.bin)`.
   `drop(handle); join.join()`.

Record `{case, fen, outcome, timeout, mem_limited, synthesized_root,
tree_nodes, root_depth}` per case; after all cases write
`<dump-dir>/manifest.json`:

```json
{"cases": [
  {"name":"dec01","fen":"...","outcome":"win","timeout":false,
   "mem_limited":false,"synthesized_root":false,"tree_nodes":1234,"root_depth":33}
]}
```

(If a case's tree produces 0 nodes — e.g. an instant draw before any proof
event — skip it with a warning and record `"bin": null` in the manifest.)

### 3. Load submodule

1. Read all `*.bin` in `--dump-dir` (filename-sorted); skip files without a
   corresponding manifest entry when the manifest exists (warn).
2. For each: `ProofTree::from_bin(&mut reader)`, which yields per-node
   `outcome` (parity-derived), `depth` (post-order-derived), and `mv`
   reconstructed. Nodes' `hash` is 0 — recomputed below.
3. `subtree_sizes(tree)`: post-order `Vec<u64>` over `tree.children(id)`,
   identical to `move_order_fractions::subtree_sizes`.
4. DFS replay, one mutable `Position` from `tree.root_fen`:
   `pos.do_move(child.mv)` on descent, `pos.undo_move` on ascent
   (assert-free; if `do_move` panics, report the case and skip it).
   At each node:
   - skip `outcome == None` (defensive. never seen post-finalize);
   - skip depth-0 nodes unless `--include-leaves`;
   - `let mut moves = MoveList::new(); pos.legal_moves(&mut moves);`
     (movegen order — deterministic, and is the corpus's `legal_moves`);
   - `let mut state = StateInfo::new(); pos.populate_state(&mut state);`
     `let nearest = nearest_commoner_map(pos.board(), them);`
     score every legal move with
     `scorer.score_with_map(pos.board(), m, &state, &nearest, is_or_node = (node.outcome == Win))`
     (default `StaticAtomicScorer`), stable-sort descending, and record per
     move: static score + rank; `first_decisive_rank` = min rank over
     children with `outcome == Loss`; `win_rows` only.
   - `children`: for each tree child in stored order,  `{mv: uci, outcome,
     subtree_size}` — the subtree sizes for AND-node ranking labels;
   - `hash = pos.hash()`, `fen = pos.fen()`, `stm = "w"|"b"`,
     `depth = tree.nodes[id].depth`,
     `subtree_size = sizes[id]`, `partial = manifest[case].timeout ||
     manifest[case].synthesized_root`.
5. Dedup per Decisions: a `HashMap<u64, Row>` keyed by hash keeps the
   largest-`subtree_size` occurrence; output follows insertion order (case
   order, then node id order), so provenance stays visible and output is
   deterministic for a fixed input directory.
6. Emit NDJSON: a meta first line, then one row per line to `--output` or
   stdout (compact; never pretty).

### 4. NDJSON schema (v1)

First line (meta; single JSON object with reserved key `_meta`):

```json
{"_meta":"atomic-corpus/1","suite":"quick","timeout":10,"epsilon":0.125,
 "tt_size":64,"pt_size":256,"cases":30,"bins":30,"rows":150000,
 "partial_rows":1200}
```

Every other line is one row:

| Key | Type | Always? | Meaning |
|---|---|---|---|
| `hash` | u64 (JSON number) | yes | `Position::hash()` (includes fullmove clock); dedup key |
| `source` | string | yes | case name (bin file stem) |
| `fen` | string | yes | board position as FEN (for trainer feature extraction) |
| `stm` | string | yes | `"w"`/`"b"` |
| `outcome` | string | yes | `"win"`/`"loss"` (never `"draw"` in a realized tree) |
| `depth` | u32 | yes | proven distance to a terminal (binary-derived post-order) |
| `subtree_size` | u64 | yes | post-order node count of this node's subtree (proxy for search effort) |
| `legal_moves` | [string] | yes | UCI strings, movegen order |
| `static_scores` | {string:number} | yes | `StaticAtomicScorer::default()` score per legal move (key: UCI); OR-view if `outcome == "win"` else AND-view (`is_or_node` flag) |
| `children` | [{`mv`, `subtree_size`, `outcome`}] | yes | proven children in tree order; the AND-node label source |
| `first_decisive_rank` | int | only on `win` rows | min static rank of a child with `outcome == loss` (OR label) |
| `partial` | bool | yes | true if the case timed out or its root was synthesized |

`children.outcome` mirrors `outcome` parity (a `win` node's children are
all `loss`). `hash` serializes as a decimal JSON number; tools must parse
integers exactly (Python int / numpy uint64; JS needs BigInt).

### 5. Output and stats

- stdout / `--output` contains the NDJSON only (no tables, no
  progress). All progress, per-case lines, and the final summary
  (`bins`, `rows`, `or_rows`, `and_rows`, `dedup_dropped`,
  `partial_rows`) go to stderr.
- The examples' convention of tables to stdout is not violated here:
  NDJSON is the tool's output contract.

### 6. Integration test

`tests/test_corpus_gen.rs` (RUN_LOCK-serialized, release build, mirroring
`tests/test_move_order_fractions.rs`):

- `corpus_gen solve --fen "…" --timeout 2 --dump-dir <temp>` (tiny decisive
  FEN, e.g. `4k3/8/8/8/8/8/8/4R1K1 w - - 0 1` and `--tt-size 16
  --pt-size 16` to keep it fast); assert exit 0, dump dir contains
  `fen.bin` and `manifest.json`.
- `corpus_gen load --dump-dir <temp> --output <temp/out.ndjson>`; assert
  exit 0, the file has ≥ 1 line, meta line parses, every line parses as
  JSON, `hash` values are unique, every row carries the required keys
  (`fen`, `outcome`, `legal_moves`, `subtree_size`, `children`).
- Unknown option and unknown subcommand exit 1; `-h` exits 0.

Put the temp dir under `std::env::temp_dir()` with a per-test unique suffix
and remove it in the test.

## Implementation steps

1. Add `examples/corpus_gen.rs`:
   - CLI parsing (subcommand dispatch, option tables, help).
   - `solve`: suite-loading (`load_decisive_suite`, move-order `m≥23`
     filter), sequential solve loop mirroring
     `move_order_fractions`, dump + manifest writer.
   - `load`: bin discovery + manifest reading, `ProofTree::from_bin`,
     subtree sizes, DFS replay + rank/score pass (adapted from
     `move_order_fractions::rank_samples`, extended with `children` and
     per-move `static_score` arrays), leaf skip, hash dedup, NDJSON emitter.
2. Add `tests/test_corpus_gen.rs`.
3. Add the example to the `examples/` list in `AGENTS.md`.
4. `cargo fmt`, `cargo clippy --all-targets`, `cargo test`.
5. Manual verification (below), including a transposition-heavy case.
6. Write `docs/plans/nn/report2.md` (final task).

## Files changed

- `examples/corpus_gen.rs` (new)
- `tests/test_corpus_gen.rs` (new)
- `AGENTS.md` (examples list)
- `docs/plans/nn/report2.md` (new, final report; written by the final task)

No changes to `src/`, `Cargo.toml`, or any fixture. `serde_json` (already a
dev-dependency) is used for NDJSON and the manifest.

## Verification

```bash
cargo fmt --check
cargo clippy --all-targets
cargo test

# Tiny end-to-end (solve + load round trip).
cargo run --release --example corpus_gen -- solve --fen \
    "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1" --timeout 2 --dump-dir /tmp/pt1
cargo run --release --example corpus_gen -- load --dump-dir /tmp/pt1 \
    | python3 -c "import sys,json; rows=[json.loads(l) for l in sys.stdin if l.strip()]; \
    assert rows[0]['_meta']; assert len({r['hash'] for r in rows[1:]})==len(rows)-1; \
    print('ok', len(rows)-1, 'rows')"

# Corpus-suite pass (the real deliverable; ~30 cases × up to 10 s each).
cargo run --release --example corpus_gen -- solve --suite quick --timeout 10 \
    --dump-dir data/corpus/trees
cargo run --release --example corpus_gen -- load --dump-dir data/corpus/trees \
    --output data/corpus/train.ndjson
```

Sanity on the corpus:

- Meta + all rows parse as JSON; `hash` values unique across the file.
- `len(legal_moves) == len(static_scores)`; every `children[].mv` is in
  `legal_moves`.
- `partial_rows` reported is small relative to rows (target < ~10%);
  `rows` nonzero; dedup-dropped count reported (nonzero after transposition
  cases).
- Execute at least one run with `--suite decisive` to exercise the
  `dec10`-style copied-subtree replay invariant (validated in report1; the
  loader reuses the same replay).

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Replay `do_move` assert on a copied transposition subtree | Hash-consistent copies (report1 verified invariant). Defensive: catch/panic_report, skip the case, warn. |
| Huge corpus rows (children arrays × several positions) blow memory | Rows are emitted one at a time to the writer; the dedup map holds only the compact row structs, not the serialized strings. |
| Hash collisions across trees merge unrelated rows | 64-bit Zobrist; document the dedup rule (max subtree) rather than silently wrong data; the trainer can split by `source`. |
| `legal_moves`/`static_scores` order mismatch (schema drift at Gate 2) | Both arrays are index-aligned by `legal_moves` order and the schema table in this file pins it; Gate 2 must not assume JSON key order. |
| Timeout cases dominate with unrepresentative early-tree rows | `partial: true` flag per row; stats expose `partial_rows`; position-timeout-specific corpus can be regenerated with higher `--timeout`. |
| `subtree_size` is a proxy, not real work | Documented limitation from concept/report1; optional `child_evals` counters are a later ablation. |
| Double solving `decisive` + `quick` | `quick` already contains `decisive`; suite option documents the mapping; no double runs. |

## Success criteria

1. `examples/corpus_gen.rs` builds; both subcommands run the verification
   commands and produce parseable, self-consistent NDJSON.
2. `cargo test` passes including `tests/test_corpus_gen.rs`.
3. A real corpus is produced from `--suite quick --timeout 10` and its
   NDJSON satisfies the sanity checks (schema, dedup, alignment).
4. `docs/plans/nn/report2.md` exists and documents: the schema (field
   table), measured row counts per suite (total rows, unique hashes,
   `partial_rows`, dedup-dropped), the chosen dedup rule, any deviations,
   and the exact file-format contract that Gate 2 must consume.

## Final task

Write `docs/plans/nn/report2.md` covering:

- the example's CLI and the NDJSON schema exactly as implemented,
- measured corpus numbers: cases, bins, total nodes, unique rows, OR vs
  Loss rows, partial rows, dedup drops, wall time for solve + load,
- differences from this plan ("diff vs plan"),
- the static-vs-runtime-ordering and subtree-size-proxy limitations,
- the synthesized-root data-consistency note,
- the concrete input contract for the Gate 2 trainer (the
  schema table + the `hash`-as-JSON-number caveat).

End the report with the next step (Gate 2 / training plan contract review
in `docs/spec/nn.md` §8; weight-file layout agreement required before any
Rust weight-loading code).