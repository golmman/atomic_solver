# Plan 4: Clock-pressure ordering — AND-side shuffle preference

Initiative: `conversion` backlog #3. Prerequisite reading:
`dfpn/research_repetition_cache.md` (the repetition-dominated work profile
this signal targets), `lean/initiative.md` backlog #5 (the general AND-side
ordering lane this plan carves the clock-specific signal out of),
`initiative.md` backlog #2/report2 (why ordering-only is the surviving
soundness class in this initiative).

**Scope decision (recorded per the plan2 precedent).** Backlog #3 names two
sides: an OR-side bonus for clock-resetting moves (pawn pushes/captures once
rule50 passes a threshold) and an AND-side preference for reversible
defender moves. This plan implements **the AND side only** — it is the
cheap, zero-soundness-risk half and the one the work profile points at
(defender shuffles are the drawing resource and where disproving work
concentrates). The OR-side clock-reset bonus is a separate ordering change
on a side where `lean` measured OR ordering at its oracle floor: touching it
needs its own sizing evidence and would entangle two drift surfaces in one
plan. It stays open in backlog #3 as a successor lever if this plan's Phase
0 shows the AND-side surface is real.

## Goal

At AND nodes (defender to move) when the halfmove clock is high, the
defender's drawing resource is *shuffling*: non-capture, non-pawn moves that
leave the clock running and the board's repetition key reusable. The static
scorer is blind to the clock — `score_with_context` (`src/search/ordering.rs`)
scales attacker bonuses down at AND nodes but ranks all defender retreats by
board-geometry terms only. History may partially cover this (repeated
shuffles accumulate history scores), but history is exactly the mechanism
that only pays *after* the first re-proof — and the repetition profile
(`dfpn/research_repetition_cache.md`: 96% re-proofs, cost in re-descent
churn) means the search re-sorts and re-descends these nodes thousands of
times, so paying the shuffle preference from move one rather than after
history accumulation is the lever.

Deliverable: a clock-gated, AND-side-only ordering term that adds a bonus to
non-capture, non-pawn defender moves once `rule50 ≥ threshold`, validated
under the initiative's two-sided protocol (quick-suite outcomes unchanged,
drift confined to repetition-heavy cases, repetition soundness gate green).
Primary metric: `child_evals` to first decisive outcome on the `make stress`
case vs the inherited post-plan9 baseline (first-outcome 249,480,478 child
evals / 13,907,467 nodes / 53.8 s; default mode 338,094,183 / 19,943,731 /
72.3 s).

Ordering-only ⇒ no soundness argument is needed beyond the drift protocol
(every outcome the search can return is one it could have returned before —
only the exploration order changes). The `m22_white` control
(`4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22`, ~3 s win,
858,117 nodes / 14,156,269 evals) and the non-repetition quick-suite cases
must stay within noise.

## Phase 0 — sizing spike (go/no-go, then either productionized or reverted)

