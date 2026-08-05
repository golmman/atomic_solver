# Plan 6: Iterative bounded search with proof-tree emission, replacing PPV/SPPV extraction

## Summary

Phase 5 made the solver emit the full proven OR-AND subtree into the
`ProofTreeWorker`, but it still does this through a separate extraction pass
inside `find_ppv` / `refine_sppv`.  That keeps the staged pipeline
`solve_outcome` → `find_ppv` → `refine_sppv`, the `ProofMode::Ppv` /
`ProofMode::Sppv` variants, and a dedicated proven-subtree walker.

This plan removes that entire PPV/SPPV extraction stage.  Instead,
`Search::solve` will:

1. Search for any decisive outcome and extract its line length `N`.
2. Repeatedly run a bounded, work-chunked `dfpn` with `max_depth = N - 2`
   (plies).
3. Each time a shorter decisive line of length `M` is found, log `outcome` and
   `M`, then continue with `max_depth = M - 2`.
4. During every search, every node that `dfpn` proves or disproves is emitted
   as a `NodeProven` event to the `ProofTreeWorker`.
5. The existing pre-exit hook exports the accumulated proof tree to
   `proof_tree.bin`.

The final returned line is no longer labelled PPV or SPPV; it is simply the
last decisive line found before a bounded search fails or time runs out.  The
proof tree is a collection of all proven/disproven nodes, not a minimal
shortest-line tree.  The returned PV is the authoritative shortest line.

Because decisive plies have fixed parity for a given starting side (a `Win`
from the side to move is odd; a `Loss` is even), shorter decisive lines always
differ by two plies, so the `N - 2` step is correct.

## Goal and scope

### Goal

* Replace the staged `solve_outcome` → `find_ppv` → `refine_sppv` flow in
  `Search::solve` with a single iterative bounded search.
* Remove `ProofMode::Ppv`, `ProofMode::Sppv`, `Search::find_ppv`,
  `Search::refine_sppv`, `Search::ppv_cache`, the
  `extract_ppv_from_proven_subtree_emit` pass, and the `refine_shortest`
  configuration from the production solver path.
* Make `dfpn` emit `NodeProven` events for every proven node whenever a
  proof-tree sender is configured, instead of only during a separate
  extraction pass.
* Update the CLI and examples: remove `--no-refine-shortest`, add a
  `--first-outcome` flag, log each newly found decisive line length, and keep
  the pre-exit proof-tree dump.
* Keep `extract_pv` / `extract_pv_checked` / `validate_pv` because they are
  still needed to produce the returned PV and for tests/examples.

### Non-goals

* No change to the compact binary dump format, `ProofTreeWorker` data model,
  or `ProofMessage` protocol.
* No new external export targets (PostgreSQL remains post-MVP).
* No change to move generation, GHI handling, the transposition table, or the
  epsilon/threshold mechanism.
* No guarantee that the proof tree's own `extract_ppv` (if kept) produces the
  shortest line.  The returned PV from `solve` is the shortest line.

### Assumptions

* "Line length" is measured in plies.  Because the winner delivers the final
  blow on its own turn, a root `Win` has odd length and a root `Loss` has even
  length.  Shorter decisive lines therefore differ by multiples of two plies,
  so the `N - 2` step is correct.

## Background / current state

`Search::solve` currently runs three stages:

1. `solve_outcome` — work-bounded, `max_depth = u32::MAX`, with
   `in_proof_tree = false` so nothing is emitted to the worker.
2. `find_ppv` — clears the worker, then runs
   `extract_ppv_from_proven_subtree_emit` to emit the full proven subtree for
   the first decisive line.
3. `refine_sppv` — binary searches for shorter lines with
   `in_proof_tree = false` (to avoid polluting the worker) and, if a shorter
   line is found, clears and re-emits the proven subtree.

This is awkward because:

* The minimax logic is partially duplicated in
  `extract_ppv_from_proven_subtree_emit`.
* `dfpn` carries an `in_proof_tree` flag and conditional `proof_path` /
  `move_stack` bookkeeping that exists only for the extraction pass.
* The CLI exposes `--no-refine-shortest` and `examples/solve_no_refinement.rs`
  tests the non-refining path.

