# plan24 — Proven-draw scheduling (step 1): a `Complete` Draw is not a censor

**Initiative:** `proofdb` · **Executes:** backlog item 21 (added 2026-10-26;
step 1 of the proven-draw handling) · **Size:** S (proofdb example tooling
only; no product-solver or proof-tree changes) · **Date drafted:** 2026-10-26
(backfilled after the implementation; the tasks below describe the shipped
change and remain self-contained for a separate session).

> **Item 21's text:** "Classify `Draw` + `ExitReason::Complete` as a proven
> draw; persist one path-keyed `drawn` row (schema v2 additive scheduling
> state, no tree outcome/shard); the walk drops that path and its subtree
> from candidacy, treats it as handled during expansion, and blocks a drawn
> scope root; budget-exhausted/other draws remain censors; job/summary/census
> grammar gains `draw`/`drawn`; spec/manual/AGENTS updated."

## 0. Why this plan

The harvest currently classifies every `Outcome::Draw` as **censored**
(`classify` returns `None` for Draw). That rule was written when a Draw was
assumed to mean "no decisive result within the budget". It is wrong for a
completed solver proof: the `make quick_draw` position

```
7k/5p1P/5P2/2p5/1NPB4/8/8/K7 w - - 0 1
```

solves in 263 child-evals with `exit_reason: Complete`. The harvest throws
that proof away, labels the visit `censored`, and records only a fact of
absence. Re-searching the same path is at best wasted work and, if the TT
does not serve the retry at zero evals, can burn the flat budget before the
walk expands into the drawn subtree.

**Goal of this step:** keep the tree model unchanged (schema v2 still has
no `draw` outcome and no draw shards), but stop conflating a proven draw with
an unknown one. Persist the proof as path-keyed scheduling state and prune
its subtree.

## 1. Decisions (fixed before implementation)

1. **D1 — the proof signal.** `Outcome::Draw` with
   `ExitReason::Complete` is a proven draw. Every other Draw (budget
   exhausted, timeout, quit) stays censored. Win/Loss remain decisive and
   unchanged. The harvest searches with an unbounded `max_depth`
   (`search_depth_with_prefix(..., u32::MAX, ...)`), so `Complete` is
   natural exhaustion, not a depth cutoff.
2. **D2 — persistence is scheduling state, not a tree fact.** Add one
   schema-v2 additive table:
   `drawn (path TEXT PRIMARY KEY)`. No `nodes` row, no `shards` row, no
   `outcome='draw'`, no change to `proofdb_verify`'s tree invariants. The
   table is path-keyed like `censor` and is ignorable by consumers.
3. **D3 — the walk prunes the proof.** A proven draw marks the path and its
   whole subtree out of candidacy (no `Loss` child can exist under a drawn
   node, so no subtree work can be on the startpos proof). A drawn path is
   *handled* during expansion, and a drawn scope root stops the session
   `exhausted` with a scoped stderr note. A draw is not counted as a
   `complete` (complete means exhaustion, not proof).
4. **D4 — writer split.** The harvest remains the only writer of
   `censor`/`drawn`; `proofdb_merge` remains the sole writer of
   `nodes`/`shards`/`meta` and leaves both scheduling tables untouched. A
   fold creates the `drawn` table if an older v2 DB lacks it and preserves
   its rows. The pre-step-1 database path must keep working: a missing
   `drawn` table reads as empty, and the first proven draw creates it.
5. **D5 — output grammar.** A proven-draw job emits
   `"outcome":"draw"` with `"exit_reason":"Complete"`, no tag/shard nodes.
   The summary line gains `drawn R`; the census line gains `drawn N`.
   Budget-exhausted draws keep `"censored"` and the censor bump.
6. **D6 — no first-class draw facts.** Do not add a `draw` outcome to
   `nodes`, do not emit draw proof events, do not teach the proof-tree
   validator/merge about draws. That is a separate schema-v3 decision
   (step 2), because a game-theoretic draw proof is an AND-of-non-loss
   children plus an OR draw witness and repetition draws are path-dependent.
7. **D7 — no product changes.** `src/` is untouched. This is proofdb example
   tooling plus the spec/manual/AGENTS documentation.

## 2. Implementation tasks

1. **Classification** — `examples/proofdb/harvest.rs`
   - Replace `classify(Outcome) -> Option<Outcome>` with
     `classify(Outcome, ExitReason) -> JobVerdict`.
   - `JobVerdict::{Decisive(Outcome), ProvenDraw, Censored}`.
   - `Draw + Complete -> ProvenDraw`; `Draw` otherwise -> `Censored`.

2. **Session records** — `examples/proofdb/session.rs`
   - Read the exit reason once; a `ProvenDraw` returns a `JobRecord` with
     `outcome: "draw"`, no tag/shard; `Censored` keeps the existing shape.
   - Decisive outcomes still go through `produce_shard`.

3. **Schema** — `examples/proofdb/schema.rs`
   - `DRAWN_DDL = CREATE TABLE IF NOT EXISTS drawn (path TEXT PRIMARY KEY)`.
   - Fresh fold schema and `write_db` create it.
   - `ensure_fold_schema` on an existing v2 DB creates it in place
     (idempotent; additive, no version bump, no proof surface).
   - `read_drawn_rows` (missing table -> empty) and `insert_drawn_row`
     (create-if-needed, `INSERT OR REPLACE`) helpers.

