# Provari Plan 2: move-class starting proof numbers (OR nodes only, opt-in)

Initiative: `provari`, backlog item 3 (first half). Drafted 2026-10-29.
Status: ready for execution. This plan changes search behavior behind a
**default-off** flag; it does not change any correctness contract.

Prerequisite reading: [`initiative.md`](initiative.md),
[`plan1.md`](plan1.md), [`report1.md`](report1.md), `AGENTS.md`, and the
OR/AND and child-evaluation docs in `src/search/dfpn/{mod,core,children}.rs`.

## Why this plan exists

`report1.md` measured a latency gap, not a constant-factor Rust gap:

- on the core FEN (`r1b1kbnr/ppp3pp/n4q2/3pppN1/1P1PP3/8/P1P2PPP/RNBQKB1R w KQkq - 0 1`)
  provari proves in ~0.39 s median with its bundled model and ~0.52 s with
  its hand guide, while `atomic_solver --first-outcome` times out at 10 s
  (and needs ~60 s to find a win in this container);
- on the fixed `sample20`, provari wins 4/20 and our solver wins 1/20; the
  one shared win costs us ~65× provari's wall time.

`report1.md` recommends the **smaller move-class initialization spike** next,
not the attacker-only front-end refactor. This plan executes that
recommendation.

## Scope decision (one lever)

Backlog item 3 is a deliberately coupled pair: move-class starting numbers and
quiet-move grouping. This plan executes **only the first half** and splits the
item:

- **plan2 (this plan):** OR-node move-class starting proof numbers.
- **plan3 (next):** quiet-move grouping (the delayed plain-move child), measured
  separately.
- Deferred to item 4 / the static-tactical-closure plan: the *safe*-evasion
  filter for checks, the "double threat no defence parries" static win, and
  any other expansion-avoiding shortcut.
- Deferred to item 5: evaluation-scaled proof numbers (EF-PN) and any learned
  guide.

Reason for splitting: the two halves have different failure modes. Starting
numbers change DF-PN thresholds and selection; quiet grouping changes child
enumeration and the move list itself. Measuring them together makes a
negative result uninterpretable. The initiative's working agreement allows one
mechanism per plan; this is that one mechanism.

The general solver remains authoritative for draws/losses. The flag changes
only the *initial estimate* for newly seen non-terminal children; it never
changes terminal classification, outcomes, repetition handling, or proof-tree
events.

## Plan-session probe (2026-10-29, `/tmp`, not committed)

A prior sizing spike (`/tmp/atomic_init`, a throwaway copy of the tree) added
only this line to `evaluate_all_children`:

```rust
// EXPERIMENT: rank-initialized proof numbers for newly seen children.
if info.outcome.is_none() && info.pn == 1 && info.dn == 1 {
    info.pn = i as u64 + 1;   // i = move-order rank
}
```

One core run at `--timeout 60 --first-outcome --outcome-only --tt-size 192`
(then-current release build) timed out, while the unmodified tree won at
59.65 s in the same probe. The 60 s window is too tight for a firm ratio, and
this is a single run, but the direction is clear enough for planning:

- **move-order rank is not a proxy for move class.** Ranking the first scored
  move at `pn = 1` and the ninth at `pn = 9` mainly deepens the scorer's top
  choice; it does not promote checks/threats as a class.
- The plan must therefore implement the class rules below, not reuse the rank
  spike, and must measure with a deterministic budget in addition to wall
  time.
- The initiative record already notes that a separate immediate-explosion
  shortcut prototype did not move the 40 s needle. Static tactics are item 4;
  this plan does not retry them.

## Design

### Class table (attacker/OR-node moves only)

For a parent that is an **OR node** (the side to move is the side whose win is
being proved) and a newly seen, non-terminal child, assign the child's
starting proof number `pn` from the parent move's class. The child is then an
AND node; `pn` is still expressed in root-prover orientation, exactly like
every other `ChildInfo` bound. Leave `dn = 1` (the existing neutral value) for
all classes.

Constants mirror provari's published starting numbers (`src/dfpn.rs` in the
oracle, consulted read-only; no code imported):

