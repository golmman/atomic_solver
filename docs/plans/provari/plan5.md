# Provari Plan 5: evaluation-scaled proof numbers (EF-PN), opt-in

Initiative: `provari`, backlog item 5. Drafted 2026-10-30. Status: ready for
execution. This plan changes search behavior behind a **default-off** flag; it
does not change any correctness contract.

Prerequisite reading: [`initiative.md`](initiative.md), [`report2.md`](report2.md),
[`report3.md`](report3.md), [`report4.md`](report4.md), `AGENTS.md`, and the
child-initialization / selection docs in
`src/search/dfpn/{children,selection,move_class,quiet_group}.rs`.

## Why this plan exists

Plans 2 and 3 turned the core FEN from 437.2 M child evals / 60.1 s into
27.9 M / 4.15 s with `--move-class-pn --quiet-grouping` (the "best" stack);
plan 4a (node-entry static closure) measured **negative/neutral** and is not
part of the stack. The remaining work gap is node count, not per-node cost: on
the one shared `sample20` win our best stack spends 347,559 nodes / 0.74 s
while provari proves the same win in 72,523 nodes / 0.09 s.

Report 4's diagnosis is that short-circuiting frames throw away the ordering
feedback DF-PN+ needs. The remaining provari lever that *prices subtrees inside
DF-PN* instead of skipping them is **EF-PN** (evaluation-scaled proof numbers,
Winands & Schadd as provari implements it): the parent's static evaluation
scales its fresh children's starting proof and disproof numbers. Provari's own
ablation (re-measured in this plan session, see below) attributes ~1.96× fewer
nodes to its hand guide on the core FEN.

This plan implements **the hand guide only**. The learned provability model /
NNUE is a non-goal of the initiative.

## Plan-session probe (2026-10-30, read-only, not committed)

The pinned provari oracle (`48686bc6c786d6ea70568f0b2ad4002cf0520206`, `v0.1.0`)
was rebuilt read-only in `/tmp/provari-src` (`cargo build --release`, ~21 s) to
size EF-PN's hand guide. Command shape:

```sh
/tmp/provari-src/target/release/provari prove "<FEN>" --seconds 10 --memory 192 --guide hand
/tmp/provari-src/target/release/provari prove "<FEN>" --seconds 10 --memory 192 --guide none
```

3 repeats each; provari's node counts are identical across repeats (its `ms`
varies by ±3 %), so the node ratio is the signal.

| Case | `--guide hand` | `--guide none` | hand vs none |
| --- | --- | --- | --- |
| core `r1b1kbnr/…` | 342,289 nodes · 0.403 s · mate 17 | 670,136 · 0.742 s · mate 16 | **1.96× fewer nodes, 1.84× faster** |
| shared `1eb80b92f92c` | 72,523 · 0.089 s · mate 15 | 31,953 · 0.041 s · mate 15 | **2.27× more nodes, 2.17× slower** |
| `db6a741c723f` | 4,574,912 · 5.06 s · mate 22 | 5,789,506 · 5.90 s · mate 24 | 1.27× fewer nodes, 1.17× faster |
| `b9a7ed7eb368` | censored (10.1 M) | censored (10.4 M) | — |
| `dd3f89766a40` | censored (9.3 M) | censored (9.5 M) | — |

Two findings for planning:

1. **The lever is real but case-dependent.** Provari's own hand guide is a
   ~2× win on the core FEN and a >2× *regression* on the one case that is a
   shared win for us. The plan therefore pre-registers per-case floors (below)
   instead of assuming the core-FEN ablation transfers, and it measures the
   *same stack* we ship (move-class + grouping) rather than a bare baseline.
2. **Both guides lose `b9a7ed7eb368`/`dd3f89766a40`, which plan1 won with the
   bundled network.** The learned model is load-bearing on that part of the
   corpus; it stays out of scope, and the censored rows are reported, never
   dropped.

Our own baseline was re-confirmed in the same session (release build, best
stack, shared case): 5,568,743 child evals · 347,559 nodes · 0.74 s — identical
to `report4.md`.

No repository file was changed by the plan session; nothing from provari is
imported, copied, or committed.

