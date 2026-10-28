# Plan 10: `proof_tree` layer — one proven-depth rule, dead worker protocol removed, `Clear` decision

Initiative: `cleanup` (housekeeping: DRY, YAGNI, lint, module sizing).
Opened by: the owner-requested code-size/readability review session
(2026-10-28). This plan touches the **offline** proof-tree layer only; the
search path is untouched, so all bars are structural/unit-level rather
than search-eval anchors (the search anchors are still run, because they
are free and they pin "nothing leaked into `search`").

Plan numbering: independent of `plan8.md`/`plan9.md`.

## Prerequisite reading (self-contained)

- `src/proof_tree/worker.rs` — 663 lines / 25.2 KB. The proven-depth
  rule (Win ⇒ `min(child depths) + 1`, Loss ⇒ `max(child depths) + 1`,
  Draw/None ⇒ 0) is implemented **twice**: `implied_depth`
  (lines 414–442, and it allocates a `Vec` per node) and the
  `finalize_tree` recompute (lines 549–572).
- `src/proof_tree/worker.rs:129-139` — `ProofTreeWorkerHandle::dump_to_bin`
  (with its doc comment) and the `ProofTreeWorkerMessage::DumpToBin`
  variant (lines 39–42) + its handler arm (lines 217–221). `dump_to_bin` has **no caller anywhere in the
  repository** (`grep -rn 'dump_to_bin' src examples tests` → the
  definition only); every consumer (`examples/reconstruct_pt.rs`,
  `examples/proofdb/*`) obtains the tree via `handle.tree()` and serializes
  it with `ProofTree::to_bin` / `binary::write_proof_tree`.
- `src/proof_event.rs` — `ProofEvent::{Clear,
  NodeProven}`. `Clear` has **no producer**: `Search` never emits it
  (pinned by `src/search/dfpn/children.rs:1135`,
  `ProofEvent::Clear => panic!("the search never clears proof events")`),
  the reconstruct walker only sends `NodeProven`, and the only consumer of
  the variant is the worker's `clear()` (lines 223–228) plus one unit test
  (`src/proof_tree/worker/tests.rs:371`).
