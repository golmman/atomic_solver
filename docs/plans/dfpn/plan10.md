# Plan 10: Clock-budget-aware solved-entry reuse (cross-clock shadow index)

Initiative: `dfpn` backlog #2. Prerequisite reading: `initiative.md`
(Motivation — the 2026-09-11 deep-shuffle position study), `report7.md` (why
twins were removed), `research_ghi.md` §7/§9, `report9.md` (the repetition
cache this plan complements).

## Goal

Let the solver reuse solved **Win/Loss** entries across **halfmove-clock
values**: a proof found for a board at one clock becomes reusable for the same
board at a different clock whose 50-move budget still covers the stored proof
depth. The dominant miss pattern on the stress case is "same board, different
clock": shuffle lines revisit identical boards at many clock values, and the
full-Zobrist key (rule50 included, deliberately — `research_ghi.md` §7.2)
turns every revisit into an exact-key miss. Primary metric: `child_evals` to
first decisive outcome on the stress case. The lever is **unmeasured**, so
Phase 0 is a sizing spike with a hard go/no-go.

The recommended next item is backlog #2: backlog #1 is done (plan9), #4 is
explicitly gated on #1's follow-up landing, and #3 changes only PV-refinement
termination (a behavior lever with no node reduction in outcome-finding).
#2 is the remaining node-count lever that composes with plan9 without
touching repetition semantics.

## Background

The solver's Zobrist keys include the halfmove clock, so a board revisited at
a different clock is a guaranteed TT miss — and shuffle lines revisit the same
boards at many clock values by construction (pawn pushes between rook
triangulation cycles). Today every such revisit re-descends. Two existing
precedents shape the fix:

- The reconstruct layer already scans clocks offline
  (`src/reconstruct/mod.rs` `clock_scan`: `board_key ^ rule50_key(c)`,
  `c in 0..=100`) — the same board-key idea, but its adoption rule there is
  deliberately a hole, not a proof.
- `evaluate_child` (`src/search/dfpn/children.rs`) reuses solved child
  results through `resolved_from_entry` + the one-ply guard, classifying
  ~95% of evaluated children without movegen or descent. A cross-clock hit
  at *that* site removes an entire child descent, which is where the win
  must come from.

One lever: make solved Win/Loss entries reusable across halfmove clocks,
under a conservative depth-vs-budget rule. Draws are never touched
(first-player-loss shortcut untouched).

## Soundness contract (must hold after the change)

1. Only fully solved `Outcome::Win` / `Outcome::Loss` facts enter the index;
   draws never do (the first-player-loss shortcut is untouched), so no false
   draw can be manufactured and the plan7 false-win chain (simulation guessing
   an outcome) is excluded by construction: every hit replays a *real* proof.
2. **Shift lemma (normative soundness argument).** A solved entry's proof
   tree contains only Win/Loss nodes (a Win proof is one winning subtree; a
   Loss proof covers every legal reply with winning subtrees; Draw nodes
   cannot appear). Its leaves are statically correct wins/losses: checkmate,
   stalemate-irrelevant, or commoner extinction — and both checkmate/
   stalemate (moves-empty) and commoner extinction **outrank** the rule50
   draw in `outcome_from_state` precedence, so leaf values are
   clock-independent. Along any path the halfmove clock grows by at most 1
   per ply (captures/pawn moves reset it lower), so a node at depth `k` has
   clock ≤ c₁ + `depth(node)`. Replaying the proof from the same board at
   clock `c₁`: every *non-leaf* node has clock ≤ `c1 + (d − 1) ≤ 99` and
   every leaf has clock ≤ `c1 + d ≤ 100`, where `d` is the stored proven
   depth — so given the adoption rule below, no non-leaf can newly fire the
   rule50 auto-draw, and leaves are clock-independent. The proof structure
   therefore replays identically: the reuse is the value a fresh search
   would recompute.
