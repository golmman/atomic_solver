# winprover bench measurements — provenance

Measurement record behind [`../../bench.md`](../../bench.md), captured
**2026-10-30**. This directory holds the driver, the parsed results, and this
provenance record. Raw solver/provari transcripts are not committed.

## Provenance

| Item | Value |
| --- | --- |
| Date | 2026-10-30 (UTC) |
| Host | container `d7891fb11b96`; aarch64, 8 CPU; `Linux-7.1.13-402.asahi.fc44.aarch64+16k`; 15 GB RAM |
| Runs | sequential, one process at a time, release builds |
| Provari | `aiorla/provari` commit `48686bc6c786d6ea70568f0b2ad4002cf0520206` (`v0.1.0`), `provari 0.1.0` |
| Provari binary | `/tmp/provari-bench/target/release/provari` (scratch clone + `cargo build --release`, ~6.6 min; not committed) |
| Provari guide | `hand` (primary column); no model/network artifacts fetched |
| `atomic_solver` | commit `96edb8334527291225a9db3b4750f9aebcb88f96`, `./target/release/atomic_solver` |
| Corpus | [`../../../provari/measurements/plan1/corpus.json`](../../../provari/measurements/plan1/corpus.json), sha256 `9cd0169a30bb7d5b162b6a1ec119fe194b776c8a24a46cdf1582c07062847d0e` |
| Bench cells | `core`, `easy-win` (`1eb80b92f92c`), `deep-win` (`db6a741c723f`) — selection rule and the id convention in `bench.md` |
| Budget | 10 s and 192 MB label on both sides; 3 repeats per cell per side |
| Tablebases | none |

The host differs from the one used by `provari` plan1/plan5 (x86, 32 vCPU, AMD
Ryzen 9 5950X): the reference walls shifted (`core` 0.518 s → 0.491 s,
`easy-win` 0.089 s → 0.111 s) and our per-eval throughput is ~1.4× lower. All
factors in this directory are same-session ratios, which is what `bench.md`
requires.

Identifier note: the corpus file and `results.json` key cells by **cell id** —
provari's `bench/high200.jsonl` record id, i.e. the first 12 hex characters of
`sha256(FEN)`, **not** a git hash. `bench.md` maps ids to the short names used
in prose and ledger rows (`core`, `easy-win`, `deep-win`).

## Files

- [`run_bench.py`](run_bench.py) — driver (`--dry-run` prints the commands).
- [`results.json`](results.json) — per-run rows, provenance, and the derived
  ledger (per-cell factors, geomean, coverage) produced by the driver.
- `README.md` — this provenance record.

No `__pycache__` is created (`sys.dont_write_bytecode = True`); no logs,
`*.out`, `*.err`, or TT snapshots are written here.

## Command

```sh
python3 docs/plans/winprover/measurements/bench/run_bench.py \
  --provari /tmp/provari-bench/target/release/provari \
  --solver ./target/release/atomic_solver \
  --cases core,1eb80b92f92c,db6a741c723f --repeats 3 \
  --stack 'default=' \
  --stack 'opt=--move-class-pn --quiet-grouping --eval-pn or' \
  --provari-commit 48686bc6c786d6ea70568f0b2ad4002cf0520206 \
  --solver-commit 96edb8334527291225a9db3b4750f9aebcb88f96
```

Result (2026-10-30, ~2 m 21 s): `default` shared=1 geomean=`51.468`
coverage=1/3; `opt` shared=2 geomean=`6.856` coverage=2/3.

One supplementary probe is recorded in `bench.md` but not in `results.json`
(it exceeds the canonical budget): `deep-win` (`db6a741c723f`) under the `opt`
stack with `--timeout 120` wins in **60.21 s / 326,876,664 child evals**, i.e.
factor 9.70 against the 6.200 s reference.

## Metric conventions

- Wall is measured by the driver around the whole command (process start
  included) for both sides; provari's own `ms`/`first_mate_ms` are also stored.
- Provari's `mate` counts attacker moves; our `outcome: win length: N` is
  plies, so the two lengths are compared only as `2 * mate` vs `length`.
- Provari `nodes` (expansions + simulation steps) and our `child_evals` are
  recorded but not comparable; no ratio between them is published as a score.
- A cell counts as a win only if **every** repeat won; a censored cell keeps
  its bound form and its coverage entry.
