# Lean Plan 3 — Upstream `has_legal_move` fast path: terminal-check movegen elimination

Implements the "upstream `atomic-movegen` fast paths" next step from
`docs/plans/lean/report2.md` (the profile-gated leftover of initiative item 3).
Because this plan claims `plan3`, the roadmap entry for parallelism (item 2)
moves to `plan4`; `initiative.md` is updated accordingly.

Everything in this plan is **behavior-preserving** in `atomic_solver`:
identical outcomes, identical move ordering, identical `child_evals` per
position. The upstream change adds a new function without touching the
existing ones' observable behavior (including generated move *order*).
The bit-identical drift check remains the primary correctness gate, run
after every solver-side step.

## Context

After plan2, the solver's hot path is dominated by the terminal check in
`evaluate_child` (`src/search/dfpn/children.rs`). The plan2 addendum's perf
profile (m22_white, first-outcome, release):

| symbol (leaf) | % of samples |
| --- | --- |
| `generate_legal_with_state` | 46.2% |
| `Board::legal` (legality filter) | 12.1% |
| `evaluate_child` (own frame) | 23.2% |

The structural reason this is now the ceiling: there are ~19× more child
*evaluations* (37.5 M on the quick suite, m22 ≈ 2 M dfpn entries vs 37.5 M
evals) than child *searches*. Plan2 made the generation for searched
children single-pay, but for the ~95% of evaluated children that are never
searched, `evaluate_child` still runs a full `legal_moves_with_state`
(children.rs:246: populate `StateInfo` + `generate_legal_with_state` over
all pieces) **only to learn the boolean "does the child have any legal
move?"**. On the rare `false` answer, the checkers bit additionally
classifies checkmate (`Loss`) vs stalemate (`Draw`) — `position.rs`
`outcome_from_state` (position.rs:197-219) uses nothing else from the move
list besides `moves.is_empty()`.

Full generation cannot be short-circuited inside `atomic_solver`: legality
lives in the movegen crate. What is needed is an **early-exit existence
query** in `atomic-movegen`, which can stop at the first legal move instead
of enumerating all of them. This is the same upstream-contribution channel
as before: `docs/plans/movegen/feedback.md` → implemented items in 2.0.0
(`generate_legal_with_state` et al., see
`docs/plans/movegen/report_update_2.0.0.md`).

Sizing facts that drive the design:

- `MoveList` is `[Move; 256] + len` (~1 KiB, plan2 already pools these);
  `StateInfo` is a handful of bitboards/ints. The new query needs only a
  caller-populated `StateInfo` — no move list at all on the `true` path.
- `is_move_trivially_legal` (atomic-movegen board.rs:1243-1280) already
  encodes the cheap existence proof: when `state.checkers` is empty and
  `state.commoners_count > 0`, any *quiet* pseudo-legal move by an
  unpinned non-commoner piece is legal without calling `Board::legal`.
  Nearly every non-terminal position has such a move, so the expected cost
  of the query on the hot path is a short bitboard scan, not a generation.
- `atomic-movegen` is an external crates.io dependency (2.1.0). Phase 1 of
  this plan happens in the movegen repository (or as a local vendored
  checkout behind `[patch.crates-io]` until a version is published).

## Decisions

1. **Upstream API: `has_legal_move` + `has_legal_move_with_state`.**
   Mirroring the existing `generate_legal` / `generate_legal_with_state`
   convention:
   - `pub fn has_legal_move(board: &Board) -> bool` — convenience, calls
     `populate_state` internally.
   - `pub fn has_legal_move_with_state(board: &Board, state: &StateInfo)
     -> bool` — caller-supplied populated `StateInfo`; no list, no
     allocation.
   Equivalence contract (the only allowed observable behavior): for a
   populated `StateInfo` and any `MoveList`,
   `has_legal_move_with_state(b, s)` must equal `list.len() > 0` after
   `generate_legal_with_state(b, s, &mut list)`. Edge cases that fall out
   of `Board::legal` and must be preserved:
   - `commoners(us).is_empty()` ⇒ `false` (every candidate is rejected by
     the `our_commoners.is_empty()` guard, board.rs:1175-1177).
   - Positions where the only legal move is a capture, en passant,
     promotion, or castling ⇒ the fast quiet-move scan finds nothing and
     the fallback must run.
   - Positions in check ⇒ the trivial-legality fast path is disabled
     (`is_move_trivially_legal` returns `false` when `checkers` is
     non-empty); the fallback decides.