## Scope decision (one lever, two pre-registered arms)

One mechanism: **new children's starting proof/disproof numbers are scaled by
their parent's hand evaluation**, behind a default-off `--eval-pn <mode>`:

- `off` (default) — shipped behavior, byte-identical.
- `both` — provari's formula at both node kinds (attacking = the parent is an
  OR node; defending = the parent is an AND node).
- `or` — the attacking branch only (AND-parent children keep the neutral
  `(1, 1)`).

The `or` arm exists because the probe's hypothesis is that the shared-case
regression comes from provari's *defending* branch, which raises every fresh
OR child's proof number by `a·(2 − step)` (up to +21) even when the defender is
clearly worse off. Two arms of one mechanism, both pre-registered, are in scope;
anything else (a graded/logit step, a network, a learned model, per-node
tuning knobs) is not.

Deliberately out of scope: item 4b (`exposed`/no-safe-evasion/double-threat
closures — the static-closure line is not ranked next, see "Ranking" below),
item 2 (win-only front-end), item 6 (rule50-free keying + proof length/limit),
item 7 (convergence), the NNUE/provability model, and any change to move
ordering, grouping, or the static-tactics hook.

## Design

### 1. The hand evaluation (`src/search/dfpn/eval_guide.rs`)

A new module, self-contained (no dependency on `ordering`, on `move_class`, or
on `quiet_group`), with tests in `eval_guide/tests.rs` if it grows past ~10 KB.

```rust
/// Atomic piece values (provari's Atomic-Stockfish midgame/endgame blend).
const VALUES: [i32; 6] = [305, 545, 634, 930, 1670, 0]; // P N B R Q commoner
const TARGET: i32 = 400;      // one attacked enemy piece beside their commoner
const TARGETS: i32 = 1000;    // two or more
const FUSE: i32 = 60;         // each of their pieces beside their commoner
const PRESSURE: i32 = 20;     // each of our attacks on a square beside it
const CHECKER: i32 = 60;      // each of our pieces that can check next move

/// The evaluation for `side`, from `side`'s perspective.
pub(super) fn evaluation(board: &Board, side: Color) -> i32;

/// EF-PN's `step`: 0 = the OR side is clearly behind, 1 = unclear,
/// 2 = the OR side is clearly ahead. `DECISIVE` = 500 centipawns.
pub(super) fn step(board: &Board, or_side: Color) -> u8;
```

Terms, per side, mirroring provari's `src/eval.rs` (read-only reference):

- **material** — sum of `VALUES` over the side's non-commoner pieces;
- **targets** — squares in the union of the enemy commoners' king rings that
  hold an enemy non-commoner piece and are attacked by one of our
  non-commoner pieces, capped at 2 (a capture there explodes the commoner);
- **fuses** — enemy non-commoner pieces standing in that ring (a fuse our
  attack can light);
- **pressure** — the number of (our non-commoner piece, ring square) attack
  pairs;
- **checkers** — our non-commoner pieces that could check the enemy commoner
  from a square they reach next move.

