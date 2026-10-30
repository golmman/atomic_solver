# Bench — performance ledger vs. provari

Status: **standing document of the [`winprover`](initiative.md) initiative.**
It fixes the benchmark set, the metric, and one *appended* row per executed
plan, so the performance leg of the initiative has a single, comparable
history. It is not a plan and not a report: plans still live in `planN.md`
and their evidence in `reportN.md` / `measurements/planN/`.

## Names and ids (read this first)

Three kinds of identifier appear here and in the measurement files; **only one
of them is a git commit**.

| Kind | Example | Meaning |
| --- | --- | --- |
| Cell short name | `core`, `easy-win`, `deep-win` | Use this in prose, plans, reports and ledger rows. Defined in [The bench set](#the-bench-set). |
| Cell id | `1eb80b92f92c` | **Not a git hash** — provari's record id for a position in its corpus file (detail below). Used by tooling and by the corpus file. |
| Commit pin | `provari-v0.1.0`, `solver@96edb83` | A real git commit: the oracle build, and the solver state a row was measured at. Written in full (40 hex) in the pins list below. |

**Commit pins.**

- `provari-v0.1.0` = `48686bc6c786d6ea70568f0b2ad4002cf0520206` — the pinned
  provari oracle (`v0.1.0`), rebuilt from source for every row, never
  committed. This is the 1.000 side of every factor.
- `solver@96edb83` = `96edb8334527291225a9db3b4750f9aebcb88f96` — the
  `atomic_solver` commit the `bench-v0` row was measured at. Later rows name
  their own `solver@<sha>`.

**Cell ids in detail.** A cell id (`1eb80b92f92c`, `db6a741c723f`, …) is
provari's own record id in its corpus file `bench/high200.jsonl` at the pinned
commit: the first 12 hex characters of `sha256(FEN)`, i.e. a fingerprint of the
position, unique inside that file. The frozen `sample20`
([`corpus.json`](../provari/measurements/plan1/corpus.json)) copies those
`{id, fen}` pairs verbatim, and the bench driver keys cells by id because that
is what the corpus file holds. `core` is the one hand-named cell (it is not in
`high200.jsonl`); its fingerprint would be `8d0ff8bca7f0`, so a measurement
file showing that hash means the same position.

**Rule of thumb:** prose uses short names, tooling uses cell ids, rows record
the commit pins. A cell id is stable forever; a short name is stable as long as
the cell is in the set.

## The goal this ledger measures

The initiative's success condition is a head-to-head win on a **shared-win
set** — provari `--guide hand` is the primary column:

- geometric-mean wall factor `atomic_solver / provari < 1.0`;
- coverage at least as high as provari's at the same budget;
- every claimed win replay-validated (and, from item 2 on, accepted by the
  independent checker);
- no outcome regressions, existing gates green.

`bench.md` tracks the first two. It says nothing about correctness; a speed
claim from this ledger is void unless the same run passed the initiative's
[Working agreement](initiative.md).

## The metric

For one position (a *cell*) and one candidate configuration:

```
factor(cell) = median candidate wall [s] / median provari --guide hand wall [s]
```

- **provari `--guide hand` is 1.000 by definition.** It is the fair
  hand-evaluation-for-hand-evaluation column. The bundled NNUE/model column
  (`--guide model`) is *recorded separately and never mixed in* — it is a
  stretch column that needs an out-of-scope external artifact.
- **`< 1.0` means we are faster than provari on that cell.**
- **Ledger score = geometric mean of `factor(cell)` over the *canonical
  shared cells*** — cells where both sides prove a win inside the canonical
  budget. Censored cells are not part of the geometric mean (they have no
  wall time), they are counted in **coverage** instead.
- **Censoring convention.** A cell where we do not win inside the budget is
  reported as `censored`, and where a bound is meaningful it is printed as
  `≥ budget / reference_wall` (a *lower* bound on the factor, never a factor).
  A censored cell is never dropped and never silently converted into a win.
- **Deterministic accompaniment.** Wall time is the product metric but noisy;
  every row also records `child_evals` (ours, deterministic per stack) and
  provari's `nodes` for the same cell. They are **not** comparable
  one-to-one — our evals per expansion vs. provari's nodes is a diagnostic,
  not a ratio to optimise directly.

### Canonical measurement protocol

| Item | Value |
| --- | --- |
| Budget, both sides | 10 s, 192 MB label (`--seconds 10 --memory 192` vs `--timeout 10 --tt-size 192`) |
| Reference command | `provari prove "<FEN>" --seconds 10 --memory 192 --guide hand` |
| Candidate command | `atomic_solver --fen "<FEN>" --timeout 10 --tt-size 192 --first-outcome --outcome-only <stack flags>` |
| Repeats | 3 per cell per side (medians reported); one process at a time |
| Ordering | all reference runs and candidate runs of a row are measured in the same session, on the same host |
| Host | recorded per row; factors are same-session ratios, so a row is valid on its own host even when another row ran elsewhere |
| Oracle | `provari-v0.1.0`, rebuilt from source per row, never committed |
| Corpus | `corpus-v1` = [`../provari/measurements/plan1/corpus.json`](../provari/measurements/plan1/corpus.json), sha256 `9cd0169a30bb7d5b162b6a1ec119fe194b776c8a24a46cdf1582c07062847d0e` |
| Tablebases | none |

The driver is [`measurements/bench/run_bench.py`](measurements/bench/run_bench.py)
(it writes [`measurements/bench/results.json`](measurements/bench/results.json)
with the per-run rows and the derived ledger). `--cases` takes **cell ids**;
`--stack` names a configuration:

```sh
python3 docs/plans/winprover/measurements/bench/run_bench.py \
  --provari /tmp/provari-bench/target/release/provari \
  --solver ./target/release/atomic_solver \
  --cases core,1eb80b92f92c,db6a741c723f --repeats 3 \
  --stack 'default=' \
  --stack 'opt=--move-class-pn --quiet-grouping --eval-pn or' \
  --provari-commit 48686bc6c786d6ea70568f0b2ad4002cf0520206 \
  --solver-commit "$(git rev-parse HEAD)"
```

The oracle is **rebuilt, never committed** (`provari-v0.1.0`, tag `v0.1.0`);
the hand-guide column needs only `cargo build --release` in a scratch clone,
the model column additionally needs provari's `data/fetch.sh` artifacts.

## The bench set

Three cells, drawn from `corpus-v1` (`core` + the pinned `sample20`).

**Selection rule (fixed):** the bench set is every cell of `core + sample20`
where the pinned provari oracle proves a win **with the hand guide** inside
the canonical budget. Cells where provari itself is censored cannot carry a
factor and are therefore not bench cells, however interesting they are.

| Short name | Cell id | FEN | provari hand (1.000) | Why this cell |
| --- | --- | --- | --- | --- |
| `core` | `core` (hand-named; fingerprint `8d0ff8bca7f0`) | `r1b1kbnr/ppp3pp/n4q2/3pppN1/1P1PP3/8/P1P2PPP/RNBQKB1R w KQkq - 0 1` | 0.491 s · 342,289 nodes · mate 17 | the initiative's motivating FEN; provari's hand guide is ~2× *better* here than `--guide none` |
| `easy-win` | `1eb80b92f92c` | `rnbq1b1r/pp1p1kpp/2p1pp1n/8/3PP1P1/2N2N2/PPP2P1P/R1BQKB1R w KQ - 0 1` | 0.111 s · 72,523 nodes · mate 15 | the one easy shared win; provari's hand guide is >2× *worse* here than `--guide none` (the lever is case-dependent) |
| `deep-win` | `db6a741c723f` | `rnb1kbnr/pppp2pp/5q2/4ppN1/3P4/8/PPP1PPPP/RNBQKB1R w KQkq - 0 1` | 6.200 s · 4,574,912 nodes · mate 22 | the deepest hand-guide win in the corpus; the coverage cell (we do not solve it at 10 s) |

Cells measured and deliberately **out** of the set (recorded so the exclusion
is not silently revisited):

| Cell id | Why not a bench cell |
| --- | --- |
| `b9a7ed7eb368`, `dd3f89766a40` | provari **hand** is censored (10.1 M / 9.3 M nodes); only the out-of-scope bundled model wins them (provari plan1: 4.46 s and 1.86 s) |
| the remaining 16 `sample20` ids | provari's bundled model is censored on all of them, so the hand guide is expected to be censored too; item 1 confirms the hand-guide coverage of the whole corpus |

If item 1 finds an additional hand-guide win, that cell joins the set by the
rule above — it gets a short name, its reference values are measured, and the
ledger gains a new reference row. Earlier rows are not rewritten.

## Ledger

**Row protocol.** Every executed plan whose result touches the win path
appends **one row** (never rewrites an earlier row) with: plan number and
date, host, the measured stack, per-cell factor (or `censored` + bound),
the canonical geometric mean, coverage, and the deterministic
`child_evals`/`nodes`. A plan that only changes housekeeping, tests, or
documentation appends nothing — and says so in its report. A plan that
re-measures an old state to check a host drift appends a row labelled
`re-measure`. A row is read with the `Stack` column, not in isolation: a
speedup that only exists behind an opt-in flag is not a product win until
item 6 decides the default flag set.

### `bench-v0` — 2026-10-30, pre-plan baseline (no initiative plan executed yet)

Host `d7891fb11b96` (aarch64, 8 CPU, Linux 7.1.13); `solver@96edb83`;
`provari-v0.1.0`, `--guide hand`. Reference walls in this session:
`core` 0.491 s, `easy-win` 0.111 s, `deep-win` 6.200 s.

Cell syntax: `factor (our wall / our child_evals)`; the reference
(`provari-v0.1.0` hand) is **1.000** in every row and every cell.

| Plan | Stack | `core` | `easy-win` | `deep-win` | Geomean (canonical) | Coverage |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `bench-v0` | **default** (shipped flags) | censored (≥ 20.4) 10.013 s / 72.9 M | **51.47** (5.713 s / 43.3 M) | censored (≥ 1.61) 10.008 s / 75.3 M | **51.47** (1 cell) | 1/3 |
| `bench-v0` | **opt** (`--move-class-pn --quiet-grouping --eval-pn or`) | **7.67** (3.767 s / 17.2 M) | **6.13** (0.680 s / 3.23 M) | censored at 10 s (47.8 M); **9.70** at a 120 s budget (60.21 s / 326.9 M) | **6.86** (2 cells) | 2/3 |

Readings of the baseline:

- **Distance to the goal: 6.86×.** The canonical geomean must fall below 1.0
  with coverage 3/3 — i.e. roughly a 7× improvement on the two shared cells
  *plus* the `deep-win` coverage gap closed.
- The coverage gap is as large as the latency gap: `deep-win` needs 60.2 s
  where provari needs 6.2 s, and the shipped default stack does not solve
  `core` at all.
- The two shipped win-path levers (measured-out table in `initiative.md`)
  account for the difference between the two rows (`easy-win` 51.5 → 6.13,
  `core` censored → 7.67), and they are still opt-in.
- Proof lengths differ by cell (ours 29 plies vs provari's 34 on `core`; 31 vs
  30 on `easy-win`; 49 vs 44 on `deep-win`), so a factor is "time to *a*
  proven win", not time to the shortest one — see the initiative's item 5
  (proof length) before treating length differences as noise.

### Rows to come

Empty. Item 1 (`plan1`) is a **reference-only** plan: it re-measures the
hand-guide column over the whole corpus with the committed driver and, by the
selection rule above, may add cells to the bench set. It appends no ledger row
(it does not touch the win path) — reference revisions and any set addition are
recorded here as a dated note, with the earlier rows left intact.

## Reading rules (do not violate these when appending a row)

1. **Same session, same host.** A factor compares two walls measured back to
   back on one host. Cross-host precision is ~±15–20 %: the reference wall for
   `core` has measured 0.403 s (plan-session probe, x86 32-vCPU host), 0.518 s
   (provari plan1 driver, same host) and 0.491 s (this host), and our own
   per-eval throughput differed by ~40 % between the x86 host and this one.
   Do not compare a new factor against an old row and call the difference a
   lever effect — re-measure, or state the drift.
2. **Wall is the product metric; `child_evals` is the mechanism metric.** A
   child-eval reduction that does not move the wall is a mechanism finding,
   not a speed win (and vice versa: a wall win with identical evals is a
   per-node cost effect, `performance`'s territory).
3. **Censored is a result.** `≥ budget/reference` bounds and coverage counts
   are the honest form; a censored cell may never be replaced by the wall of
   a longer run without stating the budget used (as done for `deep-win`
   above).
4. **Opt-in ≠ shipped.** The ledger's `Stack` column is part of the result.
   Only a default-flag decision (item 6) turns a stack row into a product
   row; until then both stack rows stay visible.
5. **Correctness gates precede the number.** A row is only written after the
   plan's gate passed (`make test`, plus quick suite / repetition soundness /
   drift protocol for behaviour-changing levers) and, for new proof shapes,
   after the item-2 checker accepted the exported proof.
6. **Coverage is a first-class target.** A geomean below 1.0 on two cells
   while the third stays censored does not meet the goal. State coverage in
   every row.

## History

- **2026-10-30** — `bench.md` opened with the `bench-v0` baseline: three-cell
  set (`core`, `easy-win`, `deep-win`; selection rule above), the
  `provari-v0.1.0` `--guide hand` reference column rebuilt from source on a new
  host, the canonical protocol, and the driver
  `measurements/bench/run_bench.py`. Baseline: default stack 51.47× on one
  shared cell (coverage 1/3); shipped opt-in stack 6.86× geomean on two
  shared cells (coverage 2/3, the third cell 9.70× only at a 120 s budget). No
  product code changed.
- **2026-10-30 (same session)** — identifiers separated explicitly after a
  read-through: cell short names (`core`, `easy-win`, `deep-win`) for prose,
  cell ids (provari `high200` record ids = `sha256(FEN)[:12]`, **not** git
  hashes) for tooling, and full commit pins (`provari-v0.1.0`, `solver@<sha>`)
  for provenance. Numbers and verdicts unchanged.