The new design inverts the data flow: `dfpn` itself emits proven nodes, so the
worker accumulates the proven subtree while the search is running.  The
shorter-line loop is just a sequence of bounded, work-chunked `dfpn` calls.

## Detailed design

### 1. Simplify `dfpn` and proof-mode bookkeeping

`ProofMode::Ppv` and `ProofMode::Sppv` exist only to control when `dfpn` stops
evaluating OR-node children.  With the new flow the solver is always in the
equivalent of `Outcome` mode: stop as soon as one winning child is proven for
an OR node, evaluate all children for an AND node.

* Remove the `ProofMode` enum and the `Search::proof_mode` field.
* Remove `proof_mode` parameters from `dfpn`, `evaluate_all_children`,
  `evaluate_child`, `select_from_children`, and `select_child_with_early_exit`.
* Simplify the solved-outcome handling in `dfpn`:
  * `Outcome::Win` (OR node): break after the first proven winning child.
  * `Outcome::Loss` (AND node): continue until `all_solved` is true.
  * `Outcome::Draw`: break when `all_solved` is true.
* Remove the `in_proof_tree` argument from `dfpn`,
  `evaluate_all_children`, and `evaluate_child`.
* Replace the `in_proof_tree` flag with an `emit` boolean computed once at the
  top of `dfpn` as `self.proof_tree_sender.is_some()`.
* When `emit` is true, always maintain `self.proof_path` and
  `self.move_stack` around recursive calls and always emit a `NodeProven`
  event when a node is proven.  When `emit` is false, skip all the string/path
  overhead entirely.
* Update `emit_proof_node` so it only checks `outcome != Outcome::Draw` and
  `self.proof_tree_sender.is_some()`.
* In `evaluate_child`, emit a `NodeProven` event whenever `info.outcome` is
  `Win` or `Loss` and a sender exists.

Result: `dfpn` has a single proof-mode; the proof tree is a side-effect of the
search itself.

### 2. Replace `solve` with iterative bounded refinement

Remove from `Search`:

* `find_ppv`
* `refine_sppv`
* `ppv_cache`
* `bootstrap_success_depth`
* `bootstrap_fail_depth`
* `refine_shortest` (field and setter)

Add a `first_outcome_only` flag:

```rust
pub fn set_first_outcome_only(&mut self, value: bool) { ... }
```

When set, `solve` stops after the first decisive outcome and still emits all
proven nodes to the proof tree during that first search.

Add an internal helper that runs one bounded, work-chunked `dfpn` without
resetting the overall node counter or deadline:

```rust
fn bounded_search(&mut self, pos: &mut Position, max_depth: u32) -> (Outcome, Vec<Move>) {
    let mut outcome = Outcome::Draw;
    let mut chunk = 500_000u64;
    let mut last_child_evals_before = 0u64;

    while !self.time_exceeded() {
        self.reset_search_state();
        last_child_evals_before = self.child_evals;
        outcome = self.dfpn(pos, INF, INF, max_depth, chunk, true);
        if outcome != Outcome::Draw {
            break;
        }

        let work_done = self.child_evals - last_child_evals_before;
        if self.linear_chunks {
            chunk = chunk.saturating_add(self.chunk_increment);
        } else {
            chunk = ((chunk as u128 * self.chunk_multiplier_num as u128)
                / self.chunk_multiplier_den as u128) as u64;
        }
        self.log_chunk(work_done, chunk, "bounded_search");
    }

    if outcome == Outcome::Draw && !self.time_exceeded() {
        self.reset_search_state();
        let work_done = self.child_evals - last_child_evals_before;
        self.log_chunk(work_done, u64::MAX, "bounded_search_fallback");
        outcome = self.dfpn(pos, INF, INF, max_depth, u64::MAX, true);
    }

    let pv = if outcome == Outcome::Draw {
        self.extract_pv(pos)
    } else {
        self.extract_pv_checked(pos, outcome, None)
            .unwrap_or_else(|| self.extract_pv(pos))
    };
    (outcome, pv)
}
```

Implement `Search::solve` as:

