# Correctness Review 4: `atomic_solver` after Plans 15–17

Date: 2026-07-18  
Scope: `src/{position,zobrist,search/{dfpn,tt,ordering},main}.rs`, the
implementation reports `docs/plans/review/report{15..17}.md`, the earlier
reviews `review1.md` through `review3.md`, and the research notes
`research_epsilon.md`, `research_ghi.md`, and `research_parallel.md`.

## Executive summary

Plans 15–17 successfully close the three highest-priority correctness gaps
from `review3.md`:

- `Position::outcome_from_state` now evaluates checkmate/stalemate before the
  `rule50` and two-piece draw heuristics.
- `extract_pv` follows path-dependent twin entries with the same 1-indexed
  move-depth arithmetic used by `dfpn`.
- The transposition table stores a `remaining_depth` budget, so a
  `max_depth == 0` cutoff Draw is never reused as a proven result for a
  larger depth.

`cargo fmt`, `cargo clippy --all-targets`, `cargo test --all-targets`,
`cargo test --release`, `cargo test --release --test test_ghi -- --ignored`,
`cargo test --release --test test_epsilon`, and `cargo doc --no-deps` all
pass. Manual CLI checks on the review FENs match the expected outcomes.

The solver is now correct on the tested corpus of single-commoner mates,
50-move terminals, transposition-heavy wins, cyclic rook/king draws, and
shortest-PV refinement. The only remaining material correctness risk is the
Graph-History-Interaction simulation, which is still a pragmatic
approximation of the full Kawano cross-path verification described in
`research_ghi.md`. The remaining notes below are mostly cleanup and
robustness rather than new correctness bugs.

## 1. What is implemented correctly

### 1.1 Terminal detection ordering

<ref_snippet file="/workspace/atomic_solver/src/position.rs" lines="104-127" />

`Position::outcome_from_state` now tests own/opponent commoner extinction,
then the no-legal-moves checkmate/stalemate branch, then `rule50 >= 100`,
and finally the two-piece material draw. This makes 50-move checkmates and
two-piece checkmates return `Loss` instead of `Draw`. The new regression
suite in `tests/test_terminal_ordering.rs` and the unit tests in
`src/position.rs` lock the behavior in.

### 1.2 Depth-bound cutoff storage

`TtEntry` and `TwinEntry` carry `remaining_depth`, and `dfpn` sets it to
`u32::MAX` for terminal and proven Win/Loss results and to the current
`max_depth` for cutoff/unsolved Draws.

<ref_snippet file="/workspace/atomic_solver/src/search/tt.rs" lines="10-17" />
<ref_snippet file="/workspace/atomic_solver/src/search/tt.rs" lines="49-51" />

`try_use_tt` uses `remaining_depth` to reject base or twin results whose
validity budget is smaller than the current `max_depth`:

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="581-650" />

`tests/test_review.rs::depth_zero_cutoff_is_not_reused_as_proven_draw` now
passes, confirming that `search_depth(0)` followed by `search_depth(3)` can
still find a forced win.

### 1.3 Shortest-PV refinement and CLI output

`solve_refined` resets the search state, transposition table, history, and
killers before every binary-search probe. It also no longer prints a final
`outcome:`/`pv:` block, leaving `main.rs` as the single source of final
output. The regression tests `two_rook_shortest_pv_is_three_plies`,
`promotion_shortest_pv_is_seven_plies`, and
`epsilon_mate_shortest_pv_is_five_plies` all pass, and
`cli_does_not_duplicate_final_output` confirms a single stdout block.

### 1.4 `1 + ε` threshold trick

`epsilon_ceil` guarantees a minimum step of `x + 1`, so `ε = 0.0` reproduces
the classic DF-PN `+1` threshold and the default `ε = 0.25` keeps the
multiplicative speed-up.

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="817-823" />
<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="500-516" />

`tests/test_epsilon.rs` confirms `ε = 0.0`, `0.25`, `0.5`, and `1.0` all
solve the simple mate, and the ignored release tests also pass.

### 1.5 GHI infrastructure

The base/twin split, path-code Zobrist encoding, base re-initialization after
a twin proof/disproof, the board-only `repetition_key()`, and the
instrumented twin capacity (`twin_insertions`, `twin_evictions`,
`peak_twins`) are all in place and tested.

