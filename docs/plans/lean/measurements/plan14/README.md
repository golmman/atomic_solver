# plan14 measurement provenance (arm A: same-MB 4-slot TT bucket)

Executed 2026-10-11 per `docs/plans/lean/plan14.md` on top of HEAD `a7c3745`.
Host: x86_64 container (AMD 5950X, 4 vCPU) — **not** the aarch64 reference
host; relative comparisons only. Full environment: `env.json`.

## Prefetch variant micro-measurement (task 3)

| metric | 2-hint (shipped form) | 4-hint (pinned) |
| --- | --- | --- |
| m22 default wall ×3 (s) | 2.771 / 2.736 / 2.708 | **1.995 / 2.007 / 1.983** |
| m20_fo s0, evals in 60 s | 418,837,835 | **595,450,282** |

4-hint (byte offsets 0/64/128/192 of the 224-byte bucket) wins decisively;
the 2-hint variant was deleted after the measurement.

## Gate v1.0 rollout (task 4)

- Grid: frozen plan12 corpus (22 cases) × salts {0..5} + a second salt-0
  run per case (`s0r1`, within-build determinism anchor), 156 solver runs,
  3 parallel workers. Budgets: stress 2.5 B, m20_white 2 B, others 1 B;
  wall cap 600 s.
- Baseline: `docs/plans/research/measurements/plan12/state/runs.json`
  (shipped HEAD is byte-identical to the plan12 code base).
- Soundness: **S1** within-build determinism 22/22 (byte-identical salt-0
  stdout + identical evals); **S2** zero decisive-outcome flips; **S3**
  every uncensored run matches its fixture expectation. (Gate invariant 1
  was renegotiated per plan14: shipped-trajectory identity is N/A for a
  behavior-changing arm.)
- Per-case verdicts + lever verdict: `state/comparison.json`. Headline:
  stress **censored-win** (adopt support, hard-class, basins 5/3), m20
  **under-budgeted** (5/6 censored, unresolved list — flagged, contributes
  no verdict), m22/m23 unclear (+1.5%/+1.0% median), dec controls ~1.00×
  (dec01 +5.4% median, basin-1 vs basin-6), no regression/censored-loss
  anywhere → **adopt** per the pinned rule.

## Quick-suite secondary read (task 4, reported not gated)

`benchmark --suite quick --json --first-outcome --timeout 3 --runs 1`
vs the shipped baseline (`../plan12/quick_base.json`): 59/59 outcomes
identical, 52/59 child-eval counts identical, total wall 5.606 → 4.165 s
(−25.7%), total child evals 38.97 M → 42.11 M (+8.1%). No wrong outcomes.
(`quick_arma.json`.)

## Mechanism measurement (tasks 5–6)

Temporary hook (`ATOMIC_TTPROBE=1`, reconstructed from report13 "Method")
re-applied on top of arm A; cells m19_fo / m20_fo / m22_fo (control) ×
salts {0,3} + salts {1,4} on the stress cells (gray-zone protocol). Hook
and driver reverted to byte-identity afterwards (`git diff` over
`src/search/tt/table.rs` identical to the pre-hook arm-A diff; hook files
deleted); `make test` green again.

| cell | re-probe/hits | persistence vs plan13 | victim-live/stores | evals (60 s) |
| --- | --- | --- | --- | --- |
| m19_fo s0 | 14.18% | 81.5% | 80.4% | 473.4 M |
| m19_fo s1 | 15.50% | — | 80.6% | 478.7 M |
| m19_fo s3 | 14.95% | 96.3% | 79.2% | 447.3 M |
| m19_fo s4 | 14.67% | — | 79.7% | 456.5 M |
| m20_fo s0 | 17.36% | 92.6% | 80.7% | 462.8 M |
| m20_fo s1 | 12.65% | — | 81.0% | 478.8 M |
| m20_fo s3 | 13.84% | 69.0% | 75.5% | 472.3 M |
| m20_fo s4 | 12.19% | — | 77.6% | 479.5 M |
| m22_fo s0 | 0.06% | — | 0.16% | 15.0 M (win) |
| m22_fo s3 | 0.05% | — | 0.28% | 18.0 M (win) |

plan13 baseline (`../plan13/state/results.json`): m19 17.40%/15.52%,
m20 18.75%/20.06%, m22 0.44%/0.58% re-probe/hits; victim-live ~70% on the
stress cells. Re-probed work means: 31.7–58.5 (stress) vs live-population
means 20.4–24.1 (plan13: 72–87 vs 22–39).

Fork outcome (pre-registered bands): neither ≤50% (collision-tolerance) nor
≥80% on all four (capacity) fires; gray zone resolved by the salt extension
— the reduction is small (roughly 10–30% relative, salt-noisy), the stress
class stays saturated (~100% occupancy, victim-live ~75–80%), so the
binding constraint is capacity/saturation, not the 2→4 comparison window.
Arm-B sizing sketch: `report14.md` next steps.

## Committed files

`env.json`, this README, `driver.py`, `parse.py`, `compare.py`,
`quick_arma.json`, `state/runs.json`, `state/summary.json`,
`state/comparison.json`, `state/probe_results.json`. Uncommitted
(gitignored): `raw/` — 156 rollout transcripts + 10 probe transcripts
(wall timestamps parsed into `state/runs.json` before the `.meta.json`
files were pruned).
