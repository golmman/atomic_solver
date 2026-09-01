# Report: Gate 4b — residual training iteration (option B)

Plan: `docs/plans/nn/plan7.md`. Status: **partially implemented — Rust side
(Steps 1–3) done and committed; trainer side (Step 4) and re-measurement
(Step 5) pending in the external repo.** The Gate-4b success bar is
therefore **unjudged**; this report covers the in-repo portion only and
will be extended once `data/corpus/weights.v2.bin` lands.

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

## Problems encountered

- None in this session: the Rust side was found already implemented,
  verified, and committed. The only gap is the external half of the plan.

## Unresolved parts

- **Step 4 (external trainer):** the §6a v2 recipe (residual logits +
  work weighting) is not yet implemented in the trainer repo; no
  `weights.v2.bin` exists in `data/corpus/`. `docs/plans/nn/trainer_handoff.md`
  already reflects the §5/§6a changes for the next handoff.
- **Step 5 (Gate-4 re-measure):** blocked on Step 4. The harness and
  success bar are reused verbatim from `report6.md`/plan7 Step 5; the v1
  baseline numbers stand unchanged (baseline loads no weights).
- The v1 weights remain loadable but semantically stale (trained for
  replacement composition) — correct per plan decision 5, worth
  remembering if anyone benchmarks with `weights.v1.bin` today.

## Missing tests

- None beyond what the plan already flagged: the composition lives in
  `sort_moves` and is covered by the one integration-style test added in
  Step 1; the `--nn-weights` example flags remain untested by convention
  (see `report6.md`).

## Next steps

1. Hand off to the external trainer (per `trainer_handoff.md`), implement
   the §6a v2 recipe, emit `weights.v2.bin` + summary JSON
   (`recipe: "residual-v2"`), keep the seed-0 fixture byte-frozen.
2. Run the plan7 Step-5 measurement commands (benchmark
   `--suite move-order --first-outcome` with `--nn-weights
   data/corpus/weights.v2.bin`, plus the long-timeout deconfounding runs
   and `move_order_fractions`), judge against the unchanged Gate-4 bar,
   and extend this report with the verdict.
