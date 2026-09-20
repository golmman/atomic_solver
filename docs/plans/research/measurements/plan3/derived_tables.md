# Plan 3 derived tables — subgame material-mass diagnostic

Source: raw stderr dumps in this directory (`plan3_m22_instrumented.txt`,
`plan3_stress_instrumented.txt`, `plan3_dec13_instrumented.txt`). All runs
first-outcome mode (`--first-outcome --outcome-only`), default ε, 128 MB TT
release build. Instrumentation reverted after measurement (see `report3.md`).

Bucket map: `b0` = 1–3 men, `b1` = 4–5, `b2` = 6–8, `b3` = 9–12, `b4` =
13–16, `b5` = 17+. `frame_evals[b]` is the cumulative descendant child-eval
delta of all `dfpn` frames whose *own* position has `b` men (frames nest, so
the per-bucket sums exceed `child_evals`). `frame_cut_evals[b]` is the
threshold-cut slice of the above (plan1's classification: solved and
budget/timeout exits excluded). Harvestable = widened plan13 predicate
(≤5 men, pawnless, no castling) evaluated on the frame's own position.

## m22_white (timeout 30, finished 3.0 s; win length 95)

`child_evals = 14,156,269` (baseline 14,156,269 — bit-identical)

| bucket | men | frames | frame evals | eval share | cut evals | cut share of bucket |
|--------|-----|--------|-------------|-----------:|----------:|--------------------:|
| b0 | 1–3   | 0 | 0 | 0.000% | 0 | — |
| b1 | 4–5   | 0 | 0 | 0.000% | 0 | — |
| b2 | 6–8   | 532 | 69,201 | 0.052% | 38,721 | 55.95% |
| b3 | 9–12  | 50,618 | 2,840,730 | 2.135% | 2,371,005 | 83.46% |
| b4 | 13–16 | 481,235 | 52,429,293 | 39.411% | 50,214,239 | 95.78% |
| b5 | 17+   | 325,732 | 77,687,320 | 58.402% | 54,200,104 | 69.77% |
| Σ  |       | 858,117 | 133,026,544 | 100% | 106,824,069 | 80.30% |

Cumulative eval share at material density ≤ k men: ≤5 = 0.000%, ≤8 = 0.052%,
≤12 = 2.187%, ≤16 = 41.598%.

Harvestable frames: 0, harvestable evals: 0.

## Stress case = m21_white (timeout 300, finished 53.7 s; win length 477)

`child_evals = 249,480,478` (baseline 249,480,478 — bit-identical)

| bucket | men | frames | frame evals | eval share | cut evals | cut share of bucket |
|--------|-----|--------|-------------|-----------:|----------:|--------------------:|
| b0 | 1–3   | 0 | 0 | 0.000% | 0 | — |
| b1 | 4–5   | 0 | 0 | 0.000% | 0 | — |
| b2 | 6–8   | 3,113 | 1,597,438 | 0.074% | 481,518 | 30.14% |
| b3 | 9–12  | 169,454 | 24,898,387 | 1.156% | 20,537,963 | 82.49% |
| b4 | 13–16 | 3,539,905 | 253,801,013 | 11.776% | 245,154,414 | 96.59% |
| b5 | 17+   | 10,194,995 | 1,874,942,230 | 86.994% | 1,485,680,828 | 79.24% |
| Σ  |       | 13,907,467 | 2,155,239,068 | 100% | 1,751,854,723 | 81.29% |

Cumulative eval share at material density ≤ k men: ≤5 = 0.000%, ≤8 = 0.074%,
≤12 = 1.230%, ≤16 = 13.006%.

Harvestable frames: 0, harvestable evals: 0.

## dec13 (rich middlegame control, castling rights KQ, timeout 30, finished 0.87 s; win length 17)

`child_evals = 3,822,602` (no plan1 baseline; internal consistency only)

| bucket | men | frames | frame evals | eval share | cut evals | cut share of bucket |
|--------|-----|--------|-------------|-----------:|----------:|--------------------:|
| b0–b3 | 1–12 | 0 | 0 | 0.000% | 0 | — |
| b4 | 13–16 | 356 | 9,873 | 0.031% | 9,873 | 100% |
| b5 | 17+   | 164,524 | 32,397,615 | 99.970% | 25,667,353 | 79.23% |
| Σ  |       | 164,880 | 32,407,488 | 100% | 25,677,226 | 79.23% |

Harvestable frames: 0, harvestable evals: 0.

## Sanity invariants (vs plan1's frame-level accounting)

| invariant | m22 | stress |
|-----------|----:|-------:|
| Σ frame_bucket_counts = plan1 `frame_total` | 858,117 ✓ | 13,907,467 ✓ |
| Σ frame_bucket_evals = plan1 solved+cut+budget | 133,026,544 ✓ | 2,155,239,068 ✓ |
| Σ frame_bucket_cut_evals = plan1 `frame_cut_evals` | 106,824,069 ✓ | 1,751,854,723 ✓ |
| first-outcome `child_evals` = baseline | 14,156,269 ✓ | 249,480,478 ✓ |

Quick-suite drift: aggregate `child_evals` 38,974,090 instrumented vs
38,974,090 clean; 0/59 per-case mismatches.
