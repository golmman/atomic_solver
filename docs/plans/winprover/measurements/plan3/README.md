# plan3 measurements — provenance

Measurement record behind [`../../report3.md`](../../report3.md): the canonical
three-cell bench for the opt-in win-only front-end (`plan3`, item 3), plus the
front-end's own work accounting. Captured **2026-11-02** (UTC).

## Provenance

| Item | Value |
| --- | --- |
| Date | 2026-11-02 (UTC) |
| Host | container `67db7a1089e2`; aarch64, 8 CPU; `Linux-7.1.13-402.asahi.fc44.aarch64+16k`; glibc 2.43 |
| Runs | sequential, one process at a time, release builds; 3 repeats per cell per side |
| Provari | `aiorla/provari` commit `48686bc6c786d6ea70568f0b2ad4002cf0520206` (`v0.1.0`), `--guide hand` |
| Provari binary | `/tmp/winprover-oracle/target/release/provari` (scratch clone; **not** committed) |
| `atomic_solver` | `d623b2114308f9b3fb224c4f641fab9c0be731ba` + plan3 working tree, `./target/release/atomic_solver` |
| Corpus | [`../../../provari/measurements/plan1/corpus.json`](../../../provari/measurements/plan1/corpus.json) |
| Bench cells | `core`, `easy-win` (`1eb80b92f92c`), `deep-win` (`db6a741c723f`) |
| Budget | 10 s and 192 MB label on both sides; 3 repeats per cell per side |
| Front-end cap | 1,000,000 child evals (the shipped default) |
| Tablebases | none |

## Stacks

- `opt` (baseline): `--move-class-pn --quiet-grouping --eval-pn or`
- `optwin` (candidate): the same stack plus `--win-front-end`

The bench driver also measures its always-on `default` stack; those rows are
in `results.json` but are not part of the plan3 ledger row.

## Files

- [`run_plan3.py`](run_plan3.py) — driver: delegates the canonical bench to
  [`../bench/run_bench.py`](../bench/run_bench.py), then builds and runs the
  front-end probe per cell and writes the two derived files below.
- [`probe.rs`](probe.rs) — runs the front-end alone on one FEN and prints its
  child evals, frames, whether it proved, and wall time. It exists because the
  CLI reports only the combined run counters.
- [`results.json`](results.json) — the bench driver's per-run rows, provenance,
  and derived ledger (per-cell factors, geomean, coverage).
- [`front_end.json`](front_end.json) — per-cell front-end work and the
  `opt`/`optwin` totals.
- [`results.csv`](results.csv) — the ledger cells as a flat table.
- `README.md` — this provenance record.

No raw transcripts, no `__pycache__` (`sys.dont_write_bytecode`), no TT
snapshots, no provari artifacts.

## Commands

```sh
cargo build --release

# canonical bench (provari hand reference + opt + optwin), ~3 min
python3 docs/plans/winprover/measurements/plan3/run_plan3.py \
  --provari /tmp/winprover-oracle/target/release/provari \
  --solver ./target/release/atomic_solver

# re-derive front_end.json / results.csv from an existing results.json
python3 docs/plans/winprover/measurements/plan3/run_plan3.py \
  --provari /tmp/winprover-oracle/target/release/provari \
  --solver ./target/release/atomic_solver --skip-bench
```

The probe is built against the newest
`target/release/deps/libatomic_solver-*.rlib` with
`rustc --edition 2024 -O -C panic=abort -C lto=thin`.

## Result summary

Reference walls this session (provari hand, medians): `core` 0.500 s,
`easy-win` 0.111 s, `deep-win` 6.163 s.

| Cell | `opt` factor (wall / evals) | `optwin` factor (wall / evals) | Front-end evals / frames | Proved? |
| --- | --- | --- | --- | --- |
| `core` | 7.568 (3.784 s / 17,217,064) | 8.308 (4.154 s / 18,217,078) | 1,000,014 / 29,338 | no |
| `easy-win` | 6.126 (0.680 s / 3,231,686) | 9.207 (1.022 s / 4,231,713) | 1,000,027 / 30,387 | no |
| `deep-win` | censored (≥ 1.62), 10.010 s / 47,761,034 | censored (≥ 1.62), 10.007 s / 47,300,751 | 1,000,007 / 32,322 | no |

Canonical geomean: `opt` **6.809** (2 shared cells), `optwin` **8.746** (2
shared cells); coverage 2/3 in both. The front-end proved none of the bench
cells and added ~0.34–0.37 s / ~1.00 M child evals to each run — the expected
neutral result for a skeleton.

For the two cells that finish, `optwin_evals - opt_evals` equals the probe's
front-end evals exactly (the general search is bit-identical under an
unbounded global budget). On `deep-win` the sum is *lower* than the `opt`
control because the front-end consumed ~0.37 s of the 10 s wall budget before
the general search started; the front-end's own count is the probe value.
