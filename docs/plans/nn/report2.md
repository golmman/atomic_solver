# Report 2: `corpus_gen` example (Gate 1 corpus generation)

## Summary

Gate 1 of `docs/plans/nn/concept.md` is implemented and verified:
`examples/corpus_gen.rs` (plan: `docs/plans/nn/plan2.md`) with `solve` and
`load` subcommands, `tests/test_corpus_gen.rs`, and the `AGENTS.md` examples
entry. A real corpus was produced from `--suite quick --timeout 10`
(`data/corpus/trees` + `data/corpus/train.ndjson`), and its NDJSON satisfies
all sanity checks: schema, hash dedup, `legal_moves`/`static_scores`
alignment, parity, and subtree-size consistency.

## CLI as implemented

```
corpus_gen solve [OPTIONS]
  --fen <FEN>       single position; case name "fen"
  --suite <NAME>    quick | decisive        (default: quick)
  --timeout <S>     10 default
  --epsilon <F>     0.125 default
  --tt-size <MB>    64 default
  --pt-size <MB>    256 default
  --dump-dir <DIR>  data/trees default; created if missing

corpus_gen load [OPTIONS]
  --dump-dir <DIR>  data/trees default; reads manifest.json if present
  --output <FILE>   NDJSON file (default stdout)
  --include-leaves  emit depth-0 rows too

  -h, --help        exit 0; unknown subcommand/option exit 1
```

`--suite quick` maps to the benchmark `quick_suite`: the `decisive` fixture
(dec01..dec23, 23 cases) plus move-order cases with number ≥ 23
(m23_white..m29_white, 13 cases) = 36 cases. `--fen` bypasses suites.

`solve` per case: `Position::from_fen` → `Search::new(tt_size)` with
`set_timeout`/`set_epsilon` (no `--first-outcome`: the refined search grows
the tree, which is the corpus's goal) → `ProofTreeWorkerHandle::spawn(fen,
pt_size, Arc::new(AtomicBool::new(false)))` + `search.set_proof_event_sender`
→ `solve_with_progress` (progress to stderr) → on `Draw` outcome (timeout
with unproven root) synthesize a Loss root via a `NodeProven` event →
`handle.finalize()` → `handle.stats()` → `handle.dump_to_bin(<dir>/<case>.bin)`
→ drop handle, join. A case whose tree has 0 nodes is skipped with a warning
and recorded `"bin": null`. After all cases, `<dump-dir>/manifest.json` is
written.

`load`: reads `*.bin` (filename-sorted; bins without a manifest entry are
warned about and skipped when the manifest exists), `ProofTree::from_bin` per
bin, post-order `subtree_sizes`, DFS replay with one mutable `Position`
(assert-free; a replay crash aborts the process — see "diff vs plan" item on
`panic = "abort"`), then per node:

- skip depth-0 rows unless `--include-leaves`; skip unrealized nodes
  (defensive; none occur post-finalize);
- `legal_moves` in movegen order; `static_scores` per move under
  `StaticAtomicScorer::default()` with `is_or_node = (outcome == Win)`
  (`score_with_map`), stable-sorted descending for ranks;
- `first_decisive_rank` = min static rank over children with `outcome ==
  loss` (OR rows only; absent on AND rows);
- `children` = `[{mv, outcome, subtree_size}]` in stored tree order;
- `hash = pos.hash()`, `fen = pos.fen()`, `stm`, `depth = node.depth`,
  `subtree_size = sizes[id]`,
  `partial = manifest[case].timeout || manifest[case].synthesized_root`.

Deduplication key is `Position::hash()` (u64 Zobrist incl. halfmove clock).
Keep the occurrence with the largest `subtree_size`; ties keep the first
seen. Output follows insertion order (case order as `*.bin` filename-sorted,
then node id order) so provenance stays visible and the output is
deterministic for a fixed input directory. NDJSON is emitted compactly
(`serde_json::to_string`), one object per line; the first line is the `_meta`
object. The NDJSON itself is the only stdout/`--output` content; progress and
the summary go to stderr.

## NDJSON schema (v1) as implemented

First line:

```json
{"_meta":"atomic-corpus/1","suite":"quick","timeout":10,"epsilon":0.125,
 "tt_size":64,"pt_size":256,"cases":36,"bins":36,"rows":20564,
 "partial_rows":16092}
