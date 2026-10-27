# plan2 measurement provenance (zero-init / lazy TT allocation)

Executed 2026-10-27 per `docs/plans/performance/plan2.md`; report at
`docs/plans/performance/report2.md`. Host: x86_64 container (4 CPUs, 15 GiB);
full environment in `env.json`.

## Arms

| arm | binary | configuration |
|---|---|---|
| baseline | pristine `HEAD` `fe491b4` built from `git archive HEAD` into `/tmp/plan2_base` | `--tt-size 256 --first-outcome --outcome-only` + per-case `--timeout`/`--budget` |
| after | plan2 tree `target/release/atomic_solver` | same as baseline (the lazy allocation is unconditional) |

## Files

| file | content |
|---|---|
| `driver.py` | single-arm corpus (`--mode corpus`) and `--tt-size` sweep (`--mode sweep`) runs: wall min/median, peak RSS (`os.wait4` `ru_maxrss`), minor faults (`ru_minflt`), child evals, parsed stdout |
| `ab_wall.py` | interleaved baseline/after A/B (one run of each arm per repetition) on the same per-run metrics — the authoritative wall comparison for the plan's bars 2–5 |
| `compare.py` | renders the `report2.md` tables from `state/*.json` and evaluates the pre-registered bars |
| `state/baseline.json` | pristine HEAD corpus, 5 runs/case |
| `state/after.json` | plan2 tree corpus, 5 runs/case |
| `state/ab_wall.json` | interleaved A/B, 15 reps/case, full corpus |
| `state/ab_wall_preflight.json` | interleaved A/B, 25 reps/case, the three closure-decided cells (`draw_kr`, `draw_cyc`, `ladder_root`) — the tight 10%-bar measurement |
| `state/baseline_sweep.json`, `state/after_sweep.json` | `--tt-size {1,16,64,256,512}` on mate-in-1, 5 runs each |
| `state/quick_after.json` | `benchmark --suite quick --first-outcome --timeout 3 --json` |

## Headline numbers

- Short class (interleaved medians): mate-in-1 **104.6 → 1.2 ms** (87×, RSS
  229 → 16 MB), easy KQ **103.7 → 1.7 ms** (61×, 230 → 16 MB).
- `--tt-size` sweep on mate-in-1: the after arm is flat at ~1 ms / 15 MB from
  1 MB to 512 MB; the baseline arm pays 3.2 ms → 193.8 ms / 16 → 453 MB.
- Closure class (25 reps): `draw_kr` 0.832 → 0.740 s (−11.1%), `draw_cyc`
  0.831 → 0.747 s (−10.1%), `ladder_root` 1.013 → 0.927 s (−8.5%; 86 ms of
  fixed saving, but only 8.5% of a 1.01 s solve — the plan's bar-3 miss).
- Deep cells (15 reps): dec10 ×1.036, m22 ×1.027, m22_b ×1.034, stress_b
  ×1.036 — `max` 1.036 against the pre-registered 1.06 ceiling.
- **Every cell is child-eval identical** (outcome, PV length and `pv_status`
  too); quick suite 59/59 identical, 42,109,768 evals in both arms.

Raw solver transcripts are not committed (AGENTS.md measurement layout); the
drivers plus these parsed JSON files are the reproducible record.
