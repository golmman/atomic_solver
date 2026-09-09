# Lean Report 4 — TT capacity default: `--tt-size` 64 MB → 128 MB

Implements `docs/plans/lean/plan4.md` (item #16 of the lean initiative).
Unlike plans 1–3 this is an **intentionally behavior-changing** plan: the
bit-identical drift protocol does not apply, and the validator is the
move-order benchmark suite plus the quick suite as a shallow-control
group.

**Headline (m22_white, default settings, this container):**
first-outcome wall **5.32 s → 2.67–2.78 s (−49%)**, default-mode wall
**12.7 s → 3.30–3.32 s (−74%)**. First-outcome work 37.5M → 14.2M child
evals (−62%). The refined default-mode informational PV changed
(23 → 95 plies) and was re-verified as a valid PPV (`verify_ppv`:
`is_ppv: true`).

## Summary of changes

- `src/cli.rs`: `tt_size` default `64` → `128`, with a comment pointing at
  the research note; default-asserting unit test updated.
- `src/main.rs`: doc-comment and `--help` default text.
- `examples/benchmark.rs`: `tt_size` default `64` → `128` (the optimizer
  evaluator stays in lockstep with the CLI default).
- `docs/spec/optimizer_interface.md`: example JSON `"tt_size": 128`.
- `AGENTS.md`: CLI bullet default updated.
- `tests/fixtures/m22_default_stdout_golden.txt`: re-baselined from the
  post-change binary; `tests/test_lean3.rs` header updated to record the
  re-baseline and its reason.
- New research note `docs/plans/lean/research_tt_capacity.md` (the basis
  of this plan) and this report.

Explicit `Search::new(64)` call sites in tests/examples were left alone —
they pin deterministic fixtures, not defaults.

## Validation

### Hard tier (move-order suite, first-outcome, `--runs 1 --timeout 30`)

Same binary, explicit `--tt-size 64` vs `--tt-size 128`
(`measurements/plan4/mo64.json`, `mo128.json`). Solved cases
(deterministic work):

| case | 64 MB | 128 MB | delta | outcome |
| --- | --- | --- | --- | --- |
| m22_white | 37,503,264 | 14,158,593 | **−62.2%** | win → win |
| m23_black | 2,844,260 | 2,877,582 | +1.2% | loss → loss |
| m23_white | 9,775,865 | 9,673,403 | −1.0% | win → win |
| m24…m29 (11 cases) | unchanged | unchanged | ±0.0% | all stable |

- Net: −23.4M evals saved, +33k added; every outcome stable, no wrong
  results.
- The five genuinely deep cases (m20_white/black, m21_white/black,
  m22_black) hit the 30 s cap at both sizes; their evals are
  throughput-at-timeout, not solution work, and are not comparable. They
  remain unsolved at 30 s at either size.

### Quick suite control (59 cases, first-outcome, `--runs 1`)

- 64 MB: 38,714,551 total child evals (matches the plan1–3 artifacts).
- 128 MB: 39,496,274 (+2.0%), 59/59 `status == ok`, no wrong outcomes
  (`quick_tt64.json`, `quick_tt128.json`). The shallow cases are
  TT-size-insensitive; the ±2% aggregate is chaotic-regime noise, and the
  largest single-case movement was dec10 (−29%). After the default flip,
  a fresh default run reproduced 39,496,274 exactly
  (`quick_after_plan4.json`, `"tt_size": 128`).

### m22 default mode

- New trajectory (default settings): first win at ~1.4 s (was ~9.9 s at
  64 MB), refinement exhausts the bounded tree at ~3.2 s. Final stdout:
  win, 95-ply PV — stored as the new golden
  (`m22_default_new_stdout.txt` = `tests/fixtures/…golden.txt`).
- The 95-ply line was verified as a Proof Principal Variation with the
  `verify_ppv` example (`is_ppv: true`, 1.34 s).
- The re-baselined slow golden test
  (`tests/test_lean3.rs::m22_default_mode_trajectory_matches_golden`)
  passes in 3.41 s (it took ~7 s before — the bounded tree now exhausts
  faster than the 20 s deadline).

### Test tiers and lint

- `make test` (fast tier): 259 passed, 0 failed, 29 ignored (slow tier).
- The slow golden test re-run explicitly with `--include-ignored`: green.
- `cargo fmt --check` clean; `cargo clippy --release --all-targets` clean.

## Deviations from the plan

1. **Timeout-tier treatment.** The plan's acceptance criterion said "no
   case regresses beyond chaotic-regime noise; no case flips to
   `status != ok`". The five > 30 s cases are excluded from the eval
   comparison (their work is truncated by the timer), but the outcome
   stability half of the criterion holds for them: all five time out at
   both sizes with identical (draw-at-timeout) status and no wrong
   results. Documented rather than papered over.
2. **`m23_black` +1.2%.** Strictly this is a small regression, within the
   chaotic regime the research note describes (single-draw trajectories
   above the capacity knee vary 14–19M on m22-scale work). The −62% on
   the dominant case and the outcome stability across the suite outweigh
   it.

## Problems encountered

- The `benchmark` CLI rejects underscores (`move_order` → panic), only
  the hyphenated `move-order` is accepted; cosmetic, pre-existing.
- No GNU `time` in the container; walls measured with `date +%s.%N`
  deltas around the direct binary.

## Missing tests

- No automated guard for the capacity knee itself (e.g. an assertion that
  m22 first-outcome `child_evals` stays near the 128 MB value). The
  re-baselined golden covers the default-mode trajectory; first-outcome
  work is only locked by the JSON artifacts in `measurements/plan4/`.
- The timeout-tier cases (m20/m21/m22_black) still lack any solved-at-
  default-settings regression signal; they need minutes-to-hours budgets
  and remain a manual-tier concern.

## Additional tools/examples used

- `benchmark --suite move-order --json --first-outcome --runs 1` at both
  sizes for the hard-tier table.
- `verify_ppv` for the re-baselined PV.
- `python3` for JSON diffing.
- Raw outputs: `docs/plans/lean/measurements/plan4/`.

## Next steps

- **plan5 candidate — #2 parallelism design spike**: the determinism
  story for the `child_eval_budget` contract is now the main blocker for
  the only remaining multiplicative lever. Note the TT default change
  interacts with any parallel design (shared vs. per-worker tables), so
  the spike should start from the post-plan4 baseline.
- Micro-wins (#13 clock sampling, #12a TT non-terminal skip, #14 wrapper
  slimming, #15 playout cross-check) remain available as a small session.
