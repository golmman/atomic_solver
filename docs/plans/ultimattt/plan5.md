# Plan 5: Adopt `ultimattt`-style work-bounded iterative deepening

## Goal

Replace the hybrid depth/work `solve_outcome` bootstrap with a pure work-bounded
iterative-deepening loop, similar to `ultimattt`'s sequential `dfpn`.  During the
bootstrap `max_depth` is effectively unbounded; the search is stopped and resumed
by doubling work chunks, reusing the transposition table between chunks.

This prioritizes **proving decisive outcomes for deep positions** (roughly 30
full moves / 60 plies or more).  `find_ppv` and PPV extraction remain important,
but `refine_sppv` and SPPV refinement are secondary; the plan does not optimize
SPPV at the expense of outcome correctness or PPV validity.

The current hybrid (`docs/plans/ultimattt/report4.md`) already added a
`max_work` parameter and capped each depth probe with a work budget.  Removing
the fixed `max_depth` schedule entirely should eliminate the last horizon cliff:
a deep forced win no longer has to wait for the `max_depth=64` probe (or the
unbounded fallback that currently wipes the transposition table), because a work
chunk can naturally grow past any fixed depth once the winning line is found
inside it.

## Concrete changes

### 1. `src/search/dfpn/mod.rs`

#### 1.1 `solve_outcome` becomes work-bounded only

Remove the `max_depth` schedule and run `dfpn` with `max_depth = u32::MAX` and
a doubling work chunk.  Only path-dependent state is reset between chunks; the
transposition table, history, and killer tables are retained.

```rust
pub fn solve_outcome(&mut self, pos: &mut Position) -> Outcome {
    self.begin_run();
    self.proof_mode = ProofMode::Outcome;

    let mut outcome = Outcome::Draw;
    let mut chunk = 500_000u64;
    let mut success_depth: Option<u32> = None;

    while !self.time_exceeded() {
        self.reset_search_state();
        outcome = self.dfpn(pos, INF, INF, u32::MAX, chunk, true);

        if outcome != Outcome::Draw {
            if let Some(entry) = self.tt.probe(pos.hash())
                && entry.outcome.is_some()
            {
                success_depth = Some(entry.depth);
            }
            if success_depth.is_none() {
                if let Some(pv) = self.extract_pv_checked(pos, outcome, None) {
                    success_depth = Some(pv.len() as u32);
                }
            }
            if success_depth.is_none() {
                // Last-resort cap so the follow-up stages have a finite bound.
                success_depth = Some(self.max_ply as u32);
            }
            break;
        }

        chunk = chunk.saturating_mul(2);
        if chunk == u64::MAX {
            break;
        }
    }

    // If the work loop ran out of budget without a decisive result, spend the
    // remaining wall-clock time on a single unbounded search.  Keep the table
    // and history from the work chunks; only reset path state.
    if outcome == Outcome::Draw && !self.time_exceeded() {
        self.reset_search_state();
        outcome = self.dfpn(pos, INF, INF, u32::MAX, u64::MAX, true);

        if outcome != Outcome::Draw {
            if let Some(entry) = self.tt.probe(pos.hash())
                && entry.outcome.is_some()
            {
                success_depth = Some(entry.depth);
            }
            if success_depth.is_none() {
                if let Some(pv) = self.extract_pv_checked(pos, outcome, None) {
                    success_depth = Some(pv.len() as u32);
                }
            }
            if success_depth.is_none() {
                success_depth = Some(self.max_ply as u32);
            }
        }
    }

    self.bootstrap_success_depth = success_depth;
    // A pure work-bounded loop has no reliable "deepest searched depth".
    // Zero is a safe lower bound: a non-terminal position cannot win or lose
    // in zero plies, so refinement starts from there.
    self.bootstrap_fail_depth = 0;
    outcome
}
```

Notes:

- `bootstrap_success_depth` must always be concrete after a decisive outcome.
  The decisive root TT entry records a concrete depth (`shortest win` for a
  `Win`, `longest loss` for a `Loss`).  For a `Win` in `Outcome` mode this is an
  upper bound on the shortest mate: the search breaks as soon as any winning
  child is proven, so the stored depth is the depth of the first proven winning
  line.  `find_ppv` may therefore return a non-shortest PPV for `Win` before
  `refine_sppv` tightens it; for a `Loss` the depth is exact because all
  children must be solved.  If for any reason the root entry lacks a depth, fall
  back to a validated PV length, then to `max_ply`.  Never pass `u32::MAX` to
  `find_ppv` / `refine_sppv`.
