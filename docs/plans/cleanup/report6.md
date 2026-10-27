# Report: Plan 6 — remove the rejected cross-racer facts carrier

Implements `cleanup/plan6.md` in one session. The cross-racer solved-fact
channel from the closed `parallel` initiative (plan10/plan11) is gone from
the product surface; search behaviour is unchanged.

## Baseline

- Pre-removal commit (`/tmp/facts-cleanup-pre.sha`): `65d7e42eead270332f4c95a0e5bea4e599731cca`.
- Pre-removal build: `cargo build --release` clean; four-position capture and
  the quick benchmark were run before any edit.
- Post-removal `HEAD` at report time: `65d7e42eead270332f4c95a0e5bea4e599731cca`
  (the removal is in the working tree; this session used no writing `git`
  commands, so there is no removal commit yet). The carrier can be recovered
  from `git show 65d7e42:src/facts/mod.rs` / `src/facts/tests.rs` and the
  corresponding file revisions of this commit.

## Removed

| File | Change |
| --- | --- |
| `src/facts/mod.rs` | deleted (624 lines) |
| `src/facts/tests.rs` | deleted (396 lines) |
| `src/lib.rs` | dropped `pub mod facts;` |
| `src/search/dfpn/mod.rs` | removed `FactImportCounters`, `FactImporter`, `FactExporter`, the three `Search` fields and their setters/getters, `drain_fact_imports()`, the chunk-boundary drain call, the `begin_run` counter reset, the `emit_proof_node` exporter branch, and the test-only `test_resolved_from_entry` re-export |
| `src/search/dfpn/children.rs` | removed the terminal-child exporter branch; collapsed the now-redundant nested `proof_event_sender` `if` (behaviour identical) |
| `src/search/dfpn/tests.rs` | deleted the `CountingImporter` and `RootFactImporter` tests; kept the TT-reuse test as `evaluate_child_resolves_solved_entry_without_move_hint` with facts-free framing |
| `src/cli.rs` | removed `facts_out`/`facts_in` fields, defaults, parse arms, destructuring entries, and their tests |
| `src/main.rs` | removed the header/help contract, `use atomic_solver::facts`, wiring and summary blocks |
| `AGENTS.md` | removed `facts` from the lib re-export list, the `src/facts/` architecture bullet, and the two CLI flags |
| `docs/plans/cleanup/initiative.md` | added the plan6 “Done” entry |
| `docs/plans/parallel/initiative.md` | appended the dated removal sentence to Status (prior text untouched) |

`git diff --stat` (working tree, deletion-inclusive):

```
 11 files changed, 26 insertions(+), 1469 deletions(-)
 AGENTS.md                         |  15 +-
 docs/plans/cleanup/initiative.md  |   4 +
 docs/plans/parallel/initiative.md |   5 +
 src/cli.rs                        |  52 ----
 src/facts/mod.rs                  | 624 ------------------------------
 src/facts/tests.rs                | 396 ------------------------------
 src/lib.rs                        |   1 -
 src/main.rs                       |  92 ------
 src/search/dfpn/children.rs       |  29 +-
 src/search/dfpn/mod.rs            | 126 --------
 src/search/dfpn/tests.rs          | 151 +--------
```

No residual product references remain:

```bash
grep -rn "FactImporter\|FactExporter\|FactImportCounters\|FactsExport\|FactsImport\|--facts-\|src/facts" \
  src tests examples AGENTS.md
# (no output)
```

Matches remain only in `docs/plans/parallel/**` history, as intended. The
plan10/plan11 plans, reports, and measurement scripts were not edited.

## Gates

- `cargo fmt` — clean.
- `cargo clippy --all-targets` — clean, 0 warnings. (The only new warning
  after the mechanical removal was `collapsible_if` in
  `children.rs::evaluate_child`; the two nested `if`s were collapsed into one
  `if let` chain, preserving the exact `ProofEvent` emission behaviour.)
- `make test` — green (all default-gate unit/integration tests; slow tests
  ignored as designed).
- `cargo doc --no-deps` — clean.

## Behaviour-free evidence

### Four-position wall-clock capture

The pre/post `--timeout 20 --outcome-only` capture is identical for three of
four positions. The fourth (atomic startpos) is a *timeout-bounded* run that
reports elapsed work, so its final `evals:` line is not reproducible even
against the same binary:

