# Plan 2: Outlier hard-position cluster analysis

Initiative: `research` backlog #2 (problem inventory).

## Goal

Identify which positions across all benchmark suites are outlier-hard (highest
`child_evals` to first decisive outcome), extract FEN-derived structural
features for those outliers, and test whether they cluster around shared
material or positional signatures. The report answers: is there a single
structural class that dominates the tail of the distribution, or are hard
positions structurally diverse?

This is a pure-analysis plan: no `src/` change, no optimization attempt, no
new CLI flags.

## Context

Plan 1 showed that >98% of `evaluate_child` calls return unsolved and ~80% of
cumulative descendant evals sit inside threshold-cut `dfpn` frames. That tells
us *where* the work goes, but not *which* positions concentrate it. The stress
case (`4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`) is one
data point; the `m22_white` class is another. Before spending a POC on a
specific lever (dynamic ε, TT eviction, frontier priors), we need to know
whether the hardest positions share a feature that an existing initiative
could address (e.g., a pre-flight recognizer expansion, an EGTB class, or a
dep-conversion heuristic).

`lean` and `conversion` have optimized against the `m22_white` and
shuffle-win baselines. If the true outliers in the full suite cluster around
a different feature (e.g., all are ≤4-man endgames, or all contain a
bishop-vs-knight imbalance, or all have pawns on the 7th rank), the next
lever should target that class specifically.

## Method

### Step 1 — Run the full benchmark matrix

Build release and run every suite in first-outcome mode with a timeout long
enough that the decisive suites solve and only the genuinely unsolved cases
time out:

```bash
cargo build --release

for suite in default move-order decisive quick thorough; do
  target/release/examples/benchmark \
    --suite "$suite" \
    --first-outcome \
    --timeout 60 \
    --runs 1 \
    --json \
    > "docs/plans/research/measurements/plan2/${suite}.json"
done
```

`--suite all` is avoided here because the suites have heterogeneous timeout
needs; keeping them separate makes per-suite medians meaningful. The
`decisive` and `thorough` suites may need 60 s; `quick` and `default` solve
comfortably in 5 s. Timeouts are recorded as data (a timeout is itself an
indicator of hardness).

### Step 2 — Parse JSON and define outliers

For each suite, compute:
- median `child_evals` among solved cases
- 90th percentile
- absolute top-N (e.g., top 5)

An outlier is any case with `child_evals` > max(2× median, 90th percentile).
Also include every timed-out case as an automatic outlier regardless of
budget consumed.

### Step 3 — Build name→FEN lookup

The benchmark JSON emits `name` but not `fen`. Construct a lookup table by
parsing the hardcoded suites in `examples/benchmark.rs` and the fixture files
under `tests/fixtures/` (`move_order_positions.txt`,
`decisive_remaining.txt`, etc.). A small Python or Rust script under
`measurements/plan2/` does this once.

### Step 4 — Extract FEN-derived structural features

For each outlier FEN, extract features that require no legal-move generator
(pure FEN parsing):

| Feature | How derived |
|---|---|
| `side_to_move` | FEN active color field |
| `total_men` | Sum of all piece letters in placement field |
| `white_men` / `black_men` | Per-side counts |
| `pawnless` | `white_pawns == 0 && black_pawns == 0` |
| `preflight_eligible` | `total_men <= 3 && pawnless && no_castling` |
| `material_config` | Sorted tuple of non-pawn material per side, e.g. `(RB, R)` |
| `white_pawns` / `black_pawns` | Count from FEN placement |
| `max_white_pawn_rank` | Highest rank (1–8) with a white pawn |
| `min_black_pawn_rank` | Lowest rank with a black pawn |
| `white_king_file/rank` | File `a–h`, rank `1–8` |
| `black_king_file/rank` | File `a–h`, rank `1–8` |
| `king_chebyshev_dist` | `max(\|file_diff\|, \|rank_diff\|)` |
| `castling_rights` | `KQkq` present? |
| `rule50` | FEN halfmove clock |
| `fullmove` | FEN fullmove number |
| `has_queen` / `has_rook` / `has_bishop` / `has_knight` | Per-side boolean flags |

These are cheap to compute and cover material, pawn structure, king
geometry, and phase.

### Step 5 — Cluster and compare distributions

For each feature, compare the outlier distribution to the full-suite
distribution:

1. **Material-configuration tabulation** — count outliers per material
   configuration. If one config (e.g., `RB vs R`) dominates, that points to
   an endgame recognizer or EGTB expansion.
2. **Pawn-structure concentration** — are outliers disproportionately
   pawnless? High pawn count? Pawns on the 7th rank?
3. **King-distance / rule50 correlation** — is there a correlation between
   `king_chebyshev_dist` or `rule50` and `child_evals`?
4. **Pre-flight miss rate** — what fraction of outliers are
   `preflight_eligible` but still timed out / high-eval? If non-zero, the
   pre-flight detector is misfiring or the budget is too tight for those
   cases.

No sophisticated ML clustering is needed; frequency tables, histograms, and
simple ratios are sufficient.

### Step 6 — Hand-off decision

If a structural class accounts for ≥50% of outliers by a single feature (or a
conjunction of two features), that class becomes a formal candidate for the
next plan or for hand-off to an implementation initiative:

| Class | Likely owner |
|---|---|
| Pawnless ≤4 men | `egtb` (reopen for 4-man) or `conversion` (recognizer) |
| Pawnless >4 men, no castling | `conversion` (deep tempo / progression) |
| Material-rich, pawns present, high rule50 | `dfpn` (reopen for repetition-dominated class) |
| Diverse, no single feature | `lean` or `conversion` (generic lever: ordering, ε, TT) |

If the outliers are structurally diverse, the finding supports pursuing a
generic lever (e.g., the POC candidates #11–#13) rather than a class-specific
one.

## Deliverables

- `docs/plans/research/measurements/plan2/{default,move-order,decisive,quick,thorough}.json`
- `docs/plans/research/measurements/plan2/outlier_features.csv` (one row per
  outlier)
- `docs/plans/research/measurements/plan2/summary.txt` (frequency tables)
- `docs/plans/research/report2.md` (interpretation, hand-off decision)

## Verification

- `cargo fmt --check` and `cargo clippy --release --all-targets` (the
  benchmark example must still compile cleanly).
- `make test` (fast gate) — no test breakage because no `src/` code changes.

## Non-goals

- No modification to `src/` or `examples/` source. The analysis script lives
  under `measurements/plan2/` and is not part of the build.
- No attempt to optimize positions, build recognizers, or change search
  semantics.
- No per-depth or per-move histograms (frame granularity belongs to a
  follow-up plan if the outlier class demands it).
- No training of learned models; feature extraction is hand-coded from FEN.

## Out of scope

- Plan 3 will be chosen from:
  - the POC candidates (#11 dynamic ε, #12 TT eviction, #13 frontier prior)
    if outliers are structurally diverse, **or**
  - a class-specific spike (e.g., 4-man EGTB feasibility, mating-net
    recognizer, repetition-class diagnostic) if a single class dominates.

## Final task

Write `docs/plans/research/report2.md`: include the per-suite outlier table,
raw feature frequencies, the interpretation of whether outliers cluster, the
hand-off decision, and the raw data file paths.