3. **Adoption rule**: reuse a stored `(Win|Loss, d)` at a position with the
   same board key and clock `c1` **iff `rule50 + d ≤ 100`** (the new
   position's budget `100 − rule50` covers the proof depth) and `d ≤
   max_depth` (mirrors the `entry.depth > max_depth` guard of
   `resolved_from_entry`). The stored clock is *irrelevant* by the lemma —
   the bound is recomputed from the new clock — so the payload stores no
   budget. (This is a deliberate simplification of the backlog wording
   "store with budget 100 − rule50": that quantity is implied by the
   probe-side condition and is redundant.) Mid-line pawn/capture resets only
   lower clocks, so the bound is conservative by construction.
4. Only fully solved `Win`/`Loss` results enter the index — never draws
   (repetition-dependent or rule50-expiry), never unsolved bounds. The index
   is probed only after the exact-key solved-result check has missed, so
   exact-key behavior is byte-identical.
5. Cross-path conservatism is *inherited, not extended*: solved reuse
   already assumes Win/Loss proofs are path-independent ("path-independent
   base entries"), guarded by the same one-ply `best_move_repeats_path`
   check. The index stores the best move and applies exactly that guard.
6. `child_eval_budget` / `ExitReason::BudgetExhausted` semantics unchanged
   (a hit consumes no budget).
7. The index is never serialized into TT snapshots, proof events, or any
   other artifact.

## Design

New file `src/search/dfpn/cross_clock.rs` (keep under 10 KB):

```rust
pub(super) struct CrossClockIndex {
    map: HashMap<u64, (Move, Outcome, u32)>, // rep_key -> (best_move, outcome, proven depth)
    capacity: usize,                          // hard bound; drop inserts when full
    #[cfg(test)] pub(super) hits: u64,        // test instrumentation, like plan9
}
```

- Key: `pos.repetition_key()` — the documented board-only key
  (`zobrist::board_hash`), clock-independent by construction. This is the
  *value-side* attack: the full position key (incl. rule50) stays untouched.
- Payload: `(best_move, outcome, depth)`. `best_move` is required to inherit
  the one-ply guard on adoption; `depth` feeds the budget rule.
- Capacity: default `1 << 18` (matching the repetition cache default), hard
  bound, drop inserts when full — deterministic, no eviction. Record the
  actual working set in the report and shrink if far below.
- Store site: the frame-exit TT store in `core.rs` `dfpn` — only when
  `store_outcome` is `Some(Win | Loss)` (never for draws, never for unsolved
  bounds). Terminal-branch stores (`core.rs` entry terminal store) are
  excluded in v1: terminals are re-classified in O(1) at frame entry, so
  their entries would bloat the working set; the spike can revisit this.
- Probe site 1 — `dfpn` node entry (`core.rs`), immediately after the plan9
  repetition-cache probe (order: terminal check → local repetition → exact
  TT solved result → repetition cache → **cross-clock probe** → ordering
  hint), before `sort_moves`. On a hit satisfying the budget rule and the
  one-ply guard: emit `NodeProven(pos, outcome, depth)` and return the
  outcome.
- Probe site 2 — `evaluate_child` (`children.rs`), mirroring the existing
  solved-reuse block (single TT probe → `resolved_from_entry` → one-ply
  guard): when the exact-key `resolved` is `None`, probe the index with
  `child_rep_key` and adopt under the same rule (`rule50 + depth ≤ 100`,
  `depth ≤ child_max_depth`, one-ply guard). This site classifies ~95% of
  children without any descent — the win concentrates here. `repetition_seen`
  stays `false`, exactly like an exact-key resolved hit.
- Lifetime mirrors the TT: the payload is a proof fact, sound regardless of
  eviction, so the index persists across `begin_run` (like the TT) and is
  cleared wherever `tt.clear()` is called. Never serialized into TT
  snapshots, proof events, or any other artifact.
- Position of the adoption relative to the plan9 cache: the repetition-cache
  probe keeps its current position (after the exact solved check). A
  position cannot be both a proven repetition-dependent draw and a stored
  win — Win proofs are path-independent, so no genuine draw proof can coexist
  for the same (position, context) — but the repetition probe keeps its
  existing place so plan9's ordering invariants stay untouched.

Shift-rule boundary cases to pin in tests: `(rule50, d) = (99, 1)` → adopt
(the leaf is a mate/extinction at clock ≤ 100, both outrank rule50);
`(99, 2)` → reject (an interior node could sit at clock 100); `(98, 2)` →
adopt.

### Alternatives considered (and rejected)

- **Clock-scan at probe** (`board ^ rule50_key(c)`, `c in 0..=100`, the
  reconstruct pattern): 101 probes per node on the hot path — rejected.
- **Storing shadow entries inside the main TT under the normalized key**
  (`board_key ^ rule50_key(0)`): collides with the real clock-0 position's
  entry and adds eviction churn to the primary table. Rejected.
- **Removing rule50 from the Zobrist key**: non-goal (Motivation).

## Phase 0 — sizing spike (go/no-go, temporary instrumentation, then reverted)

1. Instrument both exact-key reuse sites (temporary counters + the
   reconstruct-style board scan over `c in 0..=100` *only inside the
   instrumented build*): count nodes/children where a same-board solved
   Win/Loss entry exists that satisfies the adoption rule; record the
   `(rule50 − rule50_stored)` delta distribution and the would-be savings
   upper bound (the adopted entry's cumulative `work`).
2. Measure: stress case first-outcome (`--timeout 120 --first-outcome
   --outcome-only`), one default-mode run, and `m22_white` as a no-adoption
   control. Record adoption rate, distinct board keys (working set), and the
   estimated saving.
3. **Go/no-go**: proceed only if the projected `child_evals` reduction on the
   stress case is meaningful (report the threshold; order of ≥ 5% is the
   expected bar, given plan9's −29% from the neighboring lever). If the
   would-be adoptions are rare or the working set is pathological, close the
   item with the measurement, write `report10.md` documenting the negative
   result, and re-rank the backlog (#3 becomes next).

## Tasks

1. Phase 0 spike per above; revert all instrumentation; record numbers in the
   report.
2. Implement `cross_clock.rs` with unit tests: store/probe round-trip,
   budget boundary (adopt at `rule50 + depth == 100`, reject at `101`),
   capacity stop, overwrite updates, distinct-clock boards mapping to one
   entry (clock-independent key), draws-never-stored.
3. Wire the store site and both probe sites per the design; add a
   `#[cfg(test)]` dfpn test: solve a small winning position at a low clock,
   reset the same board at a higher (but budget-covered) clock in the same
   `Search` instance, and show the second bounded search hits the index
   (`hits` counter increases) with a materially lower node count and the same
   decisive outcome.
4. Soundness gate: `cargo test --release --test test_repetition --
   --include-ignored` (both cyclic-rook regression tests must pass — the
   index must never manufacture a win there). Add one integration test to
   that file: solving the cyclic rook position at two different clocks in one
   process never yields `Outcome::Win`.
5. Drift protocol: `benchmark --suite quick --json --first-outcome` before vs.
   after. Cases without cross-clock adoptions must be bit-identical (the
   probe adds lookups, never behavior). For intended-delta cases
   (repetition/clock-heavy), verify outcomes unchanged and PVs verified legal;
   list deltas in the report. Run `cargo test --release --test test_move_order
   -- --include-ignored` for regressions.
6. Deterministic-budget contract: `cargo test --release --test test_plan6 --
   --include-ignored` — `BudgetExhausted` behavior untouched.
7. Stress measurement vs. the post-plan9 baseline (first-outcome
   249,480,478 child evals; default mode 338,094,183): record
   `child_evals`/`nodes`/wall time and the PV-length effect. Sanity: the
   cyclic rook position must still never claim a win.
8. Memory check: report the peak index entry count on the stress case; shrink
   the default if the working set is far below the cap (plan9's pattern).
9. Update AGENTS.md (dfpn paragraph: one sentence on the cross-clock index,
   its adoption rule, and its TT-mirrored lifetime) and `initiative.md`
   backlog #2 status.
10. Write `report10.md` in this directory (tools/examples used, problems,
    unresolved parts, missing tests, next steps).

## Validation checklist (summary)

- `cargo fmt`, `cargo clippy --all-targets`, `cargo test --release` (fast
  gate), `cargo doc --no-deps`.
- Repetition soundness gate: `cargo test --release --test test_repetition --
  --include-ignored` (both cyclic-rook regression tests).
- Quick-suite drift (bit-identical except intended deltas) + move-order suite
  (`cargo test --release --test test_move_order -- --include-ignored`).
- Budget-contract tests (`test_plan6`) unchanged.
- Stress-case measurement against the post-plan9 baseline, plus the spike's
  go/no-go numbers.

## Non-goals

- Any change to the Zobrist keys (rule50 stays in the key) or to the main TT
  entry layout/replace policy.
- Adopting Draws across clocks (the repetition cache and first-player-loss
  shortcut stay exactly as plan9 left them).
- Twins, path codes, or Kawano-style simulation (plan5's removed mechanism).
- Cross-path *verification* (backlog #4) — that lever pairs with the cache
  family only after #2's outcome is known.
- Clock-scan lookups in the hot path; wall-time tuning of the added probe
  (that is `lean`'s mandate if the drift gate shows a regression).
- Proof-tree layer / snapshot format changes (the index is never serialized).
