# Winprover Report 5 — pivot disposition and head-to-head close-out (goal not met)

Initiative: `winprover`, backlog **item 6** (head-to-head convergence), with
the report3 §7 candidate/verification pivot and item 5 (rule50-free key)
disposed of first, executed as [`plan5.md`](plan5.md). Date: **2026-11-02**
(UTC). This is a **decision/close-out** session, not a search change: no
product code was modified. Verdict: **the initiative's success condition is
not met**; the search thread is closed, the opt-in front-end stays opt-in and
default-off, and the only measured path forward is a new from-scratch
provari-parity attacker-only prover initiative.

## Headline

- **Default decision: keep the shipped defaults.** The pre-registered
  quick-suite rule rejected promoting the opt stack
  (`--move-class-pn --quiet-grouping --eval-pn or`): it turns three `Loss`
  quick-suite cases (`dec06`, `dec42`, `dec44`) into 3 s draws. No CLI,
  `Search`, help, golden, or test change was made.
- **Final head-to-head** (canonical 10 s / 192 MB, provari `--guide hand` =
  1.000): product default **44.061** (1 shared cell, coverage 1/3); best
  measured opt stack **6.301** (2 shared cells, coverage 2/3); both censored
  on `deep-win`; provari hand coverage 3/3 in the same session. Goal
  (geomean `< 1.0`, coverage ≥ provari) **not met**.
- The three plan-session probes are recorded as measured-out: the
  candidate/verification pivot has no viable candidate source at an
  affordable cap, the rule50-free key buys ~0 %–0.7 % and no coverage, and the
  front-end with the opt starting-number levers is ~2.2× the general solver's
  work (4× its wall) on `core` and still cannot prove `easy-win` at 50 M.
- **Item 5 closed** with the key half measured-out; the
  proof-length/holding-clock half is closed with it rather than implemented
  speculatively. **Item 6 done.** Initiative **closed**; items 7 (optional
  packaging) and 8 (evidence horizon) carried to the recommended successor.
- Gate: `make test` and `make test-full` green, `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo doc --no-deps`, and
  `cargo build --release` clean. One ledger row appended.

## 1. Provenance

| Item | Value |
| --- | --- |
| Date | 2026-11-02 (UTC) |
| Host | container `67db7a1089e2`; aarch64, 8 CPU; `Linux-7.1.13-402.asahi.fc44.aarch64+16k`; glibc 2.43 |
| `atomic_solver` | `35f78bfc6d1da7baff368f52bc2d9375c45d62c1`, clean tree (no product code changed) |
| Solver binary | `./target/release/atomic_solver` |
| Provari | `aiorla/provari` commit `48686bc6c786d6ea70568f0b2ad4002cf0520206` (`v0.1.0`), `--guide hand` |
| Provari binary | `/tmp/winprover-oracle/target/release/provari` (scratch clone; **not** committed) |
| Corpus | [`../provari/measurements/plan1/corpus.json`](../provari/measurements/plan1/corpus.json), sha256 `9cd0169a…47d0e` |
| Bench cells | `core`, `easy-win` (`1eb80b92f92c`), `deep-win` (`db6a741c723f`) |
| Bench budget | 10 s / 192 MB label, 3 repeats per cell per side |
| Quick suite | 59 cases (`decisive_positions.txt` 46 + `move_order_positions.txt` `mNN` with `NN >= 23`, 13); `--timeout 3 --first-outcome`, 1 run per case per stack |
| Tablebases / NNUE | none |

The plan-session probes (§4) ran on a temporary copy of commit `5e8a1bb` +
the plan4 working tree. The copy was deleted; no probe code is committed. They
are **sizing evidence**, not product results.

## 2. Exact commands

```sh
# build
cargo build --release

# full measurement record: quick suite + canonical bench (~3 min)
python3 docs/plans/winprover/measurements/plan5/run_plan5.py \
  --provari /tmp/winprover-oracle/target/release/provari \
  --solver ./target/release/atomic_solver

# sections independently / re-derive results.csv
python3 docs/plans/winprover/measurements/plan5/run_plan5.py ... --skip-bench
python3 docs/plans/winprover/measurements/plan5/run_plan5.py ... --skip-quick
python3 docs/plans/winprover/measurements/plan5/run_plan5.py ... \
  --skip-quick --skip-bench

# gates
cargo fmt --check
cargo clippy --all-targets -- -D warnings
make test
make test-full
cargo doc --no-deps
cargo build --release
python3 - <<'PY'
import ast
from pathlib import Path
for p in Path("docs/plans/winprover/measurements/plan5").glob("*.py"):
    ast.parse(p.read_text())
PY
```

