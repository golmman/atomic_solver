# Winprover Report 4 — sibling proof replay, first form (PIVOT)

Initiative: `winprover`, backlog **item 4**, executed as
[`plan4.md`](plan4.md). Date: **2026-11-02** (UTC). This is a **search**
change confined to the opt-in win-only front-end: it adds the first form of
provari's sibling simulation (Kawano's replay) and leaves the general `dfpn`
solver, its TT, and its PV extraction untouched. Verdict: **PIVOT** — the
first form is implemented, exact, deterministic, and budget-charged, but the
pre-registered gate failed: the front-end with simulation still proves none of
the bench cells at a 10,000,000 child-eval cap. No second-form
(`replay_threat`) or other search lever was stacked; the recommended next
session is the candidate/verification-layer pivot from
[`report3.md`](report3.md) §7.

## Headline

- Added `src/search/winprover/simulation.rs`: at an AND node, when a defence
  is refuted, its proof is replayed on every open sibling that has not already
  been simulated. A replay is accepted only when every sibling defence is
  answered by a terminal loss, a simulation-local one-ply win, a
  path-independent scratch-TT proof, or the same defence in the guide.
- Added `WinProver::set_simulation` (default `true`),
  `Search::set_win_front_end_simulation`, the CLI flag
  `--no-win-front-end-simulation`, and simulation attempt/success/eval
  accessors on both `WinProver` and `Search`.
- Replay work is charged to the front-end's deterministic child-eval cap and
  the run's global budget; each replay has a 16-position budget matching
  provari's `SIMULATION = 16`.
- A successful replay marks the sibling solved, stores its root in the scratch
  TT, and emits proof events for the replay nodes; a failed replay only sets
  `simulated` so it is not retried (no proof-number change).
- 18 unit tests (7 new) and 6 integration tests (3 new, one strengthened to
  exercise simulation) pass. A simulated win builds a proof tree that the
  replay validator accepts.
- Bench at 10 s / 192 MB: `opt` **6.69** → `opt+win` (simulation off) **8.62**
  → `opt+win+sim` **8.49** (2 shared cells each), coverage 2/3 unchanged. The
  front-end proved **0/3** cells at 1 M and 10 M evals. At 10 M, replay cut
  front-end frames by ~20–24 % at the same child-eval cap, but did not close a
  root.
- One ledger row appended to [`bench.md`](bench.md). No default-flag decision
  (item 6), no second form, no new corpus, no rule50-free key.

## 1. Provenance

| Item | Value |
| --- | --- |
| Date | 2026-11-02 (UTC) |
| Host | container `67db7a1089e2`; aarch64, 8 CPU; `Linux-7.1.13-402.asahi.fc44.aarch64+16k`; glibc 2.43 |
| `atomic_solver` | `e3d7b1f` + plan4 working tree, release build |
| Provari | `aiorla/provari` commit `48686bc6c786d6ea70568f0b2ad4002cf0520206` (`v0.1.0`), `--guide hand` |
| Provari binary | `/tmp/winprover-oracle/target/release/provari` (scratch clone; **not** committed) |
| Corpus | [`../provari/measurements/plan1/corpus.json`](../provari/measurements/plan1/corpus.json) |
| Bench cells | `core`, `easy-win` (`1eb80b92f92c`), `deep-win` (`db6a741c723f`) |
| Budget | 10 s / 192 MB label, 3 repeats per cell per side |
| Front-end caps | 1,000,000 (shipped default) and 10,000,000 (gate cap) child evals |
| Replay budget | 16 positions per replay |
| Scratch TT | 32 MB (`WIN_FRONT_END_TT_MB`), dropped per run |
| Tablebases / NNUE | none |

## 2. Exact commands

Build and gates:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
make test
make test-full
cargo doc --no-deps
cargo build --release
```

Bench and probe:

```sh
python3 docs/plans/winprover/measurements/plan4/run_plan4.py \
  --provari /tmp/winprover-oracle/target/release/provari \
  --solver ./target/release/atomic_solver