2. **Upstream implementation: shared candidate stream, early exit.**
   Factor `generate_pseudo_legal` (movegen.rs:15-70) into a
   `for_each_pseudo_legal(board, f: impl FnMut(Move) -> ControlFlow<...>)`
   that walks the same stages in the same order (pawns → knights → bishops
   → rooks → queens → commoners → castling, movegen.rs:21-69).
   `generate_pseudo_legal` becomes a thin wrapper collecting into the
   `MoveList` — **generated move order must remain bit-identical**
   (perft tests plus an order-pinning test guard this). The new
   `has_legal_move_with_state` drives the same stream with a closure that
   tests `is_move_trivially_legal(board, m, state) || board.legal(m,
   state)` per candidate and returns early (`ControlFlow::Break`) on the
   first legal one — the filter is exactly `generate_legal_with_state`'s,
   so correctness is inherited and only the early exit is new logic.
   An optional cheaper pre-pass (accepting the first quiet move by an
   unpinned non-commoner piece without per-candidate filter calls) is
   measure-first: add it only if the step 1c benchmark shows the closure
   overhead matters, and only accepting what `is_move_trivially_legal`
   would accept.
   Rejected alternative: a standalone duplicated generation loop inside
   `has_legal_move` — smaller diff, but the pseudo-legal rules would then
   live in two places; the shared-stream refactor keeps one source of
   truth.
3. **Solver terminal check becomes boolean + bitboard classification.**
   In `evaluate_child`'s final `else` (children.rs:233-288), replace
   `moves.clear(); pos.legal_moves_with_state(moves, state);
   pos.outcome_from_state(state, moves)` with:
   - `pos.populate_state(state)` (gives `checkers` and the counts),
   - `if pos.has_legal_move_with_state(state)` → non-terminal, fall
     through to the unsolved-bounds path (unchanged);
   - `else if state.checkers.is_empty()` → `Outcome::Draw` (stalemate),
   - `else` → `Outcome::Loss` (checkmate).
   Equivalence argument: the extinction branches already returned, and
   `rule50 >= 100` was handled by the earlier `rule50_expired` branch, so
   from `outcome_from_state` (position.rs:197-219) only the moves-empty
   and `occupied == 2` branches can fire. `occupied == 2` is
   board-static (no movegen needed) and *follows* the moves-empty branch,
   so it must be applied explicitly after the `has_legal_move` check:
   `else if pos.board().occupied().count() == 2 → Draw`. Result
   precedence is identical to `outcome_from_state`.
   The `rule50_expired` branch (children.rs:180-201) is rewired the same
   way: `has_legal_move` true ⇒ `Draw` (rule50 outranks `occupied == 2`),
   false ⇒ checkmate/stalemate classification. This preserves the
   deviation-4 semantics that tests 4 and 5 in `children.rs` lock.
4. **Slots become frame-local: the parent no longer pre-fills.** With the
   terminal check no longer generating, `evaluate_child` has no move list
   to hand to the recursion. Consequences:
   - `evaluate_child` loses its `slot: Option<&mut ChildPrecompute>`
     parameter; the recursion calls `dfpn` without a precompute.
   - `dfpn` loses its `precomputed: Option<ChildPrecompute>` parameter;
     the entry generates into a pooled slot taken from
     `precompute_pool[path_stack.len() + 1]` (the depth the frame will
     occupy after `path_push`), preserving plan2's zero-alloc invariant.
     The existing `None` branch (core.rs:66-74) is replaced by this.
   - Net per-child cost: searched children still generate exactly once
     (at their own frame, as plan2 arranged); non-searched children pay
     the early-exit query instead of a full generation.
   - `ChildPrecompute` survives unchanged as the pooled slot type; only
     its producer moves from the parent's `evaluate_child` to the child's
     `dfpn` entry.
5. **`populate_state` runs twice per searched child** (once in
   `evaluate_child` for the classification state, once at the child's
   `dfpn` entry). This is a deliberate trade: `populate_state` was 1.8%
   of samples post-plan2 across *all* entries, and the alternative
   (returning the populated state from `has_legal_move_with_state`, or
   caching states on `Position`) adds API complexity for a few percent of
   a much smaller denominator. Step 0's profile re-check confirms it
   stays below the 10% gate.
6. **Dependency versioning.** Phase 1 ships as `atomic-movegen 2.2.0`
   (additive, semver-minor). If the release is not yet published when
   Phase 2 starts, develop against a local checkout via
   `[patch.crates-io]` in `Cargo.toml` (dep requirement stays `"2.1"`,
   the patch supplies the new function), then bump to `"2.2"` and drop
   the patch when published. `Cargo.lock` is regenerated with the bump.