- Do **not** call `self.tt.new_generation()` or `self.reset_history_and_killers()`
  in the unbounded fallback.  The work chunks have built useful bounds and
  ordering; discarding them would restart the search from scratch.
- `child_evals` is not reset between chunks, but each `dfpn` call captures its
  own `child_evals_start`, so each chunk is allowed `chunk` new child
  evaluations.  Total work therefore grows like `500k, 1.5M, 3.5M, ...` until
  time runs out.

#### 1.2 `refine_sppv` uses binary search on depth

Keep the existing binary-search structure but tighten the initial best-length so
an empty `last_pv` does not prevent the first discovered PV from being recorded.

```rust
pub fn refine_sppv<F>(&mut self, pos: &mut Position, outcome: Outcome, mut on_shorter: F)
where
    F: FnMut(&[Move]),
{
    let start_depth = self
        .bootstrap_success_depth
        .unwrap_or(self.last_pv.len() as u32);
    let mut hi = start_depth;
    let mut lo = self.bootstrap_fail_depth;

    // If last_pv is empty, use hi as the initial best length so any proven PV
    // at a probe below hi is reported as shorter.
    let mut current_best_len = if self.last_pv.is_empty() {
        hi
    } else {
        self.last_pv.len() as u32
    };

    while hi > lo + 1 && !self.time_exceeded() {
        let probe = lo + (hi - lo) / 2;
        let mut chunk = 500_000u64;
        let mut proved_at_probe = false;

        // A few retries with doubling work avoid false negatives caused by a
        // tight budget; if the depth bound itself is too low, the retries are
        // cheap because the tree is shallow.
        for _ in 0..3 {
            if self.time_exceeded() {
                break;
            }
            self.reset_search_state();
            self.proof_mode = ProofMode::Sppv;
            let o = self.dfpn(pos, INF, INF, probe, chunk, true);

            if self.time_exceeded() {
                break;
            }

            if o == outcome {
                if let Some(pv) = self.extract_pv_checked(pos, outcome, None) {
                    let pv_len = pv.len() as u32;
                    if pv_len < current_best_len {
                        self.last_pv = pv;
                        current_best_len = pv_len;
                        on_shorter(&self.last_pv);
                    } else if pv_len == current_best_len {
                        self.last_pv = pv;
                    }
                }
                proved_at_probe = true;
                break;
            }

            chunk = chunk.saturating_mul(2);
            if chunk == u64::MAX {
                break;
            }
        }

        if self.time_exceeded() {
            break;
        }

        if proved_at_probe {
            hi = probe;
        } else {
            lo = probe;
        }
    }
}
```

For an exact depth-bounded search the predicate "`outcome` is decisive in `d`
plies" is monotonic in `d` for both `Win` and `Loss`, so binary search on
`[lo, hi]` would be sound.  With a finite `max_work` budget a probe can return
`Draw` because it ran out of work rather than because the depth is too low; the
retry loop mitigates this but does not eliminate it.  The binary search is
therefore best-effort for SPPV, and PPV validity is not affected.

#### 1.3 `Search::solve()` unbounded branch

For consistency, the `!self.refine_shortest` branch of `solve()` should also
extract a concrete `bootstrap_success_depth` instead of defaulting to
`u32::MAX`.  Use the same precedence: TT entry depth, then validated PV length,
then `max_ply`.

### 2. `src/search/dfpn/core.rs`

#### 2.1 Harden `max_work` enforcement

The top-of-loop work check already prevents starting a new iteration with an
exhausted budget, but a re-evaluated child can push `child_evals` past the limit
just before the recursive expansion.  Add an explicit short-circuit before the
recursive call:

```rust
let (mv, child_pn, child_dn) = selection.best_child;
if mv == Move::NONE {
    break;
}
let (second_pn, second_dn) = selection.second_child;

let work_spent = self.child_evals - child_evals_start;
if max_work != u64::MAX && work_spent >= max_work {
    break;
}
let child_max_work = max_work.saturating_sub(work_spent);
```

This guarantees that a work-bounded `dfpn` call never recurses with a zero or
negative remaining budget.

The first call to `evaluate_all_children` evaluates every legal move without an
explicit budget check.  With the proposed `chunk = 500_000` this is negligible
(branching factor is far smaller), but the plan relies on `chunk` being larger
than the number of legal moves at any node.

#### 2.2 Store a clean work-cutoff result

