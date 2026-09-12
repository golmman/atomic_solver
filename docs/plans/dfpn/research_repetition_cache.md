# Research: Per-search repetition cache (backlog #1 soundness design spike)

Created 2026-09-12 as the soundness design spike required by `initiative.md`
backlog #1 before plan9. Covers the sizing-spike measurements (temporary
instrumentation, since reverted) and the argument for why per-search scoping
cannot reintroduce the plan5 false win.

## 1. What the current solver does with repetition-dependent draws

- A child whose repetition key is on the current path is a terminal `Draw`
  (`evaluate_child`, `repetition_seen = true`); a `dfpn` frame whose board is
  already on the path returns `Draw` before any TT use (`path_contains`).
- A node whose proven `Draw` *depends* on such a child (`repetition_seen`
  propagates only through the selected-child chain of `Draw` selections) is
  stored in the TT as an unsolved `(1, 1)` entry — the first-player-loss
  shortcut (report7).
- Consequence: the entire chain of draw proofs rooted at a repetition-dependent
  node is uncacheable. Every re-entry — df-pn threshold re-expansion on the
  same path, later work chunks, transposed paths, refinement rounds — re-walks
  the whole region down to the repetition leaves.

## 2. Sizing spike (2026-09-12, temporary instrumentation, reverted)

Instrumented the `suppress_draw` store branch in `dfpn` with per-run counters:
total repetition-dependent draw proofs (`suppress_draw_stores`), re-proofs
(`repeat_stores`: stores for a `tt_key` already proven repetition-dependent
this run), and per-proof subtree work (`child_evals - child_evals_start` at
store time, summed separately for first proofs and re-proofs).

Stress case `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`,
`--first-outcome` (deterministic; node count reproduced the 2026-09-11
baseline exactly):

```text
suppress_draw_stores=33087 repeat_stores=31620 first_proof_work=1597
repeat_proof_work=34118 distinct_keys=1467
total_child_evals=351297052 total_nodes=20313879
```

Cyclic control `8/8/8/8/2k5/8/8/4KR2 w - - 0 1`, `--first-outcome --timeout 10`
(time-bounded, so node counts vary per run):

```text
suppress_draw_stores=16621 repeat_stores=16467 first_proof_work=155
repeat_proof_work=16490 distinct_keys=154
total_child_evals=77122318 total_nodes=13840384
```

Findings:

1. **~96% of repetition-draw proofs are re-proofs** (31,620/33,087 on the
   stress case; 16,467/16,621 on the cyclic control), spread over very few
   distinct positions (1,467 / 154).
2. **The proof frames themselves are cheap** (first proofs sum to 1,597
   child evals on the stress case — their children hit the path-repetition
   terminal after one eval each). The direct re-proof work is not the cost.
3. The cost is the **churn around the uncacheable chain**: because the
   repetition-dependent draw and every ancestor draw proof above it are stored
   unsolved, the TT cannot prune re-entry into these regions; the search pays
   movegen + child evaluation at every frame of every re-descent (20.3M nodes /
   351M child evals total).

Design implication: an O(1) lookup at the repetition-draw node alone saves
little; the win comes from making the *whole chain* cacheable so re-entries
stop descending. The cache must therefore store every node whose proof is
repetition-dependent (the same set `repetition_seen` already identifies), not
only the near-leaf repetition hits.

An earlier interval-based work metric (child-eval delta between consecutive
stores of a key) was discarded: nested re-proofs overlap, producing 28.9B
against a 351M total — meaningless as an absolute number.

## 3. Semantics: the value of a node given an ancestor set

Notation: for a search path, let A be the **set of repetition keys** of the
positions strictly above node P (the set `path_contains` tests against). The
solver's repetition semantics define a partial game:

- terminal edges: commoner extinction / stalemate / rule50 — all determined by
  the position state alone (the halfmove clock is part of the hash);
- repetition edges: any move leading to a board whose repetition key is in the
  current path's key set evaluates as `Draw`.

Claim: under these semantics, the value of P is a deterministic function
f(P, A) of the position (including its clock) and the ancestor key set.

Proof sketch, by structural induction over the sub-search below P:

- The sub-search below P is driven by P's state, the moves it generates, and
  the repetition checks. Every repetition check below P tests membership in
  A ∪ (keys of boards below P on the current line); the boards below P are
  determined by P's subtree structure. So the same (P, A) yields the same
  terminal and repetition classifications at every frame.
- The two path-dependent mechanisms beside `path_contains` — the one-ply
  `best_move_repeats_path` TT guard — also test set membership only, so equal
  A gives equal decisions.
