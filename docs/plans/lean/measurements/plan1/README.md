# plan1 raw measurements (2026-09-07)

All runs on the release build, reference host, default scorer params,
`--tt-size 64`, `--epsilon 0.125`.

| file | contents |
| --- | --- |
| `lean_before_fo.json` | `benchmark --suite quick --json --first-outcome`, **before** the change (baseline) |
| `lean_after_fo.json` | same command, **after** the change |
| `m22_before_stdout.txt` / `m22_before_stderr.txt` | `atomic_solver --fen <m22_white> --timeout 20 --outcome-only` before the change (default refinement, uncapped) |
| `m22_after_uncapped_stdout.txt` / `m22_after_uncapped_stderr.txt` | same command after the change plus `--refine-cap 0` (drift check) |
| `m22_after_default_stdout.txt` / `m22_after_default_stderr.txt` | same command after the change with the default cap (`--refine-cap 0.25`) plus `--dump-path /tmp/opencode/lean_after.bin` |

`m22_white` FEN: `4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22`

## Drift check results

- `--suite quick --json --first-outcome`: 59/59 cases with **identical
  `child_evals`** before vs after (38,714,551 total in both), identical
  `pv_len` per case.
- m22_white `--refine-cap 0` vs baseline: stdout byte-identical; stderr chunk
  logs identical in every deterministic field (`work_done`, `next_chunk`,
  `max_depth`, `nodes`); only `elapsed`/`nps` (wall-clock) differ.
- m22_white default cap vs baseline: same outcome (`win`), same PV length
  (23 plies), and the PV move list is byte-identical.
