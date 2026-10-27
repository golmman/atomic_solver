# Plan 12: Restart policy vs. hard work budget — measure the schedule, split the plumbing

Initiative: `conversion`, **backlog #9** (new item, opened by this plan).
Plan numbering: **plan11 is retired, not reused** — it was pencilled in for
backlog #8's question B (external-PV fidelity), which `report10.md` found
"not draftable" when #8 closed no-go. This plan therefore takes the next
free number.

Prerequisite reading (all self-contained pointers):

- `src/search/dfpn/mod.rs` — `bounded_search` (the chunk/restart loop,
  `set_child_eval_budget` docs, `RoundTermination`), `begin_run`,
  `reset_search_state`, `time_exceeded`.
- `src/search/dfpn/core.rs` — the `max_work` / `child_max_work`
  threading inside the `dfpn` frame loop (the "work-cut" path and the
  `explored` marking).
- `src/search/dfpn/children.rs` — `evaluate_all_children` (sweep size,
  the unit of budget overshoot).
- `docs/plans/ultimattt/analysis.md` §2.3 — where the mechanism came from
  (`mid` in `CHECK_TICK_WORK = 500_000` slices, so the driver could check
  the clock).
- `docs/plans/conversion/report6.md` §"Chunk-boundary waste" — the only
  prior measurement of the chunk boundary (19.8% FO / 34.7% default
  ceiling on stress, judged "measured but not permitted" under the
  reuse-widening lane).
- `docs/plans/conversion/report9.md`, `report10.md` — the threshold/ε
  hazard class this plan must stay out of (it changes pacing only, never
  threshold arithmetic).

## Why this plan exists — measured evidence

2026-10-26 investigation session (env-gated `SPIKE_CHUNK_BASE` /
`SPIKE_CHUNK_FACTOR` / `SPIKE_CHUNK_CAP` inside `bounded_search`, measured,
**fully reverted**: `git diff` empty, `test_trajectory_golden --include-ignored`
green). All numbers are deterministic `child_evals` at
`--first-outcome`, `--tt-size 256` unless stated; the shipped schedule is
`chunk = 500_000`, geometric ×2, no cutoff.

Identity anchors at HEAD (shipped schedule; these are the Phase 0
baselines):

| case (FEN) | tt 256 | tt 128 |
| --- | --- | --- |
| stress `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21` | 287,872,755 (win, 147 plies, Complete) | 305,404,068 (win 669) |
| m22 `4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22` | 15,023,911 (win 75) | 19,111,111 (win 301) |
| m23_white `4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23` | 11,621,376 (win 27) | 9,553,618 (win 65) |
| dec31 `r1bq1k2/ppN3p1/2p1p2P/6P1/1b1P1p2/8/PP3P2/R2K1B1R b - - 0 15` | 510,426 (loss 22) | 510,426 |
| `benchmark --suite quick --first-outcome --timeout 3` (59 cases) | 42,109,768 / 59 solved | 40,042,745 / 59 solved |
| m22 default mode (CLI default tt 256, `--timeout 20`) | 18,779,890, `pv_status: cap-cut` (== `tests/fixtures/m22_default_stdout_golden.txt`) | — |

Schedule arms (tt 256):

| restart schedule | quick @3 s | stress FO | dec31 | m23_white | m22 FO |
| --- | --- | --- | --- | --- | --- |
| **shipped 500k ×2** | 42.1M, 59/59 | 287.9M, win | 0.51M | 11.62M, win | **15.02M**, win 75p |
| **no restart** | 78.3M, **57/59** (2 timeouts) | **227.3M**, win 321p | **141.85M** (278×) | **122.4M → Draw @20 s** | 19.38M, win 35p |
| single restart @500k (then unbounded) | 42.3M, 59/59 | — | 0.51M | 11.37M | 57.7M |
| **constant 500k restarts** | — | **1,207M → Draw @200 s** | 0.51M | 9.72M | 22.6M |
| restarts only while ≤ 5M spent | 42.1M, 59/59 | 309.0M, win | — | — | — |
| 500k ×1.25 | 39.19M, 59/59 | — | 0.51M | 11.46M | 24.07M |

