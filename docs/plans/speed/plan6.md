# Plan 6: Replace `HashSet<u64>` repetition path with a stack + linear search

## Start

Read `docs/plans/speed/analysis.md`.  Inspect `src/search/dfpn/mod.rs`,
`src/search/dfpn/core.rs` and `src/search/dfpn/simulate.rs` to see how
`self.path` and `self.path_stack` are used for repetition detection and
simulation.

## Goal

Reduce the overhead of the repetition-detection path set on every make/unmake.

## Background

The search keeps two copies of the current path: a `HashSet<u64>` for fast
membership and a `Vec<u64>` for the stack order:

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn/mod.rs" lines="32-33" /> <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/core.rs" lines="84-85" /> <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/core.rs" lines="263-264" />

For the relatively short paths typical of atomic chess, a single `Vec<u64>`
with linear search has better locality and avoids the hashing overhead of the
separate `HashSet`.  The two structures are also redundant: the stack already
contains every key in the path.

## Implementation tasks

1. Remove the `path: HashSet<u64>` field from `Search`.
2. Use `path_stack: Vec<u64>` as the single authoritative path.  Add helper
   methods:
   ```rust
   fn path_contains(&self, key: u64) -> bool { self.path_stack.contains(&key) }
   fn path_push(&mut self, key: u64) { self.path_stack.push(key); }
   fn path_pop(&mut self) { self.path_stack.pop(); }
   ```
3. Replace `if !self.path.insert(rep_key)` with:
   ```rust
   if self.path_stack.contains(&rep_key) {
       return Outcome::Draw;
   }
   self.path_stack.push(rep_key);
   ```
   and remove the separate `self.path_stack.push(rep_key)` that follows.
4. Update `try_use_tt` / `simulate` in `simulate.rs` to use the same stack-based
   path and avoid cloning a `HashSet`.
5. If benchmarks show that linear search becomes a bottleneck on deep paths,
   consider a small `FxHashSet` (a custom `BuildHasherDefault` with a tiny
   FxHasher) as a drop-in replacement.  Start with the `Vec` implementation
   because it is simpler and interacts well with Plan 12.

## File changes

- `src/search/dfpn/mod.rs`
- `src/search/dfpn/core.rs`
- `src/search/dfpn/simulate.rs`
- `src/search/dfpn/children.rs` (if it queries `path.contains` directly)

## Risks

- `Vec::contains` is O(depth) rather than O(1).  On very deep paths it could lose
to a hash set, so this change must be measured.
- `simulate.rs` currently clones `self.path`; removing `path` changes the clone
  site.  Plan 12 overlaps here; coordinate the two plans or do them together.
- `path_random` uses `self.path_stack.len()` as the depth index; keeping the
  stack semantics exactly the same is critical.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
$ cargo run --release --example play_and_solve
```

All repetition and GHI tests must still pass.

## Final task

Write `docs/plans/speed/report6.md` with timing or node-count results on a few
positions and a note about whether `Vec` search remained competitive on deep
paths.
