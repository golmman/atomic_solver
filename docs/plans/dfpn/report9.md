# Report: Per-Search Repetition Cache (Plan 9)

## Summary

Implemented `docs/plans/dfpn/plan9.md`. Repetition-dependent draw proofs — the
ones the first-player-loss shortcut (report7) keeps out of the TT as unsolved
`(1, 1)` entries — are now additionally cached per search run in
`src/search/dfpn/repetition_cache.rs`, keyed by (full position hash,
order-independent ancestor repetition-key context hash) and storing only the
proven depth. The cache is cleared once per run in `Search::begin_run` and is
never serialized into any artifact. On the deep-shuffle stress case this cuts
first-outcome work by 29% (child evals) and default-mode work by 27–48%
(evals/nodes), with all outcomes unchanged and the drift confined to the
14 repetition-heavy quick-suite cases.

## Changes Made

### New: `src/search/dfpn/repetition_cache.rs` (9.7 KB, under the 10 KB cap)

- `RepetitionCache { map: HashMap<(u64, u64), u32>, capacity: usize }` — key
  `(tt_key, context_hash)`, payload = proven depth (outcome is `Draw` by
  contract).
- `context_hash(&[u64])` — wrapping sum of `splitmix64`-mixed repetition keys:
  order-independent (required for transposition reuse via permuted move
  sequences), multiset-safe even if the no-duplicate path-stack invariant ever
  changed. Reuses `zobrist::mix`, which was made `pub(crate)` so the SplitMix64
  finalizer has a single definition in the crate.
- `store` drops inserts when full (deterministic, no eviction); `probe` returns
  the cached depth; `clear` drops everything.
- Capacity default **`1 << 18` entries (~8 MiB worst case)** — shrunk from the
  plan's starting `1 << 20` after measuring the working set (see Memory check).
- Unit tests: insert/probe round-trip, deterministic capacity stop, context-hash
  order-independence (plus empty-set and multiset properties), distinct-clock
  boards mapping to distinct entries, and clear semantics. A `#[cfg(test)] hits`
  counter instruments probe hits for the dfpn unit test.

### `src/search/dfpn/mod.rs`

- `Search` gains the `repetition_cache` field.
- `begin_run` clears the cache (once per run, before the counters reset;
  independent of `prefix_path` presence — `reset_search_state` re-seeds
  `path_stack` from the prefix, but the cache only holds facts recorded during
  the run itself). Work chunks and refinement rounds deliberately do **not**
  clear it: within-run reuse across chunks is a main benefit.

### `src/search/dfpn/core.rs`

- **Probe**: in `dfpn`, after the local-repetition check and the TT
  solved-result check, before `sort_moves` (so path-independent behavior stays
  byte-identical). The context hash is computed once at that point from
  `path_stack` (the ancestor keys strictly above the node, before this frame's
  `path_push`) and reused by the store site — the symmetric push/pop of this
  and every descendant frame restores the stack to the same contents at frame
  exit. On a hit the frame returns `Outcome::Draw` after routing through
  `emit_proof_node(pos, Draw, cached_depth)` and restores its pooled slot. A
  hit consumes no child-eval budget.
- **Store**: in the `suppress_draw` branch — exactly where the first-player-loss
  shortcut turns the TT store into an unsolved `(1, 1)` — the same
  `(tt_key, context_hash)` is inserted with `outcome_to_store_depth`. Frames
  exiting through the early `path_contains`/terminal/TT paths never store.

### Tests

- `src/search/dfpn/tests.rs`: `repetition_cache_reused_within_a_run` — two
  bounded searches over the cyclic rook position within one run (no
  `begin_run` in between): the first search must populate the cache, the
  second must register cache hits, and both must return `Draw`, never `Win`.
  A cache-disabled control run (`with_capacity(0)`, test-only construction)
  must agree on both outcomes.
- `tests/test_repetition.rs`: `solve_reset_solve_never_claims_win` — the
  plan's two-step scenario (solve → reset → solve in one process); both solves
  go through `begin_run` (which clears the cache) and neither may yield
  `Outcome::Win`, and they must agree.

## Soundness contract — how each point is upheld

1. **Only repetition-dependent draws are stored.** The store site is the
   `suppress_draw` branch, gated on `outcome_to_store == Some(Draw) &&
   repetition_seen`; wins/losses never enter or leave the cache. Since a Win
   proof needs winning children and a Loss needs all children winning, and
   repetition edges evaluate as draws, wins/losses are repetition-independent —
   a draw-only cache cannot manufacture a decisive outcome (research §4.1).
