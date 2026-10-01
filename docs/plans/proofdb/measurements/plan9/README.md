# plan9 measurements — the standing shape's first batch: the fresh-tail sweep (armA) and the strict-growth completion-head escalation (armB) (2026-10-01)

Executes `../../plan9.md`: the standing-mode batch after report8's "only
arm 2 yields" switch. Two arms over the standing post-plan8 state (root =
startpos; staging copies; fresh TT; policy `and-close`; no mid-run
tuning). D1 = the **strict-growth ladder** (`ladder_budget`: the plan8 §2
floor `work_done` → `2 × work_done`, plan9 §2).

## Provenance

| file | content |
| --- | --- |
| `assemble_census.py` | the driver: parses the transcripts + replays, runs the H1/H5/union/H2/flip checks, writes the census JSONs + `verdicts.json` |
| `census_armA.json` / `census_armB.json` | per-arm job records + metrics + session-start census + H5 replay result |
| `verdicts.json` | combined gate verdicts + the §4.4 escalation policy's application |
| `ledger_snapshot_armA.json` / `ledger_snapshot_armB.json` | per-arm post-run ledgers (shas `fbf7de23…` / `9e028a7b…`) |
| `ledger_union.json` | the standing advance: union(armA_post, armB_post), 8,446 records (sha `a696610b…`) |
| `flip_analysis.json` | the §4.6 implied-outcome analysis over the grown DB (0 flips; completion branch did not fire) |
| `flip_prerun.json` | the pre-promotion standing-DB sanity (0 flips) |
| `env.json` | environment + rev |

Input state (all digest-verified before the runs; the committed §1 probe
output was reproduced first on a throwaway staging copy): standing DB
`data/proofdb.db` sha `186d0e89…` (38,779 nodes); standing manifest
`docs/plans/proofdb/shards/manifest.json` sha `cd84fac7…` (231 entries);
standing ledger `data/proofdb_work.json` sha `462467d6…` (8,214 records
= 1,101 censored + 7,113 fresh). Session-start manifest snapshot:
`manifest_start.json` is not committed (regenerable: `git show
0d933c3:docs/plans/proofdb/shards/manifest.json`).

## Command table (in run order; binaries `target/release/examples/…`;
raw transcripts stay in `/tmp/plan9/` — regenerable, gitignored)

0. **Census probe** (§1 pins, zero evals): `proofdb_harvest … --policy
   and-close --stop-file STOP` on a throwaway copy → the §1 census lines
   reproduced verbatim; plus a 1-job fresh-order head probe (4M evals,
   committed in plan9 §1's description: a ply-19 quiet ladder reply,
   censored at 4M).
1. **Arm A (fresh sweep)**: `proofdb_harvest --policy and-close
   --and-close-order fresh --ledger ledger.json --budget-evals 4000000
   --max-total-evals 1100000000` → **257 jobs** (the full fresh pool,
   `stop=exhausted`), **25 facts**, 232 censored, 931,601,887 evals, wall
   217.6 s; manifest 231 → 256 entries (`9992dba4…`).
2. **Arm B (completion head escalation)**: `proofdb_harvest --policy
   and-close --and-close-order completion --ledger ledger.json
   --budget-evals 4000000 --max-total-evals 6500000000 --tt-mb 1024` →
   exactly the pinned head `g1f3 d7d6` / `g1f3 e7e5` / `g1f3 f7f6` at
   **budgets = 2 × work_before ≈ 2B each** (strict growth), then the
   26-job 8M sub-plateau tier, then deeper 8M/24M rows until the cap:
   **56 jobs, 0 facts**, 56 censored, 6,504,002,872 evals, wall 1534.8 s
   (stop=budget); manifest unchanged.
3. **H5 replays**: both arms re-run from fresh staging copies → job
   sequences identical (excl `wall_s`), post-run ledgers byte-identical
   (`fbf7de23…` / `9e028a7b…`).
4. **Union advance (§4.5)**: `proofdb_ledger_union --out union/ledger.json
   --expect <armA>=<sha> --expect <armB>=<sha> armA/ledger.json
   armB/ledger.json` → 8,446 records (armA: +232 new paths; armB: 56 sole
   pass upgrades); reversed inputs → byte-identical (`a696610b…`);
   idempotence → byte-identical; N=1 normalization of both arms →
   byte-identical. The g1f3 defenses keep arm B's pass-2 state (work ~3B);
   the union's max-rule absorbed the (empty by construction) overlap.
5. **Promotion**: armA's 25 validated shards + grown manifest →
   `docs/plans/proofdb/shards/` (256 entries, `9992dba4…`).
