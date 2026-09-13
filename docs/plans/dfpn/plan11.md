# Plan 11: Cross-clock solved-entry reuse, decoupled from threshold arithmetic (plan10 resurrection)

Initiative: `dfpn` backlog #2 (reopened). Prerequisite reading: `plan10.md`
(the lever and its shift lemma), `report10.md` (why the original design was a
no-go), `report7.md` (why twins were removed), `report9.md` (the repetition
cache this lever composes with), `research_ghi.md` §9 (Option A, the
verification fallback).

## Goal

Recover plan10's measured **−37.7% / −41.6%** child-eval win on the stress
case without the DF-PN destabilization that killed it (m22_white control
3.1 s → 120 s timeout). plan10 proved the lever *sound* (444/444 adopted
claims re-verified, 59/59 quick outcomes unchanged, cyclic rook safe); the
defect was exclusively **search dynamics**: solved children's `(0, INF)` /
`(INF, 0)` bounds folding into unsolved parents' pn/dn
(`select_from_children`) at ~50–100× the baseline frequency crossed a
stability boundary. This plan implements the two decoupling mechanisms named
in the resurrection proposal and measures them in one Phase 0 spike.

## Background: the two structural facts this plan is built on

### Fact 1 — the parent's only child channel is `evaluate_child`

In `core.rs::dfpn`, the recursive call's return value is discarded
(`let _ = search.dfpn(...)`); after the recursion the frame re-runs
`evaluate_child` on the best child, which re-probes the TT. Therefore:

- A "frame-entry-only" adoption (plan10 site 1, conversion-initiative lemma
  framing) can only reach the parent by **storing the adopted outcome back
  into the TT at the child's current (clock-qualified) key** — after which
  the parent folds it through the *pre-existing* exact-key resolved path.
  The folding volume is then ≈ the number of adopted (board, clock) pairs —
  the same volume that destabilized plan10. Mechanism A below is this
  design; it is measured because it is cheap and because plan10 never ran
  site-1-only, but the structural prediction is that it does **not**
  decouple.
- A genuine decoupling must live **inside the site-2 probe**
  (`evaluate_child`), where plan10 measured all 1,111,158 stress adoptions
  and the entire −37.7%. Mechanism B below is that design.

### Fact 2 — the destabilizer is direction-specific

report10's differential table:

| Variant | stress FO | m22 control |
|---|---|---|
| full lever (plan10) | 155.4M win | timeout (714.6M @ 120 s) |
| adopt-Loss only | 240.1M win | **win 3.1 s / 13.4M — safe, mild gain** |
| adopt-Win only | **145.5M win (best)** | timeout (168.0M @ 30 s) |

"Loss child" = side-to-move at the child loses (the parent's winning move —
a proof-completion fact the parent would have derived anyway, just later).
"Win child" = side-to-move at the child wins (from an OR parent's view: a
*refuted* attacker move; from an AND parent's view: a *proof-completing*
attacker win). Loss-facts-as-solved were measured harmless on the control;
Win-facts-as-solved are the pathology. The quarantine targets exactly the
Win direction.

## Mechanism A — frame-entry adoption with TT store-back

Identical index and store site as plan10 (`plan10.md` §Design, unchanged):

- `HashMap<u64, (Move, Outcome, u32)>` keyed by `rep_key`
  (board-only `repetition_key`), payload `(best_move, outcome, proven
  depth)`; capacity `1 << 18`, drop inserts when full; persists across
  `begin_run` like the TT; never serialized into any artifact.
- Store at the frame-exit TT store in `core.rs::dfpn`, only for
  `store_outcome == Some(Win | Loss)`.

Probe site: `dfpn` node entry only (after the plan9 repetition-cache probe,
before `sort_moves`), **replacing** plan10's site 2. On a hit satisfying:

- `rule50 + d ≤ 100` (shift-lemma budget rule, `plan10.md` §Soundness
  contract 3),
- `d ≤ max_depth`,
- the one-ply `best_move_repeats_path` guard on `best_move`,

the frame: emits `NodeProven(pos, outcome, d)`, **stores the solved result
into the TT at the current full key** (`remaining_depth = u32::MAX`,
`depth = d`, `work = 0`), and returns the outcome. The parent learns the
fact on its next `evaluate_child` through the ordinary exact-key resolved
path — no new parent-side mechanism exists.

**Prediction (to be falsified or confirmed by the spike):** the TT
store-back routes the fact into the same `select_from_children` folding that
destabilized plan10, at approximately plan10's adoption volume (site-1
adoptions were ≈ 0 only because site 2 preempted the descents; with site 2
off, frame-entry adoptions take over that volume). A is the control arm for
"decoupling requires the quarantine, not a different probe site".

