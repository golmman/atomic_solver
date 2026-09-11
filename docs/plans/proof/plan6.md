# Implementation Plan: Proof-Tree Finalization Completeness — Twin Preference + Loss-Completeness Validator

Initiative: `docs/plans/proof/initiative.md`, backlog item #6. This plan is
self-contained; it fixes the proof-tree-layer defect that plan5's dual-build
oracle surfaced (`report5.md`, root-cause analysis) and adds the builder-side
validation tooling that makes the fix observable. It is the precondition for
the worker-off default flip, which it renumbers to **plan7** (report5 called
the flip "plan6"; per working agreement #2 the fix and the flip are separate
levers — this plan is the "small proof-tree plan before plan6" that report5's
next steps call for).

## Goal

1. **`finalize()` must select a completely expanded twin when one exists.**
   Today the canonical `(hash, outcome)` selection can pick an incompletely
   expanded transposition twin over a complete one, preserving an invalid
   proof subtree in the dump (measured on the decisive suite: dec10, dec44,
   dec46 — the live event-built tree was a strict *subset* of the true proof).
2. **A replay-based Loss-completeness validator** that checks a finalized
   tree against the structural rules of a proof (Loss covers *all* legal
   replies, Win has exactly one winning child, depths bottom-up consistent,
   terminals statically correct) and reports defects with their paths.
3. **The experiment verdict becomes unconditional**: re-running plan5's
   `reconstruct_pt --experiment` must report **46/46 oracle-isomorphic**.

Success criteria:

- `make test` green, including the new twin-preference and validator tests.
- Drift check bit-identical: `benchmark --suite quick --json --first-outcome`
  produces identical `child_evals` per case (proof-tree changes must not touch
  search paths). One documented stdout change: a new `pt_validate:` line in
  the solver's pre-exit output.
- `reconstruct_pt --experiment` (release): 46/46 isomorphic, hole stats
  unchanged from report5 (hit/terminal split may shift only because the live
  reference tree grows to full completeness).
- No search-behavior change of any kind.

## Context (current state, verified 2026-09-11)

**The defect mechanism** (`report5.md`, established on dec46): when a position
is solved by a *transposition* frame at path Q, the complete expansion (all
children) is emitted under Q's path; a later frame at the same position's own
path P resolves from the TT and re-emits the node *without* children
(`src/search/dfpn/core.rs:121`). `reconcile_children`
(`src/proof_tree/worker.rs:279–332`) then keeps whatever children happened to
exist under P. `finalize()` must choose one canonical node per
`(hash, outcome)` — P and Q tie on consistency and depth, and the tie-break
keeps the incumbent:

`build_expanded_index` (`src/proof_tree/worker.rs:349–378`):

```rust
let better = match self.expanded_by_hash.get(&(node.hash, outcome)) {
    None => true,
    Some(&other) => {
        // consistency first ...
        if consistent != other_consistent {
            consistent
        } else {
            node.depth < other_node.depth   // strict: equal depth keeps the FIRST
        }
    }
};
```

In dec46 the first-created twin P (5 of 6 required Loss replies) won; Q (all
6) lost. The dump then copied P's incomplete subtree. The reconstruction is a
strict superset because its walk expands all Loss replies by construction —
the fix makes the *live* tree equally complete.

**Relevant invariants:**

- `emit_proof_node` (`src/search/dfpn/mod.rs:442–447`) returns early for
  `Outcome::Draw` — **Draw events are never emitted**. A finalized proof tree
  from a decisive search contains only Win/Loss nodes.
- `reconcile_children` prunes a realized Win node to its single best Loss
  child and keeps every Win child of a realized Loss node. So at finalize
  time, child count is a *completeness proxy for Loss nodes* (more children =
  closer to covering all legal replies) and neutral for Win nodes (≤ 1).
- `finalize_tree` (`worker.rs:416–569`) rebuilds the tree from the canonical
  index, recomputes depths bottom-up (leaves forced to depth 0,
  `worker.rs:516–540`), hard-exits on any remaining unexpanded internal node
  (`worker.rs:544–555`), and rebuilds the child index.
- The cycle guard (`worker.rs:475–487`) turns a hash repeated on the current
  path into a leaf. Since Draw events are suppressed and a repetition on a
  proof path would be an uncached GHI draw (never a proof child), cycle-guard
  leaves *should not occur* in a valid decisive proof tree. The validator
  treats one as a defect (see Risks).
- `ProofTree::validate_ppv` (`src/proof_tree/node.rs:161–177`) checks only
  that a path exists in the tree — no board replay, no completeness. It is
  not a substitute for this validator.
