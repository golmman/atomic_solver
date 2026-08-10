# Plan 5: Configurable move-ordering parameters via TOML

## Goal

Move the hand-tuned constants inside `StaticAtomicScorer` into a TOML config
file and load it through a new `--config <PATH>` CLI option. This makes the
move-ordering weights externally tunable and is the prerequisite for any
automated hill-climbing or coordinate-descent tuning.

## Background

`src/search/ordering.rs` currently hard-codes all static scoring constants as
module-level `const` items, and `StaticAtomicScorer` is a unit struct. The
DF-PN+ solver is order-sensitive: the most-proving child is selected from the
sorted move list, so changing these weights changes which subtree is explored
first. `docs/plans/move_order/report4.md` already identifies the
`AND_*_SCALE` values as the next tuning levers, but editing Rust source and
recompiling makes iteration slow and error-prone.

Outsourcing the weights to a config file gives three benefits:

1. Faster tuning iterations (no recompile for weight changes).
2. Version-controlled candidate sets that can be A/B tested.
3. A single place to document why each weight exists.

## Scope

This plan covers the constants in `StaticAtomicScorer` and the piece values
used by atomic SEE. It does **not** cover the dynamic history/killer constants
in `src/search/dfpn/history.rs` (`HISTORY_MAX`, `HISTORY_BONUS`, `SCORE_KILLER`,
etc.); those can be moved into the same config file in a follow-up plan once
the static scorer path is proven.

## Design

### Config file location and format

- Default file: `config.toml` in the current working directory.
- Override: `--config <PATH>` on the CLI, or `SCORER_CONFIG` environment
  variable as a secondary fallback.
- If no config file is supplied, the compiled-in defaults are used so current
  behavior is unchanged.
- Format: TOML with a `[scorer]` table and an optional nested
  `[scorer.pieces]` table.

Example `config.toml`:

```toml
[scorer]
score_winning_capture = 100_000_000
score_promotion = 1_000_000
score_capture = 5_000
capture_net_scale = 10
score_threat_last = 10_000
score_threat = 1_000
score_kamikaze_last = 9_000
score_kamikaze = 3_000
score_approach = 100
score_approach_step = 10
score_center = 50
score_center_step = 10
score_pawn_storm = 5_500
score_pawn_storm_step = 100
score_rook_center = 500
score_rook_open_file = 2_000
score_rook_open_file_step = 50
score_rook_back_rank = 300
and_pawn_storm_scale = 50
and_rook_attack_scale = 50
and_approach_scale = 75

[scorer.pieces]
pawn = 100
knight = 320
bishop = 330
rook = 500
queen = 900
commoner = 20_000
```

TOML 1.0 permits underscores in integer literals; the `toml` crate accepts
them.

### New module: `src/search/ordering/params.rs`

Introduce `ScorerParams` and `PieceValues`:

```rust
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
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

#[derive(Clone, Debug, Deserialize)]
pub struct PieceValues {
    pub pawn: i32,
    pub knight: i32,
    pub bishop: i32,
    pub rook: i32,
    pub queen: i32,
    pub commoner: i32,
}
```

Implement in this module:

- `impl Default for ScorerParams` — returns the current hand-tuned values.
- `impl ScorerParams { pub fn validate(&self) -> Result<(), ScorerParamsError> }`
  — enforces hierarchical score invariants (see Validation section).
- `pub fn piece_value(&self, pt: PieceType) -> i32` — lookup used by atomic SEE.
- Unit tests for `Default`, `validate` success, and `validate` failure cases.

### Refactor `StaticAtomicScorer`

Change `StaticAtomicScorer` from a unit struct to a struct carrying params:

```rust
pub struct StaticAtomicScorer {
    params: ScorerParams,
}
```

Provide constructors and accessors:

```rust
impl StaticAtomicScorer {
    pub fn new() -> Self {
        Self { params: ScorerParams::default() }
    }

    pub fn from_params(params: ScorerParams) -> Self {
        Self { params }
    }

    pub fn params(&self) -> &ScorerParams {
        &self.params
    }
}

impl Default for StaticAtomicScorer {
    fn default() -> Self { Self::new() }
}
```

Update `score_with_map` and the capture-net helper to read from
`self.params` instead of module-level `const` items. Remove the module-level
`const SCORE_*` declarations from `ordering.rs` or keep them only as private
fallbacks used by `ScorerParams::default()`; do not let bare constants shadow
the configurable values.

`src/search/ordering.rs` should add:

```rust
mod params;
pub use params::{PieceValues, ScorerParams};
```

### New module: `src/config.rs`

Create a small crate-level config loader:

```rust
pub fn load_scorer_config<P: AsRef<Path>>(path: P) -> Result<ScorerParams, ConfigError>;

pub enum ConfigError {
    Io(std::io::Error),
    Parse(toml::de::Error),
    Invalid(ScorerParamsError),
}
```

Implementation steps:

