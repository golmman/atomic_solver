# plan5 measurements (2026-10-04) — stage 2a smoke + stage 2b campaign (plan5b)

Shared measurement directory for plan5 (stage 2a, smoke) and plan5b (stage 2b:
W sweep, interleaved campaign, verdict). All runs on the release build,
reference container (Apple Silicon, 4 CPUs), default TT (128 MB), default ε
(0.125), default refine-cap (0.25).

**Stage-2b verdict (plan5b): NO-GO per the pre-registered bands.**
S4 = 1.79× (m22) / 2.68× (shuffle-win) — m22 misses the ≥ 2.0× GO line;
I4 = 2.06× (m22) > 2× fires the NO-GO clause outright; V = 0 decisive
disagreements (2 cap-hit timeouts recorded separately). Condition 1
executed: stage 1b + 2a reverted, byte-identical sequential solver restored
(`git diff` vs pre-plan4b empty, `make test` green, quick suite 59/59,
snapshot sha `eaa5f2b9…`/195 B). The TT-concurrency tax is refunded with the
revert. Details: `report5.md`.

## Command table

| file | command |
| --- | --- |
| `state/baseline_quick_pre5.json` | `benchmark --suite quick --json --first-outcome --runs 1` on the pre-plan5 build (sha256 `7672edeb…`) |
| `state/drift_results.json` | plan5a final N=1 drift check: quick suite 59/59, m22/shuffle-win stdout, snapshot sha256 |
| `state/smoke_m22.json` | plan5a smoke: 5×3 interleaved runs (threads 1/2/4), m22 first-outcome, 30 s cap |
| `sweep_w.py` → `state/sweep_raw.jsonl`, `state/sweep.csv` | plan5b W sweep: patch `MAX_WORK_PER_JOB`, rebuild, 3 reps at `--threads 4` on m22 (30 s cap) per W ∈ {250, 1000, 4000, 10000, 20000, 50000} |
| `campaign.py` → `state/campaign_raw.jsonl` | plan5b campaign: 5 interleaved rounds of N=1,2,3,4 per case (m22 30 s cap, shuffle-win 100 s cap, `--first-outcome --outcome-only`); per run wall/outcome/`[parallel]` counters |
| `analyze_campaign.py` → `state/campaign_summary.json`, `state/campaign.csv` | medians, speedups, inflation, cap-hits, agreement, helper share |
| `logs/moveorder_seqref.json` | `benchmark --suite move-order --first-outcome --runs 1 --timeout 100 --json` — in-session sequential `child_evals` references (m22 14 156 269; m21_white = shuffle-win 249 480 478) |
| `state/quick_post_campaign.json` | plan5b post-campaign N=1 drift: quick suite 59/59 identical vs `baseline_quick_pre5.json` |
| `state/quick_post_revert.json` | post-revert quick suite: 59/59 identical (child_evals pass) |

Logs under `logs/` are gitignored.

## Case FENs (same as plan4b)

- m22: `4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22` (`--timeout 30`)
- shuffle-win (= m21_white): `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21` (`--timeout 100`)
- snapshot: `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1 --timeout 5 --first-outcome --outcome-only --tt-size 16 --tt-dump-path …`

## W sweep (plan5b, m22, t4, 3 reps, median wall)

| W | median wall | note |
| --- | --- | --- |
| 250 | 7.586 s | one 30 s cap-hit |
| 1000 | 4.122 s | |
| 4000 | 3.147 s | one cap-hit |
| **10000** | **1.769 s** | **locked** |
| 20000 | 5.578 s | the landed plan5a value — worse than 10000 |
| 50000 | 3.921 s | |

Variance is high at every W (coordinator perturbation); 10000 wins the
median clearly and was locked before the campaign (doc comment updated in
`src/search/dfpn/parallel/mod.rs`; the only plan5b code change).

## Campaign (plan5b, medians over 5 interleaved reps; sequential N=1 in-session)

| case | N | median wall | speedup | inflation | cap-hits | agreement |
| --- | --- | --- | --- | --- | --- | --- |
| m22 (seq 3.296 s / 14.16 M evals) | 2 | 2.600 s | 1.27× | 1.59× | 0 | 5/5 win |
| | 3 | 2.549 s | 1.29× | 2.25× | 0 | 5/5 win |
| | 4 | 1.842 s | **1.79×** | **2.06×** | 0 | 5/5 win |
| shuffle-win (seq 58.207 s / 249.5 M evals) | 2 | 30.672 s | 1.90× | 1.08× | 1 | 4/4 win |
| | 3 | 30.757 s | 1.89× | 1.57× | 1 | 4/4 win |
| | 4 | 21.734 s | **2.68×** | 1.41× | 0 | 5/5 win |

- Outcome agreement: every decisive run equals the sequential `win`; the two
  cap-hit `draw`s are resource-cut timeouts (rc = 0, wall ≈ cap), not false
  proofs. Zero panics, zero lock failures across all 30 parallel runs.
- Helper work share at t4: 0.73–0.74 on both cases (t2 ≈ 0.48) — helpers do
  real work; the bottleneck is coordinator perturbation, not idling.
- Inflation driver: in ~1 of 5 runs the coordinator's trajectory is steered
  into a bad region (m22 t4 worst total 59.5 M ≈ 4.2× sequential; shuffle-win
  t3 worst 1311.9 M total with coordinator 478 M ≈ 8.5× its healthy median) —
  the same pathology as the plan5a smoke outlier, now median-visible on m22.

## Drift (N = 1) — all green, hashes identical to plan4/plan4b/plan5a records

- Pre-campaign (W = 10000 build) and post-revert: quick suite 59/59 identical;
  m22 stdout `b7c74f17…`; shuffle-win stdout `64129ef0…`; snapshot
  `eaa5f2b9…`/195 B. `make test` green post-revert.

## Verdict inputs (pre-registered bands, plan5b)

- GO needs S4 ≥ 2.0× on both cases, I4 ≤ 2×, V = 0.
- m22: S4 = 1.79× (miss), I4 = 2.06× (miss) → **NO-GO** (the I4 > 2× clause
  fires regardless of speedup; shuffle-win alone would have been GO/MARGINAL).
- V = 0 → the soundness kill points never fired; the miss is purely
  performance/mechanism quality.
