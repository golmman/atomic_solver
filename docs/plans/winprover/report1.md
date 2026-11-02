# Winprover Report 1 — fair hand-guide baseline and corpus re-derivation

Initiative: `winprover`, backlog **item 1**, executed as
[`plan1.md`](plan1.md). Measurement-only: no product code, no search
behaviour, no proof-format change. Date: **2026-11-02** (UTC).
Plan2 recommendation: **item 2, the differential proof-checker oracle**
([`interop/plan1.md`](../interop/plan1.md) re-cut against the current binary
format) — evidence in [§9](#9-recommended-plan2).

## Headline

- The shipped bench set is **confirmed by the fixed selection rule**: provari
  `--guide hand` wins exactly `core`, `easy-win` (`1eb80b92f92c`) and
  `deep-win` (`db6a741c723f`) and nothing else in `core + sample20`.
  **No additions, no removals.**
- Hand-guide coverage: **3/21** cells (`2/20` on `sample20`). Stretch
  `--guide model` coverage: **5/21** (`4/20`).
- Our stacks in the same session: `default` **1/21**, `opt` **2/21**.
- Shared-win factors (`our wall / provari hand wall`): `hand×default`
  **51.739** (1 cell), `hand×opt` **6.848** (2 cells) — statistically
  indistinguishable from `bench-v0` (51.468 / 6.856), so this host is in the
  same performance class.
- The model column was **measured**, not `INCONCLUSIVE`: the `--guide model`
  column is 2.00× faster geometrically on the three bench cells, but it pays a
  ~1.45× throughput penalty per node, which is why node counts flatter it and
  the wall is the honest comparison.
- **No ledger row** (plan1 does not touch the win path).

## 1. What was done

1. Rebuilt the pinned oracle from source in a scratch clone
   (`48686bc6c786d6ea70568f0b2ad4002cf0520206`), both plain and
   `--features bundled`; the bundled build succeeded, so both guides are
   measured with one binary (the hand guide does not read the model).
2. Extended the bench driver with `--guide {hand,model,none}` (default
   `hand`) and the dynamic reference label `provari-<guide>`; the default
   command, default output path and the `provari-hand` label are unchanged, so
   the `bench.md` canonical command still works verbatim. `results.json`
   (`bench-v0`) is untouched.
3. Verified the corpus: digest
   `9cd0169a30bb7d5b162b6a1ec119fe194b776c8a24a46cdf1582c07062847d0e`,
   exactly 20 unique `sample20` rows, every id equal to `sha256(fen)[:12]`,
   `core` FEN byte-identical to `bench.md` (fingerprint `8d0ff8bca7f0`).
4. Measured: hand census (21 cells × 1 repeat), hand bench (3 cells × 3
   repeats), model census (21 × 1), model bench (3 × 3) — reference plus both
   candidate stacks in every invocation, sequential, one process at a time.
5. Derived `census.json` / `census.md` with `derive.py`, recomputing medians
   and coverage from the raw rows (never from the driver's own ledger).
6. Updated `bench.md` with a dated reference note (bench set unchanged) and
   wrote this report.

## 2. Provenance

| Item | Value |
| --- | --- |
| Date | 2026-11-02 (UTC) |
| Host | container `67db7a1089e2`; aarch64, 8 CPU; `Linux-7.1.13-402.asahi.fc44.aarch64+16k`, glibc 2.43 |
| Provari | `aiorla/provari` `48686bc6c786d6ea70568f0b2ad4002cf0520206` (`v0.1.0`) |
| Provari version | `provari 0.1.0, with its network and model built in` (`--features bundled`) |
| Model artifacts | `network.nnue` `99dc67eabf26a64faeeca3a88b4c38597a840b8d4a874b9f2cf658c6f92a04a6`; `model.json` `8822a85fde620b734b23e99bf48263b3c82bc6fbe706fc8a836f710560f7f987` (fetched to the scratch clone, not committed) |
| `atomic_solver` | `1f949812415e252a9d665c9719a0bd1d3e65a8a2`, release build |
| Corpus | [`provari/measurements/plan1/corpus.json`](../provari/measurements/plan1/corpus.json), sha256 `9cd0169a…47d0e` (verified) |
| Budget | 10 s and 192 MB label on both sides; `--seconds 10 --memory 192` vs `--timeout 10 --tt-size 192` |
| Stacks | `default` (no flags) and `opt` = `--move-class-pn --quiet-grouping --eval-pn or` |
| Repeats | 1 per cell (census), 3 per cell per side on the three bench cells |
| Tablebases | none |

Hand-guide invariance check: the bundled binary reproduces `bench-v0`'s hand
search exactly (`core`: 342,289 nodes, mate 17), so the bundled feature did
not perturb the primary column. Cross-host sanity: the re-measured reference
medians on the three bench cells are `core` 0.490 s (was 0.491 s),
`easy-win` 0.111 s (was 0.111 s), `deep-win` 6.240 s (was 6.200 s) — within
0.7 %, so this host is a comparable measurement class; all factors below are
still same-session ratios, as `bench.md` requires.

## 3. Exact commands

Oracle (scratch only, nothing committed):

```sh
git clone https://github.com/aiorla/provari /tmp/winprover-oracle
git -C /tmp/winprover-oracle checkout 48686bc6c786d6ea70568f0b2ad4002cf0520206
cd /tmp/winprover-oracle && ./data/fetch.sh && cargo build --release --features bundled
./target/release/provari --version   # provari 0.1.0, with its network and model built in
```

Measurements (repo root, one process at a time):

```sh
python3 docs/plans/winprover/measurements/bench/run_bench.py \
  --provari /tmp/winprover-oracle/target/release/provari \
  --solver ./target/release/atomic_solver \
  --guide hand --repeats 1 \
  --stack 'opt=--move-class-pn --quiet-grouping --eval-pn or' \
  --provari-commit 48686bc6c786d6ea70568f0b2ad4002cf0520206 \
  --solver-commit "$(git rev-parse HEAD)" \
  --out docs/plans/winprover/measurements/plan1/results_hand_census.json

# 3-repeat bench cells (no new hand win was found, so the set is unchanged):
#   --repeats 3 --cases core,1eb80b92f92c,db6a741c723f
#   --out …/results_hand_bench.json
# model column, same two invocations: --guide model → results_model_{census,bench}.json
```

Derivation: `python3 docs/plans/winprover/measurements/plan1/derive.py`
(writes `census.json` + `census.md`). Runtimes: 9 m 48 s / 2 m 21 s / 9 m 30 s
/ 2 m 7 s, dominated by censored 10 s cells.

## 4. Results

### 4.1 Coverage per column (canonical budget)

| Column | `core + sample20` | `sample20` only |
| --- | ---: | ---: |
| provari `--guide hand` (primary) | **3/21** | **2/20** |
| provari `--guide model` (stretch) | 5/21 | 4/20 |
| `atomic_solver` default | 1/21 | 1/20 |
| `atomic_solver` opt | 2/21 | 1/20 |

Provari hand's wins are `core`, `easy-win`, `deep-win`. Our `opt` stack wins
`core` and `easy-win`, so its only coverage gap on the primary column is
`deep-win`; `default` wins `easy-win` only, so it also misses `core`.
On `sample20` alone the hand column is 2/20 and `opt` is 1/20 (the same
`deep-win` gap).

### 4.2 Per-cell table (all 21 cells)

`ref` = provari wall / nodes / mate (mate in attacker moves); ours = wall /
child_evals / PV plies; `cens` = censored at the 10 s budget (with the
`≥ 10 s / ref_wall` lower bound where a reference win exists). Bench cells use
their 3-repeat rows, the other 18 their 1-repeat census row.

| Cell | provari hand | provari model | default | opt |
| --- | --- | --- | --- | --- |
| `core` | 0.490 s / 342,289 n / mate 17 | 0.357 s / 169,485 n / mate 13 | cens (≥ 20.41) | 3.801 s / 17.2 M / 29 ply |
| `2a0443d446ee` | cens | cens | cens | cens |
| `b9a7ed7eb368` | cens | 4.530 s / 2,452,328 n / mate 29 | cens | cens |
| `2e8276dc4530` | cens | cens | cens | cens |
| `easy-win` (`1eb80b92f92c`) | 0.111 s / 72,523 n / mate 15 | 0.071 s / 13,779 n / mate 20 | 5.743 s / 43.3 M / 37 ply | 0.671 s / 3.2 M / 31 ply |
| `6559fbdddb47` | cens | cens | cens | cens |
| `3004d09cef79` | cens | cens | cens | cens |
| `5862b1958212` | cens | cens | cens | cens |
| `dd3f89766a40` | cens | 1.853 s / 911,709 n / mate 29 | cens | cens |
| `8271708709d4` | cens | cens | cens | cens |
| `60af5542800b` | cens | cens | cens | cens |
| `deep-win` (`db6a741c723f`) | 6.240 s / 4,574,912 n / mate 22 | 1.670 s / 810,140 n / mate 27 | cens (≥ 1.60) | cens (≥ 1.60) |
| `531810cc7d4f` | cens | cens | cens | cens |
| `8862dbcb70d4` | cens | cens | cens | cens |
| `3a7f66e6b796` | cens | cens | cens | cens |
| `bcc5c3ebe435` | cens | cens | cens | cens |
| `e4cca32930e4` | cens | cens | cens | cens |
| `9da3ab788e86` | cens | cens | cens | cens |
| `5d89e584375f` | cens | cens | cens | cens |
| `eefc6b5c0aa5` | cens | cens | cens | cens |
| `43f66371de39` | cens | cens | cens | cens |

No row was dropped. The 18 hand-censored cells are genuine censoring: provari
hand burned 7.4–8.4 M nodes (~775 kn/s) in 10 s without a proof, and both of
our stacks were still unproven at their own 10 s cut-off (default 66–78 M
child evals, opt 47–51 M).

### 4.3 Shared-win sets and wall factors

| Set | Cells | Per-cell factor | Geomean |
| --- | ---: | --- | ---: |
| `hand × default` | 1 | `easy-win` 51.739 | **51.739** |
| `hand × opt` | 2 | `core` 7.757, `easy-win` 6.045 | **6.848** |
| `model × opt` | 2 | `core` 10.647, `easy-win` 9.451 | **10.031** |
| `model × default` | 1 | `easy-win` 80.887 | **80.887** |

Censored cells with a meaningful bound: `core` under `default` ≥ 20.41;
`deep-win` under both stacks ≥ 1.60 (and, against the model column,
≥ 10 s / 1.670 s = 5.99). The `opt` stack's remaining coverage gap is
`deep-win`, where `bench-v0` needed 60.21 s / 326.9 M child evals at a 120 s
budget (factor 9.70) — not re-probed here.

### 4.4 Hand vs. model column (context only, never the target)

| Cell | hand wall / nodes | model wall / nodes | wall ratio hand/model |
| --- | --- | --- | ---: |
| `core` | 0.490 s / 342,289 | 0.357 s / 169,485 | 1.37 |
| `easy-win` | 0.111 s / 72,523 | 0.071 s / 13,779 | 1.56 |
| `deep-win` | 6.240 s / 4,574,912 | 1.670 s / 810,140 | 3.74 |
| geomean | | | **2.00** |

The model guide proves the same cells with 2.0–5.7× fewer nodes but runs at
~475–485 kn/s versus ~653–733 kn/s for the hand guide (censored cells:
~536 kn/s vs ~775 kn/s, i.e. a **1.45× throughput tax**). That is exactly why
`bench.md` fixes the hand guide as 1.000 and treats the model column as a
recorded stretch column: a node-count comparison would overstate the model's
advantage by roughly the throughput ratio. Model-only wins remain
`b9a7ed7eb368` (4.530 s, matching provari report 1's 4.46 s) and
`dd3f89766a40` (1.853 s vs 1.86 s), reproducing the retired initiative's
numbers and confirming the rebuilt oracle.

### 4.5 Mechanism note (not a speed claim)

On the censored cells the `opt` stack performs 47–51 M child evals against
`default`'s 66–78 M in the same 10 s — a ~1.5× eval reduction with no wall
gain, because neither side finishes. Consistent with `bench-v0`: the levers
cut search work, and the wall follows only where a proof is actually found.

## 5. Bench set re-derivation

Fixed rule: *the bench set is every cell of `core + sample20` where the pinned
provari oracle proves a win with the hand guide inside the canonical budget.*

- Hand wins found: `core`, `1eb80b92f92c`, `db6a741c723f` → **3 cells**.
- New hand wins not already in the set: **none**.
- Cells in the set that are not hand wins: **none** — the expected
  exclusions are confirmed (`b9a7ed7eb368`, `dd3f89766a40` are hand-censored
  at 8.37 M / 7.85 M nodes; the other 16 `sample20` cells are hand-censored
  too). Nothing was removed and nothing was added.
- Consequence: the 3-repeat bench run covered exactly the existing three
  cells; the model column's two extra wins are **not** eligible under the
  rule and stay out of the set.

`bench.md` therefore gained a dated reference note only (no ledger row, no set
change), recording the re-derived hand reference medians and both-guide
coverage: hand 3/21, model 5/21.

## 6. Deviations from the plan

1. **No `INCONCLUSIVE` model column.** The clone, `data/fetch.sh` and the
   bundled build all succeeded, so `model_column.json` was not written and the
   model column was measured over all 21 cells as the plan's primary path.
2. **Bench 3-repeat run covered only the shipped three cells.** The plan
   allowed adding a cell id if the census found a new hand win; none was
   found, so no addition was made.
3. **`derive.py` reads the candidate rows from the hand files.** Our runs do
   not depend on provari's guide, and the hand file carries the designated
   3-repeat rows; the duplicate candidate rows in the model files were left in
   place in the raw results but not used for medians. Documented in the script
   header.
4. **Driver extension is the only non-doc code change** (`--guide`, dynamic
   `provari-<guide>` label), exactly as the plan scoped it. `--dry-run` output
   was verified for both guides and the canonical default shape.

No corruption of `bench-v0`: `measurements/bench/results.json` is unchanged,
and the bundled build reproduces its hand search values.

## 7. Findings

1. **The corpus is thinner than the goal needs.** The hand guide wins 3 of 21
   canonical-budget cells, so the initiative's geometric mean rests on at most
   two shared cells. A 1.0 target measured over two noisy wall ratios will
   need the item-6 decision to state the cell count and consider whether a
   larger hand-win set (longer budget, `--guide none` control, or the
   out-of-scope `high200`) is warranted — all out of scope here.
2. **Both our stacks are coverage-limited, not just slow.** `opt` matches
   provari's hand coverage on `easy-win` and needs 6.0× its wall, needs 7.8×
   on `core`, and does not prove `deep-win` at all. The remaining work is
   mechanism work on the win path — which is item 3's scope, not a tuning
   story.
3. **The model column is a real but bounded gap.** Even against the stretch
   column, `opt` wins the two shared cells (10.03×) and misses the two
   model-only cells; matching it would need a learned artifact this
   initiative's non-goals exclude from the product path.
4. **The oracle is reproducible.** All three retired-initiative numbers we can
   cross-check (342,289 nodes hand `core`; 4.46 → 4.530 s and 1.86 → 1.853 s
   model wins; 13,779 model nodes `easy-win`) reproduce on a third host.

## 8. Not measured (explicit)

- Tablebases of any kind.
- NNUE training or any learned-model work; the model column is context only.
- The full `high200` corpus — only the frozen `core + sample20`.
- The independent checker (`provari check`) and any proof export — item 2.
- `provari --guide none` (the retired plan1 control) and any budget other than
  the canonical 10 s / 192 MB (the 120 s `deep-win` probe from `bench-v0` was
  not repeated).
- No ledger row: plan1 does not change the win path.

## 9. Recommended plan2

**Item 2 — differential proof-checker oracle.** Evidence:

- The census changes nothing about the ranking: the coverage/latency gap is
  ~7× plus one coverage cell, and closing it requires new proof shapes on the
  win path (item 3's attacker-only front-end and item 4's sibling replay).
  Per the initiative's ranking, an independent rules implementation must
  validate those shapes *before* they are trusted — item 2 gates item 3.
- The oracle rebuild just proved the export substrate is stable: the pinned
  commit builds cleanly, its `prove` JSON and its node/mate accounting
  reproduce prior numbers exactly, so `provari check` is available for a
  deterministic differential harness on this host.
- Nothing in this measurement blocks or reshapes item 2; the `bench-v0`
  three-cell set and its reference walls remain the evaluation target for
  anything item 2 or 3 ships.

Alternative if item 2 is deferred: re-cut item 5 (rule50-free key) — but it is
soundness-sensitive and gains nothing until the checker can independently
validate a changed key/replay contract, which is a second argument for doing
item 2 first.

## 10. Deliverables and gate

- `measurements/plan1/`: `README.md` (provenance), `derive.py`,
  `results_hand_census.json`, `results_hand_bench.json`,
  `results_model_census.json`, `results_model_bench.json`, `census.json`,
  `census.md`.
- `measurements/bench/run_bench.py`: `--guide` + dynamic reference label.
- [`bench.md`](bench.md): dated reference note, `bench-v0` row intact, no
  ledger row.
- No `src/`, `examples/`, `tests/`, or `Cargo.*` changes, so `make test` was
  not required; the plan's validation (both scripts parse with `ast`, driver
  `--dry-run` for both guides, every results/census JSON parses) passed.
- No `__pycache__`, no raw transcripts, no provari artifacts in the tree.