Score = material + `TARGET`/`TARGETS` + `FUSE`·fuses + `PRESSURE`·pressure +
`CHECKER`·checkers, and `step = 0/1/2` from `clamp(score, −500, +500)` split
at `±DECISIVE` (provari's `band`).

Deliberate approximations, all documented in the module header and covered by
tests:

- **Commoners instead of kings.** Provari's variant has one king per side; our
  board can carry several commoners. The rings are the *union* over the
  side's commoners and the attack terms are dropped for a side whose commoner
  stands adjacent to an enemy commoner (provari's "touching kings" exclusion;
  with touching commoners neither can be checked or exploded safely). For the
  single-commoner norm both readings coincide — every corpus fixture is of that
  shape.
- **Per-piece reach, not `attackers_to` per square.** `reach` is generated with
  the crate's attack tables (`knight_attacks`, `magic::{bishop,rook,queen}_
  attacks`, `pawn_attacks`, `king_attacks`) in one pass over the side's
  pieces, so `targets`/`pressure`/`checkers` come out of the same bitboards
  provari intersects. Any disagreement with an `attackers_to`-based reading is
  an evaluation difference, never a soundness one.
- **No blast-aware material.** Provari's `VALUES` are variant-blended
  constants, not our tunable `ScorerParams`; the guide must not inherit the
  external optimizer's parameter surface.
- **Fixed constants, no CLI knobs.** `EF_A = 10`, `EF_B = 5`, `DECISIVE = 500`
  are provari's shipped values, documented as constants. Tuning them is a
  later (item 7) question, not this plan's.
- **The evaluation is never a claim.** It is a starting estimate for
  selection. It can never create an outcome, never touch the TT by itself, and
  never overrides a learned or TT-reused bound.

### 2. The scaling rule (provari's `scale`, verbatim in our numbering)

With `step ∈ {0,1,2}` provari's linear branch is exact integer arithmetic
(the logit/network branch is unreachable for a 3-valued step and is not
implemented):

| Parent | number | `step = 0` | `step = 1` | `step = 2` |
| --- | --- | ---: | ---: | ---: |
| OR ("attacking") | proof `pn × (1 + EF_B·(2 − step))` | ×11 | ×6 | ×1 |
| OR ("attacking") | disproof `dn × (1 + EF_A·step)` | ×1 | ×11 | ×21 |
| AND ("defending") | proof `pn + EF_A·(2 − step)` | +21 | +11 | +1 |
| AND ("defending") | disproof `dn × (1 + EF_B·step)` | ×1 | ×6 | ×11 |

(`step = 2` means the OR side is clearly ahead.) Implement as one `Copy` value
so the parent computes it once:

```rust
#[derive(Clone, Copy)]
pub(super) struct ChildScale {
    proof_mul: u64,
    proof_add: u64,
    disproof_mul: u64,
    disproof_add: u64,
}

impl ChildScale {
    /// The scale a node of this kind applies to its fresh children, or `None`
    /// when EF-PN is off / disabled for this parent kind.
    pub(super) fn for_parent(is_or_node: bool, step: u8, mode: EvalPnMode) -> Option<Self>;

    /// Never returns 0 and never overflows; the magnitudes stay ≤ 15 × 21.
    pub(super) fn apply(self, pn: u64, dn: u64) -> (u64, u64);
}
```

Note the asymmetry that `or` mode tests: the AND branch *adds* to the proof
number of an already-provable-anything child, while the OR branch only
*multiplies*.

### 3. Which children are scaled

A child is scaled iff its bounds are **fresh**, defined exactly as plan 2
defined it for move-class numbers: the TT-reuse result was still the neutral
`(1, 1)` before any heuristic rewrote it. Solved, repetition, rule50,
TT-resolved, and previously scaled children are never touched — the ordering
guarantees the same non-override property plan 2 relies on.

Fresh bounds are decided at exactly one place,
`Search::evaluate_child_played`'s tail (`src/search/dfpn/children.rs`, the
`unsolved_bounds` → `move_class::starting_proof_number` branch). The tail
becomes:

```text
let neutral = pn == 1 && dn == 1;
let (pn, dn) = if neutral && self.move_class_pn && !child_is_or {
    (move_class::starting_proof_number(pos, mv), 1)   // plan 2, unchanged
} else {
    (pn, dn)
};
let (pn, dn) = match scale if neutral { scale.apply(pn, dn) } else { (pn, dn) };
```

so EF-PN multiplies the plan-2 class number (the composition provari's own
stack has) and the neutral `(1, 1)` at the other node kind. A neutral child
that is re-evaluated later is re-scaled to the same values, so the pass is
deterministic under re-visits.

**The quiet-move group child** (plan 3) is created outside that tail
(`evaluate_all_children_grouped`) and is always fresh within a frame; it is
scaled by the parent's scale in `both`/`or` mode, matching provari, which
scales its group child like any other fresh child.

### 4. Where the step is computed (one evaluation per frame visit)

The scale belongs to the *parent*, so it is computed once per frame visit in
`Search::dfpn` (`core.rs`) at the point where `is_or_node` and `pos` are
already known — after the terminal/depth/repetition/TT fast paths, before the
children loop — and threaded down explicitly (no `Search` field, no hidden
frame state; the same style as `is_or_node`):

```rust
let scale = if self.eval_pn.is_enabled() {
    eval_guide::ChildScale::for_parent(
        is_or_node,
        eval_guide::step(pos.board(), if is_or_node { pos.side_to_move() } else { pos.side_to_move().flip() }),
        self.eval_pn,
    )
} else {
    None
};
```

`ChildScale` (`Copy`, `Option`) is then passed as one extra parameter to
`evaluate_all_children`, `evaluate_all_children_grouped`, `open_quiet_group`,
`evaluate_child`, `evaluate_child_keyed`, and `evaluate_child_played`.

`or_side` is the side to move at the **root** — the OR side at even plies —
i.e. provari's `attacker`. At an AND frame the OR side is the side *not* to
move, hence the `flip()`.

Cost, stated up front: one `step()` call per frame visit when the flag is on
(provari pays the same). The report must quantify the tax from the censored
`sample20` rows (`child_evals` at a fixed wall ≈ throughput, so
`evals(eval)/evals(best) ≈ 1/(1 + tax)`); no extra code is added for
measurement. If the tax alone is ≥10 %, the report records it (a lazy
"only when a fresh child exists" step is the named follow-up, not in scope).

### 5. Budget and counters

`step()` and `apply()` are internal work: no child evals, no restart slice, no
budget. A node visit bills exactly what it billed before. Runs with the flag on
change *which* children DF-PN selects and therefore how many child evals the
search needs — that is the lever — but the counter semantics are untouched.

### 6. Configuration, defaults, and CLI

- `EvalPnMode { Off, Both, Or }` with `is_enabled()`; `src/cli.rs` plus the
  search config.
- `Search::eval_pn: EvalPnMode` (default `Off`), `Search::set_eval_pn(mode)`
  in `src/search/dfpn/config.rs`.
- `CliOptions::eval_pn: EvalPnMode` (default `Off`), parser arm
  `"--eval-pn" => { let v = value(args, &mut i, "--eval-pn")?; ... }` accepting
  exactly `off|both|or` and erroring otherwise (`error: --eval-pn must be one
  of off, both, or, got '<v>'`), one `HELP` entry marked
  experimental/default off, and `main` wiring.
- Parser tests: default `off`, `both`, `or`, and a rejected value. Help-text
  test updated if one pins the option list.

## Tasks

1. **Read and confirm.** Re-read this plan, `report4.md`, `report3.md`, and the
   fresh-bounds/`move_class` tail in `children.rs`. Confirm the scope: hand
   evaluation only, two arms (no graded step, no network, no item-6 or
   static-tactics changes).
2. **Add the evaluation module.** `src/search/dfpn/eval_guide.rs` with
   `evaluation`, `step`, `ChildScale`, and the documented approximations.
   Unit-test:
   - material-only positions score by the `VALUES` difference and `step` is
     `2` / `0` for a clearly-winning / clearly-losing side;
   - `step` is exactly `1` for a balanced position and monotone in the score;
   - one attacked enemy piece beside the enemy commoner adds `TARGET`, two add
     `TARGETS` (and a third adds nothing more);
   - a "fuse" with no attacker adds only `FUSE`;
   - an adjacent-commoner (touching) position drops the attack terms;
   - **the step is computed for the OR side**: the same board yields
     complementary steps for the two colours (this is the sign-convention pin);
   - a multi-commoner position uses the union of the rings and stays
     finite/deterministic.
3. **Add the scale.** `ChildScale::for_parent` for both parent kinds and both
   modes, plus `apply`. Unit-test the exact factor table (`step` 0/1/2 × both
   parent kinds × `both`/`or`), the `off`/`or` identity cases, and
   non-degeneracy (`pn ≥ 1`, `dn ≥ 1`, no overflow via `u64::MAX` inputs).
4. **Thread the scale.** Add the parameter through the six call sites; in
   `evaluate_child_played`'s fresh tail apply it after the move-class rewrite
   (only when the bounds were neutral); scale the group child in
   `evaluate_all_children_grouped`; scale the members appended by
   `open_quiet_group`. Tests:
   - `off` mode: identical nodes and child evals on a small fixture, and the
     shipped trajectory tests pass unchanged;
   - `both` mode on a fresh-child fixture: the OR-child bounds are the class
     number × the OR factor, the AND-child bounds are the AND factor table;
   - `or` mode: AND-parent children stay `(1, 1)` while OR-parent children are
     scaled;
   - a TT-resolved / solved child is never rescaled;
   - the group child carries the scaled numbers and opening it keeps the
     scaled member bounds.
5. **Wire the flag.** `EvalPnMode`, `Search` field/setter, `CliOptions`
   field/parser/`HELP`/`main` wiring, parser tests, and a compile-time default
   of `Off`.
6. **Smoke and validate.** Build release; run the core FEN and the shared case
   with each mode at a short budget; validate every decisive PV through
   `--tt-dump-path` + `reconstruct_pt` (`validate: ok`, `root_outcome` equal to
   the measured outcome). A claimed win that fails replay validation vetoes
   that arm.
7. **Measure.** Add `docs/plans/provari/measurements/plan5/` with
   `run_eval_pn.py` (adapt `../plan4/run_static.py`), a provenance `README.md`,
   and `results.json`. Reuse `../plan1/corpus.json` by path; never edit
   `sample20`.
8. **Run the gates.** `make test`, `make test-full`, the quick-suite drift
   check (`cargo run --release --quiet --example benchmark -- --suite quick`,
   must stay 59/59 `ok` — the flag is off there, so `child_evals` must be
   identical), and `cargo test --release --test test_repetition --
   --include-ignored`.
9. **Write `report5.md`.** All repeats/censoring, the per-arm tables, the
   interaction with the best stack, the tax estimate, validation results, gate
   results, and one explicit verdict per arm.
10. **Update the initiative.** Record item 5's verdict in
    `initiative.md` (item row + a History entry), state what is next (item 6 vs
    a new sibling-simulation item), and update the `provari` row of
    `docs/plans/README.md` if the ranking moved.

## Measurement protocol

Release build, one process at a time, no tablebases, 192 MB TT label, corpus
from `measurements/plan1/corpus.json`.

| Name | Flags |
| --- | --- |
| `best` | `--move-class-pn --quiet-grouping` (the plan3/plan4 reference) |
| `best+eval` | best + `--eval-pn both` |
| `best+eval-or` | best + `--eval-pn or` |
| `eval` (isolation, core only) | `--eval-pn both` alone |

Command shape:

```sh
./target/release/atomic_solver --fen "<FEN>" --tt-size 192 \
  --first-outcome --outcome-only --timeout <T> [--budget <B>] [flags]
```

Runs:

- **Core FEN:** `--timeout 90 --budget 600000000`, 3 repeats × 4 configurations
  (`best`, `best+eval`, `best+eval-or`, `eval`). Report all repeats, medians,
  and whether `budget exhausted`/`timeout` fired.
- **Shared case `1eb80b92f92c`:** `--timeout 10 --budget 300000000`, 3 repeats
  × 3 configurations.
- **`sample20`:** `--timeout 10` (no explicit budget; plan1 parity), 1 repeat ×
  3 configurations. Every row kept; censoring reported; the aggregate billed
  evals and the coverage census reported per configuration.
- **Tax probe:** from the censored `sample20` rows, the ratio
  `evals(best+eval)/evals(best)` at a fixed wall (≈ 10 s) estimates the
  evaluation's throughput cost; report it with its spread.

Primary metric: `child_evals` to the first decisive outcome (deterministic).
Secondary: wall time, outcome, first-win length, `pv_status`, exit reason,
coverage.

## Acceptance and pre-registered verdict rule

Correctness (a veto on that arm, regardless of speed):

- no decisive case changes outcome versus `best`;
- every decisive measured run replays: `reconstruct_pt` reports
  `root_outcome == measured outcome` and `validate: ok`;
- `make test`, `make test-full`, the repetition gate, and the quick-suite drift
  check are green; with the flag off nothing is observable.

Speed (per arm; ratio = arm / `best`, lower is better):

- **GO → the arm joins the stack** when all of:
  1. neither core nor the shared case is worse than **1.10×** child evals;
  2. the geometric mean of the core and shared `child_evals` ratios is
     **≤ 0.85**;
  3. `sample20` coverage is unchanged and the aggregate billed evals on the
     censored rows are no worse than **1.10×**.
- **NEUTRAL → keep the flag, do not rank it** when (1) and (3) hold but (2)
  does not.
- **REJECT → keep default-off and record the arm as measured out** when any of
  (1)–(3) fails.

If both arms fail, item 5 closes as neutral/negative (a valid outcome) with
this evidence, and the next ranked step is item 6. The default stays `off` in
every case: which lever (if any) becomes the product default is item 7's
decision, following the plan2/plan3 precedent of keeping measured levers
opt-in.

## Validation / gates

- Default gate: `make test`.
- Search-behavior gate: `make test-full` (~25 min; required for a
  search-behavior change).
- Repetition: `cargo test --release --test test_repetition -- --include-ignored`.
- Drift: `cargo run --release --quiet --example benchmark -- --suite quick`,
  recorded verbatim (must be 59/59 `ok`, `child_evals` columns identical).
- Replay: the proof-tree reconstruct/validate path from plan2–4; `verify_ppv`
  on the informational PV is reported but is not the acceptance criterion.

## Risks and mitigations

- **Case-dependent outcome (the observed risk).** Provari's own hand guide is
  2.27× *worse* on our one shared win. Mitigation: per-case floors (1)–(3)
  above, the `or` arm as a diagnostic, and the rule that a bad shared case
  vetoes the arm even if the core FEN shines.
- **The step is too coarse.** `step ∈ {0,1,2}` is all provari's hand guide
  offers; a 6×/11× multiplier on an "unclear" band is a big perturbation.
  Mitigation: pre-registered measurement, not tuning; a graded step is a named
  follow-up, not silent scope.
- **Interaction with plan 2.** EF-PN multiplies the class numbers, so a class
  `pn = 1` (double threat) stays 1 at `step = 2` and becomes 11 at `step = 0`.
  That composition is provari's own; measure `best+eval` (composed) as the
  primary arm so the reported ratio is the stack's, not a bare-baseline
  artifact.
- **Cost.** One evaluation per frame visit could be 5–15 % of node time.
  Mitigation: the tax probe, and the report states the tax explicitly so a
  neutral-evidence outcome cannot be misread as "the lever does nothing".
- **Hidden state creep.** Threading `Option<ChildScale>` instead of storing it
  on `Search` keeps recursion-safe and restart-safe behavior; do not "optimize"
  this into a field.
- **Scope creep.** If the implementation needs per-type tuning, a graded step,
  a network, or coverage of the item-6 keying, stop and record it as a
  follow-up.

## Ranking note (item 4b)

Item 4b (`exposed`/no-safe-evasion/double-threat closures) is **not** the next
step. Report 4 measured the whole node-entry-closure family as non-additive
with the plan2+plan3 stack on the core FEN (1.18× *more* work) and attributed
it to skipped ordering feedback. 4b is the same family (a frame short-circuit),
while EF-PN prices the subtree *inside* DF-PN, which is exactly the direction
report 4's diagnosis points at. 4b stays open but unranked; reopen it only if a
new diagnostic shows closure-shaped wins that pricing cannot reach.

## Report requirements

`report5.md` is the final task and must include:

- the evaluation and scaling rule as implemented, with every approximation and
  the deviations from provari (commoners vs kings, fixed constants, no logit
  branch) marked;
- why `off` is inert (threading, not state) and the test that pins it;
- all repeat tables (core × 4 configurations, shared × 3, `sample20` × 3) with
  medians and censoring;
- the per-case `child_evals` and wall ratios against `best`, plus the geometric
  mean;
- the tax estimate from the censored rows;
- the replay-validation results for every decisive row;
- gate results (all six);
- one verdict per arm (GO / NEUTRAL / REJECT) per the pre-registered rule, and
  the resulting stack for later plans;
- explicit "not measured" items (4b, item 2, item 6, the NNUE/provability
  model, tablebases, full `high200`);
- the next ranked step with a one-line reason (item 6 vs a sibling-simulation
  item).
