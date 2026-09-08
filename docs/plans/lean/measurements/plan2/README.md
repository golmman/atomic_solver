# plan2 raw measurements (2026-09-07)

All runs on the release build, this container, default scorer params,
`--tt-size 64`, `--epsilon 0.125`. `perf`/valgrind are unavailable here;
the profiles are gdb backtrace samples (see report2, deviation 1).

| file | contents |
| --- | --- |
| `quick_before.json` | `benchmark --suite quick --json --first-outcome`, **before** the change (baseline; 38,714,551 `child_evals`) |
| `quick_after.json` | same command, **after** the change (59/59 identical `child_evals`) |
| `m22_before.log` | `atomic_solver --fen <m22_white> --timeout 20 --first-outcome --outcome-only --dump-path /tmp/opencode/lean2_before.bin` stderr, baseline |
| `m22_after_first_outcome.log` / `m22_after_first_outcome_stdout.txt` | same command after the change (stdout byte-identical, chunk `work_done`/`nodes` identical) |
| `m22_after_default_stderr.log` | `--timeout 20 --outcome-only` after the change; chunk `work_done`/`nodes` bit-identical to the pre-change binary (verified by rebuilding the baseline via `git stash`) |
| `perf_before.txt` | aggregated gdb sampling profile, baseline (55 samples) |
| `perf_before_gdb_samples.txt` | raw gdb backtrace samples, baseline |
| `perf_stat_unavailable.txt` | empty `perf stat` output (no counters in container) |
| `perf_after_boxed_stepB_samples.txt` | raw samples of the intermediate boxed-precompute build (shows malloc 11.5% / memset 6.6% — the measured reason for deviation 2) |
| `perf_after_gdb_samples.txt` | raw gdb backtrace samples, final build (malloc/free/memset/memcpy at 0%) |

`m22_white` FEN: `4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22`

Sampling profiler script (for reproducibility):

```sh
# attach gdb to the running solver and dump backtraces ~6-7x/s
gdb -p "$pid" -batch -ex "bt" | sed -n '/^#/p' >> samples.txt
```

## Headline numbers

- quick suite: 59/59 cases identical `child_evals` and `pv_len`;
  38,714,551 total before and after.
- m22 first-outcome wall: 12.375 s (baseline) → 11.87 s (final),
  chunk-6 phase 10.738 s → 10.301 s (~4%).
- m22 default mode: stdout byte-identical to plan1's stored artifact
  (win, 23-ply PV); deterministic chunk trajectory bit-identical.
- `make test-full`: 283 passed, 0 failed.