- TT entries are path-independent (solved results prove wins/losses/draws that
  hold in any context; unsolved bounds only steer thresholds and never
  conclude a proof). Which proofs happen to be cached differs between
  traversals, but any *conclusion* is the same function of (P, A).

Also note the path stack never contains a duplicate repetition key (a repeat
returns `Draw` before being pushed), so "set" is accurate, not multiset.

## 4. Why per-search scoping cannot reintroduce the plan5 false win

The plan5 false win (`8/8/8/8/2k5/8/8/4KR2 w - - 0 1` claimed a win) arose
because a **twin-stored win** was reused under a path whose ancestor set did
not contain the repetition the proof relied on, and `simulate` neither carried
the twin's original ancestor set nor re-searched on failure
(`research_ghi.md` §9).

The proposed cache differs from twins+simulation at every step of that failure
chain:

1. **Only `Draw` is ever stored or returned.** Repetition edges evaluate as
   draws, so no Win proof can route through one (a winning child must be a
   Win), and no Loss can depend on one (a Loss requires *all* children to be
   wins; a repetition-draw child excludes it). By induction, wins and losses
   are repetition-independent — which is exactly why `repetition_seen` only
   ever accompanies `Draw` in the current code. A cache that returns only
   draws can therefore never manufacture a false decisive outcome; the worst
   possible defect is a **false draw**.
2. **No cross-context reuse.** The cache key contains the *complete* ancestor
   key set; a hit is only possible when the probe path's ancestor set is
   identical to the store path's. The plan5 failure mode (proof relying on a
   repetition legal under the store path but absent under the probe path) is
   excluded by construction. Cross-context reuse is explicitly out of scope
   (backlog #4's bounded verification is the separate lever for that).
3. **A hit is a semantic identity, not a guess.** f(P, A) = Draw means "under
   the solver's repetition semantics with ancestor key set A, P is a draw" —
   on an exact key match the reuse returns precisely the value the search
   would recompute. No simulation, no best-move chain following, no fallback
   path exists that could silently weaken a rejected reuse into base bounds.
4. **Per-run scoping bounds the blast radius.** Entries live outside the TT,
   are discarded at `begin_run`, and never leak into TT snapshots or proof
   dumps as path-independent facts. A defective entry cannot outlive the run
   that produced it.

Residual risk to test explicitly: a **false draw** would require the value
claim f(P, A) to be wrong, which by §3 requires the sub-search below P to
conclude a draw proof from something that is *not* context-invariant. The only
candidates are TT-solved results (context-invariant) and terminal/repetition
classifications (context-invariant given (P, A)). The repetition soundness
gate (`tests/test_repetition.rs --include-ignored`) plus a new regression test
that solves a repetition-heavy position twice in one process (different
`Search` instances must agree; a second `solve` on the same instance after a
first decisive result must not flip) guard this.

## 5. Design decisions fixed by this spike

- **Node identity = full hash** (`tt_key`, includes rule50 clock), not the
  repetition key: two boards with the same repetition key but different clocks
  can have different values. The initiative's "(repetition key, path context)"
  wording is refined accordingly; the *context* component remains
  repetition-key based because that is what the repetition rule consumes.
- **Context hash = order-independent multiset hash of `path_stack`** (e.g.
  wrapping sum of splitmix64-mixed keys). Order-independence is required for
  transposition reuse (rook triangulation reaches a board via permuted move
  sequences with equal ancestor sets); sum-of-mixed is multiset-safe even if
  the no-duplicate path invariant were ever violated.
- **Outcome perspective is key-free**: a cached `Draw` is Draw from either
  side's perspective, so `is_or_node` is not part of the key.
- **Clear at `begin_run` only.** `reset_search_state` runs per work chunk and
  must not clear the cache (within-run reuse across chunks is a main benefit);
  every prefix change goes through `begin_run`
  (`search_depth_with_prefix` sets the prefix first), so clearing there is
  sufficient.
- **Cache hits must emit `NodeProven`** (with the cached depth) so the offline
  proof-tree reconstruction sees the same event stream shape as a fresh proof.
- **Bounded memory**: a fixed-capacity map with deterministic
  insertion-stop-when-full (no eviction, no randomness). Spike shows the
  working set is small (1,467 distinct keys first-outcome; contexts multiply
  it, so cap and measure in plan9).

## 6. Open items handed to plan9

- Choose and document the capacity bound (spike suggests ≤ 2^20 entries is
  generous; measure the stress case's actual (key, context) count).
- Confirm the quick-suite drift surface: cases without repetition-dependent
  proofs must stay bit-identical; list any case whose `child_evals` changes.
- Measure the stress-case delta (primary metric `child_evals` to first
  decisive outcome; baseline recorded in `initiative.md`).
