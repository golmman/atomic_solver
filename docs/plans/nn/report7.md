# Report: Gate 4b — residual training iteration (option B)

Plan: `docs/plans/nn/plan7.md`. Status: **fully measured; success bar NOT
met — the wall-time half fails again, but the ordering-quality hypothesis
is validated.** Rust side (Steps 1–3) and trainer side (Step 4, see
`report_trainer_gate_4b.md`) are done; `data/corpus/weights.v2.bin` landed
and Step 5 was executed. Raw outputs: `docs/plans/nn/measurements/gate4b/`.

## What was verified in this repo

All of the following were found implemented and committed (commit
`6e3b201` "implement nn plan 6", which also contains the plan file
itself):

### Step 1 — residual composition at inference (done)

- `src/search/dfpn/history.rs` `sort_moves`: the nearest-commoner map and
  static term are computed unconditionally; when the NN scorer is set its
  scores are **added** to the static term (`static + nn + history +
  killer`). The old `nn_scores.is_none()` branch around the
  nearest-commoner map is gone.
- `examples/move_order_fractions.rs` `rank_samples`: the NN branch ranks
  by `static + nn`, matching `sort_moves`.
- Replacement-semantics doc comments updated to residual semantics:
  `src/nn/mod.rs`, `src/nn/scorer.rs` (incl. the head-scale cap note),
  `AGENTS.md` (nn paragraph), `docs/plans/nn/concept.md` §6.

### Step 2 — spec updates (done)

- `docs/spec/nn.md` §5 documents the `static + s (scaled)` composition;
  §6a adds the **v2 training recipe** (residual logits, per-node
  max-normalized static prior with λ = 1.0, `1 + log2(1 + w)` pair
  weighting per plan decisions 2–3); v1 recipe kept as history.
- `Makefile` `nn_corpus` regenerates at the deep budgeted recipe
  (`--timeout 420 --budget-seconds 19200 --pt-size 1024`).

### Step 3 — corpus regeneration (done)

The plan's Step-3 outcome block is already the record: 59/59 cases
solved in ~2.6 h of the 19,200 s budget, 37 converged, raw rows
73,248 → 24,109 after dedup with **9,552 clean** rows (vs 9,130 at
timeout-20), `dec03/dec07/dec16/dec34` newly converged,
`weights.v1.bin{,.json}` preserved, m20–m22 verified absent. The
artifacts are git-ignored (`data/corpus/train.ndjson` present locally).

## Verification

- `cargo test --release --test test_nn`: 4 passed (solve correctness
  with/without NN scorer, promotion-index dedup, fixture header).
- `cargo test --release --lib nn_scorer_is_residual`: passes — the
  integration-style test asserts that with a fresh search the ordering is
  exactly `static + nn` per the additive rule (plan Step 1's test
  requirement).
- Working tree clean at `6e3b201`; the Step-1/2/3 changes carry no
  uncommitted remainder.

Measurement session (at commit `45287a8`, tree clean apart from this
report and the raw outputs under `docs/plans/nn/measurements/gate4b/`):

- `cargo test --release --test test_nn`: 4 passed (re-run before
  measuring, with `weights.v2.bin` in place).

## Problems encountered (implementation)

- None in the implementation session: the Rust side was found already
  implemented, verified, and committed. The trainer side was subsequently
  delivered (`report_trainer_gate_4b.md`); the measurement-session
  problems are listed under "Problems encountered (measurement)" below.

## Unresolved parts

See "Unresolved parts (updated)" below — the original blockers (Steps 4
and 5) are resolved.

## Missing tests

- None beyond what the plan already flagged: the composition lives in
  `sort_moves` and is covered by the one integration-style test added in
  Step 1; the `--nn-weights` example flags remain untested by convention
  (see `report6.md`).

## Next steps (original plan, superseded below)

1. Hand off to the external trainer (per `trainer_handoff.md`), implement
   the §6a v2 recipe, emit `weights.v2.bin` + summary JSON
   (`recipe: "residual-v2"`), keep the seed-0 fixture byte-frozen.
2. Run the plan7 Step-5 measurement commands (benchmark
   `--suite move-order --first-outcome` with `--nn-weights
   data/corpus/weights.v2.bin`, plus the long-timeout deconfounding runs
   and `move_order_fractions`), judge against the unchanged Gate-4 bar,
   and extend this report with the verdict.

## Step 4 (trainer side) — done

Reported in `report_trainer_gate_4b.md`: v2 recipe (residual logits,
λ = 1.0, `1 + log2(1 + w)` weighting) implemented, 63 trainer tests pass,
fixture byte-verified, `data/corpus/weights.v2.bin` (967,312 bytes,
§10 v1 layout) + summary JSON delivered. Trainer-side validation: the
final v2 weights score 0.505 under the unweighted v1 loss on the
validation split vs 0.504 for the v1 reference — the margins alone are
unchanged, as expected for a residual net whose quality is realized
through the §5 composition.

## Step 5 — Gate-4b re-measurement