| Class | Condition (after playing the move) | Starting `pn` |
| --- | --- | ---: |
| Check | the move gives check | `min(legal_evasions, 8)`, at least 1 |
| Double threat | ≥ 2 distinct king-explosion targets and the mover is not immediately lost | 1 |
| Threat | exactly 1 target | 4 |
| Weak threat | exactly 1 target, but the moved piece is cheaply blastable by the defender | 8 |
| Hanging quiet | no check/threat, and the moved piece is cheaply blastable | 15 |
| Quiet | anything else | 9 |

Definitions for the implementation:

- **King-explosion target:** an enemy non-commoner piece adjacent to the
  enemy commoner, not adjacent to our own commoner, that one of our
  non-commoner pieces attacks after the move. Count distinct target squares.
  (This is the count used by provari's `threats`/`exploding_captures`; our
  ordering already computes the same shape for one target in
  `src/search/ordering.rs` §4–§5 and may be refactored into a shared helper
  without changing scores.)
- **Legal evasions:** the number of legal replies in the child position,
  capped at 8. This plan deliberately counts *all* legal replies, not only
  replies that survive an immediate explosion. The safe-reply filter needs the
  explosion detector and is item 4; document the approximation in the module
  header and in the report.
- **Cheaply blastable:** the defender has at least one non-commoner attacker
  on the moved piece's square and either that square is undefended by us or
  the cheapest attacker is worth no more than the moved piece. If the existing
  atomic capture-net helper can be exposed without changing ordering scores,
  reuse it; otherwise use this conservative attack/defense test and document
  the choice. Blast-aware exchange precision is item 4, not this plan.
- **Immediately lost:** the defender has a legal move that explodes our
  commoner at once. A cheap conservative check is acceptable here; never
  upgrade a double threat to `pn = 1` when this is true.
- **Plain/quiet:** any legal move that is not a check and does not create a
  target. Captures and promotions are not special-cased for this plan; if they
  create no target they follow the quiet/hanging rule, matching provari's
  attribution of `QUIET` to non-plain non-threatening moves.

Every value is in `[1, 15]`. The implementation must never produce `0` or
`INF` here: `Children::unsolved_bounds`'s non-degeneracy invariant and the
TT pre-probe rely on positive, finite bounds.

### Where the initialization is applied

Add a small module `src/search/dfpn/move_class.rs` (target < 10 KB; unit tests
may live in `move_class/tests.rs` or inline):

- `pub(super) enum MoveClass` (or an equivalent struct) with the table above.
- `pub(super) fn classify_and_start_pn(pos, mv, child_state, opts) -> Option<u64>`
  or split into `classify` + `starting_pn`.
- A `#[cfg(test)]` helper that returns the class and the number, so unit tests
  pin each branch.

Apply it in the final **unsolved** branch of
`Search::evaluate_child_played` in `src/search/dfpn/children.rs`, where the
child board is still live after `do_move_with_scratch` and before
`undo_move_with_scratch`:

```text
let (pn, dn) = ChildInfo::unsolved_bounds(entry, child_max_depth);
// new, flag-gated:
if self.move_class_pn && !child_is_or && pn == 1 && dn == 1 {
    let start = move_class::starting_proof(pos, mv, &child_state);
    return ChildInfo::unsolved(mv, start, 1);   // then undo and return
}
ChildInfo::unsolved(mv, pn, dn)
```

Invariants to preserve and test:

1. **Default off is byte-identical.** With `move_class_pn == false`, skip the
   classification call entirely; the code path and values are the shipped
   ones. Add a unit test pinning the default and a regression test that a
   small position's `evaluate_all_children` table is unchanged when the flag
   is off.
2. **Never overwrite learned or TT-reused bounds.** The `pn == 1 && dn == 1`
   guard is the neutral/fresh signal, matching the existing
   `unsolved_bounds` contract. A child with any other non-degenerate bounds
   keeps them.
3. **Never touch solved outcomes.** Terminal, repetition, rule50, and
   TT-resolved children return before this branch; do not move the branch
   above them.
4. **OR parents only.** Apply when the parent is OR (`!child_is_or`). AND-node
   children keep `(1, 1)`. Provari's defender-side `UNLIKELY = 2` disproof
   heuristic is out of scope.
5. **First encounter only.** The pre-probe branch that reuses an
   outcome-less TT entry without playing the move keeps its bounds; do not
   add a second classification path there.
6. **No effect on the TT.** The initialized `pn`/`dn` are ordinary
   `ChildInfo` bounds; the child's own frame still stores whatever it learns.
   Do not change `unsolved_bounds`, `TtEntry`, or any store site.
