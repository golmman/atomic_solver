# Plan 1: Node-classification profiler

Initiative: `research` backlog #10 (POC candidate), feeding backlog #1
(problem inventory).

## Goal

Build a temporary instrumentation pass that classifies every child
evaluation and every `dfpn` frame into a coarse category, run it on the
two validation cases (`m22_white` and the stress case), aggregate the
partition, and revert the instrumentation. The report answers: of all
child evals spent to reach the first decisive outcome, what fraction is
OR decisive-child work, AND refutation work, threshold-cut churn,
TT-resolved reuse, path-repetition draws, fast-terminal exits, and
preflight hits?

This is a throwaway POC: no permanent `src/` change survives the plan.

## Context

`lean` plan9's frame-level counters gave a coarse partition (99.7% of
AND-frame own evals in threshold-cut frames), but it could not separate:
- individual child evals that returned from a TT hit vs. those that
  actually searched,
- OR-node decisive-child evals vs. sibling evals,
- AND-node refuting-child evals vs. sibling evals,
- evals inside frames that eventually solved vs. frames that eventually
  threshold-cut.

A sharper profiler lets us rank the remaining open levers (#7 lazy/staged
evaluation, #10 history/killer retuning, #12 TT eviction, #13 frontier
priors, `conversion` #4 parallel) by which unmeasured surface is actually
largest.

## Categories

### Child-eval level (`evaluate_child`)

Increment one counter per call based on the exact return path taken:

| Counter | Trigger | Notes |
|---------|---------|-------|
| `tt_solved` | TT probe returned a solved outcome (depth check + one-ply guard passed) | Reuse, no search |
| `path_rep` | `path_contains(child_rep_key)` fired before TT probe | Reuse, no search |
| `terminal_fast` | One of the fast terminal branches: commoner extinction (either side), rule50-expired terminal, occupied==2 draw | No search |
| `preflight` | Pre-flight phase handled this position (if reachable) | Rare in first-outcome after preflight fires at root |
| `searched_or_win` | Recursive `dfpn` returned `Win`, parent is OR node | Decisive child at OR |
| `searched_or_loss` | Recursive `dfpn` returned `Loss`, parent is OR node | Non-decisive sibling at OR |
| `searched_or_draw` | Recursive `dfpn` returned `Draw`, parent is OR node | Non-decisive sibling at OR |
| `searched_and_loss` | Recursive `dfpn` returned `Loss`, parent is AND node | Decisive refuter at AND |
| `searched_and_win` | Recursive `dfpn` returned `Win`, parent is AND node | Non-decisive sibling at AND |
| `searched_and_draw` | Recursive `dfpn` returned `Draw`, parent is AND node | Non-decisive sibling at AND |

The parent node type is `is_or_node` passed into `evaluate_child`. The
recursive outcome is the `Outcome` returned by `dfpn` in the unsolved
branch (the only branch that actually searches).

### Frame level (`dfpn`)

At frame entry, capture `child_evals_start`. At every frame exit,
compute `frame_evals = child_evals - child_evals_start` and add it to
one of three buckets based on why the frame returned:

| Bucket | Trigger |
|--------|---------|
| `frame_solved_evals` | Frame returned `Win`, `Loss`, or `Draw` (including fast-terminal, path-rep, TT-solved, and repetition-cache exits) |
| `frame_cut_evals` | Frame returned because `pn >= th_pn` or `dn >= th_dn` without a solved outcome |
| `frame_budget_evals` | Frame returned because `time_exceeded()` or `max_work` exhausted without a solved outcome |

A separate counter `frame_total` records the number of `dfpn` entries
(regardless of early exit path).

## Design decisions

1. **No compile-time feature gate.** The counters are plain `u64` fields
   added to `Search` and incremented unconditionally. The overhead of a
   handful of `u64` adds is negligible compared to the search work (single
   instructions, no atomics, no branches). The *reporting* is gated on a
   new `dump_profiler` flag or simply printed unconditionally at the end of
   `solve_with_progress` to stderr; it is harmless noise in normal runs
   and is reverted at plan close.
2. **Frame eval accounting uses `child_evals`.** Frame overhead
   (`nodes += 1`, `path_push`, `sort_moves`, threshold arithmetic) is *not*
   included in `frame_evals` — only the descendants' `evaluate_child` calls
   are. This matches the primary metric (`child_evals`) and separates
   control-flow overhead from "real" search work.
3. **Preflight hits are tracked at the `dfpn` entry point.** The
   pre-flight claim returns before `dfpn` is entered, so the profiler
   adds a counter in the caller (`solve_with_progress` or the
   pre-flight probe site) rather than inside `dfpn`.
4. **No per-position or per-depth histograms in this plan.** A follow-up
   spike could add depth or move-number histograms if the aggregate
   partition points at a specific class; keeping this plan to one session
   means one aggregate table per validation case.

## Implementation steps

### Step 1 — Add profiler fields to `Search` (`src/search/dfpn/mod.rs`)

Add a `Profiler` struct and a `profiler: Profiler` field on `Search`.
Initialize in `Search::new`. Zero in `begin_run`.

```rust
#[derive(Default)]
struct Profiler {
    // Child-eval counters
    child_total: u64,
    child_tt_solved: u64,
    child_path_rep: u64,
    child_terminal_fast: u64,
    child_preflight: u64,
    child_searched_or_win: u64,
    child_searched_or_loss: u64,
    child_searched_or_draw: u64,
    child_searched_and_loss: u64,
    child_searched_and_win: u64,
    child_searched_and_draw: u64,

    // Frame-level eval counters
    frame_total: u64,
    frame_solved_evals: u64,
    frame_cut_evals: u64,
    frame_budget_evals: u64,
}
```

Add a `Profiler::dump(&self)` method that prints the table to stderr in
a single `eprintln!` block (plain text, parseable by eye). Keep the
output self-contained so `report1.md` can copy the raw table verbatim.

### Step 2 — Instrument `evaluate_child` (`src/search/dfpn/children.rs`)

At every early-return site inside `evaluate_child`, increment the
corresponding counter *before* returning:

- `commoners empty` return → `profiler.child_terminal_fast += 1`
- `opponent commoners empty` return → `profiler.child_terminal_fast += 1`
- `path_contains` return → `profiler.child_path_rep += 1`
- `rule50_expired` branch (both checkmate and stalemate sub-branches) →
  `profiler.child_terminal_fast += 1`
- `resolved` from TT return → `profiler.child_tt_solved += 1`
- `occupied == 2` return inside `terminal_outcome` →
  `profiler.child_terminal_fast += 1`
- No-legal-move return inside `terminal_outcome` →
  `profiler.child_terminal_fast += 1`

At the *single* return site at the end of `evaluate_child` (after the
recursive `dfpn` call on the unsolved branch), the outcome is known:

```rust
let outcome = ...; // returned from dfpn
if is_or_node {
    match outcome {
        Outcome::Win => profiler.child_searched_or_win += 1,
        Outcome::Loss => profiler.child_searched_or_loss += 1,
        Outcome::Draw => profiler.child_searched_or_draw += 1,
    }
} else {
    match outcome {
        Outcome::Loss => profiler.child_searched_and_loss += 1,
        Outcome::Win => profiler.child_searched_and_win += 1,
        Outcome::Draw => profiler.child_searched_and_draw += 1,
    }
}
```

Also increment `profiler.child_total += 1` at the very top of
`evaluate_child` (next to `self.child_evals += 1`).

### Step 3 — Instrument `dfpn` frame accounting (`src/search/dfpn/core.rs`)

At the top of `dfpn`, after `self.nodes += 1`:

```rust
let frame_child_evals_start = self.child_evals;
self.profiler.frame_total += 1;
```

At every `return` site (there are many early returns and the final loop
break), compute `let frame_evals = self.child_evals - frame_child_evals_start;`
and add to the appropriate bucket based on the exit path:

- Early returns that carry a solved `Outcome` (terminal, path-rep,
  TT-solved, repetition-cache, recursive-solved) →
  `profiler.frame_solved_evals += frame_evals`
- The `break` from `pn >= th_pn || dn >= th_dn` (threshold-cut, no solved
  outcome stored) → `profiler.frame_cut_evals += frame_evals`
- The `break` from `time_exceeded()` or `max_work` exhausted (budget/timeout
  cut, no solved outcome) → `profiler.frame_budget_evals += frame_evals`

Because there are many early-return sites, refactor the exits to go
through a single `dfpn_return` helper if practical in one session; if
not, add the accounting inline at each exit (there are ~6 early exits
plus the final loop break — manageable).

### Step 4 — Instrument pre-flight hits (`src/search/preflight/mod.rs` or caller)

The pre-flight claim returns a solved result before `dfpn` is entered.
At the site in `solve_with_progress` where the pre-flight result is
consumed, add:

```rust
self.profiler.child_preflight += 1;
```

(Only if the pre-flight fires; this is a single site in
`src/search/mod.rs` or `src/search/dfpn/mod.rs`.)

### Step 5 — Add dump call at end of search (`src/search/dfpn/mod.rs`)

In `solve_with_progress`, after the final outcome is determined and
before returning, call `self.profiler.dump()` to stderr. This prints
once per `solve` call.

### Step 6 — Run validation cases

Build release and run both validation cases in first-outcome mode:

```bash
cargo build --release

# m22_white (from tests/fixtures/move_order_positions.txt)
time target/release/atomic_solver \
  --fen "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22" \
  --timeout 30 --first-outcome --outcome-only 2>&1 | tee /tmp/opencode/research_plan1_m22.txt

# Stress case
time target/release/atomic_solver \
  --fen "4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21" \
  --timeout 300 --first-outcome --outcome-only 2>&1 | tee /tmp/opencode/research_plan1_stress.txt
```

Also run the quick suite once to verify the profiler does not perturb
the search trajectory (bit-identical child_evals per case):

```bash
target/release/examples/benchmark --suite quick --json --first-outcome > /tmp/opencode/research_plan1_quick.json
```

Compare against a clean baseline (build from a temp stash or a backup
copy if the instrumentation changes are extensive).

### Step 7 — Revert instrumentation

Remove all profiler fields, increments, and the dump call. Verify clean
revert:

```bash
git diff --exit-code src/
```

If any profiler code remains, clean it up before closing.

## Verification

- `cargo fmt --check`
- `cargo clippy --release --all-targets`
- `make test` (fast gate — the instrumentation must not break tests)
- Quick-suite drift: `benchmark --suite quick --json --first-outcome` child_evals
  must match the clean baseline bit-identically per case.

## Non-goals

- No permanent `src/` change survives this plan.
- No new CLI flags, no API changes, no `docs/spec/` changes.
- No per-depth or per-move histograms (next plan if the aggregate points
  at a specific category).
- No attempt to optimize based on the profiler results in this plan.

## Out of scope

- Plan2 will be chosen from the POC candidates (#11–#13) or a new
  literature target (#5–#9) based on which category the profiler shows
  as largest and unmeasured.

## Final task

Write `docs/plans/research/report1.md`: include the raw profiler dump
for both validation cases, the derived percentages, a brief interpretation
of which category dominates, and the quick-suite drift check result. Put
raw output files under `docs/plans/research/measurements/plan1/`.
