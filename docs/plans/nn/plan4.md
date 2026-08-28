# Plan 4: real work counters in the proof tree (design B, Gate 1.5)

## Goal

Implement the design B escalation from `docs/plans/nn/plan3.md` §Escalation,
which `report3.md` recommends (pair flip rate 28–29%, work-weighted flip share
45–48%, both far above the 5–10% go/no-go ceiling). Design A measured the
proxy against the TT's `work` counter; design B makes the **real per-child
work a first-class datum of the proof tree** so the corpus AND label can rank
children by it.

- add `work: u64` to `NodeProven` (`src/proof_event.rs`) and `ProofNode`
  (`src/proof_tree/node.rs`), recorded at prove time by the search (the
  `child_evals` delta of the proven subtree),
- bump the `.bin` dump to v2 (`src/proof_tree/binary.rs`,
  `docs/spec/proof_tree_dump.md`),
- `corpus_gen` emits `work` per child (`atomic-corpus/2`), so the AND label is
  "rank children by `work`",
- re-run the ablation with the recorded tree work as the authoritative ground
  truth (the TT probe becomes a cross-check), and regenerate the corpus.

## Background

- `TtEntry.work` (plan3 design A) is max-updated per hash, not per path, and
  only survives while the entry is live. Design B records the same value at
  prove time into the events the tree already consumes, so the finalized tree
  — the actual corpus source — carries it exactly.
- `Search::dfpn` already computes `work = self.child_evals -
  child_evals_start` at prove time (`src/search/dfpn/core.rs:240`); the emit
  site is `emit_proof_node`. Terminal nodes emit before any child evaluation
  (work 0); TT-reused nodes emit before re-expansion (work 0, later corrected
  by the canonicalization copy).
- `ProofNode` already carries `hash`, so a child's work is probeable directly
  from the tree; no TT probe is needed for the label itself.

## Decisions (pinned here)

1. **`NodeProven.work: u64`**, set by the search at every emit site:
   - terminal node (`core.rs` early return): `0`,
   - prove time (`core.rs` `emit_proof_node(pos, outcome,
     outcome_to_store_depth, work)`): the existing `work` delta,
   - TT reuse (`core.rs` `try_use_tt` path): `0` (the node may be an
     unexpanded twin; canonicalization later copies the expanded twin's
     work),
   - child resolved without recursion (`children.rs` `evaluate_child`):
     `self.child_evals - child_evals_start_at_evaluate_child_entry` = 1 for
     terminal / TT-resolved children (the `evaluate_child` increment itself).
     This keeps every leaf child of an AND node out of the censored (work 0)
     class.
2. **`ProofNode.work: u64`** (default 0). `ProofTree::add_node` gains a `work`
   parameter; `ProofTree::new` keeps its signature (root work starts 0).
   `apply_event` **max-updates** `node.work` on duplicate events, matching
   `TtEntry.work`'s max-update semantics.
3. **Dump v2.** `VERSION = 2`; every node record becomes `parent_id` (4) +
   `move_code` (2) + `work` (8, u64 LE) = 14 bytes. `read_proof_tree` accepts
   v1 (6-byte records, `work = 0`) and v2; anything else is rejected.
   `docs/spec/proof_tree_dump.md` is updated to v2.
4. **`corpus_gen`**: `CORPUS_VERSION` → `atomic-corpus/2`; `children[]` rows
   gain `"work"` (from `tree.nodes[c].work`). The AND label (concept.md §5)
   becomes "rank the children by recorded `work`" — lowest work = cheapest
   subtree first (`docs/spec/nn.md` §6 pins the direction).
5. **`work_proxy_ablation`**: child work now reads `ProofNode.work` (always
   present post-finalize), so every AND node is complete by construction; the
   TT probe stays as a cross-check: report `tt_agree` (fraction of probed
   children whose TT `work` equals the recorded tree work) and keep reporting
   TT coverage. The `Search::tt_work_for` accessor stays (used by the
   cross-check and its unit test).
6. No change to the `Search` algorithm, scorer, history/killer, or
   `ProofTreeWorker` message protocol; only the event/node/dump schema grows a
   field. Old v1 dumps still load (work 0); corpora must be regenerated.

## Files changed

- `src/proof_event.rs` (work field + constructor param)
- `src/search/dfpn/mod.rs` (`emit_proof_node` signature)
- `src/search/dfpn/core.rs` (three emit sites)
- `src/search/dfpn/children.rs` (`evaluate_child` emit site)
- `src/proof_tree/node.rs` (field + `add_node` param)
- `src/proof_tree/worker.rs` (apply_event max-update; finalize copies work)
- `src/proof_tree/binary.rs` (v2 writer, v1/v2 reader)
- `src/proof_tree/node/tests.rs`, `src/proof_tree/worker/tests.rs`,
  `src/proof_tree/binary.rs` tests (round trip + v1 compat)
- `examples/corpus_gen.rs` (work per child, corpus v2)
- `examples/move_order_fractions.rs`, `examples/work_proxy_ablation.rs`
  (constructor updates; ablation uses tree work + TT cross-check)
- `tests/test_corpus_gen.rs` (work in child rows)
- `AGENTS.md` (worker/corpus/ablation entries)
- `docs/spec/proof_tree_dump.md` (v2), `docs/plans/nn/concept.md` (§5 label)
- `docs/plans/nn/report4.md` (final report; this plan's final task)

## Verification

```bash
cargo fmt --check
cargo clippy --all-targets
cargo test --release

cargo run --release --example corpus_gen -- solve --fen \
    "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1" --timeout 2 --dump-dir /tmp/pt4
cargo run --release --example corpus_gen -- load --dump-dir /tmp/pt4
# assert children[].work present and > 0 for AND children

cargo run --release --example work_proxy_ablation -- --suite quick --timeout 10
cargo run --release --example work_proxy_ablation -- --suite decisive --timeout 10
# proxy-flip numbers with recorded work; tt_agree cross-check ~1.0 where
# the TT entry survived and is not stale
```

Sanity invariants:

- every non-root node in a finalized tree has `work >= 1` (leaves reached as
  children measure ≥ 1; internal nodes measure ≥ 1; canonical copies inherit
  the expanded twin's work);
- v1 dumps load with `work == 0` everywhere;
- `children[].work` in the NDJSON agrees with the recorded tree for a tiny
  fen case (and the row invariant `work >= 1` holds for AND children).

## Success criteria

1. Builds pass, `cargo test --release` passes on every target except the
   pre-existing `test_plan6::m22_black_loses` machine-speed timeout.
2. `work_proxy_ablation` reports per-case and aggregate flip / kendall /
   top-agreement / work-flip from the recorded tree work, plus `tt_agree` and
   TT coverage per case.
3. A freshly generated `atomic-corpus/2` NDJSON carries `children[].work`;
   `corpus_gen load` on the generated v2 bins reproduces it.
4. `report4.md` states the re-measured flip rates (recorded work as ground
   truth) and confirms (or refutes) that the design-A TT probe was a faithful
   stand-in.

## Proxy decision (carried from plan3)

With recorded work in hand, the corpus AND label is "rank by work". The report
must state whether the design-A conclusion (proxy rejected) holds unchanged,
and what the corrected label implies for Gate 2 (`plan_external_trainer.md`).

## Final task

Write `docs/plans/nn/report4.md` covering: the schema changes (event/node/dump/
corpus), the emission work sites, the measured numbers (with the recorded-work
ground truth and the TT cross-check), the v1/v2 compat decision, and the
updated verdict and next step (Gate 2 on `work`-ranked labels).