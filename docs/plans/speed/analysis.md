# Speed-analysis of the atomic-chess solver

This document collects the most promising ways to make the DF-PN solver faster,
ordered from the lowest implementation effort to the highest.  The speed impact
estimates are qualitative; they have not been benchmarked on a fixed suite.

## 1. Release-profile build options — trivial effort, moderate impact

`Cargo.toml` currently has no `[profile.release]` section.  Enabling `lto`,
`codegen-units = 1`, `panic = "abort"` and optionally `target-cpu = "native"`
can give an immediate 10-40% speed-up for the shipped binary, and is the single
cheapest change to apply. <ref_file file="/workspace/atomic_solver/Cargo.toml" />

## 2. Remove `Box<dyn MoveScorer>` virtual dispatch — trivial effort, small-moderate impact

`Search` stores `scorer: Box<dyn MoveScorer>`, so `sort_moves` pays a vtable
call for every move comparison.  Replacing it with a concrete `StaticAtomicScorer`
field or a generic `S: MoveScorer` parameter removes that overhead from the hot
sorting loop. <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/mod.rs" lines="39" />

## 3. Integerize `epsilon_ceil` — trivial effort, tiny-small impact

The threshold update in the main DF-PN loop currently does `f64` multiplication
and `ceil` on every child expansion.  Precomputing the `epsilon` ratio as a
fraction and using integer arithmetic removes a floating-point operation from a
very hot path. <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/core.rs" lines="270-276" />

## 4. Avoid the double `tt.probe` in `dfpn` — trivial effort, small impact

`core.rs` probes the same `tt_key` twice in a row: once for `try_use_tt` and once
to fetch the best move if the entry could not be reused.  Storing the first
entry in a local variable eliminates the second probe. <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/core.rs" lines="77-102" />

## 5. Cache per-node move scores — low effort, large impact

`sort_moves` calls `scorer.score` inside the sort comparator, which means a
move is scored O(N log N) times per node.  `StaticAtomicScorer::score` is not
cheap: it recomputes piece-attack bitboards, commoner-distance loops and blast
expansions every time.  Score each move once, store the result, and sort by the
cached value. <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/history.rs" lines="26-34" /> <ref_snippet file="/workspace/atomic_solver/src/search/ordering.rs" lines="79-175" />

## 6. Replace `HashSet<u64>` repetition path with a stack + linear search — low effort, small-moderate impact

The search path is maintained in a `HashSet<u64>` that is inserted into and
removed from on every make/unmake.  For the relatively short paths typical of
atomic chess, a `Vec<u64>` with a small linear search has better locality and
avoids hashing overhead. <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/mod.rs" lines="32-33" /> <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/core.rs" lines="84-85" /> <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/core.rs" lines="263-264" />

## 7. Stop copying `TtEntry` on every probe — low-medium effort, large impact

`TtEntry` contains eight `TwinEntry` slots and is `Copy`; callers such as
`core.rs` and `children.rs` use `.copied()` on every `tt.probe`, copying well
over 200 bytes per lookup.  Refactoring `probe` to return a small summary struct
or returning a reference and reading only the fields that are needed would cut
memory bandwidth in the transposition-table hot path.
<ref_snippet file="/workspace/atomic_solver/src/search/tt/entry.rs" lines="55-54" /> <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/core.rs" lines="77" /> <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/children.rs" lines="140" />

## 8. Improve and tune move ordering — low-medium effort, moderate-large impact

The history table uses a flat `+100` bonus and a fixed aging interval, and the
static scorer is the only source of move ordering.  Adding depth-scaled history,
counter-move, follow-up and possibly static-exchange-evaluation capture ordering
can shrink the search tree by bringing winning lines to the front.  Better
ordering is the cheapest way to reduce node count.
<ref_snippet file="/workspace/atomic_solver/src/search/dfpn/history.rs" lines="10-15" /> <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/history.rs" lines="47-67" /> <ref_snippet file="/workspace/atomic_solver/src/search/ordering.rs" lines="12-34" />

## 9. Incremental Zobrist hash update in `Position` — medium effort, large impact

`do_move` and `undo_move` recompute the full position hash from the board after
every make/unmake.  Because a move and its undo are called once for every child
evaluation and every child expansion, incrementally XORing the changed pieces,
side-to-move and rule50 key removes a full `board.hash()` and key lookup per
edge. <ref_snippet file="/workspace/atomic_solver/src/position.rs" lines="67-78" /> <ref_snippet file="/workspace/atomic_solver/src/zobrist.rs" lines="101-115" />

## 10. Use a TT generation counter instead of `tt.clear()` — medium effort, moderate-large impact

The bootstrap and refinement loops call `tt.clear()` repeatedly, zeroing the
table and discarding useful entries.  Adding a `generation` field to `TtEntry`
and skipping stale entries avoids memory writes and preserves work between
iterative-deepening probes. <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/mod.rs" lines="125-138" /> <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/mod.rs" lines="214-281" />

## 11. Cache `ChildInfo` across `select_children` iterations — medium effort, moderate impact

`select_children` evaluates every child from scratch after each child expansion.
Keeping a per-node `Vec<ChildInfo>` and updating only the child that was just
expanded would avoid repeated `do_move`/`undo_move`, outcome generation and
TT probes for siblings. <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/children.rs" lines="34-105" />

## 12. Reduce Kawano-simulation overhead — medium effort, moderate impact

`try_use_tt` clones `self.path` (`HashSet`) and `self.path_stack` (`Vec`) before
calling `simulate` for a twin from another path.  Running the simulation with a
borrowed, rollback-capable path stack would remove allocations and copies from
the twin-reuse path. <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/simulate.rs" lines="71-113" />

## Higher-effort follow-ups (not planned here)

- **Parallel DF-PN search** (root split or shared TT with worker threads) —
  high effort, large impact, the only change that can scale with core count.
- **Lazy / staged move generation** or additional forward pruning — high effort,
  moderate-large impact, but correctness with DF-PN bounds is non-trivial.

## Recommended short-term order

The biggest payoffs for the least work are items **5** (cached move scores),
**7** (avoid `TtEntry` copies), **9** (incremental Zobrist), and **1** (release
build flags).  Items 2, 3, 4, 6 and 8 are small, safe clean-ups that can be
stacked on the same pass.  Items 10, 11 and 12 require more care but still fit
inside the current single-threaded design.  Parallel search should be tackled
only after the sequential speed ceiling is reached.
