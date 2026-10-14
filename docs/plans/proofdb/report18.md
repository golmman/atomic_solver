# report18 — policy consolidation: `frontier-walk` becomes the only harvest policy

Executes `plan18.md` (owner request 2026-10-14; initiative item 15).
Gate: `make test` green — 33 suites, 0 failures (release profile); clippy
and `cargo fmt --check` clean; `cargo doc` clean; all examples build.

## What was done

**Removed** (≈5.4k lines incl. tests):

- `examples/proofdb/pns.rs` + `pns/` (the breadth-pns selection mechanism,
  plans 4–6: build, census, config, driver, numbers, pacing, selector).
- `examples/proofdb/and_close.rs` + `and_close/` (the plan8–10
  completion-gradient policy and ladder).
- `examples/proofdb/descend.rs` (the plan15 off-tree bootstrap policy).
- `examples/proofdb/batch.rs`, `frontier.rs` + `frontier/`, `policy.rs`
  (the plan2/3 legacy gradients and their frontier-class extraction).
- The plan8/plan15/legacy test sections in `tests/proofdb.rs`
  (1,725 → 981 lines); the CLI branches and flags (`--pns-config`,
  `--and-close-order`, `--and-close-max-budget`, `--descend-plies`,
  `--heavy-budget-evals`, `--heavy-sample`) in `proofdb_harvest.rs` +
  `harvest_args.rs`; `Session::run_pns_job`; the `parent_bound` (C2-only)
  and `number` (PNS-only) census fields.

**Relocated** (the three shared bits the walk borrowed):

- `and_close::driver::on_censored` → `ledger::on_censored` (ledger-native:
  bump + atomic save).
- `Pns::child_key` → free fn `harvest::child_key` (path identity).
- `BUDGET_PNS_BASE_EVALS` → `walk::BASE_EVALS` (the walk's standing base).

**Kept** (behavior unchanged inside the walk):

- `walk.rs` + `walk/` semantics, the ledger format and pick-up gate, the
  durable layer (shard format, manifest contract, DB schema), and the
  `data/proofdb/` defaults. The `--policy` flag is kept for command-line
  compatibility but accepts only `frontier-walk` (default), so existing
  scripts keep working.

**Docs**: `docs/proofdb_pipeline.md` rewritten where policies mattered —
§2.2 tool description; §3 quickstart now a single walk boot-and-grow
cycle (§3.1 smoke re-pinned from a fresh run: `jobs 2 decisive 0
censored 2 evals 400015`; §3.2 production cycle with the plan16
reference-boot numbers: 553 jobs / 53 shards / 6,706 nodes / 2G evals);
the old §3.3 removed; §7.2 CLI reference rewritten (slim option table,
walk selection-policy summary, current job-JSON grammar); §8.1 recipes
(R0's boot batch is now a capped walk session; R2 harvest command);
§11 history gained the plan18 entry, and pre-plan18 entries are
explicitly historical. `AGENTS.md` harvester bullet updated.
`docs/spec/*` untouched (verified: no policy references there).

## Verification

- `make test` (default gate) green; `tests/proofdb.rs` down to 41 tests,
  all passing, including the walk boot tests (`walk_boot_from_empty_layer`
  proves the walk-only cold start on a root-only layer), the crash-sync
  tests (plan17), the stale-DB guard, the derived-DB rebuild gate, and
  the plan14 defaults pin.
- Functional smoke (release build, temp layer, no production state
  touched): R0 bootstrap merge → `--policy frontier-walk --budget-evals
  200000 --max-jobs 2` produced the expected virtual-root C1 search visit
  and `a2a3` C3 expansion visit, both censored, ledger written, manifest
  unchanged. (The first smoke run stopped at `stop-file` — a `STOP` file
  exists in the repo root; re-run with an explicit `--stop-file` to
  confirm. Operators should be aware of this pre-existing STOP file.)

## Problems encountered

- Two truncated-file incidents while editing `tests/proofdb.rs`: caused
  by my own tooling error (running an edit and a same-file `sed` in
  parallel; and a bad interactive edit), not by the codebase. Recovered
  from git both times and redone sequentially; final file verified
  intact (981 lines, tail check, full test run).

## Unresolved parts / notes

- **Ledger records with old censor history**: unchanged format, so the
  standing `data/proofdb/work.json` (and committed snapshots) pick up
  under the walk exactly as before — the walk already interpreted
  ladder-era records as flat `work_done`. No migration needed.
- **The `lad_*`/`p2_*`/`sib_*` manifest tag conventions** remain
  documented (§6) — historical provenance tags in the standing shard
  set; new shards are all `h_*`.
- **Item 11 (ledger-leaf harvest policy)** predates the consolidation
  and described a descend-tail scheduling mode; with `descend` removed
  its "the censored leaves are the frontier" framing is subsumed by the
  walk itself (ledger records *are* the frontier). Left open/low-prio in
  `initiative.md` for the owner to retire or re-frame at the next pivot.
- **Item 6 (parallel harvesters)** is untouched and remains the next
  lever; its prerequisite (ledger = per-worker partition state) is
  unchanged by this plan.

## Missing tests

- None intentionally. The removed policies took their tests with them;
  the walk's D1–D7 contract tests (plan16 section) were already
  policy-independent. A future test could pin the *usage text* (the
  accepted-policy list) — currently only exercised manually; judged not
  worth a snapshot test.

## Tools / examples used

- `cargo build/test/clippy/fmt/doc`, `make test`.
- Functional smoke via `proofdb_merge` + `proofdb_harvest` against a
  `/tmp` layer (release build).

## Next steps

- Owner decision on item 11 (retire vs. re-frame post-consolidation).
- Item 6 (independent harvesters) remains the initiative's next lever;
  the ledger-as-partition-state prerequisite is intact.
