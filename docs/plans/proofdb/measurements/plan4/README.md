# plan4 measurements — breadth-first PNS harvester, first batch (2026-09-30)

Executes `../../plan4.md` (the plan3 pivot): job selection moved from class
gradients over the full frontier to a **live breadth-first PNS priority
queue over the open frontier**, with structural proof/disproof numbers and
a sidecar work ledger (`data/proofdb_work.json`, gitignored working layer)
as the pick-up state. Pre-registered yield expectation: ≈ 0 facts for the
number-1 pool (plan3 P0 measured the identical per-node spend over the same
class at 0) — **measured: 0 facts at 300.0M child-evals, as pre-registered.**
The deliverable of the batch is the ledger baseline: 1,443 known-open
records (75 censored at pass 1 + 1,368 freshly exposed, undecided).

## Command table (in run order)

| file | command |
| --- | --- |
| `data/proofdb.db` *(working layer, gitignored; not committed)* | `proofdb_merge --manifest ../../shards/manifest.json --shard-dir ../../shards --db data/proofdb.db --dump nodes_data.txt` — byte-identical to plan3's committed `proofdb_grown2.db` (sha256 `670e19e3…`, dump `e3060d14…`), so no duplicate is committed here; the canonical input DB is plan3's artifact |
| `data/proofdb_work.json` *(gitignored)* | empty ledger init `{"records":[]}` |
| `pns_stdout.log`, `pns_stderr.log` *(not committed)* | the batch: `proofdb_harvest --db data/proofdb.db --manifest ../../shards/manifest.json --shard-dir ../../shards --policy breadth-pns --ledger data/proofdb_work.json --max-total-evals 300000000 --stop-file /tmp/plan4_STOP` → 75 jobs (all censored, 0 decisive), 300,001,024 evals, stop=budget, 65.1 s |
| `census_breadth-pns.json` | `assemble_census.py` (driver): parses the batch transcript into job records + summary, runs the H1/H5 checks, records the exclusion census |
| `ledger_snapshot.json` | the post-batch ledger (copy of `data/proofdb_work.json` at batch end): 1,443 records = 75 censored (`passes_failed` 1, ≈ 4.0M `work_done` each) + 1,368 freshly exposed (`passes_failed` 0, `work_done` 0) — the escalation state for the next session |
| `merge_a.log`, `merge_b.log` *(not committed)* | H2: `proofdb_merge` over the (unchanged) manifest twice → DB sha256 `670e19e3…` both, dump `e3060d14…` both, and byte-identical to the input DB (H3: 0 new shards → no proven node changed) |
| `replay_stdout.log`, `replay_stderr.log` *(not committed)* | H5: the batch re-run from the same input DB + the same (initial) ledger + a fresh TT and throwaway manifest/shard-dir → 75/75 job records identical on every field except wall time, identical job sequence, and **byte-identical post-batch ledger**; doubles as the verification that the file-size-driven module split (`pns.rs` → `pns/{build,numbers,selector}`) was pure code motion |

## Result

| metric | value |
| --- | --- |
| new facts | **0 / 300.0M** (pre-registered expectation: ≈ 0) |
| jobs run | 75 — all censored at the base 4M budget, `exit_reason BudgetExhausted` |
| job classes | 9 C1 (open rows) + 66 L (fresh ledger children) |
| visited plies | 0 (1 job), 1 (20), 2 (54) — **ply sequence strictly non-decreasing** |
| session-start census | 75 open rows: 26 excluded (proven-ancestor), **49 jobs**; ledger empty |
| post-batch ledger | 1,443 records (75 censored + 1,368 exposed undecided) |

The live queue descended breadth-first exactly as the (number, ply, path)
ordering pre-registers: the whole ply-0/1/2 layer was censored at 4M each
before any deeper node; the 300M cap fired before the ply-3 layer started.
The first ladder rung (8M revisits) never fired in-session: with 0 facts,
every censor exposes fresh number-1 children, so the number-1 pool never
drains. Escalation is therefore a *cross-session* dynamic: the next session
starts with the exposed frontier at `passes_failed = 0` (number 1) and the
75 censored nodes at number 2.

## Census discrepancy vs the plan header (finding)

Plan4 §1 pre-registered "75 open rows → 26 behind a proven win, 7 implied
wins, 1 implied loss, 41 jobs". Measured: 26 proven-ancestor exclusions (✓),
**0 implied exclusions, 49 jobs**. The §1 numbers came from a no-movegen
probe; the normative §2 rule ("unexpanded legal replies count as unvisited
children = 1, movegen at the replayed position") leaves the 7 "implied win"
rows and the "implied loss" row genuinely undecided — e.g. the 1.f3-type
AND rows have 11–17 stored losing replies each but 2–9 unvisited replies
(pn > 0). §2's rule is the sound one and is what the implementation follows.
