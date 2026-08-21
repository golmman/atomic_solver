# Plan 3: subtree-size proxy ablation (design A, Gate 0.5)

## Goal

Answer the open question from `docs/plans/nn/report1.md` ("Next steps", item
3) and `report2.md` ("Limitations"): **does the corpus's `subtree_size`
label reliably proxy the solver's real per-child work?** The AND-node label
in the corpus is "rank the children by derived post-order `subtree_size`"
(concept.md §5, plan2 Decisions). If that ranking disagrees with the real
child work the search recorded, training on `subtree_size` labels would
teach a noisy target.

Add `examples/work_proxy_ablation.rs`, an example binary that solves each
case, walks the finalized proof tree, and at every AND (Loss) node compares
the stored child `subtree_size` against the child's recorded real work — the
TT `work` counter (`cumulative child_evals spent under this subtree`,
src/search/tt/entry.rs) — reporting ordering agreement per case and in
aggregate.

Design **A** reads the real work from the in-memory transposition table via
one new public accessor on `Search` (`tt_work_for`). It changes no dump
format and no `src/` behavior. Design **B** (escalation, hinted at the end)
would add explicit `work` counters to `NodeProven`/`ProofNode` and a dump
v2 — only if A shows the proxy cannot be trusted.

## Background

- The corpus labels AND nodes by `children[].subtree_size` (post-order node
  count of the finalized proof tree) because the solver never records
  per-child work (concept.md §2; report2 Limitations; plan2 explicitly
  deferred "real per-child `child_evals` work counters" as a separate
  ablation).
- The real work already exists, hidden in the TT: every `TtEntry.work` is
  the cumulative child-evals spent while searching that subtree
  (`src/search/tt/entry.rs`, comment + the `work` field). The value is
  recorded at `store` time as `self.child_evals - child_evals_start`
  (`src/search/dfpn/core.rs:231`), which counts recursive child evaluations
  (`evaluate_child` increments `self.child_evals`, `src/search/dfpn/children.rs:83`).
  This is the same kind of "number of positions visited under this subtree"
  the subtree-size proxy intends.
- The TT persists for the whole solve (`corpus_gen`-style search), so after
  `solve_with_progress` + `finalize()` the entries are still present and
  probeable by Zobrist hash — the same hash carried by every `ProofNode`
  (`src/proof_tree/node.rs`).
- `TranspositionTable::probe_summary(key) -> Option<TtSummary>` is public
  (`src/search/tt/table.rs`); `TtSummary.work` is public (`src/search/tt/entry.rs`).
  The only missing API is a public accessor on `Search` (`tt` is private).
- Precedents: `examples/corpus_gen.rs` and `examples/move_order_fractions.rs`
  already implement the solve → worker spawn → finalize → subtree-size
  pass pattern this example reuses.

## Decisions (pinned here)

- **Design A.** Probe the TT by child hash at every AND node. One new public
  accessor: `Search::tt_work_for(key: u64) -> Option<u64>` returning
  `self.tt.probe_summary(key).map(|s| s.work)`. No other `src/` change.
- **Unit of comparison:** an AND node's *children*, as pairs. A "complete"
  node is one where every child's hash has a probed work value. Only
  complete nodes contribute; coverage is reported per case.
- **Metrics (report flat + work-weighted):**
  1. **Pair flip rate:** over all child pairs of complete AND nodes, the
     fraction where `subtree_size_i > subtree_size_j` disagrees with
     `work_i > work_j`. This is the label-noise rate the trainer would see.
  2. **Kendall τ** (mean over complete nodes, and pooled over all complete
     nodes).
  3. **Top-child agreement**: the fraction of complete AND nodes whose
     max-`subtree_size` child is also the max-`work` child.
  4. **Work-weighted flip share**: mis-ordered pairs weighted by
     `min(work_i, work_j)` over all complete-node pairs, divided by the
     total `min` weight. This is how much *real* work the proxy
     misattributes.
- **Coverage must be reported** per case: fraction of AND children whose
  hash hit the TT. TT eviction makes large cases (dec10) partial.