```
# startpos, pre-removal binary rebuilt from HEAD, repeated:
evals: 133503096
evals: 132750468
# startpos, post-removal binary, repeated:
evals: 129112743
evals: 130166033
```

The original capture differed only on that line
(`132852374` pre vs `125413296` post); every other line of all four
positions matched exactly. This is clock-sampling variance at the timeout,
not a trajectory change.

To obtain a deterministic diff, the same four FENs were rerun against the
pre-removal binary and the post-removal binary with a fixed child-eval
budget (`--timeout 20 --budget 12000000 --outcome-only`). The outputs,
including `outcome`, `pv`, `evals`, and `nodes`, are **byte-identical**:

```
diff -u /tmp/facts-budget-pre.txt /tmp/facts-budget-post.txt
# (empty)
```

### Deterministic quick suite

`benchmark --suite quick --json`, before and after, compared per case on
`name`, `outcome`, and `child_evals`:

- 59/59 cases present;
- 0 differences;
- total `child_evals` = `90,514,218` on both sides.

Wall time was allowed to differ, per the plan; it did not affect any
outcome/`child_evals` field.

## Findings and deviations

- The plan’s required literal four-position `diff` cannot be empty for the
  startpos case because that case exits on the wall clock. This was
  investigated and pinned as pre-existing timing variance (two runs of the
  pre binary differ by ~0.75 M evals; two post runs differ similarly). The
  deterministic budget rerun and the quick suite provide the actual
  bit-identity proof.
- `src/search/dfpn/children.rs` carried an exporter branch that the plan’s
  prose inventory listed only indirectly (via `emit_proof_node`); it was
  removed too, which was required for the grep to be clean.
- The plan asked to keep the `proof_event_sender` block “exactly as is”, but
  removing the exporter left a collapsible nested `if`. Clippy’s “no new
  warnings” gate took precedence; the block was collapsed with identical
  emission semantics.
- No unexpected coupling was found: the only non-test consumer of the
  test-only `test_resolved_from_entry` re-export was `src/facts/tests.rs`,
  which was deleted with it. `Arc` remains in `main.rs` for the stop flag,
  and `Outcome`/`TranspositionTable` remain used elsewhere in `search::dfpn`.

## Tools/examples used

- `cargo build --release`, `cargo fmt`, `cargo clippy --all-targets`,
  `make test`, `cargo doc --no-deps`.
- `git archive HEAD` into a scratch directory to rebuild the pre-removal
  binary for the deterministic comparison (no writing git commands).
- `examples/benchmark --suite quick --json` plus a small Python comparison
  of `name`/`outcome`/`child_evals`.
- `grep -rn` residual-reference sweep.

## Unresolved parts / missing tests

- No new tests were added, by design: the removed flags now fall through to
  the existing unknown-option error path, and the plan explicitly forbids
  replacement tests for removed options.
- The retained `evaluate_child_resolves_solved_entry_without_move_hint` test
  continues to pin the TT-reuse invariant that the deleted importer tests
  happened to exercise.
- The plan10/plan11 measurement scripts still reference the removed flags and
  will not run; this was accepted by the owner’s 2026-10-26 decision. They
  are recoverable from commit `65d7e42` (or the later removal commit).

## Next steps

- Commit the working-tree removal (owner/next session, since this session is
  read-only with respect to git). Record that commit as the post-removal SHA
  in this report if desired.
- A future cleanup sweep could grep for other closed-mechanism leftovers
  whose flags/hooks survived their initiative’s closure; this plan found none
  beyond the facts carrier.

SESSION COMPLETE
- plan6.md executed; report6.md written; gate `make test` green (fmt/clippy/doc
  clean); cleanup/initiative.md Done list updated; parallel/initiative.md
  removal note appended.
Follow-up options:
  1. "Commit the plan6 removal (working tree: deleted src/facts/, edited
     search/dfpn, cli, main, AGENTS.md, and plan initiative docs; report is
     docs/plans/cleanup/report6.md) and record the resulting SHA in report6."
     — closes the loop so the removed carrier has a stable post-removal commit.
  2. Close the cleanup initiative as dormant again — plans 1–6 are complete
     and no named follow-up remains.
