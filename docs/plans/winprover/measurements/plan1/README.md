# plan1 measurements — provenance

Measurement record behind [`../../report1.md`](../../report1.md) and the dated
note in [`../../bench.md`](../../bench.md). Captured **2026-11-02** (UTC).

## Provenance

| Item | Value |
| --- | --- |
| Date | 2026-11-02 (UTC) |
| Host | container `67db7a1089e2`; aarch64, 8 CPU; `Linux-7.1.13-402.asahi.fc44.aarch64+16k`; glibc 2.43 |
| Runs | sequential, one process at a time, release builds |
| Provari | `aiorla/provari` commit `48686bc6c786d6ea70568f0b2ad4002cf0520206` (`v0.1.0`) |
| Provari version | `provari 0.1.0, with its network and model built in` (`cargo build --release --features bundled`) |
| Mode artifacts | `network.nnue` sha256 `99dc67eabf26a64faeeca3a88b4c38597a840b8d4a874b9f2cf658c6f92a04a6`; `model.json` sha256 `8822a85fde620b734b23e99bf48263b3c82bc6fbe706fc8a836f710560f7f987` (fetched by `data/fetch.sh`, **not** committed) |
| Provari binary | `/tmp/winprover-oracle/target/release/provari` (scratch clone + bundled build; not committed) |
| Guides | `hand` (primary column) and `model` (stretch column, recorded separately) |
| `atomic_solver` | commit `1f949812415e252a9d665c9719a0bd1d3e65a8a2`, `./target/release/atomic_solver` |
| Corpus | [`../../../provari/measurements/plan1/corpus.json`](../../../provari/measurements/plan1/corpus.json), sha256 `9cd0169a30bb7d5b162b6a1ec119fe194b776c8a24a46cdf1582c07062847d0e` |
| Corpus check | digest matches; `sample20` has exactly 20 unique rows; every row id equals `sha256(fen)[:12]`; `core` FEN byte-identical to `bench.md`; `core` fingerprint `8d0ff8bca7f0` |
| Budget | 10 s and 192 MB label on both sides |
| Repeats | 1 per cell for the 21-cell census; 3 per cell per side for `core`, `1eb80b92f92c`, `db6a741c723f` |
| Tablebases | none (provari built without syzygy data; ours has no tablebase support wired in) |

Hand-guide invariance check: the bundled binary reproduces the `bench-v0`
hand numbers exactly (`core`: 342,289 nodes, mate 17), so switching to the
bundled build did not disturb the primary column.

## Files

- [`results_hand_census.json`](results_hand_census.json) — provari hand + both
  solver stacks, 1 repeat, all 21 cells.
- [`results_hand_bench.json`](results_hand_bench.json) — same, 3 repeats, the
  three bench cells.
- [`results_model_census.json`](results_model_census.json) — provari model +
  both stacks, 1 repeat, all 21 cells.
- [`results_model_bench.json`](results_model_bench.json) — same, 3 repeats,
  the three bench cells.
- [`derive.py`](derive.py) — merges the four files, recomputes medians and
  coverage from the raw rows, writes `census.json` + `census.md`.
- [`census.json`](census.json) / [`census.md`](census.md) — the derived
  per-cell table, coverage per column, shared-win sets, bench-set verdict.
- `README.md` — this provenance record.

No raw transcripts, no logs, no `__pycache__` (`sys.dont_write_bytecode`), no
TT snapshots, no provari artifacts.

## Driver extension (measurement tooling only)

`../bench/run_bench.py` gained `--guide {hand,model,none}` (default `hand`)
and a dynamic reference side label `provari-<guide>`; the default command,
default output path and the `provari-hand` label are unchanged, so the
`bench.md` canonical command still works verbatim.

## Commands

Oracle build (scratch, not committed):

```sh
git clone https://github.com/aiorla/provari /tmp/winprover-oracle
git -C /tmp/winprover-oracle checkout 48686bc6c786d6ea70568f0b2ad4002cf0520206
cd /tmp/winprover-oracle && ./data/fetch.sh
cargo build --release --features bundled
./target/release/provari --version   # provari 0.1.0, with its network and model built in
sha256sum data/network.nnue data/model.json
```

Measurements (from the repo root):

```sh
python3 docs/plans/winprover/measurements/bench/run_bench.py \
  --provari /tmp/winprover-oracle/target/release/provari \
  --solver ./target/release/atomic_solver \
  --guide hand --repeats 1 \
  --stack 'opt=--move-class-pn --quiet-grouping --eval-pn or' \
  --provari-commit 48686bc6c786d6ea70568f0b2ad4002cf0520206 \
  --solver-commit "$(git rev-parse HEAD)" \
  --out docs/plans/winprover/measurements/plan1/results_hand_census.json

# same with --repeats 3 --cases core,1eb80b92f92c,db6a741c723f
#           --out .../results_hand_bench.json
# and with --guide model for results_model_{census,bench}.json

python3 docs/plans/winprover/measurements/plan1/derive.py
```

Runtimes on this host: hand census 9 m 48 s, hand bench 2 m 21 s, model census
9 m 30 s, model bench 2 m 7 s (dominated by censored 10 s cells).

## Result summary

- provari `--guide hand` wins **3/21** cells (`core`, `1eb80b92f92c`,
  `db6a741c723f`) — exactly the shipped bench set; **no new hand wins**, none
  removed.
- provari `--guide model` wins **5/21** (`core` plus the four above-named
  sample20 model wins `b9a7ed7eb368`, `dd3f89766a40`), i.e. 4/20 on
  `sample20`, matching `provari/report1.md`.
- Our stacks: `default` 1/21, `opt` 2/21. Shared-win factors: `hand×default`
  51.739 (1 cell); `hand×opt` 6.848 (2 cells).
