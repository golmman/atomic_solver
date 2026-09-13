# Plan 1: Monotone draw cache v2 — superset-context, cross-clock, draws-only

Initiative: `conversion` backlog #1. Prerequisite reading: `dfpn/plan9.md` and
`dfpn/report9.md` (the exact-context cache this plan generalizes),
`dfpn/report10.md` (the measured no-go whose defect mechanism this plan must
structurally exclude), `dfpn/research_repetition_cache.md` §3–§5 (semantics of
`f(P, A)`), and the working agreements in `initiative.md` (in particular #5:
the monotonicity lemma below is normative and must land as a property test).

## Goal

Generalize the plan9 per-search repetition cache so a repetition-dependent
draw proven at one (clock, ancestor set) is reusable at *nearby* contexts —
clock-drifted revisits of the same board, and revisits whose ancestor set
differs by unrelated earlier excursions — instead of only at the exact
(context hash, clock-qualified hash) match. Primary metric: `child_evals` to
first decisive outcome on the `make stress` case, measured against the
inherited post-plan9 baseline (first-outcome 249,480,478 child evals; default
mode 338,094,183). No behavior change other than work reduction: every
outcome and PV must be one the search could have returned before this plan.

Phase 0 is a sizing spike with a hard go/no-go; the effect is unmeasured and
plausibly small (the plan9 cache already intercepts node-level revisits, and
plan10's large win came mostly from a mechanism this plan deliberately does
not implement).

## Background (one paragraph)

The first-player-loss shortcut stores repetition-dependent draws as unsolved
`(1, 1)` TT entries; plan9 added a per-search cache keyed by the exact
`(tt_key, context_hash)` pair, where `tt_key` includes the rule50 component
and `context_hash` is the order-independent multiset hash of the ancestor
repetition keys. Report9 measured 31,620 re-proofs of 1,467 distinct
positions (96% redundancy) and the exact cache cut the stress case by a
further −29% after plan8, but the key is *exact*: a re-entry at the same
board with a drifted halfmove clock, or under an ancestor set extended by an
unrelated earlier excursion, misses and re-walks the draw chain. Plan10
showed that the aggressive fix — reusing cross-clock **Win/Loss** facts as
child bounds — is a measured no-go (DF-PN threshold destabilization, m22_white
3.1 s → 120 s timeout). This plan keeps plan10's *value-side* idea (key on the
board-only `repetition_key`, not the clock-qualified hash) but applies it to
**draws only, at frame entry only**, with a reuse rule justified by a
monotonicity lemma instead of by empirical sampling.

## The monotonicity lemma (normative)

