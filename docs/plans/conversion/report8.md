# Plan 8 Report: Default-ε decision — ε=0.375 **closed won't-fix**, shipped default stays 0.125

## Summary

Phase 1 executed the full zero-code-change validation matrix on the current
binary (`--epsilon 0.375` vs the 0.125 shipped default). The environment
sanity check passed (all eight recorded FO baselines reproduce exactly) and
the default-mode surfaces are **confirmatory** (stress −13.5%, m22 −12.7%,
dec10 −12.4%; dec13 +23.2%), but the two suite gates **failed**: at ε=0.375
the quick suite drops to 57/59 (dec01 and m23_white flip `win → draw` via
timeouts) and the thorough FO suite gains two new timeout-status flips on
previously-solving cases, with 34/66 cases regressing (nine by >+50%).

**Verdict per the pre-registered rule: CLOSE WON'T-FIX.** Measured surfaces
regress at 0.375 while first-outcome improves on the gate object — the
shipped default keeps the broader evidence and 0.375 stays opt-in via
`--epsilon`. The four-point sample that motivated the candidate (stress /
m22 / dec13 / dec10) does not generalize: the ε=0.375 trajectory churn is
broadly distributed across the suite, not confined to the sampled class.
`DEFAULT_EPSILON` was not touched; `src/` is byte-identical to HEAD
(`git diff --exit-code -- src/` clean). Backlog #7 is closed.

## Phase 1 matrix

All runs on clean HEAD (commit `e0d5c71`), `benchmark --suite thorough
--runs 1 --json --filter <case>` per case (in-process `Search` with CLI
defaults: TT 128 MB, refine-cap 0.25, preflight on; `solve` ≡
`solve_with_progress` with a no-op callback). Artifacts:
`measurements/plan8/`. The harness runs one untimed warm-up + one measured
run per invocation; both are deterministic (identical evals).

### Step 0 — environment sanity (FO baselines, exact reproduction)

| case (FO) | ε=0.125 | ε=0.375 | recorded |
|---|---|---|---|
| stress (`m21_white`, 300 s) | 249,480,478 ✓ | 227,834,433 ✓ | ✓ (research plan4/plan8) |
| m22 (`m22_white`, 30 s) | 14,156,269 ✓ | 12,351,299 ✓ | ✓ |
| dec13 (30 s) | 3,822,602 ✓ | 3,921,011 ✓ | ✓ |
| dec10 (60 s) | 4,262,128 ✓ | 3,642,166 ✓ | ✓ |

All eight exact. The bonus evidence reproduces too: stress FO win line
477 plies at 0.125 vs 199 at 0.375. Host validated.

### Items 1–3 — default mode (iterative PV refinement, `--refine-cap 0.25`)

| case | ε=0.125 evals | ε=0.375 evals | delta | wall 0.125 → 0.375 | final PV 0.125 → 0.375 |
|---|---|---|---|---|---|
| stress (300 s) | **338,094,183** (HEAD baseline reproduced exactly) | 292,293,215 | **−13.5%** | 60.3 s → 51.5 s | 129 → 71 plies |
| m22 (30 s) | 17,695,336 | 15,447,517 | −12.7% | 3.07 s → 2.51 s | 95 → 71 |
| dec13 (30 s) | 4,822,604 | 5,942,001 | **+23.2%** | 0.88 s → 1.07 s | 17 → 15 |
| dec10 (60 s) | 5,327,672 | 4,667,113 | −12.4% | 0.68 s → 0.54 s | 41 → 23 |

The unmeasured surface (a) from the `research` plan4 hand-off is mostly
friendly: the default-mode refinement rounds preserve the FO gains on
stress/m22/dec10 and even enlarge them (stress −8.7% FO → −13.5% default).
The exception is dec13 (+23.2% default vs +2.6% FO) — the PARK-shaped
signal, superseded by the suite results below.

### Item 4 — thorough suite, FO (`--timeout 30 --runs 1`)

| | ε=0.125 | ε=0.375 |
|---|---|---|
| solved / timeouts | 61 solved / 5 timeouts | 61 solved / 5 timeouts |
| `wrong` | 0 | 0 |
| total child evals | 1,092,063,073 | 1,456,522,913 (**+33.4%**) |
| timeout set | m20_white, m20_black, m21_white, m21_black, m22_black | m20_white, m20_black, m21_white, **dec01**, **m23_white** |

- **Timeout-status flips: 2 new** (`dec01`, `m23_white` were `ok` at 0.125
  and time out at 0.375); offset by 2 genuine improvements (`m21_black`,
  `m22_black` now solve at 0.375).
- The two new timeouts are not 30 s boundary artifacts: at 0.375 dec01 burns
  210,480,959 evals and m23_white 363,056,135 without finishing, vs
  5,713,706 / 9,673,403 at 0.125 (finishing in ~4 s / ~7 s).
- 34/66 cases regress at 0.375; nine by >+50%: m23_white +3653%, dec01
  +3584%, dec40 +1265%, dec36 +626%, dec05 +491%, dec31 +207%,
  m24_white +120%, dec14 +77%, dec30 +71%.

### Item 5 — quick suite, FO (`--timeout 5 --runs 1`)

