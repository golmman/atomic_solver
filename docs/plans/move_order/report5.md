# Report 5: Configurable move-ordering parameters

This report documents the implementation of `docs/plans/move_order/plan5.md`:
moving the hand-tuned `StaticAtomicScorer` constants into a TOML config file
and loading them through a new `--config <PATH>` CLI option (with a
`SCORER_CONFIG` environment-variable fallback).

## Summary

- All `StaticAtomicScorer` constants and the piece-value table used by atomic
  SEE are now configurable at runtime.
- The compiled-in defaults are unchanged, so existing behavior is preserved
  when no config is supplied.
- `ScorerParams` validates loaded values to prevent collapsing the score
  hierarchy that the solver relies on.
- The benchmark and move-order diagnostic examples support `--config`, making
  hill-climbing iteration possible without recompiling.
- No benchmark position returned a wrong decisive outcome, and the `m22_white`
  refined PV length stayed at 23 plies, matching the `report4.md` baseline.

## Files changed

- `Cargo.toml` — added `serde` (with `derive`) and `toml` dependencies.
- `config.toml` — new default config file at the repository root.
- `src/lib.rs` — re-exported `pub mod config`.
- `src/config.rs` — new loader for `ScorerParams` from TOML.
- `src/cli.rs` and `src/cli.rs` unit tests — added `config_path` / `--config`.
- `src/main.rs` — loads a config (if any) and calls `Search::set_scorer`.
- `src/search/ordering.rs` — `StaticAtomicScorer` now carries `ScorerParams`;
  all score constants are read from `self.params`.
- `src/search/ordering/params.rs` — new `ScorerParams`, `PieceValues`,
  `Default`, and `validate` implementation.
- `src/search/ordering/tests.rs` — updated to use `StaticAtomicScorer::default()`
  and `ScorerParams::default()`.
- `src/search/dfpn/mod.rs` — `Search` initializes and exposes `set_scorer`.
- `src/search/dfpn/history.rs` — `move_order_breakdown` uses `self.scorer`.
- `examples/benchmark.rs` — added `--config` and `SCORER_CONFIG` support.
- `examples/static_move_scores.rs` — added `--config` and `SCORER_CONFIG` support.
- `examples/move_order_debug.rs` — added `--config` and `SCORER_CONFIG` support.
- `docs/plans/move_order/report5.md` — this report.

## Design

### `ScorerParams` and `PieceValues`

`src/search/ordering/params.rs` defines two plain structs with `serde::Deserialize`:

```rust
pub struct ScorerParams {
    pub score_winning_capture: i32,
    pub score_promotion: i32,
    pub score_capture: i32,
    pub capture_net_scale: i32,
    pub score_threat_last: i32,
    pub score_threat: i32,
    pub score_kamikaze_last: i32,
    pub score_kamikaze: i32,
    pub score_approach: i32,
    pub score_approach_step: i32,
    pub score_center: i32,
    pub score_center_step: i32,
    pub score_pawn_storm: i32,
    pub score_pawn_storm_step: i32,
    pub score_rook_center: i32,
    pub score_rook_open_file: i32,
    pub score_rook_open_file_step: i32,
    pub score_rook_back_rank: i32,
    pub and_pawn_storm_scale: i32,
    pub and_rook_attack_scale: i32,
    pub and_approach_scale: i32,
    pub pieces: PieceValues,
}
```

`#[serde(default)]` on both structs lets a TOML file omit any field and still
load; missing values use the compiled-in defaults.

### Validation

`ScorerParams::validate()` enforces:

1. All scores and scale factors are non-negative.
2. `and_*_scale` values are in `[0, 100]`.
3. Piece values are positive and `commoner` exceeds the sum of all other piece
   values.
4. `score_winning_capture` exceeds the highest possible promotion score.
5. `score_promotion` exceeds the highest possible non-winning capture score.
6. No intermediate computation overflows `i32`.

### Deviation from plan5 validation

The plan proposed an additional check: the smallest threat/kamikaze bonus must
exceed the largest possible *quiet* bonus. With the default weights this is not
true — a rook lift can accumulate quiet bonuses that exceed `score_threat` — so
that check was dropped. The current validation still protects the three
early-return categories (winning capture, promotion, capture) and prevents
catastrophic misconfiguration.

### Config loading

`src/config.rs` provides `load_scorer_config<P>(path) -> Result<ScorerParams, ConfigError>`.
It reads the file, deserializes a thin `ConfigFile { scorer: ScorerParams }`
wrapper, and runs `ScorerParams::validate()`.

