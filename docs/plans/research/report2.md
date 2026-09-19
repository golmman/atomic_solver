# Report 2: Outlier hard-position cluster analysis

## Summary

Across all five benchmark suites (default, move-order, decisive, quick, thorough)
run in first-outcome mode with a 60 s timeout, the hardest positions are **not**
structurally diverse. One material configuration — `BNPPPPPPPRR vs PPPPPPR` (20
men, pawns present, no castling) — accounts for **58.3 % of all outlier
occurrences** and **67 % of unique outlier positions**. These are the
consecutive positions from a single known atomic-chess game (m20–m23 in the
move-order suite). The remaining decisive-suite outliers are structurally
diverse and do not cluster around any single feature.

## Method

1. Built release and ran every suite with `--first-outcome --timeout 60 --runs 1 --json`.
2. Parsed JSON, computed median and 90th-percentile `child_evals` per suite,
   and flagged outliers as any case with `child_evals > max(2×median, p90)` or
   any timeout.
3. Built a `name → FEN` lookup from `examples/benchmark.rs` and the fixture files
   under `tests/fixtures/`.
4. Extracted 16 pure-FEN features per outlier (material, pawn structure, king
   geometry, castling, rule-50, etc.).
5. Compared outlier feature frequencies to the full 197-case population.

Raw data files:
- `docs/plans/research/measurements/plan2/{default,move-order,decisive,quick,thorough}.json`
- `docs/plans/research/measurements/plan2/outlier_features.csv`
- `docs/plans/research/measurements/plan2/summary.txt`

## Per-suite outlier table

| Suite | Cases | Solved | Timeouts | Median child_evals | P90 child_evals | Outlier threshold | Outliers |
|---|---|---|---:|---:|---:|---:|
| default | 8 | 8 | 0 | 7,803 | 294,599,281 | 294,599,281 | 0 |
| move-order | 19 | 15 | 4 | 5,146 | 14,156,269 | 14,156,269 | 5 |
| decisive | 46 | 46 | 0 | 71,200 | 1,507,474 | 1,507,474 | 4 |
| quick | 59 | 59 | 0 | 39,549 | 2,397,489 | 2,397,489 | 5 |
| thorough | 65 | 61 | 4 | 47,068 | 2,892,606 | 2,892,606 | 10 |

**Total unique outlier positions:** 12 (24 occurrences across suites).

## Unique outlier positions (sorted by max child_evals seen)

| Name | Max child_evals | Timeout? | Outcome | PV len | Material | Men | Pawnless | Preflight | King dist | Rule-50 |
|---|---|---:|---|---:|---|---:|---|---|---:|---:|
| m21_black | 522,362,085 | **Yes** | draw | 0 | BNPPPPPPPRR vs PPPPPPR | 20 | No | No | 7 | 0 |
| m22_black | 400,469,049 | **Yes** | draw | 0 | BNPPPPPPPRR vs PPPPPPR | 20 | No | No | 7 | 0 |
| m20_black | 276,188,883 | **Yes** | draw | 0 | BNPPPPPPPRR vs PPPPPPR | 20 | No | No | 7 | 5 |
| m20_white | 275,777,343 | **Yes** | draw | 0 | BNPPPPPPPRR vs PPPPPPR | 20 | No | No | 7 | 4 |
| m21_white | 249,480,478 | No | win | **477** | BNPPPPPPPRR vs PPPPPPR | 20 | No | No | 7 | 0 |
| m22_white | 14,156,269 | No | win | 95 | BNPPPPPPPRR vs PPPPPPR | 20 | No | No | 7 | 0 |
| m23_white | 9,673,403 | No | win | 33 | BNPPPPPPPRR vs PPPPPPR | 20 | No | No | 7 | 1 |
| dec01 | 5,713,706 | No | win | 27 | NPPPPQR vs BPPPPPPPRR | 19 | No | No | 6 | 0 |
| dec10 | 4,262,128 | No | win | 41 | BPPPPPRR vs PPPPPPRR | 18 | No | No | 7 | 5 |
| dec13 | 3,822,602 | No | win | 17 | BBNNPPPPPPQRR vs BBNNPPPPPPQRR | 28 | No | No | 7 | 1 |
| m23_black | 2,892,606 | No | loss | 24 | BNPPPPPPPRR vs PPPPPPR | 20 | No | No | 7 | 2 |
| dec14 | 2,397,489 | No | win | 21 | BBNPPPPPPPQRR vs BNPPPPPPPPQRR | 28 | No | No | 7 | 0 |

## Feature frequencies

### Material configuration