- `src/position.rs:326-336` — `impl Default for Position` (delegates to
  `Position::new()`). `Position::default` has **no caller** (`Position::new`
  itself is used by `list_legal`, `move_order_debug`, `twin_stats`, and
  `notation`'s tests).
- `AGENTS.md` §"Dependency direction" — `search` depends only on
  `proof_event`; `proof_tree` consumes `ProofEvent`. That contract is why
  the protocol type stays (see D1/D2).

## Why this plan exists

1. **A load-bearing value is computed two ways.** `implied_depth` decides
   which expanded node is canonical (`build_expanded_index`), and
   `finalize_tree` recomputes the same quantity on the rebuilt tree. They
   must agree; if one is ever edited alone, canonicalization and the
   installed tree disagree and the failure looks like a validator defect
   rather than a refactor slip.
2. **`implied_depth` allocates.** It collects `Vec<u32>` of child depths
   for every expanded node on every finalize, then takes min/max. It is an
   offline path (no wall budget), so this is not a performance plan — but
   the allocation is also the reason the two copies *look* different
   (`itertools`-free min/max over a slice vs. over an iterator).
3. **A dead protocol endpoint.** `dump_to_bin` + `DumpToBin` are ~25 lines
   of message plumbing, a channel round-trip, and an `io::Result` channel
   type for a capability that nothing uses. Keeping it invites a future
   reader to think the worker owns dump I/O (it does not: `finalize()` +
   `tree()` + `to_bin` is the shipped contract, `AGENTS.md` §"Output
   priorities" item 3).
4. **A protocol variant with no producer.** `ProofEvent::Clear` is a
   documented "for workers and tests" reset. YAGNI: `ProofTree::new` is
   reachable from every consumer, and the worker's `clear()` re-implements
   three of the four resets `ProofTreeWorker::new` already performs.
5. **A vestigial `Default` impl** on `Position` (unused; `Position::new`
   is the named constructor).
6. Two adjacent micro-verbosities in `finalize_tree`: an
   `if node.outcome.is_none() { continue; }` immediately followed by
   `let Some(outcome) = node.outcome else { continue; };` (the same test
   twice), and `let eff_outcome = eff_node.outcome;` + a later
   `let Some(leaf_outcome) = eff_outcome else { continue; };`.

## Scope

**In scope**

1. Phase A — extract `fn proven_depth(outcome, child_depths: impl
   Iterator<Item = u32>) -> u32` and use it in both places; drop the `Vec`.
2. Phase B — delete `ProofTreeWorkerHandle::dump_to_bin`, the `DumpToBin`
   message variant, its handler arm; delete the doubled
   `is_none()`/`let-else` and inline the `eff_outcome` handling.
3. Phase C — delete `impl Default for Position`.
4. Phase D — **decision point** on `ProofEvent::Clear` (recommendation P1
   below), executed only if the decision is taken in this session.

**Out of scope (non-goals)**

- No change to the tree structure, canonicalization *result*, validation
  rules, memory accounting, or the binary format.
- No rename of the public proof-tree API (`ProofTree`, `ProofNode`,
  `ProofTreeWorkerHandle`, `validate_proof_tree`, `ProofStats`, `binary`).
- No collapse of `ProofEvent` into `NodeProven` (see D2: recorded as a
  follow-up decision with its own rationale, not done here).
- No touch of `src/reconstruct`, `examples/proofdb`, or the worker's event
  loop / dummy-node reconciliation.

## Normative design

### Phase A — one rule, no allocation

```rust
/// The proven depth implied by an expanded node's children.
///
/// A Win node is proven as soon as one child is proven (min depth + 1); a
/// Loss node needs every reply proven (max depth + 1). Draws and unrealized
/// nodes carry no proven depth.
fn proven_depth(outcome: Option<Outcome>, child_depths: impl Iterator<Item = u32>) -> u32 {
    match outcome {
        Some(Outcome::Win) => child_depths.min().unwrap_or(0).saturating_add(1),
        Some(Outcome::Loss) => child_depths.max().unwrap_or(0).saturating_add(1),
        _ => 0,
    }
}
```

- A-1. `implied_depth` becomes:

```rust
    fn implied_depth(&self, id: usize) -> u32 {
        let node = &self.tree.nodes[id];
        if node.depth == 0 {
            return 0;
        }
        proven_depth(
            node.outcome,
            self.tree.children(id).map(|c| self.tree.nodes[c].depth),
        )
    }
```

  (Note for the implementer: `node`, `self.tree.children(id)` and
  `self.tree.nodes[c]` are all shared borrows — no borrow conflict.)

- A-2. `finalize_tree`'s recompute loop uses the same helper:

```rust
        for i in (0..new_tree.nodes.len()).rev() {
            if new_tree.nodes[i].first_child.is_none() {
                new_tree.nodes[i].depth = 0;
            } else {
                new_tree.nodes[i].depth = proven_depth(
                    new_tree.nodes[i].outcome,
                    new_tree.children(i).map(|c| new_tree.nodes[c].depth),
                );
            }
        }
```

- A-3. Equivalence is exact: both sites previously used
  `iter().min()/max()` over a collected `Vec` of the identical iterator;
  `unwrap_or(0) + 1` is preserved (`saturating_add(1)` is the same
  operation that `finalize_tree` used; `implied_depth` used
  `saturating_add(1)` too).
- A-4. Free function placement: next to `implied_depth`, i.e. inside the
  `impl ProofTreeWorker` block as an associated-function-free item is
  **not** possible; put it as a private module-level `fn` above
  `struct ProofTreeWorker` (it has no `self`). Do not mark it `pub`.

### Phase B — dead protocol + doubled test

- B-1. Delete from `ProofTreeWorkerHandle`: the `dump_to_bin` method
  (lines 129–139, doc comment included). Delete the
  `ProofTreeWorkerMessage::DumpToBin { path, tx }` variant and the
  corresponding `handle_query` arm. `std::io` stays imported only if still
  used (the worker returns `io::Result` nowhere else — check and drop the
  `use std::io;` if it becomes unused).
- B-2. In `finalize_tree`'s `Action::Enter` arm, replace

```rust
                    let node = &old_tree.nodes[raw_id];
                    if node.outcome.is_none() {
                        continue;
                    }
                    let Some(outcome) = node.outcome else {
                        continue;
                    };
```

  with

```rust
                    let node = &old_tree.nodes[raw_id];
                    // Unrealized dummy ancestors are skipped during the rebuild.
                    let Some(outcome) = node.outcome else {
                        continue;
                    };
```

- B-3 (**analysed, not executed**). In the same arm,
  `let eff_outcome = eff_node.outcome;` feeds two later uses (the
  cycle-guard leaf and `add_node`). Folding the cycle guard into
  `if !is_root && path_hashes.contains(&hash) && let Some(leaf_outcome) = eff_outcome { … }`
  looks like a cleanup, but it changes behaviour: today a cyclic child
  whose effective node has no outcome `continue`s **without adding a
  node**; the folded form falls through to the normal path and adds one.
  That is exactly the class of edit this plan forbids — recorded here so
  the next reader does not "fix" it. Only B-2's duplicate
  `if node.outcome.is_none()` goes away.
- B-4. `ProofTreeWorkerMessage` shrinks to `GetStats(Sender<ProofResponse>)`,
  `GetTree(Sender<ProofResponse>)`, `Finalize`. Nothing else changes: the
  query channel still exists because `stats()`/`tree()`/`finalize()` are
  the shipped handle surface.

### Phase C — vestigial `Default`

- C-1. Delete `impl Default for Position { … }` (`src/position.rs`
  lines ~326–336). `Position::new()` stays. No caller is affected (verified
  by `grep -rn 'Position::default'` → none; `Position::new` is unaffected).
- C-2. If `cargo clippy` grows a `new_without_default` warning, add
  `#[allow(clippy::new_without_default)]` on `Position::new` **with a
  one-line reason** rather than resurrecting the impl (an unused
  `Default` on a type whose constructor parses a FEN and cannot fail is
  noise).

### Phase D — `ProofEvent::Clear` decision (owner call, default D1)

Two protocol options; the plan executes **P1** unless the owner says
otherwise in the session kickoff.

- **P1 (recommended, execute):** delete the `Clear` variant, the worker's
  `clear()` method, its handler arm, and
  `src/proof_tree/worker/tests.rs:371`'s `worker.handle_event(ProofEvent::Clear)`
  test (if that test asserts only the reset, delete it; if it asserts
  something else, split it). Then `proof_event.rs`'s header replaces the
  "the solver never emits `Clear`; `Clear` exists for workers and tests"
  paragraph with: "`ProofEvent` has one variant today; it stays an enum so
  that new event kinds (e.g. an explicit invalidation) can be added without
  touching `search`'s signature — see `AGENTS.md` §Dependency direction."
  `Search`'s emit site becomes `ProofEvent::NodeProven(..)` unchanged, and
  `children.rs:1135`'s `panic!("the search never clears proof events")`
  match arm disappears (the match becomes irrefutable — keep a
  `let ProofEvent::NodeProven(np) = event;` pattern instead).
  Removes ~30 lines across `proof_event.rs`, `worker.rs`, `tests.rs`,
  `children.rs`.
- **P2 (recorded, not executed):** collapse `ProofEvent` into
  `NodeProven` (`Sender<NodeProven>`), deleting the enum entirely
  (~25 more lines). Rejected for now because `AGENTS.md` names
  `ProofEvent` as the decoupling contract and a future `ProofSink` trait
  is a documented stretch goal; revisit only if the protocol stays
  single-variant across another initiative.

- D-1. The unit test that pins "the search never emits `Clear`"
  (`children.rs`'s match arm) must be **re-expressed**, not deleted, when
  P1 lands: with one variant the property is enforced by the type system,
  so the assertion to keep is the search's own emitted-event set (already
  covered by the proof-tree integration tests: `tests/reconstruct.rs`,
  `tests/test_proof_tree.rs`, `tests/test_proof_validate.rs`).

## Phase 0 — baseline capture (read-only)

1. `grep -rn 'dump_to_bin\|DumpToBin' src examples tests` → one site
   (the definition + variant + arm).
2. `grep -rn 'ProofEvent::Clear' src examples tests` → 3 sites
   (variant, handler arm, one unit test, plus the `panic!` arm).
3. `grep -rn 'Position::default' src examples tests` → none.
4. Fast gate baseline: `cargo test --release` (record the test count and
   wall time), `wc -l src/proof_tree/worker.rs src/position.rs
   src/proof_event.rs`.
5. The three search anchors from `plan8.md` §Phase 0 (quick @ 256 =
   42,109,768 / 59, dec31 = 510,426, m22 FO = 15,023,911) — these must be
   unchanged at every bar; they are the tripwire for "an edit leaked into
   `search`".

## Execution phases and bars

**Bar D1 (Phase A)** — `grep -c 'child_depths' src/proof_tree/worker.rs`
shows one implementation; no `Vec<u32>` in `implied_depth`; `cargo fmt` +
`clippy` clean; the worker unit tests (`src/proof_tree/worker/tests.rs`,
806 lines) green, in particular the finalize/canonicalization tests;
`tests/test_proof_tree.rs` + `tests/reconstruct.rs` +
`tests/test_proof_validate.rs` green (they exercise real finalize +
validate).

**Bar D2 (Phase B)** — the three greps from Phase 0 return nothing;
`make test` green; the proof-tree binary dump is byte-identical for a
fixed input (`examples/reconstruct_pt` on a committed fixture snapshot, or
`tests/reconstruct.rs`'s dumps compared by sha256 before/after).

**Bar D3 (Phase C)** — `Position::default` absent; `clippy` clean or the
documented allow present; `make test` green.

**Bar D4 (Phase D, only if executed)** — `ProofEvent` has exactly one
variant; proof-tree integration tests green; no test deleted (D-1).

**Bar D5 (whole plan)** — the three search anchors are child-eval
identical; `cargo fmt --check`, `cargo clippy --all-targets`,
`cargo doc` no new warnings; `make test` green; `git status
--porcelain` lists only `src/proof_tree/worker.rs`,
`src/proof_tree/worker/tests.rs` (P1 only), `src/proof_event.rs`
(P1 only), `src/position.rs`, `src/search/dfpn/children.rs` (P1 only),
plus the `docs/plans/cleanup/` deliverables.

**Rollback rule** — Phases A–D are independent; revert only the failing
phase. A canonicalization/canonical-node *result* change is a
stop-and-investigate event: Phase A must be an exact rewrite (A-3); if a
finalize test changes its expected canonical id, the extraction is not
equivalent and must be reverted, never re-baselined.

## Tests to add / change

- Phase A: no new test (both call sites are covered by the worker's
  finalize tests). If `implied_depth` has no direct unit test at HEAD,
  add one: an explicit Win/Loss/None depth table fed through
  `proven_depth` (3 asserts, cheap) — the plan **recommends** this, since
  the item is a pure function now.
- Phase B: none.
- Phase C: none.
- Phase D (if executed): delete the `Clear` unit test (its subject is
  removed) and note the deletion in the report — this is the only
  sanctioned test deletion in this plan.

## Measurement artifacts

`docs/plans/cleanup/measurements/plan10/`: `README.md`, `env.json`,
`state/` (before/after greps, line counts, gate timings, dump shas). No
snapshots; any `.tt` written while testing becomes an empty `.keep`
tombstone or is deleted before the commit.

## Final task

Write `docs/plans/cleanup/report10.md`: the duplication inventory
(two proven-depth copies → one, `Vec` allocation removed), the dead-code
inventory with the grep evidence that `dump_to_bin`/`DumpToBin` had no
caller, the D-phase decision and its rationale, the `proven_depth` unit
table if added, and the note that `B-3`'s "simplification" was analysed
and deliberately **not** taken because it changes behaviour.
