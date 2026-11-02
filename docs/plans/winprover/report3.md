# Winprover Report 3 — win-only front-end skeleton

Initiative: `winprover`, backlog **item 3**, executed as
[`plan3.md`](plan3.md). Date: **2026-11-02** (UTC). This is a **search**
change: it adds an opt-in attacker-only win prover in front of the general
solver and leaves the general `dfpn` unchanged. Verdict: **NEUTRAL (GO for the
skeleton, no product win)** — the front-end is correct, deterministic,
fallback-identical, and default off, but it proved none of the three bench
cells inside its 1 M-eval cap and added ~0.34–0.37 s / ~1.00 M child evals per
cell. Next: **item 4 (sibling simulation)** with a pre-measurement gate on the
front-end, or a pivot if item 4 cannot make the front-end prove the easy
shared cell.

## Headline

- Added `src/search/winprover/` (`mod.rs`, `search.rs`, `children.rs`,
  `tests.rs`): a small win-only DF-PN+ sibling of `dfpn` with its own 32 MB
  **scratch** TT. It returns a replay-validated `Win` plus
  `PvStatus::WinFrontEnd`, or `None`; every non-Win result falls back to the
  general search.
- Added `--win-front-end` (default off) and `--win-front-end-budget`
  (default 1,000,000 child evals), plus `Search::set_win_front_end`,
  `set_win_front_end_budget`, `win_front_end_evaluations`, and
  `win_front_end_nodes`.
- Added 11 unit tests and 3 integration tests. The integration
  **fallback-identity** test asserts the same outcome, PV, general-search
  nodes/evals, and byte-identical main-TT snapshot with the flag on and off;
  the **proven-win** test replays the returned PV to a terminal win.
- The front-end emits `ProofEvent::NodeProven` for every proven `Win`/`Loss`;
  a unit test replays every emitted path/hash/outcome.
- Bench (same session, three cells, 10 s / 192 MB): `opt` control geomean
  **6.81** (2 shared cells, coverage 2/3) vs `opt+win` **8.75** (2 shared
  cells, coverage 2/3). The front-end proved **0/3** cells and spent
  1,000,014 / 1,000,027 / 1,000,007 evals. This is the pre-registered neutral
  result: overhead, not a lever.
- One ledger row appended to [`bench.md`](bench.md); no default-flag
  decision (item 6).

## 1. Provenance

| Item | Value |
| --- | --- |
| Date | 2026-11-02 (UTC) |
| Host | container `67db7a1089e2`; aarch64, 8 CPU; `Linux-7.1.13-402.asahi.fc44.aarch64+16k`; glibc 2.43 |
| `atomic_solver` | `d623b2114308f9b3fb224c4f641fab9c0be731ba` + plan3 working tree, release build |
| Provari | `aiorla/provari` commit `48686bc6c786d6ea70568f0b2ad4002cf0520206` (`v0.1.0`), `--guide hand` |
| Provari binary | `/tmp/winprover-oracle/target/release/provari` (scratch clone; **not** committed) |
| Corpus | [`../provari/measurements/plan1/corpus.json`](../provari/measurements/plan1/corpus.json) |
| Bench cells | `core`, `easy-win` (`1eb80b92f92c`), `deep-win` (`db6a741c723f`) |
| Budget | 10 s / 192 MB label, 3 repeats per cell per side |
| Front-end cap | 1,000,000 child evals (shipped default) |
| Scratch TT | 32 MB (`WIN_FRONT_END_TT_MB`), dropped per run |
| Tablebases / NNUE | none |

## 2. Exact commands

Build and gates:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
make test            # fast gate, green
make test-full       # whole suite incl. ignored, green
cargo doc --no-deps
cargo build --release
```

Bench (the plan3 driver delegates the canonical run to the committed
`measurements/bench/run_bench.py` and then measures the front-end alone):

```sh
python3 docs/plans/winprover/measurements/plan3/run_plan3.py \
  --provari /tmp/winprover-oracle/target/release/provari \
  --solver ./target/release/atomic_solver