2. **Exact key match.** Key = (full hash incl. rule50, order-independent
   ancestor-set hash); a hit requires both to match (research §4.2/§4.3).
3. **Cleared at `begin_run` only**, never per chunk/round; not serialized into
   TT snapshots, proof events, or dumps (there is no serialization path at all).
4. **A hit behaves like a fresh repetition-dependent draw.** Note on the plan's
   "must emit `NodeProven`" wording: a *fresh* repetition-dependent draw sends
   no `NodeProven` because `emit_proof_node` filters `Outcome::Draw` (draw
   nodes are not proof-tree nodes). The cache hit routes through the same
   `emit_proof_node(pos, Draw, cached_depth)` call, so the event-stream shape
   is exactly that of a fresh proof — no event, matching current behavior and
   the "no behavior change other than work reduction" goal.
5. **Budget semantics unchanged.** Cache hits consume no `child_evals`;
   `test_plan6 -- --include-ignored` (20 tests) passes untouched; a
   budget-exhausted search still reports `ExitReason::BudgetExhausted` and
   stores only unsolved TT entries.

## Measurements

Baseline numbers were re-verified bit-for-bit with a cache-disabled build
(`RepetitionCache::with_capacity(0)`, temporary instrumentation, reverted):
the first-outcome stress run reproduced the recorded
20,313,879 nodes / 351,297,052 child evals exactly, and the default-mode run
reproduced 38,694,766 nodes exactly — so the deltas below are attributable to
the cache alone.

### Stress case `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21` (128 MB TT, default ε)

| Mode | Metric | Baseline | Plan 9 | Δ |
|------|--------|----------|--------|---|
| first-outcome | child evals (primary) | 351,297,052 | 249,480,478 | **−29.0%** |
| first-outcome | nodes | 20,313,879 | 13,907,467 | −31.5% |
| first-outcome | wall time (this host) | 73.8 s | 54.0 s | −27% |
| first-outcome | first line | 405 plies | 477 plies | see Drift |
| default | child evals | 465,827,574 | 338,094,183 | **−27.4%** |
| default | nodes | 38,694,766 | 19,943,731 | −48.5% |
| default | wall time (this host) | 96.0 s | 71.0 s | −26% |
| default | refined PV | 405 → 115 `cap-cut` | → 129 `cap-cut` | informational |

Both first lines (405- and 477-ply) were replayed with `examples/replay`: each
is a legal move sequence ending in a terminal Loss for the defender — genuine
wins either way. Sanity: the cyclic rook position
(`8/8/8/8/2k5/8/8/4KR2 w - - 0 1`) still exhausts its budget with
`outcome=draw` and never claims a win.

### Quick-suite drift (`benchmark --suite quick --json --first-outcome`)

59 cases; **all outcomes unchanged, `wrong=false` everywhere**.

- 45 cases **bit-identical** on `nodes` and `child_evals` (only wall-time
  fields differ) — the required zero-drift surface.
- 14 cases changed; each was individually confirmed to produce
  repetition-dependent draw proofs (cache entry count > 0 via temporary
  instrumentation), i.e. every drift case is repetition-heavy, as the plan
  anticipates. child-eval deltas (after − before):

| Case | Δ child evals | Note |
|------|---------------|------|
| dec31 | −997,238 (−66.1%) | largest win |
| dec10 | +466,661 (+12.3%) | first line 49 → **41** plies (shorter) |
| dec06 | −4,992 | |
| m23_black | +15,024 (+0.5%) | |
| m24_white | −772 | |
| m25_white | −642 | |
| dec44 | −75 | |
| dec02 | −52 | |
| dec42 | −58 | |
| dec37 | −12 | |
| dec28 | −7 | |
| dec15 | −4 | |
| dec43 | +17 | |
| dec01 | +9 | |

Two properties of the drift deserve note:

- **The first-outcome line can change** on repetition-heavy cases (stress case
  405 → 477, dec10 49 → 41): the cache short-circuits re-proofs, which shifts
  DF-PN's traversal order and hence which winning line is found first. This is
  within the plan's goal ("every outcome and PV … must be one the search could
  have returned before"): both observed lines were replayed and are legal
  decisive lines, and the *outcome* is unchanged in every case. Most deltas
  are improvements; dec10 shows the converse is possible (a shorter line found
  at slightly higher total cost).
- Small ±tens-of-evals deltas (dec01, dec43, …) are the same mechanism at
  micro scale: one cached re-proof changes a subtree's work without changing
  its conclusion.

