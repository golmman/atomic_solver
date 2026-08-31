# Report: Gate 3 — Rust weight loader and inference path

Plan: `docs/plans/nn/plan5.md`. Status: **implemented and verified**. The
Rust side now loads the §10 weight file, extracts the §2 two-perspective
features, runs the §3 forward pass, and ranks legal moves by
`s[policy_index]` behind `Search::set_nn_scorer` / the CLI `--nn-weights`
flag. Gate 4 measurement is out of scope, as planned.

## What was built

| File | Contents |
|---|---|
| `src/nn/mod.rs` | Module root, re-exports the public API and the pinned dims |
| `src/nn/weights.rs` | §10 loader: 16-byte header validation (magic/version/dims/flags), size check, six float32 LE tensors, `W_1` transposed in memory to `[input][accumulator]`; fixture-driven conformance tests |
| `src/nn/features.rs` | §2 feature extractor (`f = 64 * p + sq`, other view = color swap + file mirror), fixed-capacity `FeatureSets` (no allocation), §5 `policy_index`; the six hand-computed conformance vectors from `test_features.py` are ported as unit tests |
| `src/nn/eval.rs` | Stages 1–5: per-view accumulator (`a = b_1 + Σ W_1[:, f]`, also the §4 incremental primitive), ClippedReLU with **hard-coded max = 1.0**, hidden layer, lazy per-index stage-5 rows; hand-computed fixture forward test and a synthetic-weights clamp test |
| `src/nn/scorer.rs` | `NnMoveScorer`: deduplicates policy indices (promotion variants collapse), maps margins monotonically (round-after-scale, saturating) to the `i32` ordering scale, never thresholds |
| `src/search/dfpn/mod.rs` | `Search::set_nn_scorer` / `nn_scorer()` |
| `src/search/dfpn/history.rs` | `sort_moves` uses the network score in place of the static term when enabled; history + killer stay additive; TT best move still first; the nearest-commoner map is only computed for the static path |
| `src/cli.rs`, `src/main.rs` | `--nn-weights <FILE>` option + help text; loading hard-errors exit 1 |
| `tests/test_nn.rs` | Integration: fixture header/value assertions, search correctness with and without the NN scorer, promotion dedup through the public API |
| `docs/plans/nn/plan5.md` | This plan |
| `AGENTS.md` | Documents `src/nn/`, the flag, and the `weights.rs` size justification |

## Spec erratum (Step 0)

`docs/spec/nn.md` §2 in this repo already states `f = 64 * p + sq ∈ [0, 768)`
(piece-major); the erratum was applied to this copy before the handoff. No
correction was needed and the weight-file version stays 1. Implemented from
the spec with that pinned reading; the conformance vectors (320/711 and
196/326/764 vs 379/579/705) confirm it.

## Tools and verification

- `sha256sum docs/nn_trainer_ref/fixtures/weights.v1.bin` →
  `cb6dafd458d6ad044204f65f4faf378223527eee4ef09e707c9771d4946db2e0` — the
  fixture is intact (verified before implementation; no in-test sha256
  because that would add a hashing dependency; the file size + exact header
  + all known tensor entries + per-tensor nonzero counts pin the bytes).
- `cargo fmt --check` — clean.
- `cargo clippy --all-targets` — 0 warnings.
- `cargo test` (debug) and `make test` (`CARGO_PROFILE_RELEASE_LTO=thin
  cargo test --release`) — all suites pass, 0 failures; the new suites are
  `nn::*` (31 unit tests) and `test_nn` (4 integration tests).
- End-to-end smoke: `cargo run --release -- --nn-weights
  data/corpus/weights.v1.bin --fen "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1"
  --outcome-only` loads the trained production model and solves to
  `win` with `pv: e1e8`.
- `docs/nn_trainer_ref/` untouched (byte-frozen); `data/corpus/` values are
  never used in tests.

## Design notes / deviations

- **`W_1` in-memory layout**: stored transposed `[768][128]` so
  `w1_column(f)` (the §4 incremental-update vector) is one contiguous
  128-float slice; the fresh sparse stage-1 pass uses the same access
  pattern, so wiring the incremental make/unmake stack later only touches
  the accumulator. The file layout is untouched. Note:
  `report_external_trainer.md` open item 5 describes the column as living at
  file offset `16 + 4 * 128 * f`; that offset actually corresponds to the
  transposed layout, while the §10 file stores `W_1` row-major
  `[128][768]` (element `(r, c)` at `16 + 4 * (r * 768 + c)`). The loader
  follows §10 and transposes at load; worth a one-line correction in the
  trainer report if that document is ever revised.
- **Lazy stage 5**: `s` is evaluated only at the deduplicated legal-move
  policy indices instead of all 4096 rows — semantics equal to compute-then-
  mask (§5 allows "drop them"), but ~40× fewer 32-wide dot products per node.
- **Margin → i32 mapping**: `round(s * NN_SCORE_SCALE)`, default scale 4096
  (tunable via `NnMoveScorer::with_scale` for Gate 4). Monotone, saturating;
  close margins may merge but never invert, and scores are only ever sorted.
  The default was chosen so typical margins outweigh history (≤ 10,000) but
  not killers (50,000) — Gate 4 should treat the scale as a tuning knob.
- **Composition**: per `concept.md` §6 the network replaces the static term;
  history/killer/TT ordering stay additive. Unset (`--nn-weights` absent),
  the search is byte-for-byte the old path (the nearest-commoner map is now
  computed lazily, but only when the static scorer runs — same values).
- **`move_order_breakdown`** still reports the static scorer's breakdown;
  it does not reflect a configured NN scorer (documented in its doc comment).

## Problems encountered

- None blocking. The promotion-dedup tests initially assumed one promotion
  target; the test position (`4k3/3P4/...`) offers both `d7d8*` and the
  capture-promotions `d7e8*`, so the tests were tightened to group variants
  by `(from, to)` — which also covers the dedup across two targets.

## Missing tests / known limitations

- No wall-time/child-eval comparison against the hand-crafted ordering —
  that is Gate 4's success bar.
- The fixture conformance is pinned by values, not by an in-repo hash check
  (no `sha2` dependency); re-verify the sha256 if the fixture is ever
  re-copied from the trainer repo.
- The full incremental accumulator (make/unmake stack inside the search
  loop) is deliberately not wired yet; stage 1 is shaped for it
  (`accumulator` over a feature slice, contiguous `W_1` columns).
- `NnMoveScorer` recomputes stages 1–4 per `sort_moves` call (twice per node
  expansion at most). Fine for Gate 4's first measurement; the incremental
  path or a small position→hidden cache is the obvious follow-up if
  inference overhead shows up.

## Next steps

1. Gate 4: re-run `move_order_fractions` and
   `benchmark --suite move-order --first-outcome --json` with and without
   `--nn-weights data/corpus/weights.v1.bin` at identical
   `--epsilon/--tt-size`; success bar ≥ 10–15% fewer `child_evals` **and**
   wall time with `wrong == 0`; tune `NN_SCORE_SCALE` alongside.
2. Consider wiring the §4 incremental accumulator into the search's
   make/unmake if the dense recompute shows up in the profile.
3. Retrain with the m20–m22 move-order cases actually contributing rows
   (they hold out 0 rows in the current corpus), then re-measure.