The plan5 driver delegates the canonical bench to the committed
[`measurements/bench/run_bench.py`](measurements/bench/run_bench.py), so the
row uses the same corpus, budget, repeats, and oracle as earlier rows.

## 3. Quick-suite comparison and the default decision

The pre-registered rule (plan5 §Design) is: measure the old default and the opt
stack on the 59-case quick suite at its canonical settings; promote only if
there is **no outcome regression**.

| Stack | Correct | Draws (censored at 3 s) | Wrong | Total child evals |
| --- | ---: | ---: | ---: | ---: |
| `default` (shipped) | **59/59** | 0 | 0 | **42,109,768** |
| `opt` (`--move-class-pn --quiet-grouping --eval-pn or`) | 56/59 | 3 | 0 | 74,298,511 |

Regressions (all `Loss` → `draw` at the 3 s cap):

| Case | Expected | `default` | `opt` |
| --- | --- | --- | --- |
| `dec06` | Loss | loss, 1,507,482 evals, 0.188 s | draw (timeout), 18,749,994 evals, 3.003 s |
| `dec42` | Loss | loss, 80,849 evals, 0.016 s | draw (timeout), 11,400,625 evals, 3.006 s |
| `dec44` | Loss | loss, 102,615 evals, 0.025 s | draw (timeout), 19,874,241 evals, 3.007 s |

Reading: the opt stack is genuinely better on most cases (fewer evals on
41/59), but on these three it burns the full budget where the default solves
them in 16–188 ms. That is a real outcome regression at the product budget, so
**the promotion branch is not taken**: the shipped defaults (all four
experimental levers off) stay the product configuration, and the opt stack
stays the best measured opt-in stack. No `--no-move-class-pn` /
`--no-quiet-grouping` flags, help change, golden fixture, or test re-baseline is
needed.

Sanity anchor: the default stack's quick-suite total (**42,109,768** child
evals) reproduces the unbudgeted quick-suite total recorded in conversion
report12, which validates the CLI-based driver against the canonical
harness. (The canonical harness cannot set the search flags, which is why the
driver runs the product CLI directly.)

## 4. Plan-session probes (carried from plan5, measured-out)

All probes ran on a temporary copy of `5e8a1bb` + the plan4 working tree; they
are read-only sizing evidence and were deleted afterwards. See plan5 §Plan
session probes for the full protocol.

### 4.1 Candidate/verification pivot (report3 §7)

| Probe | Cell | Cap | Result |
| --- | --- | ---: | --- |
| Candidate line from a bounded general search | `core` | 1,000,000 | 8-move best-move chain; not a winning line |
| Hard-guide front-end | `core` | 10,000,000 | not proved; 10,000,017 evals, 2,126,387 frames, 4.256 s |
| Root-move verification | `core` | 1,000,000 per move | **0/41** root moves proved; 41,000,908 evals, 15.382 s |
| Root-move verification | `easy-win` | 1,000,000 per move | **0/38** root moves proved; 38,000,943 evals, 13.027 s |

The available candidate sources do not produce a line the front-end can
verify at a cap that fits the product budget. Root-move verification is the
strongest form (the root move is fixed, so only one child must be proved) and
it still fails on every root move at 1 M evals.

**Limitation:** the probe used a temporary hard-guide hack and a root-move
loop, not a faithful candidate/verification layer. The negative result is
about the **available candidate sources** and the front-end's verification
power at these caps, not a proof that no candidate/verification design can
ever work.

### 4.2 Rule50-free key (item 5, key half)

The prototype made `zobrist::rule50_key` return `0`, i.e. the board-only key.
This is the **unsound upper bound** of the lever (it reuses proofs across
clocks without the proof-length/holding-clock check); a sound implementation
cannot do better on the same cells.

| Cell | Baseline evals | Rule50-free evals | Outcome |
| --- | ---: | ---: | --- |
| `core` | 13,845,432 | 13,845,432 | win, identical |
| `easy-win` | 2,697,438 | 2,697,438 | win, identical |
| `deep-win` | 49,089,691 | 48,744,931 | draw (censored), ~0.7 % fewer evals |

The key half is measured-out: no measurable TT-miss cost on the two shared
cells and ~0.7 % on the censored cell without changing coverage. The
proof-length/holding-clock half is not separately measured; its only measured
motivation was the key, so item 5 closes with it.

**Limitation:** the prototype is deliberately unsound; this bounds the
lever's upside, not a candidate implementation.

### 4.3 Front-end heuristic parity

The front-end was given the shipped opt starting-number levers
(`move_class_pn` + `eval_pn = Or`) in the temporary copy.