```

The driver delegates the canonical bench to the committed
`measurements/bench/run_bench.py` and builds
`measurements/plan4/probe.rs` against the newest release rlib with
`rustc --edition 2024 -O -C panic=abort -C lto=thin`.

## 3. Design and deviations

### Implemented design

- **Trigger** (`search.rs`): inside the DF-PN frame loop, after a child is
  re-evaluated, an AND node whose child becomes `Some(Win)` (the defence was
  refuted) replays that child's proof on the other still-open children. The
  child's `simulated` flag is saved across re-evaluation and restored, so a
  failed replay is not retried.
- **Guide and recursion** (`simulation.rs`): the guide is the refuted child
  position. `simulate_attacker` follows the guide's scratch-TT `best_move`; a
  terminal guide uses the simulation-local `immediate_win_move` helper.
  `simulate_defender` requires every legal defence to be answered; failure of
  any one defence rejects the replay. A position repeated on the search path
  or on the replay line is a Draw and is rejected; rule50 is a terminal Draw
  through `Position::outcome`. The same check is applied to each child after a
  defence, so a TT proof for a position that repeats the current context is
  never accepted.
- **Immediate-win helper**: one-ply scan only, used by the replay and never by
  the search's child evaluation, ordering, or proof numbers.
- **Budget**: each simulated position and each legal move tried by the helper
  charges one `child_evals` unit (also counted in `simulation_evaluations`).
  Each replay has its own 16-position budget. The front-end cap is checked at
  frame-loop heads, so one replay can cross it by at most its own budget.
- **TT and events**: a successful replay stores the sibling root in the
  scratch TT (`Win`, best move, depth, `pn/dn`); intermediate replay nodes are
  not cached. Proof events are emitted with the replay path
  (`emit_proof_node_with_path`), never through `self.move_stack`.
- **Configuration**: `WinProver::set_simulation` default `true`,
  `Search::set_win_front_end_simulation`, `CliOptions::win_front_end_simulation`
  default `true`, parsed `--no-win-front-end-simulation`; `run.rs` wires the
  setter. `search_depth`/`search_depth_with_prefix` remain unwired.

### Deviations and findings

1. **Terminal children now get proof events.** The plan3 front-end emitted a
   non-terminal `Win`/`Loss` event without the event for a terminal child
   classified by `evaluate_child`; the worker therefore had a node that
   `finalize` recomputed as a depth-0 leaf, and `validate_proof_tree` reported
   `terminal_mismatch`. `evaluate_child` now emits the terminal child event
   (path = current search path + move). This is a first-form event-completeness
   fix, not a search change; it was required for the simulated win's proof tree
   to replay-validate.
2. **Reused TT proofs are re-emitted at the replay path.** The plan assumed a
   reused TT proof's earlier event would suffice. It does not: the worker's
   dummy path node has no hash, so canonicalization cannot connect it to the
   original twin. A TT-sourced replay node (attacker `Win` or defender `Loss`,
   and a defender's TT-solved child `Win`) now emits a node event at the current
   replay path with its stored hash/outcome/depth. Duplicates are tolerated and
   canonicalized by the worker.
3. **Search-level simulation accessors were added.** The plan only named
   `WinProver` accessors; the integration/global-budget test needs to
   distinguish replay work from search work in a real `Search` run, so
   `Search::win_front_end_simulation_{attempts,successes,evaluations}` were added
   (zero when the front-end is skipped). They are accounting only.
4. **No second form.** `replay_threat`, null move, static tactics, the
   "parried threat" proof-number halving, rule50-free keys, and proof
   length/limit are untouched.
5. **The gate fixture for unit tests** is
   `6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26` at cap 1,200 evals:
   simulation off needs 1,240 evals and fails; simulation on proves depth 7 in
   1,195. This natural fixture exercises transfer through the real search, not
   just the replay API.

## 4. Plan-session probes (carried from plan4)

The plan session ran two temporary observation-only probes on a copy of the
current tree (deleted afterwards; no probe code committed). They are sizing
evidence, not the final implementation.

### Front-end census (simulation off)

| Cell | Cap | Evals | `and_nodes` | `and_multi` | `win_child_events` | with open | `open_siblings` |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `core` | 1 M | 1,000,014 | 21,054 | 21,038 | 1,054 | 1,024 | 13,331 |
| `easy-win` | 1 M | 1,000,027 | 20,045 | 20,039 | 1,736 | 1,715 | 20,914 |
| `core` | 10 M | 10,000,043 | 204,541 | 204,283 | 12,945 | 12,575 | 174,795 |
| `easy-win` | 10 M | 10,000,023 | 185,553 | 185,398 | 18,504 | 18,152 | 273,479 |

The front-end reaches refuted-defence states even at 1 M evals, and 97–99 % of
those have open siblings. This sizes the first form but does not measure proof
transfer.

### Naive replay prototype (simulation on, not faithful)

| Cell | Cap | Sim attempts | Sim successes | Sim evals | Proved? |
| --- | ---: | ---: | ---: | ---: | --- |
| `core` | 1 M | 3,682 | 433 | 16,609 | no |
| `easy-win` | 1 M | 4,583 | 641 | 30,386 | no |
| `core` | 10 M | 42,798 | 4,482 | 309,738 | no |
| `easy-win` | 10 M | 62,670 | 7,939 | 408,629 | no |

The prototype suggested a negative result for the naive form; the execution
session implemented the faithful first form (guide proof replay, immediate-win
helper, `simulated` flag, proof events) and re-measured. The final probe is in
§7.

## 5. Tests

Unit tests (`src/search/winprover/tests.rs`, default gate; 18 total, 7 new):

1. `simulation_transfers_a_refuted_defence_proof_to_a_sibling` — simulation on
   proves the natural fixture inside 1,200 evals; off does not (1,240 needed);
   asserts attempts/successes/eval counters and a replayable PV.
2. `simulation_reduces_the_work_on_the_transfer_fixture` — at 10,000 evals both
   prove, but simulation uses fewer child evals; the off arm reports zero replay
   attempts.
3. `repetition_on_the_search_path_refuses_a_replay` — a sibling whose
   repetition key is on the search path is rejected without spending evals.
4. `rule50_position_is_not_a_replay_win` — a halfmove-clock-100 position is
   rejected as a Draw without spending evals.
5. `replay_budget_bounds_the_work` — a one-position replay budget returns
   `None`, leaves the budget at zero, and reports exactly one simulation eval.
6. `simulation_runs_are_deterministic` — two runs on the same
   `(FEN, config, cap)` return the same proof/evals/nodes/attempts/successes.
7. `simulated_proof_events_replay_with_correct_paths_and_hashes` — every event
   path is legal, the replayed hash matches, and every depth-0 event is
   terminal.

All pre-existing front-end tests remain green, including the proof-event replay
and cap/stop tests.

Integration tests (`tests/test_win_front_end.rs`, 6 total, 3 new):

8. `fallback_is_bit_identical_to_flag_off` — unchanged contract, now with
   simulation on by default: same outcome, PV, general nodes/evals, and
   byte-identical main-TT snapshot.
9. `simulated_win_returns_replay_validated_pv` — the natural fixture returns
   `Win` + `PvStatus::WinFrontEnd`, the PV replays to a terminal Loss, and no
   general-search evals run after the claim.
10. `simulated_win_builds_a_valid_proof_tree` — consumes the front-end's proof
    events through the existing worker, finalizes, and passes
    `validate_proof_tree` (this is the test that exposed deviations 1–2).
11. `simulation_work_counts_against_the_global_budget` — a busy forcing line
    with a 10,000-eval front-end cap and a 10,100-eval global budget returns
    `Draw`/`BudgetExhausted`; replay attempts/successes are non-zero,
    front-end evals are at least the cap, and the total includes the replay
    work.

Gate results: `make test` green; `make test-full` green (see §8);
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo doc --no-deps`, and `cargo build --release` clean.