- Trees loaded via `ProofTree::from_bin` carry `hash: 0` on every node
  (`src/proof_tree/binary.rs:182`); any hash-based check must skip zero
  hashes (same finding as plan5's signature DFS).
- `Position` (`src/position.rs`) provides `from_fen`, `do_move`/`undo_move`,
  `legal_moves_vec()`, `outcome_from_state(&state, &moves)` (the solver's
  exact terminal priority order), and `hash()` (full Zobrist incl. clock).
  `Position` depends only on `atomic_movegen` — it is a base layer, not part
  of `search`.

**Dependency-direction note.** `proof_tree` currently depends on
`proof_event` (plus `atomic_movegen` types) and "knows nothing about
`search`". The validator adds `proof_tree → position`, which does not violate
that rule (position is below search) but must be documented in `AGENTS.md`.

## Design

### 1. Twin preference in `build_expanded_index` (the fix)

New comparison priority for candidate vs. incumbent at the same
`(hash, outcome)`:

1. **Consistency** (`node.depth == implied_depth(id)`) — unchanged, first.
2. **Shallower proven depth** — unchanged, second.
3. **More children** — new tie-break. For Loss twins this is the
   completeness proxy (a Loss proof must cover *all* legal replies; the twin
   with more replies is closer to complete). For Win twins, whose child
   count is ≤ 1 after `reconcile_children`, it is neutral.
4. **First created** — final fallback, preserving the current deterministic
   behavior when everything else ties.

Implementation: compute each node's child count once while iterating (an
incremental counter is enough — `children(id).count()`), and replace the
final `node.depth < other_node.depth` arm with the two-step comparison
above. Equal child count ⇒ incumbent wins (strict `>` for the challenger),
so all existing finalize tests that tie on children keep passing.

**Documented limit** (module doc + code comment): child count cannot detect
a missing reply when *both* twins are incomplete, nor rank twins of different
positions' legality — it is a proxy. The validator (§2) is the authoritative
completeness check; the proxy only needs to be right in the measured failure
mode (one complete twin exists).

### 2. Replay-based validator: `src/proof_tree/validate.rs`

```rust
pub struct TreeDefect {
    pub path: Vec<Move>,     // root-relative move path of the defective node
    pub kind: DefectKind,    // see below
    pub detail: String,      // human-readable, e.g. missing reply "d2e3"
}

pub enum DefectKind {
    NotDecisive,        // root (or any node) is Draw / unrealized
    ChildNotLoss,       // child of a Win node is not Loss
    ChildNotWin,        // child of a Loss node is not Win
    WinNotSingle,       // non-terminal Win node has != 1 child
    LossIncomplete,     // a legal reply has no child under it
    IllegalChildMove,   // child move not legal in the replayed position
    TerminalMismatch,   // depth-0 outcome contradicts Position::outcome_from_state
    DepthInconsistent,  // depth != bottom-up Win=min+1 / Loss=max+1
    HashMismatch,       // replayed Zobrist hash != node.hash (skipped when node.hash == 0)
    CycleLeaf,          // non-terminal node truncated by finalize's cycle guard
}

pub fn validate_proof_tree(tree: &ProofTree) -> Result<(), Vec<TreeDefect>>
```

Mechanics: one iterative DFS from the root carrying a `Position` replayed
from `tree.root_fen` via `do_move`/`undo_move` along the path (so state,
legality, static outcome, and hash are exact); one `StateInfo` reused per
node. Rules checked per node:

- **Outcome domain**: every realized node is Win or Loss; the root is
  decisive. (Draw events are never emitted — `mod.rs:443` — so a Draw node
  in a finalized tree is a defect.)
- **Win, non-terminal** (`first_child.is_some()`): exactly one child, its
  move is legal, its outcome is Loss.
- **Loss, non-terminal**: the set of child moves equals the full
  `legal_moves_vec()` of the replayed position (each missing reply is one
  `LossIncomplete` defect with the move in `detail`); every child is Win.
- **Terminal** (`depth == 0`): `Position::outcome_from_state` on the replayed
  position must yield the node's outcome. A depth-0 node that is *not*
  statically terminal is a defect — this catches cycle-guard leaves and any
  "leaf-ified" interior.
- **Depths**: bottom-up `Win = min(child)+1`, `Loss = max(child)+1`,
  terminal = 0 (re-deriving what `finalize_tree` recomputes — an independent
  cross-check on the *final* tree, including copied canonical subtrees).
