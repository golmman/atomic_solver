# Lean Plan 6 — micro-wins bundle: #14 wrapper slimming, #13 clock sampling, #12a TT non-terminal skip (+ #17 sizing spike)

Implements the **micro-wins bundle** named as a leading candidate in the
lean initiative history: backlog rows **#14** (anchor), **#13**, **#12a**,
plus the open **#17 sizing spike** that the working agreement requires
before #17 can be ranked. All three levers are wall-time-only with
**no search-semantics change**, so the bit-identical drift protocol
applies in full. The bundle is treated as one plan per the initiative's
own grouping ("micro-wins bundle"); each lever is independently revertible.

Naming constraint: `tests/test_plan6.rs` is already taken by another
initiative's plan numbering — any new test file must not use that name.

## Rationale (from the post-plan4 profile, 2026-09-09)

Per evaluated child (~37M on m22 first-outcome): make/unmake wrapper fat
~4.3 ns (#14, ceiling ~3% wall; push/pop kill alone ~1%), clock sampling
~2.2% of wall (#13), TT `outcome == None` skip ceiling ~2.5% (#12a,
15.5% of existence checks × 16% populate share). Combined realistic
target: **−3–7% wall** on m22-class cases, bit-identical trajectories.
#17 is unmeasured and the shuffle-win study position exercises it harder
than any suite case, so it gets a sizing spike, not an implementation.

## Phase 0 — #17 sizing spike (measurement only, reverted)

Temporary instrumentation, reverted after measuring (lean's spike
pattern; #12 and #14 spikes are the precedent):

1. Add temporary counters on `Search` (plain `u64` fields + a temporary
   accessor, or `AtomicU64` statics — whatever is quickest to revert):
   - `path_contains`: call count, sum of scan lengths, max scan length
     (`self.path_stack.contains(&key)`, `mod.rs:776-777`; hot call per
     evaluated non-rule50 child at `children.rs:170`, plus per dfpn
     entry at `core.rs:107`).
   - `best_move_repeats_path` (`core.rs:378-383`): call count and hit
     rate; each call costs a full do/undo round-trip + scan.
2. Measure both validation cases (per Measurement conventions):
   - `m22_white` first-outcome, `--timeout 30` (dominant case);
   - the shuffle-win FEN
     `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`
     first-outcome, `--timeout 100` (the case that stresses path scans
     at depth 51+ with 41 root moves).
3. Convert to a wall estimate (scan length × ~1 ns/element is precise
   enough at this scale; cross-check against the `evaluate_child` share)
   and record in `report6.md`.
4. **Revert the instrumentation completely.** Decision rule: potential
   ≥ ~3% wall ⇒ spin out a #17 implementation plan (next session);
   below ⇒ demote the backlog row to "spiked, below bar" with numbers.
5. Do not promote or demote anything else based on this spike.

## Phase 1 — #14 solver-side wrapper slimming (anchor)

Constraint from the initiative re-scope: **no upstream round** — the
upstream `Board::do_move`/`undo_move` core (7.6 ns/round-trip) is
inherently tight and `atomic-movegen` 2.2 is not vendored. The win is
solver-side: bypass the 64-byte `StateInfo::new()` zeroing and the
64-byte undo-stack push/pop on the child-eval make/unmake pair.

1. Add to `src/position.rs` two wrapper variants used only by the hot
   path (naming per full-word convention, e.g.
   `do_move_with_scratch` / `undo_move_with_scratch`):
   - Same body as `do_move`/`undo_move` except the caller supplies the
     `StateInfo` slot: `self.board.do_move(m, state)` +
     `self.refresh_zobrist()` (and the symmetric undo). The undo stack
     is **not** touched.
   - Documented soundness contract on the methods: `state` may contain
     stale bytes because `Board::do_move` writes every field
     `Board::undo_move` reads (the same argument the initiative spike
     records for a hypothetical upstream `undo_scratch()`); the caller
     must strictly pair the calls around the move and must not consume
     `checkers`/`pinned` from the slot between them. Conservative
     guard: pooled slots are initialized once with `StateInfo::new()`,
     so "stale" bytes are always bytes a previous `do_move` or
     `populate_state` wrote — no assumption about unwritten fields.
2. In `src/search/dfpn/children.rs`, replace the `pos.do_move(mv)` /
   `pos.undo_move(mv)` pair in `evaluate_child` (lines ~133/322) with
   the scratch pair over a **per-frame pooled slot** (extend the
   existing `ChildPrecompute` pooling). Document the slot-staleness
   invariant next to `ChildPrecompute` per the AGENTS.md convention.
   - The slot must be distinct from the existence-check `StateInfo`
     and the `sort_moves` buffers; verify no aliasing before wiring.
   - `best_move_repeats_path` keeps its nested regular-stack
     do/undo — the undo stack is untouched by the scratch path, so
     nesting stays LIFO-correct; `Position::clone` (which empties
     `undo_stack`) is unaffected.
3. Micro-benchmark before/after, reusing the spike methodology (5M
   round-trips on the m22 root position, release build): wrapper
   round-trip target **≤ ~8.5 ns** (from 11.9 ns; raw board floor is
   7.6 ns). Record both numbers in the report.

## Phase 2 — #13 clock sampling

1. `Search::time_exceeded` (`src/search/dfpn/mod.rs:790-802`) keeps the
   `stop_flag` / `memory_limited` atomic loads on **every** call and
   gates only the `Instant::now()` read behind a node-count sampler:
   re-read the clock when `self.nodes` has advanced by
   `CLOCK_SAMPLE_INTERVAL` (start at 4096 dfpn entries; constants live
   with the other search tunables) since the last read. `&self` means
   the two sampler fields need interior mutability (`Cell<u64>` /
   `Cell<Instant>`); keep them private.
2. All call sites keep calling `time_exceeded`; the per-chunk and
   per-round sites (`mod.rs:511/547/661`) are negligible either way,
   and the hot per-entry site (`core.rs:47`) is what the sampler buys
   back (~2.2% of wall per the profile).
3. Contract notes (must hold, add to the method doc):
   - Sampling only *delays* wall-deadline detection by at most
     `CLOCK_SAMPLE_INTERVAL` dfpn entries; `ExitReason::Timeout`
     granularity becomes coarse. Wall-clock tests use generous margins
     — acceptable.
   - Budget mode is unaffected: `child_eval_budget_exceeded`
     (`mod.rs:357-358`) is a pure integer compare, and `ExitReason`
     checks `BudgetExhausted` before `Timeout` — the deterministic
     `child_eval_budget` / `ExitReason::BudgetExhausted` semantics
     (lean non-goal) are untouched.

## Phase 3 — #12a TT non-terminal skip

1. In `evaluate_child`, immediately after the probe (`children.rs:215`)
   and before the populate/existence block (~246): if the entry is
   valid with `outcome.is_none()`, **skip** `populate_state` +
   `has_legal_move` **and** the `occupied() == 2` recheck, falling
   through to the unsolved-bounds reuse (~283).
2. Soundness invariant, documented at the skip site and in the
   initiative backlog row: `outcome == None` entries are only stored
   after the node passed the entry-level `outcome_from_state`
   full-movegen check (`core.rs:78`) ⇒ the position had legal moves,
   `rule50 < 100`, and `occupied > 2`. GHI draw-suppressed stores
   (`core.rs:294-300`, stored as `None` with `(1, 1)`) also refer to
   expanded positions, so the invariant covers them too. Do **not**
   key off `remaining_depth` shape (suppressed entries carry
   `remaining_depth == u32::MAX`; the existing degeneracy guard at
   `children.rs:283-294` already excludes them from bound reuse and
   stays as is).
3. Verify (and note in the report) that a subsequently *searched*
   child does not depend on the existence-check's populated
   `StateInfo` for its own movegen — the dfpn frame generates moves
   into its own pooled slot (`core.rs:74`); if any sharing is found,
   populate lazily at the search hand-off instead.
4. Expected gain ~2.5% ceiling; kept in the bundle because it shares
   the validation pass and is a ~5-line change.

## Companion tasks (same session, not levers)

1. **initiative.md**: flip backlog rows #14/#13/#12a (and #17 per the
   spike verdict) to done/demoted with one-line evidence; add the
   plan6 history bullet.
2. **Post-plan6 profile** (working agreement §4): re-run the m22
   first-outcome profile command from AGENTS.md and refresh the
   profile section shares — per-child costs shrink, so the pie
   re-ranks. Numbers only; no new promotions without spikes.
3. No AGENTS.md change is expected (no user-visible surface moves;
   clock-granularity notes live in the method docs).

## Validation (drift protocol is the gate)

1. **Baselines before any change**: `benchmark --suite quick --json
   --first-outcome --runs 1` (capture per-case `child_evals`), m22
   first-outcome stdout (`--timeout 30 --outcome-only`), and the
   committed `tests/fixtures/m22_default_stdout_golden.txt` is the
   default-mode reference.
2. **After all three levers** (and per-lever if a regression appears,
   bisect by reverting one lever):
   - quick suite `child_evals` **bit-identical per case**;
   - m22 first-outcome stdout **byte-identical**;
   - slow-tier lean3 golden byte-identical
     (`cargo test --release --test test_lean3 -- --include-ignored`);
   - `dfpn_frame_local_slots_deterministic` green (fast tier).
3. **Wall measurement**: m22 first-outcome `--timeout 30`, 5 runs,
   median, before vs after; one shuffle-win first-outcome run each
   side (also feeds the #17 context). Expect −3–7% combined; any
   regression > noise on either case is a stop-and-bisect.
4. **Budget-mode regression**: `cargo test --release --test
   test_decisive_remaining -- --include-ignored` passes unchanged.
5. `make test` green within the ~60 s gate.
6. `git diff` scope check: `src/position.rs`,
   `src/search/dfpn/{mod,core,children}.rs`, `docs/plans/lean/` docs.
   Phase 0 instrumentation must not appear in the final diff. If a
   unit test is added for the scratch make/unmake pairing, its file
   name must not be `test_plan6.rs` (taken).

## Deliverable

`report6.md` in this directory, including: the #17 spike numbers and
verdict, the wrapper micro-benchmark table (11.9 → target ≤ 8.5 ns),
drift evidence (quick-suite per-case + m22 stdout + golden), wall
deltas on both validation cases, the post-plan6 profile table, the
#12a soundness-invariant verification note, problems encountered,
missing tests, and next steps (parallelism determinism spike remains
the named successor candidate).

Per repo convention, this plan ends with the task of writing its
`report6.md` in this directory.