```rust
pub fn solve(&mut self, pos: &mut Position) -> (Outcome, Vec<Move>, u64) {
    self.solve_with_progress(pos, |_, _| {})
}

pub fn solve_with_progress<F>(&mut self, pos: &mut Position, mut on_progress: F) -> (Outcome, Vec<Move>, u64)
where
    F: FnMut(Outcome, &[Move]),
{
    self.begin_run();
    self.clear_proof_tree();

    // 1. First decisive outcome (work-chunked, unbounded depth).
    let mut outcome = Outcome::Draw;
    let mut chunk = 500_000u64;
    let mut last_child_evals_before = 0u64;
    while !self.time_exceeded() {
        self.reset_search_state();
        last_child_evals_before = self.child_evals;
        outcome = self.dfpn(pos, INF, INF, u32::MAX, chunk, true);
        if outcome != Outcome::Draw {
            break;
        }

        let work_done = self.child_evals - last_child_evals_before;
        if self.linear_chunks {
            chunk = chunk.saturating_add(self.chunk_increment);
        } else {
            chunk = ((chunk as u128 * self.chunk_multiplier_num as u128)
                / self.chunk_multiplier_den as u128) as u64;
        }
        self.log_chunk(work_done, chunk, "solve_outcome");
    }
    if outcome == Outcome::Draw && !self.time_exceeded() {
        self.reset_search_state();
        let work_done = self.child_evals - last_child_evals_before;
        self.log_chunk(work_done, u64::MAX, "solve_outcome_fallback");
        outcome = self.dfpn(pos, INF, INF, u32::MAX, u64::MAX, true);
    }

    let mut pv = if outcome == Outcome::Draw {
        self.extract_pv(pos)
    } else {
        self.extract_pv_checked(pos, outcome, None)
            .unwrap_or_else(|| self.extract_pv(pos))
    };
    if outcome != Outcome::Draw || !pv.is_empty() {
        on_progress(outcome, &pv);
    }

    // 2. Iteratively tighten the bound by two plies, unless the user asked
    //    for the first outcome only.
    let mut n = pv.len() as u32;
    while !self.first_outcome_only
        && outcome != Outcome::Draw
        && n > 2
        && !self.time_exceeded()
    {
        let bound = n - 2;
        let (new_outcome, new_pv) = self.bounded_search(pos, bound);
        if new_outcome == Outcome::Draw || new_pv.len() as u32 >= n {
            break;
        }
        outcome = new_outcome;
        pv = new_pv;
        n = pv.len() as u32;
        on_progress(outcome, &pv);
    }

    (outcome, pv, self.nodes)
}
```

`Search::search_depth` remains a public convenience wrapper around
`bounded_search` that calls `begin_run` first:

```rust
pub fn search_depth(&mut self, pos: &mut Position, max_depth: u32) -> (Outcome, Vec<Move>, u64) {
    self.begin_run();
    self.clear_proof_tree();
    let (outcome, pv) = self.bounded_search(pos, max_depth);
    (outcome, pv, self.nodes)
}
```

`Search::search_depth_with_prefix` should no longer call `solve`, because the
new `solve` would run the iterative loop and ignore the prefix.  Reimplement
it as a direct bounded `dfpn` with the prefix set:

```rust
pub fn search_depth_with_prefix(
    &mut self,
    pos: &mut Position,
    max_depth: u32,
    prefix_keys: &[u64],
    prefix_path_code: u64,
) -> (Outcome, u32, u64) {
    let saved_prefix = self.prefix_path.take();
    self.prefix_path = Some((prefix_keys.to_vec(), prefix_path_code));
    self.begin_run();

    let outcome = self.dfpn(pos, INF, INF, max_depth, u64::MAX, true);
    let pv = if outcome == Outcome::Win {
        self.extract_pv_checked(pos, Outcome::Win, None)
            .unwrap_or_else(|| self.extract_pv(pos))
    } else {
        Vec::new()
    };
    let depth = pv.len() as u32;

    self.prefix_path = saved_prefix;
    (outcome, depth, self.nodes)
}
```

`solve_outcome` can be removed as a public method; the first-outcome logic is
inlined into `solve`.  Any remaining callers (`examples/chunk_growth.rs` and
some tests) should switch to `solve` or `search_depth`.

### 3. `pv.rs` cleanup