- **Hashes**: skip when `node.hash == 0` (loaded trees); otherwise replayed
  `pos.hash()` must equal `node.hash`.

Cost: O(nodes) with one movegen per Loss node — linear in tree size,
negligible post-search. Memory: one `Position` + DFS stacks. Deterministic.

Public API re-exported from `lib.rs` via the existing `proof_tree` module.

### 3. Wiring (report + non-zero exit; dump always written)

- **Worker** (`src/proof_tree/worker.rs`): `finalize_tree` runs
  `validate_proof_tree` on the rebuilt tree *after* the unexpanded-internal
  hard check, *before* installing it. On defects: print one stderr line per
  defect (cap the listing at, say, 20 lines, then a `... N more` line) in the
  form `pt_validate: FAILED path=<uci path> kind=<kind>: <detail>`; store the
  defect count in `ProofStats`.
- **`ProofStats`** gains `pub validation_errors: usize` (0 on success).
  `GetStats` already flows through the handle, so both CLIs read it for free.
- **`main.rs`** pre-exit hook: after `dump_to_bin` succeeds, print
  `pt_validate: ok` or `pt_validate: FAILED n defect(s)` (the per-defect
  lines already went to stderr from the worker) and exit with code 1 on
  failure. The dump is still written — it is the debugging artifact
  (decision recorded 2026-09-11).
- **`examples/reconstruct_pt.rs`**: after finalize, report
  `validate: ok|FAILED n` in both the default and `--json` outputs; exit
  non-zero on failure. The experiment mode records `validate` per case.
- **`examples/inspect_pt.rs`**: add `--validate` (off by default to keep the
  plain JSON dump cheap) that runs the validator on the loaded tree and
  prints defects. This is the item-#6 "inspect_pt runs against reconstructed
  trees" surface.

## Acceptance criteria

1. All new unit/integration tests pass in the fast tier.
2. Drift check: `benchmark --suite quick --json --first-outcome` per-case
   `child_evals` identical before/after (the fix touches only the worker's
   post-search finalization; the validator runs only at finalize).
3. `reconstruct_pt --experiment` on the decisive suite (release): **46/46
   oracle-isomorphic** (dec10, dec44, dec46 now match because the live tree
   grew to full completeness), zero anomalies, hole breakdown comparable to
   report5. This un-gates plan7.
4. Manual dec46 reproduction: solve dec46 with default `--dump-path`
   (without `--outcome-only`, stdin closed), `reconstruct_pt --oracle` →
   isomorphic; `inspect_pt --validate` on the *live* dump → `ok` (previously
   the live tree was incomplete).

## Scope

1. `src/proof_tree/worker.rs` — twin-preference tie-break in
   `build_expanded_index`; validator call + defect printing in
   `finalize_tree`; `ProofStats.validation_errors`. (File already carries a
   >20 KiB justification; the addition is ~30 lines.)
2. **New `src/proof_tree/validate.rs`** (+ `validate/tests.rs`): `TreeDefect`,
   `DefectKind`, `validate_proof_tree`, module docs stating the proof rules
   and the hash-0 / cycle-leaf policies.
3. `src/lib.rs` / `src/proof_tree/mod.rs` — re-export `validate_proof_tree`,
   `TreeDefect`, `DefectKind`.
4. `src/main.rs` — `pt_validate:` line + non-zero exit on defects.
5. `examples/reconstruct_pt.rs` — validate reporting in default/JSON/
   experiment outputs.
6. `examples/inspect_pt.rs` — `--validate` flag.
7. **Docs**: `AGENTS.md` — dependency-direction note (`proof_tree → position`
   for validation), the `pt_validate:` output line, `--validate` flag, and
   the `ProofStats` field.
8. **Initiative update**: backlog #6 → done; #3 renumbered plan7 (see Final
   task).

## Explicitly out of scope