**Definitions.** A *search position* is a triple `(P, A, c)`: `P` is the board
state including side to move (everything `repetition_key()` and the terminal
checks depend on), `A` is the set of repetition keys of the positions
strictly above the node on the current path (`path_stack` before this
frame's `path_push`), and `c` is the halfmove clock of `P`. Terminals, in the
solver's evaluation order:

- **T1** — no legal moves (checkmate/stalemate), commoner extinction,
  `occupied == 2`: a function of `P` only.
- **T2** — `c ≥ 100` → `Draw`.
- **T3** — `rep_key(P) ∈ A` → `Draw` (the solver's two-fold-repetition-as-draw
  rule, `path_contains`).

The *ideal value* `val(P, A, c) ∈ {Win, Loss, Draw}` is the side-to-move
negamax game value under these terminals with unbounded depth and work
(Win iff some child is Loss, Loss iff all children are Win, Draw otherwise).

**Fact 1 (genuine proofs).** Whenever the search completes a solved Draw
(the `suppress_draw` branch), the stored Draw equals `val(P, A, c)` at the
store-time triple. Solved results propagate only through genuine terminals:
depth-truncation leaves and work-bounded chunk stops store unsolved `(1, 1)`
bounds and can never complete a proof (the `remaining_depth`/non-degeneracy
guards in `children.rs`/`selection.rs` exclude them).

**Fact 2 (repetition-free decisiveness).** If `val(P, A, c)` is decisive, it
is witnessed by a finite proof tree none of whose nodes is a T2 or T3
terminal. A T2/T3 terminal evaluates to `Draw`, and a `Draw` child supports
neither role: a Win witness needs a Loss child at its OR node and all-Win
children at its AND nodes; a Loss witness needs all children Win.

**Lemma (decisive monotonicity).** If `val(P, A′, c′)` is decisive and
`A ⊆ A′`, `c ≤ c′`, then `val(P, A, c) = val(P, A′, c′)`.

*Proof.* Take the Fact-2 witness tree `T` for `(P, A′, c′)` and replay the
identical tree under `(P, A, c)`. Along any root-to-node path both replays
visit identical boards, so: (i) T1-terminality of every node is identical;
(ii) the clock increment per ply is board-determined, hence the replayed
clock at each node is ≤ the original one (`c ≤ c′`), so a node with replayed
clock ≥ 100 had original clock ≥ 100 — a T2 terminal, excluded from `T`;
(iii) the replayed ancestor set of each node is the original set minus
`A′ \ A`, so a node repeating against the replayed set also repeated against
the original set — a T3 terminal, excluded from `T`. No node of `T` becomes a
terminal in the replay that was not already a terminal in the original, and
every leaf of `T` is a T1 terminal with an identical evaluation. By induction
over `T`, every node's value is preserved. ∎

**Corollary (reuse rule).** `val(P, A, c) = Draw`, `A′ ⊇ A`, `c′ ≥ c`
⟹ `val(P, A′, c′) = Draw`. (If the probe triple were decisive, the lemma —
with `A ⊆ A′`, `c ≤ c′` — would make the store triple decisive with the same
value, contradicting the stored Draw.)

In words: a decisive proof never leans on a repetition or on the 50-move
rule, so it survives both taking repetition-escape options away from the
defender (shrinking `A`) and handing the attacker more quiet-move budget
(shrinking `c`). A stored Draw therefore transfers to every probe context
where drawing is at least as easy: **more ancestors** and a **larger clock**.
This is exactly the backlog #1 rule: *reuse iff probe clock ≥ stored clock
and probe ancestor set ⊇ stored set.*

Boundary notes the implementation must respect:

- The lemma is stated over the solver's actual semantics (two-fold-as-draw,
  rule50 draw at 100, `repetition_key = board.hash()` covering piece/side/
  castling/en-passant). It is not a statement about official threefold
  repetition rules; re-derivation under other semantics is out of scope.
- The search's `repetition_seen` flag tracks subtree *search history*, not
  proof structure, so stored Win/Loss entries may carry it. Irrelevant here:
  v2 never stores or returns decisive values.
- Worst-case defect bound, independent of the lemma: hits return at frame
  entry exactly like path-repetition terminals and only ever as `Draw` — the
  worst possible defect is a false Draw (the same bound plan9 has), never a
  false decisive outcome, and no bound is ever folded into a parent's
  `(pn, dn)` (the plan10 pathology is structurally absent).

## Soundness contract (must hold after the change)

1. Only `Outcome::Draw` results whose proof was repetition-dependent
   (`suppress_draw` branch) enter the cache; wins/losses never enter or
   leave it.
2. Cache key = board-only `repetition_key()`. Payload = `(clock, ancestors,
   depth)` with `clock = pos.rule50()` at proof time, `ancestors` the
   complete ancestor repetition-key set strictly above the node (sorted,
   deduplicated), `depth` the proven depth.
