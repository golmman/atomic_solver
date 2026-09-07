# Lean Plan 2 — Hot-path compute: movegen dedup + allocation removal + scorer cost

Implements items **3, 6, and 8** from `docs/plans/lean/initiative.md`
(one shared profiling pass, three independent work packages). Everything
in this plan is **behavior-preserving**: identical outcomes, identical
move ordering, identical `child_evals` per position. The proof that this
is achievable is mechanical — every change either removes a recomputation
of a value that is already in scope, reorders pure reads, or replaces a
heap allocation with a reused buffer. The bit-identical drift check is
therefore the primary correctness gate, run after *every* step.

## Context

The hot path is `dfpn` (`src/search/dfpn/core.rs`) → `evaluate_child`
(`src/search/dfpn/children.rs`) and `sort_moves`
(`src/search/dfpn/history.rs`). Measured redundancy, with references:

1. **Child positions are generated twice.** `evaluate_child` calls
   `pos.outcome()` (children.rs:89) for the terminal check;
   `Position::outcome` (position.rs:186-191) runs a full
   `legal_moves_with_state`. When the child is then actually searched,
   the child's `dfpn` entry regenerates exactly the same `MoveList` +
   `StateInfo` (core.rs:61-63). That is two full legal-move generations
   per searched child. `evaluate_child` is called for every child of
   every re-expansion, so this dominates the node-level cost after move
   ordering.
2. **`sort_moves` rebuilds the node's `StateInfo`.** `dfpn` already
   produced it via `legal_moves_with_state` (core.rs:61-63), but
   `sort_moves` calls `pos.populate_state` again (history.rs:80-82).
   `populate_state` (atomic-movegen board.rs:770-775) is not free: it
   computes `checkers` and `pinned` (attack scans) plus two bitboard
   counts — once per `dfpn` entry, again per sort.
3. **`score_with_map` recomputes per-node invariants per move.** For
   every quiet move it re-derives `board.commoners(them)` several times
   (ordering.rs:181, 258, 267, 278), the lone-commoner square
   (ordering.rs:244-250), and back-rank masks (ordering.rs:338-349).
   All of these are constant across the sort of one node.
4. **Per-node heap allocations on the recursion path.** A fresh
   `Vec<ChildInfo>` per `dfpn` frame (core.rs:143, capacity ≈ branching
   factor), a fresh `Vec<(Move, i32)>` per `sort_moves` call
   (history.rs:90), and one `move_stack.clone()` per emitted proof event
   (children.rs:182, mod.rs:349).

Sizing facts that drive the design:

- `MoveList` is `[Move; 256] + len` (atomic-movegen types.rs:736-754),
  i.e. ~1 KiB by value; `StateInfo` is a few `Bitboard`s and ints
  (board.rs:88-100). A boxed pair is ~1 KiB of heap per child —
  negligible against the 64 MB default TT, but it means "cache every
  child's move list inline in `ChildInfo`" is a non-starter; a boxed
  optional is the right shape.
- `atomic-movegen` is an **external crates.io dependency** (2.1.0), not
  a workspace crate. A short-circuiting `has_legal_move` fast path for
  the terminal check would require an upstream release; this plan stays
  inside `atomic_solver` and instead *reuses* the generation it already
  must pay for.
- `perf` is available in the container (`/usr/bin/perf`) for the
  profiling pass.
- `ChildInfo` is currently ~40 bytes; adding an 8-byte `Option<Box<…>>`
  keeps it in one or two cache lines.

## Decisions

1. **Dedup by passing the generated child movegen down, not by avoiding
   it.** `evaluate_child` must generate the child's legal moves anyway
   for the stalemate/checkmate half of the terminal check (the other
   halves — commoner extinction, rule50, two-piece — are cheap bitboard
   checks). The generated `MoveList + StateInfo` is therefore stored in
   the child's `ChildInfo` as a boxed precompute and consumed by the
   child's `dfpn` entry when the search recurses into it. Per searched
   child: two generations → one.
