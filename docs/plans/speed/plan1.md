# Plan 1: Release-profile build options

## Start

Read `docs/plans/speed/analysis.md` for context.  Inspect `Cargo.toml` to
confirm there is no custom `[profile.release]` section yet.  Pick a small set of
decisive FENs to use as a before/after sanity check (e.g. the m19 FEN and a
short forced mate from `tests/`).

## Goal

Make the release binary faster with only build-configuration changes.

## Background

`Cargo.toml` uses the default release profile, which is `opt-level = 3` but
leaves LTO off and uses many codegen units.  Enabling full LTO, reducing codegen
units to one, and aborting on panic can give a noticeable speed-up for a
compute-bound solver with a small hot loop.  `target-cpu = "native"` can be added
as a project-wide `.cargo/config.toml` or as an environment flag, but it is
optional because it makes the binary non-portable.

<ref_file file="/workspace/atomic_solver/Cargo.toml" />

## Implementation tasks

1. Add a `[profile.release]` section to `Cargo.toml`:
   ```toml
   [profile.release]
   opt-level = 3
   lto = true
   codegen-units = 1
   panic = "abort"
   ```
2. Optionally add `.cargo/config.toml` with:
   ```toml
   [build]
   rustflags = ["-C", "target-cpu=native"]
   ```
   Only do this if portability is not a concern.  Otherwise document the flag
   for manual benchmarking (`RUSTFLAGS='-C target-cpu=native' cargo build --release`).
3. Build and run a few sample positions with `cargo run --release -- --fen ...`.
4. Confirm that `cargo test --release` still passes.

## File changes

- `Cargo.toml`
- `.cargo/config.toml` (optional)

## Risks

- `lto = true` and `codegen-units = 1` slow down release builds.
- `panic = "abort"` removes unwinding/backtraces; the solver does not rely on
  `catch_unwind`, so this is safe.
- `target-cpu = "native"` produces a binary that may not run on older CPUs.

## Verification

```text
$ cargo build --release
$ cargo test --release
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
$ cargo run --release -- --fen "4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19"
```

Measure wall-clock time before and after on the sample FENs.  There should be
no functional change in `outcome` or `pv`.

## Final task

Write `docs/plans/speed/report1.md` documenting the measured speed difference,
any build-time regressions, and whether `target-cpu=native` was enabled.
