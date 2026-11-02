# Winprover Plan 4: sibling proof replay (first form) with a front-end gate

Initiative: `winprover`, backlog **item 4**. Drafted 2026-11-02. This is a
**search** plan: it adds the first form of provari's sibling simulation
(Kawano's replay) to the opt-in win-only front-end from
[`plan3.md`](plan3.md). It adds **no general-search heuristic** and does not
change the general `dfpn` solver. Its plan session records a read-only
front-end census and a temporary replay prototype; the execution session
implements the faithful first form and applies a **pre-registered gate**:
if the front-end with simulation still proves neither `core` nor `easy-win`
at a 10,000,000 child-eval cap, the verdict is **PIVOT** and no further
sibling-simulation levers are stacked.

Prerequisite reading: [`initiative.md`](initiative.md), [`bench.md`](bench.md),
[`report3.md`](report3.md), [`plan3.md`](plan3.md), [`AGENTS.md`](../../../AGENTS.md),
and the retired [`provari/initiative.md`](../provari/initiative.md) item 8 plus
[`provari/report5.md`](../provari/report5.md) §"Next ranked step".

## Why this plan exists

Item 3 built the attacker-only front-end and measured it **neutral**: it is
correct, deterministic, fallback-identical, and default off, but it proved
none of the three bench cells inside its 1 M-eval cap and added ~0.34–0.37 s
per cell. [`report3.md`](report3.md) §7 therefore recommends item 4 **with a
gate**, not as an open-ended lever:

1. re-measure the item-4 census on the **front-end**, not the general search;
2. if sibling simulation plus the front-end still proves 0 of
   `core`/`easy-win` at a stated cap, pivot instead of stacking more levers.

This plan does exactly that. The first form is the one the item-3 census
sized: when a defence is refuted, replay its proof on the other open defences
of the same AND node. The second form (`replay_threat`, a null-move probe
plus static tactics) is **out of scope** here: it needs a null move and the
static-tactics closure, which is a separate mechanism. Item 4 is therefore
partially addressed by this plan; the second form stays open (or moves to the
horizon) and is not a fallback for a failed gate.

## Plan-session probes (read-only, 2026-11-02)

Two temporary probes were run on a copy of the current tree (commit
`6b0ea24` + plan3 working tree). The copy was deleted after the probe; no
probe code is committed. Both probes are **sizing evidence**, not the final
implementation.

### Front-end census (simulation off)

The front-end was instrumented with observation-only counters at the AND-node
child evaluation, exactly as the item-3 census was, but on the front-end
instead of the general search. Counters: `and_nodes`, `and_multi`,
`win_child_events` (a child of an AND node transitioned from open to `Win`,
i.e. a defence was refuted), `with_open` (those with at least one open
sibling), and `open_siblings` (the sum of open siblings over those events).

| Cell | Cap | Evals | `and_nodes` | `and_multi` | `win_child_events` | with open | `open_siblings` |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `core` | 1 M | 1,000,014 | 21,054 | 21,038 | 1,054 | 1,024 | 13,331 |
| `easy-win` | 1 M | 1,000,027 | 20,045 | 20,039 | 1,736 | 1,715 | 20,914 |
| `core` | 10 M | 10,000,043 | 204,541 | 204,283 | 12,945 | 12,575 | 174,795 |
| `easy-win` | 10 M | 10,000,023 | 185,553 | 185,398 | 18,504 | 18,152 | 273,479 |

Reading: the front-end **does** reach refuted-defence states even at the
shipped 1 M cap (1,054 / 1,736 events), and 97–99 % of them have open
siblings. The item-3 census's shape survives the front-end pivot. This is a
necessary condition for the first form, not a measurement of proof transfer.

### Naive replay prototype (simulation on)

A temporary first-form replay was then added to the same copy: on a refuted
defence, replay the guide's TT `best_move` chain on each open sibling, with a
16-position replay budget, a repetition line, and a one-ply immediate-win
check for defences that lose at once. It is **not** the faithful
implementation (it does not reconstruct the guide's full proof tree and does
not emit proof events), but it is enough to size the transfer.

| Cell | Cap | Sim attempts | Sim successes | Sim evals | Proved? |
| --- | ---: | ---: | ---: | ---: | --- |
| `core` | 1 M | 3,682 | 433 | 16,609 | no |
| `easy-win` | 1 M | 4,583 | 641 | 30,386 | no |
| `core` | 10 M | 42,798 | 4,482 | 309,738 | no |
| `easy-win` | 10 M | 62,670 | 7,939 | 408,629 | no |

Reading: replay succeeds on ~10–14 % of attempts, so the mechanism is not
vacuous, but it does **not** prove either cell at 1 M or 10 M. This is a
negative sizing signal for the naive form. The execution plan must implement
the faithful first form (full guide proof tree, immediate-win shortcut,
`simulated` flag, proof events) and re-measure; the gate below is
pre-registered so the result is not reinterpreted after the fact.

## Scope and non-goals

In scope:

- add `src/search/winprover/simulation.rs` with the first-form replay;
- trigger it at AND nodes when a defence is refuted, replaying on open
  siblings that have not already been simulated;
- add a simulation-local immediate-win check (one-ply explosion/mate) used
  only by the replay, not by the search;
- charge simulation work to the front-end's deterministic child-eval cap and
  the run's global budget; add `simulation_attempts()`,
  `simulation_successes()`, and `simulation_evaluations()` accessors;
- emit `ProofEvent::NodeProven` for every node the replay newly proves, with
  the correct path;
- add `WinProver::set_simulation(bool)` (default `true`),
  `Search::set_win_front_end_simulation(bool)`, and the CLI flag
  `--no-win-front-end-simulation` (default: simulation on when the front-end
  is enabled);
- add unit/integration tests for transfer, repetition/rule50 soundness,
  budget accounting, determinism, proof-event replay, and fallback identity;
- measure the three bench cells with the front-end simulation on and off,
  apply the gate, append one ledger row, and write `report4.md`.

Out of scope (do not do these here):

- **no second form** (`replay_threat`): no null move, no static-tactics
  closure, no "parried threat halves the proof number" rule;
- no move-class numbers, quiet grouping, EF-PN, static tactics in the
  **search**, history/killers, pre-phase, cheap probe, refinement, or
  repetition cache;
- no rule50-free key, proof length/limit, or proof-tree format change;
- no change to the general `dfpn` search, its TT, its pre-phase, its cheap
  probe, its refinement, or its PV extraction;
- no default-flag decision (item 6), no new corpus, no tablebases, no NNUE;
- no root-`Loss`/root-`Draw` front-end claims: the front-end still only
  returns a proven **Win** for the side to move; everything else falls back.

## Design

### Module layout

`src/search/winprover/` gains:

- `simulation.rs` — the first-form replay, the simulation budget, the
  immediate-win helper, and the simulation stats;
- `mod.rs` — the `simulation_enabled` flag, the stats fields/accessors, and
  the `set_simulation` setter;
- `search.rs` — the AND-node trigger and the `simulated` flag preservation;
- `children.rs` — the `simulated: bool` field on `ChildInfo`;
- `tests.rs` — the new unit tests (or a `simulation/tests.rs` submodule if
  the file grows past the ~10 KB guideline).

Keep each file under the ~10 KB guideline; split further if needed. The
simulation reuses only `Position`, `Outcome`, `MoveList`, the scratch
`TranspositionTable`, and the existing `ProofEvent` path.

### Trigger and guide

In `WinProver::dfpn`, at an AND node (`is_or_node == false`), after a child
is re-evaluated and its outcome becomes `Some(Win)` (the defence is
refuted), replay that child's proof on every other open child that has not
already been simulated. This mirrors provari's
`if !attacking && value.numbers.proof == 0` block.

The **guide** is the just-proven child position (the refuted defence). Its
proof is in the scratch TT: the guide's entry has `outcome == Some(Win)` and
a `best_move` (the attacker's winning move). The guide's proof tree is
reconstructed by probing the TT for each child position; no separate proof
table is needed because the front-end stores every proven node's outcome.
The guide is played forward in lockstep with the sibling position.

### Simulation recursion

`simulate_attacker(pos, guide, budget, line, path) -> Option<SimulatedProof>`:

1. If `pos.outcome()` is `Some(Win)`, return depth 0 (terminal win).
2. If `pos.outcome()` is `Some(Draw | Loss)`, return `None`.
3. If `pos.repetition_key()` is on the search path or the simulation line,
   return `None` (a repetition is a Draw, never a win).
4. If the scratch TT has a solved `Win` for `pos`, return its depth and
   `best_move`.
5. If the replay budget is exhausted, return `None`.
6. Charge one child eval to the front-end cap.
7. Get the guide's winning move: the guide's TT `best_move` if the guide is
   solved `Win`; otherwise, if the guide is terminal `Win`, find an
   immediate winning move in `pos` (the one-ply helper) and use it. If no
   move is available, return `None`.
8. Check the move is legal in `pos`; play it in both `pos` and `guide`;
   recurse into `simulate_defender`.
9. Return depth = 1 + defender depth, `best_move` = the move.

`simulate_defender(pos, guide, budget, line, path) -> Option<u32>`:

1. If `pos.outcome()` is `Some(Loss)`, return depth 0 (the defender is
   already lost).
2. If `pos.outcome()` is `Some(Draw | Win)`, return `None`.
3. If `pos.repetition_key()` is on the search path or the simulation line,
   return `None`.
4. If the scratch TT has a solved `Loss` for `pos`, return its depth.
5. If the replay budget is exhausted, return `None`.
6. Charge one child eval.
7. For every legal defence `reply` in `pos`:
   - play `reply` to get `child`;
   - if `child.outcome()` is `Some(Win)` (the attacker to move wins),
     accept with depth 0;
   - else if `child` has an immediate winning move for the attacker (the
     one-ply helper), accept with depth 1;
   - else if the TT has a solved `Win` for `child`, accept with its depth;
   - else if `reply` is legal in `guide`, play it in `guide` and recurse
     `simulate_attacker(child, guide_child, ...)`; accept with the returned
     depth;
   - else fail (return `None`).
8. Return depth = 1 + max over accepted defence depths.

The simulation line is a `Vec<u64>` of repetition keys, pushed/popped around
recursion. The search path is checked with the existing `path_contains`.
The replay budget is `SIMULATION_BUDGET = 16` positions per replay, matching
provari's `SIMULATION = 16`; it is per replay, not global.

### Immediate-win helper

`immediate_win_move(pos) -> Option<Move>` iterates the legal moves of `pos`
and returns the first move after which the opponent's `outcome()` is
`Loss` (the opponent's commoner is gone or the opponent is mated). It is
used **only** by the replay, to recognize a defence that loses at once
(provari's `exposing` / `exploding_capture` shortcut). It is not wired into
the search's child evaluation, move ordering, or proof numbers. Each call
decrements the replay budget by one and charges one child eval per legal
move tried, so the 16-position budget bounds the number of helper calls and
the child-eval cap bounds the total work. This is the minimum tactical
primitive the replay needs; without it, the many defences refuted by an
immediate explosion would not transfer.

### Budget and accounting

- Every simulated position and every legal move tried by the immediate-win
  helper increments `child_evals`, so the deterministic front-end cap bounds
  the replay. The cap is checked at frame-loop heads, so one replay can
  cross it by at most the work of its 16-position budget (including the
  helper's move scans).
- The front-end's `child_evals` are already folded into the run counters and
  the global budget by the plan3 wiring; no new budget path is needed.
- Add `simulation_attempts`, `simulation_successes`, and
  `simulation_evaluations` counters and accessors so the probe and the
  report can separate replay work from search work.
- The replay never touches the production TT; the scratch TT is still
  dropped when the front-end returns, so fallback identity is preserved.

### TT store and proof events

On a successful replay, mark the sibling `ChildInfo` as solved `Win` with
the returned depth and `pn/dn` from `Outcome::Win.pn_dn_for(true)`. Store the
sibling in the scratch TT with `best_move` = the returned move,
`outcome = Some(Win)`, `depth`, `pn/dn`, `work = 0`, `best_child = u8::MAX`,
and `remaining_depth = u32::MAX`. Set `simulated = true` on the sibling.

Emit `ProofEvent::NodeProven` for every node the replay newly proves, with
the path = the search path to the AND node + the sibling move + the replay
moves. Add a `simulation_move_stack` (or pass a `&mut Vec<Move>` path) and a
`emit_proof_node_with_path` helper; do not reuse `self.move_stack` for the
replay. Reused TT proofs were already emitted when they were proven; the
replay emits only the new sibling-side nodes. Duplicate events are tolerated
by the proof-tree worker, as in plan3.

On a failed replay, leave the sibling open and set `simulated = true` so it
is not retried. Do not change its proof numbers (the first form has no
"parried threat" halving; that is the second form).

### `simulated` flag

Add `simulated: bool` to `ChildInfo`, initialized `false` in both
constructors. In the frame loop, save the flag before `evaluate_child` and
restore it after, so a failed replay is not retried when the child is
re-evaluated. This matches provari's `sibling.simulated`.

### CLI and configuration

- `WinProver::set_simulation(bool)` (default `true`).
- `Search::set_win_front_end_simulation(bool)` and the corresponding field in
  the search configuration; `run.rs` calls the setter when it constructs the
  front-end.
- `CliOptions::win_front_end_simulation: bool` (default `true`), parsed by
  `--no-win-front-end-simulation` (sets it to `false`). The flag is only
  meaningful with `--win-front-end`; document that in `print_help()`.
- Update `tests/fixtures/cli_help_golden.txt` and the `AGENTS.md` CLI option
  list. The `PvStatus` list is unchanged.

### Soundness argument

The replay is exact by construction: it accepts a sibling only if every
defence is answered by a terminal win, a known path-independent TT proof, or
a recursive replay of the guide's proof. It never accepts a repetition or a
rule50 draw as a win. The TT stores only path-independent results, and the
replay's own line is checked for repetition. The immediate-win helper is a
one-ply terminal check, not a heuristic verdict. The front-end still returns
a `Win` only after `extract_pv` replays the TT's `best_move` chain and
`validate_win` reaches a terminal win; the replay only adds solved nodes to
that chain.

## Tests

Unit tests in `src/search/winprover/tests.rs` (or `simulation/tests.rs`):

1. **Transfer** — a small fixture where a refuted defence's proof transfers
   to a sibling; assert the sibling is marked solved and the root is proven
   with fewer child evals than simulation off. If a natural fixture is hard
   to find, use a position where the front-end proves a win with simulation
   on but not off at a small cap; record the fixture and the cap in the
   test.
2. **Repetition is not a win** — a sibling whose replay would repeat the
   search path or the simulation line is not accepted.
3. **Rule50 is not a win** — a sibling at halfmove clock 100 is not
   accepted.
4. **Budget bounds the replay** — a tiny replay budget returns `None` and
   reports the spent simulation evals; the front-end cap still bounds the
   total.
5. **Determinism** — two runs on the same `(FEN, config, cap)` return the
   same proof/evals/nodes/simulation stats.
6. **Proof events replay** — a simulated win's `NodeProven` events replay to
   the claimed paths/hashes/outcomes; a `Vec`-collecting sink is enough.
7. **Simulation off matches plan3** — with `set_simulation(false)`, the
   front-end's result and evals match the plan3 behavior on a fixture.

Integration tests in `tests/test_win_front_end.rs`:

8. **Fallback identity with simulation** — on a position the front-end does
   not prove with simulation on, run `Search` with the front-end on and off;
   assert the same outcome, PV, general-search nodes/evals, and byte-identical
   main-TT snapshot.
9. **Simulated proven win** — on a position the front-end proves with
   simulation, assert `Win`, `PvStatus::WinFrontEnd`, and a replay-validated
   PV; if the position is small enough, consume the proof events through the
   existing proof-tree worker and validate the tree.
10. **Simulation work counts against the global budget** — a small front-end
    cap and a global budget return `Draw`/`BudgetExhausted`; the simulation
    evals are included in `child_evaluations()` and the fallback sees the
    remaining budget.

The tests stay in the default gate (`make test`); `make test-full` is
required because the change is under `src/search/`.

## Measurement and gate

Run the three bench cells at the canonical 10 s / 192 MB budget with
`--first-outcome --outcome-only`, three repeats per cell, in one session on
one host:

- `opt` — the shipped opt-in stack
  (`--move-class-pn --quiet-grouping --eval-pn or`);
- `opt+win` — the same stack plus `--win-front-end
  --no-win-front-end-simulation` (the plan3 control);
- `opt+win+sim` — the same stack plus `--win-front-end` (simulation on).

Also run the front-end alone (the plan3 probe, extended with the simulation
stats) at caps 1,000,000 and 10,000,000, with simulation on and off, and
record evals, nodes, proved, depth, wall, simulation attempts/successes/evals.

**Pre-registered gate.** At a **10,000,000 child-eval front-end cap**
(including simulation work), the front-end with simulation must prove at
least one of `core` / `easy-win`. The cap is ~10× the shipped default and
~3.7 s on the reference host, leaving ~6 s of the canonical 10 s budget for
the general fallback; it is the largest cap that still fits the product
budget. If it proves neither, the verdict is **PIVOT**: do not add the
second form, rule50-free keys, or any other lever in this plan; write the
negative report and recommend the candidate/verification-layer pivot from
[`report3.md`](report3.md) §7. The 1 M result is recorded separately (the
shipped default); a win only at 10 M is a mechanism signal for item 6, not
a default-flag change here.

Append **one** ledger row to [`bench.md`](bench.md) for the candidate stack
(`opt+win+sim`), with per-cell factors or `censored` + bound, the canonical
geometric mean, coverage, and the deterministic evals. If the gate fails,
the row still records the negative result and the report states the pivot.
The row is read with the `Stack` column: the front-end is opt-in and is not
a product win until item 6.

## Tasks

1. **Read and confirm.** Re-read this plan, `initiative.md`, `bench.md`,
   `report3.md`, `plan3.md`, `AGENTS.md`, and the provari item-8 notes.
   Confirm the first-form-only boundary, the gate, and the scratch-TT
   fallback contract.
2. **Implement `simulation.rs`** per the design: trigger, guide, recursion,
   immediate-win helper, budget, stats, TT store, and proof events.
3. **Wire the flag.** Add `WinProver::set_simulation`,
   `Search::set_win_front_end_simulation`, the `CliOptions` field, the
   `--no-win-front-end-simulation` parser, `print_help()`, and the golden
   fixture. Wire the setter in `run.rs`; do not wire it into
   `search_depth`/`search_depth_with_prefix`.
4. **Update `AGENTS.md`**: the `src/search/winprover/` description, the CLI
   option list, and the simulation's budget/accounting note. The `PvStatus`
   list and the 32 MB scratch-TT exception are unchanged.
5. **Add the tests** from the Tests section. Run `cargo fmt`,
   `cargo clippy --all-targets -- -D warnings`, `make test`, and
   `cargo doc --no-deps`. Run `make test-full` because the change is under
   `src/search/`.
6. **Measure** the three bench cells and the front-end probe per the
   Measurement section; write the driver/results under
   `measurements/plan4/` (provenance README, driver, probe, parsed JSON/CSV;
   no raw transcripts, no `__pycache__`).
7. **Apply the gate.** Record the 1 M and 10 M front-end results and the
   gate verdict. If the gate fails, stop the lever work and write the
   negative report with the pivot recommendation; do not add the second
   form.
8. **Append the ledger row** to `bench.md` (one row, candidate stack,
   per-cell factors/censoring, geomean, coverage, evals).
9. **Write `report4.md`** (final task). Include: provenance and exact
   commands; the design and deviations; the plan-session probe tables and
   their limitations; the test results; the bench table (`opt`, `opt+win`,
   `opt+win+sim`); the front-end probe (1 M/10 M, simulation on/off); the
   gate verdict; the ledger row; the gate results; the verdict (GO for the
   first form / PIVOT) and the next-step recommendation; and the explicit
   "not measured" list.
10. **Update `initiative.md`** (item 4 status: first form done or pivoted;
    second form deferred; dated history entry) and the
    `docs/plans/README.md` winprover row.
11. **Clean up.** No `__pycache__`, no raw logs, no TT snapshots, no
    temporary instrumented copy, no provari artifacts. Confirm with
    `git status --short` and `git add --dry-run` that only the intended
    files would be added.

## Acceptance

- The first-form replay is implemented, opt-in with the front-end, default
  on, deterministic, and adds no general-search heuristic.
- A refuted defence's proof is replayed on open siblings; a successful
  replay marks the sibling solved, stores it in the scratch TT, and emits
  replay-validated proof events.
- Repetition and rule50 are never accepted as wins; the replay's line and
  the search path are both checked.
- Simulation work is charged to the front-end cap and the global budget,
  and is separable via the new accessors.
- Fallback identity holds with simulation on: a failed front-end run leaves
  the general search bit-identical (outcome, PV, general nodes/evals, main-TT
  snapshot).
- The gate is measured and reported honestly. If it fails, the report
  recommends the pivot and does not stack more levers.
- One ledger row is appended; `make test` and `make test-full` are green;
  `cargo fmt`, `cargo clippy --all-targets`, and `cargo doc --no-deps` are
  clean.
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
for p in Path("docs/plans/winprover/measurements/plan4").glob("*.py"):
    ast.parse(p.read_text())
PY
```

Do not use `py_compile` (it writes `__pycache__`). Confirm every committed
JSON/CSV parses with the standard library.

## Risks and mitigations

- **The gate fails.** The plan-session prototype already suggests it may.
  That is a valid negative result: the report records the measured evidence
  and recommends the candidate/verification-layer pivot. Do not compensate
  by adding the second form or other levers in this plan.
- **Replay is expensive.** The 16-position budget and the child-eval charge
  bound it; the probe separates simulation evals from search evals. If the
  replay crowds out search, the gate will show it.
- **Proof-event completeness.** The replay proves nodes that need events.
  Emit them with the replay path; the test consumes the stream and replays
  every event. If the proof-tree worker cannot consume a simulated proof,
  the report records it as a finding and the front-end claim is not trusted
  for proof construction.
- **TT eviction.** The guide's proof tree may be partially evicted from the
  32 MB scratch TT; the replay then fails. This is a measured failure mode,
  not a soundness issue. A larger scratch TT is out of scope (the 32 MB
  bound is documented).
- **The immediate-win helper is a tactical primitive.** Keep it
  simulation-local; do not wire it into the search. It is the minimum needed
  to replay a proof faithfully, not a new search heuristic.
- **The second form is out of scope.** `replay_threat` needs a null move and
  static tactics; it is a separate lever. Item 4 remains partially open if
  the first form is adopted.
- **Scope creep.** Do not add rule50-free keys, proof length/limit, default
  flag changes, or a new corpus here.

## Report requirements

`report4.md` is the final task. It must include:

- the provenance table (date, host, solver commit, budgets, front-end cap,
  scratch-TT size, simulation budget);
- the exact commands used;
- the design and any deviations from this plan;
- the plan-session probe tables (front-end census and naive replay) and
  their limitations;
- the test results (unit + integration, including transfer, repetition,
  rule50, budget, determinism, proof-event replay, and fallback identity);
- the bench table (`opt`, `opt+win`, `opt+win+sim`, per cell, with
  front-end evals/nodes and whether the front-end proved the cell);
- the front-end probe (1 M/10 M, simulation on/off, attempts/successes/
  evals);
- the gate verdict and the ledger row;
- the gate results (`make test`, `make test-full`, fmt, clippy, doc);
- the verdict (GO for the first form / PIVOT) and the next-step
  recommendation;
- explicit "not measured" items (the second form, rule50-free keying, proof
  length/limit, default-flag decision, new corpus, tablebases, NNUE,
  root-`Loss`/root-`Draw` front-end claims).

## Deliverables

- `src/search/winprover/simulation.rs` (new), `mod.rs`, `search.rs`,
  `children.rs`, `tests.rs`.
- `src/search/dfpn/config.rs` (setter), `run.rs` (wire the flag).
- `src/cli.rs` (new option and parser test), `src/main.rs` (help text),
  `tests/fixtures/cli_help_golden.txt`.
- `AGENTS.md` (module description, CLI option, simulation accounting).
- `tests/test_win_front_end.rs` (new integration tests).
- `docs/plans/winprover/measurements/plan4/` (provenance README, driver,
  probe, parsed results).
- `docs/plans/winprover/report4.md`.
- `docs/plans/winprover/bench.md` (one ledger row).
- `docs/plans/winprover/initiative.md` and `docs/plans/README.md` status
  updates.
