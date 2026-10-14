# report19 — `--moves <path>`: the scoped harvest (frontier-walk re-anchored at a startpos path)

Executes `plan19.md` (initiative item 16, added with the plan).
Gate: `make test` green — all suites 0 failures (release profile); clippy
and `cargo fmt --check` clean; `cargo doc` clean (no broken intra-doc
links). No `src/` changes — selection-layer only, as planned.

## What was done

**S1 — flag, parsing, validation** (`examples/proofdb/harvest_args.rs`):

- `Args.moves: Vec<String>`; `--moves <string>` splits on whitespace (the
  path-key join; no comma dialect — a comma-joined token is rejected as a
  non-legal move) and is replay-validated sequentially from the startpos
  via `parse_moves(Some(&raw)) -> Result<Vec<String>, String>` (a
  `#[cfg(test)]`-testable function; `parse_args` maps `Err` → usage
  exit 1). `None` (flag absent) = global session; `--moves ''` (or
  whitespace-only) is rejected. A malformed or prefix-illegal token is
  named in the message (`token "zz9x9" at ply 1 is not a legal move — the
  whole path must replay legally from the startpos`). Validation runs
  inside `parse_args`, before `main` loads any manifest/DB/ledger state.
- Usage text updated; parser unit tests (good path + whitespace
  normalization, absent flag, empty value, nonsense token, illegal
  prefix, comma dialect) live in the new `harvest_args.rs` tests module
  (run via the `tests/proofdb.rs` verbatim include).

**S2 — re-anchoring semantics** (`examples/proofdb/walk.rs` +
`examples/proofdb/walk/driver.rs`):

- `WalkFrontier::new(ledger, db, scope)`; the frontier stores the scope
  and keys its order/complete/decided sets **scope-relative** (`rel_of` /
  `join_scope` helpers; the scope root is always relative `""`, so
  `root_blocked` needed no signature change). Candidacy: `in_scope`
  (empty scope admits everything — the global rules degenerate exactly to
  the pre-plan19 ones, so absent-flag behavior is bit-identical);
  out-of-scope records are counted as `out_of_scope` and never picked,
  visited, or written.
- Drops are evaluated on the scope-relative path against a proven set
  built from DB rows *within* the scope, stripped — proven rows above the
  scope root are deliberately invisible; a proven row at the scope key
  drops everything in scope (relative `""` prefix counts).
- Ordering stays `(relative ply, relative path)` lex — among in-scope
  records this equals the old absolute order (they share the scope
  prefix), and the scope root is ply 0 like the game start globally.
  `pick` returns the **absolute** key (driver and job records keep
  speaking absolute paths — job JSON unchanged).
- `root_state(db, session, scope)` finds the row at the scope key (the
  empty-scope find is unchanged: the game root row). The driver's
  `root_proven_stored` and virtual-root visit now speak the scope key
  (`JobClass::Open` = C1 relative to the scope). A proven-stored scope
  root stops the session `exhausted` and prints the stderr note
  `scope root "<path>" is proven-stored — nothing to harvest` (ruling b);
  the note fires only under a non-empty scope, so the global session's
  stderr shape is unchanged.
