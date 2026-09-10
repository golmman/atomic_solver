# Plan: Report Refinement-Round Termination Status (cap-cut vs. natural exhaustion)

## Summary

The refinement loop in `Search::solve_with_progress` runs bounded rounds that
try to prove a shorter decisive line than the one already found. Each round
ends in one of two ways that the rest of the program cannot tell apart:

1. **Natural exhaustion** — `bounded_search`'s work-chunk loop observed
   `work_done < call_max_work`: the bounded tree at `bound = pv_len - 2` was
   fully explored without a decisive line. _No win of length ≤ bound exists_
   (within the solver's search semantics), so the current PV is **proven
   shortest**.
2. **Cap-cut** — the per-round work cap (`--refine-cap`) or a global resource
   budget stopped the round before exhaustion. The result is _unknown_: a
   shorter win may still exist.

Today both produce `Draw` with unsolved TT stores, refinement stops, and
nothing in the public API or the CLI output distinguishes the two. A user who
sees a converged-looking PV cannot know whether it is proven optimal or merely
the best line found before the cap fired.

This plan makes `bounded_search` report _why_ it returned without a decisive
line, threads that status through the refinement loop, exposes it as a
`Search` accessor, and prints it from the CLI. Search decisions are unchanged.

## Goal and Scope

### Goal

1. `bounded_search` distinguishes `Decisive`, `Exhausted`, `CapCut`, and
   `ResourceCut` terminations and reports which one happened.
2. `solve_with_progress` maps the last round's termination onto a public
   `Search::pv_status()` result that answers exactly one question: _is the
   returned PV proven shortest, or not?_
3. The CLI prints a `pv_status:` line for decisive outcomes.
4. Zero change to search behavior: identical outcomes, identical PVs,
   identical `child_evals` trajectories.

### Non-goals

- **Continuing after a cap-cut round.** "On cap-cut with time left, one could
  argue for continuing" changes search behavior and interacts with the
  deterministic `child_eval_budget` contract. That is a separate plan; this
  plan only makes the two cases _visible_.
- **Changing the benchmark JSON** or `docs/spec/optimizer_interface.md`. The
  optimizer contract is normative and externally copied; the status is a
  human-facing diagnostic.
- **Changing `ExitReason`.** `ExitReason` describes why the _whole search_
  stopped (timeout/quit/memory/budget/complete); the round status describes
  why one _bounded call_ returned `Draw`. They are different axes and stay
  separate types.
- **Proof-tree changes.** The `ProofEvent` protocol and the proof-tree layer
  are untouched. The "proven" here is a property of the search's own bounded
  round, not a PPV validation (pv plan7 moved PV validation out of `Search`
  deliberately; see Semantics below).

## Design

A flow diagram of the whole status path (`bounded_search` break sites →
`RoundTermination` → `PvStatus` → CLI labels) lives in
[`docs/diagrams/status_flow.mmd`](../../diagrams/status_flow.mmd); the
prose below is normative.

### 1. `bounded_search` reports its termination cause

Change the private signature:

```rust
/// Why `bounded_search` returned. Only meaningful when `outcome == Draw`
/// for the non-`Decisive` variants; a decisive return always reports
/// `Decisive`.
enum RoundTermination {
    /// A decisive outcome was found.
    Decisive,
    /// The bounded tree at `max_depth` was fully explored with no decisive
    /// line (`work_done < call_max_work` on the final chunk).
    Exhausted,
    /// The per-round work cap (`round_work_cap`) stopped the round before
    /// exhaustion (`call_max_work == 0` reached via the round cap).
    CapCut,
    /// Wall time, the stop flag, the memory flag, or the global
    /// child-eval budget stopped the round before exhaustion.
    ResourceCut,
}
```

Mapping of the existing break sites in `bounded_search`:

| Break site                                                        | Today     | Becomes                                                                                                |
| ----------------------------------------------------------------- | --------- | ------------------------------------------------------------------------------------------------------ |
| `outcome != Outcome::Draw`                                        | break     | `Decisive`                                                                                             |
| `call_max_work == 0` (round cap or global budget)                 | break     | `CapCut` when the binding constraint is `round_remaining`, `ResourceCut` when it is `remaining_budget` |
| loop condition `time_exceeded()` / `child_eval_budget_exceeded()` | loop exit | `ResourceCut`                                                                                          |
| `work_done < call_max_work`                                       | break     | `Exhausted`                                                                                            |

Note the mixed break site: `call_max_work == 0` can be caused by either the
round cap or the global budget (`min` of the two). The implementation must
compare `round_remaining` and `remaining_budget` (or record which one is
smaller when computing `call_max_work`) so the two causes are not conflated.

#### Enforcement mechanics and edge cases (context for the mapping)

The budget is never checked directly inside `dfpn`; it acts only through the
`max_work` plumbing (`bounded_search` folds `remaining_budget` into
`call_max_work`, the child loop breaks on
`child_evals - child_evals_start >= max_work`, and recursive frames receive
the remaining work). Consequences the implementation must respect:

- **Overshoot granularity.** Work checks sit between loop iterations and
  `evaluate_all_children` classifies a node's remaining children in one
  batch, so `child_evals` can overshoot the budget or round cap by up to one
  node's branching factor before any check fires. Status mapping must not
  assume exact-count termination.
- **Timeout asymmetry.** The stop/timeout/memory flag is checked at the top
  of every `dfpn` loop iteration (`core.rs`), while the budget only ever cuts
  via `max_work`; a `Quit`/timeout can interrupt a frame where a budget cut
  could not.
- **Conservative exhaustion.** If the bounded tree is exhausted *and* the
  clock expires during the same final chunk, the `while` condition exits the
  loop first and the round is reported `ResourceCut`, not `Exhausted`. This
  is the intended direction (never claim exhaustion when a resource cut also
  fired) and must be preserved.

The non-refinement entry points (`search_depth`,
`search_depth_with_prefix`, the first-outcome phase) ignore the status or
expose it unchanged; their signatures do not change.

### 2. Public status on `Search`

`solve`/`solve_with_progress` keep their `(Outcome, Vec<Move>, u64)`
signatures. The status is exposed as an accessor, reset in `begin_run`:

```rust
/// How the PV of the last solve relates to optimality.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PvStatus {
    /// No decisive outcome: there is no PV to qualify.
    None,
    /// PV came from the first-outcome phase (including
    /// `first_outcome_only` runs). Length is informational.
    FirstOutcome,
    /// The last refinement round ended in natural `Exhausted` exhaustion
    /// at `bound = pv_len - 2`, or the loop exited with `pv_len <= 2`
    /// (a 1-move win cannot be shortened). Within the solver's search
    /// semantics, no shorter win exists.
    ProvenShortest,
    /// The last refinement round was cap-cut or resource-cut, or ended
    /// decisive-but-not-shorter. A shorter win may exist.
    Unproven,
}
```

Loop-to-status mapping in `solve_with_progress` (normative):

| Refinement loop exit                                                                                          | `PvStatus`                            |
| ------------------------------------------------------------------------------------------------------------- | ------------------------------------- |
| Last round returned `Draw` + `Exhausted` at `bound = n - 2`                                                   | `ProvenShortest`                      |
| Loop condition `n <= 2` reached after an improving round                                                      | `ProvenShortest` (trivially shortest) |
| Last round returned `Draw` + `CapCut` / `ResourceCut`                                                         | `Unproven`                            |
| Last round decisive but not shorter (defensive branch)                                                        | `Unproven`                            |
| Loop condition `time_exceeded()` / budget exceeded fires _between_ rounds (last round was decisive-improving) | `Unproven`                            |
| `first_outcome_only`, or no improving round ever ran                                                          | `FirstOutcome`                        |
| Final outcome `Draw`                                                                                          | `None`                                |

The two-variant distinction the motivating note asks for is
`ProvenShortest` vs. everything else; `FirstOutcome` and `Unproven` are kept
apart because the CLI can word them differently ("first outcome" vs.
"cap-cut").

### 3. CLI output

After the existing `pv:` line, for decisive outcomes print:

```text
pv_status: proven-shortest | first-outcome | cap-cut | cut-short
```

Mapping: `ProvenShortest → proven-shortest`, `FirstOutcome → first-outcome`,
`Unproven → cap-cut` when the last round was `CapCut`, `cut-short` otherwise
(`ResourceCut` or defensive branches). Draw outcomes print no `pv_status`
line.

This is an **additive** stdout change. The lean initiative's drift protocol
("m22 first-outcome stdout byte-identical") does not apply to this plan by
construction; the plan's own drift check is that the _pre-existing_ lines are
byte-identical and only the new line appears.

### Semantics: what "proven shortest" means here

This must be worded carefully in code docs, because pv plan7 deliberately
moved PV validation out of `Search`:

- A naturally exhausted bounded round is a complete exploration of the bounded
  tree at `bound`. It proves _no decisive line of length ≤ bound exists_ with
  exactly the same trust level the solver's `outcome: win` itself has: the
  solver's search semantics (TT base entries, repetition-Draw non-caching,
  epsilon thresholds) are presumed correct. It is not an independent PPV
  validation of the returned move sequence, and it does not consult the
  proof-tree layer.