* Remove `extract_ppv_from_proven_subtree_emit` and the emitting machinery.
* Remove the `#[cfg(test)]` `extract_ppv_from_proven_subtree` unless it is
  still useful for a unit test; if it is kept, move it behind `#[cfg(test)]`
  only.
* Keep `extract_pv`, `extract_pv_checked`, and `validate_pv` (used by `solve`,
  the CLI, tests, and examples).

### 4. `ProofTree` PPV extraction

`ProofTree::extract_ppv` is no longer needed by the main flow.  The solver
returns its own authoritative PV from `solve`; the proof tree is just a
container of all proven/disproven nodes for the binary dump.

* Stop calling `tree.extract_ppv()` in the pre-exit hook.
* Pass the `solve` PV to the pre-exit hook and validate it with
  `tree.validate_ppv(&pv)` and `Search::validate_pv(&pv, &fresh, outcome, None)`.
* Optionally remove `ProofTree::extract_ppv` entirely and update
  `tests/test_proof_tree.rs` to validate the returned PV instead.  If kept, it
  should be considered a diagnostic only, not the authoritative shortest line.

### 5. CLI and example updates

#### `src/main.rs`

* Remove `--no-refine-shortest`.
* Add `--first-outcome` (default off).  When set, `solve` stops after the
  first decisive outcome and still exports the proof tree for that outcome.
* Update the help text accordingly.
* Replace the `run_search` closure with something like:

```rust
let (outcome, pv, timed_out) = {
    let (outcome, pv, _nodes) = search.solve_with_progress(&mut pos, |o, line| {
        println!("outcome: {} length: {}", outcome_str(o), line.len());
    });

    if outcome != Outcome::Draw {
        println!("pv: {}", pv_str(&pv));
    }

    (outcome, pv, search.time_exceeded())
};

if timed_out {
    let msg = match search.exit_reason() {
        ExitReason::Quit => "quit",
        ExitReason::MemoryLimit => "memory",
        _ => "timeout",
    };
    println!("{msg}");
}
```

* Update the pre-exit hook signature to receive the PV:

```rust
type PreExitHook = Box<dyn FnOnce(ExitReason, Outcome, u64, &[Move]) + Send>;
```

The hook then validates and logs:

```rust
println!("proof_tree_ppv: {}", pv_str(&pv));
let valid = tree.validate_ppv(&pv) && Search::validate_pv(&pv, &fresh, outcome, None);
println!("ppv_valid: {valid}");
```

The rest of the hook (stats, binary dump) is unchanged.

Proposed CLI output for a winning position:

```text
outcome: win length: 7
outcome: win length: 5
outcome: win length: 3
pv: b1b8 g8f7 c3e2
```

For a draw:

```text
outcome: draw
```

For timeout:

```text
timeout
```

#### Examples

* `benchmark.rs`: remove `--refine-shortest`; `solve` always performs the
  iterative bounded refinement within the configured timeout.
* `solve_no_refinement.rs`: delete.  It no longer has a meaning.
* `chunk_growth.rs`: replace `search.solve_outcome` with `search.solve` or
  `search.search_depth` depending on what the harness needs to measure.
* `find_winning_child.rs`, `play_and_solve.rs`, `solve_depth_limited.rs`,
  `twin_stats.rs`, `static_move_scores.rs`, `verify_ppv.rs`: no API-breaking
  changes are expected, but verify after `solve` is reworked.

### 6. Test updates

#### `tests/common/mod.rs`

* Remove `solve_refined` and `solve_refined_moves`; rename or repurpose them to
  simple `solve` wrappers, because `solve` now always refines.
* Update callers accordingly.

#### `tests/test_plan6.rs`

* Remove or rewrite tests that call `find_ppv`, `refine_sppv`, or
  `solve_outcome` directly.
* Keep the position assertions: expected `Outcome` and, where relevant, PV
  length and first moves.
* Replace `m27_ppv_only` and `timeout_message` (which relied on `find_ppv` and
  the old CLI output) with tests that match the new iterative output.

#### `tests/test_proof_tree.rs`

* The proof tree is now built during `solve` itself, so `solve_and_get_tree`
  can use `Search::solve` with a proof-tree sender.
