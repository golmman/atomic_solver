# Report 10: `proof_tree` layer — one proven-depth rule, dead worker protocol removed, `Clear` decision

Implements `docs/plans/cleanup/plan10.md` (initiative `cleanup`). Base
commit `f2f9ad1`. All four phases (A, B, C and D as **P1**) executed; every
bar passes. Measurements: `docs/plans/cleanup/measurements/plan10/`.

## 1. Duplication inventory (Phase A)

| site | before | after |
| --- | --- | --- |
| `implied_depth` (canonical-twin selection) | collected `Vec<u32>` of child depths, then `iter().min()/max()` | `proven_depth(node.outcome, children.map(..))` |
| `finalize_tree` depth recompute | same rule, second copy, same `Vec` | same helper |
| proven-depth rule bodies | 2 | 1 (`fn proven_depth`, private, module level) |

Wording of the rule unchanged (`min + 1` for Win, `max + 1` for Loss,
`0` for Draw/None/leaf); `saturating_add(1)` preserved on both sides.
The extraction is allocation-free: the iterator is consumed by
`min`/`max` directly, so the per-expanded-node `Vec` on every finalize is
gone (an offline path, so this is a readability/latent-cost fix, not a
performance claim).

Equivalence evidence, matching plan A-3 exactly:

- worker unit tests, in particular the finalize/canonicalization fixtures
  (`finalize_prefers_shorter_consistent_twin`, `finalize_prunes_dummy_subtree`,
  `worker_replaces_win_child_with_shortest_loss`, …) — green with no
  expected-value change;
- `tests/reconstruct.rs`, `tests/test_proof_tree.rs`,
  `tests/test_proof_validate.rs` — green;
- the proof-tree binary dump from `reconstruct_pt` is **byte-identical**
  (sha256 `ab94c69f…`, 70 bytes, mate-in-3 fixture), which pins both the
  canonical-node choice and the recomputed depths end to end.

New test (recommended by the plan): `proof_tree::worker::tests::proven_depth_table`
— a three-assert Win/Loss/None table, plus a Draw assertion (4 asserts).

## 2. Dead-code inventory (Phases B, C, D)

| symbol | evidence before | action |
| --- | --- | --- |
| `ProofTreeWorkerHandle::dump_to_bin` (+ doc comment) | `grep -rn dump_to_bin src examples tests` → definition only; every consumer uses `handle.tree()` + `ProofTree::to_bin` / `binary::write_proof_tree` | deleted |
| `ProofTreeWorkerMessage::DumpToBin { path, tx }` + its `handle_query` arm | same grep; the only sender was `dump_to_bin` | deleted |
| `use std::io;` | the variant's `io::Result<()>` channel was its only use | deleted |
| `ProofEvent::Clear` | no producer in `search` (the old `children.rs` match arm asserted this) nor in `reconstruct/walker.rs`; only consumer was `ProofTreeWorker::clear` + one unit test | deleted (**P1**) |
| `ProofTreeWorker::clear` | only call site was the `Clear` handler arm; it re-implemented 3 of the 4 resets `ProofTreeWorker::new` already performs | deleted |
| `impl Default for Position` | `grep -rn 'Position::default' src examples tests` → none; `Position::new()` is the named constructor | deleted |

Phase B also removed the doubled test in the `Action::Enter` arm
(`if node.outcome.is_none() { continue; }` immediately followed by
`let Some(outcome) = node.outcome else { continue; };`) — one `let-else`
remains, with a comment that unrealized dummy ancestors are skipped.

`proof_event.rs` header now states the single-variant enum rationale (the
enum stays as the `search`↔`proof_tree` decoupling contract, per
`AGENTS.md` §Dependency direction) plus the monotone-stream property.
`AGENTS.md`'s `proof_event` bullet was updated to drop the `Clear` name.

### 2a. Phase D decision: **P1 executed** (delete `Clear`), P2 not taken

P1 was the plan's default and no kickoff objection was raised. P2 (collapse
`ProofEvent` into `Sender<NodeProven>`) stays recorded as the plan's D2
follow-up and was **not** taken: `AGENTS.md` names `ProofEvent` as the
decoupling contract and a future `ProofSink` trait is a documented stretch
goal. Cost of keeping the enum today: one wrapper type and one irrefutable
`let` in the worker.

The deleted unit test is `worker_clears_tree_directly` (its only subject was
the removed reset). D-1's assertion "the search never emits a reset" is
re-expressed, not deleted: `children/tests.rs`'s
event-stream match arm became

```rust
// `ProofEvent` has exactly one variant, so "the search never emits a
// reset" is enforced by the type system rather than by a match arm.
let ProofEvent::NodeProven(event) = &streams[0][0];
```

with the four original field assertions untouched. This is the only test
deletion in the plan (sanctioned by its §"Tests to add / change").

Note: the plan cites the panic arm at `src/search/dfpn/children.rs:1135`;
at HEAD the search-path test module lives in
`src/search/dfpn/children/tests.rs` (moved there by `cleanup/plan9`), so
the edit landed at `children/tests.rs:458`.

