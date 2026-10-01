# plan5 measurements — batch 2, the unchanged-policy ladder-reachability control (2026-10-01)

Executes `../../plan5.md`: batch 2 runs the **unchanged plan4 policy**
(`breadth-pns`, base 4M, geometric ladder, cap 300M) over the exposed
frontier as the control measurement that converts "the ladder can never
fire in-session" (report4 finding 3, plan5 Lemma 2) from arithmetic into a
measured result. Deliberately a no-code-change plan — the §2 decision
(per-layer budget split with a scheduled ladder reserve) is plan6 work.

## Pre-registered expectations (§4 decision 2) — measured verdicts

| expectation | pre-registered | measured |
| --- | --- | --- |
| **E1** queue shape | every visit from the ply-2 fresh sublayer (289 available > 75 visits), constant job ply 2, cap fires before drain, ply 3 never reached | **confirmed** — 75/75 visits at ply 2 (class L, pass 1, number 1, work 0), cap before drain (75 < 289) |
| **E2** yield | ≈ 0 facts (cheap facts live at ply ≥ 4) | **confirmed** — 0 facts / 300.0M evals |
| **E3** ladder never fires | no visit at `pass ≥ 2` (Lemma 2); a measured **no** is the success outcome | **confirmed** — max pass = 1, no rung fired |
| **E4** ledger growth | 1,443 → ≈ 2,890 (accepted 2,600–3,200), all new records at ply 3 | **confirmed** — 1,443 → 3,033 (in range); 1,590 new records, all ply 3; exactly 75 bumps (passes_failed 0→1); exposure 21.2 children/visit |
| **E5** determinism | full replay → identical job sequence, byte-identical post-batch ledger | **confirmed** — job records identical on every field except wall_s; post-batch ledger sha256 `4f168105…` identical; `pns:` census line identical |

All three contingency triggers (§4 decision 3) are clear: E3 not violated,
E2 = 0 ≤ 10 facts — **the §2 decision stands as pre-registered.**

## Command table (in run order)

| file | command |
| --- | --- |
| `data/proofdb.db`, `data/proofdb_work.json` *(gitignored, not committed)* | session-start state verified by sha256: DB `670e19e3…` (= plan3's committed `proofdb_grown2.db`), ledger `c15661d5…` (= plan4's committed snapshot), manifest `37c1f5b2…` |
| `census_breadth-pns.json` | `assemble_census.py` (driver): parses the batch + replay transcripts, runs the decision-5 census match and the H1/E1–E5/H5 checks, writes the job records + summary |
| `ledger_snapshot.json` | the post-batch ledger (copy of `data/proofdb_work.json` at batch end): 3,033 records = 150 censored (all at `passes_failed` 1, ≈ 4.0M `work_done` each — batch 1's 75 and batch 2's 75 first visits; no node has yet earned a second visit) + 2,883 fresh undecided — the escalation state for plan6's batch 3 |
| `pns_stdout.log` / `pns_stderr.log`, `replay_*`, `merge_a/b.log`, `probe_*` *(not committed)* | raw transcripts, per the measurement conventions |

Commands (batch: `target/release/examples/…`):

1. **Census probe** (read-only, throwaway copies in `/tmp/plan5/`): `proofdb_harvest --db data/proofdb.db --manifest docs/plans/proofdb/shards/manifest.json --shard-dir docs/plans/proofdb/shards --policy breadth-pns --ledger /tmp/plan5/probe_ledger.json --max-total-evals 300000000 --stop-file /tmp/plan5/STOP` (stop-file pre-created → 0 jobs, census line only; probe ledger unchanged, sha `c15661d5…` after).
2. **Batch 2**: same command with `--ledger data/proofdb_work.json`, no stop-file → 75 jobs (all censored, 0 decisive), 300,001,045 evals, stop=budget, 63.9 s job time (65.6 s wall).
3. **H2**: `proofdb_merge --manifest …/manifest.json --shard-dir …/shards --db /tmp/plan5/merge_{a,b}.db --dump /tmp/plan5/merge_{a,b}.txt` twice → DB sha256 `670e19e3…` both, dump `e3060d14…` both, byte-identical to the input DB (H3 vacuous-and-bytewise at 0 new shards).
4. **E5/H5 replay**: batch re-run from the same input DB + a copy of the initial ledger → job records identical (excl. wall_s), post-batch ledger byte-identical (`4f168105…` both).

## Result

| metric | value |
| --- | --- |
| new facts | **0 / 300.0M** (pre-registered: ≈ 0) |
| jobs run | 75 — all class L (fresh ledger children), all censored at the base 4M budget, `exit_reason BudgetExhausted` |
| visited plies | 2 only (75 of the 289 fresh ply-2 records) — **job ply sequence constant** |
| session-start census | `pns: rows 35055 (open 75), ledger records 1434 (1434 new, 0 dropped as decided); exclusions proven-ancestor r26/l0, implied-win r0/l0, implied-loss r0/l0; jobs 1483 (rows 49, ledger 1434); base budget 4000000` — byte-matches the §1-pinned line |
| number split (measured) | 1,408 number-1 (1,368 fresh ledger + 40 unvisited rows) / 75 number-2 (66 censored ledger + 9 censored rows) |
| post-batch ledger | 3,033 records: 150 censored + 2,883 fresh; 1,590 new ply-3 records = 21.2 exposed per censored visit (batch 1 measured 18.2) |

**The ladder-reachability verdict (the plan's goal measurement):** with the
measured number-1 pool at 1,408 × 4M ≈ 5.6B against the 300M cap, and each
censored visit exposing 21.2 new number-1 records while removing at most
itself, the ladder rungs are — as Lemma 2 predicted and batch 2 now
measures — **never reachable in-session**. The session spent its entire cap
on 26% of the fresh ply-2 sub-layer (75/289) and died before ply 3. This is
the data justification for the pre-registered §2 decision (per-layer budget
split with a scheduled ladder reserve; plan6 implements it; plan6's arm A′
must byte-match this run as its refactor-equivalence anchor).

Derived (not measured in-session) next-session census arithmetic from the
post-batch snapshot: ≈ 3,073 jobs ≈ 2,923 number-1 + 150 number-2 — the
pool grew +1,515 net over 75 visits (+20.2/visit), confirming the
monotone-growth premise.
