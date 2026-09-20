# plan4 measurement summary

Raw per-run outputs: `phase0_<case>_e<eps>.{out,err}` (Phase 0 global-ε
sweep), `phase1_<arm>_<case>.{out,err}` (Phase 1 schedule arms),
`quick_clean.json` / `quick_spikeoff.json` (spike-off bit-identity check),
`postrevert_stress.{out,err}` (post-revert reproduction run),
`plan4_derived.csv` (derived table).

## Setup

- Cases: stress = m21_white, m22_white, dec13, dec10 (FENs in plan4.md;
  dec10 via `tests/fixtures/decisive_positions.txt`).
- All runs `--first-outcome --outcome-only`, default TT (128 MB), default
  refine-cap; `--epsilon` per Phase 0 grid, default 0.125 for Phase 1 arms.
- Timeouts: stress 300 s, m22 30 s, dec13 30 s, dec10 60 s.
- `child_evals` was captured with a temporary env-gated stderr dump
  (`RESEARCH4_DUMP=1`, plan1/plan3 pattern): the CLI prints no child-eval
  count. Dump code was part of the spike instrumentation and reverted.
- Reference host: this container (4 cores); stress baseline run took ~50 s,
  well under the 300 s bound, so no timeout noise contaminates the table.

## Verification trail

- Clean-HEAD quick suite (`benchmark --suite quick --json --first-outcome
  --timeout 5`): aggregate `total_child_evals` = 38,974,090.
- Same suite with full spike instrumentation, all env unset: 38,974,090,
  per-case `child_evals`/`pv_len`/outcome bit-identical (`quick_spikeoff.json`).
- Phase 0 ε=0.125 runs reproduce the post-plan9 baselines exactly:
  249,480,478 (stress), 14,156,269 (m22); dec13/dec10 match the quick-suite
  values 3,822,602 / 4,262,128.
- Post-revert stress run: win length 477, `pv_status: first-outcome`, and the
  per-chunk stderr `work_done` fingerprint (8 chunks) is identical to the
  instrumented spike-off run — the search trajectory is unchanged by the
  reverted instrumentation.

## Invariants

- Outcome invariance: `win` everywhere except the two timeouts (stress ε=1.0,
  m22 × d_root), which return `draw` with `pv_status` absent — a timeout, not
  a soundness divergence (all finishing runs agree with the baseline outcome).
- PV length varies across ε/arms. This is inherent: ε is a trajectory knob,
  and `--first-outcome` stops at the *first* decisive line found, which
  depends on the path. plan4.md's "PV length identical" invariant is
  unsatisfiable for any ε change that fires at all (the ε=0.125 rows and the
  never-firing arms are the only ones that can hold it); outcome +
  `pv_status: first-outcome` is the soundness-relevant invariant and holds.

## Key derived numbers (delta vs the ε=0.125 baseline of each case)

Best global constant on the gate object: ε=0.5, stress −37.3% — but control
regressions (m22 +12.7%, dec13 +52.4%) fail GO control parity.

Pareto-improving global constant: ε=0.375 — stress −8.7%, m22 −12.7%,
dec13 +2.6% (within the +5% control bound), dec10 −14.5%.

Best arm on the gate object: r50/d_leaf at 0.0% (both effectively never
fire); d_root +28.4% (and times out m22); linear +11.0% (and +351% m22).
No scheduling arm beats the global default anywhere.