- Consequently the label must not be understood as "this move sequence is a
  verified proof" — only "no shorter win exists". The doc comment on
  `PvStatus::ProvenShortest` says exactly that, and the CLI label
  `proven-shortest` refers to _length_, not to PV validity.

### 4. Documentation

- `AGENTS.md` "Output priorities" / CLI section: mention `pv_status` and the
  `PvStatus` accessor, one or two sentences, matching the existing style.
- `Search::solve_with_progress` doc comment: state that the returned PV is
  proven shortest iff `pv_status() == ProvenShortest`.

## Affected code

- `src/search/dfpn/mod.rs` — `RoundTermination`, `PvStatus`,
  `bounded_search` signature, refinement loop bookkeeping, accessor,
  `begin_run` reset. (The file already carries a >10 KB justification in its
  header; the addition is ~60 lines and stays within the 20 KB split
  guideline.)
- `src/main.rs` — print `pv_status:`.
- `src/search/dfpn/tests.rs` — new tests (below).

## Tests

Fast tier (no `#[ignore]`), extending the existing refine-cap tests:

1. **Exhaustion is reported.** A known mate position searched with
   `search_depth` at a `max_depth` below the mate length returns
   `Draw` + `Exhausted` (assert via the new internal status or the public
   accessor after a `solve` that converges by exhaustion).
