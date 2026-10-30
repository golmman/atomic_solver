# Provari Plan 3: quiet-move grouping (OR nodes only, opt-in)

Initiative: `provari`, backlog item 3b (second half of the split item 3).
Drafted 2026-10-29. Status: ready for execution. This plan changes search
behavior behind a **default-off** flag; it does not change any correctness
contract.

Prerequisite reading: [`initiative.md`](initiative.md), [`plan2.md`](plan2.md),
[`report2.md`](report2.md), `AGENTS.md`, and the OR/AND, child-evaluation, and
selection docs in `src/search/dfpn/{mod,children,selection,core}.rs`.

## Why this plan exists

Plan 2 adopted OR-node move-class starting proof numbers as an opt-in lever
and measured 8.74×/7.58× (work/wall) on the core FEN and 3.41×/3.18× on the
only shared `sample20` win. The initiative's remaining non-evaluation lever
from provari is **quiet-move grouping**: the attacker's plain moves wait as
one delayed child and are opened only when the search wants one. Provari's own
ablation attributes ~1.4× to it on the core FEN; plan3 measures the same idea
in our architecture, separately from and in combination with plan2's flag.

Plan 2 deliberately split item 3; this plan closes the second half. It is not
the static tactical closure (item 4), not EF-PN (item 5), and not the win-only
front-end (item 2).

## Plan-session probe (2026-10-29, read-only, not committed)

The provari oracle clone and its ablation binaries from plan1 still live in
`/tmp`. They were run again only to confirm the target size; no repository file
and no provari source was modified, and nothing is imported.

Command shape (hand guide, no bundled network; 10 s / 192 MB, 3 runs):

```sh
/tmp/provari_{base,nogroup} prove "<FEN>" --seconds 10 --memory 192
```

| Case | Binary | Wall (3 runs) | Nodes | Mate |
| --- | --- | --- | ---: | ---: |
| core | `base` (grouping on) | 0.534 / 0.529 / 0.522 s | 342,289 | 17 |
| core | `nogroup` | 0.717 / 0.709 / 0.719 s | 401,485 | 17 |
| shared `1eb80b92f92c` | `base` | 0.134 / 0.133 / 0.140 s | 72,523 | 15 |
| shared `1eb80b92f92c` | `nogroup` | 0.173 / 0.165 / 0.166 s | 71,677 | 14 |

Core: **1.35× wall, 1.17× nodes**. Shared: **1.23× wall**, but nodes are flat
(and `nogroup` finds a one-move-shorter line), so the shared case is not a
clean work signal. Conclusion for planning: the lever is real on the core
case and must be measured with the deterministic `child_evals` metric, not
assumed from provari's ablation. A neutral or negative result is a valid
outcome.

## Scope decision (one lever)

This plan implements **quiet-move grouping at OR nodes only**, behind a
default-off `--quiet-grouping` flag:

- one delayed group child represents the attacker's plain moves;
- the group is opened the first time DF-PN selects it;
- only then are the member moves evaluated and added as ordinary children.

Deliberately out of scope:

- **No group TT persistence.** Provari stores a synthetic group entry; this
  plan keeps the group frame-local. Member children are stored normally, so a
  restart can rediscover them when the group is reopened. If this proves to be
  the limiting factor, a follow-up plan may add a group key; it is not part of
  the one-lever measurement here.
- **No exact parent-side plainness.** The grouping predicate is a cheap,
  conservative parent-side approximation. A false positive only delays a
  forcing move until the group opens; it is not a soundness risk. All group
  members are reclassified by the existing `move_class` code when opened.
- No change to machine semantics outside the flag: terminal/repetition/rule50
  handling, the TT layout/store sites, proof events, budget accounting, and
  the AND-node path stay as shipped.

## Design

### 1. Group representation

Add one field to `ChildInfo`:

```rust
pub group: bool,
```

