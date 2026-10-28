# plan4 measurement provenance (repetition-path bookkeeping)

Plan: [`../../plan4.md`](../../plan4.md). Report: [`../../report4.md`](../../report4.md)
(written by the execution session).

## plan_session/ — the plan session's read-only probe (2026-10-28)

Calibration record for plan4's pre-registered bars. No product code was
modified: the probe builds live under `/tmp` (`git archive HEAD` copies) and
nothing from them landed in the working tree.

Three arms were measured against a pristine-`HEAD` baseline:

| arm | build | change |
|---|---|---|
| `incremental_only` | `/tmp/plan4_proto` | item 3 alone: `path_context_hash` maintained in `path_push`/`path_pop`; `core.rs` reads the field instead of folding `path_stack` |
| `bloom_only` | `/tmp/plan4_bloom` | item 4 alone: 256-slot `u16` path Bloom filter; `path_contains` tests the slot before the linear scan |
| `combined` | `/tmp/plan4_combo` | both changes together (the plan's design) |

| file | content |
|---|---|
| `ab_path_bookkeeping.py` | counterbalanced A/B driver (one base+after pair per repetition, arm order alternating): wall, process CPU time (`ru_utime + ru_stime`), peak RSS, minor faults, `child_evals`, stdout; child-eval and stdout equality is a hard abort |
| `combined.json` | parsed combined-arm results: `dec10` 0.954 wall / 0.954 CPU, `m22_b` 0.992 / 0.992, `stress_b` 0.988 / 0.988, `m20_b` 0.989 CPU, `dec13` 0.994 wall, `rem12` 0.994 wall |
| `incremental_only.json` | the item-3-only arm: `dec10` 0.988, `m22_b` 1.006, `stress_b` 1.001, `m20_b` 1.002 CPU |
| `bloom_only.json` | the item-4-only arm: `dec10` 0.979 wall / 0.979 CPU, `m22_b` 0.997 / 0.997, `stress_b` 0.999 / 0.999, `m20_b` 1.001 CPU |

Every measured cell was `child_evals`- and stdout-identical across all arms and
repetitions (the driver aborts otherwise). The probe's per-cell rep counts and
the exact medians are in the JSON files; raw solver transcripts are not
committed (per AGENTS.md's measurement layout).

## Handoff to the execution session

- Re-measure the baseline **in-session**: absolute walls drift between
  sessions (the plan-session `dec10` baseline is ~0.63 s; `report3.md` recorded
  ~0.35 s on an earlier host state). The bars are ratios, never remembered
  absolutes.
- The item-3-only arm is not a fallback product: it is slower on `m22_b` and
  neutral on `stress_b`/`m20_b` in the probe. The combined design is the plan.
  If the combined change fails, the execution session reverts both and records
  the measured arms; it does not ship a single arm without the owner's call.
- `m20_b` (`--budget 30000000`) is added to the plan corpus because report
  §7.1 measured the item-3 effect on m20, and the plan-session probe used it as
  the deepest path-stack case. Its baseline is ~3.9 s CPU on the current host;
  keep the rep count modest (the A/B driver defaults to 21).

## state/ — the execution session (2026-10-28)

Host and methodology: [`state/env.json`](state/env.json). The execution host was
x86_64 (the plan-session probe ran on aarch64); ratios, not absolutes, are the
contract.

| file | content |
|---|---|
| `state/baseline.json`, `state/after.json` | single-arm corpus (5 runs/case): pristine-`HEAD` baseline vs final tree — median wall, `child_evals`, RSS, minor faults, outcome/PV/`pv_status`, stdout |
| `state/ab_final.json` | authoritative counterbalanced A/B, 11–31 reps/case; per-run `child_evals`/stdout identity is a hard driver abort |
| `state/ab_dec10_repeat.json` | `dec10` confirmation at 41 reps (0.9569 wall / 0.9563 CPU) |
| `state/census.json` | Bloom-negative census from a throwaway instrumented `/tmp` copy: `path_contains` calls, zero-slot negatives, rate per cell |
| `state/quick_after.json` | `benchmark --suite quick --first-outcome --timeout 3 --json`: 59/59 eval-identical to `measurements/plan2/state/quick_after.json`, 42,109,768 child evals |

`driver.py` is plan3's corpus driver adapted to this plan's corpus (`m20_b`
added, `m22` removed). The authoritative A/B is
`plan_session/ab_path_bookkeeping.py`; single-arm ratios are not accepted for
bars 3/4.

Not committed: raw solver transcripts, perf data, and the `/tmp` baseline and
census build trees. The census is reproduced by compiling the test-only
Bloom-negative counter unconditionally in a `/tmp` copy, adding a
`path_lookups` counter, and printing both at `bounded_search` exit (counters
only, no search decision changed).
