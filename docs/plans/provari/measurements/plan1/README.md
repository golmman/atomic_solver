# Provari plan1 measurements — provenance

Baseline captured **2026-10-29** for provari plan1
([`../../plan1.md`](../../plan1.md)). This directory contains the committed
corpus, the driver, the parsed results, and the provenance record. Raw
transcripts are not committed.

## Provenance

| Item | Value |
| --- | --- |
| Date | 2026-10-29 (UTC) |
| Host | container `bc5b4ea98f93`; AMD Ryzen 9 5950X, 32 vCPU; 31 GB RAM; Linux 7.2.13-200.fc44.x86_64 |
| Runs | sequential, one process at a time, release builds |
| Provari | `aiorla/provari` commit `48686bc6c786d6ea70568f0b2ad4002cf0520206` (`v0.1.0`) |
| Provari binary | `/tmp/provari-oracle/target/release/provari` (not committed) |
| Provari version | `provari 0.1.0, with its network and model built in` |
| Build | `./data/fetch.sh`, then `cargo build --release --features bundled` |
| Network | `data/network.nnue`, sha256 `99dc67eabf26a64faeeca3a88b4c38597a840b8d4a874b9f2cf658c6f92a04a6` (47,721,376 bytes) |
| Model | `data/model.json`, sha256 `8822a85fde620b734b23e99bf48263b3c82bc6fbe706fc8a836f710560f7f987` (49,792 bytes) |
| Toolchain | `rustc 1.99.0 (b940084d7 2026-09-28)`, `cargo 1.99.0 (5f94df478 2026-08-27)` |
| `atomic_solver` | commit `9ae627a958283db76f52eb6127fbf45c1ee3207a`, `./target/release/atomic_solver` |
| Budget | 10 s, 192 MB label on both sides |

No tablebases were used. No provari source, binary, network, or model is
committed. The clone lives in `/tmp` and only ideas may cross the GPL
boundary.

## Corpus

[`corpus.json`](corpus.json) is the committed definition:

- `core` — `r1b1kbnr/ppp3pp/n4q2/3pppN1/1P1PP3/8/P1P2PPP/RNBQKB1R w KQkq - 0 1`;
- `sample20` — every 10th line of provari's `bench/high200.jsonl` at the
  pinned commit, 1-based line indices 1, 11, 21, …, 191 (20 rows), copied
  as `{id, fen}` pairs.

Corpus check at execution time:

- `sample20` has 20 rows;
- the rows equal the every-10th selection from the pinned
  `bench/high200.jsonl`;
- the core FEN is byte-identical to the one in `plan1.md`.

## Commands

Provari, primary configuration:

```sh
/tmp/provari-oracle/target/release/provari prove "<FEN>" \
  --seconds 10 --memory 192 --guide model
```

Provari, core hand control (3 runs):

```sh
/tmp/provari-oracle/target/release/provari prove "<core FEN>" \
  --seconds 10 --memory 192 --guide hand
```

Our solver:

```sh
./target/release/atomic_solver \
  --fen "<FEN>" --timeout 10 --first-outcome --outcome-only --tt-size 192
```

Proof validation of the core run (wall time not used for the baseline):

```sh
/tmp/provari-oracle/target/release/provari prove "<core FEN>" \
  --seconds 10 --memory 192 --guide model --proofs /tmp/provari-core-proof.pvr
/tmp/provari-oracle/target/release/provari check /tmp/provari-core-proof.pvr
# {"valid":true,"moves":13,"won_positions":9043,"highest_counter":99}
```

Driver invocation that produced [`results.json`](results.json):

```sh
python3 docs/plans/provari/measurements/plan1/run_baseline.py \
  --provari /tmp/provari-oracle/target/release/provari \
  --solver ./target/release/atomic_solver \
  --provari-commit 48686bc6c786d6ea70568f0b2ad4002cf0520206 \
  --provari-features bundled \
  --solver-commit 9ae627a958283db76f52eb6127fbf45c1ee3207a \
  --date 2026-10-29 \
  --host 'bc5b4ea98f93 (container; AMD Ryzen 9 5950X, 32 vCPU)'
```

The driver runs core 3× per configuration (bundled model, hand control, and
our solver), then one run per `sample20` case, then the core proof check.
Re-running the driver replaces `results.json`; the rows are deterministic for
these commands except for wall-clock noise.

## Metric conventions

- Provari's JSON `mate` is the number of attacker moves. The plan schema's
  `first_win_plies` is recorded as `2 * mate` (e.g. core mate 13 → 26),
  matching the convention in `plan1.md`.
- `wall_s` is measured by the driver around the whole command for both sides.
  Provari's `first_mate_ms` and `ms` are also stored; `first_mate_ms` is its
  first-win latency, while `ms` includes proof bookkeeping.
- Provari nodes are search expansions plus simulation steps and live in
  `provari_nodes`. Our `solver_nodes` is unavailable under `--outcome-only`
  (the `pre_exit:` line is suppressed), so the machine-readable work metric is
  `child_evals`. The two node counters are not comparable and are never
  collapsed.
- 192 MB is a single numeric label with different meanings:
  `--memory 192` is provari's table-plus-proofs split; `--tt-size 192` is our
  TT reservation bound (pages populated on use). `results.json` records both
  labels.
- Censored rows are kept: provari `found:false` → `outcome:"none"`,
  `censored:true`, `exit:"Budget"`; our budget-ended non-win →
  `outcome:"draw"`, `censored:true`, `exit:"Timeout"`. No timeout was
  converted into an omitted row.

## Files

- [`corpus.json`](corpus.json) — fixed corpus definition and selection rule.
- [`run_baseline.py`](run_baseline.py) — driver (`--help`, `--dry-run`).
- [`results.json`](results.json) — parsed rows, provenance, core proof check,
  and summary.
- `README.md` — this provenance record.
- [`../../report1.md`](../../report1.md) — baseline report.

## `.gitignore` / cleanup note

Raw solver stdout/stderr, driver progress logs, the provari clone/binary, and
`network.nnue`/`model.json` are intentionally absent. The measurement
convention ignores `logs/`, `*.out`, `*.err`, `*.log`, and `*.time` under
`measurements/`; no such files were written here (driver output went to
`/tmp`). The driver sets `sys.dont_write_bytecode = True`, so no
`__pycache__` is created.