When the loop breaks due to `max_work`, `outcome_to_store` is still `None`.  The
final `store` call therefore stores:

- `outcome = None`,
- `pn = pn.max(1)`, `dn = dn.max(1)`,
- `remaining_depth = max_depth` (which is `u32::MAX` during the bootstrap),
- `depth = selection.depth` (zero if no child was solved).

This is already the current behavior; the only change is that unsolved
bootstrap entries now carry `remaining_depth = u32::MAX` at the root and
`u32::MAX - ply` below it.  Solved `Win` / `Loss` entries still store
`remaining_depth = u32::MAX` and the concrete mate distance in `depth`.

### 3. `src/search/dfpn/children.rs`

#### 3.1 `evaluate_child` TT reuse

The existing unsolved-summary guard is correct for the work-bounded bootstrap
and should be kept:

```rust
let use_as_unsolved = summary.outcome.is_none()
    && summary.remaining_depth != u32::MAX
    && summary.remaining_depth <= child_max_depth
    && summary.pn > 0
    && summary.dn > 0;
```

With `max_depth = u32::MAX`, the only unsolved entry whose `remaining_depth`
is exactly `u32::MAX` is the root entry, and `evaluate_child` is never called
for the root.  Deeper unsolved entries have `remaining_depth = u32::MAX - ply`,
which equals the `child_max_depth` seen when the same node is reached again in
the next work chunk, so the `<=` test allows reuse.  The guard also rejects
over-deep summaries during bounded `refine_sppv` probes, which is the desired
safety property.

Add a short comment explaining that `remaining_depth == u32::MAX` on an
unsolved entry means "unbounded work cutoff" and is intentionally ignored when
`child_max_depth` is finite.

### 4. `src/main.rs`

No change required.  The existing flow

```
solve_outcome -> find_ppv -> (optionally) refine_sppv
```

already uses the concrete `bootstrap_success_depth` produced by `solve_outcome`.
The earlier idea of calling `Search::solve()` directly for `--no-refine-shortest`
is unnecessary: `Search::solve()` with `refine_shortest = false` does a single
unbounded search, but that would reset the timer and discard any staged
progress.  Keep the current staged call sequence.

### 5. `examples/benchmark.rs`

No change required; the benchmark uses `Search::solve()`, which calls
`solve_outcome` when `refine_shortest` is `true`.  Optionally add a
`--work-chunk` argument for tuning, but this is not required for correctness.

## Verification

Run the standard quality checks from `AGENTS.md`:

```bash
cargo fmt
cargo clippy --all-targets
cargo test
cargo test --release
cargo doc --no-deps
```

### Regression checks

```bash
# fen1: the original max_depth=8 horizon case, black to move, expected loss.
cargo run --release -- --fen \
    '6k1/3p4/2pB2p1/6Pp/7P/p1N2P2/P1PP4/1R5K b - - 0 25' \
    --timeout 60

# fen2: shallow white mate, expected win.
cargo run --release -- --fen \
    '6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26' \
    --timeout 60 --no-refine-shortest

# m24 white to move, expected win.
cargo run --release -- --fen \
    '4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R2R2K w - - 0 24' \
    --timeout 60

# m24 black to move, expected loss.
cargo run --release -- --fen \
    '4r1k1/3p4/2pB2p1/6Pp/p4p1P/2N1PP2/P1PP4/1R2R2K b - - 0 24' \
    --timeout 60
```

All four must return decisive outcomes within 60 seconds.

### Test suite

```bash
cargo test --release --test test_plan6 m27_shortest_pv m27_streaming_output m27_ppv_only timeout_message
cargo test --release --test test_plan6 m24_ppv --ignored
```

`m27_shortest_pv` must still report a 7-plies win.  `m24_ppv` must still pass
within 60 seconds.  Failures in SPPV-only assertions are lower priority than
outcome or PPV failures.

### Benchmark

```bash
cargo run --release --example benchmark -- --runs 10 --timeout 5
cargo run --release --example benchmark -- --runs 10 --timeout 5 --refine-shortest
```

There should be no regressions in nodes, `child_evals`, or outcome on the
existing benchmark suite.

## Risks and mitigations

- **Work-budget explosion.** With `max_depth = u32::MAX` the first 500k chunk
  could spend its entire budget on one deep but irrelevant line.  Mitigation:
  the explicit `max_work` short-circuit stops each chunk cleanly, the
  `explored` flag prevents re-expanding exhausted children within a chunk, and
  DF-PN's threshold propagation focuses later chunks on the most-proving lines.
