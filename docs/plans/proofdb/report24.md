# report24 — plan24: proven-draw scheduling (step 1)

**Date:** 2026-10-26 · **Executes:** `plan24.md` (initiative item 21) ·
**Scope:** proofdb example tooling + docs; **no `src/` changes**, no
first-class draw facts.

## 1. Result

A `Draw` with `ExitReason::Complete` is no longer classified as a censor.
The harvest now exports no shard for it (the tree model still has no draw
outcome), commits one path-keyed `drawn` scheduling row, and prunes the path
and its whole subtree from the walk. Budget-exhausted and other draws keep
the old censored behavior.

The `make quick_draw` regression fixture is pinned:

```
7k/5p1P/5P2/2p5/1NPB4/8/8/K7 w - - 0 1
outcome = Draw, exit_reason = Complete, 263 child-evals
classify(..) = JobVerdict::ProvenDraw
```

## 2. What shipped (by plan task)

| Area | Change |
| --- | --- |
| Classification (`harvest.rs`) | `classify(Outcome, ExitReason) -> JobVerdict`; `Decisive(Outcome)` / `ProvenDraw` / `Censored`; `Draw + Complete` is the only `ProvenDraw`. |
| Session (`session.rs`) | A proven draw returns a `JobRecord` with `outcome: "draw"`, `exit_reason: "Complete"`, no tag/shard nodes; censor record unchanged for other draws. |
| Schema (`schema.rs`) | Additive schema-v2 `drawn (path TEXT PRIMARY KEY)`; created by fresh fold `write_db`, by `ensure_fold_schema` on an existing v2 DB, and on demand by `insert_drawn_row`; `read_drawn_rows` treats a missing table as empty. |
| Ledger (`ledger.rs`) | `Ledger` carries a `BTreeSet` of drawn paths; `load_db` reloads it; `is_drawn` / `drawn_iter` / `mark_drawn`; `on_proven_draw` commits the row and marks memory without a censor bump. |
| Walk (`walk.rs`, `walk/driver.rs`, `walk/keys.rs`, `walk/census.rs`) | Drawn paths/subtrees drop from candidacy, are handled during expansion, and block the virtual-root rule; `mark_drawn` sweeps in-session; scoped drawn root prints a note; census `drawn N`; summary `drawn R`. |
| Docs | Spec §2/§2.1/§3/invariant 8; manual artifact graph, scheduling pick-up, walk rules, output grammar, troubleshooting/writer split; AGENTS.md harvest paragraph; initiative item 21/history and the step-1 decision note. |

The durable new artifact is only `drawn`; `nodes`, `shards`, `meta`, and the
tree invariants are untouched. A fold preserves `drawn` rows in place.

## 3. Tests and gate

Added/updated coverage:

- `examples/proofdb/harvest/tests.rs`
  - `quick_draw_is_a_complete_proof_not_a_censor` (the real FEN above).
  - `draw_classification_distinguishes_proof_from_censor` (Complete vs
    BudgetExhausted vs Timeout; decisive unaffected).
- `examples/proofdb/ledger/tests.rs`
  - `proven_draw_is_durable_and_not_a_censor` (reload, no censor row,
    missing-table path, on-demand table creation).
- `examples/proofdb/walk/tests.rs`
  - drawn path/subtree drop, handled rule, drawn root blocked, in-session
    sweep, draw not counted as an exhaustion `complete`.
- `tests/proofdb.rs`
  - the on-path repetition case now asserts a zero-eval `Complete` draw is
    proven and pruned (`summary.drawn = 1`, `censored = 3`,
    `completes = 0`), with the record not bumped.
- `tests/proofdb_fold.rs`
  - `drawn` rows survive a tree-changing fold.
- Grammar pins in `tests/proofdb_harvest_cli.rs` and the walk census tests
  updated for `drawn N` / `drawn R`.

Gate:

```text
cargo fmt --all                      clean
cargo clippy --all-targets           clean
cargo doc --no-deps                  clean
cargo test --test proofdb            71 passed, 0 failed
cargo test --test proofdb_fold       62 passed, 0 failed
cargo test --test proofdb_harvest_cli 7 passed, 0 failed
make test                            exit 0 (36 green test-result lines)
```

