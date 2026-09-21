# plan8 measurement summary

Raw per-run outputs: `clean_stress_e<eps>.{out,err}` (clean-HEAD baselines),
`spike_<case>.{out,err}` (Phase 0 instrumented runs, `RESEARCH8_STATS=1`),
`stats_<case>.csv` (Phase 0 separation tables), `p1_<arm>_<case>.{out,err}`
(Phase 1 arm runs), `quick_*.json` (bit-identity / outcome-preservation
proofs), `postrevert_stress_e0.375.{out,err}` (post-revert reproduction),
`analyze.py` (table analysis script).

## Setup

- Cases: stress = m21_white
  (`4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`),
  m22_white (`4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22`),
  dec13 (`r1bq1k1r/ppN4p/n1p1p3/3p1n1P/1b1P2P1/2P5/PP6/RNBQKB1R w KQ - 1 14`),
  dec10 (`3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41`).
- All runs `--first-outcome --outcome-only`, default TT (128 MB), default
  refine-cap; `--epsilon` per the run matrix. Timeouts: stress 300 s,
  m22 30 s, dec13 30 s, dec10 60 s (plan4's bounds).
- Spike code: one throwaway module `src/search/dfpn/spike8.rs` plus hooks in
  `core.rs` (cut-site capture + resolution events), `history.rs`
  (`sort_moves` static-score retention), `mod.rs` (field + end-of-run dump).
  Env-gated: `RESEARCH8_STATS=1` (recording), `RESEARCH8_DUMP=1`
  (child-eval stderr dump), `RESEARCH8_ARM` (Phase 1 arm), `RESEARCH8_OUT`
  (CSV path), `RESEARCH8_CAP_LOG2` (table capacity, default 24).
  All reverted before this report (`git diff --exit-code -- src/` clean).

## Event-log / table contract (pre-registered)

- Keyed by `pos.repetition_key()` (board-level Zobrist hash including side
  to move; rule50-independent so the same board maps to one key).
- Fixed open-addressing table, `2^24` slots × 32 B = 512 MB, 7/8 load cap;
  on overflow recording **stops** (counted, never evicts). Overflow
  occurred on **no** run (`overflow_dropped=0` everywhere; stress used
  7.59 M / 7.42 M slots, controls < 0.6 M).
- One record per key, first cut wins; per-cut eval mass accumulates.
- Features (first cut): side; depth bucket (1–4/5–8/9–12/13–16/17–24/25–40/41+);
  `second/best` ratio bucket ([1,1.25)/[1.25,1.5)/[1.5,2)/[2,3)/[3,10)/[10,∞)/n-a
  on the node-type's primary number (pn@OR, dn@AND); `epsilon_ceil(second) −
  best` bucket (1/2/3/4/5–8/9+/n-a); parent-clamp flag (`ε(second) ≥ th`);
  primary-dimension fire flag; exiting gap `bound − th` bucket (same scale);
  static ordering margin `static(second) − static(best)` bucket, captured
  from the frame's `sort_moves` scores (≤−32/−31..−16/−15..−8/−7..−4/−3..−1/
  0/1..3/4..7/8..15/16..31/≥32/n-a).
- The plan's "iteration round" feature was dropped: all measured runs are
  `--first-outcome`, where `max_depth` is constant `u32::MAX` — no round
  variation exists to record.
- Resolution classes (end of run): **rescued** (decisive outcome after the
  cut), **draw-resolved** (only a Draw after the cut), **resolved-before**
  (resolution preceded the cut), **dead** (never resolved in-run).
  The kill gate's rescue rate is **mass-weighted**
  (`rescued_mass / (rescued_mass + dead_mass)`), consistent with the
  metric of record (`child_evals`); draw-resolved and resolved-before are
  excluded from the denominator and reported separately.

## Drift / verification trail (all on this container)

- Clean-HEAD quick suite (`benchmark --suite quick --json --first-outcome
  --timeout 5`): aggregate `total_child_evals` = 38,974,090, 59/59 solved,
  0 wrong (matches plan4's clean-HEAD aggregate exactly).
- Same suite with the full spike present, env unset: bit-identical in every
  search-relevant field (only `total_time` and per-case wall-clock fields
  differ) — `quick_clean.json` vs `quick_spikeoff.json`.
- Clean-HEAD stress runs reproduce plan4's per-chunk stderr fingerprints
  (`work_done`/`next_chunk`/`nodes`/`max_depth` identical, 8 chunks at both
  ε=0.125 and ε=0.375).
- Instrumented stress runs (stats ON) reproduce the two gate-object
  baselines **exactly** and with identical fingerprints:
  ε=0.125 → `child_evals=249480478`; ε=0.375 → `child_evals=227834433`.
- Controls at ε=0.375 reproduce their baselines exactly: m22 12,351,299;
  dec13 3,921,011; dec10 3,642,166.
- Post-revert stress run (ε=0.375): win length 199 and per-chunk fingerprint
  identical to the instrumented spike-off run — the search trajectory is
  unchanged by the reverted instrumentation.
- Housekeeping at plan close: `cargo fmt --check` clean,
  `cargo clippy --release --all-targets` clean, `make test` green.

## Phase 0 — separation tables

Headline counters (full tables in `stats_*.csv`, analysis in `analyze.py`):

| Run | cut frames | cut keys | cut eval mass | rescued keys | dead keys | base rescue rate (keys) | base rescue rate (mass) |
|---|---|---|---|---|---|---|---|
| stress ε=0.125 | 13,520,001 | 7,355,377 | 1,710,835,713 | 40,765 | 7,314,032 | 0.55% | 14.16% |
| stress ε=0.375 | 12,735,935 | 7,203,307 | 1,552,703,501 | 33,656 | 7,168,934 | 0.47% | 12.95% |
| m22 ε=0.375 | 703,881 | 528,497 | 81,221,236 | 4,789 | 523,687 | 0.91% | 31.24% |
| dec13 ε=0.375 | 143,730 | 125,435 | 28,174,480 | 2,226 | 123,204 | 1.77% | 14.17% |
| dec10 ε=0.375 | 473,698 | 48,577 | 39,003,562 | 1,813 | 46,742 | 3.73% | 83.96% |

Draw-resolved and resolved-before keys are tiny everywhere (≤ 0.04% of keys;
≤ 1.4% of mass), so the rescued/dead split dominates.

### Kill-gate evaluation (stress ε=0.375, the operative mix)

Single features (mass-weighted): **no bucket with ≥ 5% coverage reaches 2×
lift**. Best single-feature lifts at ≥ 5% coverage: rank-margin=0 1.44×
(36.4%), OR-side 1.31× (46.4%), depth 1–4 1.30× (29.8%), exit-gap `n/a`
1.94× (7.9%). The plan6 class-B replication holds: 97.0% of cut mass sits
at ratio < 1.25 and `ε(second) − best = 1` (buckets 0 of `ratio`/`gap`);
97.7% of cuts fire on the primary dimension.

Composite (two-feature) candidates — **the kill gate fails on these**:

| Bucket | Coverage (mass) | Rescue rate | Lift |
|---|---|---|---|
| OR ∧ depth 1–4 | 8.5% | 31.4% | 2.42× |
| depth 1–4 ∧ static-margin = 0 | 7.3% | 35.3% | 2.73× |
| exit-gap n/a ∧ static-margin −15..−8 | 7.0% | 28.4% | 2.19× |

Sharpest form of the first bucket: OR ∧ depth 1–4 ∧ non-clamped = 7.8% of
mass at 34.3% (2.65×); OR ∧ depth 1–4 ∧ non-clamped ∧ margin = 0 = 6.9% at
36.9% (2.85×).

### ε mix-shift (stress ε=0.125 vs ε=0.375)

The separation is **not** a stable node-local property — it tracks the
trajectory the global constant produces:

| Bucket | lift @ ε=0.375 | lift @ ε=0.125 |
|---|---|---|
| OR ∧ depth 1–4 | 2.42× | 0.99× (rate 14.1% ≈ base 14.2%) |
| OR ∧ depth 1–4 ∧ non-clamped | 2.65× | 1.93× |
| AND ∧ tie (ratio < 1.25) | 0.70× | 0.59× |
| exit-gap n/a | 1.94× | 0.13× |

At ε=0.125 **no single or composite bucket passes** the kill gate at all.

## Phase 1 — conditioned-ε arm table (base ε=0.375 via `--epsilon`)

| Arm | Definition | stress (227,834,433) | m22 (12,351,299) | dec13 (3,921,011) | dec10 (3,642,166) |
|---|---|---|---|---|---|
| OR_SHALLOW | ε=1.0 @ OR ∧ depth≤4, else base | 910,954,290 (+300.0%) | 21,819,225 (+76.7%) | 4,199,073 (+7.1%) | 3,967,790 (+9.0%) |
| OR_SHALLOW_NC | OR_SHALLOW restricted to non-clamped frames | 910,954,290 (+300.0%) | ≡ OR_SHALLOW (see below) | ≡ | ≡ |
| AND_TIGHT | ε=0.0 @ AND ∧ tie (second < 1.25·best), else base | timeout (draw @ 2.71 B evals / 300 s) | 17,406,960 (+40.9%) | 4,828,633 (+23.1%) | 3,419,891 (−6.1%) |

- OR_SHALLOW ≡ OR_SHALLOW_NC is mathematically forced: raising ε can never
  push `min(th, ε(second))` below its saturation point at clamped frames
  (both yield `th`), so the "non-clamped only" restriction is a no-op for a
  pad-more mapping. The plan's mandatory non-clamp arm is only
  distinguishable for pad-less mappings, which AND_TIGHT already covers.
- Quick-suite outcome preservation (`--runs 1`, ε=0.375): both arms flip
  2 outcomes — `dec01` and `m23_white` `win → draw`, both via 5-second
  timeouts (`status: timeout`), i.e. the arms explode even small positions;
  no soundness divergence on any finishing run.
- Every GO gate fails: no arm is within an order of magnitude of the ≥ 10 %
  stress win; all controls outside +5% (except dec10 under AND_TIGHT, which
  still fails on stress and m22/dec13); quick outcomes not preserved.

**Gate decision: NO-GO (H2).**