| Cell | Cap | `off` | `mc + ep` |
| --- | ---: | --- | --- |
| `core` | 10,000,000 | not proved, 10,000,034 evals | not proved, 10,000,005 evals |
| `easy-win` | 10,000,000 | not proved, 10,000,032 evals | not proved, 10,000,027 evals |
| `core` | 50,000,000 | not proved, 50,000,024 evals | **proved**, 30,851,728 evals, 15.042 s, depth 27 |
| `easy-win` | 50,000,000 | not proved, 50,000,020 evals | not proved, 50,000,027 evals |

The levers change the search shape but do not make the front-end competitive:
on `core` it is proved only at ~30.9 M evals / ~15 s versus the general
solver's 13.8 M evals / ~3.8 s (2.2× the work, 4× the wall), and `easy-win` is
not proved at 50 M. The front-end's per-eval cost and search efficiency, not
its starting numbers, are the bottleneck. The front-end thread is **closed**
(the code stays, opt-in, default-off, gates green).

**Limitation:** the prototype is a temporary port, not a faithful integration;
it is sizing evidence for closing the thread.

## 5. Canonical head-to-head

Reference (provari `--guide hand`, medians, same session): `core` 0.499 s ·
342,289 nodes · mate 17; `easy-win` 0.131 s · 72,523 nodes · mate 15;
`deep-win` 6.297 s · 4,574,912 nodes · mate 22. Factor = our wall / reference
wall; the reference is **1.000** in every cell.

| Cell | `default` (chosen) | `opt` (best measured, not promoted) |
| --- | --- | --- |
| `core` | censored (≥ 20.04) 10.011 s / 72,189,193 evals | **7.615** (3.800 s / 17,217,064 evals) |
| `easy-win` | **44.061** (5.772 s / 43,266,033 evals) | **5.214** (0.683 s / 3,231,686 evals) |
| `deep-win` | censored (≥ 1.59) 10.012 s / 74,317,419 evals | censored (≥ 1.59) 10.011 s / 47,552,244 evals |

| Stack | Geomean (canonical) | Coverage | Shared cells |
| --- | ---: | ---: | --- |
| `default` (chosen product) | **44.061** | 1/3 | `easy-win` |
| `opt` (best measured) | **6.301** | 2/3 | `core`, `easy-win` |
| provari hand (reference) | 1.000 | 3/3 | all |

The opt stack's deterministic evals equal the plan3/plan4 control exactly on
`core` (17,217,064) and `easy-win` (3,231,686), so this row is consistent with
the earlier measurements; the reference walls are this session's own (bench
reading rule 1: factors are same-session ratios). The `easy-win` reference wall
is 0.131 s here versus 0.113 s in plan4 — a same-host, same-session drift
worth noting, not a lever effect. Likewise the product default's 44.06 must
not be read against `bench-v0`'s 51.47 as a speed win: the code is unchanged,
our `easy-win` wall moved 5.713 → 5.772 s (+1 %), and the reference wall moved
0.111 → 0.131 s.

The optional `opt+win` front-end control was **not** measured: the front-end
thread is closed and it proved nothing on these cells at 1 M or 10 M evals in
plan3/plan4, so it would only add overhead.

### Ledger row

Appended to [`bench.md`](bench.md) §`plan5`:

| Plan | Stack | `core` | `easy-win` | `deep-win` | Geomean (canonical) | Coverage |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `plan5` | **default** (shipped; opt not promoted) | censored (≥ 20.04) 10.011 s / 72.2 M | **44.06** (5.772 s / 43.3 M) | censored (≥ 1.59) 10.012 s / 74.3 M | **44.06** (1 cell) | 1/3 |

One row, per the plan: it records the **chosen default** (the item-6 decision
kept the shipped flags). The same-session opt control is recorded in the
per-plan summary immediately below the table, not as a second ledger row.

## 6. Gate results

| Gate | Result |
| --- | --- |
| `cargo fmt --check` | clean |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `make test` | green (all unit + fast integration tests) |
| `make test-full` | green (12 m 34 s, exit 0; 38 `test result: ok` groups, no failures) |
| `cargo doc --no-deps` | clean |
| `cargo build --release` | clean |
| plan5 Python parses (`ast.parse`) | clean |

Because the default was not promoted, there are no product-code changes and no
golden/trajectory re-baselines. The quick-suite comparison is the regression
gate that blocked promotion; no test expectation changed.

## 7. Verdict

**Goal not met; search thread closed.** The initiative's success condition —
geometric-mean wall `< 1.0` on the shared-win set with coverage at least
provari's — is false by a wide margin:

- the product default is **44.06×** provari on its one shared cell and is
  censored on `core` and `deep-win` (coverage 1/3);
- the best measured opt-in stack is **6.30×** on two shared cells and is
  censored on `deep-win` (coverage 2/3);