### 2b. B-3 analysed, deliberately **not** taken

The plan's B-3 "simplification" folds the cycle guard into
`if !is_root && path_hashes.contains(&hash) && let Some(leaf_outcome) = eff_outcome { … }`.
It is **not** behaviour-preserving: today a cyclic child whose effective
node has no outcome `continue`s *without adding a node*; the folded form
falls through and adds one (changing the dump). Left as is, and the reason
is restated here so a later reader does not "fix" it.

## 3. Census

| metric | before | after |
| --- | --- | --- |
| `src/proof_tree/worker.rs` | 663 lines / 25,213 B | 611 lines / 23,531 B |
| `src/proof_tree/worker/tests.rs` | 806 | 800 |
| `src/proof_event.rs` | 40 lines / 1,010 B | 41 lines / 1,057 B (+header) |
| `src/position.rs` | 573 | 570 |
| `src/search/dfpn/children/tests.rs` | 649 | 647 |
| `fn clear` in worker | 1 | 0 |
| `ProofTreeWorkerMessage` variants | 4 | 3 |
| `ProofEvent` variants | 2 | 1 |
| `impl Default for Position` | 1 | 0 |
| proven-depth rule copies | 2 | 1 |
| `Vec<u32>` in the depth recompute | 2 | 0 |
| fast-gate tests | 628 passed / 0 failed / 41 ignored | 628 passed / 0 failed / 41 ignored (+1 new `proven_depth_table`, −1 `worker_clears_tree_directly`) |

`worker.rs` stays above the 10 KiB guideline; its existing header
justification (worker state machine + query protocol + link-list child
bookkeeping + child-move index are one state machine) still holds and now
covers ~10% less code.

## 4. Bars

- **D1 (Phase A)** — pass. `grep -c child_depths src/proof_tree/worker.rs`
  = 3, all inside the single `fn proven_depth` signature/body; no
  `Vec<u32>` in `implied_depth`; `cargo fmt --check` and
  `cargo clippy --all-targets` clean; worker unit tests and the three
  proof-tree integration suites green.
- **D2 (Phase B)** — pass. `dump_to_bin` / `DumpToBin` greps return
  nothing; `make test`-equivalent (`cargo test --release`) green; the
  proof-tree dump is byte-identical (`ab94c69f…`).
- **D3 (Phase C)** — pass. `Position::default` absent; `clippy` grew the
  predicted `new_without_default` warning, so C-2's documented
  `#[allow(clippy::new_without_default)]` with a two-line reason is on
  `Position::new`; gate green.
- **D4 (Phase D)** — pass. `ProofEvent` has exactly one variant; proof-tree
  integration tests green; the only deleted test is the sanctioned one.
- **D5 (whole plan)** — pass, fast gate **and** `make test-full`. The three search anchors are child-eval
  identical: quick @ 256 = 59 cases / 42,109,768 evals / 0 wrong / 0
  timeouts (per-case diff: none), dec31 FO = 510,426 evals (loss, 22),
  m22 FO = 15,023,911 evals (win, 75). `make test-full` = 669 passed /
  0 failed / 0 ignored. `cargo fmt --check`,
  `cargo clippy --all-targets`, `cargo doc --no-deps` produce no warnings.
  `git status --porcelain` lists exactly `src/proof_tree/worker.rs`,
  `src/proof_tree/worker/tests.rs`, `src/proof_event.rs`,
  `src/position.rs`, `src/search/dfpn/children/tests.rs`, `AGENTS.md`
  (one-line protocol description sync) and the `docs/plans/cleanup/`
  deliverables.

No phase was reverted; no canonical-node result changed; the binary format
is untouched.

## 5. Deviations, findings, next steps

Deviations: none material. Two path corrections were needed (see §2a: the
search test module lives under `children/tests.rs`, not `children.rs`; and
`grep` shows `ProofEvent::Clear` at `worker/tests.rs:371`, consistent with
the plan).

Findings not fixed (out of scope, reported only):

- `worker.rs` is still the largest file in the proof-tree layer (~23.5 KB).
  A future split would have to cut through the single worker state machine
  (tree + child index + expanded index + memory accounting), which is the
  reason it is currently justified.
- `ProofTreeWorker::new` and the finalize-time `ProofTree::new` calls are
  the only tree constructors left; with `clear()` gone there is exactly one
  reset path (construct a new worker/tree), which is what P1 intended.
- `examples/reconstruct_pt --experiment` and the tooling-DRY items remain
  with `cleanup/plan11`.

Missing tests: none planned. `make test-full` was run as well (a
`proof_tree` change per `AGENTS.md` §"Testing tiers"): **669 passed / 0
failed / 0 ignored**, i.e. the fast tier's 628 plus all 41 previously
ignored slow tests, including the 60 s stress/regression suites.

Next steps: `cleanup/plan11` (tooling DRY + owner-gated
`reconstruct_pt --experiment` retirement).
