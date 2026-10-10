# plan15 measurements (2026-10-10) — the m22 collapse diagnosis (item #18, Phase 0)

Evidence for [`../../report15.md`](../../report15.md). Zero product-code
changes remain: the instrumentation (a hand-port of the archived reexam
`probe.patch` to `d7de659` — counters C1–C8 plus the plan15 additions,
`PROBE_INIT` micro-arms, `PROBE_TRACE` selection traces, and
`examples/probe_solve.rs`) was added, measured, and reverted; the pristine
release build reproduces the plan12 salt-0 counts exactly before and after
(`git diff --exit-code` verified).

| artifact | role |
| --- | --- |
| `probe15.py` | driver: anchor_b + P1 (base/mob × 4 cases) + P2 (traces) + P3 (4 micro-arms); resumable, ≤ 3 concurrent processes |
| `parse15.py` | raw JSONs + traces → `state/{anatomy,divergence,arms}.json` |
| `state/anatomy.json` | P1 per-run derived metrics (first-sweep shares OR/AND, fresh share, children/ply distributions, unique share, repetition counters) |
| `state/divergence.json` | P2 trace alignment (first divergence index, post-divergence share, ply distributions) |
| `state/arms.json` | P3 decision table: ratios vs baseline, GO-qualification flags |
| `env.json` | environment, arms, run grid, soundness-invariant results |

Raw transcripts and traces (`/tmp/plan15/raw`) are not committed.

## Headline results (ratios = child evals vs baseline, salt 0)

| arm | stress | dec13 | dec10 | m22 s0 / s1 / s2 | direction win | m22 ≤1.5× uncens. | GO |
| --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 249.5 M | 3.82 M | 4.26 M | 14.16 M / 23.65 M / 15.82 M | — | — | — |
| mob (naive) | **0.235** | **0.160** | 1.83 | cens / cens / cens | yes | 0/3 | no |
| **mob-or** | **0.285** | **0.127** | 1.21 | **1.12 / 0.50 / 0.90** | yes | 3/3 | **yes** |
| mob-and | cens ≥1.20 | 2.19 | 1.27 | 9.15 / 2.02 / 5.89 | no | 0/3 | no |
| **mobcap8** | **0.396** | **0.133** | 1.35 | **1.06 / 0.54 / 0.85** | yes | 3/3 | **yes** |
| moblog | 0.853 | 0.457 | 1.31 | cens / cens / cens | yes | 0/3 | no |

**Verdict: GO-plan16.** `mob-or` (OR-parent children `(n, 1)` only) and
`mobcap8` (both sides `min(n, 8)`) satisfy the pre-registered GO rule; the
shaped-variant gated rollout (plan16) covers them. `mob-and` and `moblog`
are retired.

## Mechanism classification (P1–P3)

- **The win is carried by the OR-side `(n, 1)` init** (the half H-A named).
  On stress/dec13, `mob-or` transfers the first-sweep-cut mass from AND
  frames to OR frames (stress: OR 27.1 %→60.3 %, AND 56.4 %→9.0 %; dec13:
  OR 29.0 %→62.5 %, AND 58.5 %→1.4 %) and cuts fresh-frame thrash — the
  OR-side init alone captures ~all of the naive arm's win (dec13 even beats
  it: 0.486 M vs 0.613 M).
- **The collapse is carried by the AND-side `(1, n)` init** (the half H-B
  named): `mob-and` alone is catastrophic everywhere (stress censored at
  300 M, dec13 2.19×, m22 9.15×/2.02×/5.89×).
- **The m22-specific amplifier is deep repetition-region thrash**: under the
  full naive arm the m22 trajectory immediately enters a >90-ply maneuvering
  region (99.7 % of all 208 M frames at ply > 90; P2 trace at ply ~144–152
  from the first million recursions) holding only **176,385 distinct
  positions** (unique share 0.085 % vs 83.3 % baseline), revisited with
  ~1 `explored` mark per frame and 4.8 evals/frame — censored at 1 B on all
  salts. The P2 selection traces diverge at the **first recursion** (index 0)
  on both m22 and dec13.
- Neither pre-registered full-run signature (H-A: OR-side cuts drop; H-B:
  AND-side cuts collapse while OR-side stays near baseline) matched the
  observed full-mob anatomy (OR-side cuts collapsed 22.7 %→0.2 %, AND-side
  unchanged 58.0 %→57.4 %) — the two halves interact; the causal split comes
  from the P3 micro-arms. H-D (honest expansion) is refuted: the unique share
  collapses instead of rising.