2. **Terminal-check reorder in `evaluate_child`, with an explicit
   equivalence argument.** Current order: full movegen (`pos.outcome`)
   → repetition → TT. New order: cheap extinction checks → repetition →
   TT-resolved probe → movegen (only if still undecided) → remaining
   terminal checks. This is result-equivalent because:
   - Commoner extinction strictly precedes the moves-empty branch in
     `outcome_from_state` (position.rs:200-205), so checking it early
     cannot change any result. `rule50` and `occupied == 2` must **not**
     be hoisted above the moves-empty branch (a rule50 checkmate is a
     Loss, position.rs test `fifty_move_checkmate_is_loss`), so they
     stay inside `outcome_from_state`.
   - The repetition check keeps its place before any TT use, preserving
     the first-player-loss GHI shortcut.
   - A TT-resolved hit for a child that is also terminal is identical to
     the terminal path: `dfpn` stores terminal nodes with `depth 0`,
     `remaining_depth = u32::MAX` (core.rs:67-77), and terminality is
     position-static (the same hash always maps to the same terminal
     result), so `resolved_from_entry` returns the same outcome and
     depth the movegen path would have produced.
   - Unsolved TT bounds are only consulted *after* the movegen-based
     terminal check says non-terminal, exactly like the current code
     where the outcome check precedes the TT probe.
   The GHI/repetition/transposition suites plus the drift check verify
   this empirically.
3. **All allocations that can be pooled, are pooled on `Search`.**
   - `Vec<ChildInfo>` per `dfpn` frame → a per-depth pool of vectors on
     `Search`, borrowed with `mem::take` at frame start and returned at
     frame end (the frame owns the vec across the recursive call, so a
     plain `&mut` borrow cannot span the recursion; `mem::take` ends the
     borrow cleanly). Depth index = `path_stack.len()` after the frame's
     `path_push`, which is unique among active frames.
   - `Vec<(Move, i32)>` in `sort_moves` → a single reusable scratch
     buffer on `Search` (`sort_moves` becomes `&mut self`; it completes
     before any recursion, so one buffer suffices).
   - `move_stack.clone()` per proof event is **protocol-bound**
     (`NodeProven` carries an owned `Vec<Move>` into another thread) and
     only fires for non-Draw proven nodes with an attached sender. It is
     measured in the profiling pass and otherwise left alone.
4. **Scorer work beyond hoisting is profile-gated.** Hoisting the
   per-node invariants (decision in step 3a below) is a pure refactor
   and always in scope. Anything deeper (e.g. replacing
   `board.attackers_to` in the threat downgrade, incremental attack
   maps) changes the computation and is only attempted if the profile
   shows `score_with_map` above ~10% of search time after steps a–c.
5. **No changes to** move ordering *results*, DF-PN threshold math,
   proof events, the TT store/replacement policy, `docs/spec/`, or the
   CLI. `benchmark --refine-cap` stays out (plan1 deferred it until the
   optimizer needs it).
6. Each work package is measured **stepwise** (quick-suite `child_evals`
   sum + m22_white wall) so report2 can attribute the win per package,
   not just in aggregate.

## Implementation steps

### Step 0 — Baseline + profile (do this first, before touching `src/`)

Release build, then:

```sh
# deterministic baselines
target/release/examples/benchmark --suite quick --json --first-outcome \
  > docs/plans/lean/measurements/plan2/quick_before.json
target/release/atomic_solver --fen "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22" \
  --timeout 20 --first-outcome --outcome-only \
  --dump-path /tmp/opencode/lean2_before.bin 2> docs/plans/lean/measurements/plan2/m22_before.log

# profile (m22 first-outcome phase, ~10 s of pure search)
perf record -g -o /tmp/opencode/lean2_perf.data -- target/release/atomic_solver \
  --fen "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22" \
  --timeout 20 --first-outcome --outcome-only
perf report -i /tmp/opencode/lean2_perf.data --stdio \
  > docs/plans/lean/measurements/plan2/perf_before.txt
```

