# Plan 8: Default-ε decision — adopt / close won't-fix / park 0.375

Initiative: `conversion` backlog #7 (sized note handed over from the
`research` pivot, 2026-09-21). Direct successor of `research/plan4.md` +
`report4.md` (Phase 0 global-ε sweep; the ε=0.375 spin-off finding) and
`research/plan8.md` + `report8.md` (Phase 0 separation tables at the 0.375
mix; Phase 1 conditioned-ε arms all no-go — the constant is the closed
ε-surface's best candidate). Prerequisite reading: `research/report4.md`
(the evidence and the four open questions this plan answers),
`research/measurements/plan4/plan4_derived.csv` (the recorded baselines
this plan reproduces), and `research/measurements/plan8/summary.md`
(the exact-reproduction protocol and the drift trail this plan reuses).

## Background

`DEFAULT_EPSILON` (`src/search/dfpn/mod.rs`, currently `0.125`, consumed
as the exact rational `fraction_from_f64(1.0 + ε)` = 11/8 for 0.375) is
the DF-PN+ second-child threshold pad. All ε measurements to date ran
`--first-outcome --outcome-only`:

| case (FO `child_evals`) | ε=0.125 (shipped) | ε=0.375 | delta |
|---|---|---|---|
| stress (`m21_white`) | 249,480,478 | 227,834,433 | −8.7% |
| m22 (`m22_white`) | 14,156,269 | 12,351,299 | −12.7% |
| dec13 | 3,822,602 | 3,921,011 | +2.6% |
| dec10 | 4,262,128 | 3,642,166 | −14.5% |

0.375 is the only constant with **no control regression** (worst case
dec13 at +2.6%, inside the +5% control bound used by `research` plan4).
It sits **under the 10% GO bar** this initiative uses for mechanism
levers, so per the backlog row the decision is *adopt / close won't-fix /
park* — not a GO/NO-GO spike. `research` plan4's hand-off names the
unmeasured surfaces this plan must cover before touching the default:

- **(a) default mode with PV refinement**: every recorded run was
  first-outcome; the refine-cap rounds (`--refine-cap`, default 0.25)
  re-search under the new ε, so the shipped mode's behavior is unmeasured.
  `make stress` runs default mode; the m22 trajectory golden
  (`tests/test_trajectory_golden.rs`) is a default-mode run.
- **(b) the wider suite matrix** under `benchmark --json` (thorough
  suite), for outcome preservation beyond the four-point sample.
- **(c) golden and doc updates** plus `make test-full` on adoption.
- **(d) per-case ε autotune** — explicitly *not* pursued (see Non-goals).

Bonus evidence from the recorded runs: at ε=0.375 the stress FO win line
is 199 plies vs 477 at 0.125 (`research/measurements/plan8/
postrevert_stress_e0.375.*`) — the trajectory change also shortens the
first decisive line.

## Scope decision

**One lever**: the shipped default constant, decided on measured evidence.
Phase 1 is a **zero-code-change measurement round** on the current binary
(`--epsilon 0.375` vs the 0.125 default) covering the unmeasured surfaces
(a)/(b); the one-line `const` flip happens in Phase 2 only on a clear
adopt verdict. No ordering, caching, threshold-arithmetic, or budget
change; `--epsilon` stays a CLI knob either way. The decision closes
backlog #7 whichever way it lands.

## Phase 1 — validation matrix on the unmeasured surfaces (no code changes)

Environment sanity first (deterministic, must be exact): the four FO
baselines reproduce at both settings — ε=0.125: 249,480,478 / 14,156,269 /
3,822,602 / 4,262,128; ε=0.375: 227,834,433 / 12,351,299 / 3,921,011 /
3,642,166 (stress/m22/dec13/dec10; the 0.375 column reproduces
`research` plan8's post-revert captures). Any mismatch invalidates the
host and stops the plan.

Then measure, both settings, artifacts under
`docs/plans/conversion/measurements/plan8/`:

1. **Default mode, stress** (`--outcome-only`, 300 s, default refine-cap):
   new numbers against the HEAD default-mode baseline 338,094,183
   (re-capture at 0.125 first — it must reproduce exactly).
2. **Default mode, m22** (`--outcome-only`, 30 s): wall time and evals vs
   a fresh 0.125 capture; this is also the golden-test runtime margin
   (the golden test runs `--timeout 20`).
3. **Default mode, dec13/dec10** (30 s / 60 s): completes the four-point
   default-mode matrix.
4. **`benchmark --suite thorough --json --first-outcome`** at both ε:
   outcome vs `expected` on every case, `wrong=false` everywhere,
   timeout-status flips counted; work deltas recorded per case.
5. **`benchmark --suite quick --json --first-outcome`** at ε=0.375:
   59/59 outcomes unchanged, `wrong=false` (the standing outcome gate;
   a plain quick@0.375 capture does not exist yet — plan8's quick runs
   were arm-mode).
6. **`cargo test --release --test test_repetition -- --include-ignored`**
   at ε=0.375 (cyclic rook stays a draw; `tests/test_epsilon.rs` already
   covers ε∈{0, 0.25, 0.5} — add 0.375 to its list in Phase 2).

### Pre-registered decision rule

**ADOPT** (proceed to Phase 2) iff all of:

1. The four FO baselines reproduce exactly at both ε (environment sanity).
2. Default-mode stress at 0.375 is **not worse** than at 0.125
   (deterministic metric — exact comparison, no noise band); wall time
   tracked, not gated.
3. Default-mode m22 at 0.375 finishes inside 15 s wall (margin under the
   golden test's 20 s timeout).
4. Quick suite 59/59 + thorough suite: outcomes unchanged, `wrong=false`,
   no new timeout-status flips.
5. Cyclic-rook repetition gate green at 0.375.

**CLOSE WON'T-FIX** if any measured surface regresses at 0.375 while
first-outcome improves (the shipped default keeps the broader evidence;
0.375 stays opt-in via `--epsilon`).

**PARK** if the default-mode results are mixed relative to the FO gains
(e.g. stress default improves but a control's default mode regresses past
its FO delta) — record the matrix, leave the default, re-open on new
evidence. Per repo convention the verdict and its numbers go into
`report8.md` regardless of the branch taken.

## Phase 2 — adoption mechanics (only on ADOPT)

- Flip `DEFAULT_EPSILON` to `0.375` in `src/search/dfpn/mod.rs` (one
  line); add 0.375 to the ε list in `tests/test_epsilon.rs`'s
  simple-mate loop.
- Re-baseline the m22 default-mode trajectory golden
  (`tests/fixtures/m22_default_stdout_golden.txt`) from the post-flip
  binary and **verify the new PV as a valid PPV via the `verify_ppv`
  example** (precedent: lean plan4, dfpn plan8 re-baselines); update the
  golden-file header comment with the re-baseline provenance.
- `cargo fmt`, `cargo clippy --release --all-targets`, `cargo doc` clean;
  `make test` green, then **`make test-full`** (required for search
  changes per AGENTS.md).
- Docs: update the `docs/spec/optimizer_interface.md` JSON example's
  `"epsilon"` value if it reflects the shipped default; sweep `src/main.rs`
  doc comments for a stated default; update this initiative's backlog #7
  row + history entry; refresh the stale `docs/plans/README.md`
  `conversion` row (it still omits #7) when the decision lands.
- Deliverable: `report8.md` in this directory — the Phase 1 matrix
  (default-mode numbers, suite tables, gate checks), the decision verdict
  with its evidence, and (on adopt) the golden re-baseline record with the
  PPV verification output.

## Non-goals

- **Per-case ε autotune** (report4 open question (d)): `research` closed
  the ε surface — the response is trajectory chaos (H1/H2 both rejected),
  so a per-case constant would be overfitting the four-point sample; a
  revisit needs a new mechanism, not a new knob.
- Any scheduled/conditioned ε (closed by `research` plan4 Phase 1 and
  plan8 Phase 1 — every arm no-go).
- Any threshold-arithmetic, caching, ordering, or budget change (this is
  a trajectory knob only; soundness-neutral — outcomes are unaffected on
  every finishing run ever recorded).
- Re-measuring the FO surface beyond the exact-reproduction sanity check.

## Measurement conventions

- Primary metric: `child_evals` (deterministic); wall time secondary.
  Capture via the existing CLI/stderr fields (no instrumentation needed —
  the plan8 `RESEARCH8_DUMP` hook pattern only if a value is not exposed).
- Baselines: FO numbers inherited from `research` plan4/plan8 (table
  above); default-mode stress 338,094,183 and m22 default from a fresh
  0.125 capture at HEAD (must be exact before any 0.375 comparison).
- Soundness gates: quick/thorough outcome preservation (`wrong=false`
  everywhere), `tests/test_repetition.rs --include-ignored` green.
- Timeouts per the plan4 bounds: stress 300 s, m22 30 s, dec13 30 s,
  dec10 60 s.
