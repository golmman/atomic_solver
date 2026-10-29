# Provari plan2 report — OR-node move-class starting proof numbers

Initiative: `provari`, backlog item 3a (first half of the split item 3).
Executed 2026-10-29. Product code changed behind a default-off flag; all
measurement artifacts are under
[`measurements/plan2/`](measurements/plan2/).

## Result in one paragraph

Move-class starting proof numbers are a large, reproducible win on the cases
this initiative is measured on. On the core FEN the flag cuts first-outcome
work from **437,166,180 to 50,044,940 child evals (8.74×)** and median wall
from **62.37 s to 8.22 s (7.58×)**; on the only plan1 shared win
(`1eb80b92f92c`) it cuts work from **43,266,033 to 12,680,751 evals (3.41×)**
and median wall from **5.97 s to 1.88 s (3.18×)**. All 12 decisive measured
runs (3 repeats × 2 configs × 2 cases) were independently proof-tree validated
(`root_outcome: win`, `validate: ok`). `sample20` coverage is unchanged
(1/20; the same shared case), 19/20 rows censored for both configs, and the
flag costs roughly 10% per child evaluation on those censored rows. The flag
**stays default-off (opt-in)** until the stack is assembled; plan3 (quiet
grouping) is the next step it gates.

## Scope

Implemented one mechanism: provari-style **starting proof numbers for newly
seen OR-node children**, behind `Search::move_class_pn` / `--move-class-pn`
(default `false`). Per the plan's scope decision:

- no quiet-move grouping (plan3);
- no safe-evasion filter, blast-aware exchange, or static tactical closure
  (backlog item 4);
- no evaluation-scaled proof numbers or learned guide (item 5);
- no change to terminal classification, repetitions, rule50, the TT layout or
  store sites, proof events, or budget accounting.

Files changed:

- `src/search/dfpn/move_class.rs` + `move_class/tests.rs` — the classifier
  (new);
- `src/search/dfpn/{mod,config,children}.rs` — the field/default, setter, and
  the flag-gated initialization in the final unsolved branch of
  `evaluate_child_played`;
- `src/search/dfpn/children/tests.rs` — wiring/invariant unit tests;
- `src/cli.rs`, `src/main.rs`, `tests/test_cli.rs`,
  `tests/fixtures/cli_help_golden.txt` — CLI surface, help text, parser tests.

## Class table as implemented

For an **OR parent** (`!child_is_or`) and a newly seen, non-terminal child
whose `ChildInfo` bounds are still the neutral `(1, 1)`, the child's starting
`pn` comes from the played move's class. `dn` stays at the neutral `1`. All
values are in `[1, 15]`.