Supporting probes:

- **Restart position, not a magic number**: dec31 solves after *any* early
  boundary — base 100k/250k/500k/1M/2M → 130,616 / 260,419 / 510,426 /
  1,006,796 / 2,006,820 evals (≈ base + 10k). No boundary → 141,854,807.
- **The `max_work` sub-paths are inert without a budget**: finite
  `max_work = 1e12` and `u64::MAX` both give exactly 12,782,135 nodes /
  227,285,638 evals on stress FO (the "no restart" arm). The
  `explored`-marking path is therefore not load-bearing today.
- **The schedule's sign flips with TT size** (`examples/chunk_growth`,
  hardcoded 64 MB TT): stress 451.7M chunked vs 900.0M/916.3M
  no-restart; m22 >150 s chunked vs 2.9 s no-restart. The schedule is a
  TT-capacity-dependent hyperparameter, so it must never be treated as a
  fixed performance guarantee.
- **The documented purpose is obsolete**: `time_exceeded()` already
  samples the clock every 4096 `dfpn` entries (`CLOCK_SAMPLE_INTERVAL`),
  so the original "check the clock between slices" rationale costs
  nothing when removed. The `core.rs:208-210` comment ("the next chunk can
  resume from the same bounds") is wrong: only TT entries survive a
  boundary; frame thresholds and child tables are rebuilt, and the root
  re-enters at `INF/INF`. It is a **restart**.

## The finding this plan acts on

The work-chunk loop carries three claimed roles; the measurement separates
them:

1. **Clock/quit responsiveness** — obsolete (per-node sampling).
2. **Restart/diversification policy** — real and large (278× on dec31,
   m23_white unprovable within 20 s without it, +1 quick-suite case pair),
   but undocumented and unprincipled, and a ~21% tax on the
   deep-conversion stress class at 256 MB TT.
3. **Hard budget / round-cap enforcement** — real, and the only role that
   cannot simply be deleted; it is currently implemented *through* the
   same `max_work` plumbing, which is why the two are entangled.

## Questions (one lever each)

- **Q1 (measurement)**: which restart schedule should ship, on the
  `child_evals` metric, over the reference set at both TT budgets? Does
  any candidate dominate the shipped 500k ×2?
- **Q2 (plumbing)**: can the *hard* child-eval budget be enforced by a
  per-node check instead of by shrinking the restart slice, keeping the
  unbudgeted path **bit-identical** and the pacing semantics
  (restart slice + per-round cap) unchanged?

Q2's answer is designed to be "yes, by construction" (see normative
design); its job is to prove it with the gate, not to find a win.

## Scope

**In scope**

1. A restart-schedule measurement grid (Phase 1) with the pre-registered
   decision rule D1 for changing the shipped default.
2. Making the restart schedule an explicit, named policy in
   `Search` (`RestartSchedule`), replacing `linear_chunks` /
   `chunk_increment` / `chunk_multiplier_num` / `chunk_multiplier_den` and
   their two public setters (`set_linear_chunks`,
   `set_chunk_multiplier_from_factor`) — the only in-repo caller is
   `examples/chunk_growth.rs`.
3. Splitting hard-budget enforcement (per node) from pacing (restart
   slice + round cap), and re-deriving `RoundTermination` from that split.
4. Correcting the misleading comments/docs listed above, plus the
   `AGENTS.md` `src/search/dfpn/` bullet.

**Out of scope (non-goals)**

- Any change to ε, threshold arithmetic, move ordering, TT sizing,
  eviction/retention, the repetition cache, or the pre-phase. D1 is a
  pacing-only decision; the plan10/plan11 hazard class (solved facts
  folded into unsolved threshold arithmetic) is untouched by design.
