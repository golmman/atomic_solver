# Plan 2: Improve solver testability

## Start

1. Read `AGENTS.md`, the `Plan 1` report (once it exists), and this plan.
2. Identify the modules where behaviour is hardest to exercise in isolation:
   `main.rs` (CLI), `src/search/dfpn/selection.rs`,
   `src/search/dfpn/history.rs`, `src/search/dfpn/pv.rs`,
   `src/position.rs`, `src/search/tt/table.rs` and `src/proof_tree/mod.rs`.

## Goal

Refactor the source so that more behaviour can be unit-tested directly,
without spawning processes or constructing full `Search`/`ProofTreeWorker`
instances, while keeping the hot solving path fast and the public API
unchanged except for small, test-only additions.

## Background

* `src/main.rs` mixes argument parsing, process exit, stdin handling, and
  solver output in one `main()` function. It cannot be unit-tested.
  <ref_snippet file="/workspace/atomic_solver/src/main.rs" lines="96-213" />
* Several pure helpers in `src/search/dfpn/selection.rs` are defined inside
  `impl Search` even though they do not use `&self`. They can only be called as
  associated functions (`Search::is_solved_by_children`) and `select_from_children`
  still requires a `&Search` receiver.
  <ref_file file="/workspace/atomic_solver/src/search/dfpn/selection.rs" />
* `src/search/dfpn/history.rs` history/killer logic is tightly bound to the
  `Search` struct. The `[[[i32; 64]; 64]; 2]` and `[[Move; 2]; 256]` arrays cannot
  be tested without a full `Search`.
  <ref_file file="/workspace/atomic_solver/src/search/dfpn/history.rs" />
* `Position` exposes `pub board: Board` directly, letting callers mutate it or
  depend on its internals instead of using the `Position` API.
  <ref_snippet file="/workspace/atomic_solver/src/position.rs" lines="55-59" />
* `Search::log_chunk` writes directly to `stderr` with `eprintln!`, so tests of
  the chunk-loop output must spawn the binary.
  <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/mod.rs" lines="408-420" />
* `TranspositionTable::with_mb` always allocates at least 32 entries, making it
  awkward to test bucket collisions and eviction deterministically.
  <ref_file file="/workspace/atomic_solver/src/search/tt/table.rs" />
* `ProofTreeWorker` can only be created through `spawn`, which starts a thread.
  <ref_snippet file="/workspace/atomic_solver/src/proof_tree/mod.rs" lines="202-221" />

## Implementation tasks

### 1. Refactor CLI parsing into a testable module

1.1 Create `src/cli.rs` that contains only argument parsing and the
`CliOptions` struct. It must not depend on any solver code so it can be
compiled into both the library and the binary.

1.2 Add `mod cli;` (private) to `src/lib.rs` and `mod cli;` to `src/main.rs`.
The library gets `cli` for unit tests; the binary gets a local copy for
`main()` to call. Keep all solver/output logic in `src/main.rs`.

1.3 Add a `#[cfg(test)] mod tests` block in `src/cli.rs` covering:

* defaults (FEN, TT size, epsilon, timeout, proof-tree size, dump path),
* `--fen`, `--tt-size`, `--epsilon`, `--timeout`, `--pt-size`, `--dump-path`,
  `--first-outcome`, `--outcome-only`,
* missing values and unknown options return `Err`,
* out-of-range `--epsilon` and non-positive `--tt-size`/`--timeout` return
  `Err`,
* `--help` and `-h`.

1.4 Keep `src/main.rs` thin: read `std::env::args()`, call `cli::parse_args()`,
print errors and exit, otherwise run the solver with the parsed options.

### 2. Decouple pure DF-PN selection logic from `Search`

2.1 In `src/search/dfpn/selection.rs`, move the following from `impl Search` to
free functions in the same module:

* `is_solved_by_children`
* `select_child_with_early_exit`
* `best_and_second_unsolved`
* `selection_for_child`
* `second_best_unsolved_excluding`
* `select_from_children` (remove the unused `&self` receiver)