```

| Key | Type | Always? | Meaning |
|---|---|---|---|
| `hash` | u64 (JSON number) | yes | `Position::hash()` (includes fullmove clock); dedup key |
| `source` | string | yes | case name (bin file stem) |
| `fen` | string | yes | position FEN (`c`/`C` kings before atomic-movegen 2.1.0, standard `k`/`K` since; see diff item) |
| `stm` | string | yes | `"w"`/`"b"` |
| `outcome` | string | yes | `"win"`/`"loss"` |
| `depth` | u32 | yes | proven distance to a terminal (binary-derived post-order) |
| `subtree_size` | u64 | yes | post-order node count of the node's subtree |
| `legal_moves` | [string] | yes | UCI strings, movegen order |
| `static_scores` | {string:number} | yes | `StaticAtomicScorer::default()` per legal move (key: UCI); OR-view if `outcome == "win"` else AND-view |
| `children` | [{`mv`,`subtree_size`,`outcome`}] | yes | proven children in tree order |
| `first_decisive_rank` | int | `win` rows only | min static rank of a `loss` child (OR label) |
| `partial` | bool | yes | case timed out or root was synthesized |

## Measured corpus numbers

Solve (`--suite quick --timeout 10 --epsilon 0.125 --tt-size 64 --pt-size
256`, release, 16-core container):

| Metric | quick (36 cases) | decisive (23 cases) |
|---|---|---|
| cases | 36 | 23 |
| bins written | 36 (0 skipped) | 23 (0 skipped) |
| total tree nodes (manifest sum) | 215,414 | 186,764 |
| cases reaching the timeout | 20 | 17 |
| wall time | 3 m 30 s | ~2 m 20 s |

Load (same box):

| Metric | quick | decisive |
|---|---|---|
| raw rows | 163,916 | 142,635 |
| unique rows emitted | 20,564 | 15,602 |
| OR (win) rows | 14,355 | 11,153 |
| AND (loss) rows | 6,209 | 4,449 |
| dedup drops | 143,352 | 127,033 |
| partial rows | 16,092 | 13,685 |
| load wall time | 0.65 s | 0.6 s |

The quick corpus (`data/corpus/train.ndjson`, 20,564 rows) passes every
sanity check: all lines parse as JSON; `hash` values unique; meta `rows` and
`partial_rows` agree with the data; `len(legal_moves) == len(static_scores)`;
`children[].mv` always in `legal_moves`; win rows carry
`first_decisive_rank` and only loss children, loss rows never carry the key
and only win children; `subtree_size == 1 + sum(children.subtree_size)`.
Rows come from 31 of the 36 cases: the other 5 cases' rows were absorbed by
cross-case transpositions (all their hashes dedup onto larger-subtree rows
from other cases), which is expected dedup behavior, not data loss.

Measured static rank-1 share on the quick corpus is 68.1% of OR rows
(median 1), consistent with the Gate-0 range (57–69%) from
`docs/plans/nn/report1.md`.

## Diff vs plan

- **Manifest carries the solve settings.** The plan's manifest snippet
  showed only `{"cases": [...]}`; the implemented manifest adds top-level
  `suite/timeout/epsilon/tt_size/pt_size`, which `load` needs for the NDJSON
  meta line. Per-case `fen`, `outcome`, `timeout`, `mem_limited`,
  `synthesized_root`, `tree_nodes`, `root_depth`, `bin` are as planned.
- **Meta line without a manifest.** When `--dump-dir` has no
  `manifest.json`, `load` warns and processes all bins with `"suite":
  "unknown"`, settings `0`, and `partial: false` for every row.
- **`fen` uses the movegen library normalization.** `Position::fen()` (via
  `Board::fen()`) writes kings as commoners (`c`/`C`), e.g.
  `4k3/.../4R1K1` becomes `4c3/.../4R1C1`. It is the round-trip FEN of the
  engine feature source; a trainer must parse this same convention (or
  regenerate features from the move list). *(Superseded: atomic-movegen
  2.1.0 standardizes on `k`/`K` notation only and rejects `c`/`C` on input;
  the regenerated `atomic-corpus/2` uses standard FENs.)*
- **Replay-crash mitigation is exit, not catch.** The release profile sets
  `panic = "abort"`, so the plan's "catch the `do_move` panic, skip the
  case" mitigation is not available in the profile the corpus runs under. A
  malformed dump aborts the process loudly. The risk materialized during
  verification (see below) and was **fixed at the source**; every bin in the
  produced corpora replays cleanly.
- **Bug found and fixed (leaf-skip desync).** The first implementation
  returned prematurely (`continue`) at depth-0 `Enter`s, leaving the
  pending `Descend` `do_move` un-undone, so the same position accumulated
  phantom moves and eventually `do_move` hit an empty from-square (panic in
  atomic-movegen `board.rs:802`). The DFS traversal is now independent of
  row emission: every `Enter` pushes its `Exit` and children regardless of
  skips.
- **`quick` = 36 cases**, not 30 as in the plan's example meta.
- **Empty-tree cases** (`stats.nodes == 0`): skipped with a warning,
  `"bin": null`; never observed in practice (finalize always keeps the
  realized root).
- `--include-leaves` verified: emits depth-0 rows with `depth: 0` (no
  `first_decisive_rank` on win leaves, no children).

## Limitations (as in concept.md §5 and report1)

- **Static rank is not runtime order.** history/killer/TT promotions are
  per-run state and are absent from the corpus by design; Gate 4 must
  compare against the full runtime ordering.
- **`subtree_size` is a proxy for effort.** The solver has no per-child
  `child_evals` counters; subtree size is the planned proxy, and per-child
  work counters remain an optional ablation.
- **The echo chamber argument applies:** rows come from trees the current
  ordering can build; positions it never expands never appear. The move-order
  suite (m20..m29) is held out for evaluation.
- **Synthesized-root data-consistency note:** when a case times out with no
  root proof (or is an immediate draw), the corpus loader synthesizes a Loss
  root event so `finalize()` keeps the realized (refuted-line) children,
  then `dump_to_bin` writes `root_outcome = Loss` even though the manifest
  records the search outcome (`"draw"`). The tree below such a root is the
  search's refutation material, and its parity derives from the synthetic
  root; labels there stem from a partial proof. Every such row carries
  `"partial": true` and should be filtered by the trainer for trusted-label
  work.

## Input contract for the Gate 2 trainer

> Schema drift note (design B, `docs/plans/nn/report4.md`; atomic-movegen
> 2.1.0): the shipped corpus is `atomic-corpus/2` — every `children[]` entry
> additionally carries `work` (the cumulative `child_evals` spent proving that
> child's subtree), the AND label is "rank the children by `work`" instead of
> by `subtree_size`, and (since atomic-movegen 2.1.0) `fen` uses standard
> `k`/`K` notation instead of the `c`/`C` commoner spelling. The parsing
> caveats below (NDJSON structure, `hash` precision, key order, alignment)
> still apply verbatim; the AND-label bullet below reflects the v2 `work`
> target.

- One JSON object per text line after a leading `_meta` line (NDJSON, never
  pretty). The file contains `hash` as a decimal u64 JSON number; decode
  with exact integer semantics (Python `int`/`numpy.uint64`; JavaScript
  requires `BigInt`). Do not assume JSON key order inside `static_scores`
  (serde_json's default `BTreeMap` sorts object keys). Use
  `legal_moves[i]` ⇄ `static_scores[legal_moves[i]]` ⇄ the static rank
  derived by sorting the score map keys descending.
- `children[]` is ordered as stored, and `work` there is the AND-node ranking
  label: for `outcome == "loss"` rows, sort the children by `work` descending
  (ties: any stable order) and rank that ordering against
  `static_scores`-derived order to produce AND labels offline; `win` rows
  carry `first_decisive_rank` directly.
- `hash` is the dedup key; `source` keeps provenance; `partial` must filter
  rows from partially-solved cases when only fully-proven subtrees should
  train.
- Features: `fen` + `stm` pin the position; `legal_moves` is the mask; the
  trainer is expected to generate features from `fen` (or the move list) —
  no extra columns are added.

Example row (subject to the meta line above):

```json
{"children":[{"mv":"e1e8","outcome":"loss","subtree_size":1}],"depth":1,
 "fen":"4k3/8/8/8/8/8/8/4R1K1 w - - 0 1","first_decisive_rank":1,
 "hash":13158622503721752706,"legal_moves":["e1a1","e1b1",...],
 "outcome":"win","partial":false,"source":"fen",
 "static_scores":{"e1a1":0,"e1b1":560,...},"stm":"w","subtree_size":2}
```

## Verification (all `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test`)

- `cargo test` — all targets pass, including
  `tests/test_corpus_gen.rs` (4 tests: solve+load round trip on the tiny FEN,
  unknown subcommand exit 1, unknown option exit 1, `-h` exit 0).
- Tiny end-to-end: `solve --fen "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1"
  --timeout 2 --dump-dir /tmp/pt1`, then `load`; NDJSON has meta + rows,
  hashes unique, required keys present, meta `rows` count matches.
- Real deliverable: `solve --suite quick --timeout 10 --dump-dir
  data/corpus/trees`, then `load ... --output data/corpus/train.ndjson`; the
  NDJSON passes the full schema/dedup/alignment sanity script (see the
  measured numbers above). A separate `--suite decisive` run exercised the
  dec10-style copied-subtree replay invariant.
- Transposition-heavy case: dec10-like FEN solved with `--timeout 2`
  (209,256-node tree): loaded in ~0.7 s into 4,626 unique rows,
  `dedup_dropped = 165,172`.

## Next step

Gate 2 / training contract review in `docs/spec/nn.md` §8 (`policy_size`
scheme, weight-file layout) and the training run itself. An explicit
weight-file header + layout agreement is required before any Rust
weight-loading code in a Gate 3 plan.