4. **Ledger / durable hook** — `examples/proofdb/ledger.rs`
   - `Ledger` carries a `BTreeSet<String>` drawn set alongside the censor
     map; `load_db` reads it; `is_drawn`, `drawn_iter`, `mark_drawn`.
   - `on_proven_draw(ledger, conn, key)` commits the `drawn` row, then marks
     memory. No censor row is written; no evals are charged to
     `work_done`.

5. **Walk selection** — `examples/proofdb/walk.rs`,
   `walk/driver.rs`, `walk/keys.rs`, `walk/census.rs`
   - `WalkFrontier` carries the scope-relative drawn set; `new` drops
     records under it; `admit` refuses them.
   - `mark_drawn` sweeps the path and subtree from candidacy.
   - `is_handled` returns true for a drawn path.
   - `root_drawn`/`root_blocked` stop the virtual-root rule; the driver
     prints the scoped "proven draw — nothing to harvest" note.
   - Census adds `drawn N`; `WalkSummary`/summary adds `drawn R`.

6. **Tests**
   - `examples/proofdb/harvest/tests.rs`: the real `make quick_draw` FEN
     solves `Draw` + `Complete` and classifies `ProvenDraw`; budget/timeout
     draws classify `Censored`.
   - `examples/proofdb/ledger/tests.rs`: `on_proven_draw` durability, no
     censor row, reload, missing-table path, on-demand table creation.
   - `examples/proofdb/walk/tests.rs`: drawn path/subtree drop, handled
     rule, drawn root blocked, in-session sweep.
   - `tests/proofdb.rs`: the on-path-repetition test now exercises a
     zero-eval `Complete` draw (drawn + pruned, not censored).
   - `tests/proofdb_fold.rs`: `drawn` rows survive a tree-changing fold.
   - Update the census/summary grammar pins in
     `tests/proofdb_harvest_cli.rs` and the walk census tests.

7. **Documentation**
   - `docs/spec/global_proof_store.md`: `drawn` DDL in §2, scheduling-state
     wording in §2.1, invariant 8 covers both scheduling tables, §3 states
     that proven draws remain non-tree facts.
   - `docs/proofdb_pipeline.md`: artifact/daemon description, scheduling
     pick-up, walk search/expansion/complete rules, output table and summary
     grammar, scheduling-state caveat, troubleshooting writer split.
   - `AGENTS.md`: the proofdb harvest paragraph.
   - This plan, `report24.md`, and the initiative item/history entries.

## 3. Acceptance criteria

- `make quick_draw` in a harvest prints one proven-draw job
  (`"outcome":"draw"`, `"exit_reason":"Complete"`), writes one `drawn` row,
  writes no censor row for that path, and exports no shard.
- A budget-exhausted Draw still censors, bumps `work_done`, and keeps the
  old expansion behavior.
- A later session reloads the `drawn` rows and does not re-search or expand
  that path/subtree; a drawn scope root exhausts with a note.
- `SELECT path FROM drawn` is the only durable new artifact; `nodes`,
  `shards`, `meta`, and all §4 tree invariants are unchanged.
- The spec remains standalone (no plan vocabulary).
- Gate green: `make test`, `cargo fmt`, `cargo clippy --all-targets`,
  `cargo doc`.

## 4. Test plan

Run, at minimum:

```bash
cargo fmt --all
cargo clippy --all-targets
cargo test --test proofdb
cargo test --test proofdb_fold
cargo test --test proofdb_harvest_cli
make test
cargo doc --no-deps
```

The targeted targets cover the classification, ledger, walk, fold, and CLI
grammar changes; `make test` is the project gate. This change does not touch
the search/TT/proof-tree crates, so the slow tier (`make test-full`) is not
required by the testing conventions.

## 5. Risks and limitations

- **`Complete` is a solver contract.** If a future harvest entry point uses
  a finite `max_depth` or a refinement cap, `Complete` could mean "bounded
  tree exhausted", not "game-theoretically proven". This plan relies on the
  harvest's existing unbounded `search_depth_with_prefix` call. Any future
  draw proof must keep that invariant or add an explicit proof-status flag.
- **Repetition draws are path-dependent.** The `drawn` row is keyed by the
  exact startpos path, so the scheduling mark is valid for that path. It is
  *not* a transposition-independent fact and must not be copied across
  paths. This is exactly why the tree model is not changed in this step.
- **No draw fact is exposed to consumers.** A website still sees `open`;
  the `drawn` table is internal scheduling state.
- **Census `drawn` counts censor records dropped under a drawn path**, not
  drawn-only paths (a proven draw does not create a censor record). SQL
  inspection is the authoritative count of `drawn` rows.

## 6. Out of scope

- Schema v3 / `outcome='draw'` tree rows, draw proof events, validator,
  merge, and consumer contract changes (step 2).
- Any attacker-centric "draw = loss" reinterpretation.
- Changing the product solver's `Outcome` or proof-event behavior.
- Harvest policy, budget, or frontier-selection changes beyond the drawn
  drop/handled/root rules.
- A live full-harvest batch and DB growth.

## 7. Final task

Write `report24.md`: what shipped, the exact gate result, deviations and
findings (including the zero-eval repetition classification change), missing
tests, and the step-2 next steps. Update `initiative.md`'s item/history
entries.