## Implementation steps

### Phase 1 — upstream (`atomic-movegen` repository, separate session)

Work happens in the movegen repo. The standalone, self-contained handoff
spec for this phase — the file to copy into that repo — is
**`docs/plans/movegen/plan_has_legal_move.md`** (repo-agnostic: no
references to `atomic_solver` paths or conventions). The section below
summarizes it for reviewers of this plan; the handoff file is
authoritative for Phase 1.

- **1a.** Refactor `generate_pseudo_legal` into
  `for_each_pseudo_legal(board, f)` with identical stage order; keep
  `generate_pseudo_legal` as a collecting wrapper. Gate: existing crate
  tests pass; add an order-pinning test (fixed positions → exact UCI move
  sequence, compared against the pre-refactor output recorded in the
  test) and run the crate's perft suite.
- **1b.** Implement `has_legal_move_with_state` (+ convenience wrapper)
  per Decision 2. Gate: an equivalence property test — random playouts
  (several thousand positions from varied start FENs, including
  promotion-heavy and capture-heavy playouts) assert
  `has_legal_move_with_state(b, s) == { generate_legal_with_state(...);
  list.len() > 0 }` on every position, with `populate_state` before both;
  plus deterministic fixtures: bare checkmate, stalemate, in-check with
  one escape, castling-as-only-move, pinned-piece-only moves,
  en-passant-as-only-move, extinction positions (zero own commoners),
  K vs K.
- **1c.** Benchmark the query itself (criterion if the crate has it;
  otherwise a `--release` timing loop over the fixtures): the fast path
  should be O(few bitboard ops), and a no-legal-move position should cost
  ≈ one full generation (unavoidable).
- **1d.** Publish 2.2.0 (or hand the diff to the maintainer and record
  the outcome in a `docs/plans/movegen/` note, as report_update_2.0.0.md
  did).

### Phase 2 — solver (`atomic_solver`)

- **Step 0 — baseline + profile (before touching `src/`).**
  - `cargo build --release`
  - `cargo run --release --example benchmark -- --suite quick --json
    --first-outcome > docs/plans/lean/measurements/plan3/quick_before.json`
    (expected total `child_evals` 38,714,551 — if it differs from plan2's
    stored artifact, stop and investigate before proceeding).
  - m22_white (`tests/fixtures/move_order_positions.txt`) first-outcome
    and default-mode runs, stdout/stderr logs stored alongside.
  - `perf record -e cpu-clock -g` on the m22 first-outcome run; store
    `perf report --no-children` top sections as the before-profile.
- **Step a — dependency bump + `Position` wrapper.** Add
  `Position::has_legal_move(&self, state: &StateInfo) -> bool`
  delegating to `has_legal_move_with_state` (position.rs, next to
  `legal_moves_with_state`). Bump `atomic-movegen` (or add the interim
  `[patch.crates-io]`). Drift check: quick suite byte-identical.
- **Step b — terminal-check rewiring in `evaluate_child`.** Apply
  Decision 3 to the final `else` and the `rule50_expired` branch
  (children.rs). The unsolved-bounds TT path, repetition check, TT probe,
  and proof-event emission are untouched. Drift check: quick suite
  identical `child_evals`/`pv_len` 59/59; m22 first-outcome stdout
  byte-identical.
- **Step c — slot plumbing simplification.** Apply Decision 4: drop the
  `slot` parameter from `evaluate_child` and the `precomputed` parameter
  from `dfpn`; the entry takes its slot from `precompute_pool` at
  `path_stack.len() + 1` and generates into it. Update plan2's slot tests
  (see New tests). Drift check: quick suite identical; m22 first-outcome
  chunk `work_done`/`nodes` sequences identical; m22 **default-mode**
  trajectory bit-identical (this is the check that caught plan2's
  deviation 4 — do not skip it).
- **Step d — post-change profile + wall measurement.** Re-run the m22
  first-outcome perf profile: `generate_legal_with_state` share should
  collapse into `has_legal_move` (expected a small fraction of the old
  cost); verify `populate_state` stays below the 10% gate (Decision 5).
  Record first-outcome wall vs step 0 (plan2 measured 11.87-11.96 s on
  this container; the target is a measurable double-digit-percent
  improvement, to be reported honestly rather than promised).
