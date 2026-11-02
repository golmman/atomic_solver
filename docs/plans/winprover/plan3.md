# Winprover Plan 3: win-only front-end skeleton

Initiative: `winprover`, backlog **item 3**. Drafted 2026-11-02. This is a
**search** plan: it adds an opt-in attacker-only win prover that proves a win
and otherwise falls back to the existing solver unchanged. It adds **no new
heuristics** (no move-class numbers, quiet grouping, EF-PN, static tactics,
sibling simulation, or rule50-free keying). Its plan session also records the
read-only **item-4 sibling-simulation census**.

Prerequisite reading: [`initiative.md`](initiative.md), [`bench.md`](bench.md),
[`report2.md`](report2.md), [`AGENTS.md`](../../../AGENTS.md), and the retired
[`provari/initiative.md`](../provari/initiative.md) (items 2 and 8) plus
[`provari/report5.md`](../provari/report5.md) §"Next ranked step".

## Why this plan exists

Item 2 put the independent checker in place. Item 3 is the architectural
pivot: provari's advantage is not one heuristic but a **win-only proof
semantics** — an attacker-only AND/OR search that never carries the general
solver's draw/loss/PV/refinement machinery. Our `dfpn` is a general game
solver; every win is proved through the same frames that also prove draws and
losses, and the product path adds a cheap probe, a pre-phase, first-outcome
handling, and iterative PV shortening.

The skeleton isolates the win path so that items 4 (sibling simulation) and 5
(rule50-free key + proof length) have a clean home, and so that the cost of
the general machinery can be measured rather than guessed. It is deliberately
**not** a performance lever: the front-end may prove nothing on the bench
cells, and the flag stays default off regardless (item 6 decides defaults).
The success criterion is correctness and fallback identity, not speed.

The initiative ranks item 3 ahead of item 4 because sibling replay's home is
the attacker-only front-end; implementing it in `dfpn` first risks writing it
twice. The census below prices item 4 before either is implemented.

## Plan-session census (read-only, 2026-11-02)

**Method.** A temporary copy of the current tree (not committed, not part of
the repo) was instrumented with counters at the `dfpn` AND-node child
evaluation. The counters are observational only; no search behaviour changed.
The three bench cells were run at the canonical 10 s / 192 MB budget with
`--first-outcome --outcome-only`, once with the shipped opt-in stack
(`--move-class-pn --quiet-grouping --eval-pn or`) and once with the default
stack. The instrumented copy was deleted after the probe.

Counters:

- `and_nodes` — AND (defender-to-move) frames entered;
- `and_multi` — those with at least two children;
- `and_not_check` / `and_not_check_multi` — those not in check (the
  `replay_threat` precondition), and with at least two children;
- `win_child_events` — a child of an AND node transitioned from open to
  `Win` (i.e. a defence was just refuted);
- `win_child_events_with_open` — those events with at least one other open
  sibling;
