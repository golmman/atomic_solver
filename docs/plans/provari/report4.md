# Provari plan4 report — static immediate-explosion closure (item 4a, opt-in)

Initiative: `provari`, backlog item 4 (first half — split as 4a/4b).
Executed 2026-10-30. Product code changed behind a default-off flag; all
measurement artifacts are under
[`measurements/plan4/`](measurements/plan4/).

## Result in one paragraph

The static immediate-explosion closure is **sound but not a speed lever**.
Every decisive measured run proof-tree validated (`root_outcome: win`,
`validate: ok`; 8 unique proofs), coverage on `sample20` is unchanged
(1/20 wins, 19/20 censored for all four configurations), and no outcome moved —
but the work it saves by pricing a forced win at zero evals is outweighed by
the DF-PN trajectory it perturbs. On the shared plan1 win (`1eb80b92f92c`)
alone it is a real win: **43,266,033 → 29,121,003 child evals (1.49×)** and
median wall **4.34 s → 2.89 s (1.50×)**. On the core FEN it is a regression:
**437,166,180 → 542,743,023 evals (1.24× more work)** and median wall
**43.98 s → 52.59 s (1.20× slower)**, and stacked on the plan2+plan3 best
stack it is **27,861,992 → 32,937,211 evals (1.18× more)** with wall
**4.15 s → 4.63 s (1.12× slower)**; on the shared case the best+static stack is
flat in wall (0.738 s → 0.743 s) while costing 1.08× more evals. On the
censored `sample20` rows it costs **+4.0 %** (`static` vs `off`) and **+5.7 %**
(`best+static` vs `best`) billed evals. Decision: **the flag stays
default-off**, item 4a closes as a *negative/neutral* lever with the mechanism
kept opt-in for reference, and the next ranked step is 4b (the `exposed`
two-ply closure) planned with an explicit kill criterion, since the measured
evidence is about *node-entry closures in general*, not about this primitive's
soundness.

## Scope

Implemented exactly one mechanism: **a legal capture beside the enemy commoner
is claimed as a `Win` in one move at `dfpn` node entry**, behind
`Search::static_tactics` / `--static-tactics` (default `false`).

Deliberately not implemented (per plan): provari's `exposed` two-ply closure,
the no-safe-evasion check filter, the parry-less double-threat verdict, a
dedicated mate-in-one pre-scan, cannot-win material, blast-aware exchange
precision, EF-PN, and any win-only front-end work.

## Primitive as implemented

`src/search/dfpn/static_tactics.rs` (with tests in `static_tactics/tests.rs`):

```rust
pub(super) fn immediate_exploding_capture(board: &Board, moves: &MoveList) -> Option<Move>
```

`moves` is the frame's already-generated legal move list; the function
generates no moves, plays nothing, mutates nothing, reads no TT entry. A
capturing move qualifies when its destination

1. holds an enemy non-commoner piece — `targets` is
   `pieces_color(them) & !pieces_pt(Commoner)`;
2. lies in the blast zone of **every** enemy commoner;
3. lies outside the blast zone of every commoner of ours;
4. is a capture, not en passant, and the mover is not a commoner.

### Approximations and deliberate deviations (all marked in the module header)

- **All enemy commoners, not any.** The plan sketched a *union* over the enemy
  commoners; the implementation intersects. With one enemy commoner (the norm,
  and every fixture in the corpus) the two are identical; with several, a union
  would claim a win for an explosion that leaves another enemy commoner on the
  board — a false win. This is the sound reading of the same idea and is
  covered by `multi_commoner_positions_need_every_commoner_in_the_blast`.
- **Legal-move scan only.** No pseudo-legal candidate can create a false win;
  an explosion the generator rejects is simply not claimed.
- **No own-commoner blast.** Squares beside our own commoner are excluded, so
  nothing of ours is destroyed. `atomic_movegen` also rejects such a capture
  under its pseudo-royal rule, i.e. the filter is conservative rather than
  load-bearing.
- **Commoner capturers are rejected.** Under `atomic_movegen`'s rules the
  capturing piece is destroyed by its own blast (pawns, and the capturing pawn,
  excepted), so a commoner capturer loses its own commoner; the geometry of
  rule 3 already excludes it, and the explicit mover guard keeps the intent
  local instead of relying on that interaction.
- **Only beside-commoner captures.** Exposed-defender, no-safe-evasion and
  double-threat verdicts are 4b; mate-in-one is covered only incidentally by
  the child sweep.