- provari hand is 3/3 in the same session.

The three plan5 probes confirm there is no small remaining lever: the
candidate/verification pivot has no affordable candidate source, the
rule50-free key is a non-factor, and front-end heuristic parity leaves the
front-end 2.2× the general solver's work (4× its wall) on `core`. The gap is
architectural.

Dispositions: item 4 first form shipped, thread closed; item 5 closed
measured-out (key half, proof-length half closed with it); item 6 done with
the default decision and the honest head-to-head. The front-end stays in the
tree, opt-in and default-off. The initiative is **closed**; items 7 (optional
packaging) and 8 (evidence horizon) are carried to the recommended successor.

## 8. Not measured

- the second-form sibling replay (`replay_threat`) and any further
  sibling-simulation lever (plan4's PIVOT closes them);
- the proof-length/holding-clock half of item 5 (closed with the key, not
  implemented);
- a new candidate generator for the candidate/verification role (a new
  mechanism; the initiative's non-goals exclude a learned/model candidate);
- the optional `opt+win` front-end control in the final head-to-head;
- a new corpus, a new bench cell, tablebases, NNUE;
- cross-host repeat of the head-to-head (all factors are same-session
  ratios).

## 9. Next direction

**A from-scratch provari-parity attacker-only prover — a new initiative, not a
winprover plan.** The measured evidence:

- provari hand proves the three bench cells with a hand-evaluation search at
  0.499 / 0.131 / 6.297 s; our best general stack is 3.8× / 5.2× / censored;
- the opt-in win front-end is the right *shape* (attacker-only, win-only,
  replay-validated) but its child evaluation is a full `Position::outcome()`
  movegen per child and its search efficiency is far behind: even with the opt
  starting numbers it needs ~30.9 M evals / 15 s for `core` (the general
  solver: 13.8 M / 3.8 s) and does not prove `easy-win` at 50 M;
- sibling replay works (12–27 % of attempts transfer a proof and cut frames
  ~20–24 %) but does not close a root at a product cap.

A parity prover therefore needs the full opt stack **and** an optimized child
evaluation / provari-style `Mover`/effect precomputation, built around the
attacker-only semantics from the start rather than bolted onto the general
search. That is the only measured path to the `< 1.0` geometric-mean goal.

Items 7 (optional crate promotion / workspace split — its draft `plan1.md` is
stale) and 8 (root-`Loss` export, compact `.pvr`, portable attacker-only
export, external corpus, native second rules implementation, cannot-win
material) move to that successor's backlog.

## 10. Problems and findings

- **Bench-driver parse gap (pre-existing, unchanged):** `run_bench.py` looks
  for `exit_reason=` in `pre_exit:`, but the CLI prints `reason=`, so its
  candidate rows have `exit = None`. The plan5 quick-suite driver parses the
  real names (and the bare `timeout` line) and is unaffected; the ledger
  fields used here (outcome, wall, evals) are unaffected. Fixing the shared
  driver was out of plan5 scope.
- **Quick-suite comparison cannot use the benchmark harness** for the opt
  stack: `examples/benchmark.rs` exposes no search-flag switches, so the
  driver runs the product CLI. The default total matches the canonical
  harness anchor exactly (42,109,768), which is the cross-check.
- **The opt stack is not uniformly worse.** It uses fewer evals on 41/59
  quick cases; the three loss→draw regressions alone invert the intended
  promotion. This is a budget-tail effect at the 3 s canonical suite budget,
  not a claim that the levers hurt the bench cells (they clearly help there,
  6.30× vs 44.06×).
- **Same-host reference drift** (`easy-win` 0.113 → 0.131 s between plan4 and
  plan5) is recorded rather than compared across rows.
- No product code, test, or golden fixture changed. No `__pycache__`, raw
  transcript, TT snapshot, or provari artifact is committed.
- **Missing tests:** none required by this plan. The plan's test section is
  conditional on promotion; the default decision took the non-promotion branch,
  so the quick-suite comparison (recorded in `results.json`) is the regression
  gate and the existing `make test` / `make test-full` suites stay authoritative.

## 11. Delivered files

- [`plan5.md`](plan5.md) (the plan);
- [`measurements/plan5/`](measurements/plan5/README.md): `run_plan5.py`,
  `results.json` (quick suite + bench), `results.csv` (bench cells),
  `README.md` (provenance);
- [`bench.md`](bench.md): one ledger row, per-plan summary, history entry;
- [`initiative.md`](initiative.md): items 4–6 closed, status **closed**, next
  direction, history;
- [`docs/plans/README.md`](../README.md): winprover moved to the Closed
  table with its successor note;
- this report.