1. Read the file to a string.
2. Deserialize into a thin wrapper:
   ```rust
   #[derive(Deserialize)]
   struct ConfigFile {
       scorer: ScorerParams,
   }
   ```
3. Call `ScorerParams::validate()`.
4. Return the validated `ScorerParams`.

Add `pub mod config;` to `src/lib.rs` so the loader is available to the binary
and to examples.

### Search integration

`Search` already stores `scorer: StaticAtomicScorer`. Initialize it with
`StaticAtomicScorer::default()` and add:

```rust
impl Search {
    pub fn set_scorer(&mut self, scorer: StaticAtomicScorer) { self.scorer = scorer; }
    pub fn scorer(&self) -> &StaticAtomicScorer { &self.scorer }
}
```

`set_scorer` must be called before `solve`; changing the scorer during a search
is unsupported and has no defined behavior.

### CLI integration

Add `config_path: Option<String>` to `CliOptions` in `src/cli.rs` and parse
`--config <PATH>`. Add unit tests covering:

- `--config /valid/path.toml` sets `config_path`.
- Missing value after `--config` returns an error.
- Unknown options still return an error.

In `src/main.rs`, after parsing options:

```rust
let scorer = if let Some(path) = config_path {
    StaticAtomicScorer::from_params(config::load_scorer_config(&path)?)
} else {
    StaticAtomicScorer::default()
};
let mut search = Search::new(tt_size);
search.set_scorer(scorer);
```

### Example integration

The examples used to inspect and benchmark move ordering should support
`--config` so a tuner can run the benchmark suite with different weights without
recompiling:

- `examples/benchmark.rs` — add `--config`, load, and call `set_scorer`.
- `examples/static_move_scores.rs` — add `--config`, load, and construct
  `StaticAtomicScorer::from_params`.
- `examples/move_order_debug.rs` — add `--config`, load, and call `set_scorer`.

### Default config file

Add a `config.toml` at the repository root containing the current default
values. This file is the documentation of the starting point and the baseline
for hill-climbing experiments.

## Validation invariants

`ScorerParams::validate()` should enforce conservative checks that prevent the
scorer from collapsing its category hierarchy. These checks may reject configs
that are actually safe, but they guarantee the current unit-test invariants
continue to hold.

1. All scores and scale factors are non-negative.
2. `and_pawn_storm_scale`, `and_rook_attack_scale`, and `and_approach_scale`
   are in `[0, 100]` (they are percent-style multipliers).
3. Piece values are positive and `commoner_value` is strictly greater than the
   sum of `queen + rook + bishop + knight + pawn`.
4. Capture scores:
   - Let `max_non_commoner_value = queen + rook + bishop + knight`.
   - Let `max_capture_net = max_non_commoner_value - pawn`.
   - Let `max_capture_score = score_capture + capture_net_scale * max_capture_net`.
   - Require `score_winning_capture > score_promotion + queen`.
   - Require `score_promotion + queen > max_capture_score + queen`.
5. Threat/kamikaze dominance over quiet moves:
   - Let `min_forcing = min(score_threat_last, score_kamikaze_last, score_threat, score_kamikaze)`.
   - Let `max_quiet_score` be the largest possible sum of quiet bonuses from a
     single move: `score_pawn_storm + 7 * score_pawn_storm_step` plus
     `score_rook_open_file + 7 * score_rook_open_file_step` plus
     `score_approach + 7 * score_approach_step` plus
     `score_center + 3 * score_center_step` plus
     `score_rook_center + 3 * score_rook_center` plus
     `score_rook_back_rank`.
   - Require `min_forcing > max_quiet_score`.

If a future tuning run needs looser bounds, add an explicit opt-out (for
example, `--no-validate-config`) in a separate change rather than weakening the
default checks.

## Implementation steps

1. Add dependencies to `Cargo.toml`:
   ```toml
   [dependencies]
   serde = { version = "1.0.229", features = ["derive"] }
   toml = "1.1.4"
   ```
   Use exact version specifiers (`=1.0.229` and `=1.1.4`) if the project
   prefers stronger supply-chain pinning; otherwise let `Cargo.lock` manage the
   exact revision. Both versions were published more than seven days before the
   implementation date.

2. Create `src/search/ordering/params.rs` with `ScorerParams`, `PieceValues`,
   `Default`, `validate`, and `piece_value`.

3. Update `src/search/ordering.rs`:
   - Add `mod params;` and re-export `ScorerParams`/`PieceValues`.
   - Convert `StaticAtomicScorer` to a struct holding `ScorerParams`.
   - Replace bare `SCORE_*` constants with `self.params.*` in
     `score_with_map`.
   - Update `capture_net_value` to use the configurable piece values.
   - Update `impl MoveScorer` to use the new constructor.
   - Update `src/search/ordering/tests.rs` to construct
     `StaticAtomicScorer::default()` instead of the unit struct.

