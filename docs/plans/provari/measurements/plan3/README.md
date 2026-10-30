# Provari plan3 measurements — provenance

Quiet-move grouping measurement captured **2026-10-30** for provari plan3
([`../../plan3.md`](../../plan3.md)). This directory contains the driver, the
parsed results, and this provenance record. Raw transcripts are not
committed.

## Provenance

| Item | Value |
| --- | --- |
| Date | 2026-10-30 (UTC) |
| Host | container; AMD Ryzen 9 5950X, 32 vCPU; Linux |
| Runs | sequential, one process at a time, release build, no tablebases |
| `atomic_solver` | `fe71029` plus the plan3 working tree (quiet-grouping flag) |
| Solver binary | `./target/release/atomic_solver` |
| Validators | `./target/release/examples/verify_ppv`, `./target/release/examples/reconstruct_pt` |
| Corpus | [`../plan1/corpus.json`](../plan1/corpus.json), reused by path; `sample20` unchanged |
| TT label | 192 MB (reservation bound; pages populated on use) |
| Configs | `off` shipped; `group` + `--quiet-grouping`; `mc` + `--move-class-pn`; `both` + both flags |

Build: `cargo build --release`, then
`cargo build --release --example verify_ppv --example reconstruct_pt`.

## Protocol

| Case | Timeout | Child-eval budget | Repeats/config |
| --- | ---: | ---: | ---: |
| `core` | 90 s | 600,000,000 | 3 |
| shared `1eb80b92f92c` | 10 s | 300,000,000 | 3 |
| `sample20` (all 20) | 10 s | none | 1 |

Solver command (baseline; config flags appended):

```sh
./target/release/atomic_solver --fen "<FEN>" --tt-size 192 \
  --first-outcome --outcome-only --timeout <T> [--budget <B>] [flags]
```

Driver invocation that produced [`results.json`](results.json):

```sh
python3 docs/plans/provari/measurements/plan3/run_group.py \
  --solver ./target/release/atomic_solver \
  --verify ./target/release/examples/verify_ppv \
  --reconstruct ./target/release/examples/reconstruct_pt \
  --solver-commit 'fe71029 (+ plan3 working tree)' \
  --host 'container (AMD Ryzen 9 5950X, 32 vCPU)'
```

`--dry-run` prints the measurement commands; `--skip-validation` measures
only.

## Validation of decisive rows

Every decisive measured row is validated twice, deduplicated by
`(case, config, PV)`:

1. **PV check** — `verify_ppv --fen <FEN> --moves "<PV>" --timeout 120`.
   This is a property of the *informational* first-outcome PV (every defender
   reply refutable within the remaining line), not of the claimed outcome.
2. **Proof-tree check** — re-run the same configuration with
   `--tt-dump-path`, rebuild with
   `reconstruct_pt --snapshot <tt> --out <pt> --pt-size 512
   --fill-attempt-budget 100000000 --fill-total-budget 0 --json`, and read the
   replay-based `validate` verdict. This checks the published outcome.

Outcomes:

- core, all four configs: PV check **inconclusive** (the informational PV is
  not a longest-defense PPV and verify_ppv's depth-limited sub-search hit its
  wall limit); proof-tree check **valid** (`root_outcome: win`,
  `validate: ok`) on all 3 repeats of every config.
- shared `1eb80b92f92c`: PV check **refuted-pv** for `off` (not a longest
  defense — expected for a best-effort PV), **valid** for `group`, `mc`, and
  `both`; proof-tree check **valid** for all four configs, all repeats.
- `sample20`: only `1eb80b92f92c` is decisive, already covered above.

## Results summary

Medians over the three repeats; `child_evals` is the deterministic work
metric (identical across repeats), wall is the product metric.

| Case | Config | Wall median | Child evals | First win (plies) | Outcome |
| --- | --- | ---: | ---: | ---: | --- |
| core | off | 60.132 s | 437,166,180 | 29 | win |
| core | group | 30.479 s | 168,365,878 | 29 | win |
| core | mc | 8.032 s | 50,044,940 | 27 | win |
| core | both | 5.473 s | 27,861,992 | 25 | win |
| shared `1eb80b92f92c` | off | 6.013 s | 43,266,033 | 37 | win |
| shared `1eb80b92f92c` | group | 3.258 s | 19,082,824 | 43 | win |
| shared `1eb80b92f92c` | mc | 1.898 s | 12,680,751 | 39 | win |
| shared `1eb80b92f92c` | both | 1.059 s | 5,568,743 | 45 | win |

Ratios versus `off`: grouping alone is **2.60×** fewer evals / **1.97×** less
wall on core and **2.27×** / **1.85×** on the shared case. The combination
`both` is **15.69×** / **10.99×** on core and **7.77×** / **5.68×** on shared.
Grouping is also additive with move-class numbers: `both` beats `mc` by
**1.80×** evals / **1.47×** wall on core and **2.28×** / **1.79×** on shared.

`sample20` coverage is unchanged: **1/20 wins** (the shared case) and 19/20
censored at 10 s for every config. On the censored rows grouping completes
~30% more child evals in the same 10 s window than `off`; `both` ~37% more.
Because both sides are wall-censored there, that is a throughput observation,
not a deterministic work ratio.

## Files

- [`run_group.py`](run_group.py) — driver (`--dry-run`, `--skip-validation`).
- [`results.json`](results.json) — parsed rows (all repeats, censored rows
  kept), per-config medians, summary, and the validation cache.
- `README.md` — this provenance record.
- [`../../report3.md`](../../report3.md) — plan3 report.

## Cleanup note

TT snapshots and reconstructed proof trees used for validation were written
to `/tmp`/`tempfile.TemporaryDirectory` and are not committed. The driver
sets `sys.dont_write_bytecode = True`, so no `__pycache__` is created. No
raw `*.log`/`*.out`/`*.err` transcript exists under this directory.