- `open_siblings` — the sum of open siblings over those events (an upper
  bound on the first simulation form's opportunities).

**Opt-in stack** (`--move-class-pn --quiet-grouping --eval-pn or`):

| Cell | Outcome | Evals | `and_nodes` | `and_multi` | `and_not_check` | `win_child_events` | with open | `open_siblings` |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `core` | win | 17,217,064 | 518,264 | 510,056 | 472,619 | 148,982 | 141,039 | 2,234,045 |
| `easy-win` | win | 3,231,686 | 98,239 | 97,363 | 93,819 | 18,873 | 17,900 | 281,269 |
| `deep-win` | draw (timeout) | 47,552,244 | 1,373,270 | 1,362,276 | 1,295,639 | 262,771 | 250,535 | 4,068,169 |

**Default stack** (shipped flags):

| Cell | Outcome | Evals | `and_nodes` | `and_multi` | `and_not_check` | `win_child_events` | with open | `open_siblings` |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `core` | draw (timeout) | 72,774,857 | 1,853,974 | 1,849,879 | 1,741,943 | 107,931 | 103,228 | 1,328,681 |
| `easy-win` | win | 43,266,033 | 1,241,699 | 1,236,074 | 1,185,992 | 92,586 | 88,796 | 1,168,334 |

**Reading.** On the opt stack, 98 %+ of AND nodes have at least two children,
and 94–96 % of refuted defences have at least one open sibling; the average
number of open siblings per event is ~15.7 (`easy-win`), ~15.8 (`core`) and
~16.2 (`deep-win`). The default stack shows the same shape. This is a strong
upper-bound signal for the first simulation form (replay a proven defence's
proof on its open siblings). It is **not** a measurement of transfer: the
census does not check whether the sibling's proof is actually the same, and it
does not count the second form (`replay_threat`, which needs a null-move
probe). The numbers are large enough to justify item 4 after the front-end,
but item 4's plan must re-measure on the front-end and must not treat this
census as a speed claim.

**Caveat.** The census is on the current general search, not on the new
front-end. The front-end has no heuristics, so its AND-node shape will differ;
the census is a sizing probe, not a prediction.

## Scope and non-goals

In scope:

- add `src/search/winprover/` with an opt-in `WinProver` front-end;
- add `Search::set_win_front_end(bool)` and the CLI flag
  `--win-front-end` (default off), plus a deterministic child-eval cap
  (`--win-front-end-budget`, default `1_000_000`);
- run the front-end before the general search in
  `Search::solve_with_progress`; a proven win is returned, otherwise the
  general search runs unchanged;
- account the front-end's child evals in the run's counters and against the
  global child-eval budget; respect the wall deadline and stop flag;
- emit `ProofEvent::NodeProven` for every node the front-end proves;
- add unit/integration tests for soundness, fallback identity, budget
  accounting, determinism, and proof-event consumption;
- measure the three bench cells with the flag on/off and append one ledger
  row;
- write `report3.md` and update the initiative/README status.

Out of scope (do not do these here):

- no move-class numbers, quiet grouping, EF-PN, static tactics, sibling
  simulation, rule50-free keying, proof length/limit, or any other new
  heuristic;
- no change to the general `dfpn` search, its TT, its pre-phase, its cheap
  probe, its refinement, or its PV extraction;
- no change to the proof-tree format, the proofdb pipeline, or the exporter;
- no default-flag decision (item 6), no new corpus, no tablebases, no NNUE;
- no root-`Loss`/root-`Draw` front-end claims: the front-end only returns a
  proven **Win** for the side to move; everything else falls back.

## Design

### Module layout

`src/search/winprover/` (new, private to `search`; re-exported as
`search::winprover`):

- `mod.rs` — `WinProver` state, configuration, `WinProof`, and the public
  `solve` entry point;
- `search.rs` — the DF-PN+ win-only loop (OR/AND selection, thresholds,
  terminal/repetition/rule50 handling, TT store/probe, proof events);
- `children.rs` — child evaluation for one frame (legal moves, terminal
  classification, TT reuse, move ordering);
- `tests.rs` — unit tests.

Keep each file under the ~10 KB guideline; split further if needed. The
module is a **simplified** DF-PN, not a copy of `dfpn`: it has no general
outcome/PV/refinement machinery, no pre-phase, no cheap probe, no quiet
grouping, no move-class/EF-PN/static-tactics hooks, and no repetition cache.
It reuses only the shipped, non-lever pieces: `Position`, `Outcome`,
`TranspositionTable`, `StaticAtomicScorer` (for move ordering), the
`fraction` helper (made `pub(crate)` if necessary), and `ProofEvent`.

### Search semantics

The front-end is an attacker-only AND/OR proof-number search:

- **OR node** (attacker to move): `Win` if any child is `Loss` (for the
  child's side); `Loss` if all children are `Win`; `Draw` otherwise.
- **AND node** (defender to move): `Win` if all children are `Win`; `Loss`
  if any child is `Loss`; `Draw` otherwise.
- The root is an OR node for the side to move. The front-end returns
  `Some(WinProof)` only when the root is a proven `Win`; a proven `Loss` or
  `Draw` is **not** a front-end claim and falls back to the general solver.
- Terminal classification uses `Position::outcome()` at node entry. A
  terminal `Win` for the side to move is a win; a terminal `Loss`/`Draw` is
  not a front-end win.
- Repetition: a position whose repetition key is already on the current path
  is a `Draw` (never a win). Rule50: a position with halfmove clock ≥ 100 is
  a `Draw`. These checks happen before expansion and before any TT store.
- The TT stores only path-independent results: solved `Win`/`Loss` outcomes
  and unsolved `(pn, dn)` bounds. A repetition-dependent `Draw` is never
  stored. The TT is a **scratch** table (see below), so the general search's
  TT is untouched.
- Move ordering uses the existing `StaticAtomicScorer` (shipped, not a new
  lever). The front-end adds no ordering heuristic of its own.
- The `1+ε` threshold uses the existing exact-fraction helper with the same
  default ε as `dfpn` (0.125), so the front-end is deterministic.

### Scratch TT and memory

The front-end uses its own `TranspositionTable` of a fixed
`WIN_FRONT_END_TT_MB = 32` MB, dropped when the front-end returns. This is
what makes fallback identity bit-identical: the production TT is never
touched by a failed front-end run. It is a **bounded, documented exception**
to the search CLI's "RAM = TT only" statement; update the CLI help and
`AGENTS.md` to say "RAM = TT only, plus the opt-in win front-end's 32 MB
scratch TT when `--win-front-end` is enabled". The scratch TT is not
serialized into `--tt-dump-path` snapshots.

### Budget, timeout, and fallback

- The front-end's child evals are added to the run's `child_evals` and count
  against `child_eval_budget`. If the budget is exhausted, the front-end
  returns `None` and the general search immediately reports
  `ExitReason::BudgetExhausted` (the same contract as the pre-phase).
- The front-end's deterministic bound is
  `--win-front-end-budget` (default `1_000_000` child evals), further clamped
  to the remaining global `child_eval_budget`. The wall deadline is a safety
  net: the front-end stops at the global deadline and returns `None`.
- On `None`, the general search runs **bit-identically** to a flag-off run:
  same outcome, PV, nodes, main-TT snapshot, and general-search child evals.
  The only difference is the total `child_evals`/`nodes` counters, which
  include the front-end's work. Add accessors
  `win_front_end_evaluations()` and `win_front_end_nodes()` so tests and the
  report can separate the two.
- On `Some(WinProof)`, the run returns the front-end's outcome and PV, sets
  a new `PvStatus::WinFrontEnd` (see below), and does not run the general
  search. The front-end's PV is informational; the proof-tree layer remains
  authoritative.

### Proof events

The front-end emits `ProofEvent::NodeProven` for every node it proves
(`Win` or `Loss`), using the same path semantics as `dfpn`: the path is the
move stack from the root to the node, including the node's incoming move; the
hash is the position hash; the outcome is from the side-to-move perspective
at the node; the depth is the proven depth in plies. The front-end's events
are monotone and valid by construction. If the front-end fails and the
general search re-proves some of the same nodes, duplicate events are
possible; the proof-tree worker already tolerates re-proving, and the plan
adds a test that consumes a front-end event stream and checks every event
replays to the claimed outcome.

### `PvStatus`

Add `PvStatus::WinFrontEnd` with the doc: "PV came from the opt-in win-only
front-end; it is a proven win, but its length is the front-end's proof
length, not necessarily shortest." Update `src/main.rs`'s label mapping to
print `win-front-end`. Do not reuse `ProvenShortest` (the front-end does not
run the refinement rounds that justify it) or `FirstOutcome` (the front-end
is not the general first-outcome phase).

### CLI

Add to `CliOptions`:

- `win_front_end: bool` (default `false`), `--win-front-end`;
- `win_front_end_budget: u64` (default `1_000_000`),
  `--win-front-end-budget <EVALS>`; `0` is rejected (use the flag to disable
  the front-end).

Update `print_help()` and the `AGENTS.md` CLI option list. The flag is
default off; flag-off runs are byte-identical to the current product.

## Tests

Unit tests in `src/search/winprover/tests.rs`:

1. **Mate in one** — a position with an immediate exploding capture returns
   `Some(WinProof)` with a one-move PV; replaying the PV reaches a terminal
   win.
2. **Terminal root** — a checkmated root returns `None` (the general solver
   owns terminal roots); a stalemate root returns `None`.
3. **Repetition is not a win** — a position whose only continuation repeats
   the path returns `None`, not a win.
4. **Rule50 is not a win** — a position at halfmove clock 100 returns
   `None`.
5. **Budget accounting** — a hard position with a tiny
   `win_front_end_budget` returns `None` and reports exactly the spent evals;
   the general fallback then sees the remaining budget.
6. **Determinism** — two runs on the same `(FEN, config, cap)` return the
   same proof/evals/nodes.
7. **Proof events** — a small forced win emits `NodeProven` events whose
   paths replay to the claimed outcomes; a `Vec`-collecting sink is enough
   (no worker needed).
8. **No new heuristics** — the front-end's result is unchanged when the
   general solver's opt-in flags are toggled (the front-end does not read
   them).

Integration tests in `tests/`:

9. **Fallback identity** — on a position the front-end does not prove
   within its cap, run `Search` with the flag on and off; assert the same
   outcome, PV, `nodes - win_front_end_nodes()`, and
   `child_evals - win_front_end_evaluations()`; assert the main TT snapshot
   is byte-identical (the front-end's scratch TT is not serialized).
10. **Proven win** — on a position the front-end proves, assert the returned
    outcome is `Win`, `pv_status() == WinFrontEnd`, and the PV replays to a
    terminal win. If the position is small enough, also export the proof
    events through the existing proof-tree worker and validate the tree.

The tests stay in the default gate (`make test`); no slow tests are needed
for the skeleton. `make test-full` is required because the change is under
`src/search/`.

## Measurement

Run the three bench cells at the canonical 10 s / 192 MB budget with
`--first-outcome --outcome-only`, three repeats per cell, in one session on
one host:

- baseline: the shipped opt-in stack
  (`--move-class-pn --quiet-grouping --eval-pn or`);
- candidate: the same stack plus `--win-front-end`.

Record per cell: outcome, wall median, total `child_evals`, front-end evals,
front-end nodes, and whether the front-end proved the cell. Append **one**
ledger row to [`bench.md`](bench.md) with the candidate stack, per-cell
factors (or `censored` + bound), the canonical geometric mean, coverage, and
the deterministic evals. The row is read with the `Stack` column: the
front-end is opt-in and is not a product win until item 6.

Pre-registered expectation: the skeleton is **not** expected to beat the
baseline. A front-end that proves nothing and adds its cap to the total evals
is a valid neutral result; the report must state the overhead and recommend
item 4 (or a pivot) on the evidence. A front-end that proves a bench cell
within its cap is a positive signal for item 4, not a product win.

## Tasks

1. **Read and confirm.** Re-read this plan, `initiative.md`, `bench.md`,
   `report2.md`, `AGENTS.md`, and the `provari` item-2/item-8 notes. Confirm
   the no-new-heuristics boundary and the scratch-TT fallback contract.
2. **Record the census.** The plan-session census is already in this plan;
   do not re-run it unless the tree has changed. If it has, re-run the
   read-only probe and update the table with a dated note.
3. **Implement `src/search/winprover/`** per the design: state, DF-PN+ loop,
   child evaluation, terminal/repetition/rule50 handling, scratch TT, proof
   events, and the `WinProof`/`PvStatus::WinFrontEnd` surface.
4. **Wire the flag.** Add `Search::set_win_front_end`,
   `set_win_front_end_budget`, the accessors, the `CliOptions` fields, the
   parser, and `print_help()`. Wire the front-end into
   `Search::solve_with_progress` before the cheap probe/pre-phase. Do not
   wire it into `search_depth`/`search_depth_with_prefix` (the offline
   certificate-bound entry points stay unchanged); document that choice.
5. **Update `AGENTS.md`**: the `src/search/` description, the CLI option
   list, the `PvStatus` list, and the "RAM = TT only" exception.
6. **Add the tests** from the Tests section. Run `cargo fmt`,
   `cargo clippy --all-targets -- -D warnings`, `make test`, and
   `cargo doc --no-deps`. Run `make test-full` because the change is under
   `src/search/`.
7. **Measure** the three bench cells per the Measurement section; write the
   driver/results under `measurements/plan3/` (provenance README, driver,
   parsed JSON/CSV; no raw transcripts, no `__pycache__`).
8. **Append the ledger row** to `bench.md` (one row, candidate stack,
   per-cell factors/censoring, geomean, coverage, evals).
9. **Write `report3.md`** (final task). Include: provenance and exact
   commands; the front-end design and its deviations; the census table and
   its limitations; the test results; the bench table (baseline vs
   candidate, per cell, with front-end evals/nodes); the ledger row; the
   gate results; the verdict (GO/neutral/negative for the skeleton) and the
   recommendation for item 4 (with the census evidence) or a pivot; and the
   explicit "not measured" list.
10. **Update `initiative.md`** (item 3 done with a one-line summary, item 4
    next or re-ranked, dated history entry) and the `docs/plans/README.md`
    winprover row.
11. **Clean up.** No `__pycache__`, no raw logs, no TT snapshots, no
    temporary instrumented copy, no provari artifacts. Confirm with
    `git status --short` and `git add --dry-run` that only the intended
    files would be added.

## Acceptance

- The front-end is implemented, opt-in, default off, deterministic, and
  adds no new heuristic.
- Flag-off behavior is byte-identical to the current product (all existing
  tests pass; the fallback-identity test passes).
- A position the front-end proves returns a `Win` with a replay-validated
  PV and `PvStatus::WinFrontEnd`; a position it does not prove falls back to
  a general search that is bit-identical to flag-off (same outcome, PV,
  nodes, main-TT snapshot, and general-search evals).
- The front-end's evals are counted in `child_evaluations()`, respect
  `child_eval_budget`, and are separable via the new accessors.
- The front-end emits `ProofEvent`s for proven nodes; a test consumes them
  and replays every event.
- The item-4 census is recorded with its method and limitations; the report
  recommends item 4 or a pivot on that evidence.
- The bench measurement and one ledger row are recorded; the report states
  the skeleton's verdict honestly (a neutral result is valid).
- `make test` and `make test-full` are green; `cargo fmt`, `cargo clippy
  --all-targets`, and `cargo doc --no-deps` are clean.
- `initiative.md` and the `docs/plans/README.md` row are updated.
- `git status --short` shows only the intended source, test, doc, and
  measurement files; no temporary instrumented copy, raw log, TT snapshot,
  or `__pycache__` is staged.

## Validation

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
make test
make test-full
cargo doc --no-deps
cargo build --release
python3 - <<'PY'
import ast
from pathlib import Path
for p in Path("docs/plans/winprover/measurements/plan3").glob("*.py"):
    ast.parse(p.read_text())
PY
```

Do not use `py_compile` (it writes `__pycache__`). Confirm every committed
JSON/CSV parses with the standard library.

## Risks and mitigations

- **The front-end proves nothing on the bench cells.** That is a valid
  neutral result; the cap bounds the overhead, the fallback is unchanged,
  and the skeleton is still the home for item 4. The report must say so.
- **Scratch-TT memory.** The 32 MB scratch TT is a bounded exception to
  "RAM = TT only"; document it in the CLI help and `AGENTS.md`. It is
  dropped after the front-end returns and is never serialized.
- **Fallback identity is subtle.** The front-end must not touch the
  production TT, the pre-phase, the cheap probe, the repetition cache, or
  the general search's counters beyond adding its own evals/nodes. The
  integration test compares the general search's own work and the main TT
  snapshot.
- **Repetition/rule50 soundness.** The front-end must treat a repeated
  position and the 50-move limit as draws, never wins. The tests pin both.
- **Proof-event duplicates.** A failed front-end may emit events the general
  search later re-proves. The proof-tree worker already tolerates
  re-proving; the test consumes a front-end stream and checks every event.
- **PV length is not shortest.** `PvStatus::WinFrontEnd` is honest about
  this; do not reuse `ProvenShortest`.
- **The census is a proxy.** It counts open siblings, not proof transfer,
  and it is on the general search, not the front-end. Item 4's plan must
  re-measure on the front-end and must not treat the census as a speed
  claim.
- **Scope creep.** The front-end is a skeleton. Do not add sibling
  simulation, rule50-free keying, or any other lever here; those are items 4
  and 5.

## Report requirements

`report3.md` is the final task. It must include:

- the provenance table (date, host, solver commit, budgets, front-end cap,
  scratch-TT size);
- the exact commands used;
- the front-end design and any deviations from this plan;
- the census table and its limitations;
- the test results (unit + integration, including fallback identity and
  proof-event replay);
- the bench table (baseline vs candidate, per cell, with front-end
  evals/nodes and whether the front-end proved the cell);
- the ledger row and the canonical geomean/coverage;
- the gate results (`make test`, `make test-full`, fmt, clippy, doc);
- the verdict (GO/neutral/negative for the skeleton) and the recommendation
  for item 4 (with the census evidence) or a pivot;
- explicit "not measured" items (sibling simulation, rule50-free keying,
  proof length/limit, default-flag decision, new corpus, tablebases, NNUE,
  root-`Loss`/root-`Draw` front-end claims).

## Deliverables

- `src/search/winprover/` (new module: `mod.rs`, `search.rs`,
  `children.rs`, `tests.rs`).
- `src/search/mod.rs` (`pub mod winprover;`).
- `src/search/dfpn/mod.rs` (`PvStatus::WinFrontEnd`), `config.rs` (setters
  and accessors), `run.rs` (front-end hook).
- `src/cli.rs` (new options and parser tests), `src/main.rs` (help text and
  `pv_status` label).
- `AGENTS.md` (module description, CLI options, `PvStatus`, RAM exception).
- `tests/` (fallback identity and proven-win integration tests).
- `docs/plans/winprover/measurements/plan3/` (provenance README, driver,
  parsed results).
- `docs/plans/winprover/report3.md`.
- `docs/plans/winprover/bench.md` (one ledger row).
- `docs/plans/winprover/initiative.md` and `docs/plans/README.md` status
  updates.
