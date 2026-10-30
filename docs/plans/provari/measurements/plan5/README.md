# Provari plan5 measurements — provenance

EF-PN hand-guide measurement captured **2026-10-30** for provari plan5
([`../../plan5.md`](../../plan5.md)). This directory contains the driver, the
parsed results, and this provenance record. Raw transcripts are not committed.

## Provenance

| Item | Value |
| --- | --- |
| Date | 2026-10-30 (UTC) |
| Host | container; AMD Ryzen 9 5950X, 32 vCPU; Linux |
| Runs | sequential, one process at a time, release build, no tablebases |
| `atomic_solver` | `8ad20e2` plus the plan5 working tree (`--eval-pn`) |
| Solver binary | `./target/release/atomic_solver` |
| Validators | `./target/release/examples/verify_ppv`, `./target/release/examples/reconstruct_pt` |
| Corpus | [`../plan1/corpus.json`](../plan1/corpus.json), reused by path; `sample20` unchanged |
| TT label | 192 MB (reservation bound; pages populated on use) |
| Configs | `best` = `--move-class-pn --quiet-grouping`; `best+eval` = best + `--eval-pn both`; `best+eval-or` = best + `--eval-pn or`; `eval` = `--eval-pn both` alone (core only) |

Build: `cargo build --release`, then
`cargo build --release --example verify_ppv --example reconstruct_pt`.

All rows in [`results.json`](results.json) are from the final code: a
pawn-direction defect in the evaluation's `checkers` term was fixed (and
pinned by a unit test) before this run, and the arms were re-measured.

## Protocol

| Case | Timeout | Child-eval budget | Repeats/config | Configs |
| --- | ---: | ---: | ---: | --- |
| `core` | 90 s | 600,000,000 | 3 | 4 |
| shared `1eb80b92f92c` | 10 s | 300,000,000 | 3 | 3 |
| `sample20` (all 20) | 10 s | none | 1 | 3 |

Solver command (baseline; config flags appended):

```sh
./target/release/atomic_solver --fen "<FEN>" --tt-size 192 \
  --first-outcome --outcome-only --timeout <T> [--budget <B>] [flags]
```

Driver invocation that produced [`results.json`](results.json):

```sh
python3 docs/plans/provari/measurements/plan5/run_eval_pn.py \
  --solver ./target/release/atomic_solver \
  --verify ./target/release/examples/verify_ppv \
  --reconstruct ./target/release/examples/reconstruct_pt \
  --solver-commit '8ad20e2 (+ plan5 working tree)' \
  --host 'container (AMD Ryzen 9 5950X, 32 vCPU)'
```

`--dry-run` prints the measurement commands; `--skip-validation` measures only.

## Validation of decisive rows

Every decisive measured row is validated twice, deduplicated by
`(case, config, PV)`; [`results.json`](results.json) holds 7 unique entries
(`core` × 4 configurations, `1eb80b92f92c` × 3):

1. **PV check** — `verify_ppv --fen <FEN> --moves "<PV>" --timeout 120`.
   This is a property of the *informational* first-outcome PV, so a refuted or
   inconclusive verdict is not a defect (plan2–plan4 saw the same on `off`).
2. **Proof check** — a re-run of the same configuration with
   `--tt-dump-path`, rebuilt by `reconstruct_pt --pt-size 512
   --fill-attempt-budget 100000000 --fill-total-budget 0 --json`, requiring
   `root_outcome == measured outcome` **and** `validate: ok`.

All 7 unique decisive proofs validated (`validate: ok`, `root_outcome: win`).
Both EF-PN arms' claims therefore replay; the `or` arm's gains are on the same
proven outcome as `best`.

## Reproducing the summary

`results.json` holds every row (all repeats, censoring included), the
per-configuration medians (`core_medians`, `shared_medians`), the `sample20`
per-configuration census (`sample20_summary`), and the validation verdicts.
The interpretation, the per-case ratios, the throughput-tax probe, and the two
pre-registered verdicts are in [`../../report5.md`](../../report5.md).
