# Provari plan5 report — evaluation-scaled proof numbers (EF-PN, hand guide)

Initiative: `provari`, backlog item 5. Executed 2026-10-30. One default-off
mechanism with two pre-registered arms; all measurement artifacts are under
[`measurements/plan5/`](measurements/plan5/).

## Result in one paragraph

Both arms were measured as pre-registered, and they split. The
attacking-branch-only arm (`--eval-pn or`) is a **GO**: it takes the core FEN
from **27,861,992 → 17,217,064 child evals (0.618×, 1.62× fewer)** and 3.85 s
→ 2.66 s, and the shared plan1 win `1eb80b92f92c` from **5,568,743 →
3,231,686 evals (0.580×, 1.72× fewer)** and 0.708 s → 0.458 s; the core and
shared cases are both inside the 1.10× per-case floor and the geometric mean is
**0.599×** against the ≤ 0.85 bar, `sample20` coverage is unchanged (1/20 wins,
19/20 censored) and its censored-row aggregate is **0.920×** `best`. The
both-node-kinds arm (`--eval-pn both`) is a **REJECT**: its core win is larger
(0.440×) and its geometric mean (0.807×) now clears the bar, but the shared
case regresses **1.483×** (5.57 M → 8.26 M evals, 0.708 s → 1.176 s), failing
the per-case floor. The plan-session hypothesis that provari's *defending*
branch is what hurts us on the shared case was therefore right, and the `or`
arm is the shipped form. All 7 unique decisive measured proofs replay-validate
(`validate: ok`, `root_outcome: win`), no outcome moved, and `--eval-pn off`
is inert. Default stays `off` (item 7 decides defaults); the `or` arm joins the
opt-in stack as `--move-class-pn --quiet-grouping --eval-pn or`.

## Scope

Implemented exactly one mechanism: **new children's starting proof/disproof
numbers are scaled by their parent's hand evaluation** (EF-PN's linear branch),
behind `--eval-pn <off|both|or>` (default `off`).

- `both` — provari's formula at both node kinds (OR = attacking, AND =
  defending).
- `or` — the attacking (OR-parent) branch only; AND-parent children keep the
  neutral `(1, 1)`.

Deliberately not implemented (per plan): a graded/logit step, the learned
provability model / network, per-node tuning knobs, any change to move ordering,
grouping, or the static-tactics hook, item 4b closures, item 2 (win-only
front-end), and item 6 keying.

## The evaluation (`src/search/dfpn/eval_guide.rs`)

```rust
pub(super) fn evaluation(board: &Board, side: Color) -> i32;
pub(super) fn step(board: &Board, or_side: Color) -> u8; // 0 / 1 / 2
```

`evaluation(side)` is `terms(side).score() - terms(other).score()`, where one
side's `terms` mirror provari's `src/eval.rs` (read-only reference):

| Term | Meaning | Weight |
| --- | --- | ---: |
| material | the side's non-commoner pieces at `[305, 545, 634, 930, 1670, 0]` | — |
| targets | enemy non-commoner pieces in the enemy commoners' ring, attacked by us, capped at 2 | 400 / 1000 |
| fuses | enemy non-commoner pieces in the enemy commoners' ring (a fuse our capture can light) | 60 each |
| pressure | (our non-commoner piece, ring square) attack pairs | 20 each |
| checkers | our non-commoner pieces that could check an enemy commoner next move | 60 each |