2.2 Update `src/search/dfpn/children.rs` and `src/search/dfpn/core.rs` to call
the free functions directly. Keep the existing public `Search` API unchanged.

2.3 Update the existing unit tests in `selection.rs` to call the free functions
instead of `Search::...`. This proves they can be tested with a `Vec<ChildInfo>`
without a `Search` instance.

2.4 Move `src/search/dfpn/pv.rs` `validate_pv` and `validate_pv_prefix` to free
functions (still `pub`/`pub(super)`). Keep `Search::validate_pv` as a thin
wrapper so existing callers and tests keep working. This lets tests validate PVs
without creating a `Search`.

### 3. Make history/killer logic testable in isolation

3.1 In `src/search/dfpn/history.rs`, extract pure helpers that operate on the
raw arrays:

```rust
fn update_history_entry(
    history: &mut [[[i32; 64]; 64]; 2],
    side: Color,
    from: Square,
    to: Square,
);

fn update_killer_slots(
    killers: &mut [[Move; KILLER_SLOTS]; MAX_KILLER_DEPTH],
    depth: usize,
    m: Move,
);

fn age_history(history: &mut [[[i32; 64]; 64]; 2]);
fn killer_bonus(
    killers: &[[Move; KILLER_SLOTS]; MAX_KILLER_DEPTH],
    m: Move,
    depth: usize,
) -> i32;
```

3.2 Keep `impl Search` methods as thin wrappers that call the helpers with
`self.history`/`self.killers`.

3.3 Add unit tests that exercise the helpers with local arrays, verifying caps,
aging, slot shifting and bonus values.

### 4. Encapsulate `Position` and add helpers

4.1 Change `Position::board` from a public field to private and add:

* `pub fn board(&self) -> &Board`
* `pub(crate) fn board_mut(&mut self) -> &mut Board` (only if still needed)
* `pub fn populate_state(&self, state: &mut StateInfo)`

4.2 Update `src/search/dfpn/history.rs` and the examples that currently access
`pos.board` directly (`examples/static_move_scores.rs`, any others) to use the
new accessors.

4.3 Add `pub fn legal_moves_vec(&self) -> Vec<Move>` as a convenience for tests
and examples that currently build a `MoveList` and then iterate.

4.4 Add a `#[cfg(test)]` or `pub(crate)` `Position::try_do_move(&mut self, m: Move) -> bool`
that returns `false` for illegal moves instead of panicking, making fuzz/property
style tests safer.

### 5. Make `Search` output and state inspectable

5.1 Add an optional `log_writer: Option<Box<dyn std::io::Write + Send>>` field
to `Search` and a setter:

```rust
pub fn set_log_writer(&mut self, writer: Option<Box<dyn std::io::Write + Send>>);
```

5.2 Change `Search::log_chunk` to write to `log_writer` if set, otherwise fall
back to `eprintln!`. This lets unit tests capture chunk progress without
spawning the binary.

5.3 Add `pub fn child_evaluations(&self) -> u64` already exists; make sure
`nodes()`, `exit_reason()` and `child_evaluations()` are public and documented
so tests can assert on search effort.

5.4 (Optional, only if needed by tests) Add a `pub(crate)` `Search::transposition_table(&self) -> &TranspositionTable`
accessor for tests in the same crate to inspect stored results.

### 6. Improve `TranspositionTable` testability

6.1 Add a `pub(crate)` constructor for tests:

```rust
impl TranspositionTable {
    pub(crate) fn with_capacity(buckets: usize) -> Self {
        // buckets must be a power of two; adjust with next_power_of_two if needed
    }
}
```

6.2 Keep `with_mb` for production. `with_capacity` makes eviction and bucket
collision tests deterministic.

6.3 Add `pub(crate) fn bucket_count(&self) -> usize` already exists; ensure it
remains usable from unit tests.

### 7. Make `ProofTreeWorker` testable without a thread

7.1 Add a `pub(crate)` constructor and single-message handler:

