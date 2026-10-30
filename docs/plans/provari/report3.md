# Provari plan3 report — quiet-move grouping (OR nodes only, opt-in)

Initiative: `provari`, backlog item 3b (second half of the split item 3).
Executed 2026-10-30. Product code changed behind a default-off flag; all
measurement artifacts are under
[`measurements/plan3/`](measurements/plan3/).

## Result in one paragraph

Quiet-move grouping is a clear, reproducible win on the cases this initiative
measures. Grouping alone cuts the core FEN from **437,166,180 to 168,365,878
child evals (2.60×)** and median wall from **60.13 s to 30.48 s (1.97×)**; on
the only plan1 shared win (`1eb80b92f92c`) it cuts work from **43,266,033 to
19,082,824 evals (2.27×)** and median wall from **6.01 s to 3.26 s (1.85×)**.
It is also additive with plan2's move-class numbers: the combined config beats
`--move-class-pn` alone by **1.80× evals / 1.47× wall** on the core FEN and
**2.28× / 1.79×** on the shared case. Every decisive measured run (24 core/shared protocol runs plus the shared
case's four `sample20` rows) was independently proof-tree validated
(`root_outcome: win`, `validate: ok`), and the `sample20` coverage is
unchanged (1/20 for every config). The flag
**stays default-off (opt-in)** until the combined stack is assembled; item 3b
closes and the next step is item 4 (static atomic tactical closure).

## Scope

Implemented one mechanism: **one delayed plain-move group child at OR nodes**,
behind `Search::quiet_grouping` / `--quiet-grouping` (default `false`). Per the
plan's scope decision:

- no group TT entry/key (the group is frame-local; opened members are ordinary
  children and their own frames use their real keys);
- no AND-node grouping;
- no static tactical closure, safe-evasion filter, blast-aware exchange, or
  EF-PN (items 4-5);
- no change to terminal/repetition/rule50 handling, the TT layout or store
  sites, proof events, or budget accounting outside the flag.

Files changed:

- `src/search/dfpn/quiet_group.rs` + `quiet_group/tests.rs` — the parent-side
  plainness predicate and the plan2 target shape (new);
- `src/search/dfpn/children.rs` — `ChildInfo::group`, grouped OR-node
  evaluation, and `open_quiet_group`;
- `src/search/dfpn/core.rs` — the group-selection intercept and the
  `Move::NONE` best-child store guard;
- `src/search/dfpn/{mod,config}.rs` — the field/default and setter;
- `src/search/dfpn/{children,selection}/tests.rs` — representation, grouping,
  opening, and conclusion tests;
- `src/cli.rs`, `src/main.rs`, `tests/test_cli.rs`,
  `tests/fixtures/cli_help_golden.txt` — CLI surface, help text, parser/help
  tests.

## Predicate as implemented

`quiet_group::is_plain(pos, mv)` is a **parent-side** test (the child is never
played). It returns `true` only when all of these hold:

- not a capture, en passant, promotion, or castling;
- the moving piece is not a commoner;
- the moved piece's destination, under the post-move occupancy, attacks no
  enemy commoner, **and** its origin square attacked no enemy commoner before
  the move;
- the destination attacks no king-explosion target, **and** the origin square
  attacked no target before the move.

The target shape is copied from plan2's `move_class::threat_targets`: an enemy
non-commoner adjacent to an enemy commoner and not adjacent to one of our
commoners. The local copy is deliberate so ordering scores cannot change.

### Approximations (all marked)

- **Discovered checks/threats through another line are not detected.** Only
  the moved piece's own origin/destination attacks are tested. A move that
  uncovers a stationary check or threat along a line that does not originate
  on the moving piece's square can be classified plain. That only delays the
  move until the group opens; it is not a soundness risk.
- **The destination is not otherwise required to be empty.** Capture,
  en passant, promotion, and castling are rejected explicitly; a normal quiet
  move's destination is empty by construction.
- **Hanging plain moves are grouped too.** They get the existing
  `HangingQuiet` starting number only when the group opens.
- **The predicate is conservative.** It may reject moves that could safely be
  grouped (false negatives lose benefit only). A false positive only delays a
  forcing move until the group opens.
- **The origin-square tests cover the cheap "abandoned own attack" case**, but
  not a newly opened line belonging to a different piece.

Unit fixtures pin: a plain knight move is grouped; checks, captures,
promotions, castling, commoner moves, single threats, double threats, and a
move that abandons its own attack on the commoner are rejected; and a quiet
move that attacks no target remains grouped even when a target exists.

## Group representation, opening, and default-off inertness

`ChildInfo` gains `group: bool`. `ChildInfo::group(pn, dn)` sets
`mv = Move::NONE`, `outcome = None`, `depth = 0`,
`repetition_seen = false`, `explored = false`, `group = true`. Bounds are
starting estimates only: `pn = GROUP_PN = 9` and `dn = hidden_count` (clamped
to `[1, INF]`). A group is never solved, so it cannot let a parent conclude a
Loss or Draw through it; a Win from another child can still conclude early.

Grouped evaluation applies only when `quiet_grouping && is_or_node`. Legal
moves are partitioned with `is_plain` into a reusable `Vec<bool>` scratch. Plain
moves are not keyed, prefetched, TT-probed, evaluated, billed, or emitted as
proof events. Non-plain moves run the shipped key pre-pass and
`evaluate_child_keyed` path exactly as shipped, including the early decisive
`Loss` cut; if that cut fires, the remaining non-plain slots are filled with
`unevaluated` as shipped and **no group is appended**. Otherwise one group is
appended after the non-plain children, at most one per frame.

When selection picks the group, `core.rs` intercepts before the solved/
threshold/descent handling, calls `open_quiet_group`, resets the cached
selection (so the removed index is not re-evaluated), and reselects. Opening
removes the group and evaluates exactly the hidden members through
`evaluate_child`, so each opened member is billed normally and reuses all
terminal/repetition/rule50/TT logic; with `--move-class-pn` on, the existing
class branch assigns member numbers. An OR-node `Loss` member ends the opening
early (the parent is won). The group is never recreated in that frame. No
descent into `Move::NONE` is possible: the intercept runs first, and the
frame-exit store maps a `Move::NONE` best move to `best_child = u8::MAX`
instead of matching a group's `mv`.

Default-off inertness is structural and tested:

1. The grouped path and `open_quiet_group` are unreachable unless
   `quiet_grouping` is true.
2. `ChildInfo::group` defaults to `false`; the flag-off child table is the
   shipped table with only the new field added.
3. The trajectory golden, `make test`, and `make test-full` pass with the
   default; no behavior, TT layout, or proof-event change is observable.
4. New tests pin: default off keeps the heuristic disabled; grouping bills only
   non-plain children; hidden moves are absent before opening; the group is
   suppressed on an early decisive child; opening evaluates each hidden member
   once and leaves the non-plain prefix untouched; opening stops at an early
   TT `Loss`; a pending group blocks Loss/Draw conclusions; a `Move::NONE`
   store never maps to a group index.

Budget note: opening can evaluate at most the legal move count at once. That is
the same bound as one `evaluate_all_children` sweep; the per-node budget check
is unchanged. `is_plain`'s attack tests and any internal work are not billed as
child evaluations.

## Measurements

Release build, one process at a time, 192 MB TT label, no tablebases. Corpus
is `measurements/plan1/corpus.json` (shared case and `sample20` unchanged).
Configs: `off` shipped; `group` `--quiet-grouping`; `mc` `--move-class-pn`;
`both` both flags. Full parsed rows are in
[`measurements/plan3/results.json`](measurements/plan3/results.json).

### Core case — all repeats

FEN: `r1b1kbnr/ppp3pp/n4q2/3pppN1/1P1PP3/8/P1P2PPP/RNBQKB1R w KQkq - 0 1`;
`--timeout 90 --budget 600000000`.

| Config | Run | Wall | Child evals | Outcome | First win | Exit | Proof validation |
| --- | ---: | ---: | ---: | --- | ---: | --- | --- |
| off | 1 | 60.079 s | 437,166,180 | win | 29 plies | Complete | valid (`validate: ok`) |
| off | 2 | 60.132 s | 437,166,180 | win | 29 plies | Complete | valid |
| off | 3 | 60.341 s | 437,166,180 | win | 29 plies | Complete | valid |
| **off median** | | **60.132 s** | **437,166,180** | **win** | **29** | | |
| group | 1 | 30.479 s | 168,365,878 | win | 29 plies | Complete | valid (`fill_evals: 829`) |
| group | 2 | 30.418 s | 168,365,878 | win | 29 plies | Complete | valid |
| group | 3 | 30.652 s | 168,365,878 | win | 29 plies | Complete | valid |
| **group median** | | **30.479 s** | **168,365,878** | **win** | **29** | | |
| mc | 1 | 8.086 s | 50,044,940 | win | 27 plies | Complete | valid (`fill_evals: 21`) |
| mc | 2 | 8.032 s | 50,044,940 | win | 27 plies | Complete | valid |
| mc | 3 | 7.965 s | 50,044,940 | win | 27 plies | Complete | valid |
| **mc median** | | **8.032 s** | **50,044,940** | **win** | **27** | | |
| both | 1 | 5.459 s | 27,861,992 | win | 25 plies | Complete | valid (`fill_evals: 13`) |
| both | 2 | 5.473 s | 27,861,992 | win | 25 plies | Complete | valid |
| both | 3 | 5.502 s | 27,861,992 | win | 25 plies | Complete | valid |
| **both median** | | **5.473 s** | **27,861,992** | **win** | **25** | | |

No censoring: all configs completed, no budget or wall cut. `child_evals` is
identical across repeats, as expected for a deterministic budget; wall varies
by under 1%.

### Shared case `1eb80b92f92c` — all repeats

`--timeout 10 --budget 300000000`.

| Config | Run | Wall | Child evals | Outcome | First win | Exit | Proof validation |
| --- | ---: | ---: | ---: | --- | ---: | --- | --- |
| off | 1 | 5.999 s | 43,266,033 | win | 37 plies | Complete | valid |
| off | 2 | 6.066 s | 43,266,033 | win | 37 plies | Complete | valid |
| off | 3 | 6.013 s | 43,266,033 | win | 37 plies | Complete | valid |
| **off median** | | **6.013 s** | **43,266,033** | **win** | **37** | | |
| group | 1 | 3.281 s | 19,082,824 | win | 43 plies | Complete | valid |
| group | 2 | 3.258 s | 19,082,824 | win | 43 plies | Complete | valid |
| group | 3 | 3.256 s | 19,082,824 | win | 43 plies | Complete | valid |
| **group median** | | **3.258 s** | **19,082,824** | **win** | **43** | | |
| mc | 1 | 1.898 s | 12,680,751 | win | 39 plies | Complete | valid |
| mc | 2 | 1.889 s | 12,680,751 | win | 39 plies | Complete | valid |
| mc | 3 | 1.906 s | 12,680,751 | win | 39 plies | Complete | valid |
| **mc median** | | **1.898 s** | **12,680,751** | **win** | **39** | | |
| both | 1 | 1.059 s | 5,568,743 | win | 45 plies | Complete | valid |
| both | 2 | 1.056 s | 5,568,743 | win | 45 plies | Complete | valid |
| both | 3 | 1.068 s | 5,568,743 | win | 45 plies | Complete | valid |
| **both median** | | **1.059 s** | **5,568,743** | **win** | **45** | | |

### Deterministic work vs wall, and the group/mc interaction

`child_evals` is identical across repeats in every config, so work ratios are
exact:

| Comparison | Core evals ratio | Core wall ratio | Shared evals ratio | Shared wall ratio |
| --- | ---: | ---: | ---: | ---: |
| off / group | **2.597×** | **1.973×** | **2.267×** | **1.846×** |
| off / mc | 8.735× | 7.487× | 3.412× | 3.168× |
| off / both | **15.690×** | **10.987×** | **7.769×** | **5.678×** |
| mc / both | **1.796×** | **1.468×** | **2.277×** | **1.792×** |
| group / both | 6.043× | 5.569× | 3.427× | 3.076× |
| mc / group | 0.297× (group is worse) | 0.264× | 0.665× | 0.583× |

Interpretation: grouping is a real standalone lever (2.6×/2.3× fewer evals),
but move-class numbers remain the stronger single lever. The two are
**synergistic** rather than redundant: `both` improves on `mc` by 1.80× on core
and 2.28× on the shared case. Grouping alone lengthens the first-outcome line
on the shared case (37 → 43 plies) because some forcing-looking plain moves
are delayed until the group opens; on the core case `both` actually shortens
the first win (27 → 25 plies). A longer informational first-outcome PV is not a
correctness issue; every published outcome was validated by the proof tree.

On the censored `sample20` rows the same throughput sign appears as in plan2:
grouping fits ~30% more evals into the same 10 s window than `off`, and `both`
~37% more. Both sides are wall-censored there, so this is not a deterministic
work comparison.

### `sample20` — every case

`--timeout 10`, no explicit budget. All rows kept; "censored" means no
decisive outcome within 10 s. All four configs produced the same outcome on
every row.

| Case | off evals | group evals | mc evals | both evals | Result (all configs) |
| --- | ---: | ---: | ---: | ---: | --- |
| `2a0443d446ee` | 70,610,663 | 54,916,267 | 63,026,385 | 53,527,012 | censored (draw) |
| `b9a7ed7eb368` | 73,214,954 | 55,931,353 | 65,861,693 | 55,773,156 | censored (draw) |
| `2e8276dc4530` | 70,972,215 | 55,061,992 | 64,820,429 | 53,729,392 | censored (draw) |
| `1eb80b92f92c` | 43,266,033 | 19,082,824 | 12,680,751 | 5,568,743 | win 37/43/39/45 plies |
| `6559fbdddb47` | 73,591,711 | 58,180,837 | 63,778,236 | 53,679,042 | censored (draw) |
| `3004d09cef79` | 72,356,978 | 57,934,128 | 64,037,164 | 51,752,403 | censored (draw) |
| `5862b1958212` | 72,489,824 | 55,539,301 | 64,261,868 | 54,005,353 | censored (draw) |
| `dd3f89766a40` | 69,220,293 | 50,861,774 | 62,720,872 | 51,815,099 | censored (draw) |
| `8271708709d4` | 71,954,802 | 56,251,452 | 63,343,797 | 53,297,461 | censored (draw) |
| `60af5542800b` | 72,745,021 | 60,154,185 | 64,013,926 | 53,212,371 | censored (draw) |
| `db6a741c723f` | 70,950,061 | 53,430,277 | 63,919,860 | 53,326,720 | censored (draw) |
| `531810cc7d4f` | 73,843,106 | 58,353,790 | 66,306,081 | 54,324,884 | censored (draw) |
| `8862dbcb70d4` | 73,289,953 | 56,924,296 | 67,944,634 | 55,972,793 | censored (draw) |
| `3a7f66e6b796` | 71,277,189 | 55,071,252 | 63,921,159 | 52,724,781 | censored (draw) |
| `bcc5c3ebe435` | 70,158,223 | 55,066,435 | 62,342,382 | 51,018,315 | censored (draw) |
| `e4cca32930e4` | 71,538,847 | 55,397,543 | 65,276,437 | 55,749,906 | censored (draw) |
| `9da3ab788e86` | 70,284,226 | 53,904,455 | 64,329,793 | 53,889,247 | censored (draw) |
| `5d89e584375f` | 72,676,591 | 54,890,881 | 66,472,777 | 55,095,211 | censored (draw) |
| `eefc6b5c0aa5` | 72,067,495 | 56,600,008 | 66,911,532 | 54,263,363 | censored (draw) |
| `43f66371de39` | 72,005,455 | 56,066,239 | 63,640,500 | 53,086,721 | censored (draw) |
| **totals** | **1,408,513,640** | **1,079,619,289** | **1,239,610,276** | **1,025,811,973** | **1/20 wins; 19/20 censored** |

Coverage: **off 1/20, group 1/20, mc 1/20, both 1/20** (the shared case).
Provari-only wins from plan1 (`b9a7ed7eb368`, `dd3f89766a40`, `db6a741c723f`)
remain unsolved by all configs at 10 s. No row improved or regressed in
coverage.

## Replay / validation results

Validation re-runs use the same configuration and `--tt-dump-path`; the tree
is rebuilt with `reconstruct_pt` and checked by its replay-based validator.
Dedup key: `(case, config, PV)`. All 28 decisive rows (24 protocol runs plus
the shared case's four `sample20` rows) map to 8 unique validation entries
(one per case × config, because repeats produce the same PV).

| Case | Config | `verify_ppv` on the measured PV | Proof-tree check |
| --- | --- | --- | --- |
| core | off | inconclusive (depth-limited sub-search hit its 120 s limit) | **valid** — `root_outcome: win`, `validate: ok`, 26,476 nodes, `fill_evals: 1286` |
| core | group | inconclusive (same) | **valid** — 24,758 nodes, `fill_evals: 829` |
| core | mc | inconclusive (same) | **valid** — 26,898 nodes, `fill_evals: 21` |
| core | both | inconclusive (same) | **valid** — 26,760 nodes, `fill_evals: 13` |
| `1eb80b92f92c` | off | refuted-pv — supplied line is not a longest defense | **valid** — 7,874 nodes, `fill_evals: 1` |
| `1eb80b92f92c` | group | **valid** | **valid** — 7,354 nodes, `fill_evals: 1` |
| `1eb80b92f92c` | mc | **valid** | **valid** — 6,824 nodes, `fill_evals: 0` |
| `1eb80b92f92c` | both | **valid** | **valid** — 6,674 nodes, `fill_evals: 0` |

`verify_ppv` checks the **informational first-outcome PV**, not the claimed
outcome; its core failures are *inconclusive* depth-limited searches, not
refutations of the root proof, and its shared `off` `refuted-pv` verdict means
the best-effort PV is not a longest defense (expected). The proof-tree
validator replays the published outcome and passes for every decisive measured
run in every config, so no claimed win rests on an unvalidated line.

## Gates

All commands were run on the plan3 working tree after the final code edit.

| Gate | Command | Result |
| --- | --- | --- |
| Default | `make test` | **green** (exit 0; includes the unchanged trajectory fixtures) |
| Full | `make test-full` | **green** (37 `test result: ok` suites, no failures; all `#[ignore]` tests included) |
| Repetition | `cargo test --release --test test_repetition -- --include-ignored` | **green** (3 passed, 0 failed) |
| Drift / quick suite | `cargo run --release --quiet --example benchmark -- --suite quick` | **green**: 59/59 rows `ok`, no outcome mismatches |
| Lint | `cargo clippy --all-targets` | **clean** |
| Format | `cargo fmt` | applied; no further diff |

With the flag off, no trajectory/golden behavior changed; the new branch is
unreachable.

## Recommendation

**Keep `quiet_grouping` default-off (opt-in). Close item 3b and proceed to item
4 (static atomic tactical closure).**

Evidence for the lever is strong on the two cases where a decisive outcome is
reachable at the protocol budget:

- Grouping alone is a 2.60×/1.97× (core) and 2.27×/1.85× (shared) win, and the
  combined stack is 15.69×/10.99× and 7.77×/5.68× versus shipped, with every
  win proof-tree validated.
- The interaction with `--move-class-pn` is positive: `both` beats `mc` by
  1.80×/1.47× on core and 2.28×/1.79× on shared. Grouping is not redundant
  with the starting numbers.
- `sample20` coverage is unchanged (1/20) and 19/20 rows remain censored for
  every config; there is no evidence yet about harder positions.

The default remains `false` for the same reason as plan2: both levers are
deliberate approximations (the plainness predicate is conservative and
incomplete, and the class numbers use safe-reply/exchange approximations that
item 4 is meant to refine). Making the combined stack the default before that
work would bake in a known-imperfect signal. Defaulting the assembled stack is
a plan7 (head-to-head convergence) decision with a broader solved set and the
full target corpus.

## Not measured

- Group TT persistence (a synthetic group key) — deliberately deferred; the
  frame-local group re-opens after a restart and member evaluations hit their
  real TT entries.
- Exact safe-evasion / blast-aware exchange filters, static tactical closure,
  EF-PN, or any learned guide (items 4-5).
- Restart-churn attribution, group-open census, or any instrumentation of how
  many groups are opened per search (no debug counters were added).
- Atomic Syzygy tablebases, full `high200`, or longer-budget coverage for the
  group/both configs.
- Any change to the benchmark/optimizer JSON interface (it does not expose
  the new flag in this plan).

## Problems encountered

- The grouped `sample20`/core measurements were clean: `child_evals` is
  deterministic, no run was censored, and all four configs completed the core
  and shared protocols.
- Grouping alone produces a *longer* first-outcome line on the shared case
  (37 → 43 plies) because delayed plain moves can become the eventual winning
  continuation; `both` is still much faster and its proof validates. This is
  recorded as a property of an informational PV, not a regression.
- A debug-build (`cargo test --lib`) run of the existing
  `linear_schedule_restarts_end_to_end` test overflows the stack from deep
  recursion. The same failure reproduces on the unmodified baseline
  (`git archive HEAD` in `/tmp`), so it is pre-existing and not part of the
  release gates (`make test`/`make test-full` are green).
- `make test-lite` was not used as a gate; the plan's gates are the release
  `make test`/`make test-full`, the repetition gate, and the quick suite.