* Remove `solve_and_extract_ppv`; replace with a helper that returns the
  `solve` PV and validates it with `tree.validate_ppv`.
* `proof_tree_contains_defender_replies` should still pass: `dfpn` still
  evaluates every child of an AND node and emits each proven child.
* Round-trip test: load `proof_tree.bin` and validate the `solve` PV in the
  loaded tree.

#### Other tests

* `tests/test_review.rs` uses `search_depth` multiple times on the same
  `Search`; ensure `search_depth` resets enough state but does not break
  between calls.
* Run `cargo test` and `cargo clippy` to catch remaining references to the
  removed `find_ppv`, `refine_sppv`, `ProofMode`, `ppv_cache`, etc.

### 7. Documentation updates

* `AGENTS.md`: update the `search/dfpn/` and `main.rs` descriptions to remove
  PPV/SPPV and describe the iterative bounded search and proof-tree emission.
* `docs/plans/storage/concept.md`: replace references to `find_ppv` /
  `refine_sppv` as the source of proof-tree events with "events are emitted by
  `dfpn` during the iterative bounded search".
* Update any README/help text that mentions `--no-refine-shortest` or the
  old staged output.

## Proof tree semantics

The proof tree is a record of every node that `dfpn` proved or disproved
during the search.  Because the transposition table is reused across bounded
iterations, the tree may contain branches from earlier, looser bounds that are
longer than the final shortest line.  Those branches are still valid proofs,
but they are not minimal.

The authoritative shortest line is the PV returned by `Search::solve`.  The
pre-exit hook should validate that PV against the tree (`tree.validate_ppv`)
and with the board (`Search::validate_pv`), not extract a fresh PPV from the
tree.

If `ProofTree::extract_ppv` is kept as a diagnostic, it may return a longer
line than the returned PV.  That is expected and not a bug.

## Test plan

1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test --lib`
4. `cargo test`
5. `cargo doc --no-deps`
6. Manual CLI checks:
   * `cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"` should
     log decreasing lengths and end with `pv: f1f7 e8d8 g1g8` and
     `ppv_valid: true`.
   * `cargo run --release -- --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26"`
     should log length 7 and then shorter lengths if found, ending with a
     valid `proof_tree.bin`.
   * `cargo run --release -- --first-outcome --fen "..."` should stop after
     the first outcome and still write `proof_tree.bin`.
   * Press `q` + Enter to trigger the pre-exit hook and dump.
7. Verify the binary dump with `ProofTree::from_bin` and
   `loaded.validate_ppv(&solve_pv)`.
8. `cargo run --release --example verify_ppv -- --fen "..." --moves "..."`
   should still report `is_ppv: true` for known PPV lines.

## Risks and mitigations

* **Parity assumption.** If the shortest decisive `Win` could be an even
  number of plies or `Loss` an odd number, the `N - 2` loop would skip it.
  The current atomic rules preserve the fixed parity described above.  If this
  ever changes, switch to `N - 1`.
* **Proof tree may contain longer branches.** This is by design: the tree is
  a collection of all proven/disproven nodes, not a minimal shortest-line
  tree.  The returned PV is the authoritative shortest line.
* **Pre-exit hook output.** `proof_tree_ppv` should now be the `solve` PV,
  not `tree.extract_ppv()`.  If external consumers expect the dump to encode a
  single minimal PPV, they must compute it themselves from the adjacency list.
* **Performance of emission.** Maintaining `proof_path` and `move_stack` for
  every recursive call adds overhead.  When `proof_tree_sender` is `None`, the
  `emit` flag is false and the overhead is a single field read and an early
  return.  With a sender, the event volume is bounded by the number of proven
  nodes; the worker's incremental memory accounting handles large trees.
* **Work chunks in bounded search.** Each bounded search uses the same
  chunk-doubling loop as the first outcome search.  This keeps the search
  responsive to the global deadline and lets the transposition table warm up
  between attempts.
* **CLI output changes.** Removing `--no-refine-shortest` and changing the
  output format may break external scripts.  Document the new format in
  `AGENTS.md` and the help text.

## Final task

After implementation, create `docs/plans/storage/report6.md` summarizing the
changes, the problems encountered, any deviations from this plan, unresolved
parts, and next steps.
