# reexam measurements (2026-10-09) — probes for `reexamination.md`

Evidence for [`../../reexamination.md`](../../reexamination.md). Zero
product-code changes: every probe ran in an isolated copy of the tree at
`bb19e7b` with `probe.patch` applied (instrumentation counters always on;
behavior changes only behind `PROBE_*` env vars). With no env vars set the
instrumented build reproduces the recorded baselines exactly (stress
249,480,478, m22 14,156,269 first-outcome child evals).

| artifact | role |
| --- | --- |
| `probe.patch` | counters C1–C8 + chunk counter (`src/search/probe.rs`), the `PROBE_INIT` / `PROBE_SUM` / `PROBE_SALT` / `PROBE_CHUNK0` arms, and `examples/probe_solve.rs` (one JSON line per run); `git apply` on `bb19e7b` |
| `driver.py` | runs the R1–R5 and R7 groups (3 concurrent processes) |
| `parse.py` | raw outputs → `state/*.json` |
| `state/runs.json` | all 76 probe runs, keyed by run name, with every counter |
| `state/suites.json` | the six R6 benchmark outputs (aggregates + per-case rows) |
| `state/derived.json` | the tables quoted in the note (salt noise, first-sweep shares, arm ratios, TT size, depth cap, chunk0, suite totals) |
| `env.json` | environment, caps, and gate provenance |

Raw transcripts are not committed (regenerable).

## Run groups

| group | what | cases |
| --- | --- | --- |
| R1 | baseline + counters | stress, m22, dec13, dec10 |
| R2 | TT size 256/512/1024 MB | stress (+ m22 at 256) |
| R3 | depth-capped first outcome, D = 100/128/160/200/256 | stress (+ m22 at 100/128/192) |
| R4 | `PROBE_SALT` 1–4 (TT bucket-index remap; semantics-neutral) | all four |
| R5 | `PROBE_INIT=mob`, `PROBE_SUM=wpns` at salt 0; at salts 1–4 except stress | all four |
| R6 | quick (10 s) and move-order (30 s) suites × baseline/mob/wpns | suites |
| R7 | `PROBE_CHUNK0` 8M / 128M / 4G initial work chunk | all four |

Arms:

- `PROBE_INIT=mob` — unsolved children with no reusable TT bound start at
  `(1, n)` (attacker to move) / `(n, 1)` (defender to move), `n` = legal-move
  count, instead of `(1, 1)`.
- `PROBE_SUM=wpns` — weak proof numbers: AND pn = `max + (k − 1)`, OR dn
  mirrored, `k` = children with a nonzero number; child thresholds
  `th − (k − 1)`.
- `PROBE_SALT=s` — TT index `((key ^ s) · 0x9E3779B97F4A7C15 >> 17) & mask`
  for `s ≠ 0`; full-key verification unchanged, so only the collision /
  eviction pattern moves.
- `PROBE_CHUNK0=n` — replaces the hardcoded initial work chunk (500,000);
  doubling growth unchanged.

## Commands

```bash
# isolated copy (the repo itself is never touched)
mkdir -p /tmp/probe && git archive bb19e7b | tar -x -C /tmp/probe
cd /tmp/probe && git apply <repo>/docs/plans/research/measurements/reexam/probe.patch
CARGO_TARGET_DIR=/tmp/probe/target cargo build --release --examples
mkdir -p /tmp/probe/results

PROBE_BIN=/tmp/probe/target/release/examples/probe_solve PROBE_RESULTS=/tmp/probe/results \
  python3 <repo>/docs/plans/research/measurements/reexam/driver.py

# R6, once per arm: (no env) / PROBE_INIT=mob / PROBE_SUM=wpns
/tmp/probe/target/release/examples/benchmark --suite quick --first-outcome --json --runs 1 --timeout 10 \
  > /tmp/probe/results/r6_quick_<arm>.json
/tmp/probe/target/release/examples/benchmark --suite move-order --first-outcome --json --runs 1 --timeout 30 \
  > /tmp/probe/results/r6_mo_<arm>.json

python3 <repo>/docs/plans/research/measurements/reexam/parse.py /tmp/probe/results
```

## Headline results

| | stress | m22 | dec13 | dec10 |
| --- | --- | --- | --- | --- |
| baseline (salt 0) | 249.5M | 14.16M | 3.82M | 4.26M |
| salts 1–4 | 758M, >2.5B ×3 | 15.8–23.7M | 5.03M ×4 | 2.9–4.8M |
| first-sweep-cut share of evals | 83.4% | 80.7% | 87.5% | 64.6% |
| mob (salt 0) | **58.7M (0.24×)** | >1B (all salts) | **0.61M (0.16×)** | 7.82M (1.83×) |
| wpns (salt 0) | >2.5B | >1B (all salts) | 9.27M (2.43×) | 5.85M (1.37×) |

Suites (total child evals, ok/timeouts): quick baseline 38.97M 59/0, mob
89.97M 58/1, wpns 420.5M 55/4; move-order baseline 1,036.9M 14/5, mob
882.0M 14/5 (solves m21_white, loses m22_white), wpns 1,665.4M 12/7. Zero
wrong outcomes in all 76 runs + 177 suite case-runs.