- Parallelism (`parallel` initiative), proof-tree/proofdb layers, the
  `Search::solve` refinement *policy* (`--refine-cap` factor, PV statuses
  definition).
- A new CLI knob for the schedule. The default is the only schedule the
  CLI can run; `Search::set_restart_schedule` stays a library/example API
  (as `chunk_growth`'s two setters are today).
- Any claim that the schedule is a performance guarantee across TT sizes.

## Normative design

### D-a. `RestartSchedule` (explicit pacing policy)

```rust
/// Deterministic restart schedule for `bounded_search`.
///
/// When the current slice of child evaluations is spent, the recursion
/// unwinds to the root and re-enters `dfpn` with INF/INF thresholds: only
/// transposition-table entries survive, frame state is rebuilt. This is a
/// *diversification* device (it changes which corridor is explored next),
/// not a resumption mechanism. Pacing only — it never enforces a budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RestartSchedule { base: u64, growth: Growth, cutoff: Option<u64> }

pub enum Growth { Geometric { num: u64, den: u64 }, Linear { step: u64 } }
```

- `RestartSchedule::SHIPPED` = `{ base: 500_000, growth: Geometric(2,1), cutoff: None }`
  — an exact reproduction of today's constants.
- `RestartSchedule::NEVER` = `{ base: u64::MAX, growth: Geometric(1,1),
  cutoff: Some(0) }` — the first slice is unbounded and the cutoff keeps it
  that way. `{ base: 500_000, .., cutoff: Some(0) }` must be an equivalent
  spelling (the cutoff fires before the first slice is spent); a test pins
  the equivalence.
- `RestartSchedule::geometric(base, factor)` uses the existing
  `fraction_from_f64` (shared with ε); `with_cutoff(Option<u64>)` returns
  the schedule with restarts disabled once `child_evals >= cutoff`.
- `next(&self, chunk: u64, spent: u64) -> u64`:
  `if cutoff.is_some_and(|c| spent >= c) { u64::MAX } else { grow(chunk) }`,
  with `grow` = today's two formulas verbatim (`saturating_add` linear,
  `u128`-widened `chunk*num/den` geometric).
- `Search::set_restart_schedule(schedule)`; `Search::restart_schedule()`
  getter for the example. `Search::new` seeds `SHIPPED`.
- `Search::restarts() -> u64`: the number of restart boundaries crossed in
  the current run (reset in `begin_run`). This makes the schedule
  observable from code, so `chunk_growth` and tests stop counting
  `chunk done` stderr lines (today's example greps them).
- `examples/chunk_growth.rs` is updated to `set_restart_schedule`, gains
  `--none` and `--cap <evals>` (replacing `--linear`'s implicit default;
  `--linear` becomes `Growth::Linear`), and keeps its table format. Its
  doc comment gains the measured caveat that its hardcoded `Search::new(64)`
  TT makes the comparison TT-size-specific.

### D-b. Hard budget is enforced per node, pacing stays in the slice

`dfpn`'s `max_work` parameter is **renamed** `restart_slice` (the pacing
slice shared by the frames of one restart), with semantics unchanged: the
frame loop breaks when `child_evals - frame_start >= restart_slice`, and
children receive `restart_slice.saturating_sub(spent)`. Renaming rather
than restructuring is deliberate: with no global budget set, today's
`call_max_work` equals the slice exactly, so this path stays
**bit-identical**.

New per-node hard check, at the top of the `dfpn` frame loop next to
`time_exceeded()`:

```rust
if self.child_eval_budget_exceeded() { break; }
```

No new field: the existing `child_eval_budget` (a run constant) already
carries the limit, and `child_eval_budget_exceeded()` is
already the documented predicate. With no budget set it is one
compare against `u64::MAX`.

