# Lean Plan 1 — Refinement accounting + tail cap + TT probe consolidation

Implements items **1, 1a, and 4** from `docs/plans/lean/initiative.md`.
Scope is deliberately small: two search-behavior changes (one intentional,
one behavior-preserving) plus metric plumbing, all verifiable with the
fast gate and one live benchmark run.

## Context

`Search::solve_with_progress` (`src/search/dfpn/mod.rs:385-425`) works in
two phases:

1. `bounded_search(pos, u32::MAX)` — first decisive outcome, work-chunked
   with geometric growth (chunks restart at 500k per call).
2. Unless `first_outcome_only`, a loop tightens the bound by two plies
   (`bound = n - 2`) and calls `bounded_search` again, stopping when a
   round produces no shorter PV, `n <= 2`, or time/budget runs out.

Live measurement on `m22_white` (`--timeout 20`, default mode,
2026-09-07): first win ~9.9 s, refinement 155→153→23 plies by ~11.1 s
(~2M evals), then **~8.9 s (44% of wall) spent on a refinement round at
bound 21 that can never succeed** — the depth-bounded tree is huge, so
the round's chunk loop keeps growing (500k → 16M) until the global
timeout fires. `--first-outcome` finishes in 10.3 s.

Two implementation facts drive the design:

- A refinement round ends in one of three ways: a decisive shorter PV
  (success), `new_outcome == Draw` via genuine tree exhaustion
  (`bounded_search`'s `work_done < call_max_work` break, mod.rs:315-321),
  or the global timeout/budget. The futile case is the third: the round
  never exhausts its bounded tree, so nothing but the global deadline
  stops it.
- `TtEntry` is `Copy` with `pub(crate)` fields
  (`src/search/tt/entry.rs:22-36`), and `probe_summary`/`probe_best_move`
  are thin maps over the same `probe()` slot selection
  (`src/search/tt/table.rs:51-85`). Consolidating probes is purely
  mechanical and behavior-preserving.

### Accounting correction (important)

An earlier audit suspected that refinement work is invisible in the
reported node/eval counts because `begin_run` resets the counters. That
is **wrong**: `begin_run` is called once per `solve`, never per
refinement round, and the live run confirms `nodes`/`child_evals`
accumulate across rounds (visible in the stderr chunk logs). There is no
reset bug. Item 1a therefore *locks in* the existing cumulative
semantics with tests and *exposes* the per-phase split, which the
benchmark/optimizer currently cannot see.

## Decisions

1. **The refinement cap is work-based, not wall-clock based.** Each
   refinement round gets a deterministic per-round child-eval cap. When
   the cap is exhausted without a shorter decisive line, the round is
   abandoned and refinement stops. This keeps
   `ExitReason::BudgetExhausted` semantics and the deterministic budget
   contract intact and makes runs reproducible.
2. **Cap formula (starting point, tuned in report1):**
   `round_cap = max(MIN_REFINE_ROUND_EVALS, factor * first_outcome_evals)`
   with `MIN_REFINE_ROUND_EVALS = 1_000_000` and `factor = 0.25`.
   Rationale: on m22_white the improving rounds each spent ≤1M evals,
   while first-outcome work was ~17M evals — a 4.25M per-round cap cuts
   the futile round at ~4M instead of ~15M while leaving genuine
   improvements untouched. The floor keeps small searches (tiny
   first-outcome work) effectively uncapped.
3. **Escape hatch:** `--refine-cap <FACTOR>` on the CLI, with `0`
   meaning unlimited (pre-change behavior). Default `0.25`. This is what
   makes the report's A/B measurement and the bit-identical drift check
   possible.
4. **TT consolidation is behavior-preserving.** One `probe()` per node
   and per child evaluation; `previous_summary` and `best_from_tt` are
   derived from the copied entry. The `Position` clone in the one-ply
   repetition guard is replaced by a `do_move`/`undo_move` round-trip on
   the actual position (net-zero side effect; `undo_move` is the exact
   inverse and is used everywhere else on the hot path).
5. **No changes to** move ordering, DF-PN threshold math, proof events,
   the TT store/replacement policy, or `docs/spec/` contracts. The
   benchmark example does not get a `--refine-cap` flag in this plan
   (its A/B runs use the CLI binary; adding it to `benchmark` is left
   for a later plan if the optimizer ever needs it).
6. Refinement-cap state lives on `Search` (not passed through `dfpn`
   arguments): the cap is enforced where `bounded_search` computes
   `call_max_work`, exactly like the existing global budget.

## Implementation steps

### Step 1 — Refinement work cap (`src/search/dfpn/mod.rs`)

Fields on `Search` (near `child_eval_budget`, mod.rs:100):

```rust
refine_cap_factor_num: u64,   // fraction_from_f64(0.25)
refine_cap_factor_den: u64,
refine_cap_min: u64,          // 1_000_000, 0 disables capping entirely
first_outcome_evals: u64,     // captured after phase 1
refinement_rounds: u32,       // diagnostics
refinement_evals: u64,        // diagnostics
```

Public API:

- `pub fn set_refine_cap_factor(&mut self, factor: f64)` — asserts
  `factor >= 0.0`; `0.0` stores `refine_cap_min = 0` semantics: simplest
  is to treat factor `0.0` as "cap disabled" by setting
  `refine_cap_min = u64::MAX` internally (document this in the doc
  comment; a separate `set_refine_cap_disabled()` is not needed).
- `#[must_use] pub fn first_outcome_evaluations(&self) -> u64`
- `#[must_use] pub fn refinement_rounds(&self) -> u32`
- `#[must_use] pub fn refinement_evaluations(&self) -> u64`

In `solve_with_progress` (mod.rs:385-425), after phase 1:

```rust
self.first_outcome_evals = self.child_evals;
```

Inside the refinement loop, before each `bounded_search` call, compute
the per-round cap and pass it down; track per-round work via the
`child_evals` delta after the call:

```rust
let round_cap = if self.refine_cap_min == u64::MAX {
    u64::MAX
} else {
    let scaled = (self.first_outcome_evals as u128
        * self.refine_cap_factor_num as u128
        / self.refine_cap_factor_den as u128) as u64;
    scaled.max(self.refine_cap_min)
};
```

Add a parameter `round_work_cap: u64` to `bounded_search` (mod.rs:296)
and clamp inside the chunk loop:

```rust
let round_remaining = round_work_cap.saturating_sub(self.child_evals - round_evals_start);
let call_max_work = chunk.min(remaining_budget).min(round_remaining);
```

(round_evals_start captured at loop entry, next to
`last_child_evals_before`.) All existing callers of `bounded_search`
(`search_depth` mod.rs:345, `search_depth_with_prefix` mod.rs:365) pass
`u64::MAX` — those entry points have no refinement loop and are
unchanged.

When a round ends because `work_done` hit the round cap (i.e. the round
neither proved a shorter line nor exhausted its tree), the existing
`if new_outcome == Outcome::Draw || new_pv.len() as u32 >= n { break; }`
(mod.rs:415-417) already stops refinement: a cap-cut round returns
`Draw` with unsolved TT stores, indistinguishable from a work-chunk
cutoff. **No new break condition is needed** — only the clamp. Record
the round as non-improving for diagnostics:

```rust
self.refinement_rounds += 1;
self.refinement_evals = self.child_evals - self.first_outcome_evals;
```

(increment `refinement_rounds` for every round attempted, successful or
not; `refinement_evals` is simply recomputed after each round).

Semantics checklist:

- `first_outcome_only = true`: loop never entered; cap irrelevant.
- Root `Draw`: loop never entered (existing `outcome != Outcome::Draw`
  guard).
- Global `child_eval_budget`: the clamp `min(remaining_budget)` keeps
  precedence; `ExitReason::BudgetExhausted` reporting is unchanged
  (mod.rs:286-287).
- Timeout: unchanged precedence; a round can still end early on wall
  time, that is fine (the cap only adds a *deterministic* earlier stop).
- Returned outcome/PV on cap-stop: same as any non-improving round —
  the previous (best so far) outcome and PV are returned.

### Step 2 — CLI flag (`src/main.rs`, `AGENTS.md`)

- Parse `--refine-cap <FACTOR>` (f64, default `0.25`, `0` = unlimited),
  mirroring how `--epsilon` is parsed/validated; unknown-value errors
  exit with an error like the other options. Call
  `search.set_refine_cap_factor(v)`.
- Update `-h/--help` text and the `main.rs` paragraph in `AGENTS.md`
  (flag list) accordingly.

### Step 3 — Refinement accounting exposure + tests (item 1a)

The accessors from Step 1 are the exposure. Add unit tests in
`src/search/dfpn/tests.rs` (or a new `#[cfg(test)]` block in `mod.rs`
if `tests.rs` organization prefers):

1. `refinement_counters_accumulate` — solve a position that triggers at
   least one refinement round (see Step 6 for how to find one; hardcode
   the FEN once found), assert:
   - `search.first_outcome_evaluations() > 0`,
   - `search.refinement_rounds() >= 1`,
   - `search.child_evaluations() >= search.first_outcome_evaluations()`
     (cumulative semantics locked),
   - with `set_refine_cap_factor(0.0)` (unlimited) the same position
     yields `refinement_rounds() >= 1` too (behavior preserved).
2. `refine_cap_bounds_round_work` — same position, cap factor small
   enough that the cap binds (e.g. `set_refine_cap_factor(0.0)` then a
   hand-set tiny `refine_cap_min` via a `#[cfg(test)]` helper or by
   scaling with the factor): assert
   `refinement_evaluations()` stays below
   `first_outcome_evaluations() * 0.25 + MIN_REFINE_ROUND_EVALS + slack`
   — the exact assertion depends on which lever is settable in tests;
   if `refine_cap_min` is not exposed, expose a
   `#[cfg(test)] pub(crate)` setter rather than public API.
3. `refine_cap_zero_disables_capping` — factor `0.0` run equals the
   pre-change refinement round count on the same position (guards
   decision 3's escape hatch).
4. Existing budget tests (`tests/test_epsilon.rs`,
   `ExitReason::BudgetExhausted` suites) must pass unchanged.

Keep the position small so all three tests stay in the fast tier
(target: < 1 s each, release). If the smallest refinement-exhibiting
position is too slow, mark that one test
`#[ignore = "slow: ..."]` per AGENTS.md — but prefer finding a fast one.

### Step 4 — TT probe consolidation (item 4a, behavior-preserving)

Node level, `src/search/dfpn/core.rs`:

- Replace the triple at core.rs:95-102
  (`try_use_tt` → `probe_summary` → `probe_best_move`) with a single
  `let tt_entry = self.tt.probe(tt_key);` copied into a local
  `Option<TtEntry>` (`TtEntry` is `Copy`, entry.rs:22-36; copying also
  ends the borrow before the later `self.tt.store`).
- Restructure `try_use_tt` (core.rs:309-328) into an entry-based helper,
  e.g. `fn resolved_from_entry(entry: &TtEntry, max_depth: u32) ->
  Option<Resolved>` for the depth checks (core.rs:312-314), keeping the
  one-ply guard separate so `evaluate_child` can reuse it without a
  second probe.
- Derive `previous_summary` (consumed at core.rs:151-158) and
  `best_from_tt` (core.rs:102) from the copied entry, replicating
  `probe_best_move`'s exact rule (table.rs:78-85: return
  `Some(best_move)` iff `outcome.is_some() || best_move !=
  Move::NONE`).
- One-ply guard without the clone (core.rs:316-322): replace
  `let mut child = pos.clone(); child.do_move(entry.best_move); ...
  path_stack.contains(&child.repetition_key())` with

  ```rust
  pos.do_move(entry.best_move);
  let rep = pos.repetition_key();
  pos.undo_move(entry.best_move);
  ```

  This requires `try_use_tt`/the new helper to take `&mut Position`
  (and the caller already holds `&mut pos`). Verify via the existing
  GHI suites (`tests/test_ghi.rs`, `tests/test_repetition.rs`,
  `tests/test_transpositions.rs`) that results are bit-identical.

Child level, `src/search/dfpn/children.rs:113-124`:

- `evaluate_child` currently does `try_use_tt` (one `probe`) then
  `probe_summary` (a second `probe`) on the same key. Probe once: copy
  the entry, run the depth checks + one-ply guard, and derive the
  unsolved-bounds reuse decision (children.rs:131-135: `outcome.is_none()
  && pn > 0 && dn > 0 && remaining_depth != u32::MAX &&
  remaining_depth <= child_max_depth`) from the same entry.

Cleanup: after the refactor, `TranspositionTable::probe_summary` and
`probe_best_move` should have no remaining callers — check with
`cargo build` + grep; remove them if (and only if) nothing else uses
them, keeping `probe()` and `store()`. Update `src/search/tt/` tests
that may exercise the removed methods.

### Step 5 — Update AGENTS.md

Beyond the CLI flag list (Step 2): the `src/search/dfpn/` bullet in the
Architecture section mentions the solver's phases; add one sentence
noting the per-round refinement cap (`--refine-cap`, default 0.25, `0`
disables) so the doc stays accurate. Do not touch `docs/spec/`.

### Step 6 — Find the refinement test position (do this first)

Before coding Step 3's tests, identify a **fast** fixture that triggers
at least one improving refinement round:

```sh
cargo build --release --examples
# try decisive fixtures with default mode, pick the smallest PV drop:
for fen in ...; do
  echo "$fen" | target/release/atomic_solver --fen "$fen" --timeout 3 --outcome-only
done
```

Candidates to try first: the `dec*`/`rem*` entries in
`tests/fixtures/decisive_positions.txt` and `decisive_remaining.txt`
(any case whose printed PV is shorter than the first `outcome: win
length:` line qualifies). Record the chosen FEN and its
before/after behavior in report1. If none qualifies within a 3 s
timeout, fall back to `m23_white` with `#[ignore = "slow: ..."]` and
say so in the report.

## Verification

```sh
cargo fmt --check
cargo clippy --release --all-targets
make test
```

Bit-identical drift checks (search-order preservation):

```sh
# baseline BEFORE touching src/ (main, release build):
target/release/examples/benchmark --suite quick --json --first-outcome > /tmp/opencode/lean_before_fo.json
target/release/examples/benchmark --suite quick --json --refine-cap... # n/a: use CLI instead
target/release/atomic_solver --fen <m22_white FEN> --timeout 20 --refine-cap 0 --outcome-only 2>&1 | tail -3
```

After the change, `--suite quick --json --first-outcome` must produce
**identical `child_evals` per case** (TT consolidation + accounting must
not perturb the search), and the m22_white `--refine-cap 0` run must
reproduce the baseline eval count exactly (uncapped default path
unchanged). The intentional behavior change is only visible in default
mode:

```sh
time target/release/atomic_solver --fen "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22" \
  --timeout 20 --outcome-only --dump-path /tmp/opencode/lean_after.bin
```

Expected: same outcome and PV as baseline, total wall ~13–15 s instead
of ~20 s (futile round cut at ~4M evals). Record actual numbers in the
report. Note the binary is written to `proof_tree.bin` by default —
keep `--dump-path` pointed outside the repo tree or clean up after.

Because this plan touches search and TT internals, run the full tier
once before closing:

```sh
make test-full   # ~25 min, required for search/TT changes per AGENTS.md
```

## Problems to watch for

- The `do_move`/`undo_move` guard swap changes `try_use_tt`'s signature
  (`&self` → `&mut self`); check all call sites (core.rs, children.rs)
  compile cleanly — if a `&self` context cannot be upgraded, keep the
  clone there and note it in the report (the guard only runs on solved
  entries with a best move, so the win is bounded).
- Copying `TtEntry` into a local must happen before any `&mut self.tt`
  use in the same scope (borrow checker will enforce).
- The refinement cap must not change the **first-outcome** phase: all
  first-outcome work is untouched (cap applies only inside the
  refinement loop's `bounded_search` calls).

## Out of scope (next plans)

- Movegen deduplication, allocation removal, scorer cost (#3/#6/#8 →
  plan2, one profiling pass).
- Parallelism (#2 → plan3, determinism design required).
- AND-side ordering signal (#5), lazy child evaluation (#7),
  tablebases (#9), history/killer retuning (#10).

## Final task

Write `docs/plans/lean/report1.md`: the chosen refinement-test position
and its measurements, before/after benchmark JSON drift check results,
the m22_white A/B numbers (default mode, uncapped, first-outcome),
deviations from this plan, problems encountered, missing tests, and
next steps. Put raw measurement outputs under
`docs/plans/lean/measurements/plan1/` (follow the layout convention of
`docs/plans/nn/measurements/`).
