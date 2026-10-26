# Plan 6: Remove the rejected cross-racer facts carrier

## Start

Read, in this order:

- `AGENTS.md` (conventions, test tiers, closure protocol).
- `docs/plans/parallel/report10.md` and `report11.md` (what the facts channel
  was and why it was rejected).
- `docs/plans/parallel/initiative.md` (the closure verdict: "CLOSED for good
  (second strike)").
- This plan, plus the files listed under "Removal inventory".

Then execute the plan in one sitting. Do not re-open the `parallel`
mechanism question: this plan only deletes the code of an already-rejected
mechanism.

## Goal

Delete the cross-racer solved-fact channel (`src/facts/`, its `search::dfpn`
hooks, its CLI flags, its tests, and its documentation surface) without
changing any search result. The mechanism was measured and rejected twice
(plan10: plumbing tax; plan11: TT pollution of winning basins), the
`parallel` initiative is closed, and the owner has decided (2026-10-26) that
the plan10/plan11 measurement scripts do **not** need to remain verbatim
runnable. The plans, reports, and parsed measurement data remain in-tree as
the historical record.

This is a `cleanup` exercise: YAGNI + Boy-Scout. It must be
behaviour-free.

## Background

What is being removed, precisely:

- `src/facts/` — a 24-byte checksummed record codec, a synchronous
  `FactsExporter`, a threadless `FactsImporter`, unit tests.
- `search::dfpn::FactImporter` / `FactExporter` traits,
  `FactImportCounters`, three `Search` fields, their setters/getters, the
  `drain_fact_imports()` call at work-chunk boundaries, and the exporter
  branch in `emit_proof_node`.
- `--facts-out` / `--facts-in` CLI options, their help text, wiring, and
  `facts_import:` / `facts_export:` stderr summaries.
- The corresponding `AGENTS.md` architecture bullet and flag listing.

Why it is safe to delete:

- With neither flag passed (the only production path), every facts code path
  is a `None` check: no TT interaction, no I/O, no state change. The module
  docs themselves assert bit-identity with the channel off.
- The mechanism has no in-tree consumer: `examples/portfolio` races with
  `--salt` only; no product code or example constructs a `FactsExporter` /
  `FactsImporter`.
- The only non-test consumer of the test-only `test_resolved_from_entry`
  re-export is `src/facts/tests.rs`; it goes away with the tests.

Why now (and not keep it as an executable record):

- It is ~1,020 lines / ~38.7 KB of source, plus ~110 reference sites across
  `search::dfpn`, `cli.rs`, `main.rs`, `lib.rs`, and unit tests.
- Every core API change (`TranspositionTable::store`, `Outcome`, TT entry
  layout, Zobrist keys) obliges updates to `src/facts` and its tests for a
  rejected feature.
- The public traits and flags present a non-feature as shipped surface.
- The reopen condition is a *new mechanism hypothesis*, not this exact
  rejected carrier; git history plus the reports are sufficient if that
  ever happens.

## Non-goals

- Do not touch `docs/plans/parallel/measurements/**`, `plan10.md`,
  `plan11.md`, `report10.md`, or `report11.md`. Those are history.
- Do not touch the `ProofEvent` / proof-tree pipeline, the portfolio driver,
  or the salt mechanism.
- Do not refactor unrelated code, rename unrelated tests, or change file-size
  justifications.
- Do not update the `cleanup` row of `docs/plans/README.md`: the initiative
  is not opening/pivoting/closing (see `AGENTS.md`); update
  `docs/plans/cleanup/initiative.md` instead.

## Removal inventory

Line numbers are approximate; use the symbol names as the authority.

### 1. Delete the module

- Delete `src/facts/mod.rs` and `src/facts/tests.rs` (remove the directory).
- `src/lib.rs`: remove `pub mod facts;`.

### 2. `src/search/dfpn/mod.rs`

Remove:

- `FactImportCounters` (the `accepted`/`skipped` struct and its docs).
- `FactImporter` trait and `FactExporter` trait (with their long
  plan10/plan11 doc comments).
- `Search` fields `fact_importer`, `fact_exporter`,
  `fact_import_totals` and their doc comments; their initializers in
  `Search::new`.
- `Search::set_fact_importer`, `Search::set_fact_exporter`,
  `Search::fact_import_totals`, `Search::drain_fact_imports`.
- The `#[cfg(test)] pub(crate) fn test_resolved_from_entry` re-export.
- The `self.drain_fact_imports();` call at the top of the `bounded_search`
  chunk loop.
- The `fact_exporter` emit block in `emit_proof_node` (keep the early
  `Outcome::Draw` return and the `proof_event_sender` block exactly as they
  are).
- The `self.fact_import_totals = FactImportCounters::default();` reset in
  `begin_run`.
- Any now-unused imports (`Outcome` and `Arc` remain needed elsewhere;
  check with the compiler).

Do **not** change the chunk loop's other work (time/budget checks, fact-free
`reset_search_state`, chunk growth), and do not change
`emit_proof_node`'s signature.