- **Go/no-go threshold (tentative, confirm in the report):** if the pair
  flip rate is ≤ 5–10% and the work-weighted flip share is similarly
  small, the proxy stands and Gate 2 proceeds on `subtree_size`. Above
  that, escalation to design B.

## Scope

In scope:

- `Search::tt_work_for` (one public accessor, `src/search/dfpn/mod.rs`).
- `examples/work_proxy_ablation.rs` (CLI, solve loop, tree walk, TT
  probing, metrics).
- `tests/test_work_proxy_ablation.rs` (small fen round-trip + exit-code).
- `AGENTS.md` examples entry.
- `docs/plans/nn/report3.md` (final report; the plan's final task).

Out of scope:

- Any change to the corpus schema, the `.bin` dump format, the worker, or
  the scorer.
- Design B (real `work` counters in `NodeProven`/`ProofNode`, dump v2,
  corpus v2) — escalated only if this plan's results require it.
- Any change to training labels; this plan only *measures*.

## Design

### 1. Accessor

`src/search/dfpn/mod.rs`:

```rust
/// Real work recorded in the transposition table for a position hash.
/// `None` if the entry was evicted or never stored.
#[must_use]
pub fn tt_work_for(&self, key: u64) -> Option<u64> {
    self.tt.probe_summary(key).map(|s| s.work)
}
```

### 2. Example CLI

```
work_proxy_ablation [OPTIONS]
  --fen <FEN>          Single position; case name "fen"
  --suite <NAME>       quick | decisive | all   (default: quick)
  --timeout <S>        Search budget in seconds  (default: 10)
  --epsilon <F>        DF-PN+ threshold          (default: 0.125)
  --tt-size <MB>       TT size                   (default: 64)
  --pt-size <MB>       Proof-tree memory budget  (default: 256)
  -h, --help
```

Suite mapping: `quick` = decisive + move-order cases `m ≥ 23` (same as
`corpus_gen`); `all` = move-order + decisive (same as `move_order_fractions`).
The move-order suite is held out of training but is fine for this
measurement — it is not training data.

### 3. Per-case flow

For each case, mirror `corpus_gen::solve_case`:

1. `Position::from_fen`, `Search::new(tt_size)` with `set_timeout`/`set_epsilon`.
2. `ProofTreeWorkerHandle::spawn(fen, pt_size, ...)`,
   `search.set_proof_event_sender(handle.event_sender())`.
3. `search.solve_with_progress(&mut pos, ...)`. If `outcome == Draw`,
   synthesize a Loss root event (same workaround as the other examples).
4. `handle.finalize()`; `let tree = handle.tree()`; keep `search` alive —
   the TT is needed for probing.

Then walk the tree with one mutable `Position` (DFS replay; same invariant
as report1/report2). At each node with `outcome == Loss` and ≥ 2 children:

- `subtree_sizes[id]` is the post-order proxy (from the tree).
- for each child: `child_work = search.tt_work_for(child.hash)`; record
  `None` for a miss.
- complete if all children have work; if complete and ≥ 2 children,
  accumulate the pair metrics of §2 ("Metrics").

Per case, emit to stderr a summary line:

```
=== dec10  outcome=win  and_nodes=44210  complete=38210  coverage=94.2%
    pairs=28431  pair_flip=7.8%  kendall=0.81  top_agree=88.2%  work_flip=5.9%
```

Then a suite-level aggregate with the same columns. stdout carries the
final summary table only.

### 4. Integration test

`tests/test_work_proxy_ablation.rs` (RUN_LOCK-serialized, release build,
mirroring `tests/test_corpus_gen.rs`): run with `--fen <tiny decisive>`
`--timeout 2 --tt-size 16 --pt-size 16`; assert exit 0 and a summary line
`pair_flip=` appears with plausible numbers (0–100%).

## Implementation steps

1. Add `Search::tt_work_for` (and a unit test in
   `src/search/dfpn/tests.rs`).
2. Add `examples/work_proxy_ablation.rs`.
3. Add `tests/test_work_proxy_ablation.rs`.
4. `AGENTS.md` example list entry.
5. `cargo fmt`, `cargo clippy --all-targets`, `cargo test`.
6. Run on one tiny FEN, then on `--suite quick --timeout 10` (the corpus
   settings) and note coverage + flip numbers. Also run `--suite decisive`
   to exercise dec10-style copied-subtree replay.
7. Write `docs/plans/nn/report3.md`.

## Files changed

- `src/search/dfpn/mod.rs` (accessor)
- `src/search/dfpn/tests.rs` (unit test)
- `examples/work_proxy_ablation.rs` (new)
- `tests/test_work_proxy_ablation.rs` (new)
- `AGENTS.md` (examples entry)
- `docs/plans/nn/report3.md` (new, final report)

No change to the corpus schema, the dump format, the worker, or the scorer.

## Verification

```bash
cargo fmt --check
cargo clippy --all-targets
cargo test

cargo run --release --example work_proxy_ablation -- \
    --fen "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1" --timeout 1 --tt-size 16 --pt-size 16
cargo run --release --example work_proxy_ablation -- --suite quick --timeout 10
cargo run --release --example work_proxy_ablation -- --suite decisive --timeout 10
```

Sanity: coverage (children probed / children total) is reported per case and
is plausibly high on small cases; `pair_flip` is well-defined on every
complete node (≥ 2 children).

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| TT eviction on big trees collapses coverage | Report coverage per case; interpret results through the covered sample and the coverage-weighted aggregate. |
| `TtEntry.work` is max-updated, not per-path | It is a per-hash approximation, but it is the *real* work the search spent; rank-level comparison is robust to mild staleness. |
| Same hash appears at multiple nodes (transpositions) | The tree canonicalization copies subtrees; probing by hash gives the same work for each copy — accepted and reported as a coverage nuance. |
| AND nodes with 1 child are useless for pairs | Excluded (they carry no ordering signal); complete requires ≥ 2 children. |
| `Draw` root / timeout cases | Synthesized-loss-root workaround (as before); partial trees analyzed and marked. |
| Proxy actually fine → B never needed | Then the report simply says so and Gate 2 proceeds on `subtree_size`. |

## Success criteria

1. `Search::tt_work_for` added with a unit test; everything builds and
   `cargo test` passes.
2. `work_proxy_ablation` runs on the corpus suites and produces per-case and
   aggregate flip / kendall / top-agreement / work-flip metrics with coverage.
3. `docs/plans/nn/report3.md` states the measured pair flip rate and the
   work-weighted flip share on the corpus suites, and a clear verdict:
   **subtree_size stays** (flip small) or **escalate to B** (flip large).

## Escalation (design B, only if A fails)

If the pair flip rate or work-weighted flip share is large (tentative
threshold ~10–15% / the report's judgment), the proxy is not trustworthy and
Gate 2 needs real work:

- add `work: u64` to `NodeProven` (`src/proof_event.rs`) and `ProofNode`
  (`src/proof_tree/node.rs`), recorded at prove time by the search
  (the `child_evals` delta for the proven subtree),
- bump the `.bin` dump to v2 (`src/proof_tree/binary.rs`,
  `docs/spec/proof_tree_dump.md`),
- `corpus_gen` emits `work` per child (AND label = rank by `work`), and
  the trainer switches.

B is a separate plan with real `src/` and format changes; it is deliberately
not started here. This plan measures first; B's plan is only drafted if the
numbers call for it.

## Final task

Write `docs/plans/nn/report3.md` covering:

- the example's CLI and output schema,
- the accessor and its unit test,
- measured numbers per suite: total AND nodes analyzed, complete-node
  coverage, pair flip rate, Kendall τ, top-child agreement, work-weighted
  flip share,
- the effect of TT eviction/max-updates on interpretation (coverage),
- the go/no-go verdict on the `subtree_size` proxy and, if no, the B
  escalation outline.

End with the next step: with the proxy validated, proceed to Gate 2
(`docs/plans/nn/plan_external_trainer.md`); with the proxy rejected,
escalate to design B.