- **A miss is never a correctness issue.** Declining a position leaves it to
  the ordinary search, which must return the identical value (see the
  `static_tactics_is_inert_when_no_explosion_exists` and ignored
  `static_tactics_agrees_on_a_deep_draw` tests).

## Hook placement and flag-off inertness

The hook in `Search::dfpn` (`src/search/dfpn/core.rs`) sits **after** the
terminal classification, the `max_depth == 0` leaf return, the local
repetition check, the TT solved-result check and the repetition-cache probe,
and **before** the TT ordering hint / `sort_moves` and the child sweep. That
order is pinned by tests:

| Placement rule | Test |
| --- | --- |
| depth-0 leaf stays a `Draw` with `(1,1)` bounds and `remaining_depth = 0` | `static_tactics_keeps_the_depth_horizon` |
| a repeated position stays a `Draw` | `static_tactics_keeps_the_local_repetition_draw` |
| a solved TT entry is preferred over the static claim | `static_tactics_yields_to_a_solved_tt_entry` |
| a claim bills no child evals, `nodes == 1`, TT entry `Win` depth 1, `best_move` = the capture, `best_child == u8::MAX`, `remaining_depth == u32::MAX`, `work == 0` | `static_tactics_claims_a_beside_commoner_capture_without_evals` |
| the proof event is `NodeProven(Win, depth 1)` at the node path | `static_tactics_emits_a_node_proven_win_at_depth_one` |
| flag off keeps the shipped trajectory (evals billed as before) | `static_tactics_flag_off_keeps_the_shipped_search` |
| no explosion ⇒ byte-identical outcome, nodes and evals | `static_tactics_is_inert_when_no_explosion_exists` |

Flag-off inertness is by construction: the field is only read inside
`if self.static_tactics && ...`, so a default-off run executes the shipped
code path with one extra predictable branch per node. The whole default gate
(unit + trajectory/golden integration tests) passes unchanged.

### One negative probe worth recording

The ordinary proven-node exit also feeds `update_history`/`update_killers`/
`maybe_age_history`. Mirroring that in the static hook — i.e. letting a free
claim teach move ordering — was tried during execution and **made the core case
much worse**: it exhausted the 600,000,000-eval budget and returned `draw`
instead of finishing at 542,743,023 evals. The hook therefore stays
side-effect-free beyond the TT entry and the proof event, and this is
documented at the call site.

## Measurements

Protocol exactly as in [`plan4.md`](plan4.md) / the plan1 corpus: release
build, `--first-outcome --outcome-only`, 192 MB TT label, no tablebases, one
process at a time. `child_evals` is deterministic (identical across repeats),
so repeat spread is wall-clock only.