7. **No effect on proof events.** `emit_proof_node` and the event stream stay
   exactly as shipped.
8. **No budget accounting change.** Classification runs inside the existing
   child evaluation; it must not increment `child_evals` or the pre-phase
   counters. If the implementation adds a movegen for evasion counting, that
   movegen is internal work and is not billed as a child evaluation.

### Configuration surface

- `Search::move_class_pn: bool`, default `false`, in `src/search/dfpn/mod.rs`.
- `Search::set_move_class_pn(bool)` in `src/search/dfpn/config.rs`.
- `CliOptions::move_class_pn: bool`, default `false`, in `src/cli.rs`; parser
  arm `--move-class-pn`; help text in `src/main.rs::HELP` (one line, default
  off). Add parser tests for presence/default and an unknown-option case is
  already covered by the existing table.
- Wire it in `src/main.rs` next to `set_no_preflight`/`set_child_eval_budget`.
- Update the module/CLI header text so the new flag is discoverable, and note
  that it is an experimental heuristic, not a correctness switch.

## Tasks

1. **Read and confirm.** Re-read this plan, `report1.md`, and the
   `children.rs` / `core.rs` OR/AND docs. Confirm the split: no quiet
   grouping, no safe-evasion filter, no EF-PN.
2. **Add the classifier.** Create `src/search/dfpn/move_class.rs` with the
   class table, the target-count helper, the evasion counter (capped at 8),
   and the cheap-blast test. Document the approximations and the fact that
   these are heuristic bounds, never soundness claims. Keep the module under
   ~10 KB; split tests if needed.
3. **Unit-test the classifier.** Pin each class on small FENs: a check with a
   known reply count, a double threat, a single threat, a hanging quiet move,
   a plain quiet move, and an AND-side position that must not be classified.
   Test that every returned number is in `[1, 15]`.
4. **Wire the flag.** Add the search field/setter, `CliOptions` field/parser,
   help text, and `main.rs` wiring. Add parser tests for default-off and
   `--move-class-pn`.
5. **Apply the initialization.** Add the flag-gated branch in the final
   unsolved path of `evaluate_child_played` with the seven invariants above.
   Add unit tests for: default off leaves `(1, 1)`; AND parent leaves
   `(1, 1)`; non-neutral TT-reused bounds are not overwritten; a solved child
   is untouched; a check child gets the capped evasion count.
6. **Smoke and validate.** Build release. Run the core FEN with the flag off
   and on at a short budget; if the on-run returns a win, validate the UCI PV
   with `examples/verify_ppv` (or the existing replay/verify path). A claimed
   win that does not replay is a veto, not a performance result.
7. **Measure.** Add `docs/plans/provari/measurements/plan2/` with
   `run_spike.py`, `README.md`, and `results.json` (see protocol below). The
   driver reuses `../plan1/corpus.json` by path; do not edit `sample20`.
8. **Run the gates.** `make test`, `make test-full`, the quick-suite drift
   check used by previous search plans, and the repetition soundness gate
   (`cargo test --release --test test_repetition -- --include-ignored`). With
   the flag off, existing trajectory goldens must pass unchanged.
9. **Write `report2.md`.** Include the measured tables, the class-distribution
   or expansion observation if available, the adoption recommendation, the
   approximations used, and the explicit decision for the flag default.
10. **Update the initiative.** Record plan2's execution and whether item 3 is
    half-done/closed, and whether plan3 should be quiet grouping or a pivot.
    Update `docs/plans/README.md` only if the initiative pivots or closes.

## Measurement protocol

All runs are release builds, one process at a time, no tablebases, 192 MB TT
label. Use the committed `measurements/plan1/corpus.json`; never re-pick
`sample20`.

Configurations:

```sh
# baseline (shipped)
./target/release/atomic_solver --fen "<FEN>" --tt-size 192 \
  --first-outcome --timeout <T> --budget <B>

# spike
./target/release/atomic_solver --fen "<FEN>" --tt-size 192 \
  --first-outcome --timeout <T> --budget <B> --move-class-pn
```

Primary metric: `child_evals` parsed from the stderr `evals: <n>` line (both
modes print it). Secondary: wall time measured by the driver, outcome, first
win length from `pv:`, `pv_status:`, and `pre_exit:`/exit reason. Budget-
exhausted is reported as such, never as a proven draw.

Runs:

- **Core FEN:** `--timeout 90 --budget 600000000`, 3 repeats per
  configuration. The plan-session baseline needed ~60 s; 90 s keeps the
  comparison from being decided by a near-boundary censoring. Report all
  repeats, medians, and whether `budget exhausted` or `timeout` fired.
- **Shared case `1eb80b92f92c`:** `--timeout 10 --budget 300000000`, 3
  repeats per configuration. This is the only plan1 shared win and the
  smallest latency target.
- **`sample20`:** `--timeout 10` (no explicit budget; keep plan1 parity),
  1 repeat per configuration. Report coverage, censored rows, and the
  on/off delta. Do not drop a row.

If a configuration wins on any case, replay-validate that case's PV before
using it in a ratio. A win that fails validation vetoes the lever.

## Acceptance

- `--move-class-pn` exists, defaults to off, is documented in `--help`, and
  is covered by parser/unit tests.
- With the flag off, `make test` and the existing trajectory/golden tests are
  unchanged; no behavioral or TT-layout change is detectable.
- With the flag on, no produced bound is `0` or `INF`; AND children remain
  neutral; solved/TT-reused children are untouched.
- The classifier has unit tests for every class in the table and for the
  AND-node skip.
- `measurements/plan2/` contains the driver, provenance README, and parsed
  `results.json`; no raw transcript, TT snapshot, or `__pycache__` is
  committed.
- Any claimed win from the on-config replays/validates.
- `make test`, `make test-full`, the quick-suite drift check, and the
  repetition gate are green (or the report records a pre-existing failure
  with evidence).
- `report2.md` states one of: keep default-off (negative/neutral), keep
  opt-in and proceed to plan3, or adopt as default (only if the evidence is
  clear and the gates are green).

## Validation / gates

- Default gate: `make test`.
- Search-behavior gate: `make test-full` (required because starting numbers
  change selection and TT-bound trajectories).
- Repetition: `cargo test --release --test test_repetition -- --include-ignored`.
- Drift: the quick-suite invocation used by the previous search plans
  (`cargo run --release --example benchmark -- --suite quick` or the then-
  current equivalent), recorded verbatim in `report2.md`.
- Replay: `cargo run --release --example verify_ppv -- <FEN> <moves...>` (or
  the documented equivalent) on each claimed win.

If `make test-full` exposes a failure that also reproduces on the unmodified
tree, record it as pre-existing and do not silently waive it.

## Risks and mitigations

- **The lever is neutral or negative.** The rank probe already warned that
  initialization is not automatically helpful. Keep the flag default-off and
  report a negative result; the next step is quiet grouping (plan3) or the
  static closure (item 4), not a quiet retry.
- **Evasion counting is expensive.** Cap at 8 and only count for check moves;
  count legal replies, not safe replies, so no explosion detector is needed.
  If the cost shows up in `child_evals`/wall, report it and consider a cheaper
  bound rather than moving the safe filter into this plan.
- **The class rules are approximations.** Document each approximation in the
  module header. The report must distinguish "provari's rule" from "our
  approximation"; do not claim parity.
- **TT-bound contamination.** Initialized values can be reused from TT like
  any other bounds. The `pn == 1 && dn == 1` guard and positive range keep
  them compatible with `unsolved_bounds`; the full gate catches trajectory
  drift. Do not change store sites to special-case these values.
- **Near-boundary core timing.** The 60 s baseline was ~59.65 s. Use 90 s and
  the deterministic `--budget`; report censoring honestly instead of
  declaring a ratio from one run.
- **Scope creep into item 4.** If the implementation starts needing an
  explosion/exchange detector to define "cheaply blastable" or "safe evasion",
  stop at the conservative approximation and defer the precision to item 4.

## Report requirements

`report2.md` is the final task and must include:

- the exact class table as implemented, with every approximation marked;
- the flag/config surface and the proof that default-off is inert;
- the core and shared-case repeat tables (all runs, medians, censoring);
- the `sample20` coverage table and on/off delta;
- the deterministic `child_evals` comparison and the wall comparison,
  clearly separated;
- replay-validation results for every claimed win;
- gate results (`make test`, `make test-full`, repetition, drift);
- a recommendation: keep default-off, keep opt-in and proceed to quiet
  grouping, or adopt as default, with the evidence;
- explicit "not measured" items (quiet grouping, safe evasions, EF-PN,
  tablebases, full `high200`).