The effect is unmeasured and the obvious confounder is that history/killer
scores may already promote the relevant shuffles to the top. Phase 0 is a
counter-only spike (temporary instrumentation, reverted after measuring —
the initiative's "measure before planning" cadence) that answers two
questions before any ordering change is written:

1. **Is the surface real?** Instrument `sort_moves` (or the dfpn frame) to
   sample, per AND-node expansion with `rule50 ≥ T` (start with T = 60, and
   record the clock distribution so T can be re-picked from data):
   - the fraction of the node's legal moves that are shuffles
     (`!is_capture && moving piece ≠ Pawn`) — if nearly all defender moves
     at high clocks are shuffles, a uniform bonus cannot change the order
     and the lever is dead on arrival;
   - the rank under the *current* total score of (a) the move eventually
     disproving/proving at that node (the selected child) and (b) the
     refutation shuffle when the selected child is one — histogram of
     ranks 0/1/2–4/5+. If refutation shuffles are already rank 0–1 via
     history/killers, the headroom is thin and the plan is a likely no-go.
2. **Where the work is.** Count AND-node expansions at `rule50 ≥ T` as a
   fraction of total nodes on the stress case (first-outcome) — the upper
   bound on what any AND-side reorder can save is bounded by the movegen +
   child-eval work at those nodes.

Run on the stress case first-outcome and on `m22_white`. **Go/no-go bar:**
proceed only if (a) refutation-shuffle rank histograms show material mass at
rank ≥ 2 (headroom exists above history/killer), (b) shuffle-vs-non-shuffle
composition at high-clock AND nodes is mixed enough that reordering can
bite, and (c) high-clock AND nodes carry ≥ ~10% of stress-case node work.
On no-go: revert all spike code, write `report4.md` with the histograms,
and re-rank the backlog (backlog #3 closed for the AND side; the OR-side
half and the DF-PN+ `H`/`Cost` clock-flavored frontier estimates are the
remaining candidates and must be re-justified against this measurement).

## Design (on go)

- **Term site:** `Search::sort_moves` (`src/search/dfpn/history.rs`), added
  to the score sum next to history and killer — the same composition point
  the existing heuristic terms use, leaving `StaticAtomicScorer` untouched.
  `sort_moves` already holds `pos`, so the clock is a
  `pos.board().rule50()` read; no signature changes propagate.
- **Term:** when `!is_or_node && rule50 >= CLOCK_PRESSURE_THRESHOLD`, add
  `CLOCK_SHUFFLE_BONUS` to moves that are `!is_capture && moving piece type
  ≠ Pawn` (the same cheap classification `score_with_context` already
  computes per move; a pawn move or capture resets the clock, everything
  else leaves the shuffle resource intact). No board mutation probes, no
  repetition-key simulation — the term is two integer compares per move.
- **Constants vs params:** keep `CLOCK_PRESSURE_THRESHOLD` and
  `CLOCK_SHUFFLE_BONUS` as compile-time constants for this plan (module
  consts next to the ordering constants in `ordering.rs`/`history.rs`,
  values picked from the Phase 0 data). Do **not** add `ScorerParams` fields
  in this plan: that extends the external optimizer contract
  (`docs/spec/optimizer_interface.md`), and the plan's own data may show the
  term should be shaped differently before it earns a tuning dimension. If
  the report concludes for parametrization, that is a follow-up under the
  `tune`/spec lane with the spec updated in the same change.
- **Tie-breaking:** the scored sort is a total order by `(score, original
  index)` as today; the bonus changes scores, hence order, deterministically.
  No new state, no memory change, no TT interaction.
- **`move_order_debug`/`static_move_scores` examples:** extend
  `move_order_breakdown` (`history.rs`) with the clock term as its own
  column so the examples keep explaining totals; `static_move_scores` (pure
  static scorer) stays untouched by design.

## Tasks

1. Phase 0 spike per above; record histograms and the clock distribution in
   the report. On go: remove instrumentation. On no-go: revert to
   byte-identical pre-plan state and stop.
2. Implement the term in `sort_moves` per the design; add the
   `move_order_breakdown` column; keep `ordering.rs` under its size budget
   (the term lives in `history.rs` composition, not in the scorer).
3. Unit tests (`#[cfg(test)]` in `history.rs`/`ordering.rs`): term fires
   only at AND nodes; threshold boundary (clock T−1 no bonus, T bonus);
   pawn moves and captures excluded; OR side untouched (bit-identical
   ordering at OR nodes for a fixed position set); breakdown column
   correct; determinism (two `sort_moves` calls on equal input give equal
   order).
4. Drift protocol: `benchmark --suite quick --json --first-outcome` before
   vs after. All 59 outcomes must be unchanged. Node/eval deltas are
   permitted only on repetition-heavy cases (list them in the report);
   non-repetition cases should be node-identical or explainably close (the
   term fires at high-clock AND nodes, which easy cases rarely reach).
5. Soundness gate: `cargo test --release --test test_repetition --
   --include-ignored` (cyclic rook must never claim a win); ordering suite
   `cargo test --release --test test_move_order -- --include-ignored`;
   budget contract `cargo test --release --test test_plan6 --
   --include-ignored` (`ExitReason::BudgetExhausted` semantics untouched).
6. Stress measurement vs the inherited baseline: record
   `child_evals`/`nodes`/wall time for first-outcome and default mode, plus
   the m22 control (must stay ~3 s) and the PV-length effect. Compare
   against the Phase 0 rank histograms — did the mass at rank ≥ 2 move?
7. Update `initiative.md` backlog #3 status (AND side resolved; OR-side
   half explicitly re-ranked or closed per the report) and the AGENTS.md
   ordering sentence only if the term is worth a mention there (it likely
   belongs in the existing dfpn paragraph's ordering clause).
8. Write `report4.md` (tools/examples used, problems, unresolved parts,
   missing tests, next steps — including the OR-side decision).

## Validation checklist (summary)

- `cargo fmt`, `cargo clippy --all-targets`, `cargo test --release` (fast
  gate), `cargo doc --no-deps`.
- Quick-suite drift: outcomes unchanged; deltas confined to
  repetition-heavy cases.
- Repetition, move-order, and budget-contract suites with
  `--include-ignored` green.
- Stress + m22 control numbers recorded against the inherited baselines.

## Non-goals

- The OR-side clock-reset bonus (backlog #3's other half) — separate lever,
  needs its own sizing evidence against lean's oracle-floor measurement.
- DF-PN+ `H`/`Cost` flavor (frontier estimates scaled by remaining clock
  budget): touches threshold arithmetic — the plan10 hazard class; only
  reconsiderable with the mechanism-level argument the mapping protocol
  demands, and only if this cheap signal proves insufficient.
- Any change to child *evaluation* or terminal classification
  (`evaluate_child`): the signal never changes which children are terminal,
  only the order they are tried in.
- Parametrizing the term via `ScorerParams` / the optimizer TOML contract
  (explicitly deferred; see Design).
- AND-side ordering signals beyond the clock/shuffle pair (`lean` #5's
  lane): do not bundle.
