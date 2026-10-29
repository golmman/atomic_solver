# Provari plan2 measurements — provenance

Move-class starting proof-number spike captured **2026-10-29** for provari
plan2 ([`../../plan2.md`](../../plan2.md)). This directory contains the driver,
the parsed results, and this provenance record. Raw transcripts are not
committed.

## Provenance

| Item | Value |
| --- | --- |
| Date | 2026-10-29 (UTC) |
| Host | container `bc5b4ea98f93`; AMD Ryzen 9 5950X, 32 vCPU; Linux (container) |
| Runs | sequential, one process at a time, release build, no tablebases |
| `atomic_solver` | `c94636c` plus the plan2 working tree (move-class flag) |
| Solver binary | `./target/release/atomic_solver` |
| Validators | `./target/release/examples/verify_ppv`, `./target/release/examples/reconstruct_pt` |
| Corpus | [`../plan1/corpus.json`](../plan1/corpus.json), reused by path; `sample20` unchanged |
| TT label | 192 MB (reservation bound; pages populated on use) |
| Configs | `off` = shipped baseline; `on` = baseline + `--move-class-pn` |

Build: `cargo build --release`, then
`cargo build --release --example verify_ppv --example reconstruct_pt`.

## Protocol

| Case | Timeout | Child-eval budget | Repeats/config |
| --- | ---: | ---: | ---: |
| `core` | 90 s | 600,000,000 | 3 |
| shared `1eb80b92f92c` | 10 s | 300,000,000 | 3 |
| `sample20` (all 20) | 10 s | none | 1 |

Solver command (baseline; `on` appends `--move-class-pn`):

```sh
./target/release/atomic_solver --fen "<FEN>" --tt-size 192 \
  --first-outcome --outcome-only --timeout <T> [--budget <B>]
```

Driver invocation that produced [`results.json`](results.json):

```sh
python3 docs/plans/provari/measurements/plan2/run_spike.py \
  --solver ./target/release/atomic_solver \
  --verify ./target/release/examples/verify_ppv \
  --reconstruct ./target/release/examples/reconstruct_pt \
  --solver-commit 'c94636c (+ plan2 working tree)' \
  --host 'container bc5b4ea98f93 (AMD Ryzen 9 5950X, 32 vCPU)'
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

- core, both configs: PV check **inconclusive** (the informational PV is not
  a longest-defense PPV and verify_ppv's depth-limited sub-search hit its
  wall limit); proof-tree check **valid** (`root_outcome: win`,
  `validate: ok`) on all 3 repeats of both configs.
- shared `1eb80b92f92c`: `on` PV check **valid**; `off` PV check
  **refuted-pv** (not a longest defense at defender ply 30/37 — expected for
  a best-effort PV). Proof-tree check **valid** for both configs, all
  repeats.
- `sample20`: only `1eb80b92f92c` is decisive, already covered above.

## Results summary

Medians over the three repeats; `child_evals` is the deterministic work
metric (identical across repeats), wall is the product metric.

| Case | Config | Wall median | Child evals | First win (plies) | Outcome |
| --- | --- | ---: | ---: | ---: | --- |
| core | off | 62.367 s | 437,166,180 | 29 | win |
| core | on | 8.223 s | 50,044,940 | 27 | win |
| shared `1eb80b92f92c` | off | 5.969 s | 43,266,033 | 37 | win |
| shared `1eb80b92f92c` | on | 1.880 s | 12,680,751 | 39 | win |

Ratios `off / on`: core **8.74×** fewer child evals and **7.58×** less wall;
shared case **3.41×** fewer child evals and **3.18×** less wall.

`sample20` coverage is unchanged: **1/20 wins** (the shared case) and 19/20
censored at 10 s for both configs. On the censored rows the `on` config
completes ~11% fewer child evals in the same 10 s window (total
1,218,246,034 vs 1,372,443,187), i.e. classification costs roughly 10% per
eval there; this is not a deterministic work comparison because both
configs are wall-censored.

## Files

- [`run_spike.py`](run_spike.py) — driver (`--dry-run`, `--skip-validation`).
- [`results.json`](results.json) — parsed rows (all repeats, censored rows
  kept), medians, summary, and the validation cache.
- `README.md` — this provenance record.
- [`../../report2.md`](../../report2.md) — plan2 report.

## Cleanup note

TT snapshots and reconstructed proof trees used for validation were written
to `/tmp`/`tempfile.TemporaryDirectory` and are not committed. The driver
sets `sys.dont_write_bytecode = True`, so no `__pycache__` is created. No
raw `*.log`/`*.out`/`*.err` transcript exists under this directory.