Configurations: `off` (shipped), `static` (`--static-tactics`), `best`
(`--move-class-pn --quiet-grouping`, plan3's `both`), `best+static` (all three).

### Core FEN — all repeats (`--timeout 90 --budget 600000000`)

| Config | Run | Wall (s) | Child evals | First win | `pv_status` | Exit |
| --- | ---: | ---: | ---: | ---: | --- | --- |
| `off` | 1 | 41.766 | 437,166,180 | 29 | first-outcome | complete |
| `off` | 2 | 43.976 | 437,166,180 | 29 | first-outcome | complete |
| `off` | 3 | 44.061 | 437,166,180 | 29 | first-outcome | complete |
| `static` | 1 | 53.088 | 542,743,023 | 41 | first-outcome | complete |
| `static` | 2 | 52.594 | 542,743,023 | 41 | first-outcome | complete |
| `static` | 3 | 52.552 | 542,743,023 | 41 | first-outcome | complete |
| `best` | 1 | 4.150 | 27,861,992 | 25 | first-outcome | complete |
| `best` | 2 | 4.185 | 27,861,992 | 25 | first-outcome | complete |
| `best` | 3 | 4.047 | 27,861,992 | 25 | first-outcome | complete |
| `best+static` | 1 | 4.764 | 32,937,211 | 27 | first-outcome | complete |
| `best+static` | 2 | 4.590 | 32,937,211 | 27 | first-outcome | complete |
| `best+static` | 3 | 4.626 | 32,937,211 | 27 | first-outcome | complete |

Medians: `off` 43.976 s / 437,166,180 · `static` 52.594 s / 542,743,023 ·
`best` 4.150 s / 27,861,992 · `best+static` 4.626 s / 32,937,211. No run was
censored (no `timeout`, no `budget exhausted`).

Deltas: `static` vs `off` = **1.241× evals, 1.196× wall (worse)**;
`best+static` vs `best` = **1.182× evals, 1.115× wall (worse)**.

### Shared case `1eb80b92f92c` — all repeats (`--timeout 10 --budget 300000000`)

| Config | Run | Wall (s) | Child evals | First win | `pv_status` | Exit |
| --- | ---: | ---: | ---: | ---: | --- | --- |
| `off` | 1 | 4.333 | 43,266,033 | 37 | first-outcome | complete |
| `off` | 2 | 4.337 | 43,266,033 | 37 | first-outcome | complete |
| `off` | 3 | 4.335 | 43,266,033 | 37 | first-outcome | complete |
| `static` | 1 | 2.939 | 29,121,003 | 39 | first-outcome | complete |
| `static` | 2 | 2.835 | 29,121,003 | 39 | first-outcome | complete |
| `static` | 3 | 2.894 | 29,121,003 | 39 | first-outcome | complete |
| `best` | 1 | 0.738 | 5,568,743 | 45 | first-outcome | complete |
| `best` | 2 | 0.740 | 5,568,743 | 45 | first-outcome | complete |
| `best` | 3 | 0.737 | 5,568,743 | 45 | first-outcome | complete |
| `best+static` | 1 | 0.743 | 6,017,488 | 35 | first-outcome | complete |
| `best+static` | 2 | 0.772 | 6,017,488 | 35 | first-outcome | complete |
| `best+static` | 3 | 0.741 | 6,017,488 | 35 | first-outcome | complete |

Medians: `off` 4.335 s / 43,266,033 · `static` 2.894 s / 29,121,003 ·
`best` 0.738 s / 5,568,743 · `best+static` 0.743 s / 6,017,488. No censoring.

Deltas: `static` vs `off` = **1.486× fewer evals, 1.498× faster**;
`best+static` vs `best` = **1.081× more evals, 1.007× wall (flat)**. The
combined best stack is 7.77× cheaper than `off` on this case, i.e. the
closure's own contribution is dwarfed by plan2+plan3 here.

### Deterministic work vs wall, and the interaction with move-class/grouping

| Case | Evals ratio (`static/off`) | Wall ratio | Evals ratio (`best+static/best`) | Wall ratio |
| --- | ---: | ---: | ---: | ---: |
| core | 1.241× (worse) | 1.196× (worse) | 1.182× (worse) | 1.115× (worse) |
| shared | 0.673× (1.486× better) | 0.668× (1.498× better) | 1.081× (worse) | 1.007× (flat) |

The lever is *not* additive with plan2/plan3 on the core case: the closure's
free wins remove exactly the shallow tactical subtrees that the move-class
starting numbers and the quiet grouping already collapse, while the TT entries
it writes (a solved `Win` depth 1 in place of a deeper proven entry with a real
best-child hint) and the history/killer feedback it does not write alter which
line the search finds first (first-win length moves 25 → 27 plies on the core
case, 45 → 35 on the shared one). The first decisive line is informational on
both sides, so the length changes are not correctness signals.

A hypothesis for the regression, consistent with the negative probe above: in
DF-PN+ the *ordering feedback* of a proven node is worth more than the one
child evaluation a terminal-explosion capture already costs, so short-circuiting
the frame skips valuable learning while saving almost nothing.

### `sample20` — every case, 1 repeat, `--timeout 10`, no budget

All 20 cases were run under all four configurations (80 runs). Aggregate
billed evals and coverage:

| Config | Wins | Losses | Draws | Censored (`timeout`) | Σ child evals | vs `off` |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `off` | 1 | 0 | 19 | 19 | 1,950,145,160 | — |
| `static` | 1 | 0 | 19 | 19 | 2,027,366,900 | **+3.96 %** |
| `best` | 1 | 0 | 19 | 19 | 1,368,048,615 | −29.85 % |
| `best+static` | 1 | 0 | 19 | 19 | 1,446,338,836 | −25.83 % |

Coverage is identical for every configuration (the same single win,
`1eb80b92f92c`; the other 19 censored at the 10 s wall, reported as censored
draws, never as proven draws). On the censored rows alone the closure costs
**+4.0 %** (`static` vs `off`) and **+5.7 %** (`best+static` vs `best`) billed
evals — a per-node cost plus trajectory noise, since a censored row is not won
by a free one-move claim.

## Replay / validation results

Every decisive row was validated twice, deduplicated by `(case, config, PV)`
(8 unique keys, all in [`measurements/plan4/results.json`](measurements/plan4/results.json)):

- **Proof-tree path** (the acceptance criterion): re-run with
  `--tt-dump-path`, rebuild with `reconstruct_pt`, require
  `root_outcome == measured outcome` and `validate: ok` — **8/8 valid**
  (`root_outcome: win`; proof-tree node counts 26,476–29,078 for the core case,
  6,674–7,874 for the shared case; fill evals 0–1,286). This holds for the
  `static` and `best+static` rows, i.e. **the lever's own claims replay**.
- **`verify_ppv` on the informational PV**: `valid` for `static`/`best`/
  `best+static` on the shared case and for `static` on the core case;
  `inconclusive` for `off`, `best`, `best+static` on the core case and
  `refuted-pv` for `off` on the shared case. Both are *PV* properties of a
  first-outcome line (the plan2/plan3 reports saw the same for `off`), not
  proof defects; the proof-tree validator is the acceptance criterion.

## Gates

| Gate | Result |
| --- | --- |
| `make test` (default fast gate) | green — 352 lib + all integration tests, 0 failed |
| `make test-full` (`--include-ignored`, ~25 min) | green — 845 passed, 0 failed, incl. the new `static_tactics_agrees_on_a_deep_draw` |
| `cargo test --release --test test_repetition -- --include-ignored` | green — 3 passed |
| Drift: `cargo run --release --quiet --example benchmark -- --suite quick` | 59/59 rows `ok` (no outcome drift, identical `child_evals` columns) |
| `cargo clippy --release --all-targets`, `cargo fmt` | clean |
| Default-off inertness | the flag-off path is the shipped path; all trajectory/golden tests unchanged |

## Recommendation

**Keep the flag default-off; close item 4a as a negative/neutral lever.** The
mechanism is sound, cheap and test-pinned, and it is a genuine 1.49× on the one
shared win, but it is a 1.18–1.24× *regression* on the core FEN and on the
`sample20` censored rows, and it does not stack with plan2+plan3. Keeping it
opt-in costs nothing (one predictable branch), and it is the foundation 4b
would build on, so it is not deleted.

Next: **item 4b** (provari's `exposed` closure: a defender-to-move position
where every legal reply allows an immediate exploding capture is a loss in two
plies), planned with an explicit kill criterion — if it is not additive with
the `best` stack on the core FEN, the static-closure line of item 4 closes.
An alternative re-ranking, given the evidence: item 5 (EF-PN) prices subtrees
*inside* DF-PN instead of short-circuiting frames, which is the lever this
report's regression points at.

## Not measured

- `exposed` two-ply closure, no-safe-evasion check filter, parry-less double
  threat (4b).
- A dedicated mate-in-one pre-scan (the child sweep already returns the
  terminal `Loss`; the closure does not special-case it).
- Cannot-win material draws, blast-aware exchange precision.
- EF-PN / any learned or hand evaluation guide (item 5).
- Rule50-free keying and proof length/limit (item 6).
- Tablebases (excluded by the initiative's baseline), parallelism/portfolio,
  and the full `high200` corpus (only the committed `sample20`).
- Provari itself was not re-run for this plan: it measures a *relative* change
  in our solver, and the plan1 provari numbers remain the reference.

## Problems encountered

- **Multi-commoner soundness gap in the plan sketch.** The plan's target shape
  used a union over the enemy commoners; that is unsound when the opponent has
  two commoners. Changed to an intersection (identical for the single-commoner
  norm) and covered by a test.
- **Two rules are unreachable through a legal move list.** A capture beside
  our own commoner and a commoner capturer are both illegal under
  `atomic_movegen`'s pseudo-royal/blast rules, so their unit tests exercise the
  helper predicates (`explosion_targets`, `qualifies`) with synthetic inputs
  instead of a legal move list; this is documented in the test module.
- **Ordering-feedback probe.** Feeding the claimed move into history/killer
  ordering (to mirror the ordinary proven-node exit) blew the core case past
  its 600 M-eval budget. Reverted; recorded in the report and at the call
  site.
- **No `/usr/bin/time` in the container.** Wall times come from the driver's
  `time.perf_counter()` around the child process, as in plan3.