## Mechanism B — site-2 direction split: Loss adopted solved, Win quarantined

Same index, same store site, same adoption rule. Probe **only** at
`evaluate_child` (site 2, where the exact-key `resolved` is `None`),
replacing plan10's unconditional adoption with a direction split:

### Loss child → adopted solved (plan10 behavior, measured control-safe)

`ChildInfo { outcome: Some(Loss), pn/dn = Loss.pn_dn_for(child_is_or),
depth: d, repetition_seen: false }` — identical shape to an exact-key
resolved hit, including the proof-event emission. The parent may claim Win
through it.

### Win child → quarantined (never asserts, only suppresses search)

`ChildInfo { outcome: None, explored: true, repetition_seen: false }` with
bounds that fold **conservatively** at either parent type:

- OR parent (`is_or_node == true`): `(pn, dn) = (INF, 1)`
  — true contribution is `(INF, 0)`; dn floored 0 → 1.
- AND parent (`is_or_node == false`): `(pn, dn) = (1, INF)`
  — true contribution is `(0, INF)`; pn floored 0 → 1.

Effects (all verified against the current `selection.rs`/`children.rs`
code): `explored = true` removes the child from `best_and_second_unsolved`,
from `second_best_unsolved_excluding`, and from the previous-best reuse
guard (`outcome.is_none() && !explored`); `outcome = None` keeps
`is_solved_by_children`'s `all_solved` false, so the parent can never claim
any solved outcome from quarantined facts; the `evaluate_all_children`
early exit triggers only on `Some(Loss)`, so quarantined children never
trigger it; an OR parent whose children are *all* quarantined has no
searchable child (`best_child == Move::NONE`) and exits unsolved — sound
(the node's true disproof value is simply not claimed).

### Soundness contract (stronger than plan10's)

1. **Quarantine is suppress-only by construction**: a quarantined fact can
   only (i) prevent a descent into a refuted subtree and (ii) make the
   parent's folded bounds *strictly harder* (0 → 1 on the decisive
   direction) than the truth. It can never manufacture or enable a decisive
   claim. Every decisive outcome at the root is still derived exclusively
   from genuinely searched proofs — the shift lemma is not even needed for
   the quarantine direction.
2. **Solved-Loss adoption** keeps plan10's shift lemma verbatim
   (`plan10.md` §Soundness contract 2, normative): Win/Loss proof trees are
   repetition-robust (repetition edges evaluate as Draw and cannot support a
   decisive proof), leaves are clock-independent, `rule50 + d ≤ 100` bounds
   every interior node's clock.
3. Adoption rule, one-ply guard, draws-never-stored, budget contract (a
   hit still consumes its one `child_evals` increment, exactly like an
   exact-key resolved child), index lifetime, and no-serialization: all as
   `plan10.md` §Soundness contract 1, 4–7.

### Sub-arms (parent-type topology of the quarantine)

