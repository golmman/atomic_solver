# Implementation Report: Proof-Tree Finalization Completeness — Twin Preference + Loss-Completeness Validator (plan6)

Implements `docs/plans/proof/plan6.md` (backlog item #6). Zero search-path
changes (drift check bit-identical); the fix lives entirely in the
proof-tree worker's post-search finalization, plus the new replay-based
validator and its wiring.

## Summary of changes

- **Twin-preference fix** (`src/proof_tree/worker.rs`,
  `build_expanded_index`): the canonical `(hash, outcome)` selection now
  prefers, in order: consistency, shallower proven depth, **more children**
  (new tie-break — a completeness proxy for Loss twins), then first created.
  Child counts are computed from the sibling chain.
- **Replay-based validator** (`src/proof_tree/validate.rs` +
  `validate/tests.rs`): `TreeDefect`, `DefectKind`,
  `validate_proof_tree(&ProofTree) -> Result<(), Vec<TreeDefect>>`. One
  iterative two-phase (Enter/Exit) DFS replays every path on a real
  `Position` (one movegen per node, one pooled `StateInfo`/`MoveList` pair)
  and checks: outcome domain (Win/Loss only, decisive root), Win ⇒ exactly
  one legal Loss child, Loss ⇒ *all* legal replies present and all children
  Win, terminals statically correct via `outcome_from_state`, bottom-up
  depths, Zobrist hashes (skipped when `node.hash == 0`, i.e. dump-loaded
  trees), and path-hash cycles (mirrors finalize's cycle guard). Public API
  re-exported from `proof_tree`.
- **Worker wiring**: `finalize_tree` runs the validator on the rebuilt tree
  after the unexpanded-internal hard check, before installing it; defects are
  printed to stderr as `pt_validate: FAILED path=<uci> kind=<kind>: <detail>`
  (capped at 20 lines, then `... N more`) and counted in the new
  `ProofStats::validation_errors` (0 on success). The tree is installed and
  dumped regardless — the dump is the debugging artifact.
- **`src/main.rs`**: the pre-exit hook prints `pt_validate: ok` or
  `pt_validate: FAILED n defect(s)` after the dump and exits with code 1 on
  defects (decision recorded in the plan; previously a defective dump exited
  0 silently).
- **`examples/reconstruct_pt.rs`**: `validate: ok|FAILED n` reported in the
  default and `--json` outputs of the single-snapshot mode (exit non-zero on
  failure), and a `validate` column per case in `--experiment` plus a
  `validate_failures` summary counter that exits non-zero.
- **`examples/inspect_pt.rs`**: `--validate` flag (off by default) runs the
  validator on the loaded tree, prints defects, exits non-zero on failure.
- **`tests/test_proof_validate.rs`**: fast-tier integration tests (live
  two-rook mate validates clean via both build paths, `validation_errors == 0`)
  and a `#[ignore]`d dec02 end-to-end test.
- **Docs**: `AGENTS.md` — proof-tree bullet (validator, twin preference,
  `validation_errors`, dependency note), CLI `pt_validate:` output,
  `inspect_pt --validate`, `reconstruct_pt` validate reporting.

## Files changed

| File | Change |
|---|---|
| `src/proof_tree/worker.rs` | twin-preference tie-break (sibling-chain child counts), validator call + defect printing in `finalize_tree`, `ProofStats::validation_errors` |
| `src/proof_tree/validate.rs` | new — replay-based validator (+ header size justification; 16 KB) |
| `src/proof_tree/validate/tests.rs` | new — 12 unit tests |
| `src/proof_tree/mod.rs` | `pub mod validate` + re-exports, dependency-direction doc |
| `src/main.rs` | `pt_validate:` line + exit 1 on defects |
| `examples/reconstruct_pt.rs` | validate reporting (single/JSON/experiment) |
| `examples/inspect_pt.rs` | `--validate` flag |
| `tests/test_proof_validate.rs` | new — 2 fast + 1 slow integration tests |
| `src/proof_tree/worker/tests.rs` | 3 twin-preference tests; `validation_errors == 0` assertion in the live-solve test |
| `AGENTS.md` | architecture/examples updates |
| `docs/plans/proof/initiative.md` | backlog #6 done, History updated |

## Verification results

1. `cargo fmt --check`, `cargo clippy --all-targets`, `cargo doc` — clean
   (zero warnings).
2. `make test` (fast tier, release) — green, including the 3 new
   twin-preference unit tests, 12 validator unit tests, and 2 new fast
   integration tests. Full tier (`cargo test --release -- --include-ignored`)
   also green: 346 passed / 0 failed (one transient stress-suite failure
   under parallel load was not reproducible on re-run; see Problems).
3. **Drift check** — `benchmark --suite quick --json --first-outcome --runs 1`
   before vs. after: 59 cases, `status`/`outcome`/`nodes`/`child_evals`/
   `pv_len`/`timeout`/`wrong` **byte-identical per case**; only wall-clock
   `total_time` differs. Confirms zero search-path change.
4. **dec46 manual reproduction** (criterion 4), release build:
   - Solve with default `--dump-path` (stdin closed, no `--outcome-only`):
     `pt_validate: ok`, exit 0. Nodes 3001 → **3017** (the live tree grew to
     full completeness; win/loss 1500/1501 → 1508/1509).
   - `reconstruct_pt --snapshot … --oracle <live dump>` → `oracle:
     isomorphic`, `validate: ok`, holes `hit=2191 terminal=826 absent=0
     filled=0 anomalies=0`.
   - `inspect_pt --validate` on the live dump → `pt_validate: ok` (exit 0).
5. **Re-run `reconstruct_pt --experiment`** (decisive suite, release):
   **46/46 oracle-isomorphic**, `validate` = ok on all 46, zero coverage
   failures, zero validate failures, verdict **GO**. Hole breakdown
   unchanged from report5: hit=69,598 (76.1%), terminal=21,869 (23.9%),
   clock_hit=1, absent=2, filled=2, unfillable=0, anomalies=0;
   total C=65,138,783, total F=2, median F/C=0.0. Per-case C values are
   identical to report5 (search untouched). dec10 (50,544 nodes), dec44
   (4,305), and dec46 (3,017) — report5's three mismatches — now match.
   This un-gates plan7 (worker-off default flip, backlog #3).
6. **verify_ppv before/after on the dec46 live dump**: the PPV *string* is
   unchanged (`f1h1 g2g1q c1c2 …`); pre-fix the root already had
   f1h1@13 tied with c5c6@13, and the incompleteness lived in the deeper
   transposition twins, which the fix repairs. `verify_ppv` still reports
   `is_ppv: false` ("f1h1 is not a longest defense, depth 11, longest 13") —
   note that `verify_ppv` derives defense lengths from its **own re-search**,
   not from the proof tree, so this verdict is independent of the tree fix
   and reflects the informational PV's length semantics only.

### The extra bug the fix exposed (worker hygiene)

The first implementation computed twin child counts from **parent links**.
dec46 still failed: `reconcile_children` removes pruned children from the
sibling chain but leaves their `parent` link set, so pruned dummy children
inflated the parent-link count (the incomplete twin 1520 counted 6 children
while its chain held 5, tying the complete twin 2647 and keeping the
incumbent). Switching to sibling-chain counting (as the plan itself
suggested) fixed it; a comment documents the invariant. No other consumer
of parent links is affected (the finalize rebuild only walks live chains).

## Deviations from the plan

- **Integration test file named `tests/test_proof_validate.rs`**, not
  `tests/test_plan6.rs`: that name is taken by an unrelated pre-existing
  m2x regression suite; the header documents this.
- **Unparseable root FEN yields a single `NotDecisive` defect.** The plan's
  enum has no kind for this; returning one `NotDecisive`
  ("root FEN failed to parse: …") keeps the `Result` contract without adding
  a variant. This also keeps worker unit tests with synthetic `"fen"` roots
  well-defined (one defect line instead of undefined behavior).
- **Terminal vs. child checks dispatch on `node.depth == 0` / `> 0`.** The
  plan's rules quote both `depth == 0` (terminal check) and
  `first_child.is_some()` (non-terminal child checks); in well-formed trees
  these coincide. The validator keys the terminal check on `depth == 0`
  (per the plan's "leaf-ified interior" note), does not descend into
  depth-0 nodes, and gates the depth assertion on the subtree having been
  fully replayable (avoiding cascading DepthInconsistent noise behind an
  unreplayable defect).
- **`CycleLeaf` is detected by a replayed path-hash repeat** (mirroring
  finalize's guard). Since full Zobrist keys cannot repeat along a real
  replay (the clock changes every ply), cycle-guard leaves in practice
  manifest as depth-0 non-statically-terminal nodes and are reported as
  `TerminalMismatch` — exactly the plan's Risks-section expectation. No
  cycle leaves surfaced on the decisive suite.
- **Experiment additions**: per-case `validate` column and a fatal
  `validate_failures` counter (exit 1) in `--experiment`, and a
  `validate:` line/JSON field with exit 1 in the single-snapshot mode —
  "exit non-zero on failure" applied consistently to all modes.
- **`inspect_pt --validate` exits non-zero** on defects (plan said
  "prints defects"); useful for scripting, consistent with the other tools.

## Problems encountered

- **Parent-link vs. sibling-chain child counts** (above) — the interesting
  bug; caught by the dec46 manual reproduction, not by the synthetic unit
  tests (they never prune under a twin).
- **`MoveList` reuse**: `generate_legal_with_state` appends without
  clearing; the pooled list must be cleared per node or the Loss-completeness
  check sees stale moves (caught immediately by the two-rook proof test).
- **DFS frame ordering**: pushing the parent's Exit frame after the children
  pops it first (LIFO), corrupting the replay state; Exit must be pushed
  before the children.
- **Flaky stress test under parallel load**: one transient failure in
  `m19_white_unproven_in_60s`-class wall-clock tests during a full
  `--include-ignored` run executed concurrently with other builds; not
  reproducible on a clean re-run (346/346). No action taken.
- **Background job hygiene**: long-running verification commands must run
  synchronously in this environment (a nohup'd experiment run was lost).

## Missing tests

- No test drives the >20-defect stderr cap (`... N more` line) — cosmetic.
- No unit test for `DefectKind::CycleLeaf` triggering (it requires a tree
  with an impossible full-hash path repeat; the kind is defensive).
- The dec02 slow integration test covers depth but not the dec10-scale
  (50k-node) tree; the experiment covers that scale manually.
- `tests/test_proof_validate.rs` asserts `validation_errors == 0` via
  `stats()`; the worker's defect *printing* itself (stderr formatting) is
  only exercised manually.

## Next steps

1. **plan7 — flip the default (worker off during search), backlog #3.**
   Criterion 3 holds (46/46, GO), so plan7 is un-gated. With the validator
   in place, the reconstructed tree can be validated at build time, which
   was the precondition for making it the *only* tree.
2. Keep re-running `reconstruct_pt --experiment` whenever the proof-tree
   layer changes (mandatory per plan5/plan6 convention).
3. Item #7 (builder capacity) unchanged — design spike only when deep
   proofs actually overflow the builder.