2. **Converged fixture is `ProvenShortest`.** `REFINE_FIXTURE_FEN` with
   default settings (default cap, 5 s timeout) converges to the 48-ply PV;
   assert `pv_status() == ProvenShortest`. (The final round must have
   exhausted naturally; if the fixture turns out to converge via
   decisive-not-shorter or a cap-cut on the reference host, pick a bound /
   budget that forces natural exhaustion — e.g. `set_child_eval_budget` — so
   the test is deterministic.)
3. **Cap-cut is reported.** The `refine_cap_bounds_round_work` setup
   (`set_refine_round_cap_for_test(1_000)`) yields
   `pv_status() != ProvenShortest` with the `cap-cut` cause.
4. **First outcome.** `set_first_outcome_only(true)` yields `FirstOutcome`.
5. **Draw.** A drawn position yields `PvStatus::None` and no `pv_status`
   CLI line (CLI covered by an integration assertion if one exists for
   stdout; otherwise the unit-level mapping suffices).
6. **No search drift.** Existing tests (`solve_twice_identical_counts`,
   `default_refine_cap_leaves_improving_rounds`, chunk-trajectory tests) pass
   unchanged — the refactor must not alter any search decision.

## Verification

1. `cargo fmt --check`, `cargo clippy --all-targets`, `make test` (fast gate).
2. Drift check: `benchmark --suite quick --json --first-outcome` before vs.
   after — identical `child_evals` per case (status reporting must not touch
   search decisions).
3. m22 default-mode run before vs. after: identical outcome, identical PV,
   identical chunk trajectory; stdout identical except the added
   `pv_status:` line.
4. Manual CLI check on the dec44 fixture: the converged run prints
   `pv_status: proven-shortest`; a run with a tiny `--refine-cap` (e.g.
   `--refine-cap 0.001`) prints `cap-cut`.

## Report

The final task of this plan is writing `docs/plans/dfpn/report8.md`:
what shipped, the fixture used for the `ProvenShortest` test (and any
adjustment needed to make it deterministic), tools/examples used, problems
encountered, unresolved parts, missing tests, and next steps (the natural
follow-up being the "continue after cap-cut" plan, if ever justified).