`bounded_search`'s loop keeps the **round cap** in the pacing slice
(`call_slice = chunk.min(round_remaining)`), because a per-round cap is a
pacing device — it bounds how much of a round is spent before giving up,
exactly like the restart slice. **Only the global budget moves to the
per-node check.** This is what preserves bit-identity in refined mode,
where the cap is active for every round (moving the cap too would shift
the m22 default-mode trajectory and the stored golden).

- `RoundTermination` attribution is re-derived at the loop exit, in this
  order (mirroring today's tie rule):
  1. `child_eval_budget_exceeded()` or `time_exceeded()` →
     `ResourceCut`, except that `round_remaining < remaining_budget`
     (the cap is the smaller constraint) yields `CapCut` — keep today's
     comparison verbatim for ties;
  2. otherwise `spent < call_slice` (or `call_slice == u64::MAX`) →
     `Exhausted`;
  3. `call_slice == 0` with a cap remaining → `CapCut` (the `chunk == 0`
     overflow guard stays as `ResourceCut`).
- `set_child_eval_budget(0)` must still exit before the first call (the
  per-node check fires at the root's first loop iteration, and the
  `bounded_search` loop condition already tests
  `child_eval_budget_exceeded()` — keep both) and report
  `ExitReason::BudgetExhausted`.
- Overshoot contract (documented on `set_child_eval_budget`): the budget is
  crossed by at most the remainder of the `evaluate_all_children` sweep of
  the frame iteration that crosses it (≤ legal moves, < 256): every
  unwinding ancestor re-checks at its loop head and a freshly entered
  child checks before its first sweep. Pinned by a test over several
  budgets.
- Determinism: the per-node check is a pure function of `child_evals` and
  the run-constant `child_eval_budget`. No wall clock enters.

### D-c. Documentation corrections (normative)

- `core.rs:208-210`: replace the "resume from the same bounds" comment
  with the restart semantics (unwind + re-enter at `INF/INF`; TT only).
- `mod.rs` `set_child_eval_budget` doc: budget is enforced per node; the
  restart slice is pacing and must not be described as budget enforcement.
- `RoundTermination` doc: state the derived attribution rule of D-b.
- Module header `//!` of `src/search/dfpn/mod.rs` and the `AGENTS.md`
  bullet: add one sentence — deterministic restart schedule
  (`base 500_000`, geometric ×2, no cutoff; a pacing/diversification
  device) and per-node hard budget enforcement.

## Phase 0 — identity anchors (read-only, ~10 min)

Reproduce the identity-anchor table above at HEAD with the shipped schedule
(`cargo build --release`, then the exact commands in the anchors table;
`--timeout 200` for stress, 60 for m22/m23_white, 20 for dec31) and record
any drift vs this plan's numbers. **Any anchor that does not reproduce
exactly is a stop-and-investigate event** (it invalidates the Phase 1 read
and the Phase 2 neutrality bars); record the reason in the report and
re-baseline the plan's table before proceeding. Also run
`cargo test --release --test test_trajectory_golden -- --include-ignored`
(must be green) and `cargo run --release --example chunk_growth -- --factor 2 --runs 1 --timeout 60`
as a harness smoke test.

## Phase 1 — restart-schedule grid (measurement; env gate, reverted)

Re-apply the 12-line temporary gate in `bounded_search` (prototyped in the
investigation session; keep it `std::env::var`-based so the default path is
untouched):

```rust
let mut chunk = std::env::var("SPIKE_CHUNK_BASE").ok().and_then(|v| v.parse().ok()).unwrap_or(500_000u64);
let spike_factor = std::env::var("SPIKE_CHUNK_FACTOR").ok().and_then(|v| v.parse::<f64>().ok()).map(fraction_from_f64);
let spike_cutoff = std::env::var("SPIKE_CHUNK_CAP").ok().and_then(|v| v.parse::<u64>().ok());
// before computing call_max_work: if spike_cutoff.is_some_and(|c| self.child_evals >= c) { chunk = u64::MAX; }
```

Driver: `docs/plans/conversion/measurements/plan12/driver.py`
(`sys.dont_write_bytecode = True`), run in an isolated copy of HEAD so the
product tree is never touched during measurement (the `research/reexam`
pattern):

| arm | env | meaning |
| --- | --- | --- |
| S0 | — | shipped 500k ×2 (control; must equal Phase 0 exactly) |
| S1 | `SPIKE_CHUNK_BASE=18446744073709551615` | no restart |
| S2 | `SPIKE_CHUNK_BASE=500000 SPIKE_CHUNK_FACTOR=1000` | single restart @500k |
| S3 | `SPIKE_CHUNK_BASE=500000 SPIKE_CHUNK_CAP=5000000` | early-restart phase only |
| S4 | `SPIKE_CHUNK_BASE=500000 SPIKE_CHUNK_FACTOR=1.25` | many restarts |
| S5 | `SPIKE_CHUNK_BASE=500000 SPIKE_CHUNK_FACTOR=4` | few restarts |

Matrix: arms S0–S5 × TT {256, 128} × cases {stress FO, stress default, m22
FO, m23_white FO, dec31 FO} × `--suite quick --first-outcome --timeout 3`,
plus `--suite decisive --first-outcome --timeout 5` if it fits the session
(otherwise record as a gap). Budget ~20 min of solver time: 6 × 2 × 5
single runs (stress is the costly one, ~50 s) + 12 quick suites (~6 s).

Record per cell: outcome, `child_evals`, nodes, PV length (informational),
`pre_exit` reason, chunk count (from the `chunk done` stderr lines), wall
time. Do **not** run the `decisive_remaining` eval-budget suite here (it is
a budget-mode measurement, so Phase 1's restart arms do not touch it);
run it once in Phase 2 for the adopted default.

### D1 — pre-registered decision rule (change the shipped default?)

Adopt a non-shipped schedule as `RestartSchedule::SHIPPED` **iff all** of:

1. stress FO `child_evals` improves ≥ 10% at **both** tt 256 and tt 128
   (the deep-conversion class is the initiative's priority class, and the
   schedule's sign flips with TT size — one budget is not evidence);
2. no outcome change on any single case (all five keep their expected
   outcome within the anchor timeout) and no outcome change on the quick
   suite at both TT sizes (59/59, `wrong=false` everywhere);
3. quick-suite total `child_evals` at both TT sizes is not worse than S0
   (+5% allowed);
4. no case in the grid regresses ≥ 2× (a tail guard: the schedule is a
   tail-shaping device, so it may not create a new tail).

Otherwise **D1 = keep S0**. Expected outcome on the evidence above: keep
S0 (no arm dominates: S1 wins stress by 21% but loses dec31/m23_white, S3
is ~+7% on stress and equal elsewhere, S4/S5 trade stress for the tail).
A D1 "keep S0" is a **successful** result, not a failed phase — it closes
the "should we retune the chunk size?" question with data instead of
anecdote.

**Then revert the gate** (`cp` the pre-spike file back; no `git` writes),
verify `git diff` is silent, rebuild, Phase 0 anchors + golden green.

## Phase 2 — plumbing split (implementation)

Implement D-a, D-b, D-c. Order:

1. **D-a** `RestartSchedule` + setters + `examples/chunk_growth.rs`
   (`--none`, `--cap`, `--linear`, `--factor`); a unit test
   `schedule_shipped_matches_the_legacy_formula` compares `next()` against
   the two old formulas over a fixed chunk sequence, and one that
   `NEVER` (base `u64::MAX`) and `cutoff: Some(0)` coincide.
2. **D-b** rename `max_work` → `restart_slice` in `core.rs` (and
   `child_max_work` → `child_slice`), add the `child_eval_budget_exceeded()`
   loop-head check, drop the global budget from `call_max_work`,
   re-derive `RoundTermination`, update the
   `bounded_search_reports_{resource,cap}_cut_*` tests if the break sites
   moved (their assertions must not weaken).
3. **D-c** docs (`core.rs`, `mod.rs`, `AGENTS.md`) and the module header.
4. Tests (see below), `cargo fmt`, `cargo clippy`, `cargo doc`, gate.

### D2 — pre-registered acceptance rule (does the split ship?)

The split ships **iff all** of:

1. **Unbudgeted path bit-identical**: quick suite (both TT sizes)
   eval-identical to Phase 0 (59/59, exact totals), all five single-case
   anchors eval-identical, m22 default-mode anchor eval-identical
   (18,779,890, `pv_status: cap-cut`), and
   `test_trajectory_golden --include-ignored` green **without**
   re-baselining the golden. (If this fails, the split has changed pacing
   semantics and must not ship.)
2. **Budget-mode contract holds**: every `solvable_evals:` /
   `unproven_evals:` fixture in `tests/fixtures/decisive_remaining.txt`
   and `smoke_positions.txt` green, `tests/test_cli.rs::cli_budget_exhaustion_returns_draw`
   green, `decisive_positions.txt` eval-budget entries green, and
   `tests/test_repetition.rs --include-ignored` green. Budget-mode
   trajectories may move (the enforcement point changed); if a fixture
   threshold fires, re-baseline that fixture only, with the new true value
   and a one-line justification in the report (the plan6 precedent).
3. **No wall-clock regression**: stress FO and m22 default-mode wall
   within +2% of Phase 0 (median of 3 runs each; the per-node check must
   not show up in profiles of the frame loop).
4. **Simplification is real**: `max_work`/`budget` entanglement gone
   (no doc or comment claims the slice enforces a budget), net line count
   of `src/search/dfpn/{mod,core}.rs` ≤ HEAD, `clippy` clean,
   `cargo doc` warnings unchanged.
5. `make test` green; `make test-full` green (required: search-semantics
   change per `AGENTS.md`).

If 1 fails → revert Phase 2 entirely (keep Phase 1's findings) and ship
D-c's documentation corrections only; the item closes as "plumbing split
no-go, docs corrected". If 2 fails → narrow the split (keep the round cap
in the slice, revert the budget move) and re-run 1–3; a second failure
reverts. If 3 fails → revert the check placement (per-frame-entry instead
of per-iteration) or drop the split.

## Tests to add

1. `restart_schedule::shipped_matches_legacy_formula` (pure, fast).
2. `restart_schedule::never_and_zero_cutoff_are_equivalent` (pure, fast).
3. `budget_overshoot_is_bounded`: for budgets {1k, 10k, 100k, 1M} on the
   m22 FEN (or a cheap fixture), assert the run stops with
   `child_evals <= budget + 256` and `ExitReason::BudgetExhausted`.
4. `bounded_search::cap_cut_and_resource_cut_attribution`: extend the two
   existing attribution tests with a case where the budget and the cap are
   equal (tie → `ResourceCut`, today's rule).
5. `restart_schedule::never_restarts`: with `RestartSchedule::NEVER` and
   no budget, a small fixture solves with `search.restarts() == 0`
   (guards the `u64::MAX` slice path), and `SHIPPED` on a >500k-eval
   fixture yields `restarts() >= 1`.

## Gates and validation order

1. Phase 0 anchors (stop-and-investigate on drift).
2. Phase 1 gate: S0 must reproduce Phase 0 exactly (the neutrality proof
   for the gate itself); then the grid; then D1.
3. Phase 2: `cargo fmt` → `cargo clippy --all-targets` → `cargo doc` →
   `make test` → D2 anchors (`test_trajectory_golden`, quick suite,
   single-case anchors) → eval-budget fixtures (`--include-ignored`) →
   `test_repetition --include-ignored` → `make test-full`.
4. Revert check for Phase 1's gate and a final `git diff`/`git status`
   review: the only surviving source changes are Phase 2's.

## Measurement artifacts

`docs/plans/conversion/measurements/plan12/`:

- `README.md` — provenance table (what each file is, which commit, how to
  reproduce, the fixed layout of the directory per `AGENTS.md`).
- `env.json` — commit hash, host, rustc/cargo versions, CPU count, TT
  sizes, epsilon, timeouts.
- `driver.py` — the arm × TT × case matrix runner (isolated copy; sets
  `sys.dont_write_bytecode`).
- `results/*.json` — parsed per-cell results (no raw transcripts; `logs/`
  and `*.log` stay out of the commit).

Commit only the above (per `AGENTS.md`'s measurement layout rules); prune
nothing from `src/`, and delete the temporary isolated copy after the run.

## Risks

1. **The restart schedule is an unprincipled hyperparameter.** No mechanism
   theory explains the 278× on dec31 or the dec10/m22 trade-offs; the
   measured curves are non-monotone in restart frequency. Mitigation: D1's
   "≥10% at both TT sizes + no outcome change + no new tail" bars, and the
   report records the schedule as a *measured* policy with an explicit
   "no guarantee across TT sizes" note. Do not add more schedule knobs.
2. **Phase-2 bit-identity is fragile.** It rests on the claim that today's
   `call_max_work == chunk` when no budget is set; any incidental
   restructure (e.g. moving the `chunk == 0` guard) breaks it. Mitigation:
   bar D2-1 is exact equality on the whole quick suite plus the golden; if
   it fails, do not "fix" the golden — revert.
3. **Budget-mode fixtures were calibrated under slice-scoped cuts.** A
   per-node check is slightly *looser* (no subtree starvation), so
   `solvable_evals` entries can only improve and `unproven_evals` entries
   could in principle flip to solved. Mitigation: bar D2-2 and the
   re-categorization rule already documented in `tests/common/mod.rs`.
4. **Session overrun.** Phase 1 (grid + parse) is ~40 min of wall work;
   Phase 2 is a ~1.5–2 h implementation with a full gate. Mitigation: the
   plan is explicitly splittable — if the session runs out after Phase 1,
   write `report12.md` covering Phase 0/1 + D1 and leave Phase 2 to
   `plan13.md` (same item #9, no renumbering needed for a *successor*
   plan). Record the split in the report.
5. **Interaction with `--refine-cap 0` / `--refine-cap` factors**: only the
   budget moves to the per-node check; the cap stays in the slice, so
   `--refine-cap` semantics are untouched. Verify the
   `--refine-cap 0` (uncapped) path still reaches
   `bounded_search(…, u64::MAX)` with `call_slice == chunk`.

## Deliverables

1. Phase 1 results + D1 verdict (with the grid table) and the reverted-gate
   proof.
2. Phase 2 (if reached): the `RestartSchedule` API, the per-node budget
   check, the re-derived `RoundTermination`, the doc corrections, the new
   tests, and a green `make test` / `make test-full`.
3. `docs/plans/conversion/report12.md` — final task of this plan. It must
   contain: the anchor reproduction, the grid, the D1/D2 verdicts against
   the pre-registered bars, the exact numbers that changed (with the
   re-baselined fixtures listed explicitly), tools/examples used, problems
   encountered, unresolved parts, missing tests, and next steps. If Phase 2
   was deferred, the report records the deferral and `plan13.md` is the
   next session's input.
4. Index/initiative updates: item #9 is already in
   `docs/plans/conversion/initiative.md` (status **drafted**, added by this
   plan session) and `docs/plans/README.md`'s `conversion` row carries the
   one-line reactivation pointer — the execution session updates **that
   row's status cell** to the measured verdict (`report12.md` linked) when
   the item closes, and adds the matching History entry.

## Expected verdicts (pre-registered expectations, not bars)

D1: keep S0 (no arm dominates). D2: ship, bit-identical on the unbudgeted
path, budget enforcement simplified and made exact. The session is a
success if the report can answer "what is the restart schedule worth, and
why does the code still have it?" with numbers.