| | ε=0.125 | ε=0.375 |
|---|---|---|
| solved / timeouts | 59/59 / 0 | 57/59 / 2 (dec01, m23_white: `win → draw`) |
| `wrong` | 0 | 0 |
| total child evals | **38,974,090** (matches the recorded clean-HEAD aggregate exactly) | 121,943,590 (+213%) |

33/59 cases regress at 0.375; nine by >+50% (same leaders as thorough).
No soundness divergence anywhere: every flip is a timeout returning `Draw`,
never a wrong decisive claim (`wrong=false` everywhere, all suites, both ε).

### Item 6 — repetition soundness gate

- `cargo test --release --test test_repetition -- --include-ignored`: 3/3
  green (at the shipped default 0.125).
- Cyclic rook (`8/8/8/8/2k5/8/8/4KR2 w - - 0 1`) via the CLI at
  `--epsilon 0.375 --tt-size 64 --timeout 5`: `outcome: draw` — the cyclic
  rook stays a draw at 0.375 (the `test_epsilon.rs` 0.375 addition was
  Phase-2-conditional and was not made).

## Gate tally and decision

| # | Pre-registered ADOPT gate | Result |
|---|---|---|
| 1 | FO baselines reproduce exactly at both ε | **PASS** (8/8 exact) |
| 2 | Default-mode stress at 0.375 not worse | **PASS** (292,293,215 < 338,094,183, −13.5%; wall 51.5 s < 60.3 s) |
| 3 | Default-mode m22 wall at 0.375 ≤ 15 s | **PASS** (2.51 s measured; 5.0 s incl. warm-up) |
| 4 | Suites: outcomes unchanged, `wrong=false`, no new timeout flips | **FAIL** (2 new timeout flips; `wrong=false` holds) |
| 5 | Quick 59/59 at 0.375 | **FAIL** (57/59, both via timeout → draw) |
| 6 | Cyclic-rook repetition gate at 0.375 | **PASS** |

**Decision: CLOSE WON'T-FIX.** The pre-registered WON'T-FIX branch fires
directly: measured surfaces regress at 0.375 (quick 57/59, thorough +33.4%
total work with 34/66 regressions) while first-outcome improves on the gate
object (stress −8.7%). The PARK branch's example (dec13 default +23.2%
vs +2.6% FO) also holds, but the suite outcome-gate failure is dispositive
and broader: it shows the regression is not a default-mode refinement
artifact but a property of the ε=0.375 trajectory itself, visible in the
same first-outcome mode the candidate was measured in. The shipped default
keeps the broader evidence; 0.375 remains opt-in via `--epsilon`.

Assessment for the record: `research` plan4's "no control regression"
verdict was true on its four-point sample and wrong as a suite-wide claim —
the ε response is trajectory chaos (`research` H1/H2), and with only four
draws from that distribution the absence of a regression was luck of the
draw, now corrected by the 66-case matrix. The ε surface (constant,
schedule, and per-case autotune) is closed: any revisit needs a new
mechanism, not a new constant.

## Consequences

- Backlog #7 closed (won't-fix); `DEFAULT_EPSILON` stays 0.125. No code,
  golden, or doc-spec changes (all Phase 2 items were ADOPT-conditional).
- No re-open trigger is registered: the ε surface is closed by evidence,
  and the opt-in `--epsilon 0.375` knob remains available for single-case
  deep runs where the 199-ply stress line is preferable.

## Deviations from the plan

- **child_evals capture**: the plan's primary metric is not exposed by the
  CLI, but it *is* an existing `benchmark --json` field, so the cleaner
  zero-instrumentation path was used instead of the RESEARCH8_DUMP hook
  pattern: every per-case run went through
  `benchmark --suite thorough --filter <name>`. Sanity-step exact
  reproduction (8/8) validates the equivalence.
- **Thorough-suite timeout**: the plan names no timeout for item 4; 30 s
  was used (`--timeout 30 --runs 1`). Per-case timeouts for the four
  matrix points follow the plan4 bounds as specified.
- **Default-mode runs in-process**: items 1–3 were measured via `benchmark`
  (in-process `solve`) rather than the CLI with `--outcome-only`; search
  semantics are identical (same constructor defaults, `solve` = no-op
  progress callback), and the golden-test margin statement (gate 3) uses
  the harness-measured per-run wall.
- **Gate 6 scope**: `test_repetition --include-ignored` runs at the shipped
  default; the ε=0.375 half of the gate ran as the exact CLI equivalent of
  the cyclic-rook test (same FEN/TT/timeout, `--epsilon 0.375`).

## Verification

- `git diff --exit-code -- src/` clean (zero code changes; Phase 2 skipped).
- Quick@0.125 aggregate 38,974,090 == the recorded clean-HEAD aggregate
  (`research/measurements/plan4/quick_clean.json` lineage).
- All eight FO baselines exact; default-mode stress HEAD baseline
  338,094,183 exact.

## Next steps

1. Update `docs/plans/README.md` `conversion` row (#7 closed won't-fix).
2. No further ε work (constant, schedule, or per-case): surface closed.
3. Remaining live levers in this initiative: #4 parallel spike (with
   `lean` #2), #5 items (c)/(e) reading, parked #2a ordering guidance.