### 3. `src/search/dfpn/tests.rs`

- Delete `fact_import_drain_tallies_and_is_inert_without_importer` (the
  `CountingImporter` test).
- Delete `imported_root_fact_resolves_the_root_from_the_tt` (the
  `RootFactImporter` test).
- Keep `evaluate_child_resolves_imported_shaped_solved_entry`, but:
  - rename it to `evaluate_child_resolves_solved_entry_without_move_hint`,
  - rewrite its comment: it validates that a solved TT entry with
    `Move::NONE`, `work = 0`, `remaining_depth = u32::MAX` is resolved by
    `evaluate_child` with its proven depth preserved. That is a TT reuse
    invariant independent of the deleted importer, so the test stays (no
    plan10/facts framing).
- Remove imports that become unused in that test module.

### 4. `src/cli.rs`

Remove:

- `CliOptions` fields `facts_out` and `facts_in` and their doc comments.
- The `facts_out: None, facts_in: None` entries in `Default`.
- The `--facts-out` and `--facts-in` parse arms.
- The `facts_out: _, facts_in: _` fields in the `defaults_match_documentation`
  destructuring.
- Tests `facts_options_are_parsed` and `facts_options_require_values`.
- The two `--facts-*` assertions in `missing_value_returns_err`.

After removal, the flags are rejected by the catch-all as
`error: unknown option '--facts-out'`. That is the intended operator
surface; do not add a special "removed flag" message and do not add
replacement tests for removed options.

### 5. `src/main.rs`

Remove:

- The `--facts-out` / `--facts-in` entries in the module-level header
  contract (including the "Restatements with the facts options on" paragraph).
- `use atomic_solver::facts;`.
- The two help-text blocks (`--facts-out`, `--facts-in`).
- `facts_out`/`facts_in` from the `CliOptions` destructuring.
- The whole facts wiring block: `facts_export`, `facts_import_stats`,
  `FactsExportShared::create`, `search.set_fact_exporter`,
  `search.set_fact_importer`.
- The summary block that reads `search.fact_import_totals()`, calls
  `facts_import_stats`/`export.finish()`, and prints `facts_import:` /
  `facts_export:`.
- Any imports/variables left unused by the deletion.

Keep the search construction, pre-exit hook, and all other summary output
untouched.

### 6. Documentation

- `AGENTS.md`:
  - remove `facts` from the `src/lib.rs` re-export list,
  - delete the `src/facts/` architecture bullet,
  - remove `--facts-out` / `--facts-in` from the `src/main.rs` CLI flag
    list.
- `docs/plans/cleanup/initiative.md`: add a `plan6` bullet to "Done":
  removal of the rejected cross-racer facts carrier (with a pointer to
  `report6.md` once written).
- `docs/plans/parallel/initiative.md`: append one dated sentence to the
  Status section stating that the rejected plan10/plan11 carrier code was
  removed by `cleanup` plan6, while the reports/measurements remain the
  record. Do not rewrite any prior status text.
- Do **not** edit the plan10/plan11 plan/report files or their measurement
  scripts.

## Verification

Run the standard gates:

```bash
cargo fmt
cargo clippy --all-targets
make test
cargo doc --no-deps
```

`cargo clippy` must show no new warnings (dead code from a partial removal
will show up here). `make test` is the required gate.

### Behaviour-free proof (required)