Executed at commit `45287a8`, same host for all runs. Per plan7, the
baseline was re-run fresh instead of reusing the `report6.md` numbers
(wall time is host-dependent; the fresh baseline is within ~2% of the
report6 numbers, so the v1 comparisons below stay meaningful). All runs
`--epsilon 0.125 --tt-size 64`, suite `move-order` (19 cases).

### Benchmark (`--first-outcome --timeout 5 --runs 3`)

| aggregate    | baseline | nn v2 | delta |
|--------------|---------:|------:|------:|
| total_nodes  | 5567802  | 3664343 | -34.2% |
| child_evals  | 102.9 M  | 68.1 M | -33.9% |
| total_time s |   34.0   | 36.0 | **+6.0%** |
| ok / timeout | 13 / 6   | 12 / 7 | |
| wrong        |        0 |      0 | pass |

m23_white regressed from solved in 3.33 s to timeout — the same single
case that regressed under v1 (at `--timeout 60` it solves in both
configs, 5.1 s with v2). Every case solved by both configs is slower
with the network (+77%…+193% wall), including trivial sub-0.1 s cases
where the eval counts are equal — the fixed inference cost dominates
there.

### Deconfounding runs (`--timeout 60 --runs 1`)

| case      | baseline evals / time | nn v2 evals / time | eval ratio | time ratio |
|-----------|----------------------:|-------------------:|-----------:|-----------:|
| m22_white | 37.5 M / **10.3 s**   | 27.1 M / 16.2 s    | **0.72x**  | 1.57x |
| m23_white | 9.8 M / 3.3 s         | 9.3 M / 5.1 s      | 0.95x      | 1.55x |
| m20–m21   | 175–256 M / timeout   | 103–114 M / timeout | 0.40–0.62x | 1.00x |
| m22_black | 318.6 M / timeout     | 79.7 M / timeout   | 0.25x      | 1.00x |

**The key Gate-4 artifact is gone.** At fixed effort (m22_white, solved
by both) the network now needs *fewer* evals than the baseline (0.72x;
v1 needed 1.76x) — the residual recipe fixed the ordering quality, not
just the loss. Throughput is unchanged by design (~1.7 M evals/s vs
3.6 M baseline, the ~2.1x dense-inference penalty from report6), so the
quality win still nets out to +57% wall time on that case.

### Ordering quality (`move_order_fractions`)

| metric | baseline | nn v2 |
|--------|---------:|------:|
| aggregate flat rank-1 share | 69.5% | **82.4%** |
| aggregate work-weighted rank-1 share | 31.4% | **70.4%** |
| finalized tree OR nodes | 14,876 | 6,249 |

Both quality metrics improve over the baseline — v1 improved only the
work-weighted one (43.2%) while regressing the flat one (60.9%). The
finalized trees are also less than half the size.

## Verdict against the Gate-4b bar (unchanged from Gate 4)

| criterion | required | measured | result |
|-----------|----------|----------|--------|
| child_evals reduction | >=10-15% | -33.9% (and genuine at fixed effort: 0.72x on m22_white) | pass |
| wall-time reduction   | >=10-15% | **+6.0%** | **fail** |
| wrong                 | == 0     | 0 | pass |

**Gate 4b fails on the wall-time half — the same half as Gate 4 — but
the diagnosis has changed completely.** Gate 4 failed because the
ordering was worse *and* inference was slower; Gate 4b fixes the
ordering (validated at fixed effort and by both ordering-quality
metrics) and what remains is purely the inference-throughput cost, which
plan7 explicitly scoped out ("throughput work is only worthwhile if
quality wins first" — it now does).

## Problems encountered (measurement)

- The fresh baseline differs from `report6.md` by ~2% (5.57 M vs 5.47 M
  nodes) — host/run variance; comparisons in this report are against the
  fresh same-host baseline.
- m23_white flips to timeout at `--timeout 5` under v2 (3.33 s baseline,
  borderline); it solves at 60 s in both configs.

## Unresolved parts (updated)

- The v1 weights remain loadable but semantically stale (trained for
  replacement composition) — correct per plan decision 5, worth
  remembering if anyone benchmarks with `weights.v1.bin` today.

## Missing tests

- None beyond what the plan already flagged: the composition lives in
  `sort_moves` and is covered by the one integration-style test added in
  Step 1; the `--nn-weights` example flags remain untested by convention
  (see `report6.md`).

## Next steps (post-Gate-4b)

1. If the PoC is pursued: attack inference throughput first — batching
   all children through one dense forward pass per node, incremental
   stage-2..5 updates, or a smaller hidden layer. The Gate-4b result
   says a 2.1x throughput recovery converts a +6% wall regression into
   roughly -25%…-30% (quality is now on the right side).
2. If the PoC is closed: the tuned `ScorerParams` baseline remains the
   shipped ordering; `--nn-weights` stays behind the CLI flag (off by
   default). The durable artifacts are the harness, the recorded-work
   labels, and the validated residual+work-weighting training recipe.