Extract from the profile: % time in `generate_legal_with_state` /
`generate_legal`, `populate_state` (`compute_checkers`/`compute_pinned`),
`score_with_map`, `evaluate_child`, malloc/free symbols, and
`emit_proof_node`. Record the table in report2; it decides whether step
3d fires and how the packages are ordered. Keep the m22 baseline
`child_evals`/`nodes` numbers (stderr log) — they must match after.

### Step a — Pass the node `StateInfo` into `sort_moves` + hoist scorer invariants

1. `sort_moves` signature (history.rs:73):
   `fn sort_moves(&mut self, pos: &Position, state: &StateInfo, moves:
   &mut MoveList, best_from_tt: Move, is_or_node: bool)` — drop the
   `populate_state` call (history.rs:80-82); the caller at core.rs:117
   passes the state it already built at core.rs:62-63. The values are
   identical by construction (`legal_moves_with_state` =
   `populate_state` + `generate_legal_with_state`, position.rs:162-165).
2. Add a `pub(crate) struct ScoreContext` in `src/search/ordering.rs`
   holding the per-node invariants: `us`/`them`, `them_commoners`
   bitboard, `them_commoners_count`, `lone_commoner: Option<Square>`,
   `nearest: [i8; 64]`. Build it once per `sort_moves`; add an internal
   `score_with_context(&self, board, m, state, ctx, is_or_node)` and
   make `score_with_map` build a context and delegate, so the public
   signature used by examples/tests (`static_move_scores`,
   `move_order_debug`, `MoveScorer::score`) is unchanged.
3. `score_with_context` replaces the per-move recomputations of
   `board.commoners(them)`, the lone-commoner pop, and back-rank masks
   with context reads. Integer arithmetic and evaluation order per move
   stay exactly the same — same values, same operations, same order.

Tests: the existing `history.rs` sort tests and `ordering/tests.rs`
must pass unchanged (update call sites for the new signature; make the
non-mut `Search` bindings in those tests `mut`).

### Step b — Movegen dedup: child precompute consumed by `dfpn`

1. New type in `children.rs`:

   ```rust
   pub(super) struct ChildPrecompute {
       moves: MoveList,
       state: StateInfo,
   }
   ```

   `ChildInfo` gains `pub(super) precompute: Option<Box<ChildPrecompute>>`
   (dummy children from the winning-child early stop set `None`).
2. Restructure `evaluate_child` per decision 2:

   ```
   child_evals += 1; do_move;
   if commoners(us).is_empty() -> Loss info (no movegen, no TT)
   if commoners(them).is_empty() -> Win info
   if path_contains(child_rep_key) -> Draw info (repetition_seen)
   probe TT once (copied entry, as today)
   if resolved_from_entry(...) && one-ply guard passes -> resolved info (no movegen)
   // still undecided: generate once
   let (moves, state) = legal_moves_with_state
   if let Some(o) = outcome_from_state(&state, &moves) -> terminal info
   else -> unsolved-bounds branch from the probed entry (unchanged logic)
   precompute = Some(Box::new(...)) whenever we generated
   proof-event emission (unchanged); undo_move; return
   ```

   `Position::outcome` keeps its other callers; `evaluate_child` stops
   using it.
3. `dfpn` gains a parameter `precomputed: Option<ChildPrecompute>`
   (owned, `pub(super)` visibility unchanged). At the top:

   ```rust
   let (mut moves, state) = match precomputed {
       Some(p) => (p.moves, p.state),
       None => { /* existing MoveList::new + legal_moves_with_state */ }
   };
   ```

   Everything downstream (terminal check, `sort_moves`, recursion) uses
   these. The recursion at core.rs:234-245 passes
   `children[idx].precompute.take().map(|b| *b)` for
   `idx = selection.best_child_index` (pass `None` when the index is
   absent or the box was already consumed — `dfpn` regenerates in that
   case, so correctness never depends on the cache).
4. `bounded_search` (mod.rs:416) calls `dfpn(..., None)`.
5. `select_from_children`, thresholds, stores, and history updates are
   untouched.

Tests to add in `children.rs`/`tests.rs`:

- `evaluate_child_caches_precompute_for_searchable_child` — a
  non-terminal, TT-less child has `precompute.is_some()` with the same
  move list the position generates; a TT-resolved child and a terminal
  child have `precompute.is_none()`.
