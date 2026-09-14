# Lean Plan 9 — #5 AND-side ordering: Phase-0 measurement spike (go/no-go)

Initiative: `lean` backlog #5 ("AND-side ordering signal (non-NN)").
Prerequisite reading: `docs/plans/nn/report8.md` on the archived `nn` branch
(the oracle-floor decomposition this spike re-grounds), `plan7.md`/`report7.md`
(the OR/AND work-split spike and its generous refuter assumption),
`conversion/plan4.md`/`report4.md` (the env-gated counter-spike pattern and
the one *filtered* AND-surface measurement that exists).

## Scope decision (spike-only)

This plan is a **measurement spike**: temporary, env-gated instrumentation,
measured, fully reverted; **no `src/` changes land**. Deliverable is a
go/no-go decision on backlog #5 with numbers, recorded in `report9.md`.

Rationale for not bundling an implementation:

- The working agreement is one lever per plan, and the lever here is the
  *decision about the AND-side surface* — the backlog's "~5–20% evals"
  potential is an unverified estimate whose ceiling was never decomposed
  (the `nn` campaign measured per-node concentration, never the refuter's
  rank or the recoverable pre-refuter mass).
- The recorded regression precedent (`m24_white` 2.1× under the `nn`
  oracle ordering) shows AND-ordering changes have global DF-PN dynamics
  risk that must be sized before any signal is wired into `sort_moves`.
- Precedent: `plan7` (parallel ceilings) and `conversion plan4` (clock
  surface) were both counter-only spikes that closed or gated an item with
  numbers instead of an implementation.

## Goal

Answer four questions about the AND-frame (defender-to-move) surface that
no prior campaign measured:

- **Q1 — refuter rank**: at refuted AND frames (frame exits
  `solved_outcome == Win` via the first-`Loss`-child rule), what is the
  rank of the winning child in the final sorted move order? If the median
  rank is already 0–1, the surface is empty (generalizing conversion
  plan4's high-clock-only finding) and #5 closes as no-go.
- **Q2 — recoverable mass**: how much AND-frame child-eval work is spent
  on replies evaluated *before* the refuter, as a fraction of *total*
  child evals? This replaces the backlog estimate with a ceiling. The
  complement class matters: AND frames proven `Loss` (all replies
  evaluated) are locally unsalvageable and dilute the lever.
- **Q3 — signal probes**: is the refuter predictable from something
  cheap? Three probes:
  - (a) **online counter-move table**: keyed by the attacker move that
    entered the AND frame (the parent OR frame's move; available as the
    last entry of `move_stack` at AND frames). Probe *before* recording —
    hit rate and the rank the table would have given the refuter. This
    directly simulates the classic counter-move heuristic with zero
    search-behavior change.
  - (b) **resolution source** of the refuting child (terminal
    classification / TT-resolved solved hit / resolved by selection-loop
    recursion): TT-resolved refuters might already be distinguishable via
    `TtEntry` fields (`work`, `best_child`) without any new table.
  - (c) **existing-signal coverage**: the refuter's rank under static
    scoring alone vs after history+killer bonuses (are the existing
    side-indexed tables, updated on the refuting reply at `core.rs:372`,
    already doing the work?).
- **Q4 — OR-frame contrast**: re-measure the decisive-child concentration
  at OR frames on current code to confirm the `nn` 90.6% figure still
  holds post-plan6/8/9.

## Grounding facts (measured or in-code; the spike verifies, not re-derives)

- Work split is **~40% OR / ~60% AND** by child evals, stable across both
  validation cases (plan7 phase 1, instrumented + reverted): m22_white
  5,510,656 OR / 8,645,613 AND; shuffle-win 100,877,680 OR / 148,602,798
  AND. AND frames are ~4.5× more numerous and cheaper per frame.
- `evaluate_all_children` (`src/search/dfpn/children.rs:110`) breaks on
  the first child `Outcome::Loss` at both node classes; at AND frames that
  break is the refuter disproof exit. Sequential cost at a refuted frame =
  pre-refuter replies + the refuter eval. The frame loop can also resolve
  the refuter later via selection-loop recursion (`core.rs` re-evaluates
  the selected child each iteration); the spike must distinguish
  "refuter found in the initial sweep" from "refuter resolved by
  recursion".
- The `nn` decomposition (archived report8): per-node max child-share
  median **100%** at AND nodes, aggregate child-share median 52.9%, and
  the explicit conclusion that "the learnable signal lives mostly on the
  AND side". The NN PoC was closed on inference-throughput economics, not
  on the absence of the signal — a cheap non-NN signal is the untested
  residue. Caveat carried from that report: `finalize()` canonical-copy
  inflates some per-node `work` values; fresh in-search counters avoid
  this entirely.
- Report7's parallelism math *assumed* refuter = max-share child
  (pre-refuter work = the 47.1% median complement) as a generous bound.
  That identification is exactly what Q1/Q2 test.
- Existing AND-side ordering: static scorer with scaled-down speculative
  profile (`ordering.rs`, `is_or_node` selects bonuses), shared
  side-indexed history and per-depth killers (`history.rs`). No
  counter-move table exists anywhere in the codebase.

## Phase 0 — instrumented measurement (all instrumentation reverted after)

Environment gate `LEAN9_SPIKE=1` (pattern: `PLAN4_SPIKE=1`). With the gate
unset, the binary is bit-identical in behavior and output to before the
spike. Temporary state lives on `Search` (plain u64 counters + two small
std-only maps for the counter-move probe); the summary prints once to
stderr at pre-exit.

### Metric definitions

Frame-level, attributed by the parent frame's `is_or_node`:

- **M1 (Q4)**: OR-frame decisive-child concentration — child evals spent
  on the winning child vs total OR-frame child evals; rank distribution of
  the winning child.
- **M2 (Q1)**: refuter-rank histogram at refuted AND frames: index of the
  winning child in the final sorted `children` table, capped at
  `>= 8` bucket. Split by resolution path: (i) found in the initial
  `evaluate_all_children` sweep (winning index < first-pass eval count),
  (ii) resolved by selection-loop recursion.
- **M3 (Q2)**: eval-mass split of AND-frame child evals into: refuted
  frames (with pre-refuter evals counted separately from the refuter's own
  eval), proven-`Loss` frames (all replies evaluated), and frames cut by
  thresholds/time/work budget. Reported as absolute counts per case plus
  pre-refuter-mass / total-child-evals. A frame's pre-refuter evals
  include the nested selection-loop recursions attributable to replies
  other than the refuter; an attributable approximation is acceptable if
  documented (e.g. first-pass evals before the winning index + per-iteration
  re-evals before the winning child's final resolution), because the
  deliverable is a ceiling, not an audit.
- **M4 (Q3a)**: online counter-move probe — map `(attacker_move) →
  refuter_move` updated after each refuted AND frame (last writer wins,
  plus a per-key refuter-count map for concentration). On each refutation,
  probe *before* update: hit = table already holds this refuter;
  concentration = distinct refuters per attacker move and the top-refuter
  share. Also record the rank the table's move holds in the current sorted
  order when it matches.
- **M5 (Q3b/c)**: refuter resolution-source histogram (terminal class /
  TT-resolved / recursion-resolved) and, at refuted frames, the winning
  child's static-score rank vs final sorted rank (existing history/killer
  effect isolated).

### Cases and commands

Release build, `--first-outcome --outcome-only --tt-size 128`, env
`LEAN9_SPIKE=1`:

| case | FEN | command |
| --- | --- | --- |
| m22_white | `4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22` | `--fen <FEN> --timeout 30` |
| shuffle-win | `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21` | `--fen <FEN> --timeout 100` |
| m24_white (dynamics sentinel, no behavior change — sanity that counters do not perturb) | `4r1k1/3p4/2pB2p1/6Pp/p4p1P/2N1PP2/P1PP4/1R2R2K w - - 0 24` | `--fen <FEN> --timeout 20` |

Expected baseline reference points (post-plan8/9-era, aarch64 host):
m22_white first-outcome decisive in seconds (see `measurements/plan8/`),
shuffle-win ~20.3M nodes / ~65 s first-outcome. Instrumented runs must
report **identical outcomes, nodes and child_evals** to the un-instrumented
runs on the same build (counter increments only) — this is the
instrumentation-neutrality check, recorded in the report.

Raw tables go under `measurements/plan9/`.

### Decision rule (pinned before measuring)

- **GO**: pre-refuter AND-frame eval mass (M3) ≥ ~5% of total child evals
  on at least one validation case, **and** at least one M4/M5 probe shows
  the refuter would land at rank 0–1 in a material fraction of refuted
  frames (working bar: ≥ 50%). The successor plan then implements the
  single best signal behind the full drift protocol (quick-suite outcomes
  unchanged; m22 first-outcome byte-identical stdout is *not* the gate —
  ordering is behavior-changing; the move-order suite is the validator).
- **NO-GO**: median refuter rank already 0–1 (M2), or recoverable mass
  < ~5%, or no probed signal reaches the bar. Backlog #5 closes as a
  measured no-go with numbers, mirroring `dfpn` #4.
- **Gray zone**: mass real but all probed signals below bar → keep #5 open
  with the measured ceiling recorded, demoted below #7/#9/#10; do not
  implement against an unknown signal.

## Revert contract

- All instrumentation behind `LEAN9_SPIKE=1`; no counters, maps, or
  branches on the hot path with the gate unset beyond one `if` on a bool
  (acceptable, matches plan6 phase-0 precedent) — or compile-time-free by
  reading the env once into an `Option<...>` on `Search`.
- Post-revert: `cargo test --release` green; m22_white first-outcome
  stdout byte-identical to the pre-spike run; the instrumented run's
  outcome/node/child_eval triple matches the un-instrumented run's.
- The spike must not touch `sort_moves`, selection, TT, or history/killer
  update sites — counters only.

## Report requirements (`report9.md`)

1. The M1–M5 tables per case (raw under `measurements/plan9/`).
2. The decision against the pinned rule, with the numbers inline.
3. Instrumentation-neutrality evidence (identical triples).
4. If GO: the recommended signal, its measured hit/rank stats, and the
   drift-validation plan for the successor; explicitly note which
   regression mechanism from the `nn` campaign (coverage starvation vs
   DF-PN dynamics) the successor's gate must watch.
5. If NO-GO/gray zone: close or demote #5 in `initiative.md` (backlog row
   + status), and update `docs/plans/README.md` only if the initiative
   pivots or closes.