`ChildInfo::new_*` constructors set it to `false`; a new
`ChildInfo::group(pn, dn)` constructor sets it to `true`, with
`mv = Move::NONE`, `outcome = None`, `depth = 0`, `repetition_seen = false`,
`explored = false`. `ChildInfo` is internal (`pub` inside `dfpn`) so no public
API changes.

Invariants:

1. A group child is **never solved**: `outcome` stays `None`. Therefore
   `is_solved_by_children` cannot conclude a parent `Loss`/`Draw` while a
   group is pending. An OR `Win` via another child may still be concluded
   early, which is correct.
2. A group child is a **disjunction of its hidden moves at an OR node**. Its
   starting bounds are:
   - `pn = GROUP_PN = 9` (provari's `QUIET`);
   - `dn = hidden_count.clamp(1, INF)` (the number of legal plain moves).
   Neither is a proof; they are starting estimates in the same sense as
   plan2's class numbers.
3. A group child is never moved into. `mv = Move::NONE` and core intercepts
   it before the ordinary `mv == Move::NONE` break.
4. A group child never stores a best-move/best-child TT hint (see §4).
5. There is at most one group child per frame, appended after the evaluated
   non-plain children. Opening removes it permanently for that frame.

### 2. Parent-side plainness predicate

New module `src/search/dfpn/quiet_group.rs` (target < 10 KB; tests may live in
`quiet_group/tests.rs`) with:

```rust
pub(super) const GROUP_PN: u64 = 9;
pub(super) fn is_plain(pos: &Position, mv: Move) -> bool;
```

`is_plain` is a conservative superset of provari's `Mover::plain`: it may
return `false` for moves that could be grouped (losing some benefit), but it
must return `true` only for moves that are reasonably safe to delay.

Rules for `true` (all must hold):

- not a capture (`board.is_capture(mv)`), not en passant, not a promotion, not
  castling;
- the moving piece is not a commoner (king);
- the move does not give check, conservatively: the moved piece at its
  destination, under the post-move occupancy, does not attack an enemy
  commoner, and the moving piece's current square did not attack an enemy
  commoner before the move (the second test also covers the cheap
  discovered-check case);
- the move creates no king-explosion target, conservatively: the moved piece
  at its destination, under the post-move occupancy, does not attack a target
  square, and the moving piece's current square did not attack a target
  square before the move.

Use the same target shape as plan2 `move_class::threat_targets`: an enemy
non-commoner piece adjacent to an enemy commoner, not adjacent to one of our
commoners. Copy the small `attacks_from(piece_type, color, square, occupancy)`
helper locally if the ordering scorer's private helper cannot be reused; do
not change ordering scores.

Approximations to document in the module header:

- Moves that uncover a stationary check or threat through a line that does not
  originate on the moving piece's square can be misclassified as plain. That
  merely delays them; the group opens before a parent can be decided through
  them.
- The predicate does not require the destination square to be empty after the
  move beyond the capture rejection (normal quiet moves have an empty
  destination by construction).
- Hanging plain moves are grouped too; they receive the existing
  `HangingQuiet` starting number only when the group opens.

### 3. Grouped child evaluation

Extend `Search::evaluate_all_children` (or a sibling method called from it)
with a grouped path used only when `self.quiet_grouping && is_or_node`. The
ungrouped path stays byte-identical.

Grouped path:

1. Partition the already-legal `moves` with `quiet_group::is_plain` (the
   result may be cached in a small reusable `Vec<bool>` scratch field; do not
   allocate per child).
2. Skip the key pre-pass, TT prefetch, and `evaluate_child_keyed` call for
   plain moves. This is the saving: hidden moves must not increment
   `child_evals`, must not probe the TT, and must not emit proof events.
3. For non-plain moves, run the existing key pre-pass and evaluation exactly
   as shipped, including the early-decisive `Loss` cut. If a decisive child is
   found, fill the remaining non-plain slots with `ChildInfo::unevaluated` as
   shipped and **do not append a group** (the parent is already won).
4. If at least one plain move was hidden and no decisive child was found,
   append `ChildInfo::group(GROUP_PN, hidden_count)`.

The hidden members are recovered deterministically at open time by scanning
the same frame `moves` list with the same predicate. Do not store a separate
member vector or alter the move list.

### 4. Group opening

In the `dfpn` loop in `core.rs`, after a selection is computed and before the
solved/threshold/descent handling, intercept a selected group:

```text
if let Some(idx) = selection.best_child_index
    && children[idx].group
{
    self.open_quiet_group(pos, moves, max_depth, is_or_node, &mut children, idx);
    continue;                  // reselect over the opened children
}
```

`open_quiet_group` (new method in `children.rs` or a small sibling module):

1. Remove the group entry from `children`.
2. Scan `moves` with `quiet_group::is_plain`; for each hidden move call the
   existing `evaluate_child(pos, mv, max_depth, is_or_node)`. This bills one
   child evaluation per opened member, exactly like a shipped evaluation, and
   reuses all terminal/repetition/rule50/TT logic. If `move_class_pn` is on,
   the existing plan2 branch assigns the member's class number; if it is off,
   members get the shipped neutral bounds.
3. Append the evaluated members to `children`. Mirror the shipped early cut:
   if an OR-node child comes back `Outcome::Loss`, further group members need
   not be evaluated (the parent is won); stopping early is safe because
   `is_solved_by_children` already accepts a single `Loss` child.
4. Never recreate the group in that frame. If the group is selected again
   after opening, it is gone and ordinary selection proceeds.

Do not open a group during the initial `evaluate_all_children` sweep.

### 5. TT hints and stores

- `best_and_second_unsolved`, `selection_for_child`, and
  `is_solved_by_children` need no new logic: a group is an ordinary unsolved
  `ChildInfo` with a high starting `pn`, and it never has an outcome.
- At the frame-exit store, if the selected `best_move` is `Move::NONE` (which
  includes a still-closed group), store `best_move = Move::NONE` and
  `best_child = u8::MAX`. Change the current
  `children.position(|c| c.mv == store_best_move)` computation so a
  `Move::NONE`/group selection cannot store a bogus child index.
- Do not add a synthetic TT key for the group. Member entries are stored
  normally by their own frames / `evaluate_child` does not store; progress on
  members that were descended is stored by their own `dfpn` frames under
  their real keys.
- The existing TT pre-probe and `unsolved_bounds` are unchanged. A reopened
  group simply reevaluates its members, benefiting from member TT entries.

### 6. Budget and counters

- `is_plain`'s attack tests and any internal movegen are internal work and are
  **not** billed as child evaluations (same convention as plan2's
  classification).
- Evaluating an opened group member *is* a child evaluation and must go
  through `evaluate_child` so `child_evals`, the budget, restart slicing, and
  proof events keep their shipped meaning.
- The budget can be crossed by at most the remainder of one group opening
  (bounded by the legal move count), like an `evaluate_all_children` sweep.
  Document this alongside the existing budget note.

### 7. Configuration and defaults

- `Search::quiet_grouping: bool`, default `false`, `src/search/dfpn/mod.rs`.
- `Search::set_quiet_grouping(bool)` in `src/search/dfpn/config.rs`.
- `CliOptions::quiet_grouping: bool`, default `false`, `src/cli.rs`; parser arm
  `--quiet-grouping`; one-line help text in `src/main.rs::HELP` marked
  experimental/default off; wire in `main`.
- Extend the CLI help golden fixture and add parser tests for default-off and
  presence.
- Document the interaction: grouping is independent of `--move-class-pn`;
  measure both alone and together. Default remains off regardless of the
  measurement outcome unless the full gate and evidence are unambiguous.

## Tasks

1. **Read and confirm.** Re-read this plan, plan2/report2, and the
   `children.rs` pre-probe ladder, `selection.rs`, and `core.rs` loop.
   Confirm the scope: no TT group entry, no AND-node grouping, no static
   tactics, no EF-PN.
2. **Add the predicate.** Create `src/search/dfpn/quiet_group.rs` with
   `GROUP_PN`, `is_plain`, the local attack helper, and the documented
   approximations. Unit-test it on small FENs: a plain quiet move is grouped;
   a check, a capture, a promotion, castling, a king move, a single threat, a
   double threat, and a discovered-check/square-vacating move are not.
3. **Add the group representation.** Add `ChildInfo::group`, fix every
   constructor and test fixture, and unit-test the constructor's fields and
   `is_solved_by_children` behavior (a pending group cannot conclude Loss or
   Draw; another `Loss` child can still conclude Win).
4. **Implement grouped evaluation.** Add the flag-gated OR-node path in
   `evaluate_all_children`: skip keys/TT/evals for plain moves, preserve the
   early decisive cut, append at most one group. Test hidden moves are not
   billed, non-plain children match the ungrouped table, and AND nodes are
   unchanged.
5. **Implement opening.** Add `open_quiet_group` and the core intercept.
   Unit-test: opening evaluates exactly the hidden moves once, removes the
   group, respects the early Loss cut, applies the plan2 class numbers when
   that flag is on, and leaves other children untouched. Test a group
   selection does not descend into `Move::NONE`.
6. **Fix TT hints.** Exclude group/`Move::NONE` selections from stored
   best-move/best-child hints. Add a focused unit test for the store mapping.
7. **Wire the flag.** Add Search field/setter, `CliOptions` field/parser, help
   text/golden, and main wiring; parser tests for default-off and presence.
8. **Smoke and validate.** Build release. Run the core FEN and shared case
   with grouping off/on at short budgets; validate every decisive PV via the
   proof-tree path (`--tt-dump-path` + `reconstruct_pt`) as in plan2. A win
   that fails replay validation vetoes the lever.
9. **Measure.** Add `docs/plans/provari/measurements/plan3/` with
   `run_group.py`, `README.md`, and `results.json`. Reuse
   `../plan1/corpus.json` by path; never edit `sample20`.
10. **Run the gates.** `make test`, `make test-full`, the quick-suite drift
    check, and the repetition soundness gate
    (`cargo test --release --test test_repetition -- --include-ignored`).
    With the flag off, all existing trajectory/golden tests must pass
    unchanged.
11. **Write `report3.md`.** Include the configuration tables, all repeats and
    censoring, the interaction with `--move-class-pn`, validation results,
    gate results, approximations, and one explicit default decision.
12. **Update the initiative.** Record plan3's execution, close item 3b if
    appropriate, and state whether the next plan is item 4 (static tactical
    closure) or a pivot. Update `docs/plans/README.md` only if the initiative
    pivots or closes.

## Measurement protocol

Release build, one process at a time, no tablebases, 192 MB TT label. Corpus
is the committed `measurements/plan1/corpus.json`.

Configurations (four, so the interaction is visible):

| Name | Flags |
| --- | --- |
| `off` | shipped |
| `group` | `--quiet-grouping` |
| `mc` | `--move-class-pn` |
| `both` | `--quiet-grouping --move-class-pn` |

Command shape:

```sh
./target/release/atomic_solver --fen "<FEN>" --tt-size 192 \
  --first-outcome --outcome-only --timeout <T> [--budget <B>] [flags]
```

Runs:

- **Core FEN:**
  `--timeout 90 --budget 600000000`, 3 repeats per configuration. Report all
  repeats, medians, and whether `budget exhausted` or `timeout` fired. The
  shipped baseline needed ~62 s in plan2; 90 s avoids near-boundary censoring.
- **Shared case `1eb80b92f92c`:** `--timeout 10 --budget 300000000`, 3 repeats
  per configuration.
- **`sample20`:** `--timeout 10` (no explicit budget; plan1 parity), 1 repeat
  per configuration. Keep every row and report censoring.

Primary metric: `child_evals` to the first decisive outcome (deterministic).
Secondary: wall time from the driver, outcome, first-win length, `pv_status`,
exit reason. On `sample20`, coverage and the on/off delta. Budget-exhausted is
never reported as a proven draw.

Validation: for every decisive row, rerun the same configuration with
`--tt-dump-path`, rebuild with `reconstruct_pt`, and require
`root_outcome == measured outcome` and `validate: ok`; deduplicate by
`(case, config, PV)`. The informational `verify_ppv` check may be run for
completeness but is not the acceptance criterion (see report2's findings).

## Acceptance

- `--quiet-grouping` exists, defaults to off, is documented in `--help`, and
  is covered by parser/unit tests.
- With the flag off, `make test` and the existing trajectory/golden tests are
  unchanged; no behavior, TT layout, or proof-event change is detectable.
- With the flag on:
  - grouping is applied only at OR nodes and only to moves accepted by
    `quiet_group::is_plain`;
  - hidden moves are not counted as child evaluations and do not probe the
    TT before opening;
  - at most one group per frame, removed permanently on opening;
  - a pending group can never conclude a parent Loss or Draw;
  - opened members are evaluated through `evaluate_child` and are billed
    normally;
  - no group TT key, no bogus best-move/best-child hint, no proof event from
    the group itself;
  - every decisive measured win replays/validates.
- `measurements/plan3/` contains the driver, provenance README, and parsed
  `results.json`; no raw transcript, TT snapshot, or `__pycache__` is
  committed.
- `make test`, `make test-full`, the quick-suite drift check, and the
  repetition gate are green (or the report records a pre-existing failure
  with evidence).
- `report3.md` states one of: keep grouping default-off (negative/neutral),
  keep it opt-in and proceed, or adopt it as default (only with clear evidence
  and green gates).

## Validation / gates

- Default gate: `make test`.
- Search-behavior gate: `make test-full` (grouping changes selection and
  expansion order).
- Repetition: `cargo test --release --test test_repetition -- --include-ignored`.
- Drift: `cargo run --release --quiet --example benchmark -- --suite quick`
  (the invocation used by prior search plans), recorded verbatim.
- Replay: the proof-tree reconstruct/validate path from plan2; `verify_ppv`
  is secondary.

## Risks and mitigations

- **Grouping is neutral or negative.** With `--move-class-pn` on, quiet
  children already start at 9/15, so the selection effect may be small;
  grouping could still help by avoiding their evaluations, or hurt by
  delaying a winning quiet move. Measure all four configurations and report
  the interaction. Keep the flag off on a negative result.
- **The parent-side predicate is approximate.** False positives only delay
  moves until the group opens, so soundness is unaffected; document the
  approximation and make the predicate conservative. A false negative merely
  loses benefit.
- **Restart churn without a group TT key.** A frame that re-enters after a
  restart re-creates and may reopen the group. Member evaluations hit their
  real TT entries, so the cost is bounded. If measurements show restart churn
  dominating, report it as a finding; do not silently add a synthetic TT key
  in this plan.
- **Budget overshoot at open.** Opening can evaluate many plain moves at
  once. This is bounded by the legal move count and mirrors an
  `evaluate_all_children` sweep; document the bound and keep the per-node
  budget check unchanged.
- **Accidental descent into a group.** `Move::NONE` is already a sentinel in
  `core.rs`; intercept group selection before the ordinary break and add a
  regression test that no `do_move(Move::NONE)` happens.
- **Scope creep.** If the implementation starts needing an explosion
  detector, blast-aware exchange, TT group persistence, or AND-node grouping,
  stop and defer to item 4 or a follow-up plan.

## Report requirements

`report3.md` is the final task and must include:

- the predicate as implemented, with every approximation marked;
- the group representation, bounds, opening rule, and the proof that
  default-off is inert;
- the four-configuration core and shared repeat tables (all runs, medians,
  censoring);
- the `sample20` coverage table and deltas for each configuration;
- deterministic `child_evals` and wall comparisons, clearly separated, plus
  the group/mc interaction;
- replay-validation results for every decisive row;
- gate results (`make test`, `make test-full`, repetition, drift);
- a recommendation: keep off, keep opt-in and proceed to item 4, or adopt as
  default;
- explicit "not measured" items (group TT persistence, exact safe-evasion
  filters, EF-PN, tablebases, full `high200`).