| Class | Condition (after playing the move) | Starting `pn` | Approximation vs provari |
| --- | --- | ---: | --- |
| Check | the move gives check | `min(legal replies, 8)`, at least 1 | **Approximate:** counts *all* legal replies, not only replies that survive an immediate explosion. Also caps by legal (not safe) evasions. |
| Double threat | ≥ 2 distinct king-explosion targets, and the mover is **not** immediately lost | 1 | **Approximate:** provari upgrades a parry-less double threat to a static win (item 4, not implemented); we only start it at 1. |
| Threat | exactly 1 distinct target | 4 | — (target shape mirrors provari's) |
| Weak threat | exactly 1 target, and the moved piece is cheaply blastable | 8 | **Approximate:** "cheaply blastable" is a coarse attack/defence test, not a blast-aware exchange evaluation. |
| Hanging quiet | no check/target, and the moved piece is cheaply blastable | 15 | **Approximate:** provari additionally requires a *plain* move (`quiet`, non-capture, non-promotion, non-king); we do not special-case captures/promotions, per plan scope. Castling lands on an empty square and is never "hanging". |
| Quiet | anything else | 9 | **Approximate:** includes non-plain non-threatening moves, matching the plan's attribution of `QUIET`. |

Helper definitions:

- **King-explosion target:** an enemy non-commoner piece adjacent to an enemy
  commoner, not adjacent to one of our commoners, attacked by one of our
  non-commoner pieces after the move. Distinct squares are counted. The
  ordering scorer's §4-§5 threat shape is intentionally **not** refactored
  into a shared helper (no ordering-score change in this plan).
- **Immediately lost:** a cheap over-approximation — some defender
  non-commoner attacks the commoner square or a square adjacent to it. A
  false positive only suppresses the double-threat `pn = 1` (falls back to
  `Threat = 4`); it cannot manufacture a bound. This over-approximation is
  the one deliberate deviation from a strict reading of "has a legal move
  that explodes our commoner at once" and is documented in the module.
- **Legal replies:** generated with `Position::legal_moves` in the child,
  capped at 8; the movegen is internal work and is not billed as a child
  evaluation.

The module header records every approximation; the report deliberately does
not claim parity with provari.

## Configuration surface and inertness

- `Search::move_class_pn: bool`, default `false`, `src/search/dfpn/mod.rs`.
- `Search::set_move_class_pn(bool)`, `src/search/dfpn/config.rs`.
- `CliOptions::move_class_pn: bool`, default `false`, parser arm
  `--move-class-pn`, help line in `src/main.rs::HELP`, wiring in `main`.
- `--help` golden fixture updated; parser tests pin default-off and presence.

Default-off inertness is structural and tested:

1. The classification call is behind `if self.move_class_pn`. With the flag
   off the new branch is not entered; no movegen, no `checkers`, no
   classification.
2. Existing unit tests and the trajectory golden run with the default; both
   gates are green unchanged (see below).
3. New unit test `move_class_default_off_keeps_neutral_bounds` pins `(1, 1)`
   for a child that the flag would otherwise initialize to `(15, 1)`.
4. `move_class_leaves_and_children_neutral` pins AND-parent children at
   `(1, 1)` with the flag on.
5. `move_class_does_not_overwrite_tt_bounds` pins a live outcome-less entry's
   `(5, 7)` bounds against the classifier.
6. `move_class_does_not_touch_solved_children` pins a terminal `Loss` child.
7. `move_class_check_child_uses_capped_evasions` pins the check class to the
   capped reply count.
8. `move_class_off_table_matches_shipped` rebuilds a small position's child
   table through a default search and an explicitly disabled one, requires
   identical rows, and requires every unsolved flag-off row to stay
   `(1, 1)`.

The classifier itself has one fixture per class, a downgrade fixture for the
"immediately lost" double-threat branch, an evasion-cap fixture (9 legal
replies → 8), an AND-side skip test through `evaluate_child`, and a range test
over `[0, 1, 7, 8, 9, u32::MAX]` arguments.

### Invariants preserved

| # | Invariant | Evidence |
| --- | --- | --- |
| 1 | Default off is byte-identical | branch guard; trajectory golden; default-off unit test; `evaluate_all_children` table equality (`move_class_off_table_matches_shipped`) |
| 2 | Never overwrite learned/TT-reused bounds | `pn == 1 && dn == 1` guard; TT-bounds test |
| 3 | Never touch solved outcomes | branch is in the final unsolved path only; solved-child test |
| 4 | OR parents only | `!child_is_or` guard; AND-child test |
| 5 | First encounter only | classification is on the played path after the pre-probe; the outcome-less TT shortcut returns before it |
| 6 | No TT/store change | no `TtEntry`/store change; bounds are ordinary `ChildInfo` values |
| 7 | No proof-event change | event emission untouched; all decisive wins replayed to `validate: ok` |
| 8 | No budget-accounting change | evals counter untouched by classification |

## Measurements

Release build, one process at a time, 192 MB TT label, no tablebases. Corpus
is `measurements/plan1/corpus.json` (shared case and `sample20` unchanged).
`off` = shipped; `on` = `--move-class-pn`. Full parsed rows are in
[`measurements/plan2/results.json`](measurements/plan2/results.json).

### Core case — all repeats

FEN: `r1b1kbnr/ppp3pp/n4q2/3pppN1/1P1PP3/8/P1P2PPP/RNBQKB1R w KQkq - 0 1`;
`--timeout 90 --budget 600000000`.

| Config | Run | Wall | Child evals | Outcome | First win | Exit | Proof validation |
| --- | ---: | ---: | ---: | --- | ---: | --- | --- |
| off | 1 | 62.093 s | 437,166,180 | win | 29 plies | Complete | valid (`validate: ok`) |
| off | 2 | 62.533 s | 437,166,180 | win | 29 plies | Complete | valid |
| off | 3 | 62.367 s | 437,166,180 | win | 29 plies | Complete | valid |
| **off median** | | **62.367 s** | **437,166,180** | **win** | **29** | | |
| on | 1 | 8.336 s | 50,044,940 | win | 27 plies | Complete | valid (`fill_evals: 21`) |
| on | 2 | 8.223 s | 50,044,940 | win | 27 plies | Complete | valid |
| on | 3 | 8.006 s | 50,044,940 | win | 27 plies | Complete | valid |
| **on median** | | **8.223 s** | **50,044,940** | **win** | **27** | | |

No censoring: both configs completed, no budget or wall cut. `child_evals` is
identical across repeats, as expected for a deterministic budget; wall varies
by ~2%.

### Shared case `1eb80b92f92c` — all repeats

`--timeout 10 --budget 300000000`.

| Config | Run | Wall | Child evals | Outcome | First win | Exit | Proof validation |
| --- | ---: | ---: | ---: | --- | ---: | --- | --- |
| off | 1 | 5.938 s | 43,266,033 | win | 37 plies | Complete | valid |
| off | 2 | 6.182 s | 43,266,033 | win | 37 plies | Complete | valid |
| off | 3 | 5.969 s | 43,266,033 | win | 37 plies | Complete | valid |
| **off median** | | **5.969 s** | **43,266,033** | **win** | **37** | | |
| on | 1 | 1.880 s | 12,680,751 | win | 39 plies | Complete | valid (`fill_evals: 0`) |
| on | 2 | 1.880 s | 12,680,751 | win | 39 plies | Complete | valid |
| on | 3 | 1.872 s | 12,680,751 | win | 39 plies | Complete | valid |
| **on median** | | **1.880 s** | **12,680,751** | **win** | **39** | | |

### Deterministic work vs wall

The primary deterministic metric is `child_evals`; it is identical across
repeats in both configs, so the work comparison is exact:

| Case | `child_evals` off | `child_evals` on | Ratio (off/on) |
| --- | ---: | ---: | ---: |
| core | 437,166,180 | 50,044,940 | **8.74×** |
| shared | 43,266,033 | 12,680,751 | **3.41×** |

Wall is the product metric and is reported separately: core **7.58×**, shared
**3.18×** (median ratios). The wall ratio is lower than the work ratio on
both cases, i.e. the `on` config spends slightly more wall per child
evaluation (partly the classification movegen/attack tests, partly its
different search path/tree shape). On the censored `sample20` rows the same
sign appears: the `on` config fits ~11% fewer evals into the same 10 s wall
window. This is not a deterministic work comparison (both configs are
wall-censored), but it is the honest estimate of the classification cost.

### `sample20` — every case

`--timeout 10`, no explicit budget. All rows kept; "censored" means no
decisive outcome within 10 s.

| Case | off wall | off evals | off result | on wall | on evals | on result |
| --- | ---: | ---: | --- | ---: | ---: | --- |
| `2a0443d446ee` | 10.019 s | 71,454,487 | censored | 10.024 s | 63,644,134 | censored |
| `b9a7ed7eb368` | 10.025 s | 73,045,420 | censored | 10.014 s | 64,517,731 | censored |
| `2e8276dc4530` | 10.025 s | 66,108,165 | censored | 10.028 s | 64,102,513 | censored |
| `1eb80b92f92c` | 6.117 s | 43,266,033 | win 37 | 1.892 s | 12,680,751 | win 39 |
| `6559fbdddb47` | 10.019 s | 73,096,849 | censored | 10.012 s | 64,200,340 | censored |
| `3004d09cef79` | 10.015 s | 71,669,848 | censored | 10.021 s | 63,008,439 | censored |
| `5862b1958212` | 10.015 s | 70,309,523 | censored | 10.015 s | 62,862,694 | censored |
| `dd3f89766a40` | 10.023 s | 65,310,342 | censored | 10.026 s | 61,384,263 | censored |
| `8271708709d4` | 10.015 s | 70,481,555 | censored | 10.023 s | 62,737,271 | censored |
| `60af5542800b` | 10.020 s | 70,233,820 | censored | 10.012 s | 62,692,616 | censored |
| `db6a741c723f` | 10.015 s | 69,724,413 | censored | 10.024 s | 62,581,086 | censored |
| `531810cc7d4f` | 10.016 s | 71,640,436 | censored | 10.018 s | 65,325,047 | censored |
| `8862dbcb70d4` | 10.021 s | 74,237,921 | censored | 10.013 s | 67,720,041 | censored |
| `3a7f66e6b796` | 10.011 s | 67,041,874 | censored | 10.021 s | 62,881,077 | censored |
| `bcc5c3ebe435` | 10.013 s | 68,623,007 | censored | 10.016 s | 60,441,434 | censored |
| `e4cca32930e4` | 10.025 s | 69,822,914 | censored | 10.012 s | 64,204,808 | censored |
| `9da3ab788e86` | 10.022 s | 68,728,142 | censored | 10.019 s | 62,544,850 | censored |
| `5d89e584375f` | 10.019 s | 70,004,057 | censored | 10.012 s | 63,850,708 | censored |
| `eefc6b5c0aa5` | 10.019 s | 69,002,382 | censored | 10.025 s | 64,719,852 | censored |
| `43f66371de39` | 10.021 s | 68,641,999 | censored | 10.022 s | 62,146,379 | censored |

Coverage: **off 1/20, on 1/20** (shared case). Provari-only wins from plan1
(`b9a7ed7eb368`, `dd3f89766a40`, `db6a741c723f`) remain unsolved by both
configs at 10 s. No row improved or regressed in coverage; the on/off delta
is a speedup on the one solved row and a ~11% per-eval cost on the censored
rows.

## Replay / validation results

Validation re-runs use the same configuration and `--tt-dump-path`; the tree
is rebuilt with `reconstruct_pt` and checked by its replay-based validator.
Dedup key: `(case, config, PV)`.

| Case | Config | `verify_ppv` on the measured PV | Proof-tree check |
| --- | --- | --- | --- |
| core | off | inconclusive — PV refuted at defender ply 2/29 (`g7g6` not proven lost within 27 plies at the 120 s verify limit) | **valid** — `root_outcome: win`, `validate: ok`, 26,476 nodes, `fill_evals: 1286` (reconstruct ~102 s) |
| core | on | inconclusive — PV refuted at defender ply 4/27 (`f8d6` not proven lost within 23 plies at the 120 s limit) | **valid** — `root_outcome: win`, `validate: ok`, 26,898 nodes, `fill_evals: 21` (reconstruct ~0.5 s) |
| `1eb80b92f92c` | off | refuted-pv — supplied `a7a5` is not a longest defense (depth 3 vs longest 5) | **valid** — `root_outcome: win`, `validate: ok`, 7,874 nodes |
| `1eb80b92f92c` | on | **valid** | **valid** — `root_outcome: win`, `validate: ok`, 6,824 nodes |

Interpretation: `verify_ppv` checks the **informational first-outcome PV**,
not the claimed outcome. Its failures are expected for a best-effort PV (and
an informational PV is explicitly not validated as a proof by `Search`); the
core entries are additionally *inconclusive* because the depth-limited
sub-searches hit the 120 s verify limit, not because a refutation was
established. The **proof-tree validator** replays the published outcome and
passes for every decisive measured run in both configs, so no claimed win
rests on an unvalidated line.

A 180 s / 300 s `verify_ppv` attempt on the core `on` PV was also made during
the session: at defender ply 4 the depth-limited search for the `f8d6` branch
still returned `Draw` after ~1150 s, i.e. it could not prove that branch lost
within 23 plies. This confirms the PV is not a longest-defense PPV; it does
not refute the root proof (which the tree validator accepts).

## Gates

All commands were run on the plan2 working tree after the final code edit.

| Gate | Command | Result |
| --- | --- | --- |
| Default | `make test` | **green** (exit 0; includes the unchanged trajectory fixtures) |
| Full | `make test-full` | **green** (exit 0, 8 m 12 s, all `#[ignore]` tests included) |
| Repetition | `cargo test --release --test test_repetition -- --include-ignored` | **green** (3 passed, 0 failed) |
| Drift / quick suite | `cargo run --release --quiet --example benchmark -- --suite quick` | **green**: 59/59 rows `ok`, no outcome mismatches (3 m 08 s) |

The quick-suite invocation is the then-current equivalent used by previous
search plans. With the flag off, no trajectory/golden behavior changed; the
new branch is unreachable.

## Recommendation

**Keep `move_class_pn` default-off (opt-in) and proceed to plan3,
quiet-move grouping.**

Evidence for the lever is strong but narrow:

- On the two cases where a decisive outcome is reachable at the protocol
  budget (the core FEN and the only shared `sample20` win) the flag reduces
  deterministic work by **8.74×** and **3.41×**, and wall by **7.58×** and
  **3.18×**, with every win independently replay-validated.
- `sample20` coverage is unchanged (1/20), so the lever does not yet show a
  coverage effect on the fixed corpus; 19/20 rows are censored for both
  configs, and there is no evidence about harder positions.
- On those censored rows the classification costs roughly **10% per child
  evaluation**. That is small against the solved-case gains, but it is an
  unmeasured risk on positions near the 10 s boundary and on the hard,
  currently-unsolved part of the corpus.
- The implemented rules are deliberate approximations (all legal replies
  instead of safe evasions, coarse cheap-blast test, over-approximated
  immediate-loss check, no static tactical closure). These are the same
  approximations item 4 is meant to refine; making the approximated version
  the default before that work would bake in a known-imperfect signal.

Explicit flag decision: **default remains `false`**. The lever is retained,
documented, and exercised by tests; plan3 can measure quiet grouping with
this flag independently of the baseline. Defaulting the combined stack is a
plan7 (head-to-head convergence) decision, where a broader solved set and the
full target corpus can be used rather than two cases.

## Not measured

- Quiet-move grouping (plan3), safe evasions, blast-aware exchange, static
  tactical closure, EF-PN, or any learned guide (items 3b-5).
- Class-distribution / expansion census (no instrumentation was added; the
  only observations are the end-to-end evals/wall numbers above).
- Atomic Syzygy tablebases or a full `high200` run.
- Longer-budget or larger-corpus coverage for the on config; the risk of the
  ~10% per-eval cost remains an estimate.
- Any change to the default solver's tournament/optimizer interface
  (`benchmark --json` does not expose the flag in this plan).

## Next steps and missing tests

- **Plan3 (quiet-move grouping, item 3b):** the next session should be a plan
  session that drafts `plan3.md`. It should measure quiet grouping both with
  `--move-class-pn` off (isolate the new mechanism) and, if the plan keeps
  it, with the flag on (the combination). `sample20` may still be too hard to
  give a coverage signal; choose a deterministic `child_evals` target and
  keep the core case.
- **Missing end-to-end test:** the flag-on path is covered by unit tests and
  by these measurements, but there is no committed integration test that
  solves a small position with `--move-class-pn` and asserts the outcome.
  Adding one would pin the CLI wiring in the default gate; it was not added
  here because the measurement + unit coverage already exercises every
  branch and the default gate intentionally avoids slow solves.
- **Missing class census:** no instrumentation counts how many children get
  each class in a real search. The end-to-end evals/wall numbers are the only
  distribution evidence; a debug counter (like the pre-probe census) would
  explain future regressions.
- **Default decision:** revisit in plan7 (head-to-head convergence) with a
  broader solved set, once quiet grouping and the static closure are in the
  measured stack.

## Problems encountered

- The plan-session rank-initialization probe (throwaway `/tmp` copy) is not
  reused: it regressed the core case, confirming that move-order rank is not
  a proxy for move class. The implementation follows the class rules instead.
- The first measurement pass recorded no `child_evals` because the parser
  regex was anchored without `re.MULTILINE` (stderr carries the eval line
  after chunk logs). Fixed and the full protocol re-run; only the fixed
  parsed results are committed.
- `verify_ppv` is not a practical validator for the long core PV: it checks a
  proof-PV property (`longest defense`) that our informational PV does not
  promise, and its depth-limited sub-searches are expensive. The proof-tree
  reconstruct + replay validator is the right acceptance tool for the
  published outcome; it is included in the driver for decisive rows.
- The core baseline needed ~62 s, close to the plan's 90 s window; three
  repeats were stable (62.1-62.5 s) and no censoring occurred, so the ratio
  is not a near-boundary artifact.