The AND-parent case is the open question: an AND node whose replies are all
adopted attacker-wins is *truly* a defender Loss, but quarantine prevents it
from claiming that, stalling the attacker's proof completion at clock-variant
revisits (the exact mass plan10's win came from). The spike therefore measures
three topologies:

- **B1** — quarantine Win-facts at both parent types (purest decoupling).
- **B2** — Win-facts solved at AND parents (proof completion preserved),
  quarantined at OR parents (dead-child suppression only).
- **B3** — Win-facts solved at OR parents, quarantined at AND parents
  (expected worst; included to complete the factorial).

If every B-arm fails its gates, the contingency arm is **C — verified
adoption** (`research_ghi.md` §9 Option A at this probe site): adopted
Win-facts at AND parents are accepted as solved only after a bounded fresh
`dfpn` call at `max_depth = d` under the current path confirms the outcome;
unverified facts fall back to quarantine. C is *not* built in the first
spike round — it is the documented fallback if the histogram shows the
adoption mass sits at AND parents and B1 stalls on it.

## Phase 0 — sizing spike (temporary instrumentation, then go/no-go)

Spike build: temporary, env-gated instrumentation in `Search` + a temporary
`examples/solve_stats.rs` runner (plan10's pattern; all reverted after the
verdict). Env switches: `DFPN11_SPIKE_OFF=1` (baseline), `DFPN11_ARM=A |
B1 | B2 | B3 | PLAN10` (the last reproduces plan10's full lever as a
spike-build sanity check), plus counters dumped by the runner.

1. **Baseline reproduction**: `DFPN11_SPIKE_OFF=1` must reproduce the
   post-plan9 numbers exactly — stress FO 13,907,467 nodes / 249,480,478
   child evals; stress default 19,943,731 / 338,094,183; m22 3.1 s /
   14,156,269.
2. **Spike-build validation**: `DFPN11_ARM=PLAN10` must reproduce report10's
   full-lever numbers (155,394,450 FO) and, new in this spike, dump the
   **direction × parent-type histogram** of adoptions (report10's named
   missing diagnostic): for each adopted child, whether the outcome was Win
   or Loss and whether the parent was an OR or AND node. This decides which
   B-topology has room before interpreting the arms.
3. **Arm matrix** — each arm measured on: stress FO
   (`--timeout 120 --first-outcome --outcome-only`), stress default mode,
   m22 control, quick-suite outcomes (`benchmark --suite quick --json
   --first-outcome`), and the cyclic rook soundness spot-check. Counters
   per arm: adoptions by (direction, parent type), quarantines, AND-node
   all-quarantined stalls, index working set (peak entries).
4. **Go/no-go gates** (all must hold for the winning arm):
   - stress FO child evals improved ≥ 10% vs the 249,480,478 baseline;
   - m22 control ≤ 2× baseline wall (≤ ~6.2 s) — the canary that killed
     plan10;
   - all 59 quick-suite outcomes unchanged, `wrong=false` everywhere;
   - cyclic rook never claims a win;
   - no case regresses > 2× its baseline child evals without an explicit
     intended-delta note (repetition/clock-heavy cases may drift; list
     them).
5. If an arm passes: promote it to the implementation tasks below. If none
   passes but the histogram shows AND-parent mass + B1 stalls: run arm C
   once before closing. If still no-go: close backlog #2 permanently with
   the full matrix in `report11.md`, revert everything, and re-rank (#3
   next).

## Tasks (post-go implementation)

1. Extract the winning mechanism from the spike build into
   `src/search/dfpn/cross_clock.rs` (< 10 KB): index type, store/probe API,
   adoption rule, quarantine bounds; unit tests: store/probe round-trip,
   budget boundary (`rule50 + d == 100` adopt, `101` reject), capacity stop,
   overwrite, clock-independent key, draws-never-stored, quarantine
   never-solves (quarantined `ChildInfo` has `outcome: None` at both parent
   types), quarantine conservatism (`(INF, 1)` at OR / `(1, INF)` at AND).
2. Wire the store site and the winning probe topology; remove the env
   switches. Add a dfpn unit test: solve a small winning position at a low
   clock, revisit the same board at a higher (budget-covered) clock in one
   `Search` instance, assert the index hit and a materially lower node count
   with the same decisive outcome; and a test that a quarantined child never
   appears as `solved_outcome` in any selection.
3. Soundness gate: `cargo test --release --test test_repetition --
   --include-ignored` (both cyclic-rook regression tests) + a new
   integration test: the cyclic rook solved at two different clocks in one
   process never yields `Outcome::Win`.
4. Drift protocol: `benchmark --suite quick --json --first-outcome` before
   vs. after — non-adoption cases bit-identical, intended deltas listed with
   outcomes unchanged; `cargo test --release --test test_move_order --
   --include-ignored`.
5. Budget contract: `cargo test --release --test test_plan6 --
   --include-ignored` unchanged.
6. Stress measurement vs the post-plan9 baseline (both modes) with
   `child_evals`/nodes/wall and PV effects; verify the produced PVs legal
   (`verify_ppv` on the stress line).
7. Memory: report peak index entries; shrink the `1 << 18` default if the
   working set is far below (plan9's pattern).
8. Update AGENTS.md (dfpn paragraph: one sentence on the cross-clock index,
   its adoption rule and direction split) and `initiative.md` backlog #2.
9. Write `report11.md` in this directory (tools, problems, unresolved parts,
   missing tests, next steps).

## Validation checklist

- `cargo fmt`, `cargo clippy --all-targets`, `cargo test --release` (fast
  gate), `cargo doc --no-deps`.
- Repetition soundness gate (cyclic rook, both clocks) — `--include-ignored`.
- Quick-suite drift + move-order suite + budget-contract tests.
- Stress-case numbers against the post-plan9 baseline and the spike's
  go/no-go matrix.

## Non-goals

- Any change to the Zobrist keys, the TT entry layout/replace policy, or
  TT-snapshot/proof artifacts (the index is never serialized).
- Adopting Draws across clocks (plan9's cache and the first-player-loss
  shortcut untouched).
- Twins, path codes, or Kawano-style simulation (plan5's removed mechanism).
- Proof-tree layer changes (quarantined children emit no events; adopted
  Loss children emit exactly like exact-key resolved hits).
- Tuning the adoption rule thresholds (the `rule50 + d ≤ 100` rule and the
  (INF,1)/(1,INF) quarantine bounds are fixed; re-tuning is a separate
  plan only if the mechanism lands).
- Wall-time micro-engineering of the added probes (`lean`'s mandate).