The config loader is exposed as `atomic_solver::config::load_scorer_config` so
example binaries can use it.

### CLI and examples

- `atomic_solver --config path/to/scorer.toml ...`
- `cargo run --release --example benchmark -- --config scorer.toml ...`
- `cargo run --release --example static_move_scores -- --config scorer.toml ...`
- `cargo run --release --example move_order_debug -- --config scorer.toml ...`

If `--config` is not supplied, each entry point checks `SCORER_CONFIG`. If that
is also unset, the compiled-in defaults are used.

### `StaticAtomicScorer` refactor

`StaticAtomicScorer` changed from a unit struct to:

```rust
#[derive(Clone)]
pub struct StaticAtomicScorer {
    params: ScorerParams,
}
```

`Search` stores one instance and exposes `set_scorer` and `scorer` accessors.
`score_with_map` now reads every bonus from `self.params`.

## Verification

All automated checks passed:

```bash
cargo fmt
cargo clippy --all-targets
cargo test
cargo doc --no-deps
```

Release-mode regression tests:

```bash
cargo test --release --test test_move_order
cargo test --release --test stress move_order_hard_positions_unproven_in_60s
```

Manual benchmarks:

```bash
cargo run --release --example static_move_scores -- --name m22_white
cargo run --release --example static_move_scores -- --name m22_white --config config.toml
```

Both produced identical output, confirming that the default `config.toml`
carries the same values as the compiled-in defaults.

A config override was also tested:

```bash
cp config.toml /tmp/scorer_low_pawn_storm.toml
# set score_pawn_storm = 0 and score_pawn_storm_step = 0
cargo run --release --example static_move_scores \
    -- --name m22_white --config /tmp/scorer_low_pawn_storm.toml
```

With the pawn-storm bonus disabled, `g4g5` dropped from first place to 23rd,
proving that the file is actually driving the scorer.

Refined benchmark on `m22_white`:

```bash
cargo run --release --example benchmark -- --suite move-order --timeout 10 --runs 1 m22_white
cargo run --release --example benchmark -- --suite move-order --timeout 10 --runs 1 m22_white --config config.toml
```

Both runs returned a 23-ply win, matching the `report4.md` baseline.

First-outcome benchmark on the full move-order suite:

```bash
cargo run --release --example benchmark \
    -- --suite move-order --first-outcome --timeout 5 --runs 3
```

All decisive positions (`m23`–`m29`) matched their expected outcomes; `m20`–
`m22` remained timeouts. Node counts were consistent with the `report4.md`
first-outcome numbers.

## Measured impact

- Default behavior is unchanged.
- `m22_white` refined PV: 23 plies.
- No move-order benchmark position was misclassified.
- Hard stress positions (`m20`–`m21`) still timeout within 60 seconds, as
  expected.

## Problems encountered

1. **Clippy `field_reassign_with_default` warnings** in `params.rs` unit tests.
   Fixed by constructing test values with `ScorerParams { field: value, ..Default::default() }`
   instead of mutating a default instance.

2. **Validation too conservative.** The original plan's quiet-vs-forcing check
   rejected the default parameter set. It was replaced with a hierarchy check
   on the early-return categories only.

3. **`PieceType::None` did not exist.** A unit test tried to assert
   `params.piece_value(PieceType::None) == 0`; the variant does not exist in
   `atomic-movegen`, so the test was removed. The `_ => 0` branch in
   `piece_value` still handles unknown piece types.

## Unresolved parts / next steps

1. **Hill-climbing harness.** The config file makes parameter search possible,
   but no automated tuning script is included. A separate tool or example can
   generate candidate TOML files and drive `examples/benchmark.rs`.

2. **History/killer parameters.** `HISTORY_MAX`, `HISTORY_BONUS`, `SCORE_KILLER`,
   and friends are still hard-coded in `src/search/dfpn/history.rs`. These are a
   natural follow-up once the static scorer config path is stable.

3. **Looser validation opt-out.** The current validator rejects some configs
   that might be safe. If tuning needs broader exploration, add an explicit
   `--no-validate-config` flag or a `[validation]` table in the TOML.

4. **Default `config.toml` integration.** The CLI does not automatically load
   `./config.toml`; it only loads one when explicitly requested. This keeps
   behavior stable but means the shipped default file is documentation-only
   unless the user passes `--config config.toml` or sets `SCORER_CONFIG`.
