# plan1 measurement provenance (cheap DF-PN probe gate)

Executed 2026-10-27 per `docs/plans/performance/plan1.md`; report at
`docs/plans/performance/report1.md`. Host: aarch64 container (4 CPUs, 15 GiB),
same class as the performance report's host; full environment in `env.json`.

## Arms

| arm | binary | configuration |
|---|---|---|
| probe stand-in (Phase 0) | plan1 tree `target/release/atomic_solver` | `--tt-size 4 --budget 2000 --no-preflight --first-outcome --outcome-only` |
| baseline | pristine `HEAD` `1208411` built from `git archive HEAD` into `/tmp/plan1_base` | `--tt-size 256 --timeout 2 --first-outcome --outcome-only` |
| after | plan1 tree | same as baseline (the gate is automatic) |

## Files

| file | content |
|---|---|
| `driver.py` | per-corpus-case runs: wall median, peak RSS (`os.wait4` `ru_maxrss`), parsed stdout/stderr; modes `probe`/`baseline`/`after` |
| `ab_wall.py` | interleaved baseline/after wall A/B (15 reps, ABAB) — the authoritative wall comparison |
| `compare.py` | renders the report1 tables from `state/{baseline,after,ab_wall}.json` |
| `state/probe_phase0.json` | Phase 0 calibration (15-case corpus, 1 run/case) |
| `state/baseline.json` | pristine HEAD (5 runs/case) |
| `state/after.json` | plan1 tree (5 runs/case) |
| `state/ab_wall.json` | interleaved A/B (15 reps/case) |
| `state/quick_base.json`, `state/quick_after.json` | `benchmark --suite quick --first-outcome --timeout 3 --json` |

## Headline numbers

- Easy 3-man KQ wins: 0.748–0.797 s → 0.010–0.011 s wall, 5,827,491–5,827,518
  → 2–586 child evals; status `preflight-proof` → ordinary search
  (`first-outcome` / `proven-shortest`), preflight line `deferred
  reason=probe-solved`.
- Draws: +2,000–2,019 evals (0.034–0.087% of the closure), wall ratio
  1.001–1.025.
- Hard wins (ladder root/step 3): rank and PV length unchanged, +2,002/+2,003
  evals (0.034%).
- Terminal roots: `region=1`, `evals=0` (probe skipped).
- Quick suite: 59/59 eval-identical, 0 wrong, 42,109,768 evals in both arms.

Raw solver transcripts are not committed (AGENTS.md measurement layout); the
drivers plus these parsed JSON files are the reproducible record.
