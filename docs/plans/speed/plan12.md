# Plan 12: Reduce Kawano-simulation overhead

## Start

Read `docs/plans/speed/analysis.md` and inspect `src/search/dfpn/simulate.rs`.
Confirm how `try_use_tt` prepares the simulation state and how `simulate`
recurses.

## Goal

Remove the per-twin clones of the repetition-detection path structures.

## Background

When `try_use_tt` considers reusing a twin proven on a different path, it
clones the current search path before running Kawano-style verification:

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn/simulate.rs" lines="71-113" />

The clones are a `HashSet<u64>` and a `Vec<u64>`.  If Plan 6 has already
replaced the `HashSet` with a `Vec`, the clone is still a copy of the path
stack.  Because `simulate` pushes and pops symmetrically, we can lend it the
real stack and have it roll back to the previous length instead of copying.

## Implementation tasks

1. Coordinate with Plan 6: the search path should be a single `Vec<u64>` used
   as both a stack and a repetition detector (`path.contains`).  If Plan 6 is
   not done first, do the two plans together.
2. Change `simulate` to take a mutable borrow of the path stack:
   ```rust
   pub(super) fn simulate(
       &self,
       pos: &mut Position,
       path_code: u64,
       path_length: u32,
       expected: Outcome,
       best_move: Move,
       path: &mut Vec<u64>,
       sim_nodes: &mut u64,
       remaining_depth: usize,
   ) -> bool
   ```
3. Record `path.len()` at the start of `simulate`, push the current
   `repetition_key`, recurse, and then `path.truncate(original_len)` before
   returning.  This replaces the explicit `sim_path.insert/remove` and
   `sim_stack.push/pop` pairs.
4. Use `Vec::contains` for repetition detection inside `simulate`.  The
   simulation depth is bounded by `SIM_MAX_DEPTH` and `SIM_MAX_NODES`, so the
   O(depth) scan is acceptable.
5. Update `try_use_tt` to call `simulate` with `&mut self.path_stack` instead of
   cloned structures.
6. If `simulate` is called recursively from itself, ensure each call still gets
   the same shared `path` so that nested repetitions are detected across the
   whole simulation.

## File changes

- `src/search/dfpn/simulate.rs`
- `src/search/dfpn/mod.rs` (path-stack field, if Plan 6 changes are needed)
- `src/search/dfpn/core.rs` (if the path-stack field is renamed)

## Risks

- Any mistake in truncating the path stack leaks items or removes too many,
  corrupting the main search path.  The `original_len` save/restore must be
  exactly at the function entry/exit, including all early-return paths.
- If `simulate` is interrupted by `sim_nodes >= SIM_MAX_NODES`, it returns
  `false` but must still restore the path stack.
- Plan 6 and Plan 12 touch the same data structure; do them together or finish
  Plan 6 first to avoid conflicts.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo run --release --example twin_stats
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
```

The `twin_stats` output should still be consistent, and all GHI/repetition
regression tests must pass.

## Final task

Write `docs/plans/speed/report12.md` with node-count and timing results on
positions that exercise twin reuse, and confirm that `twin_insertions` and
`twin_evictions` are unchanged.
