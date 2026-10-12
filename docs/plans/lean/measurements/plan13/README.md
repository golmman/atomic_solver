# Plan 13 measurements (#21 TT retention probe)

Executed 2026-10-10 on the x86_64 container (see `env.json`). Temporary
`ATOMIC_TTPROBE` hook in the TT (counters + evicted-key shadow), driven by the
temporary `examples/tt_retention` driver; one deterministic run per cell,
fresh process per cell, 128 MB TT throughout. Raw transcripts in `raw/`
(uncommitted); `state/results.json` is the parsed committed record.

## Per-cell parsed results (fractions as decimals)

| cell | outcome | child_evals | reprobe/hits | victim-live/stores | victim s/u | work mean (live / evicted / re-probed) |
| --- | --- | --- | --- | --- | --- | --- |
| m22_fo_s0 | Win 1.5 s | 14.16 M | 0.44% | 1.55% | 33/13248 | 17.2 / 17.4 / 24.3 |
| m22_fo_s3 | Win 1.9 s | 17.83 M | 0.58% | 2.31% | 70/24439 | 17.3 / 17.7 / 30.8 |
| m22_default_s0 | Win 2.1 s | 17.70 M | 0.95% | 1.53% | 50/16974 | 17.2 / 18.3 / 25.7 |
| m22_default_s3 | Win 2.7 s | 25.79 M | 0.88% | 1.22% | 82/26179 | 17.4 / 18.1 / 24.2 |
| m20_fo_s0 | Draw (60 s timeout) | 546.5 M | **18.75%** | **71.34%** | 108105/21107120 | 38.7 / 59.0 / 84.0 |
| m20_fo_s3 | Draw (60 s timeout) | 507.0 M | **20.06%** | **70.58%** | 86800/19462628 | 22.8 / 39.7 / 71.6 |
| dec10_fo_s0 | Win 0.44 s | 4.26 M | 0.03% | 0.00% | 0/27 | 21.9 / 13.6 / 3.1 |
| dec10_fo_s3 | Win 0.46 s | 4.80 M | 0.02% | 0.00% | 0/26 | 21.5 / 15.4 / 2.5 |
| dec13_fo_s0 | Win 0.41 s | 3.82 M | 0.03% | 0.09% | 7/137 | 26.2 / 34.5 / 66.5 |
| dec13_fo_s3 | Win 0.55 s | 5.03 M | 0.02% | 0.15% | 13/303 | 25.3 / 35.9 / 54.8 |
| m19_fo_s0 | Draw (60 s timeout) | 540.0 M | **17.40%** | **70.24%** | 87527/19912392 | 24.4 / 41.6 / 86.1 |
| m19_fo_s3 | Draw (60 s timeout) | 583.9 M | **15.52%** | **71.82%** | 101986/21889470 | 28.2 / 48.5 / 87.1 |
| dec01_fo_s0 (control) | Win 0.56 s | 5.71 M | 0.06% | 0.21% | 4/596 | 22.1 / 27.5 / 60.2 |
| dec03_fo_s0 (control) | Win 0.006 s | 47 K | 0% | 0% | 0/0 | 49.8 / – / – |
| m25_fo_s0 (control) | Win 0.003 s | 22 K | 0% | 0% | 0/0 | 22.1 / – / – |
| m29_fo_s0 (control) | Win 0.000 s | 1 | 0% | 0% | 0/0 | 1.0 / – / – |

Saturation check (m19_fo_s0): 4,187,827 live of 4,194,304 slots (99.8%) —
the table is pinned; every eviction is a live eviction, so victim-live ≈ 70%
of stores is a capacity artifact (≈8.5 M of 28.5 M stores are updates of
existing entries, the rest all evict). The policy-relevant signal is the
re-probe loss, with healthy denominators (93–96 M probe hits per stress cell).

## Gate verdicts (pre-registered in plan13.md)

- **K1 (kill): FAILS.** m19/m20 exceed both margins by an order of magnitude.
- **T1 (fire → plan14): FIRES** on both disjuncts: re-probe loss ≥ 2% of hits
  on m19 and m20 (both salts), victim-live ≥ 20% of stores on the same four
  cells. Gray-zone protocol not needed (max solvable-cell re-probe 0.95% vs
  min stress-cell 15.5% — no ambiguity).
