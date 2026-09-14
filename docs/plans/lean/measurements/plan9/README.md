# Plan 9 measurement artifacts

All runs release build, `--first-outcome --outcome-only --tt-size 128`,
aarch64 container host, 2026-09-15. Instrumentation: temporary
`LEAN9_SPIKE=1` counters on `Search` (reverted; `src/` is byte-identical to
the pre-spike tree, verified with `git diff`).

## Files

- `m22_prespike_stdout.txt` / `m24_prespike_stdout.txt` /
  `shuffle_prespike_stdout.txt` — pre-spike stdout (baseline for the
  byte-identity checks). md5s: m22 `ea72f7ea…`, m24 `e8d4d51d…`, shuffle
  `ffd3d015…` (m22/shuffle match the report8 values exactly).
- `m22_spike_summary.txt` / `m24_spike_summary.txt` /
  `shuffle_spike_summary.txt` — `LEAN9_SPIKE=1` runs: stderr chunk
  trajectory + the full M1–M5 summary (the stdout of the same run is in
  `*_spike_stdout.txt`, byte-identical to the pre-spike file).
- `m22_base_stderr.txt` / `m24_base_stderr.txt` / `shuffle_base_stderr.txt`
  — `LEAN9_SPIKE=base` runs (ungated binary, prints only
  `spike9_base: nodes=… child_evals=…`) for the instrumentation-neutrality
  triple.
- `m22_postrevert_stdout.txt` — post-revert rebuild run, byte-identical to
  the pre-spike file (md5 `ea72f7ea…`).

## Commands

```bash
LEAN9_SPIKE=1   ./target/release/atomic_solver --fen <FEN> --timeout <T> \
                  --first-outcome --outcome-only
LEAN9_SPIKE=base ./target/release/atomic_solver --fen <FEN> --timeout <T> \
                  --first-outcome --outcome-only
# FENs / timeouts: m22_white 30 s, shuffle-win 100 s, m24_white 20 s (see plan9.md)
```

## Instrumentation-neutrality triples (base vs instrumented, same build)

| case | nodes (base = spike) | child_evals (base = spike) | stdout vs pre-spike |
| --- | --- | --- | --- |
| m22_white | 858,117 | 14,156,269 | identical |
| m24_white | 22,739 | 406,737 | identical |
| shuffle-win | 13,907,467 | 249,480,478 | identical |

The m22/shuffle triples also match the conversion-plan4 recorded baselines
exactly (858,117 / 14,156,269 and 13,907,467 / 249,480,478). Internal
consistency: the spike's frame-level own-eval partition summed to the exact
`child_evals` total on every run (delta 0).

Post-revert: `make test` green (all suites, 0 failures).
