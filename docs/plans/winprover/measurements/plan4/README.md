# plan4 measurements — provenance

Measurement record behind [`../../report4.md`](../../report4.md): the canonical
three-cell bench for the first-form sibling replay (`plan4`, item 4), plus the
front-end's own work accounting at 1 M and 10 M child evals with simulation on
and off. Captured **2026-11-02** (UTC).

## Provenance

| Item | Value |
| --- | --- |
| Date | 2026-11-02 (UTC) |
| Host | container `67db7a1089e2`; aarch64, 8 CPU; `Linux-7.1.13-402.asahi.fc44.aarch64+16k`; glibc 2.43 |
| Runs | sequential, one process at a time, release builds; 3 repeats per cell per side |
| Provari | `aiorla/provari` commit `48686bc6c786d6ea70568f0b2ad4002cf0520206` (`v0.1.0`), `--guide hand` |
| Provari binary | `/tmp/winprover-oracle/target/release/provari` (scratch clone; **not** committed) |
| `atomic_solver` | `e3d7b1f` + plan4 working tree, `./target/release/atomic_solver` |
| Corpus | [`../../../provari/measurements/plan1/corpus.json`](../../../provari/measurements/plan1/corpus.json) |
| Bench cells | `core`, `easy-win` (`1eb80b92f92c`), `deep-win` (`db6a741c723f`) |
| Budget | 10 s and 192 MB label on both sides; 3 repeats per cell per side |
| Front-end caps | 1,000,000 (shipped default) and 10,000,000 (gate cap) child evals |
| Simulation budget | 16 positions per replay |
| Scratch TT | 32 MB (`WIN_FRONT_END_TT_MB`), dropped per run |
| Tablebases / NNUE | none |

## Stacks

- `opt` (baseline): `--move-class-pn --quiet-grouping --eval-pn or`
- `optwin` (plan3 control): the same stack plus
  `--win-front-end --no-win-front-end-simulation`
- `optwinsim` (candidate): the same stack plus `--win-front-end`
  (simulation on by default)

The bench driver also measures its always-on `default` stack; those rows are
in `results.json` but are not part of the plan4 ledger row.

## Files

- [`run_plan4.py`](run_plan4.py) — driver: delegates the canonical bench to
  [`../bench/run_bench.py`](../bench/run_bench.py), then builds and runs the
  front-end probe at both caps with simulation on/off and writes the derived
  files below.
- [`probe.rs`](probe.rs) — runs the front-end alone on one FEN and prints its
  child evals, frames, proof, and simulation attempt/success/eval counters.
- [`results.json`](results.json) — the bench driver's per-run rows, provenance,
  and derived ledger (per-cell factors, geomean, coverage).
- [`front_end.json`](front_end.json) — per-cell front-end work at both caps,
  `opt`/`optwin`/`optwinsim` totals, and the ledger summary.
- [`results.csv`](results.csv) — the ledger cells as a flat table.
- `README.md` — this provenance record.

No raw transcripts, no `__pycache__` (`sys.dont_write_bytecode`), no TT
snapshots, no provari artifacts.

## Commands

```sh
cargo build --release

# canonical bench (provari hand reference + opt + optwin + optwinsim) and
# front-end probe (1 M/10 M, simulation off/on), ~8 min
python3 docs/plans/winprover/measurements/plan4/run_plan4.py \
  --provari /tmp/winprover-oracle/target/release/provari \
  --solver ./target/release/atomic_solver

# re-derive front_end.json / results.csv from an existing results.json
python3 docs/plans/winprover/measurements/plan4/run_plan4.py \
  --provari /tmp/winprover-oracle/target/release/provari \
  --solver ./target/release/atomic_solver --skip-bench
```

The probe is built against the newest
`target/release/deps/libatomic_solver-*.rlib` with
`rustc --edition 2024 -O -C panic=abort -C lto=thin`.

## Result summary

Reference walls this session (provari hand, medians): `core` 0.496 s,
`easy-win` 0.113 s, `deep-win` 6.198 s.

| Cell | `opt` factor (wall / evals) | `optwin` (sim off) | `optwinsim` (sim on) |
| --- | --- | --- | --- |
| `core` | 7.633 (3.786 s / 17,217,064) | 8.383 (4.158 s / 18,217,078) | 8.337 (4.135 s / 18,217,089) |
| `easy-win` | 5.858 (0.662 s / 3,231,686) | 8.858 (1.001 s / 4,231,713) | 8.646 (0.977 s / 4,231,703) |
| `deep-win` | censored (≥ 1.61) 10.009 s / 47.8 M | censored (≥ 1.61) 10.004 s / 47.2 M | censored (≥ 1.61) 10.008 s / 47.3 M |

Canonical geomean: `opt` **6.687** (2 shared cells), `optwin` **8.617**,
`optwinsim` **8.490**; coverage 2/3 in all three. The front-end proved none of
the three cells at either cap. At 10 M, simulation cut front-end frames by
~20–24% (e.g. `core` 294,360 → 224,470) while child evals stayed at the cap;
the benchmark difference from `optwin` is noise-level (4.158 → 4.135 s on
`core`, 1.001 → 0.977 s on `easy-win`).

Front-end probe (same cells, simulation off → on):

| Cell | Cap | Evals off / on | Frames off / on | Attempts | Successes | Sim evals | Proved |
| --- | ---: | --- | --- | ---: | ---: | ---: | --- |
| `core` | 1 M | 1,000,014 / 1,000,025 | 29,338 / 21,993 | 3,889 | 461 | 242,465 | no / no |
| `easy-win` | 1 M | 1,000,027 / 1,000,017 | 30,387 / 21,587 | 4,683 | 628 | 277,265 | no / no |
| `deep-win` | 1 M | 1,000,007 / 1,000,274 | 32,322 / 27,958 | 7,724 | 1,185 | 147,291 | no / no |
| `core` | 10 M | 10,000,043 / 10,000,034 | 294,360 / 224,470 | 46,194 | 4,646 | 2,438,170 | no / no |
| `easy-win` | 10 M | 10,000,023 / 10,000,032 | 303,251 / 227,110 | 63,489 | 7,553 | 2,579,543 | no / no |
| `deep-win` | 10 M | 10,000,013 / 10,000,025 | 316,483 / 260,353 | 67,003 | 18,216 | 1,907,735 | no / no |

Net: replay succeeds on ~12–27% of attempts, but the transferred proofs do not
close a root; the gate's 10 M cap proves neither `core` nor `easy-win`.
