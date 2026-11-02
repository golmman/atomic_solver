# plan5 measurements — provenance

Measurement record behind [`../../report5.md`](../../report5.md): the
default-flag decision (quick-suite comparison) and the final canonical
head-to-head bench for `plan5` (item 6). Captured **2026-11-02** (UTC).

## Provenance

| Item | Value |
| --- | --- |
| Date | 2026-11-02 (UTC) |
| Host | container `67db7a1089e2`; aarch64, 8 CPU; `Linux-7.1.13-402.asahi.fc44.aarch64+16k`; glibc 2.43 |
| Runs | sequential, one process at a time, release builds |
| Provari | `aiorla/provari` commit `48686bc6c786d6ea70568f0b2ad4002cf0520206` (`v0.1.0`), `--guide hand` |
| Provari binary | `/tmp/winprover-oracle/target/release/provari` (scratch clone; **not** committed) |
| `atomic_solver` | `35f78bf` (clean tree; **no product code changed by plan5**), `./target/release/atomic_solver` |
| Corpus | [`../../../provari/measurements/plan1/corpus.json`](../../../provari/measurements/plan1/corpus.json) |
| Bench cells | `core`, `easy-win` (`1eb80b92f92c`), `deep-win` (`db6a741c723f`) |
| Bench budget | 10 s and 192 MB label on both sides; 3 repeats per cell per side |
| Quick suite | `tests/fixtures/decisive_positions.txt` (46) + `move_order_positions.txt` `mNN` with `NN >= 23` (13) = 59 cases; `--timeout 3 --first-outcome`, 1 run per case per stack |
| Tablebases / NNUE | none |

## Stacks

- `default` — the shipped flag set (all four experimental levers off; this is
  the **chosen default** after the plan5 decision).
- `opt` — the best measured opt-in stack:
  `--move-class-pn --quiet-grouping --eval-pn or`.

The quick suite runs the product CLI with no flags vs the opt flags. The bench
driver (`../bench/run_bench.py`) always measures its own `default` stack and
adds `opt`; no `opt+win` front-end control was measured (optional in the plan;
the front-end thread is closed and the front-end is pure overhead on these
cells).

## Files

- [`run_plan5.py`](run_plan5.py) — driver: parses the quick-suite fixtures,
  runs both stacks through the CLI, compares outcomes against the fixture
  expectations, delegates the canonical bench to
  [`../bench/run_bench.py`](../bench/run_bench.py), and writes the derived
  files below.
- [`results.json`](results.json) — provenance, the 59-case quick-suite table
  and summary, and the bench driver's per-run rows plus derived ledger.
- [`results.csv`](results.csv) — the bench ledger cells as a flat table.
- `README.md` — this provenance record.

No raw transcripts, no `__pycache__` (`sys.dont_write_bytecode`), no TT
snapshots, no provari artifacts.

## Commands

```sh
cargo build --release

# full record: quick suite + canonical bench (~3 min)
python3 docs/plans/winprover/measurements/plan5/run_plan5.py \
  --provari /tmp/winprover-oracle/target/release/provari \
  --solver ./target/release/atomic_solver

# quick suite only / bench only
python3 docs/plans/winprover/measurements/plan5/run_plan5.py ... --skip-bench
python3 docs/plans/winprover/measurements/plan5/run_plan5.py ... --skip-quick

# re-derive results.csv from the existing results.json
python3 docs/plans/winprover/measurements/plan5/run_plan5.py ... \
  --skip-quick --skip-bench
```

The CLI prints `evals:` on stderr (the outcome-only stdout contract is
byte-exact), so the driver parses stdout + stderr together. The default
stack's quick-suite total (**42,109,768** child evals) reproduces the
unbudgeted quick-suite anchor recorded in conversion report12,
which validates the driver against the canonical harness.

## Result summary

### Quick suite (default decision)

| Stack | Correct | Draws (censored at 3 s) | Wrong | Total child evals |
| --- | ---: | ---: | ---: | ---: |
| `default` | 59/59 | 0 | 0 | 42,109,768 |
| `opt` | 56/59 | 3 | 0 | 74,298,511 |

Regressions (all `Loss` → `draw` at 3 s):

| Case | Expected | `default` | `opt` |
| --- | --- | --- | --- |
| `dec06` | Loss | loss, 1,507,482 evals, 0.188 s | draw (timeout), 18,749,994 evals, 3.003 s |
| `dec42` | Loss | loss, 80,849 evals, 0.016 s | draw (timeout), 11,400,625 evals, 3.006 s |
| `dec44` | Loss | loss, 102,615 evals, 0.025 s | draw (timeout), 19,874,241 evals, 3.007 s |

The opt stack uses fewer evals on 41/59 cases, but the three losses above turn
into full-budget draws, so per the pre-registered rule the opt stack is **not
promoted**: the shipped defaults stay.

### Canonical head-to-head

Reference (provari hand, medians): `core` 0.499 s · 342,289 nodes,
`easy-win` 0.131 s · 72,523 nodes, `deep-win` 6.297 s · 4,574,912 nodes.
Factor = our wall / reference wall.

| Cell | `default` (chosen) | `opt` (best measured, not promoted) |
| --- | --- | --- |
| `core` | censored (≥ 20.04) 10.011 s / 72,189,193 evals | **7.615** (3.800 s / 17,217,064 evals) |
| `easy-win` | **44.061** (5.772 s / 43,266,033 evals) | **5.214** (0.683 s / 3,231,686 evals) |
| `deep-win` | censored (≥ 1.59) 10.012 s / 74,317,419 evals | censored (≥ 1.59) 10.011 s / 47,552,244 evals |

| Stack | Geomean (canonical) | Coverage |
| --- | ---: | ---: |
| `default` (chosen) | **44.061** (1 cell) | 1/3 |
| `opt` | **6.301** (2 cells) | 2/3 |

Provari hand coverage is 3/3 in the same session. The initiative's success
condition (geomean `< 1.0`, coverage ≥ provari) is **not met** by either stack:
the product default is 44× on its one shared cell, the best measured opt-in
stack is 6.30× on two shared cells, and both are censored on `deep-win`. The
opt stack's deterministic evals equal the plan3/plan4 control exactly on
`core` (17,217,064) and `easy-win` (3,231,686), so the run is consistent with
the earlier rows; the reference walls are this session's own (reading rule 1).
