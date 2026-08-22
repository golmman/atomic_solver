# Report 4: real work counters in the proof tree (design B, Gate 1.5)

## Summary

Design B (`docs/plans/nn/plan4.md`) is implemented and verified: the solver
now records the **real per-child work** — the cumulative `child_evals` spent
proving each subtree — as a first-class datum of the proof pipeline.

- `NodeProven` gains `work: u64`, set by the search at every emit site;
- `ProofNode` gains `work: u64`; the worker max-updates it on duplicate events
  and copies the canonical twin's `work` onto unexpanded transpositions during
  `finalize()`;
- the `.bin` dump is now **v2** (`parent_id` + `move_code` + `work` = 14-byte
  records); v1 dumps still load, with `work == 0`;
- `corpus_gen` emits `children[].work` and the corpus version is
  **`atomic-corpus/2`**: the AND label is "rank the children by `work`";
- `work_proxy_ablation` re-measured with the recorded work as the
  authoritative ground truth; the TT probe (`Search::tt_work_for`) is now a
  cross-check with per-case coverage and `tt_agree`.

**Re-measured verdict: the `subtree_size` proxy is rejected, now on the
authoritative ground truth, and slightly more so than design A measured.**
With recorded work:

| | quick (36 cases) | decisive (23 cases) |
|---|---|---|
| AND nodes / complete | 22,940 / 22,940 (100%) | 18,808 / 18,808 (100%) |
| child pairs | 171,317 | 153,381 |
| **pair flip rate** | **33.1%** (A: 28.0%) | **33.8%** (A: 28.7%) |
| Kendall τ (pooled / mean) | 0.46 / 0.302 | 0.46 / 0.315 |
| top-child agreement | 79.6% (A: 83.3%) | 78.0% (A: 81.8%) |
| **work-weighted flip share** | **48.3%** (A: 47.7%) | **47.7%** (A: 45.0%) |
| TT coverage / `tt_agree` | 99.7% / 78.9% | 99.6% / 78.0% |

Design A's TT probe *understated* the label noise by ~5 points of pair flip
rate: the TT `work` is max-updated by unsolved intermediate chunk stores, so
it overestimates sibling work and masks part of the proxy's noise. The
recorded work is the exact prove-time delta. The numbers remain far above the
5–10% ceiling either way — rejecting the proxy was correct, and the corpus now
labels AND nodes with the real thing, making this the final word on the label
question. **Gate 2 proceeds on `work`-ranked AND labels.**

## Schema changes

### `NodeProven` (`src/proof_event.rs`)

```rust
pub struct NodeProven {
    pub path: Vec<Move>,
    pub mv: Move,
    pub hash: u64,
    pub outcome: Outcome,
    pub depth: u32,
    /// Cumulative `child_evals` spent proving this node's subtree.
    pub work: u64,
}
```

`NodeProven::new(path, hash, outcome, depth, work)`.

### Emission sites (`src/search/dfpn/`)