- **The worker-off default flip** (now plan7, backlog #3): no CLI default,
  `--pt-size`, or `ExitReason::MemoryLimit` change here.
- **Search behavior**: no event-emission change (e.g. re-emitting children
  under the resolved-at path would fix the tree at the source but is a
  search-side change with its own drift validation — see Risks for when that
  becomes the fallback).
- **Either binary format** (`binary.rs` dump, TT snapshot v1).
- **PPV extraction quality** (`pv/` initiative): the fix may change which
  twin is canonical and therefore PPVs derived from dumps — that is the
  intended correctness improvement, not a compatibility constraint.
- Item #7 (builder capacity), item #4 (pinning), item #5 (checkpointing).

## Tests

Unit — `src/proof_tree/worker/tests.rs` (synthetic-hash events, same pattern
as the existing `finalize_copies_expanded_twin_to_unexpanded_sibling`):

- **Incomplete-then-complete twins**: Loss root, two same-hash Win twins;
  the first expanded with 1 of 2 children, the second with both; a later
  re-emission of the first (TT-hit pattern). Assert the finalized tree gives
  *both* root children 2 grandchildren (the complete subtree is canonical).
  This is the deterministic dec46 reproduction report5 lacked.
- **Complete-then-incomplete twins**: same but first-created is the complete
  one — assert it stays canonical (incumbent must not be displaced by an
  incomplete challenger).
- **Consistency still dominates**: the existing
  `finalize_prefers_shorter_consistent_twin` must keep passing unchanged
  (consistency outranks the new tie-break); add a stale-depth *incomplete*
  twin vs. consistent *complete* twin case asserting the consistent one wins
  even with fewer children.

Unit — `src/proof_tree/validate/tests.rs`:

- Valid tree: hand-built mate-in-1 tree (root Win → single Loss reply
  covering the king's legal moves → terminal Loss children) passes with zero
  defects.
- `LossIncomplete`: drop one reply child — defect reports the missing move
  and correct path.
- `WinNotSingle`, `ChildNotLoss`, `TerminalMismatch`, `DepthInconsistent`,
  `HashMismatch` each detected with the right `DefectKind`.
- Loaded-from-bin tree (`hash == 0` everywhere) validates without hash
  defects.
- Draw root and a mid-tree Draw node each yield `NotDecisive`.

Integration — `tests/test_plan6.rs` (fast tier):

- Solve the two-rook mate fixture with worker events, finalize, pull the
  tree via the handle, `validate_proof_tree` → ok; `stats().validation_errors == 0`.
- Reconstruct the same fixture via `src/reconstruct` (snapshot path as in
  `tests/reconstruct.rs`) and validate → ok.
- One `#[ignore = "slow: ..."]` test: dec02 end-to-end (solve → finalize →
  validate → reconstruct → validate), asserting both trees validate clean.

## Verification

1. `cargo fmt --check`, `cargo clippy --all-targets`, `cargo doc` — clean.
2. `make test` — fast tier green including the new tests.
3. Drift check (criterion 2 above).
4. Manual dec46 run (release) + `inspect_pt --validate` (criterion 4).
5. **Re-run `reconstruct_pt --experiment`** on the decisive suite (release,
   ~6 min) and record the table in `report6.md`: oracle results (expect
   46/46), hole breakdown, F/C distribution unchanged.
6. `verify_ppv` on the dec46 live dump's PPV before/after the fix (the PPV
   may change when the canonical twin switches — record it as the intended
   effect).

## Risks / notes

- **Cycle-leaf strictness.** The validator flags depth-0 non-statically-
  terminal nodes, which includes finalize's cycle-guard leaves. By the GHI
  argument these should never appear in a decisive proof; if the experiment
  surfaces one, that is a finding to explain (like plan5's `clock_miss`
  policy), not a rule to relax silently.
- **Both twins incomplete.** The child-count proxy cannot fix a case where
  no twin holds the complete expansion. If criterion 3 fails on some case,
  the fallback is a search-side lever (emit the complete expansion under the
  path of the TT-resolved node, or suppress the childless re-emission) —
  its own plan with drift validation; do not widen this one.
- **Validator false positives would be correctness bugs in the validator**,
  not in the trees: the rules mirror `reconcile_children` +
  `outcome_from_state` semantics exactly; any mismatch between the two is
  exactly what the validator exists to expose.
- **Exit-code change.** `pt_validate: FAILED` now exits 1 where it previously
  exited 0 with a silently incomplete dump. This is the decision recorded
  above; it only fires on a defect that today would ship an invalid proof.
- **`worker.rs` size** stays above 20 KiB with the same justification; the
  validator is deliberately a separate file.
- **`--experiment` runtime** (~6 min, release) is unchanged; it remains a
  manual release-run tool, rerun whenever the proof-tree layer changes
  (this plan changes it — step 5 is mandatory).

## Final task

Write `docs/plans/proof/report6.md` in this directory: summary of changes,
files changed, verification results (including the drift check, the dec46
manual run, and the full re-run experiment table with the 46/46 verdict),
problems encountered, deviations, missing tests, and next steps (plan7 —
the worker-off flip, backlog #3 — if criterion 3 holds). Update the
initiative backlog (#6 → done; #3 → plan7) and History in the same pass.