- `dfpn_recursion_consumes_precompute` (behavioral): solving a small
  decisive position twice in a row (fresh `Search` each) yields
  identical `child_evals`; the pooled/`mem::take` plumbing in step c
  reuses the same assertion.

### Step c — Allocation removal

1. **Children pool** (`src/search/dfpn/mod.rs`): field
   `child_pool: Vec<Vec<ChildInfo>>`. In `dfpn`, immediately before the
   iteration loop (core.rs:146, after `path_push`):

   ```rust
   let depth = self.path_stack.len();
   if self.child_pool.len() <= depth { self.child_pool.resize_with(depth + 1, Vec::new); }
   let mut children = std::mem::take(&mut self.child_pool[depth]);
   children.clear();
   ```

   and at the single exit after the loop (before `path_pop`):
   `children.clear(); self.child_pool[depth] = children;` — after the
   TT store so `store_best_child` has been computed. All early returns
   (terminal, leaf, repetition, TT-resolved) happen *before* the take.
   `evaluate_all_children` keeps its signature but fills the caller's
   vec (pass `&mut Vec<ChildInfo>`) instead of returning one.
2. **Sort scratch** (history.rs): field `sort_scratch: Vec<(Move, i32)>`
   on `Search`; `sort_moves` (now `&mut self` from step a) does
   `self.sort_scratch.clear()` and reuses it for the score-then-writeback
   pass. No recursion happens inside `sort_moves`.
3. **Proof-event path clone**: record the profile number; change nothing
   unless it shows > ~2% — the protocol requires an owned `Vec`.

Test: `solve_twice_identical_counts` — solve dec44
(`r7/1Rp4k/4P2B/6p1/3P2P1/p6p/P6K/8 b - - 0 35`, the plan1 fixture)
twice with fresh `Search` objects; assert identical `nodes` and
`child_evals` (catches pool/scratch state leaking between runs).

### Step d — Scorer cost (profile-gated, only if step 0 says ≥ ~10%)

If and only if `score_with_map`/`score_with_context` is still a top
symbol after steps a–c: consider (in order of preference)

1. Limiting the threat-downgrade `board.attackers_to(to, new_occupied)`
   scan to when the threat bonus would actually apply (it already is —
   verify; if the scan runs before the threat test, swap the order —
   pure refactor),
2. Replacing per-move `attacks_from` sliding generation with the
   movegen crate's precomputed attack tables if they are public,
3. Caching `attackers_to` per node.

Any change here must keep scores bit-identical (assert via
`static_move_scores`/`move_order_debug` output on the move-order
fixture positions before/after). If the profile says < ~10%, skip this
step and say so in report2.

### Step e — Fold-in from report1: default-cap non-binding test

Add `src/search/dfpn/tests.rs::default_refine_cap_leaves_improving_rounds`:
solve dec44 with the default factor (0.25) and assert
`refinement_rounds() >= 1` and that at least one round improved the PV
(i.e. the run's final PV length equals the converged 48 plies from
report1) — locking in that the plan1 cap does not bind on the quick
fixtures, so this plan's default-mode A/B measurements measure
movegen/alloc wins, not cap interactions.

### Step f — AGENTS.md

Update the `src/search/dfpn/` bullet: one sentence noting the hot-path
reuse (child movegen computed once in `evaluate_child` and consumed by
the recursive `dfpn` call; node `StateInfo` shared with `sort_moves`;
pooled per-frame child vectors). Note the `ChildInfo` boxed precompute
in the file-size justification list only if `children.rs` crosses 20 KB
(it should not; it is at 12.2 KB and this plan adds ~2 KB).

## Verification

After **each** step (a, b, c; d/f optional):

```sh
cargo fmt --check && cargo clippy --release --all-targets
make test
target/release/examples/benchmark --suite quick --json --first-outcome \
  > /tmp/opencode/lean2_stepX.json
```

`/tmp/opencode/lean2_stepX.json` must be **byte-identical** to
`quick_before.json` in every `child_evals` and `pv_len` field (compare
with `diff`; total across 59 cases before: 38,714,551 — per report1).
A mismatch means a "pure refactor" was not pure: stop, bisect the step.