- **Binary search may waste time on SPPV probes below the shortest win/loss.**
  A probe `d` smaller than the true shortest distance returns `Draw` regardless
  of work.  The retry loop is capped at three attempts; after that the probe is
  treated as a failure and `lo` moves up.  This is acceptable because SPPV is
  the lowest priority; if refinement runs out of time, the already-found PPV
  remains valid.
- **`bootstrap_success_depth` must be concrete.** If the decisive root TT entry
  and `extract_pv_checked` both fail, the fallback to `self.max_ply` is safe but
  may make `find_ppv` / `refine_sppv` slow.  The plan explicitly avoids the
  `u32::MAX` sentinel that would make the follow-up stages run unbounded.
- **PPV extraction for deep wins may consume the remaining time budget.**
  `find_ppv` uses `max_work = u64::MAX` and runs until the configured timeout,
  sharing whatever time `solve_outcome` left.  This is acceptable because PPV
  is the medium-high priority and the outcome is already proven, but very deep
  wins may time out before a PPV is produced.  Use `--no-refine-shortest` to
  skip the SPPV stage and leave more time for `find_ppv`.
- **`bootstrap_fail_depth = 0` starts binary search from scratch.** This is
  correct and adds at most `log2(hi)` probes.  A future improvement could track
  the deepest fully searched ply from the root TT, but with `max_depth =
  u32::MAX` the stored `remaining_depth` is not a useful fail depth.
- **Unbounded `max_depth = u32::MAX` could in principle recurse very deeply.**
  The work budget and the default 5-second timeout prevent runaway recursion in
  practice.  `MAX_KILLER_DEPTH` and `max_ply` already cap killer-move scoring
  and PV extraction at 256 / 1000 plies, so the search cannot accidentally
  produce a PV longer than `max_ply`.
- **TT entries from the bootstrap cannot be reused as unsolved bounds during
  bounded `refine_sppv` probes.** This is the desired safety property: bootstrap
  bounds were computed with an unboundedly large horizon and may be too
  optimistic for a finite `max_depth`.  Solved entries along the winning line
  are still reused via `try_use_tt` when `entry.depth <= probe`, and previous
  `refine_sppv` probes with smaller `max_depth` can be reused by later, deeper
  probes.

## Resolved open questions

The following decisions were reached during review and are reflected in this
plan:

1. **`bootstrap_success_depth` for `Win` is an upper bound.**  The depth comes
   from the first winning child proven by `solve_outcome`, not a guaranteed
   shortest mate.  `refine_sppv` is responsible for tightening it.
2. **SPPV binary search is best-effort.**  The 3-retry loop stays; a work-limited
   `Draw` can still be misclassified as a depth-limited `Draw`, so refinement
   may not always reach the true shortest PV.
3. **Strict `evaluate_child` unsolved-summary guard is kept.**  `find_ppv` does
   not reuse bootstrap unsolved bounds with huge `remaining_depth`; it
   recomputes inside its finite bound.
4. **No recursion-depth cap.**  Search recursion is limited by the work budget
   and the timeout only.
5. **Unbounded `solve_outcome` fallback is kept.**  It remains a safety net for
   the case where the work chunk doubles to `u64::MAX` before the timeout.
6. **`evaluate_all_children` is left budget-unaware.**  The 500k initial chunk is
   assumed to exceed the legal-move count at any node.
7. **Redundant `hi = probe` in `refine_sppv` is removed.**  The post-loop
   `hi`/`lo` update remains.

## Summary

1. Convert `solve_outcome` from a depth schedule to a pure work-doubling loop
   with `max_depth = u32::MAX`.
2. Harden `max_work` enforcement in `dfpn` so work-bounded calls stop cleanly.
3. Record a concrete `bootstrap_success_depth` from the decisive root TT entry
   or a validated PV; use `0` for `bootstrap_fail_depth`.
4. Keep binary search in `refine_sppv`, but initialize the best-length
   correctly when `last_pv` is empty.
5. Preserve the existing `find_ppv` / `extract_pv` depth-aware extraction and
   the `evaluate_child` unsolved-summary guard.
6. Verify on the `fen1` / `fen2` regression, the `m24` / `m27` tests, and the
   benchmark suite.  Outcome correctness and PPV validity are the primary
   success criteria; SPPV refinement is secondary.

After implementation, write `docs/plans/ultimattt/report5.md` documenting the
changes and the verification results.