| Material config | Outliers | Full suite | Concentration ratio |
|---|---:|---:|---:|
| BNPPPPPPPRR vs PPPPPPR | 14/24 (58.3 %) | 22/197 (11.2 %) | **5.2×** |
| NPPPPQR vs BPPPPPPPRR | 3/24 (12.5 %) | 3/197 (1.5 %) | 8.3× |
| BPPPPPRR vs PPPPPPRR | 3/24 (12.5 %) | 3/197 (1.5 %) | 8.3× |
| BBNNPPPPPPQRR vs BBNNPPPPPPQRR | 3/24 (12.5 %) | 3/197 (1.5 %) | 8.3× |
| BBNPPPPPPPQRR vs BNPPPPPPPPQRR | 1/24 (4.2 %) | 3/197 (1.5 %) | 2.8× |

The `BNPPPPPPPRR vs PPPPPPR` configuration is a 5.2× over-representation in the
tail. The other four configs are also over-represented but each appears only in
the decisive suite.

### Other features

| Feature | Outlier share | Full-suite share | Interpretation |
|---|---|---|---|
| Pawnless = False | 24/24 (100 %) | 196/197 (99.5 %) | Universal; not discriminative. |
| Pre-flight eligible | 0/24 (0 %) | 0/197 (0 %) | Every position has >3 men; no pre-flight misses. |
| Total men = 20 | 14/24 (58.3 %) | 31/197 (15.7 %) | Same as dominant material config. |
| Total men = 28 | 4/24 (16.7 %) | 15/197 (7.6 %) | dec13/dec14 are rich middlegames. |
| King Chebyshev dist = 7 | 21/24 (87.5 %) | 142/197 (72.1 %) | Modest over-representation; most benchmark kings are far apart. |
| Rule-50 = 0 | 11/24 (45.8 %) | 119/197 (60.4 %) | Slight *under*-representation. |
| Rule-50 = 5 | 5/24 (20.8 %) | 8/197 (4.1 %) | 5.1× over-representation (m20b, dec10, and three others). |
| No castling | 20/24 (83.3 %) | ~145/197 (~74 %) | Mild over-representation. |

Correlation `total_men` vs `log10(child_evals)` among solved cases is **r = 0.189**
(very weak), confirming that raw material count is not a strong predictor of
difficulty.

## Interpretation

### The dominant outlier “class” is a single position family

The 14 occurrences of `BNPPPPPPPRR vs PPPPPPR` correspond to **eight unique
positions** (m20_white, m20_black, m21_white, m21_black, m22_white, m22_black,
m23_white, m23_black). These are consecutive plies from the same known atomic
chess game. They are not a generalizable structural class like “all RB vs R
endgames” — they are a *position family* that the move-order suite was
explicitly constructed to stress-test.

Within this family:
- **4 of 8 timeout** at 60 s (m20w, m20b, m21b, m22b).
- m21_white solves but emits a **477-move PV** — the longest by far in the
  entire benchmark matrix — suggesting an extremely deep conversion or
  repetition line.
- m22_white solves in 14 M evals (vs 249 M for m21w), showing that a single
  ply difference can change difficulty by an order of magnitude.

### The decisive-suite outliers are structurally diverse

The four decisive outliers span four distinct material configs, two king
distances (6 and 7), and a wide range of piece densities (18–28 men). They share
no single structural signature beyond “not pawnless” and “no castling,” both of
which are near-universal in the benchmark population.

### No pre-flight misses

Zero outliers are pre-flight eligible (`total_men <= 3 && pawnless &&
no_castling`). The pre-flight detector is not a bottleneck for any of the
hardest positions in these suites.

## Hand-off decision

The outlier tail is **dominated by a single position family** (the m20–m23
sequence), but that family is already the primary optimization target of the
`conversion` and `lean` initiatives. The decisive-suite outliers, by contrast,
are **structurally diverse** and would not benefit from a class-specific
recognizer or EGTB expansion.

Therefore:

1. **Do not open a new class-specific initiative** (e.g., 4-man EGTB or a new
   recognizer) — there is no new structural class to target.
2. **Continue generic-lever work** for the diverse decisive-suite tail. The POC
   candidates #11 (dynamic ε), #12 (TT eviction), and #13 (frontier prior)
   remain the highest-expected-value paths, because they address difficulty
   across arbitrary material configurations.
3. **Note for `conversion`/`lean`**: the m20–m23 family still times out at 60 s,
   confirming significant headroom remains on the baseline they already own.

**Plan 3 recommendation:** Pursue POC candidate #11, #12, or #13 (structurally
diverse justification). If one of those shows measurable improvement on the
m20–m23 family without regressing the decisive suite, it is the right next
step.
