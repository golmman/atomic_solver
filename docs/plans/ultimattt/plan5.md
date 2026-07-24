# Plan 5: Adopt `ultimattt`-style work-bounded iterative deepening

## Goal

Replace the current `solve_outcome` depth schedule (`1, 2, 4, 8, 12, 16, 20, 24, 32, 48, 64`) with a pure work-bounded iterative-deepening loop similar to `ultimattt`'s sequential `dfpn`. The search should grow past fixed-depth horizons naturally by reusing the transposition table across doubling work chunks, with `max_depth` effectively unbounded during the bootstrap.

## Background

`docs/plans/ultimattt/plan4.md` already described this approach:

```rust
let mut chunk = 500_000u64;
while !self.time_exceeded() {
    self.reset_search_state();
    let outcome = self.dfpn(pos, INF, INF, chunk, true);
    if outcome != Outcome::Draw {
        break;
    }
    chunk = chunk.saturating_mul(2);
}
```

The current `atomic_solver` implementation in `src/search/dfpn/mod.rs` still uses a `max_depth` schedule. Each probe is both depth- and work-bounded. A position whose shortest forced win is 13 plies can waste most of its time expanding the `max_depth = 12` tree, then time out before the `max_depth = 16` probe can find the win. The finer schedule mitigates this but does not remove the cliff.

`dfpn` already has a `max_work` parameter, but the bootstrap loop is the main obstacle.

## Concrete changes

### 1. `src/search/dfpn/mod.rs`

#### 1.1 `solve_outcome` becomes work-bounded only

Remove the `max_depth` schedule and run `dfpn` with `max_depth = u32::MAX` (or a large fixed bound such as `1024`) and a doubling work chunk. Keep `bootstrap_success_depth` and `bootstrap_fail_depth` for `find_ppv` and `refine_sppv`.

```rust
pub fn solve_outcome(&mut self, pos: &mut Position) -> Outcome {
    self.begin_run();
    self.proof_mode = ProofMode::Outcome;

    let mut outcome = Outcome::Draw;
    let mut chunk = 500_000u64;
    let mut success_depth: Option<u32> = None;
    let mut fail_depth = 0u32;

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
                success_depth = Some(u32::MAX);
            }
            break;
        }

        fail_depth = fail_depth.max(chunk.trailing_zeros() as u32 + 1); // placeholder
        chunk = chunk.saturating_mul(2);
        if chunk == u64::MAX {
            break;
        }
    }

    // If no decisive result was found within the work budget, fall back to an
    // unbounded search (or store the current best Draw).
    if success_depth.is_none() && !self.time_exceeded() {
        self.reset_search_state();
        self.tt.new_generation();
        self.reset_history_and_killers();
        outcome = self.dfpn(pos, INF, INF, u32::MAX, u64::MAX, true);
        if outcome != Outcome::Draw {
            if let Some(entry) = self.tt.probe(pos.hash())
                && entry.outcome.is_some()
            {
                success_depth = Some(entry.depth);
            }
            if success_depth.is_none() {
                success_depth = Some(u32::MAX);
            }
        }
    }

    self.bootstrap_success_depth = success_depth;
    self.bootstrap_fail_depth = fail_depth;
    outcome
}
```

The `fail_depth` bound should ideally come from the deepest fully searched ply observed during the work chunks (e.g. the `remaining_depth` stored in the root entry). A simple first implementation can set `fail_depth = 0` and rely on `refine_sppv` to search upward from `success_depth`.

#### 1.2 `refine_sppv` uses binary search on depth

Replace the decremental `probe = hi - 1` loop with binary search on the interval `[lo, hi]`, because the predicate "a win exists in `d` plies" is monotonic in `d`.

```rust
while hi > lo + 1 && !self.time_exceeded() {
    let probe = lo + (hi - lo) / 2;
    let mut chunk = 500_000u64;
    let mut proved_at_probe = false;

    for _ in 0..4 {
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
            hi = probe;
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
```

The `for _ in 0..4` retry per `probe` may be unnecessary once `max_work` enforcement is robust; it can be reduced to a single call or kept as a safety net.

### 2. `src/search/dfpn/core.rs`