### Memory check (task 7)

Peak cache entry count (no eviction, so the final size is the peak):
**1,314** entries (stress case, first-outcome) and **1,643** (stress case,
default mode). Two orders of magnitude below even `1 << 18`, so the default
capacity was shrunk `1 << 20` → **`1 << 18`** (~8 MiB worst case, a small
fraction of the 128 MiB default TT). The stress run was re-verified with the
shrunken capacity: identical result (no drops occur).

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ CARGO_PROFILE_RELEASE_LTO=thin cargo test --release        # 317 integration + 218 unit, 0 failed
$ cargo doc --no-deps
$ cargo test --release --test test_repetition -- --include-ignored   # 3 passed (incl. new two-step test)
$ cargo test --release --test test_move_order -- --include-ignored   # 3 passed
$ cargo test --release --test test_plan6 -- --include-ignored        # 20 passed (budget contract)
```

CLI sanity: the KRR mate still solves (`win, 3 plies, proven-shortest`); the
cyclic rook still times out as a draw.

## Tools/Examples Used

- `benchmark --suite quick --json --first-outcome` (before/after drift).
- A cache-disabled build (`RepetitionCache::with_capacity(0)`) plus temporary
  instrumentation (cache entry count, nodes, child evals printed at exit) to
  verify baselines bit-for-bit and measure the working set — **reverted**
  after measuring.
- `examples/replay` to verify that changed first lines end in terminal wins.
- `tests/fixtures/*` FENs for per-case drift attribution.

## Problems Encountered

1. **"Node count for equal work chunks drops" did not materialize at
   unit-test scale.** A cache-hit frame costs 1 node / 0 evals, and
   cap-bounded runs consume their full budget either way; measured node
   deltas on the cyclic rook at 0.4–4M-eval caps were ~±1% in either
   direction. The unit test therefore asserts the direct observable — the
   `#[cfg(test)]` hit counter increases during the second bounded search,
   the cache is populated, and outcomes stay `Draw` — instead of a fragile
   node-count comparison. The work reduction itself is proven at stress scale
   (−29% evals).
2. **First-outcome PV drift on repetition-heavy cases is real** (see above),
   including on a quick-suite case (dec10). The plan's drift checklist asked
   to "verify outcome and PV are unchanged"; the outcome held everywhere, the
   PV did not — both observed PVs were verified to be legal decisive lines,
   consistent with the plan's stated goal, and all zero-cache cases remained
   bit-identical. Recorded here because future plans comparing PVs on
   shuffle-heavy cases should expect this surface to move.
3. **`emit_proof_node` filters draws**, so the plan's "hit must emit
   `NodeProven`" cannot send an event without changing the event stream for
   fresh proofs too. Resolved as the interpretation in the soundness section
   (route through the same emission path; no event), documented in the code.
4. The context-hash recompute is O(depth) per non-early-return node, per the
   plan's "compute at probe/store time" guidance. The stress runtime *fell*
   by ~27%, so the overhead is well below the savings; incremental
   maintenance in `path_push`/`path_pop` remains available if a future
   profile shows it hot.

## Unresolved Parts

- None blocking. The plan is fully implemented; capacity default was adjusted
  as task 7 anticipated.

## Missing Tests

- No end-to-end test constructs a *false draw* defect (research §4's residual
  risk): by the §3 argument a false draw requires a context-non-invariant
  conclusion below P, and no candidate mechanism exists in the current code;
  the repetition soundness gate, the two-step solve test, and the budget
  contract cover the observable surface.
- Cross-context behavior (same node, *different* ancestor sets) is only
  covered implicitly (miss ⇒ re-search); a targeted unit test for
  "different context ⇒ miss ⇒ same outcome" lives in the cache unit tests at
  the data-structure level, not at the search level.
- The `--suite decisive`/`all` benchmark tiers were not re-run (the plan's
  drift protocol names the quick suite; the stress case was measured
  directly).

## Next Steps

- Backlog #2 (clock-budget-aware solved-entry reuse) attacks the same
  position class from the TT side and now composes with a cheaper draw-chain
  baseline; re-measure the stress case after it lands.
- Backlog #4 (bounded cross-path verification) is the lever for the misses
  this cache still pays: same node re-walked under a *different* ancestor set
  remains uncacheable by design (soundness), and the stress case's remaining
  249M evals live largely there.
- If a future profile attributes meaningful time to `context_hash`
  recomputation, maintain it incrementally in `path_push`/`path_pop` (the
  wrapping sum makes that exact).