`step` is `2` when the OR side's score is ≥ 500, `0` when ≤ −500, else `1`
(provari's `HAND_DECISIVE = 500` band). The evaluation is never a claim: it
cannot create an outcome, cannot touch the TT by itself, and never overrides a
solved, repetition, rule50, or TT-reused bound.

### Approximations and deviations (all marked in the module header)

- **Commoners instead of one king.** The king ring is the union of the enemy
  commoners' king rings; attack terms are dropped for a side whose commoner
  stands adjacent to an enemy commoner (provari's touching-kings exclusion).
  Single-commoner positions — every corpus fixture — are identical under both
  readings.
- **Per-piece reach, not `attackers_to` per square.** `targets`, `pressure`
  and `checkers` come from one pass over the side's non-commoner pieces with
  the crate attack tables; `checkers` uses the reverse-colour attack trick,
  matching provari. This is pinned by
  `pawn_checker_uses_the_reverse_attack_direction`: a white pawn on c6 attacks
  d7, and a d7 pawn checks the black commoner on e8, so it is a checker. Any
  disagreement with an `attackers_to` reading is an evaluation difference,
  never a soundness one.
- **Fixed constants.** `EF_A = 10`, `EF_B = 5`, `DECISIVE = 500` and the term
  weights are provari's shipped values, not CLI knobs and not the ordering
  scorer's tunable `ScorerParams`.
- **Linear branch only.** A three-valued hand step can never reach provari's
  logit/network branch, so only the exact integer linear branch exists.
- **Fuse direction.** A "fuse" is an enemy piece beside the *enemy* commoner
  (a liability for `us`), as in provari — not one of our pieces beside ours.

## The scaling rule

`ChildScale` is a `Copy` value with four exact-integer fields
(`pn' = pn·proof_mul + proof_add`, `dn' = dn·disproof_mul + disproof_add`):

| Parent | `step = 0` | `step = 1` | `step = 2` |
| --- | ---: | ---: | ---: |
| OR ("attacking") proof | ×11 | ×6 | ×1 |
| OR disproof | ×1 | ×11 | ×21 |
| AND ("defending") proof | +20 from 1 → 21 | +10 → 11 | +0 → 1 |
| AND disproof | ×1 | ×6 | ×11 |

`apply` saturates and clamps to `[1, INF]`, so it never returns 0 and cannot
overflow (`apply(u64::MAX, u64::MAX)` is `(INF, INF)`; `(0, 0)` is at least
`(1, 1)`).

### Which children are scaled

Freshness is defined exactly as plan2 defined it for move-class numbers: the
bounds are still the neutral `(1, 1)` before any heuristic rewrites them. The
one hook is `evaluate_child_played`'s tail, after the move-class rewrite:

```text
fresh = pn == 1 && dn == 1
pn = plan2 class number when fresh && move_class_pn && OR parent, else pn
(pn, dn) = scale.apply(pn, dn) when fresh && scale is Some, else (pn, dn)
```

so EF-PN composes with plan2 on the OR branch (a class `pn = 9` at step 0
becomes 99) and applies to the neutral pair on the AND branch. Solved,
repetition, rule50, and TT-reused children return before the tail and are never
rescaled. The quiet-move group child (plan3) is fresh within its frame and is
scaled like any other fresh child; its opened members are scaled through the
same tail.

### Where the step is computed

Once per frame visit in `Search::dfpn` (`core.rs`), after the terminal/depth/
repetition/TT fast paths and before the children loop, only when the flag is
enabled. The OR side is the root side to move: at an OR frame it is
`pos.side_to_move()`, at an AND frame its `flip()`. `Option<ChildScale>` is
threaded explicitly into the six child-evaluation entry points — no `Search`
field, no hidden frame state, restart-safe by construction.

## Why `off` is inert

The scale is `None` unless `EvalPnMode::is_enabled()`, and with `None` the tail
performs only the neutral comparisons it already performed for plan2. The test
`eval_pn_off_keeps_the_shipped_trajectory` pins identical outcome, nodes and
billed child evals for a default `Search` vs an explicit `set_eval_pn(Off)` one
(the default gate's trajectory/golden tests then hold the behaviour). The
levers' unit tests (`eval_pn_*` in `children/tests.rs`) additionally pin:
fresh-child scaling, `or` leaving AND children neutral, class composition, the
AND factor table, never rescaling solved or TT-reused bounds, and the scaled
group child plus its opened members.

## Measurements

Protocol: release build, `--first-outcome --outcome-only`, 192 MB TT label, no
tablebases, one process at a time. `child_evals` is deterministic (identical
across repeats), so repeat spread is wall-clock only. Ratios are arm / `best`
(lower is better).

An implementation review found one evaluation defect after the first run and
before the numbers below: the `checkers` term used the scoring side's pawn
direction instead of the reverse (enemy) direction, so pawn checkers never
counted. It is fixed (with a pinning unit test) and **all arms were
re-measured**; `measurements/plan5/results.json` is post-fix. Verdicts are
unchanged, but the eval-arm numbers moved (e.g. core `or` 15.77 M → 17.22 M
evals), which is why the earlier draft numbers are not quoted here.

### Core FEN — all repeats (`--timeout 90 --budget 600000000`)

| Config | Run | Wall (s) | Child evals | First win | `pv_status` | Exit | Proof |
| --- | ---: | ---: | ---: | ---: | --- | --- | --- |
| `best` | 1 | 3.818 | 27,861,992 | 25 | first-outcome | complete | valid |
| `best` | 2 | 3.853 | 27,861,992 | 25 | first-outcome | complete | valid |
| `best` | 3 | 3.874 | 27,861,992 | 25 | first-outcome | complete | valid |
| `best+eval` | 1 | 1.891 | 12,245,869 | 27 | first-outcome | complete | valid |
| `best+eval` | 2 | 1.888 | 12,245,869 | 27 | first-outcome | complete | valid |
| `best+eval` | 3 | 1.875 | 12,245,869 | 27 | first-outcome | complete | valid |
| `best+eval-or` | 1 | 2.677 | 17,217,064 | 29 | first-outcome | complete | valid |
| `best+eval-or` | 2 | 2.654 | 17,217,064 | 29 | first-outcome | complete | valid |
| `best+eval-or` | 3 | 2.662 | 17,217,064 | 29 | first-outcome | complete | valid |
| `eval` (isolation) | 1 | 16.415 | 162,437,026 | 31 | first-outcome | complete | valid |
| `eval` (isolation) | 2 | 16.513 | 162,437,026 | 31 | first-outcome | complete | valid |
| `eval` (isolation) | 3 | 16.578 | 162,437,026 | 31 | first-outcome | complete | valid |

Medians: `best` 3.853 s / 27,861,992 · `best+eval` 1.888 s / 12,245,869 ·
`best+eval-or` 2.662 s / 17,217,064 · `eval` 16.513 s / 162,437,026. No run was
censored. `eval` (EF-PN alone, no class/grouping) is a real lever in isolation
— 2.69× fewer evals than the plan4 `off` baseline (437,166,180 / 43.976 s) —
but 5.83× worse than the stack, i.e. EF-PN's value is compositional.

Ratios vs `best`: `best+eval` **0.440× evals / 0.490× wall**;
`best+eval-or` **0.618× evals / 0.691× wall**; `eval` 5.830× evals / 4.286×
wall.

### Shared case `1eb80b92f92c` — all repeats (`--timeout 10 --budget 300000000`)

| Config | Run | Wall (s) | Child evals | First win | `pv_status` | Exit | Proof |
| --- | ---: | ---: | ---: | ---: | --- | --- | --- |
| `best` | 1 | 0.708 | 5,568,743 | 45 | first-outcome | complete | valid |
| `best` | 2 | 0.703 | 5,568,743 | 45 | first-outcome | complete | valid |
| `best` | 3 | 0.713 | 5,568,743 | 45 | first-outcome | complete | valid |
| `best+eval` | 1 | 1.178 | 8,258,553 | 31 | first-outcome | complete | valid |
| `best+eval` | 2 | 1.176 | 8,258,553 | 31 | first-outcome | complete | valid |
| `best+eval` | 3 | 1.174 | 8,258,553 | 31 | first-outcome | complete | valid |
| `best+eval-or` | 1 | 0.458 | 3,231,686 | 31 | first-outcome | complete | valid |
| `best+eval-or` | 2 | 0.458 | 3,231,686 | 31 | first-outcome | complete | valid |
| `best+eval-or` | 3 | 0.465 | 3,231,686 | 31 | first-outcome | complete | valid |

Medians: `best` 0.708 s / 5,568,743 · `best+eval` 1.176 s / 8,258,553 ·
`best+eval-or` 0.458 s / 3,231,686. No censoring.

Ratios vs `best`: `best+eval` **1.483× evals / 1.661× wall (worse)**;
`best+eval-or` **0.580× evals / 0.647× wall (1.72× better)**.

### Ratio summary and the pre-registered geometric mean

| Arm | Core evals | Shared evals | Geomean | Core wall | Shared wall |
| --- | ---: | ---: | ---: | ---: | ---: |
| `best+eval` | 0.440× | 1.483× | **0.807×** | 0.490× | 1.661× |
| `best+eval-or` | 0.618× | 0.580× | **0.599×** | 0.691× | 0.647× |

Only `best+eval-or` clears the ≤ 0.85 geometric-mean bar *and* keeps both cases
inside the 1.10× per-case floor. `best+eval` clears the mean but vetoes on the
shared case.

### `sample20` — every case, 1 repeat, `--timeout 10`, no budget

All 20 cases were run under all three configurations (60 runs). Coverage is
identical for every configuration: the same single win (`1eb80b92f92c`), the
other 19 censored at the 10 s wall and reported as censored draws, never as
proven draws.

| Config | Wins | Losses | Draws | Censored | Σ child evals (all rows) | vs `best` | Σ censored rows | vs `best` |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `best` | 1 | 0 | 19 | 19 | 1,437,321,846 | — | 1,431,753,103 | — |
| `best+eval` | 1 | 0 | 19 | 19 | 1,376,709,795 | **0.958×** | 1,368,451,242 | **0.956×** |
| `best+eval-or` | 1 | 0 | 19 | 19 | 1,320,902,849 | **0.919×** | 1,317,671,163 | **0.920×** |

Per-case rows (all kept, censoring included; the full rows are in
[`measurements/plan5/results.json`](measurements/plan5/results.json)):

| Case | `best` evals | `best` wall | `best+eval` evals | ratio | `best+eval-or` evals | ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `2a0443d446ee` | 74,413,438 | 10.009 s | 70,112,298 | 0.942 | 72,459,860 | 0.974 |
| `b9a7ed7eb368` | 78,343,908 | 10.007 s | 69,869,921 | 0.892 | 69,150,591 | 0.883 |
| `2e8276dc4530` | 75,475,907 | 10.011 s | 70,578,142 | 0.935 | 69,118,240 | 0.916 |
| `1eb80b92f92c` (win) | 5,568,743 | 0.706 s | 8,258,553 | 1.483 | 3,231,686 | 0.580 |
| `6559fbdddb47` | 76,660,037 | 10.011 s | 68,723,713 | 0.896 | 67,667,288 | 0.883 |
| `3004d09cef79` | 73,861,462 | 10.011 s | 68,066,272 | 0.922 | 67,895,554 | 0.919 |
| `5862b1958212` | 74,369,102 | 10.008 s | 70,704,469 | 0.951 | 70,721,110 | 0.951 |
| `dd3f89766a40` | 73,542,365 | 10.006 s | 63,656,965 | 0.866 | 68,865,430 | 0.936 |
| `8271708709d4` | 75,043,466 | 10.005 s | 66,728,118 | 0.889 | 67,307,658 | 0.897 |
| `60af5542800b` | 77,363,000 | 10.009 s | 73,651,786 | 0.952 | 69,788,762 | 0.902 |
| `db6a741c723f` | 75,006,163 | 10.005 s | 69,870,598 | 0.932 | 68,250,780 | 0.910 |
| `531810cc7d4f` | 74,458,918 | 10.011 s | 67,777,768 | 0.910 | 68,455,338 | 0.919 |
| `8862dbcb70d4` | 80,898,557 | 10.007 s | 70,165,349 | 0.867 | 71,374,702 | 0.882 |
| `3a7f66e6b796` | 74,472,855 | 10.006 s | 64,332,031 | 0.864 | 68,239,918 | 0.916 |
| `bcc5c3ebe435` | 71,514,349 | 10.011 s | 70,621,894 | 0.988 | 69,901,251 | 0.977 |
| `e4cca32930e4` | 76,865,547 | 10.009 s | 71,549,213 | 0.931 | 71,550,314 | 0.931 |
| `9da3ab788e86` | 73,171,185 | 10.011 s | 67,144,709 | 0.918 | 67,512,884 | 0.923 |
| `5d89e584375f` | 76,314,031 | 10.006 s | 127,374,931 | 1.669 | 71,313,931 | 0.934 |
| `eefc6b5c0aa5` | 75,504,793 | 10.008 s | 69,015,929 | 0.914 | 70,336,281 | 0.932 |
| `43f66371de39` | 74,474,020 | 10.004 s | 68,507,136 | 0.920 | 67,761,271 | 0.910 |

### Throughput-tax probe (censored rows, fixed 10 s wall)

At a fixed wall, `child_evals` measures throughput, so
`evals(arm)/evals(best) ≈ 1/(1 + tax)`. Over the 19 censored rows:

| Arm | Median ratio | Geometric ratio | Min – max | Implied tax | Note |
| --- | ---: | ---: | ---: | ---: | --- |
| `best+eval` | 0.920 | 0.945 | 0.864 – 1.669 | median ≈ 8.7 % | one outlier (`5d89…` 1.669× — a different trajectory, not a throughput signal) |
| `best+eval-or` | 0.919 | 0.920 | 0.882 – 0.977 | **≈ 8.8 %** (2.3–13.3 %) | tight spread |

The evaluation costs roughly 9 % of node throughput on the `or` arm — inside
the plan's 5–15 % expectation. It does not change the verdict: the arm's
deterministic child-eval counts already include every consequence of the tax,
and the GO is on the `child_evals` primary metric.

## Replay / validation results

Every decisive measured row was validated twice, deduplicated by
`(case, config, PV)` (7 unique keys in
[`measurements/plan5/results.json`](measurements/plan5/results.json)):

- **Proof-tree path (the acceptance criterion)**: re-run with
  `--tt-dump-path`, rebuild with `reconstruct_pt`, require
  `root_outcome == measured outcome` and `validate: ok` — **7/7 valid**, all
  `root_outcome: win`. Core trees: 26,050–26,912 nodes for the three stack
  arms (30,246 for the isolation arm, fill evals 3–317); shared trees:
  5,456–6,674 nodes, fill evals 0. The `or` arm's gains therefore carry the
  same proven outcome as `best`.
- **`verify_ppv` on the informational PV**: `valid` for all three shared-case
  rows; `inconclusive` for every core row (the first-outcome PV is not the
  shortest line). That is a PV property, not a proof defect — plan2–plan4 saw
  the same for `off`/`best` on the core FEN.

## Gates

| Gate | Result |
| --- | --- |
| `make test` (default fast gate) | green — all unit/integration tests passed |
| `make test-full` (`--include-ignored`, ~25 min) | green — 375 lib tests + every integration suite, 0 failed (incl. the repaired `static_tactics_agrees_on_a_deep_draw`; see Problems) |
| `cargo test --release --test test_repetition -- --include-ignored` | green — 3 passed |
| Drift: `cargo run --release --quiet --example benchmark -- --suite quick` | 59/59 rows `ok`, no failures |
| `cargo clippy --release --all-targets`, `cargo fmt` | clean |
| Default-off inertness | `eval_pn_off_keeps_the_shipped_trajectory` (identical outcome/nodes/evals) + the shipped trajectory tests unchanged |

## Verdicts (pre-registered rule)

| Arm | (1) per-case ≤ 1.10× | (2) geomean ≤ 0.85 | (3) coverage + censored ≤ 1.10× | Verdict |
| --- | --- | --- | --- | --- |
| `best+eval` (`both`) | **FAIL** — shared 1.483× | pass — 0.807× | pass — 0.956×, coverage unchanged | **REJECT** |
| `best+eval-or` (`or`) | pass — 0.618× / 0.580× | pass — 0.599× | pass — 0.920×, coverage unchanged | **GO** |

The `or` arm joins the opt-in stack: the best measured stack for later plans is
`--move-class-pn --quiet-grouping --eval-pn or`. The default stays `off` in
every case — which lever becomes the product default is item 7's decision,
following the plan2/plan3 precedent of keeping measured levers opt-in. The
`both` arm stays implemented (it is the composition reference and the
isolation probe) but is recorded as measured out for the stack.

Two findings worth keeping:

- **The plan-session hypothesis was right.** Provari's own hand guide is ~2×
  better on the core FEN and 2.27× worse on our shared win; our `both` formula
  reproduces the shared regression (1.48× more work) and the *defending* (AND)
  branch scaling is the harmful half. Dropping it (`or`) flips the shared case
  to a 1.72× win while keeping a 1.62× core win.
- **EF-PN is compositional, not a standalone lever.** `--eval-pn both` alone
  (no class/grouping) is 2.69× better than the `off` baseline but 5.83× worse
  than `best`; the class numbers and the evaluation reinforce each other.

## Problems encountered

- **Pawn-direction defect in the `checkers` term.** The first implementation
  computed a piece's checking squares with the scoring side's pawn colour
  instead of the reverse (enemy) colour, so pawn checkers never counted. It
  was found on review before the reported run, fixed, pinned by
  `pawn_checker_uses_the_reverse_attack_direction`, and all arms were
  re-measured (see Measurements). Verdicts did not change; the eval-arm work
  counts did.
- **A pre-existing plan4 slow test was wall-censored and flaky.** Plan4's
  `static_tactics_agrees_on_a_deep_draw` (`src/search/dfpn/tests.rs`) solves
  `r3k3/8/8/8/8/8/8/R3K3 w - - 0 1` at full depth and asserts the static
  closure never bills more child evals. On this host the position is far too
  large for the test's 120 s wall: a probe with a 4 B child-eval budget and a
  900 s wall spent the whole budget without finishing, and the ordinary run
  hits the deadline in both arms. The test then compared two *censored*
  trajectories whose cut points are wall-dependent, and the arm with slightly
  more work at the cut failed the inequality spuriously (it did so both alone
  and in the full gate). Fix (test-only, no search behavior changed): the
  work inequality is asserted only when neither run was resource-cut
  (`exit_reason == Complete`); the outcome-agreement assertion (both `Draw`,
  never a claim) always holds. The finding is that a full-depth *work*
  cross-check on this fixture is not executable here — a smaller deep-draw
  fixture would be needed to restore it (follow-up, not this plan).
- **No `/usr/bin/time` in the container.** Wall times come from the driver's
  `time.perf_counter()` around the child process, as in plan3/plan4.
- **`eval` isolation only on the core FEN.** The plan lists it as a core-only
  configuration; it is reported for context, not as a corpus arm.

## Not measured

- Item 4b (`exposed`/no-safe-evasion/double-threat closures) — still open and
  unranked.
- Item 2 (win-only front-end), item 6 (rule50-free key + proof length/limit),
  item 7 (head-to-head convergence).
- The NNUE/provability model (initiative non-goal), tablebases, and the full
  `high200` corpus (only the committed `sample20`).
- A graded/logit step and any tuning of `EF_A`/`EF_B`/`DECISIVE` or the term
  weights (named follow-ups, not this plan).

## Next ranked step

**Open a new sibling-simulation item (item 8) and start it with a plan-session
read-only census**, not item 6. Reason: the remaining measured gap to provari
is node count on shared wins (our `or` stack is 3.23 M child evals / 0.46 s on
`1eb80b92f92c` vs provari's 72.5 k nodes / 0.09 s with its hand guide), sibling
proof replay is the last unported provari search lever, while item 6 is a
soundness-sensitive key change with no measured hit-rate evidence yet. Item 6
stays open behind it.