End-to-end:

```sh
# uncapped A/B (isolates the hot-path win from the plan1 cap)
time target/release/atomic_solver --fen "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22" \
  --timeout 20 --first-outcome --outcome-only --dump-path /tmp/opencode/lean2_after.bin
# default mode (what users run) — outcome + PV must match baseline
time target/release/atomic_solver --fen "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22" \
  --timeout 20 --outcome-only --dump-path /tmp/opencode/lean2_after_default.bin
```

Expected: identical stdout (outcome + PV) as the baseline; wall strictly
below the baseline's ~10.3 s first-outcome number, with the magnitude
set by the stepwise measurements (initiative estimates: #3 ~10–25%, #6
~3–10%, #8 ~5–15% — do not promise a specific figure in advance).
`--dump-path` outputs go to `/tmp/opencode/`, never the repo tree.

Also re-run the GHI/repetition suites explicitly (they exist:
`tests/test_ghi.rs`, `tests/test_repetition.rs`,
`tests/test_transpositions.rs`) — the step b reorder is the risk point.

Because this plan touches the search core, run the full tier once
before closing:

```sh
make test-full   # ~25 min, required for search changes per AGENTS.md
```

## Problems to watch for

- **Borrow checker vs. the children pool**: the frame owns
  `children` across the recursive `dfpn` call, so the pool hand-back
  must use `mem::take`/put-back, not a long-lived `&mut`. Every early
  return added later must not skip the put-back — keep the take/put-back
  pair within one function and avoid `?`/early returns in between.
- **Stale precompute**: a child's precompute is only valid while the
  position is unchanged. It is consumed in the same frame, after only
  do_move/undo round-trips of *other* children (exact inverses), so it
  cannot go stale — but any future change that searches between
  evaluate_child and the recursion must revisit this. Document the
  invariant on `ChildPrecompute`.
- **Reordering traps in `evaluate_child`**: rule50/two-piece checks
  must stay *after* the moves-empty branch; the repetition check must
  stay *before* any TT use. Both are covered by existing position tests
  (`fifty_move_checkmate_is_loss`) and the GHI suites — if one of those
  fails, the reorder, not the suites, is wrong.
- **Box churn**: one ~1 KiB alloc per evaluated child replaces a full
  movegen (~an order of magnitude more expensive), but if malloc shows
  up in the after-profile, switch the boxed precompute to a pooled
  free-list on `Search` (same `mem::take` pattern) before giving up on
  the dedup.
- **Peak memory**: precomputes live for the whole frame (all children),
  ~branching × 1 KiB per active frame. Fallback if this matters:
  after selection, drop precomputes of children other than the
  selected one; a later selection of a dropped child simply regenerates
  (`None` path) and loses only part of the dedup.
- `perf` in containers sometimes lacks kernel counter permissions; if
  `perf record` fails, fall back to `perf stat` or a temporary
  `#[inline(never)]` + manual symbol timing via the existing
  `child_evals`-style counters, and note it in report2.

## Out of scope (later plans)

- Parallelism (#2 → plan3, determinism design required).
- AND-side ordering signal (#5), lazy/staged child evaluation (#7),
  tablebases (#9), history/killer retuning (#10) — all change search
  behavior or need their own validation.
- Upstream `atomic-movegen` fast paths (`has_legal_move`, early-exit
  legality) — would need a new upstream release; revisit if the
  after-profile still shows generation dominant.
- `benchmark --refine-cap` (deferred from plan1 until the optimizer
  needs to tune the factor).

## Final task

Write `docs/plans/lean/report2.md`: the step 0 profile table, per-step
drift-check results (quick-suite `child_evals` identical) and stepwise
wall/eval measurements on m22_white, the movegen/alloc reduction
(before/after perf comparison), deviations from this plan, problems
encountered, missing tests, and next steps (plan3 parallelism,
profile-gated leftovers). Put raw outputs under
`docs/plans/lean/measurements/plan2/` (same layout convention as
`plan1/`).