```rust
impl ProofTreeWorker {
    pub(crate) fn new(
        root_fen: String,
        budget: usize,
        memory_limited: Arc<AtomicBool>,
    ) -> Self;

    pub(crate) fn handle_message(&mut self, msg: ProofMessage) -> Option<ProofResponse>;
}
```

7.2 Refactor `ProofTreeWorker::spawn` to call `Self::new` and then run
`handle_message` in a loop on the thread.

7.3 Add unit tests for `estimate_memory` thresholds, `process_pending`, and the
`memory_limited` flag without spawning threads.

### 8. Add test fixtures and helper macros

8.1 Extend `tests/common/mod.rs` with deterministic position helpers:

```rust
pub fn assert_position_invariants(pos: &Position);
pub fn assert_solves_to(fen: &str, expected: Outcome);
pub fn assert_solves_with_first_move(fen: &str, expected: Outcome, first: &str);
pub fn assert_pv_valid(fen: &str, expected: Outcome, pv: &[Move]);
```

8.2 Add a small `tests/fixtures/positions.txt` corpus and a reader test so new
regression FENs can be added without writing a new test function.

### 9. Keep the hot path fast

9.1 Do **not** introduce trait objects or dynamic dispatch inside the recursive
`dfpn` loop. Keep `scorer: StaticAtomicScorer` concrete and `tt` as a direct
`TranspositionTable`.

9.2 Do **not** replace `Instant::now()` with a virtual clock in the recursive
loop. `time_exceeded` is already testable by setting `timeout(0)` and sleeping
very briefly.

9.3 Ensure all new `pub`/`pub(crate)` test helpers are `#[inline]`-friendly and
prefer free functions with explicit parameters over `&self` methods that do not
need `self`.

## File changes

* `src/cli.rs` (new)
* `src/main.rs`
* `src/lib.rs`
* `src/search/dfpn/selection.rs`
* `src/search/dfpn/children.rs`
* `src/search/dfpn/core.rs`
* `src/search/dfpn/history.rs`
* `src/search/dfpn/pv.rs`
* `src/search/dfpn/mod.rs`
* `src/position.rs`
* `src/search/tt/table.rs`
* `src/proof_tree/mod.rs`
* `tests/common/mod.rs`
* `tests/fixtures/positions.txt` (new)
* `examples/static_move_scores.rs`

## Risks

* Moving helpers to free functions changes call sites; a single missed update
  will not compile, so the compiler will catch mistakes.
* Making `Position::board` private may break examples and `MoveScorer` callers;
  the accessor replacement must be applied everywhere before tests pass.
* Adding `Box<dyn Write>` to `Search` is safe because it is only used in
  `log_chunk`, which is outside the recursive hot loop.
* New `pub(crate)` constructors must not be used in production code; keep them
  `pub(crate)` or `#[cfg(test)]` to avoid leaking implementation details.

## Verification

```bash
cargo fmt
cargo clippy --all-targets
cargo test
cargo test --release -- --ignored
cargo test --all-targets
cargo run --bin atomic_solver -- --help
cargo run --example static_move_scores -- --fen "4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19"
```

New unit tests to verify after each refactor:

* `src/cli.rs` parse tests,
* `src/search/dfpn/selection.rs` free-function tests,
* `src/search/dfpn/history.rs` pure helper tests,
* `src/search/dfpn/pv.rs` free `validate_pv` tests,
* `src/search/tt/table.rs` `with_capacity` and eviction tests,
* `src/proof_tree/mod.rs` `ProofTreeWorker` non-thread tests.

## Final task

Write `docs/plans/testability/report2.md` documenting:

* which units were decoupled and how they are now tested,
* the new test-only API (`cli::parse_args`, `TranspositionTable::with_capacity`,
  `ProofTreeWorker::new`, `Search::set_log_writer`, etc.),
* any public API changes and the rationale,
* hot-path performance safeguards,
* remaining tightly-coupled code that could not be refactored safely.