3. A hit requires `probe_clock ≥ stored_clock` **and** `stored_ancestors ⊆
   path_stack` (checked per key with `path_contains`; the probe runs before
   this frame's `path_push`, so the stack is exactly the ancestor context).
   No other relaxation exists.
4. The cache is cleared once per run in `Search::begin_run` (never per chunk,
   never per refinement round, regardless of prefix presence) and is never
   serialized into TT snapshots, proof events, or any other artifact.
5. A hit behaves exactly like a freshly proven repetition-dependent draw:
   returns `Draw` and routes through `emit_proof_node` (a `Draw` emits no
   `NodeProven`, matching the shipped plan9 behavior). A hit consumes no
   child-eval budget.
6. `child_eval_budget` / `ExitReason::BudgetExhausted` semantics unchanged.
7. The reuse rule is never applied at child-classification sites
   (`evaluate_child`): hits only ever short-circuit a frame entry, never
   become child bounds.

## Design

Rewrite `src/search/dfpn/repetition_cache.rs` in place (v2 strictly subsumes
v1: equal clocks satisfy `≥`, identical sets satisfy `⊇`, so every plan9 hit
remains a v2 hit; the file name, capacity model, clear point, and the
`#[cfg(test)] hits` counter are kept, `context_hash` is removed):

```rust
pub(super) struct RepetitionCache {
    map: HashMap<u64, Vec<Entry>>, // rep_key -> entries, insertion order
    capacity: usize,               // hard bound; drop inserts when full
    #[cfg(test)] pub(super) hits: u64,
}

struct Entry {
    clock: u16,           // rule50 at proof time; always ≤ 99 (T2 is terminal)
    ancestors: Box<[u64]>, // sorted, deduplicated ancestor rep keys
    depth: u32,           // proven depth; outcome is Draw by contract
}
```

- **Probe** (same slot as the plan9 probe, `core.rs` `dfpn` entry: after the
  terminal check, `max_depth` check, local-repetition check, and the TT
  solved-result check; before `sort_moves`): look up `rep_key`; scan that
  key's entries in insertion order; the first entry satisfying
  `pos.board().rule50() >= clock` and `ancestors.iter().all(|k|
  self.path_contains(*k))` is a hit → return `Draw`. Insertion-order scan
  keeps the result deterministic. The HashMap miss path is one lookup, same
  cost shape as plan9; the subset check only runs when an entry exists under
  the same board.
- **Store** (same `suppress_draw` branch): `ancestors = path_stack[..len-1]`
  cloned, sorted, deduplicated (the stack never contains duplicates and its
  last element is this frame's own key, pushed after the probe-site snapshot
  and popped only after the store). Clone cost is paid only on
  repetition-dependent draw proofs (measured working set 1,643 entries on
  the stress case, report9). Capacity default stays `1 << 18`; drop inserts
  when full (deterministic, no eviction); record the actual working set and
  entry-size distribution in the report and shrink if far below.
- **No depth guard on hits** — same as plan9, justified by Fact 1: a stored
  Draw is the ideal game value at its triple, and the corollary's reuse rule
  carries it to the probe triple regardless of `max_depth`.
- **No one-ply `best_move_repeats_path` guard** — a Draw hit uses no stored
  move; the guard is a TT-reuse mechanism.
- **Lifetime** mirrors plan9 exactly: per-run cache, cleared in
  `begin_run` (observe that `reset_search_state` re-seeds `path_stack` from
  `prefix_path`, so the clear must happen regardless of prefix presence).
- If Phase 0/1 profiling shows the per-probe `path_contains` subset check hot,
  an incrementally maintained ancestor membership structure on
  `path_push`/`path_pop` is the sanctioned follow-up; do not pre-optimize.

### Alternatives considered (and rejected)

- **Storing the context hash instead of the ancestor set**: a multiset hash
  cannot test `⊇`; membership needs the concrete keys. Rejected.
- **Cross-clock Win/Loss reuse** (plan10's mechanism, even as a flag-gated
  option): measured no-go; the benefit and the destabilization are one
  mechanism. Stays closed.
- **Probe at `evaluate_child`** (plan10's site 2, where the measured win
  concentrated): folds solved mass into unsolved parents' threshold
  arithmetic — the exact defect mechanism report10 documents. Excluded by
  soundness-contract point 7, not merely by tuning.
- **Dominance pruning of entries per rep_key** (drop entries subsumed by a
  newer one): a memory optimization only; the working set is measured first.
  Defer unless the report shows growth.

## Phase 0 — sizing spike (go/no-go, env-gated, then either productionized or reverted)

Because v2 is draws-only and self-contained, the spike can be the real
mechanism behind a kill switch rather than a counter-only estimate:

1. Implement the v2 cache per the design behind `CONV1_SPIKE=1` (default
   off = byte-identical plan9 behavior), plus temporary counters: stores,
   probes, v2 hits, **v2-only hits** (hits the exact plan9 cache would
   miss), clock-delta histogram (`probe_clock − stored_clock`), ancestor
   set-size histogram, peak working set and bytes.
2. Measure with the spike on: stress case first-outcome
   (`--timeout 120 --first-outcome --outcome-only`), one stress default-mode
   run, and `m22_white`
   (`4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22`) as the
   control — it must remain a ~3 s win (its baseline: 3.1 s, 858,117 nodes /
   14,156,269 evals). Run `benchmark --suite quick --json --first-outcome`
   and verify all 59 outcomes unchanged.
3. **Go/no-go**: proceed only if (a) v2-only hits are frequent enough that
   the stress case improves by ≥ 5% child evals vs the 249,480,478
   post-plan9 baseline (the plan10 bar, applied to the smaller lever),
   (b) the m22 control regresses by no more than noise, and (c) every
   quick-suite outcome is unchanged. If the v2-only hit surface is thin
   (plan9's exact cache already catches nearly everything) or the control
   degrades, revert all spike code, write `report1.md` documenting the
   negative result with the histograms, and re-rank the backlog (backlog #2,
   candidate-guided outcome search, becomes next).

## Tasks

1. Phase 0 spike per above; record all numbers and histograms in the report.
   On go: remove the env gate and counters, keeping the clean
   implementation; on no-go: revert to byte-identical plan9 state.
2. Productionize `repetition_cache.rs` with unit tests: store/probe
   round-trip; cross-clock rule (store at clock 10 → probe 10 and 15 hit,
   probe 5 misses); superset rule (store `{a, b}` → probe `{a, b}`,
   `{a, b, c}` hit; probe `{a}`, `{a, c}`, `{b, a, c}`-minus-`a` variants
   miss); empty stored set hits any context (data-structure level);
   insertion-order determinism with multiple entries per key; capacity stop;
   ancestors exclude the node's own key; sorted/deduplicated payload.
3. Wire the store and probe sites per the design; add a `#[cfg(test)]` dfpn
   test in `src/search/dfpn/tests.rs`: solve a cyclic position, then re-solve
   within the same `Search` instance under a *changed* context — once with a
   drifted clock and once after an unrelated excursion added earlier on the
   path — and show the cache `hits` counter increases with a materially
   lower node count and `Draw`, never `Win`.
4. Soundness gate: `cargo test --release --test test_repetition --
   --include-ignored` (both cyclic-rook regression tests must pass; the
   cyclic rook must never claim a win). Add the working-agreement-#5
   property test to that file: for a fixed position set (cyclic rook,
   `m22_white`, one decisive quick-suite case), solve twice in one process
   under different budget splits; decisive outcomes must match a
   fresh-process solve exactly — a cache hit never flips an outcome.
5. Drift protocol: `benchmark --suite quick --json --first-outcome` before
   vs. after. Cases without repetition-dependent proofs must be
   bit-identical (the probe adds lookups, never behavior); for
   repetition-heavy cases verify outcomes unchanged and PVs replay-legal,
   and list the deltas in the report. Run
   `cargo test --release --test test_move_order -- --include-ignored`.
6. Deterministic-budget contract: `cargo test --release --test test_plan6 --
   --include-ignored` — `ExitReason::BudgetExhausted` behavior untouched.
7. Stress measurement vs the inherited post-plan9 baseline (first-outcome
   13,907,467 nodes / 249,480,478 child evals / 53.8 s; default mode
   19,943,731 / 338,094,183 / 72.3 s): record `child_evals`/`nodes`/wall
   time and the PV-length effect. Sanity: the cyclic rook position must
   still never claim a win within the timeout.
8. Memory check: report peak entry count and total payload bytes (ancestor
   sets included) on the stress case; shrink the `1 << 18` default if the
   working set is far below it (plan9's precedent).
9. Update AGENTS.md (the dfpn paragraph's repetition-cache sentence: v2 key,
   reuse rule, clear point) and `initiative.md` backlog #1 status.
10. Write `report1.md` in this directory (tools/examples used, problems,
    unresolved parts, missing tests, next steps).

## Validation checklist (summary)

- `cargo fmt`, `cargo clippy --all-targets`, `cargo test --release` (fast
  gate), `cargo doc --no-deps`.
- Repetition soundness gate with `--include-ignored`, including the new
  two-solves-in-one-process property test.
- Quick-suite drift (bit-identical except intended repetition deltas) +
  move-order suite + budget-contract suite.
- Stress-case measurement recorded against the inherited baseline; m22_white
  control clean.

## Non-goals

- Any reuse of solved **Win/Loss** facts across clocks or contexts (plan10's
  measured no-go; see also `initiative.md` non-goals).
- Probe/adoption at child-classification sites (`evaluate_child`) — v2 hits
  exist only at frame entry.
- Cross-run reuse: the cache stays per-run and is never serialized into TT
  snapshots or proof artifacts.
- Changing rule50 handling in the Zobrist keys (dfpn non-goal), or the
  two-fold-repetition-as-draw semantics the lemma is stated over.
- dfpn backlog #4 (bounded cross-path verification of solved results): do
  not fork it — the lemma here may become the soundness argument it needs;
  coordinate, don't duplicate. AND-side ordering signals in general stay in
  `lean` #5; the clock/shuffle-specific signal is conversion backlog #3.
- Throughput work unrelated to repetition churn (`lean`'s mandate).