4. Create `src/config.rs` with `ConfigError` and `load_scorer_config`.

5. Add `pub mod config;` to `src/lib.rs`.

6. Update `src/search/dfpn/mod.rs`:
   - `Search::new` initializes `scorer: StaticAtomicScorer::default()`.
   - Add `set_scorer` and `scorer` accessors.

7. Update `src/search/dfpn/history.rs`:
   - `move_order_breakdown` already has `&self`; change the bare
     `StaticAtomicScorer.score_with_map(...)` call to `self.scorer.score_with_map(...)`.

8. Update `src/cli.rs`:
   - Add `config_path` to `CliOptions`.
   - Parse `--config <PATH>` and update tests.

9. Update `src/main.rs` to load the config and call `search.set_scorer`.

10. Update `examples/benchmark.rs`, `examples/static_move_scores.rs`, and
    `examples/move_order_debug.rs` to support `--config` and `set_scorer`.

11. Add `config.toml` at the repository root with the default values.

12. Run the verification commands and write `docs/plans/move_order/report5.md`.

## Files changed

- `Cargo.toml`
- `config.toml` (new)
- `src/lib.rs`
- `src/config.rs` (new)
- `src/cli.rs`
- `src/cli.rs` unit tests
- `src/main.rs`
- `src/search/ordering.rs`
- `src/search/ordering/params.rs` (new)
- `src/search/ordering/tests.rs`
- `src/search/dfpn/mod.rs`
- `src/search/dfpn/history.rs`
- `examples/benchmark.rs`
- `examples/static_move_scores.rs`
- `examples/move_order_debug.rs`
- `docs/plans/move_order/plan5.md` (this file)
- `docs/plans/move_order/report5.md` (final report)

## Verification

Run after every meaningful change:

```bash
cargo fmt
cargo clippy --all-targets
cargo test
cargo doc --no-deps
```

Manual checks:

```bash
# Default behavior unchanged.
cargo run --release --example static_move_scores -- --name m22_white
cargo run --release --example move_order_debug -- --name m22_white

# With a custom config.
cp config.toml /tmp/test_config.toml
# Edit /tmp/test_config.toml, e.g. set and_pawn_storm_scale = 25.
cargo run --release --example static_move_scores -- --name m22_white --config /tmp/test_config.toml
cargo run --release --example benchmark -- --suite move-order --first-outcome --timeout 5 --runs 3 --config /tmp/test_config.toml

# CLI parsing.
cargo run --release -- --config /tmp/test_config.toml --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
```

Expected outcomes:

- `cargo test` passes with no new warnings.
- Default suite and move-order suite match pre-change outcomes (no new wrong
  results or new timeouts).
- `m22_white` refined PV length does not regress from the `report4.md` baseline
  (23 plies in a standalone benchmark).
- `m23_white` refined node count and PV length are measured; a tuned
  `and_pawn_storm_scale` may improve them, but the change must not misclassify
  any position.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Config loading adds startup cost or binary bloat. | Parse only once at startup; pin exact dependency versions; measure binary size and startup time in `report5.md`. |
| A tuner collapses the score hierarchy and breaks search. | `ScorerParams::validate()` enforces conservative category ordering. |
| Validation is too strict and rejects good hill-climbing candidates. | Document the constraints; add an opt-out only if measurement justifies it. |
| `StaticAtomicScorer` refactor breaks tests/examples. | Compiler catches signature changes; update every call site; run `cargo test --all-targets`. |
| Examples parse `--config` with slightly different semantics. | Keep the flag name and value behavior identical to `src/cli.rs`. |
| Piece-value tuning breaks atomic SEE. | Validate `commoner_value` dominates; keep the aSEE formula unchanged. |

## Success criteria

1. `cargo test`, `cargo clippy --all-targets`, and `cargo doc --no-deps` pass
   with no new warnings.
2. No regression on the default benchmark suite or move-order suite: every
   position that was decisive before remains decisive with the same outcome.
3. `--config <PATH>` loads a TOML file and overrides the compiled-in defaults.
4. Without `--config`, the solver behaves exactly as before.
5. `examples/benchmark.rs`, `examples/static_move_scores.rs`, and
   `examples/move_order_debug.rs` support `--config`.
6. `ScorerParams::validate()` rejects obviously broken configs (for example,
   `score_capture > score_promotion`).
7. A default `config.toml` is present at the repository root.
8. `docs/plans/move_order/report5.md` is written documenting the final design,
   the measured impact with the default config, and any deviations from this
   plan.

## Final task

Write `docs/plans/move_order/report5.md` documenting:

- Which files changed and why.
- The exact `ScorerParams` fields and default values.
- The validation rules chosen and any deviations.
- Measured benchmark impact with the default config (should be neutral) and
  with one or two hand-tuned variants.
- Problems encountered, unresolved parts, and next steps (for example, a
  hill-climbing harness and possibly history/killer parameters).