```

Front-end accounting probe (built by the driver):

```sh
rustc --edition 2024 -O -C panic=abort -C lto=thin \
  docs/plans/winprover/measurements/plan3/probe.rs -o /tmp/probe \
  -L target/release/deps \
  --extern atomic_solver="$(ls -t target/release/deps/libatomic_solver-*.rlib | head -1)"
```

## 3. Front-end design

`src/search/winprover/` is a **simplified** DF-PN, not a copy of `dfpn`:

- **State** (`mod.rs`): scratch `TranspositionTable` (32 MB), repetition
  path, move stack for proof events, `nodes`/`child_evals`, deterministic
  child-eval cap, wall deadline, stop flag, `StaticAtomicScorer`, and the
  general solver's exact `1+ε` fraction.
- **Loop** (`search.rs`): the AND/OR proof-number recurrence
  (`pn`/`dn` min/sum, solved-child detection, `1+ε` thresholds). No
  refinement rounds, pre-phase, cheap probe, repetition cache,
  history/killer learning, quiet grouping, move-class numbers, EF-PN, or
  static tactics.
- **Children** (`children.rs`): play the move, classify via
  `Position::outcome()` (terminals, rule50, bare kings), the repetition path,
  and the scratch TT; score moves with the shipped `StaticAtomicScorer` only.
- **Soundness**: a repeated position is a `Draw`; a rule50 position is a
  terminal `Draw`; only path-independent `Win`/`Loss` outcomes and unsolved
  `(pn, dn)` bounds are TT-stored. Terminal `Draw`s live only in the frame
  table (needed for minimax) and are never stored.
- **Claim**: `WinProver::solve` returns `None` for a terminal root, for any
  non-Win result, and on cap/deadline/stop. A claimed root Win is followed
  through the scratch TT's `best_move` chain, replayed legally, and returned
  only if the line reaches a terminal win.

### Deviations and clarifications

1. **AND-node wording.** The plan's bullet ("`Win` if all children are
   `Win`") is the root-attacker restatement of the standard side-to-move
   minimax. Used literally as a side-to-move rule it is unsound (all child
   wins for the defender means the defender is lost). The implementation uses
   the standard rule shared with `dfpn`: `Win` if any child is `Loss`, `Loss`
   if all children are `Win`, `Draw` otherwise. This is documented in
   `search.rs`'s header.
2. **Child evaluation is correctness-first.** It calls `Position::outcome()`
   (a full movegen) instead of `dfpn`'s pre-probe/existence fast paths. Per
   eval the front-end is therefore slower than the general solver; that is
   visible in the bench overhead and is a candidate for item 4's
   implementation work.
3. **Budget charging preserves fallback identity exactly.** Instead of
   adding the front-end's evals to `self.child_evals` before the general
   search (which would inflate `first_outcome_evals` and the refinement cap),
   the solve body was split into a private `solve_general`; the front-end's
   work is folded into the run counters after it returns, and the global
   budget is reduced by the front-end's work for the general phase. The
   integration test pins the stronger contract: same outcome/PV/nodes/evals
   and byte-identical main-TT snapshot.
4. **No `search_depth` wiring.** `search_depth` / `search_depth_with_prefix`
   stay unchanged (the offline certificate-bound entry points), as the plan
   requires.
5. `src/search/dfpn/fraction.rs` became `pub(crate)` so the front-end can
   reuse the exact `f64 → num/den` conversion. The `fraction` module was also
   made `pub(crate)`.
6. The CLI help golden fixture was regenerated for the two new options.

## 4. Item-4 census (carried from the plan session)

The census was measured by the plan session on the **general** `dfpn` with
temporary observation-only counters. The general search is unchanged in this
plan, so the table is carried verbatim; it is a sizing probe, not a prediction
for the front-end (the plan's caveat still applies).

**Opt-in stack** (`--move-class-pn --quiet-grouping --eval-pn or`):

| Cell | Outcome | Evals | `and_nodes` | `and_multi` | `and_not_check` | `win_child_events` | with open | `open_siblings` |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `core` | win | 17,217,064 | 518,264 | 510,056 | 472,619 | 148,982 | 141,039 | 2,234,045 |
| `easy-win` | win | 3,231,686 | 98,239 | 97,363 | 93,819 | 18,873 | 17,900 | 281,269 |
| `deep-win` | draw (timeout) | 47,552,244 | 1,373,270 | 1,362,276 | 1,295,639 | 262,771 | 250,535 | 4,068,169 |

Reading: 98 %+ of AND nodes have at least two children; 94–96 % of refuted
defences have at least one open sibling; the average open siblings per event
is ~15.7–16.2. Strong upper-bound sizing for item 4's first form (replay a
proven defence's proof on open siblings) — but it does not measure proof
transfer, and it is not the front-end's shape.

## 5. Tests

Unit tests (`src/search/winprover/tests.rs`, default gate):

1. `proves_mate_in_one_with_replayable_pv`
2. `terminal_root_is_not_a_front_end_claim`
3. `repetition_is_a_draw_not_a_win`
4. `rule50_root_is_not_a_win`
5. `zero_budget_is_a_no_op`
6. `cap_bounds_the_work_and_reports_it`
7. `run_is_deterministic`
8. `proof_events_replay_to_the_claimed_outcomes`
9. `front_end_ignores_general_search_heuristics`
10. `pv_extraction_stops_on_an_illegal_first_move`
11. `stop_flag_cuts_the_run`

Integration tests (`tests/test_win_front_end.rs`):

12. `fallback_is_bit_identical_to_flag_off` — same outcome, PV, general-search
    nodes/evals, and main-TT snapshot bytes; front-end work non-zero.
13. `front_end_work_counts_against_the_global_budget` — a 100-eval front-end
    cap and a 500-eval global budget return `Draw`/`BudgetExhausted`; the
    front-end's evals are included in `child_evaluations()`.
14. `proven_win_returns_replay_validated_pv` — `Win`, `WinFrontEnd`, one-ply
    PV replays to a terminal `Loss`; no general-search evals after the claim.

Gate results: `make test` green; `make test-full` green (whole suite,
`--include-ignored`); `cargo fmt --check`, `cargo clippy --all-targets -- -D
warnings`, `cargo doc --no-deps`, and `cargo build --release` clean.

## 6. Bench

Reference (provari hand, medians): `core` 0.500 s, `easy-win` 0.111 s,
`deep-win` 6.163 s. Factor = our wall / reference wall; `opt` is the
same-session control.

| Cell | `opt` (wall / evals) | `optwin` (wall / evals) | factor `opt` → `optwin` | front-end evals / frames | proved |
| --- | --- | --- | --- | --- | --- |
| `core` | 3.784 s / 17,217,064 | 4.154 s / 18,217,078 | 7.568 → **8.308** | 1,000,014 / 29,338 | no |
| `easy-win` | 0.680 s / 3,231,686 | 1.022 s / 4,231,713 | 6.126 → **9.207** | 1,000,027 / 30,387 | no |
| `deep-win` | 10.010 s / 47,761,034 (censored) | 10.007 s / 47,300,751 (censored) | ≥ 1.62 → ≥ 1.62 | 1,000,007 / 32,322 | no |

Canonical geomean: `opt` **6.809** → `optwin` **8.746** (both 2 shared cells);
coverage **2/3** both. The front-end's evals are exactly the difference
`optwin − opt` on the two finishing cells; on `deep-win` the total is lower
than the control because the front-end spent ~0.37 s of the 10 s wall budget
before the general search began (the probe value is the front-end's own
count). Details: [`measurements/plan3/`](measurements/plan3/README.md).

### Ledger row

Appended to [`bench.md`](bench.md) §`plan3`:

| Plan | Stack | `core` | `easy-win` | `deep-win` | Geomean (canonical) | Coverage |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `plan3` | opt+win | **8.31** (4.154 s / 18.2 M) | **9.21** (1.022 s / 4.23 M) | censored (≥ 1.62) 10.007 s / 47.3 M | **8.75** (2 cells) | 2/3 |

## 7. Verdict and recommendation

**Verdict: NEUTRAL — GO for the skeleton, no product win.** The implementation
meets every correctness and identity contract (opt-in, default off,
deterministic, fallback bit-identical, replay-validated claims, proof events,
budget accounting). On the bench it is pure overhead: it proves nothing inside
1 M evals and costs ~0.34–0.37 s, moving the geomean 6.81 → 8.75. That is the
plan's pre-registered acceptable result, not a failure of the architecture.

**Recommendation: item 4 next, with a gate.** The census justifies building
sibling simulation *if* the front-end can reach refuted-defence states; today
it cannot reach them on the bench cells within the cap. The item-4 plan
should therefore:

1. re-measure the census on the **front-end**, not the general search (the
   plan's caveat), and
2. add a stop rule: if sibling simulation plus the front-end still proves 0
   of `core`/`easy-win` at a stated cap, pivot instead of stacking more
   levers.

A credible pivot is to use the front-end as a **candidate/verification layer**
on top of the general solver's winning PV (attack-only closure over the
candidate line) rather than as a standalone prover. That role matches the
limited evidence better than a from-scratch attacker search with no heuristics
and a slow `Position::outcome()` child eval.

## 8. Problems, findings, and scope

- **Plan wording (AND-node rule)** was reversed for a side-to-move reading;
  the implementation uses the standard minimax rule and documents the
  interpretation. No behaviour was lost.
- **Bench driver parse gap (pre-existing):** `parse_solver` looks for
  `exit_reason=` in `pre_exit:`, but the CLI prints `reason=`, so candidate
  rows have `exit = None`. Not fixed here (out of scope); the outcome/PV/evals
  fields used by the ledger are unaffected.
- **Bench driver always measures `default`** in addition to supplied stacks;
  the extra rows are in `results.json` but not in the plan3 row.
- **Front-end child evaluation is slow** (full `outcome()` movegen per child).
  It is the likely reason 1 M evals buy so little; item 4 should either adopt
  the general solver's pre-probe fast paths or accept the overhead knowingly.
- **The release rlib is LTO**, so the standalone probe needs `-C lto=thin`
  (documented in `measurements/plan3/README.md`).
- **Scope:** no sibling simulation, rule50-free key, proof length/limit,
  default-flag decision, corpus, tablebases, NNUE, or root-`Loss`/`Draw`
  front-end claims were added.

## 9. Not measured

- Sibling simulation (item 4) and the second census form (`replay_threat`);
- rule50-free keying and proof length/limit (item 5);
- the default-flag decision (item 6);
- a front-end cap sensitivity sweep (1 M is the shipped default; a larger cap
  might prove `easy-win`, but that is a new measurement and not a plan3
  deliverable);
- root-`Loss`/root-`Draw` front-end claims (deliberately out of scope);
- tablebases, NNUE, a new corpus, and any provari model column.

## 10. Deliverables

- `src/search/winprover/{mod,search,children,tests}.rs`
- `src/search/mod.rs`, `src/search/dfpn/{mod,config,run,fraction}.rs`
- `src/cli.rs`, `src/main.rs`, `tests/fixtures/cli_help_golden.txt`
- `tests/test_win_front_end.rs`
- `AGENTS.md`
- `docs/plans/winprover/measurements/plan3/{README.md,run_plan3.py,probe.rs,results.json,front_end.json,results.csv}`
- `docs/plans/winprover/{bench.md,report3.md,initiative.md}`
- `docs/plans/README.md` (winprover row)