`make test-full` was not run: the change touches proofdb example tooling and
docs, not search/TT/proof-tree code. The default fast gate is the required
gate under the project testing conventions.

## 4. Deviations and findings

1. **Zero-eval repetition draws are now proven draws.** The old
   `walk_zero_eval_censor_guard_completes_the_node` fixture
   (`g1f3 g8f6 f3g1 f6g8`) returned `Draw` with 0 evals and
   `ExitReason::Complete`. Under the new classification it is a proven
   draw at that exact path, so the test was renamed and updated to
   `walk_zero_eval_complete_draw_is_proven_and_pruned`. The zero-eval
   *censor* guard in the driver remains for non-complete zero-eval draws,
   but this fixture no longer exercises it. This is a classification
   consequence, not a soundness change: the path key fixes the repetition
   history.
2. **Census `drawn` is a record-drop count, not a count of `drawn` rows.**
   `on_proven_draw` does not create a censor record, so a drawn-only path
   contributes 0 to `records` and to the `drawn` census drop count. The
   authoritative count is `SELECT COUNT(*) FROM drawn`. This is noted as a
   plan limitation; no grammar change was attempted in step 1.
3. **Schema remains v2.** The `drawn` table is additive scheduling state
   (like `censor`), not a proof-surface change; no `schema_version` bump.
   An existing v2 DB without the table still loads (empty drawn set), and
   the first proven draw / fold creates it. `drawn` is not a foreign key
   into `nodes`; most drawn paths have no row.
4. **Format changes are intentional:** job `outcome` gains `"draw"`, the
   summary gains `drawn R`, and the census gains `drawn N`. The manual's
   output tables and examples were updated; CLI tests pin the new wording.
5. **No measurement artifacts.** There is no performance claim and no new
   measurement directory; the quick_draw fixture is a deterministic unit
   regression (263 evals on the reference build).

## 5. Unresolved parts / missing tests

- **No driver-level quick_draw end-to-end test.** `Session` jobs replay
  from the startpos, so the quick_draw FEN cannot be reached with an empty
  job path. The proof is covered at the classification boundary and through
  the ledger/frontier/fold tests; a future test could use a real startpos
  path or a test-only session root.
- **No CLI test for the `drawn R` summary count.** The summary format is
  updated in the manual but only the census grammar is pinned by CLI tests.
- **No explicit old-v2-table migration test.** `ensure_fold_schema` creates
  a missing `drawn` table, and `insert_drawn_row` creates it on demand, but
  neither path is separately regression-tested against a hand-built
  pre-step-1 v2 file (the fresh fold and missing-table load paths are).
- **No first-class draw representation.** A consumer still sees a proven
  draw as `open`; only the harvest scheduler knows better.
- **`Complete` remains the proof signal.** The assumption that the harvest
  search is unbounded in depth is documented in the plan; it is not
  enforced by a type or a separate proof-status flag.

## 6. Next steps

1. **Step 2 (separate plan, schema v3):** decide whether proven draws become
   first-class tree facts: `outcome='draw'`, the draw proof rule (all
   children resolved to win/draw plus at least one draw witness, or a
   rule-terminal draw), draw proof events, worker/finalize/validator/merge
   support, repetition path-dependence handling (path-scoped proofs, no
   transposition copying), spec/consumer update, and `proofdb_verify`/flip
   audit. This is larger than step 1 and should start with its own plan
   session.
2. **Small hardening follow-ups:** a CLI summary pin for `drawn R`, an
   old-v2 migration test for the `drawn` table, and a driver-level draw
   fixture if a convenient startpos path or test hook is added.
3. **Operator note:** when reducing scheduling state, `DELETE FROM drawn`
   forces re-exploration of proven-drawn subtrees; deleting `censor` alone
   leaves them pruned (see the manual §4).

## Tools used

`cargo fmt`, `cargo clippy --all-targets`, `cargo doc --no-deps`,
`cargo test --test proofdb`, `cargo test --test proofdb_fold`,
`cargo test --test proofdb_harvest_cli`, `make test`. No scripts or
measurement artifacts were added.