- **Step e — fold in report2's missing tests.**
  - Default-mode trajectory guard: an integration test
    (#[ignore = "slow: ~20 s wall"]) solving the m22 fixture in default
    mode with a fixed timeout and asserting stdout equals a stored golden
    (derive the golden from the drift-verified current binary). This is
    the automated proxy plan2's report called out as missing.
  - Pool peak-memory assertion: after a bounded solve, assert
    `precompute_pool.len()` and per-depth slot counts stay within
    branching × active depth (plan2 documented ~3-4 MB on m22 scale; lock
    it).
- **Step f — AGENTS.md.** Update the dfpn bullet: the terminal check is
  now an upstream `has_legal_move` early-exit query; remove the "the
  per-evaluated-child generation for the terminal check is irreducible
  inside atomic_solver" claim; note the slot flow is frame-local.

## Verification

- `cargo test --release` (fast tier) green, including the updated slot
  tests and new classification tests.
- `make test-full` before closing the plan (search-path change ⇒ required
  per AGENTS.md).
- Drift protocol (after steps a, b, c): quick suite 59/59 identical
  `child_evals` + `pv_len`, total 38,714,551; m22 first-outcome stdout
  byte-identical; m22 default-mode chunk sequences bit-identical;
  `tests/test_ghi.rs`, `tests/test_repetition.rs`,
  `tests/test_transpositions.rs` pass.
- Perf: before/after `perf report` sections stored under
  `docs/plans/lean/measurements/plan3/` alongside the quick-suite JSONs
  and m22 logs.

## New tests (solver side)

1. `evaluate_child_checkmate_child_classifies_loss_without_movegen` and a
   stalemate twin — same fixtures as the plan2 tests, asserting the
   `ChildInfo` outcomes are unchanged under the boolean path.
2. `evaluate_child_two_commoners_is_draw` — a child that is K vs K with
   `rule50 < 100` (moves exist) must be `Draw` via the `occupied == 2`
   branch, guarding the precedence added in Decision 3.
3. Plan2's `evaluate_child_rule50_expired_repetition_is_not_repetition_seen`
   and `evaluate_child_rule50_checkmate_is_loss` stay **unchanged** — they
   lock the semantics this plan must preserve.
4. `dfpn_entry_generates_into_pooled_slot` — replaces the plan2
   slot-filling tests 1-3: after a solve, the entry's slot content equals
   the position's legal-move list, and no parent-level pre-fill happens
   (slots are only written by the frame that consumes them).
5. `solve_twice_identical_counts` (plan2) stays green — catches pool
   state leaking through the new slot lifecycle.
6. Step e's two tests (trajectory golden; pool bound).

## Problems to watch for

- **Upstream move-order drift during the `for_each_pseudo_legal`
  refactor.** The solver's ordering and TT behavior depend on the exact
  candidate sequence; the order-pinning test and crate perft are the
  gate. If the refactor turns risky, fall back to a standalone early-exit
  loop inside `has_legal_move_with_state` (duplication, but zero risk to
  existing output) and note it in the report.
- **Fast-path soundness.** The cheap pre-pass must accept only what
  `is_move_trivially_legal` accepts; a wrongly accepted "exists" answer
  silently turns a terminal child into a non-terminal one. The
  equivalence property test is the primary defense; make the playout
  generator include heavy-capture lines (explosions change occupancy
  mid-`legal()`).
- **In-check positions could regress** if the fallback path is slower
  than the old full generation. Measure in step 1c/step d; in-check
  children are a minority, so a small regression there is acceptable if
  the aggregate win holds.
- **Version availability.** If 2.2.0 cannot be published during this
  plan, use the `[patch.crates-io]` interim and record it in the report;
  do not vendor a modified copy into the solver repo.

## Out of scope (later plans)

- `Board::outcome()` / `Board::is_terminal()` rewrites on top of
  `has_legal_move` (crate-side cleanup, no solver impact).
- A resumable legal-move generator (generate-once-with-early-exit *and*
  reuse for the searched child) — strictly better asymptotics but a much
  larger API change; revisit only if the post-plan3 profile still shows
  movegen ≥ ~25%.
- Parallelism (initiative item 2, now plan4) and items #5/#7/#9/#10.

## Final task

Write `docs/plans/lean/report3.md` following the house report format
(summary of changes, stepwise measurements, drift-check evidence,
deviations, problems, missing tests, next steps), with raw outputs under
`docs/plans/lean/measurements/plan3/`. Include the honest wall-time
delta and the before/after perf profiles.
