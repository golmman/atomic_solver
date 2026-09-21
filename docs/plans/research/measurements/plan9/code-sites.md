# plan9 — code-site verification notes

Verified 2026-09-21 by reading the files (plan9 Phase 1, mapping step).
Every claim in `research_deep_dfpn.md` §4–§5 traces to an excerpt here.
No `src/` or `examples/` change was made; git diff clean.

## 1. Unsolved-leaf assignment sites

### 1a. `src/search/dfpn/children.rs` — `evaluate_child`, final `else` branch

```rust
let (pn, dn) = match entry.as_ref() {
    Some(e)
        if e.outcome.is_none()
            && e.pn > 0
            && e.dn > 0
            && e.remaining_depth != u32::MAX
            && e.remaining_depth <= child_max_depth =>
    {
        (e.pn, e.dn)
    }
    _ => (1, 1),
};
```

The `(1, 1)` fallback is df-pn's unsolved-leaf value — the site
`D_dfpn(depth) = E^(D−depth)` would replace. Note the reuse arm: a stored
TT bound takes precedence over the neutral fallback, so the hot path for
re-visited positions is *not* the leaf-constant site at all.

### 1b. `src/search/dfpn/core.rs` — `max_depth == 0` bounded leaf

```rust
if max_depth == 0 {
    // A non-terminal leaf is an unsolved frontier, not a proven draw.
    self.tt
        .store(tt_key, Move::NONE, u8::MAX, 0, None, 1, 1, 0, 0);
    self.precompute_pool[slot_depth] = slot;
    return Outcome::Draw;
}
```

The frontier leaf stores (1, 1) into the TT keyed by position hash with
`remaining_depth = 0`.

## 2. Depth-like quantities available at those sites

- **Path depth**: `self.path_stack.len()`. Already read in `evaluate_child`
  (`let scratch_depth = self.path_stack.len();` — the pooled scratch-slot
  index) and in `dfpn` (`let frame_depth = self.path_stack.len();` after
  `path_push`; doc: "Depth = `path_stack.len()` after `path_push` is unique
  among active frames; it equals `slot_depth`"). So the answer to plan
  Context 6(i) is **yes, a depth source exists** — the classification does
  not fail on "no depth source".
- **Remaining chunk budget**: `max_depth` is a parameter at both sites
  (`child_max_depth = max_depth.saturating_sub(1)` in `evaluate_child`).
- **Halfmove clock**: `pos.board().rule50()`; the clock is part of the
  Zobrist key (`src/zobrist.rs`, per AGENTS.md "including the halfmove
  clock"), hence a function of the hashed position — but it measures
  depth-since-last-irreversible-move, not depth-from-root (directionally
  wrong as a proxy; see classification (d)).

## 3. Where the values feed selection and thresholds

### 3a. `src/search/dfpn/selection.rs` — folds

```rust
// select_from_children / selection_for_child, OR side:
pn = std::cmp::min(pn, c.pn);
dn = std::cmp::min(INF, dn.saturating_add(c.dn));
// AND side mirrored (sum over c.pn, min over c.dn)
```

`best_and_second_unsolved` takes the argmin over `c.pn` (OR) / `c.dn` (AND)
excluding solved/explored children. Unchanged by the mechanism; its inputs
inflate under D_dfpn.

### 3b. `src/search/dfpn/core.rs` — threshold recursion + 1+ε site

```rust
let (np, nd) = if is_or_node {
    let new_th_pn = std::cmp::min(th_pn, self.epsilon_ceil(second_pn));
    let new_th_dn = if th_dn == INF {
        INF
    } else {
        th_dn.saturating_sub(dn).saturating_add(child_dn)
    };
    (new_th_pn, new_th_dn)
} else { /* mirrored */ };
```

The 1+ε mechanism acts on `second_pn`/`second_dn` here; deep values enter
through `child_dn`/`child_pn` (the best child's bounds) and the §3a folds.
The two compose arithmetically; behaviorally both control the same
stay-deeper dial (the paper's §4 "two methods").

### 3c. `src/search/dfpn/core.rs` — frame-exit store (the crux site)

```rust
let (store_pn, store_dn) = if suppress_draw {
    (1, 1)
} else if outcome_to_store.is_some() {
    (outcome_to_store_pn, outcome_to_store_dn)
} else {
    (pn.max(1), dn.max(1))
};
...
let store_remaining_depth = if outcome_to_store.is_some() {
    u32::MAX
} else {
    max_depth
};

self.tt.store(
    tt_key, store_best_move, store_best_child, work,
    store_outcome, store_pn, store_dn, store_depth, store_remaining_depth,
);
```

Under a path-depth-keyed deep value, the unsolved arm `(pn.max(1), dn.max(1))`
sums D_dfpn values along the path the frame sat on → path-relative. This is
where the `src/search/tt/` path-independent base-entry contract breaks for
the faithful mechanism.

## 4. TT entry contents (quoted, per the plan's verification requirement)

`src/search/tt/entry.rs`:

```rust
pub struct TtEntry {
    pub(crate) key: u64,
    pub(crate) valid: bool,
    pub(crate) generation: u32,
    pub(crate) best_move: Move,
    pub(crate) best_child: u8,   // u8::MAX means "unknown / unset"
    pub(crate) work: u64,        // cumulative child_evals spent under this subtree
    pub(crate) outcome: Option<Outcome>,
    pub(crate) pn: u64,
    pub(crate) dn: u64,
    pub(crate) depth: u32,
    pub(crate) remaining_depth: u32,
}
```

One entry per position `key`; `pn`/`dn` today are pure functions of the
subtree because every leaf contributes the position-only (1, 1) or a
previously stored subtree summary. There is **no path-depth field** — a
path-depth-tagged reuse variant would require a layout change (and a
`table.rs` two-slot bucket change). Probe-side reuse in `evaluate_child`
guards only on `outcome`, `pn/dn > 0`, `remaining_depth != u32::MAX`,
`remaining_depth <= child_max_depth` — no path-depth condition — so
foreign-path bounds are imported as-is under the faithful mechanism.

## 5. GHI / repetition machinery (compatibility check)

- First-player-loss shortcut: `core.rs`
  `let suppress_draw = outcome_to_store == Some(Outcome::Draw) && outcome_to_store_repetition_seen;`
  → stored as unsolved (1, 1) + `repetition_cache.store(tt_key, context_hash, depth)`.
  Correctness is order-independent; deep values only change which lines are
  entered. No break.
- `RepetitionCache` probe: keyed `(tt_key, context_hash)` where
  `context_hash = RepetitionCache::context_hash(&self.path_stack)` computed
  before `path_push` — hit returns `Outcome::Draw` under an identical
  ancestor set. Leaf values do not interact. No break.

## 6. Refinement-round structure (Context 6(ii))

`src/search/dfpn/mod.rs`: `Search::solve_with_progress` runs iterative
bounded chunks — `refinement_rounds`, per-round caps
(`refine_cap_factor_num/den`, `MIN_REFINE_ROUND_EVALS`), cumulative
`child_eval_budget`, per-`dfpn`-call `max_work` chunking; bounded chunks
store unsolved bounds with `store_remaining_depth = max_depth` so "the next
deeper probe can grow past the horizon" (the `max_depth == 0` comment). A
depth-dependent leaf value either stays fixed per path (→ §3c
path-relative stores) or changes per round (keyed to `max_depth` → the
leaf constant is not a round-to-round fixed point). Neither variant has an
analog in the source; no published guidance exists.