- `run_walk_batch` / `run_walk_session` gained a `scope: &str` parameter
  (clippy `too_many_arguments` allow on the batch — one arg per piece of
  the walk's per-batch state, same precedent as `visit`).

**S3 — observability** (additive only):

- Census line now `frontier-walk: records N (lineage L, ancestor A,
  out_of_scope O, complete 0) — root <state>; base budget B; scope
  <path|global>`; the `harvest: db` echo gains `; scope <path>` when
  scoped. Job records unchanged (absolute paths already).

**Call-site updates**: the five `tests/proofdb.rs` and six
`walk/tests.rs` frontier/root-state call sites, plus the driver
invocations, now pass `""` (behavior-pinning — all re-pinned tests pass
unchanged except the pinned census-line string).

**Tests added**:

- Unit (`walk/tests.rs`): scope candidacy + ordering (scope root record's
  live work respected, relative ply 0), relative drops incl. the
  ignored-above-scope case, proven-scope-root drops everything in scope
  while out-of-scope records survive, scoped `root_state` (open / proven
  / fresh / manifest-shard-proven) + the scoped census line.
- Driver-level (`tests/proofdb.rs`): virtual root at the scope key and
  in-scope-only expansion (ledger keys `a2a3`, `a2a3 a7a5`, `a2a3 a7a6`);
  proven scope root exhausts with zero jobs and a byte-untouched ledger;
  a full `run_walk_session` scoped pickup over the fixture DB (out-of-scope
  ledger record preserved value-identically).
- Binary-level (`tests/proofdb_harvest_cli.rs`, new target): the three
  exit-1 unhappy paths (nonsense token, illegal prefix, empty value) each
  proven to touch no state (started from a truly empty dir); a scoped
  session's census/echo/job paths/ledger (jobs `f2f3 e7e6` then
  `f2f3 e7e6 a2a3`, classes C1/C3); the proven-scope exhausted note with
  zero jobs; the absent-flag global shape (census `scope global`, no echo
  suffix, job paths `""` then `a2a3` — the manual's pinned smoke shape).

**Docs** (`docs/proofdb_pipeline.md`): usage block + `--moves` option
row; a scoped-harvest bullet in the selection policy (re-anchor semantics,
out-of-scope preservation, proven-scope rule); the root-rule bullet and
`stop=exhausted` paragraph now speak the scope root; both census example
lines and the census-description paragraph updated to the new grammar.
`AGENTS.md`'s `proofdb_harvest` bullet mentions `--moves`.

## Tools used

- `cargo fmt` / `cargo clippy --all-targets` / `cargo test --release`
  (the `make test` gate) / `cargo doc`.
- `tests/proofdb_harvest_cli.rs` drives the real binaries through
  `cargo run --release --quiet --example proofdb_harvest|proofdb_merge`
  (the `test_benchmark_json.rs` harness style) — a probe confirmed
  `CARGO_BIN_EXE_*` is not set for example targets, so the style was
  adopted deliberately. A one-shot `proofdb_merge` builds a shared
  per-process fixture DB (lazily, `OnceLock` + run lock) so each CLI test
  pays one merge total.

## Problems encountered

- **A stray `STOP` file at the repo root** (an operator artifact from a
  previous session) silently zeroed every CLI-test harvest session via
  the default `--stop-file STOP` (cargo runs with the repo as cwd). The
  CLI tests pass an explicit `--stop-file <tmp>/STOP`. Left in place —
  it is `.gitignore`d operator control state by design (`.gitignore`
  line 36), not repo litter; flagged here so nobody "fixes" the tests
  by assuming a clean cwd.
- **Two of my own test assertions were wrong, not the code**: job JSON
  fields are emitted alphabetically (`"class":"C3","exit_reason":…,
  "kind":"expand"` — a `"class":"C3","kind":"expand"` substring can never
  match), and after 1.f3 e6 White is to move again (first expansion child
  `a2a3`, not Black's `a7a5`).
- **`WalkCensus::describe`'s `; scope` also appears in the census line**
  for a global session (`base budget 1000; scope global`), so the
  no-echo-suffix assertion must target the echo line specifically.

## Bit-identity evidence (§0 requirement)

- All five pre-existing driver/frontier tests with `""` scope pass
  byte-unchanged (pinned ledger keys, byte-identical ledger rebuilds,
  census root states, zero-eval censor guard).
- The CLI-level absent-flag test pins the global smoke shape (job paths
  `""`, `a2a3`; classes C1/C3; `stop=max-jobs jobs 2`).
- The census **string** changed (accepted S3 format change; report16
  pinned the old wording — the manual records the new shape; no in-repo
  test pinned the old string except the one updated here).

## Unresolved parts / limitations (accepted by plan rulings)

- A scope chosen inside alternative-move territory can harvest nodes
  whose outcome is implied by a proven ancestor **above** the scope
  (those rows are invisible in-session) — ruling (a)/(b): the operator
  chose the scope deliberately; `proofdb_flip` remains the cross-check.
- Growing beneath a proven node stays out of scope (plan4 decision 9) —
  a proven scope root is a valid but empty session (`exhausted` + note).
- No in-process test captures the stderr note text (process-level only);
  the CLI test pins it.

## Missing tests

- No test pins the census line against a *global* session through
  `run_walk_session` in-process (the CLI test covers it via the real
  binary; the in-process `walk_boot_from_empty_layer` covers the session
  assembly but not the line text).
- No test for `--moves` pointing at a scope node whose *children* hit
  the zero-eval repetition guard inside a scoped session (the guard is
  scope-agnostic by construction; the global test covers the mechanism).

## Next steps

- Item 6 (independent harvesters) is the natural consumer: `--moves` is
  a per-worker subtree partition primitive — workers can each claim one
  scope with the shared ledger untouched elsewhere.
- If the census line grows further, consider splitting
  `WalkCensus`/`RootState` out of `walk.rs` (its justification header
  names the split line at ~20 KB, which plan19's growth nearly reached).
