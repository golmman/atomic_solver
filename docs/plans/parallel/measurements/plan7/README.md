# plan7 measurements (2026-10-05) — process-level portfolio racing: NO-GO

Executes `docs/plans/parallel/plan7.md`. Product solver consumed as-is (zero
product-code changes); all deliverables are drivers + measurements. Stage-0
drift gate passed (stdout byte-identical to the plan4–plan6 record, see
`state/drift7_results.json`). Stage-2 pilot clean (4 concurrent racers launch,
fit memory, terminate ≤ cap, W_first extraction works). Campaign: 8 rounds ×
3 cases, rounds outer / cases round-robin, each round = baseline sequential
run alone → race of P0–P3. Pilot (label `pilot`) excluded from analysis.

| artifact | role |
| --- | --- |
| `portfolio7.py` | campaign driver (baseline + 4-racer race per round) |
| `analyze_portfolio7.py` | H1–H3 mechanical evaluation, distributions, CPU cost |
| `state/drift7_results.json` | stage-0 byte-identity record |
| `state/portfolio7_raw.jsonl` | one JSON line per run (baseline/racer/race) |
| `state/portfolio7_results.json` | computed distributions + hypothesis verdicts |
| `state/portfolio7.csv` | flat per-run table |
| `env.json` | environment/config provenance |
| `logs/` | gitignored solver stderr/stdout transcripts |

## Verdict: H1 failed → portfolio racing NO-GO (pre-registered)

| case | baseline median (max) | W_first median (max) | ratio | racer medians P0/P1/P2/P3 | CPU core-s per race |
| --- | --- | --- | --- | --- | --- |
| m22 | 2.966 s (3.021) | 2.943 s (2.973) | 0.992× | 2.963 / 2.973 / 2.971 / 4.472 | 13.4 |
| rem12 | 15.684 s (15.777) | 15.165 s (15.468) | 0.967× | 15.535 / 15.489 / 15.662 / 15.165 | 62.0 |
| shuffle-win | 52.693 s (53.046) | 51.944 s (52.211) | **0.986×** (gate ≤ 0.75×) | 52.218 / 52.189 / 52.543 / 72.758 | 229.9 |

- **H1 FAIL** (the only gated conjunct for the motivating case): shuffle-win
  W_first median = 0.986× baseline median (needed ≤ 0.75×); race max 52.2 s
  ≤ baseline max 53.0 s (this conjunct trivially passed because *both*
  distributions are tight).
- **H2 pass** (m22 0.992×, rem12 0.967× — no regression, but nothing gained).
- **H3 pass**: 120/120 decisive runs (24 baselines + 96 racers) all `win`,
  zero cap-hits, zero panics, zero non-zero return codes.

## The diagnosis (report finding)

The solver is **deterministic per (FEN, config)**: a sequential run's TT
trajectory is fixed, so each racer's wall is a constant (± machine noise).
The 8 races are 8 replays of the same race — W_first per case spans ~1%
(2.90–2.97 s / 14.95–15.47 s / 51.6–52.2 s). **The plan6 shuffle-win t4
bimodality (5/8 runs 20–39 s, 3/8 at 90–100 s) does not exist in sequential
single-process runs** — of any of the 4 racer configurations (40 racer runs +
16 baseline runs, all within ~1% of the case median). That bimodality was
produced by the SPDFPN shared-TT mechanism itself (thread interleaving over
one table), not by an intrinsic path-dependence a portfolio could trim.
Hypothesis H's premise ("independent configurations diverge bimodally") is
falsified at the root.

Config diversity did vary trajectories (P1/P2 PVs differ from P0's) but not
wall time (~52 s each on shuffle-win); `--epsilon 0.25` (P3) is uniformly
~1.4× slower, consistent with the `conversion` #7 won't-fix record. Cost: a
4-racer race spends ~4.4× the CPU core-seconds of one baseline run to gain
−1.4% wall (and would gain nothing on the bad tail it was built for).

Per plan7.md: "if H1 fails, portfolio racing is measured NO-GO on its
motivating case → the idea joins the no-go record." No `examples/portfolio`
driver ships.