Because only `None`-branch code is removed, the default path must be
bit-identical. Capture a baseline **before** editing, then diff **after**:

```bash
# Before any edit:
git rev-parse HEAD > /tmp/facts-cleanup-pre.sha
cargo build --release

FENS=(
  '4k3/8/8/8/8/8/8/4KRR1 w - - 0 1'
  'r7/1Rp4k/4P2B/6p1/3P2P1/p6p/P6K/8 b - - 0 35'
  '4k3/8/8/8/8/8/8/4K3 w - - 0 1'
  'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'
)
for f in "${FENS[@]}"; do
  echo "=== $f"
  ./target/release/atomic_solver --fen "$f" --timeout 20 --outcome-only 2>&1 \
    | grep -E '^(outcome|pv|evals|nodes|pre_exit|preflight|pv_status):'
done > /tmp/facts-cleanup-pre.txt
```

After removal, rebuild and rerun the exact command into
`/tmp/facts-cleanup-post.txt`; `diff -u` must be empty. In addition, run
the deterministic quick suite before/after and compare only the
outcome/`child_evals` fields (wall time is allowed to differ):

```bash
cargo run --release --example benchmark -- --suite quick --json > /tmp/quick-pre.json
# ... after removal ...
cargo run --release --example benchmark -- --suite quick --json > /tmp/quick-post.json
```

A small Python/jq comparison of `name`, `outcome`, and `child_evals` per case
is sufficient; the run is deterministic and must match exactly. Any
difference is not an acceptable "cleanup effect" — investigate it.

Finally, confirm no residual references remain outside the historical
documents:

```bash
grep -rn "FactImporter\|FactExporter\|FactImportCounters\|FactsExport\|FactsImport\|--facts-\|src/facts" \
  src tests examples AGENTS.md || true
```

The only allowed matches are in `docs/plans/parallel/**` (history) and, if
you chose to keep the phrase, the renamed test's comment must not mention
facts.

## Risks and edge cases

- **Accidental behaviour change.** The chunk-loop call and the `begin_run`
  reset are the only non-`None` facts code that runs without flags. Removing
  them must leave the loop identical; the bit-identity diff is the guard.
- **Test-only re-export.** `Search::test_resolved_from_entry` has no users
  outside `src/facts/tests.rs`. Confirm with grep before deleting; if a new
  user appeared since this plan was written, keep the re-export and note it
  in the report.
- **Unused imports.** `Arc`, `Outcome`, and `TranspositionTable` remain used
  by other code; let `cargo clippy`/`cargo build` drive the cleanup rather
  than deleting imports speculatively.
- **Historical scripts.** `docs/plans/parallel/measurements/plan10,plan11`
  invoke the removed flags and will no longer run. This is accepted by the
  owner's 2026-10-26 decision; record the removal commit SHA in the report
  so they can be resurrected from git history if ever needed.
- **File-size justification.** `src/facts/mod.rs` had a size justification
  in its module header; that disappears with the file. No `AGENTS.md`
  "File size justifications" entry exists for facts, so no replacement is
  needed.

## Final task: write `docs/plans/cleanup/report6.md`

The report must contain:

- what was removed, with the file list and a `git diff --stat` summary
  (line/file delta);
- the gate results (`cargo fmt`, `cargo clippy --all-targets`, `make test`,
  `cargo doc --no-deps`);
- the bit-identity evidence: the empty `diff` of the four-position capture
  and the quick-suite `outcome`/`child_evals` comparison;
- the pre-removal commit SHA (`/tmp/facts-cleanup-pre.sha`) and the
  post-removal `HEAD`, so the deleted carrier is recoverable from history;
- any residual references intentionally left (history only) and any
  unexpected coupling found;
- tools/examples used, problems encountered, unresolved parts, missing
  tests, and next steps (including whether the same sweep should look for
  other closed-mechanism leftovers).

Then close the session with the protocol block:

```
SESSION COMPLETE
- plan6.md executed; report6.md written; gate <result>; cleanup/initiative.md
  Done list updated; parallel/initiative.md removal note appended.
Follow-up options:
  1. <copy-pasteable kickoff for the next cleanup pass, if any named
     follow-up emerged>
  2. <close the cleanup initiative as dormant again, with reason>
```