<ref_snippet file="/workspace/atomic_solver/src/search/tt.rs" lines="160-176" />
<ref_snippet file="/workspace/atomic_solver/src/zobrist.rs" lines="61-76" />

The synthetic unit tests `try_use_tt_rejects_cross_path_win_twin_without_child_proof`
and `try_use_tt_simulation_uses_current_path` verify that the current
simulation rejects cross-path twins whose proof tree cannot be followed and
that the current search prefix is seeded into `simulate`.

## 2. Remaining correctness concerns and observations

### 2.1 GHI simulation is still a pragmatic approximation

`try_use_tt` seeds `simulate` with the current search prefix
(`sim_path = self.path.clone()`) and follows the stored proof tree using
the twin's `path_code` and `path_length`:

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="652-691" />

This is sound for finite proof trees and for cycles that are also present
in the current search prefix. It is not a full implementation of Kawano's
cross-path verification because `simulate` does not carry the twin's
original ancestor set and does not fall back to a bounded fresh `dfpn` when
it cannot follow the stored proof tree. As `research_ghi.md` section 9 and
`report17.md` note, a twin whose proof relies on a repetition that was
legal in the twin's original path but is not an ancestor in the current
prefix may be accepted or rejected incorrectly.

The placeholder test in `tests/test_ghi.rs` is still ignored and TODO:

> `cross_path_repetition_dependent_win_is_not_reused` — "construct a
> concrete atomic-chess cross-path repetition-dependent win".

Until such a position is found and the solver either passes it or is
strengthened, this is the largest residual correctness risk.

### 2.2 `extract_pv` caps the principal variation at 1000 plies

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="1162-1194" />

`extract_pv` stops after 1000 moves even if the line has not reached a
terminal position. `validate_pv` then fails because the final position is
not terminal, so `extract_pv_checked` falls back to the unvalidated
`extract_pv`. For positions whose shortest win or forced draw is longer than
1000 plies, the returned PV can be truncated or empty. The outcome is
still correct, but the PV output is incomplete.

### 2.3 `simulate` does not check child twin `remaining_depth`

During recursion `simulate` calls `find_result_for_path` to locate child
twins and does not compare the child's `remaining_depth` against the
simulation budget:

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="751-766" />
<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="783-799" />

For solved Win/Loss entries this is safe because `dfpn` always stores them
with `remaining_depth = u32::MAX`. For Draw entries `find_result_for_path`
can return a cutoff Draw with `best_move == Move::NONE`; `simulate` then
returns `false` for `Outcome::Win | Outcome::Draw`, so a false Draw is not
propagated. The safety argument is therefore implicit on the storage
invariant rather than an explicit check, which makes the code fragile if
the storage convention ever changes.

### 2.4 `print_pv_update` in `dfpn` is effectively dead code

`Search::solve` and `Search::solve_refined` both disable
`refine_shortest` before calling `dfpn`:

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="441-461" />
<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="1215-1230" />

Because the guarded `print_pv_update` branch requires `self.refine_shortest`
to be true while `self.path_stack.len() == 1`, it is never reached from the
public API. The function is harmless, but it could be removed or wired to a
verbose flag.

### 2.5 `Outcome::Draw` and `Outcome::Loss` share the same `pn/dn` encoding

<ref_snippet file="/workspace/atomic_solver/src/position.rs" lines="17-23" />
<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="1309-1315" />

Both draws and losses encode as `(INF, 0)`. This is fine for the internal
DF-PN search because `is_solved_by_children` and the stored `outcome` field
are the source of truth, and `outcome_from_pn_dn` is documented as
recognizing only Win. API users should be aware that `Outcome::Draw` and
`Outcome::Loss` cannot be distinguished from their `pn`/`dn` pair alone.

### 2.6 Draw PV validation is inherently weaker

`solve_refined` skips the binary-search refinement when the full outcome is
`Outcome::Draw`:

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="194-235" />

`validate_pv` requires a terminal position:

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="274-318" />

