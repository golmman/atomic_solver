# plan18 — policy consolidation: `frontier-walk` becomes the only harvest policy

Executes owner request 2026-10-14 (proofdb initiative, tooling-first pivot).
Not a backlog item; recorded in `initiative.md` as item 15 (policy
consolidation).

## Motivation

The harvester accumulated six policies over plans 2–16 (`breadth-pns`,
`and-close`, `descend`, `frontier-walk`, plus the plan3 legacy gradients
`open-deepest`/`sharp-siblings`/`sharp-heavy-tail`). The owner uses only
`frontier-walk`. Every new harvest feature must work and be tested against
all policies — an unreasonable maintenance and extensibility cost for dead
surface. This plan removes everything except `frontier-walk`.

Accepted trade-offs (owner decision):

- **Boot speed.** `descend` was the fast cold-start batch (420 ply≤2 paths,
  ~52 decisive in one batch). The walk bootstraps a root-only layer itself
  (first visit = the root, the §3.3 manual walkthrough), so capability is
  kept; only boot speed regresses.
- **Ladder semantics.** The `and-close` strict-growth ladder was the only
  mechanism that automatically re-priced hard censored nodes at growing
  budgets. Walk-only growth is flat-budget: deepening comes from raising
  `--budget-evals` between sessions (underworked ledger records are
  re-searched) and from expansion into children.

## Changes

1. **Relocate the three shared bits the walk borrows** from doomed modules:
   - `and_close::driver::on_censored` → `ledger.rs` (bump + atomic save;
     ledger-native).
   - `Pns::child_key` → free fn `child_key` in `harvest.rs` (path identity).
   - `BUDGET_PNS_BASE_EVALS` → `BASE_EVALS` in `walk.rs` (the walk is its
     only user).
2. **Delete** `pns.rs` + `pns/`, `and_close.rs` + `and_close/`, `descend.rs`,
   `batch.rs`, `frontier.rs` + `frontier/`, `policy.rs` (the legacy gradients
   and the precomputed-sequence machinery die with them). Drop `mod.rs` decls.
3. **Job/record slimming**: `JobClass::SharpSibling`/`Descend` variants gone
   (the walk uses `Open`/`OpenChild`/`Ledger` only); `Job.parent_bound` and
   `JobRecord.number` (C2/PNS-only) removed from the job JSON grammar.
   `Session::run_pns_job` removed.
4. **CLI** (`proofdb_harvest.rs` + `harvest_args.rs`): single walk branch;
   the frontier-extraction census header goes (the extractor's AND-check was
   a PNS-era invariant). `--policy` kept, accepting only `frontier-walk`
   (default), so existing command lines keep working. Dropped flags:
   `--pns-config`, `--and-close-order`, `--and-close-max-budget`,
   `--descend-plies`, `--heavy-budget-evals`, `--heavy-sample`.
5. **Tests** (`tests/proofdb.rs`): delete the and-close (plan8), descend
   (plan15), and legacy-policy test sections; keep ledger-union, derived-DB
   gate, stale guard, CLI defaults, walk (plan16), crash-sync (plan17)
   sections; update the remaining policy-shape assertions.
6. **Docs**: `docs/proofdb_pipeline.md` — walk-only quickstart/production
   cycle, CLI reference, recipes, troubleshooting; the walk boot becomes the
   only boot path (R0 bootstrap merge unchanged). `AGENTS.md` harvester
   bullet updated. `docs/spec/*` untouched (no policy references there).
7. **Bookkeeping**: `initiative.md` item 15 + log entry; `docs/plans/README.md`
   row update; `report18.md` (final task).

## Non-goals

- No change to the durable layer: shard format, manifest contract, DB
  schema, ledger format, and the existing `data/proofdb/` state stay as-is
  (the shared ledger keeps its semantics; old censors pick up fine).
- No behavior change inside `walk.rs`/`walk/` beyond the relocated helpers.

## Final task

Write `report18.md`, update the `README.md` row and `initiative.md`.
