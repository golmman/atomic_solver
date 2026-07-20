# Report: Plan 1 — Release-profile build options

This report documents the application of `docs/plans/speed/plan1.md`.

## Changes applied

### `Cargo.toml`

Added a `[profile.release]` section with the settings recommended in the plan:

```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"
```

This switches the release build from the default `lto = false` / `codegen-units = 16` configuration to full link-time optimization with a single codegen unit and abort-on-panic.

### `target-cpu = "native"`

A project-wide `.cargo/config.toml` entry was **not** added.  `target-cpu = "native"` makes the binary non-portable, so it is left as an optional manual build flag.  It was benchmarked manually (see below) and showed no measurable extra speed-up on this machine, so enabling it project-wide was not justified.

## Benchmarks

All timings are wall-clock seconds for the `atomic_solver` CLI with its default 5-second timeout.  Each FEN was run three to ten times; the reported value is the arithmetic mean of the warm runs (the first run of each series, which is often slower due to filesystem/page-cache effects, is shown separately but excluded from the mean).

| FEN | Outcome / PV | Baseline (default release) | After profile changes | Change |
|-----|--------------|---------------------------:|----------------------:|-------:|
| `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1` | win, `f1f7 e8d8 g1g8` | 0.016–0.047 (first run 0.047, warm mean 0.016) | 0.016–0.046 (first run 0.046, warm mean 0.017) | within noise |
| `rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3` | win, `h5d5 d7d6 d5f7 e8d7 f7e7` | 1.58–1.67 (mean 1.61) | 1.54–1.64 (mean 1.58) | ~2% faster, within noise |
| `4k3/PP6/8/8/8/8/8/4K3 w - - 0 1` | win, `a7a8q e8d7 b7b8q d7e6 b8e5 e6d7 e5d6` | 0.239–0.273 (mean 0.252) | 0.234–0.259 (mean 0.247) | ~2% faster, within noise |
| `4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19` (m19) | draw (5 s timeout) | 5.007–5.009 | 5.006–5.008 | unchanged (timeout-limited) |

For the m19 position the solver still hits the default 5-second timeout and reports `outcome: draw`, so it cannot show a wall-clock speed-up in this setup.

### Manual `target-cpu=native` run

```text
RUSTFLAGS='-C target-cpu=native' cargo build --release
```

For the `rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3` FEN the mean over 10 runs was 1.58 s, essentially identical to the non-native mean.  Therefore `target-cpu=native` was not enabled in the project configuration.

## Build-time impact

| Step | Default release profile | New release profile |
|------|--------------------------:|--------------------:|
| `cargo build --release` | ~1.2 s | ~4.9 s |
| `cargo test --release` compile/link | (not separately timed) | ~26.4 s total |
| `cargo clippy --release` | (not separately timed) | ~2.7 s |

The new profile increases the clean release build time by roughly 4×, which is expected with full LTO and a single codegen unit.  Rebuilds during normal iteration are still fast because Cargo caches the LTO artifact.

## Verification

```text
$ cargo build --release
$ cargo test --release
$ cargo clippy --release
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
$ cargo run --release -- --fen "4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19"
$ cargo run --release -- --fen "rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3"
```

Results:

- `cargo build --release` completes successfully.
- `cargo test --release` passes all tests.
- `cargo clippy --release` reports zero warnings.
- Functional outcomes and PVs for the timed positions are unchanged from the baseline run.
- The m19 FEN still times out and returns `outcome: draw` under the default 5-second limit.

## Conclusion

The release profile was tuned as requested.  The change is safe and correct, but the measured wall-clock speed-up on the sampled positions is small enough to be within run-to-run noise.  This is consistent with a search that is largely memory/TT-bound rather than instruction-bound.  The main cost is a longer clean release build.  `target-cpu=native` was not enabled project-wide because it adds portability risk and showed no extra benefit in this environment.