#### 2.1 Harden `max_work` enforcement

The current code computes `child_max_work = max_work.saturating_sub(self.child_evals - child_evals_start)` and passes it down. When the remaining budget reaches zero, deeper calls still evaluate one more level. Add an explicit short-circuit before expanding a child:

```rust
let work_spent = self.child_evals - child_evals_start;
if max_work != u64::MAX && work_spent >= max_work {
    break;
}
let child_max_work = max_work.saturating_sub(work_spent);
```

This guarantees that a work-bounded `dfpn` call returns cleanly once its budget is consumed, rather than recursing on a zero budget.

#### 2.2 Store a clean work-cutoff result

When breaking due to `max_work`, store `outcome = None` with the current `pn`/`dn` and `remaining_depth` unchanged. This is already the intended behavior, but verify that the `store` call at the end of `dfpn` does not accidentally store a partial `Win`/`Loss` when the break is due to budget exhaustion.

### 3. `src/search/dfpn/children.rs`

#### 3.1 Tighten `evaluate_child` TT reuse

Ensure `evaluate_child` does not reuse unsolved `pn`/`dn` bounds from a TT entry whose `remaining_depth` is `u32::MAX` or larger than the current `child_max_depth`. The current code already rejects `u32::MAX` and requires `remaining_depth <= child_max_depth`; this plan only requires keeping that invariant while `max_depth` is unbounded during the bootstrap.

### 4. `src/main.rs`

When `--no-refine-shortest` is given, the CLI can optionally call `Search::solve()` directly (unbounded or with a single work chunk) instead of `solve_outcome` + `find_ppv`. This avoids the refinement-oriented bootstrap when the user only wants any proof PV. This is optional and can be deferred if `find_ppv` is already fast enough.

### 5. `examples/benchmark.rs`

Optionally add a work-chunk parameter or update the benchmark driver to use `solve_outcome` so the new behavior is exercised by the benchmark suite.

## Verification

- `cargo test` and `cargo test --release` pass.
- `cargo run --release -- --fen "$fen1" --timeout 60` returns the expected decisive outcome instead of timing out, where `$fen1` is the `max_depth=8` horizon regression from `ultimattt` plan 4 or the m24 FEN.
- `cargo run --release -- --fen "$fen2" --timeout 60` remains fast for a shallow mate.
- `m24_ppv` (release, `--ignored`) still passes within 60 s.
- `examples/benchmark.rs` shows no regressions.

## Risks and mitigations

- **Work budget explosion.** With `max_depth = u32::MAX` and a work chunk of 500k, the first chunk could over-expand a deep but irrelevant line. Mitigation: the explicit `max_work` short-circuit and the `explored` flag prevent re-expansion of exhausted children; the chunk doubles each iteration, so subsequent iterations focus on the most-proving lines.
- **Binary search `refine_sppv` may waste time on failed probes.** A failed depth probe returns `Draw`; the binary search still narrows the interval. The retry loop with doubling chunks reduces false negatives due to insufficient work.
- **`bootstrap_fail_depth` becomes less meaningful.** Without a depth schedule, `fail_depth` is no longer the largest known-failing depth. `refine_sppv` can start with `lo = 1` or `lo = 0` and `hi = bootstrap_success_depth`. This is safe but slightly slower; future work can track the deepest fully searched depth from the TT.
- **Interaction with `find_ppv`.** `find_ppv` currently uses `bootstrap_success_depth` as `max_depth`. With `solve_outcome` returning `u32::MAX`, `bootstrap_success_depth` may also be `u32::MAX` if no precise depth is recorded. If that happens, `find_ppv` should either run unbounded or cap `max_depth` from the actual root TT entry `depth`. The plan relies on reading `entry.depth` from the decisive root TT entry.

## Summary

1. Convert `solve_outcome` from a depth schedule to a pure work-doubling loop with `max_depth = u32::MAX`.
2. Harden `max_work` enforcement in `dfpn` so work-bounded calls stop cleanly.
3. Convert `refine_sppv` from decremental search to binary search on depth.
4. Preserve the existing `find_ppv` / `extract_pv` depth-aware extraction.
5. Verify on the m24 regression and benchmark suite.