`Position::outcome()` detects stalemate, commoner extinction, `rule50`, and
the two-piece heuristic, but it does not detect a draw by repetition. Since
drawn positions are often non-terminal, `validate_pv` can reject an
otherwise-correct drawing PV. The CLI prints only the outcome for draws,
so this does not produce wrong user-visible output, but it limits the
usefulness of `extract_pv` for drawn positions.

## 3. Research alignment

### `research_epsilon.md`

The implementation now matches the corrected description exactly:
`epsilon_ceil` enforces a minimum of `x + 1`, `ε = 0.0` behaves like
classic DF-PN, the default is `0.25`, and the CLI/runtime API exposes the
parameter. The research note's original `ε = 0.0` error has been fixed.

### `research_ghi.md`

The base/twin split, path-code Zobrist encoding, base re-initialization,
`rule50`-inclusive TT key vs. board-only repetition key, and current-path
simulation seeding are all implemented. The full Kawano cross-path
ancestor-set verification and the bounded fresh-`dfpn` fallback described in
the paper and in `plan17.md` Option A are still not implemented.

### `research_parallel.md`

The solver remains sequential. The OR/AND aggregation, threshold propagation,
and `1 + ε` enhancement all follow the DF-PN+ formulas from the paper.
`T(n,c)`, `Mark`/`Unmark`, and the shared stop set are not present because
the public API is single-threaded.

## 4. Test status

```text
$ cargo fmt -- --check                         # passed
$ cargo clippy --all-targets                   # passed
$ cargo test --all-targets                   # passed
$ cargo test --release                        # passed
$ cargo test --release --test test_ghi -- --ignored   # passed
$ cargo test --release --test test_epsilon             # passed
$ cargo doc --no-deps                         # passed
```

Manual CLI checks:

```text
$ cargo run --release -- --fen "7K/8/8/8/8/8/1Q6/k7 b - - 100 1"
outcome: loss

$ cargo run --release -- --fen "8/8/8/8/8/8/1K6/k7 b - - 0 1"
outcome: draw

$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
outcome: win
pv: f1f7 e8d8 g1g8

$ cargo run --release -- --fen "4k3/PP6/8/8/8/8/8/4K3 w - - 0 1"
outcome: win
pv: a7a8q e8d7 b7b8q d7e6 b8e5 e6d7 e5d6
```

## 5. Recommendations

1. **Strengthen GHI cross-path handling.** Implement Option A from
   `plan17.md`: when `simulate` cannot follow a twin from another path,
   run a bounded fresh `dfpn` from the twin node under the current path with
   `max_depth = twin.depth` and accept the twin only if the bounded search
   returns the same outcome. Alternatively implement full Kawano ancestor-set
   tracking.

2. **Add a concrete cross-path regression test.** Construct an atomic-chess
   position where the same board is reached by two move orders and the winning
   move depends on who still has repetition rights, and un-ignore
   `test_ghi.rs::cross_path_repetition_dependent_win_is_not_reused`.

3. **Raise or parameterize the `extract_pv` hard cap.** The 1000-plies limit
   is safe for the current test suite but can truncate long wins and forced
   draws. Consider making it a `Search` field or tying it to `max_depth`.

4. **Clean up `print_pv_update`.** Either remove the now-unreachable code or
   expose a `--verbose` flag that lets `dfpn` emit intermediate PV updates.

5. **Document the `pn/dn` outcome mapping.** Add a note in the public API
   docs that `Outcome::Draw` and `Outcome::Loss` both encode as
   `(INF, 0)`, so callers must use the `outcome` field as the source of
   truth.

6. **Consider draw-PV validation.** If draw PVs ever need to be exposed,
   `validate_pv` should accept non-terminal draws by repetition or the 50-move
   rule, not just stalemate/commoner extinction/material/two-piece terminals.

## 6. Conclusion

The implementation is in a strong state. All known concrete correctness bugs
from `review3.md` have been fixed and are covered by regression tests. The
DF-PN+ core, epsilon trick, terminal detection, depth-bounded TT reuse, and
shortest-PV refinement are correct on the tested corpus. The residual work
is concentrated in the GHI simulation: it is still a pragmatic approximation
rather than a full cross-path verifier, and it should be either hardened or
proven safe on a concrete atomic-chess cross-path example.