## 6. Bench

Reference (provari hand, medians): `core` 0.496 s, `easy-win` 0.113 s,
`deep-win` 6.198 s. Factor = our wall / reference wall.

| Cell | `opt` (wall / evals) | `opt+win` sim off | `opt+win+sim` | factor `opt` → `opt+win` → `opt+win+sim` |
| --- | --- | --- | --- | --- |
| `core` | 3.786 s / 17,217,064 | 4.158 s / 18,217,078 | 4.135 s / 18,217,089 | 7.633 → **8.383** → **8.337** |
| `easy-win` | 0.662 s / 3,231,686 | 1.001 s / 4,231,713 | 0.977 s / 4,231,703 | 5.858 → **8.858** → **8.646** |
| `deep-win` | 10.009 s / 47,761,034 (censored) | 10.004 s / 47,244,697 (censored) | 10.008 s / 47,301,018 (censored) | ≥ 1.61 → ≥ 1.61 → ≥ 1.61 |

Canonical geomean: `opt` **6.687** → `opt+win` **8.617** → `opt+win+sim`
**8.490** (all 2 shared cells); coverage **2/3** in all three. The
`opt+win+sim` improvement over `opt+win` is noise-level (4.158 → 4.135 s on
`core`, 1.001 → 0.977 s on `easy-win`); the front-end still proves none of the
cells, so this is not a product lever. On `deep-win` the front-end spends part
of the wall budget before the general search, hence the lower total eval count
than the `opt` control.

### Ledger row

Appended to [`bench.md`](bench.md) §`plan4`:

| Plan | Stack | `core` | `easy-win` | `deep-win` | Geomean (canonical) | Coverage |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `plan4` | opt+win+sim | **8.34** (4.135 s / 18.2 M) | **8.65** (0.977 s / 4.23 M) | censored (≥ 1.61) 10.008 s / 47.3 M | **8.49** (2 cells) | 2/3 |

## 7. Front-end probe and gate

The front-end alone, same cells, simulation off → on:

| Cell | Cap | Evals off / on | Frames off / on | Attempts | Successes | Sim evals | Proved |
| --- | ---: | --- | --- | ---: | ---: | ---: | --- |
| `core` | 1 M | 1,000,014 / 1,000,025 | 29,338 / 21,993 | 3,889 | 461 | 242,465 | no / no |
| `easy-win` | 1 M | 1,000,027 / 1,000,017 | 30,387 / 21,587 | 4,683 | 628 | 277,265 | no / no |
| `deep-win` | 1 M | 1,000,007 / 1,000,274 | 32,322 / 27,958 | 7,724 | 1,185 | 147,291 | no / no |
| `core` | 10 M | 10,000,043 / 10,000,034 | 294,360 / 224,470 | 46,194 | 4,646 | 2,438,170 | no / no |
| `easy-win` | 10 M | 10,000,023 / 10,000,032 | 303,251 / 227,110 | 63,489 | 7,553 | 2,579,543 | no / no |
| `deep-win` | 10 M | 10,000,013 / 10,000,025 | 316,483 / 260,353 | 67,003 | 18,216 | 1,907,735 | no / no |

Reading: replay succeeds on ~12–27 % of attempts (461/3,889 to 18,216/67,003),
so the mechanism is not vacuous. It reduces front-end frames by ~20–24 % at
10 M at the same child-eval cap, but the transferred proofs never close a root.
The pre-registered gate at **10,000,000 child evals**: prove at least one of
`core` / `easy-win`. **It proves neither → gate FAILED → verdict PIVOT.** The
1 M result (the shipped default) is also 0/3. No second form, rule50-free key,
or other lever is stacked in this plan.

## 8. Gate results

- `cargo fmt --check` — clean.
- `cargo clippy --all-targets -- -D warnings` — clean.
- `make test` — green.
- `make test-full` — green (whole suite including ignored, 39 result groups;
  the session log has no `FAILED` lines).
- `cargo doc --no-deps` — clean.
- `cargo build --release` — clean.
- Python drivers parse with `ast.parse` (no `py_compile`, no `__pycache__`);
  every committed JSON/CSV parses.

## 9. Verdict and next step

**Verdict: PIVOT.** The first-form replay is correct, exact, deterministic, and
budget-charged, and it is measurably active (attempts/successes and ~20–24 %
fewer front-end frames at 10 M), but it does not make the standalone
attacker-only front-end prove a bench cell at the pre-registered 10 M cap. The
mechanism is not the missing piece; the front-end's search shape / child-eval
cost is.

Keep the first form in the tree (it is inside the default-off front-end, it
passes all gates, and it is the home of any future simulation work), but do
**not** add the second form or further sibling-simulation levers. The next
plan session should scope the **candidate/verification-layer pivot** from
[`report3.md`](report3.md) §7: use the front-end as an attack-only closure
over the general solver's winning PV / candidate line, rather than as a
standalone prover. Item 5 (rule50-free key + proof length) remains the next
open search lever if that pivot is rejected. Item 6 (default flags /
head-to-head) stays deferred.

## 10. Not measured

- The second form (`replay_threat`: null move, static-tactics closure,
  "parried threat" proof-number halving).
- Rule50-free keying and proof length/limit (item 5).
- The default-flag decision (item 6), a new corpus, tablebases, or NNUE.
- Root-`Loss`/root-`Draw` front-end claims (still deliberately out of scope).
- A 120 s or larger-budget re-run of `deep-win` (it remains censored at the
  canonical budget).
- A front-end cap sweep beyond 1 M and 10 M.

## 11. Deliverables

- `src/search/winprover/simulation.rs` (new), `mod.rs`, `search.rs`,
  `children.rs`, `tests.rs`.
- `src/search/dfpn/{mod,config,run}.rs` (flag + accounting accessors).
- `src/cli.rs`, `src/main.rs`, `tests/fixtures/cli_help_golden.txt`.
- `tests/test_win_front_end.rs`.
- `AGENTS.md` (module description, CLI option list).
- `docs/plans/winprover/measurements/plan4/{README.md,run_plan4.py,probe.rs,results.json,front_end.json,results.csv}`.
- `docs/plans/winprover/{bench.md,report4.md,initiative.md}`.
- `docs/plans/README.md` (winprover row).