6. **H2**: `proofdb_merge` ×2 (+1) over the grown manifest → 256/256
   shards replay-validated; DB `6c724928…` (41,459 nodes = 1 root +
   41,122 overlay + 336 ancestors; proven 41,384, open 75) and dumps
   byte-identical across runs.
7. **Flip analysis (§4.6)**: `proofdb_flip --db grown1.db --manifest
   <grown> --out flip_analysis.json` → **0 flips** (no open row reached
   AND-completeness), 0 claims to verify, root undecided; sanity: also 0
   flips over the pre-run standing DB.
8. **Standing advance**: grown DB → `data/proofdb.db` (`6c724928…`);
   union ledger → `data/proofdb_work.json` (`a696610b…`, 8,446 records =
   7,113 fresh + 1,274 pass-1 + 53 pass-2 + 6 pass-3).

## Result summary

| arm | jobs | facts | censored | evals | stop | post-run ledger |
| --- | ---: | ---: | ---: | ---: | --- | --- |
| A — fresh sweep (4M each) | 257 | **25** | 232 | 931,601,887 | exhausted | `fbf7de23…` (8,446 records; +232 censors, no bumps; no-exposure exact) |
| B — completion head (strict-growth rungs) | 56 | **0** | 56 | 6,504,002,872 | budget | `9e028a7b…` (8,270 records; 56 bumps, 0 new; no-exposure exact) |

Arm A's 25 facts: 1 at ply 20, 1 at 21, 1 at 22, 3 at 24, 1 at 25, 3 at
26, **15 at ply 27** — the deepest class the arm-2 screen censored is
exactly the yielding one (9.7 % of jobs vs the arm-2 screen's 4.3 %). 17
of the 25 are odd-ply jobs (refutations of AND-row replies, toward
AND-completeness), 8 are even-ply jobs (refutations of OR-row moves).
All bounds small (1–15 plies; 16 rows bound 5, 2 rows bound 1, …). None
completed a row (0 flips).

Arm B's plateau state after the batch (the report's §3 data): the three
`g1f3` defenses now pass 2, work ≈ 3.0B each (1B measured + 2B this
batch); their pinned next rung is **6B** (2 × cumulative), then 12B —
climbing toward the 7–15B-equiv band. The 26 sub-plateau replies (e2e3 ×7,
e2e4 ×7, g1h3 ×9) now pass 2, work ≈ 12M each → next visit 24M. The 10/11
missing-row replies and the root's visited replies likewise 8M → 24M; the
pre-existing pass-2/3 records now carry 24M/36M work (next 56M/72M).

## Verdict table

| gate / expectation | verdict |
| --- | --- |
| H1 integrity | **pass** — pinned census (1,108 = 257 + 851, active rows 49, gradient head, exclusions 26/0/0) reproduced first as a committed probe; armA budgets/pass-0 pin exact; armB head + 8M tier + strict-growth inequality exact; no job path a standing manifest/DB-row path; **no-exposure check exact** (armA: +232 new censors, 0 bumps; armB: 56 bumps, 0 new) |
| H2 merge determinism | **pass** — 256/256 shards validated; ×2 (+1) byte-identical DB `6c724928…` + dumps |
| H3/H4 no-regression / spot-checks | **pass** — 25 facts, all shards replay-validated by the merger; no same-path outcome contradiction between arms (armB had 0 facts) |
| H5 policy determinism | **pass** — both arms: job sequences identical excl `wall_s`, post-run ledgers byte-identical (incl. armB's 6.5B-eval session); **plus union determinism** on the real advance inputs, byte-level (reversed order, idempotence, N=1 ×2) |
| H6 hygiene | **pass** — `make test` green (incl. the updated strict-growth ladder tests + the equal-work regression case), `cargo clippy --release --all-targets` / `cargo fmt --check` / `cargo doc` clean, no `src/` changes, new/edited example files ≤ 10 KB, `git status` confirms `data/` ignored |
| Flip-analysis consistency | **pass (vacuous)** — 0 flips over the grown DB and the pre-run standing DB; no AND-complete row, root undecided |
| Arm A expectations (several facts, 5–15) | **exceeded — 25 facts**; the full pool swept within the cap (stop=exhausted) |
| Arm B expectations (≈ 0 facts) | **confirmed — 0 facts**; the strict-growth rungs are the measured bounds now |
| §4.3 completion branch | **did not fire** — no `g1f3` defense decided, no row AND-complete |
| §4.4 escalation policy | **applied** — fresh tier yielded (25 facts) → base stays 4M; censor tier escalates automatically (g1f3 next rung 6B; sub-plateau tier 24M); one batch per escalation |
| Handover contingency | **not triggered** — root undecided |