| Site | `work` value |
|---|---|
| `core.rs` terminal early return | `0` (no child evaluation happened) |
| `core.rs` TT-reuse (`try_use_tt`) | `0` (no re-expansion; canonicalization later copies the expanded twin's work) |
| `core.rs` prove time | `work = self.child_evals - child_evals_start` (the value stored in the TT) |
| `children.rs` `evaluate_child` (terminal / TT-resolved child) | `self.child_evals - child_evals_start`, i.e. **1** — the child-evaluation increment itself |

The last site is deliberate: a leaf child of an AND node (mate in 1) must not
be censored (`work == 0` censored per `docs/spec/nn.md` §6); every child of an
AND node therefore records `work >= 1`.

### `ProofNode` (`src/proof_tree/node.rs`)

`ProofNode` gains `work: u64`; `ProofTree::add_node` takes a `work` parameter
(root starts at 0; `ProofTree::new` signature unchanged). `apply_event`
**max-updates** `node.work` on duplicate events (same semantics as
`TtEntry.work`'s max-update). `finalize()` copies the canonical expanded
twin's `work` onto unexpanded transpositions and sets the root's work, so the
finalized tree is authoritative.

### Dump v2 (`src/proof_tree/binary.rs`, `docs/spec/proof_tree_dump.md`)

Every node record is now `parent_id` (4) + `move_code` (2) + `work` (8, u64
LE) = 14 bytes. The reader accepts:

- v1 (6-byte records) → every node loads with `work == 0` (the repo's old
  `proof_tree.bin` still parses; verified via `inspect_pt`);
- v2 (14-byte records) → `work` restored;
- anything else → `InvalidData` (unit-tested for version 3).

Corpora generated from v1 dumps are stale for the `work`-ranked AND label and
must be regenerated (done below).

### `corpus_gen` (`examples/corpus_gen.rs`)

`CORPUS_VERSION` → `"atomic-corpus/2"`. `load` emits `children[].work` from
the v2 dump alongside the existing `subtree_size`, so a trainer can rank AND
children by real work (or keep subtree size for comparison). `Row` itself is
unchanged.

### `work_proxy_ablation` (`examples/work_proxy_ablation.rs`)

Child work now reads `ProofNode.work` (always present post-finalize), so every
AND node with ≥ 2 children is complete by construction (`complete == and`).
The per-child TT probe is kept as a cross-check: `coverage` is the TT hit
rate and `tt_agree` the fraction of probed children whose TT `work` equals the
recorded work. Output gained a `tt_agree` column (stderr summary + stdout
table header `tag%`).

## Measured numbers (recorded-work ground truth, `--timeout 10`, 64 MB TT, 256 MB pt)

Quick suite aggregate:

```
aggregate    22940 22940   99.7%   78.9%   171317   33.1%     0.46   30.2%   79.6%      48.3%
```

Decisive aggregate:

```
aggregate    18808 18808   99.6%   78.0%   153381   33.8%     0.46   31.5%   78.0%      47.7%
```

Per-case highlights (quick run): dec10 (the transposition-heavy case) shows
the largest shift — `flip%` 32.7% (design A) → 40.1% (recorded), `kendall`
0.53 → 0.38, `top_agree` 80.5% → 76.8%; m23_black 33.0% → 39.1%; m23_white
16.3% → 20.9%. Shallow fully-solved mates stay perfect (dec03/dec18/dec22/
dec23/m27 = 0% flips). Only m26 (3 complete nodes, 5 pairs) shows a
near-inverted ranking (`kendall −0.50`) — tiny-sample noise, not a trend.

Interpretation:

- **The TT probe understated the flip rate by ~5–8 pp.** `TtEntry.work` is
  max-updated on *every* store, including unsolved entries from work-bounded
  chunks that later prove cheaper (TT reuse); recorded work is the exact
  delta at the prove that produced the proof subtree. The design-A numbers
  were directionally right (proxy rejected) but *biased toward the proxy*.
- **`tt_agree ≈ 79%`** on both suites: for ~21% of children the TT's max
  `work` exceeds the recorded work, which is exactly the unsolved-store
  inflation above. `tt_agree` does not track TT coverage (99.6–99.7%), so
  eviction plays no role; the disagreement is the max-update bias, not
  missing entries.
- **Recorded work is monotone with depth, size-independent.** The flip noise
  grows with depth and branching (dec10: 40%), confirming report3's
  structural explanation: the proxy counts only surviving proof nodes, while
  real work includes refutation and re-expansion effort. The new label fixes
  this at the source.

## Invariants verified

- `complete == and_nodes` in every case (recorded work always present).
- Every AND child in the regenerated corpus has `work >= 1`
  (17,237 entries, min 1) — no censored leaf labels.
- The tiny bit at stake: terminal root `work == 0` is the only zero left, and
  it has no children.

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets`, `cargo doc --no-deps` —
  clean.
- `cargo test --release` (154 lib + all integration suites, 26 binaries) —
  **all pass** except two tests that are marginal on this aarch64-emulated
  container and flake only during full-suite runs:
  - `test_move_order::m22_white_solves_in_10s` — the search completes in
    ~10.0 s wall on this machine (chunk logs: 0.159/0.486/1.140/2.458/5.044/
    8.699 s, identical between passing and failing runs); a sub-0.3 s jitter
    flips the 10 s budget. Passes 5/5 in isolation; the search hot path is
    untouched by design B (verified bit-identical chunk timing).
  - `test_plan6::m22_black_loses` — the 60 s-budget regression, already
    documented in report3 as unmeetable on this container (times out even at
    180 s).
- New unit tests pass: `worker_records_work_and_max_updates_on_duplicates`,
  `finalize_copies_work_from_expanded_twin`,
  `reads_version_one_dump_with_zero_work`, `rejects_unknown_version`; the
  round-trip test now asserts `work` is preserved through
  `to_bin`/`from_bin`.
- `tests/test_corpus_gen.rs` (4) — round trip also asserts
  `_meta == atomic-corpus/2`, every child carries u64 `work`, and AND children
  have `work > 0`.
- `tests/test_work_proxy_ablation.rs` (3) — now also asserts `tt_agree=` in
  the per-case summary.
- End-to-end: tiny fen `corpus_gen solve` → v2 bin → `load` → NDJSON row has
  `children[].work == 1` for the mate-in-1 child.
- v1 compat: the repo's old 70-byte `proof_tree.bin` (version byte 1) still
  loads via `inspect_pt`; version 3 is rejected by a unit test.
- Corpus regenerated: `data/corpus/trees` (36 v2 bins + manifest,
  `--suite quick --timeout 20`) and `data/corpus/train.ndjson`
  (`atomic-corpus/2`, 20,224 unique rows: 14,203 OR + 6,021 AND,
  17,237 AND-child `work` entries, all ≥ 1; 15,899 partial rows flagged).

## Diff vs plan

- **`eval_child` work is 1 for resolved leaves** (the plan's "terminal /
  TT-resolved: 1" decision), verified end-to-end in the corpus (`min work 1`).
- **Coverage redefinition**: with recorded work always available, `coverage`
  now means TT-probe coverage for the cross-check (was: required for node
  eligibility). `complete` is 100% by construction.
- **`tt_agree` per case** added beyond the plan outline; it quantifies how
  close the design-A proxy was.
- No change to the search algorithm, the scorer, or the worker message
  protocol; only the event/node/dump schemas grew one field.
- The corpus was regenerated at the Makefile settings (`--timeout 20`),
  matching the previous deliverable's manifest.

## Next step

Gate 2 (`docs/plans/nn/plan_external_trainer.md`) can now train
**"rank the children by `work`"** as the AND label and "first decisive child
first" as the OR label, using `atom-corpus/2`. Any retained comparison to the
old label must note the v1 corpora are stale.