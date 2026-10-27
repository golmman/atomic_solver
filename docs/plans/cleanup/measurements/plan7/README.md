# plan7 measurement artifacts

Provenance for the housekeeping pass in `docs/plans/cleanup/plan7.md`.
HEAD at capture: `e2127358476bf310e01d33cb5b79d7b448c905c4`.

| file | what it is | how to reproduce |
| --- | --- | --- |
| `env.json` | commit, host, toolchain, CPU count, profile, TT sizes, timeouts, epsilon | `git rev-parse HEAD`; `rustc --version`; `nproc` |
| `verify_move.py` | mechanical move-only checker: `--snapshot HEAD` writes `state/functions_before.json`; `--check` compares normalized fn bodies across `dfpn/{mod,config,run}.rs` | `python3 verify_move.py --snapshot HEAD && python3 verify_move.py --check` |
| `state/functions_before.json` | fn name → normalized body hash + line count, parsed from `git show HEAD:src/search/dfpn/mod.rs` | `verify_move.py --snapshot HEAD` |
| `state/public_surface_before.txt` | 36-pub-fn name set on `Search` before the split | `grep -nE '^\s+pub (const )?fn' src/search/dfpn/mod.rs` at HEAD |
| `state/public_surface_after.txt` | same capture across the three split files after Phase C | `grep -nE '^\s+pub (const )?fn' src/search/dfpn/{mod,config,run}.rs` |
| `state/status_after_phase_b.txt` | `git status --porcelain` at the D3-1(c) baseline | `git status --porcelain` after Phase B |
| `state/anchors.json` | quick-suite totals (both TT sizes) and the five single-case anchors, before vs after | commands in plan7 §Phase 0 / §Gates |
| `state/wall_before.json` | Phase-0 wall medians of 3 for stress FO and m22 default mode | `time ./target/release/atomic_solver ...` interleaved |
| `state/wall_after.json` | post-split wall medians of 3, same protocol | same |
| `state/wall_interleaved_head_vs_after.json` | controlled 5-run interleaved comparison of the HEAD binary (`/tmp/atomic_head`, built from `git archive HEAD`) vs the split working tree | build HEAD via `git archive HEAD \| tar -x -C /tmp/atomic_head`, then alternate binaries |

Raw run transcripts (`/tmp/quick*.out`, `/tmp/stress.*`, etc.) are not
committed; the parsed results above are the reproducible record.

Anchor commands (stderr carries the final `evals: <n>` line):

```bash
./target/release/atomic_solver --fen '<FEN>' --timeout <t> --first-outcome --outcome-only
cargo run --release --example benchmark -- --suite quick --first-outcome --timeout 3 --runs 1 --tt-size 256
cargo run --release --example benchmark -- --suite quick --first-outcome --timeout 3 --runs 1 --tt-size 128
```

FENs: dec31 `r1bq1k2/ppN3p1/2p1p2P/6P1/1b1P1p2/8/PP3P2/R2K1B1R b - - 0 15`;
m22 / m22 default `4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22`;
m23_white `4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23`;
stress FO `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`.
