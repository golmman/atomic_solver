# Lean Plan 5 — #15 `has_legal_move` playout cross-check

Implements item **#15** of the lean initiative: a random-playout property
test that locks the solver-side existence query and terminal classifier
against the full move-generation path. This is a **test-only** plan: no
`src/` change, no search-semantics change, so the bit-identical drift
protocol does not apply (there is nothing to drift). Correctness is the
highest-priority quality attribute in `AGENTS.md`; this plan buys
correctness hardening with no perf cost.

## Rationale

The hot path decides child terminality without generating move lists:
`evaluate_child` (`src/search/dfpn/children.rs`) calls
`Position::has_legal_move` (a wrapper over upstream
`has_legal_move_with_state`, fed a caller-populated `StateInfo`) for
~95% of evaluated children, then classifies terminality via
`Position::outcome_from_state` + the checkers/occupied board bits.
Report3's "Missing tests" note: correctness of this path is currently
*inherited* from the upstream equivalence test plus deterministic
fixtures; nothing solver-side actively cross-checks the wrapper and the
classification on live playout positions. A playout-based property test
closes that gap.

## What is being verified

At every position visited during random playouts, with one `StateInfo`
populated via `Position::populate_state` (mirroring the hot-path pattern
of populating once and reusing it for both query and movegen):

- **P1 — existence equivalence.**
  `pos.has_legal_move(&state) == !generate_legal_with_state(&board,
  &state, &mut moves).is_empty()`.
- **P2 — terminal classification agreement.**
  `pos.outcome_from_state(&state, &moves)` must equal the outcome
  reconstructed from the fast-path signals: if `!has_legal_move`, the
  outcome must be `Some(Loss)` when `state.checkers` is non-empty and
  `Some(Draw)` otherwise (never `None`, never `Win`); if
  `has_legal_move`, the position must not be mate/stalemate-terminal
  (it may still be terminal via commoner extinction, `rule50 >= 100`,
  or `occupied == 2` — those branches are independent of the move list
  and must agree with `pos.outcome()`).
- **P3 — full-path consistency.** `pos.outcome()` (which re-derives the
  move list internally) must equal `pos.outcome_from_state(&state,
  &moves)` at every visited position.
- **P4 — branch coverage guard.** The run must actually exercise the
  interesting branches: the aggregate assertion counters must show at
  least one checkmate-terminal, one stalemate-draw-terminal, and one
  extinction-terminal position. Without this guard the property test
  could silently degenerate into "both sides return `false`/`None` on
  quiet middlegames".

## Changes

1. **New file `tests/test_playout_crosscheck.rs`** (integration test,
   `mod common;`). The name must NOT be `test_plan5.rs` — that file is
   already taken by another initiative.
   - **Deterministic PRNG inline** (xorshift64* or splitmix64, ~10
     lines). Do not add a `rand` dev-dependency; it is not in the
     dependency tree and determinism is a requirement, not a nicety.
     Hard-coded seed constants per test.
   - **Seed positions**: `Position::STARTPOS_FEN`, plus the FENs of
     `common::load_move_order_suite()`, `common::load_decisive_suite()`,
     and `common::load_smoke_suite()` (parse format is already handled
     by `tests/common/mod.rs`; `MoveOrderCase.fen`). Add two tiny
     endgame FENs as extra seeds to force P4 coverage (stalemate:
     `7k/8/8/8/8/8/2q5/K7 w - - 0 1`; bare-kings two-piece draw:
     `4k3/8/8/8/8/8/8/4K3 w - - 0 1`).
   - **Playout driver**: per seed, play `playouts` random games of up to
     120 plies. Per ply: fresh `StateInfo::new()` populated once,
     generate the legal list, assert P1/P2/P3, pick a PRNG move,
     `Position::do_move`. Stop at a terminal position (per
     `outcome_from_state`) or ply cap. Use a fresh
     `Position::from_fen` per game (no undo replay needed; note
     `Position::try_do_move` is `#[cfg(test)] pub(crate)` and therefore
     not visible to integration tests — the move list already provides
     the legality filter).
   - **Failure diagnostics**: every assertion message includes the hard
     coded seed, playout index, ply, and the position FEN so any
     failure is exactly reproducible.
   - **Tier sizing**: a fast-tier test (`playouts = 2` per seed, 120
     plies) that must stay within ~5 s wall in a release build and
     ~30 s in a debug build (measure both during implementation; shrink
     ply cap or seed set if debug exceeds it). A second
     `#[ignore = "slow: ..."]` test with `playouts = 32` per seed and
     deeper caps for the slow tier.
   - Optional (cheap, same file): per-ply
     `common::assert_position_invariants` to also lock the incremental
     Zobrist hash along playout lines; drop it if it doubles the
     fast-tier wall.
2. **`docs/plans/lean/initiative.md`**: flip backlog row #15 to done
   referencing this plan; add the history bullet; refresh the profile
   section (companion task below).
3. **Deliverable**: `docs/plans/lean/report5.md` (final task).

If any `src/` change turns out to be necessary (i.e. the property test
finds a real bug), **stop and re-scope**: the fix is its own plan; the
bug gets a minimal failing fixture test first per correctness-first
convention.

## Companion task (same session, not a lever): post-plan4 profile re-rank

The working agreement owes a profile refresh after plan4 (the
initiative's profile table is still post-plan3; plan4 changed m22
first-outcome work by −62%, so all shares are stale). This is
measurement only, no backlog lever claims:

1. `perf record -e cpu-clock -g -o /tmp/opencode/plan5.data -- \\
    target/release/atomic_solver --fen '<m22_white FEN>' --timeout 30
    --first-outcome --outcome-only` (default 128 MB TT now applies);
    `perf report --stdio --no-children` for leaf attribution.
2. Rebuild the cluster-share table and update the "Post-plan3 profile"
   section of `initiative.md` into a "Post-plan4 profile" section with
   the new shares.
3. Sanity-check whether the demoted items (#12a, #14 wrapper, #13
   clock) moved enough to re-rank; record the verdict (numbers only) in
   `report5.md` — do not promote anything without its own spike.

## Validation

1. `make test` green with the new fast-tier test included; record its
   wall contribution (must fit the < ~60 s gate).
2. Slow-tier variant green via `cargo test --release -- --include-ignored`;
   record playout/position counts and wall.
3. **Teeth check** (one-off, documented in the report, not committed):
   temporarily mutate `Position::has_legal_move` to return
   `!has_legal_move_with_state(..)` (or drop the checkers branch in
   `outcome_from_state`) and confirm the property test fails; revert.
   A correctness test that cannot fail is not a test.
4. `git diff` scope check: only `tests/test_playout_crosscheck.rs` and
   the three docs (`plan5.md` pre-existing, `report5.md`,
   `initiative.md`). Any `src/` diff means the plan was violated (or a
   real bug was found — see re-scope rule above).

## Deliverable

`report5.md` in this directory, including: test design notes, tier
timings (release + debug), the teeth-check evidence, branch-coverage
counts, the post-plan4 profile table, any re-rank verdicts, problems
encountered, missing tests, and next steps.
