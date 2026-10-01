# plan6 measurements — the four-arm A/B over the selection mechanism (2026-10-01)

Executes `../../plan6.md`: the pre-registered A/B over the exposed frontier
from the identical post-batch-2 state (input DB = `data/proofdb.db` sha256
`670e19e3…`; input ledger = the plan5 snapshot `ledger_snapshot.json` = sha
`4f168105…`; fresh TT, cap 300M, base 4M, root = startpos; throwaway
staging copies in `/tmp/plan6/<arm>/`). Arms A′/B/C ran the new selector
with `--pns-config`; arm A ran the pre-refactor binary (the plan4 selector
as-is; its transcript was captured before any code change).

## Verdict table (pre-registered gates + expectations)

| gate / expectation | verdict |
| --- | --- |
| H1 integrity (census line, in-tree paths, census quadruple) | **pass** — all four arms; the session-start census is identical across arms modulo the `cfg` echo; number split 2,923 number-1 + 150 number-2 = 3,073 jobs (matches plan5's derived arithmetic exactly) |
| H2 merge determinism (merger ×2 → byte-identical; = input DB at 0 facts) | **pass** — DB `670e19e3…`, dump `e3060d14…`, both runs, = input DB |
| H3/H4 no-regression / spot-checks over promoted facts | **pass (vacuous)** — 0 facts in every arm |
| H5 policy determinism (per-arm replay) | **pass** — A′, B, C: job sequences identical excl `wall_s`, post-run ledgers byte-identical |
| **H5 A′ ≡ A (the equivalence contract)** | **pass** — 75/75 job records identical on every field except `wall_s` and the new `kind`; post-run ledgers byte-identical (`7fd92ea2…` both) — the new selector is a verified strict superset of the plan4 selector |
| Arm A expectation (≈ 75 censored visits, all ply-2 fresh, ≈ 0 facts, no rung) | **confirmed** — 75 jobs, all ply 2, all pass 1, 0 facts |
| Arm B expectations | **partially** — 9 rungs (pre-registered "≈ 9, reserve-bound": **confirmed**, the reserve was the binding limit at 80.0M ≥ 75M), rotation order + mid-session eligibility flip (c2c3) as designed, 0 facts as pre-registered; **max ply reached = 4, not ≈ 8** (the ply-4 fresh pool from ply-3 censors absorbed the post-ply-3 visits — the plan's reach prediction ignored the cascade, see findings) |
| Arm C expectation (B minus rungs) | **confirmed** — 75 expansions, plies 2–5 (24/24/24/3), 0 facts |
| **§4.5 decision rule (the ladder's marginal value)** | **ladder no-go** — B ≤ C on facts (0 = 0); all 9 rungs censored (root at 8M/16M, seven ply-1 nodes at 8M); no depth dividend (B's max ply 4 < C's 5; B spent 80.0M of 300M on plies 0–1 revisits that settled nothing). **The default config becomes arm C's**: `reserve_share = 0.0` (compiled default updated; the rung mechanism stays available via `--pns-config`) |

## Command table (in run order; binaries `target/release/examples/…`)

| file | content |
| --- | --- |
| `data/proofdb.db`, `data/proofdb_work.json` *(gitignored)* | the pre-run standing state, sha-verified (DB `670e19e3…` = plan3's committed `proofdb_grown2.db`; ledger `4f168105…` = plan5's post-batch snapshot) |
| `census_arm_<A,A_ap,B,C>.json` | `assemble_census.py` (driver): parses the batch + replay transcripts, runs the H1/H5/A′≡A checks, writes the job records + summaries + verdicts |
| `verdicts.json` | the combined gate verdicts, arm-B mechanics, and the §4.5 decision table |
| `ledger_snapshot_<arm>.json` | each arm's post-run ledger (4 files); the **standing** `data/proofdb_work.json` advanced to arm C's post-run state (sha `058a6202…`, 4,569 records: 225 censored / 4,344 fresh) |
| `env.json` | environment + rev |

Commands (per arm; staging = fresh copies of the input DB + ledger +
manifest, symlinked standing shards, fresh TT = new process):

1. **Arm A** (pre-refactor binary, no config): `proofdb_harvest --db staging.db --manifest manifest.json --shard-dir shards --policy breadth-pns --ledger ledger.json --max-total-evals 300000000` → 75 jobs, 0 facts, 300,000,926 evals, stop=budget. (The pre-refactor binary was snapshotted before the code change; `cargo build` was clean at rev `4f33349`.)
2. **Arm A′**: same + `--pns-config arm_ap.toml` (`reserve_share = 0.0`, `layer_visit_cap = 0`) → 75 jobs, 0 facts, 300,000,926 evals — **≡ arm A**.
3. **Arm B**: same + `--pns-config arm_b.toml` (the §2 table verbatim) → 64 jobs (55 expand + 9 rung), 0 facts, 300,000,785 evals.
4. **Arm C**: same + `--pns-config arm_c.toml` (`reserve_share = 0.0`, `layer_visit_cap = 24`) → 75 jobs, 0 facts, 300,000,925 evals.
5. **H5 replays**: each of A′/B/C re-run from fresh staging copies → job sequences identical (excl `wall_s`), post-run ledgers byte-identical. (Arm A was replayed with the pre-refactor binary at the same protocol — pass.)
6. **H2**: `proofdb_merge --manifest docs/plans/proofdb/shards/manifest.json --shard-dir docs/plans/proofdb/shards --db merge_{a,b}.db --dump merge_{a,b}.txt` twice → byte-identical DB + dump, = the input DB (0 promoted facts).
7. **Promotion**: vacuous — 0 new shard files in every staging shard dir; the standing manifest/shard set untouched.
8. **Standing ledger advance**: `cp arm_C/ledger.json data/proofdb_work.json` (the winning arm per §4.5).

## Result summary

| arm | jobs (kind) | facts | plies visited | post-run ledger |
| --- | --- | --- | --- | --- |
| A (plan4 selector) | 75 (75 expand) | 0 | 2 (73 fresh records + 2 open rows of the ply-2 pool) | `7fd92ea2…` (4,933 records = 225 censored + 4,708 fresh: 141 ply-2, 4,567 ply-3) |
| A′ (degenerate cfg) | 75 (75 expand) | 0 | 2 (same) | `7fd92ea2…` — **byte-identical to A** |
| B (§2 mechanism defaults) | 64 (55 expand + 9 rung) | 0 | 0–4: 2×24, 3×24, 4×7 | `9aa0c326…` (4,178 records = 205 censored + 3,973 fresh) |
| C (ladder deletion) | 75 (75 expand) | 0 | 2–5: 2×24, 3×24, 4×24, 5×3 | `058a6202…` (4,569 records = 225 censored + 4,344 fresh) — **the standing state now** |

Arm B's rung ledger (the mechanism trace): root pass 2 (8M) at V=0; then
the seven ply-1 nodes whose children the standing state had fully visited
(a2a3, a2a4, b1a3, b1c3, b2b3, b2b4 — eligible at start — then c2c3, whose
10 virgin children the session's ply-2 visits closed) at 8M each; then the
root's pass-3 rung (16M) — reserve-bound (80.0M ≥ 75M spent), rotation
order fewest-passes → (ply, path) verified from the census `kind` trace.
Every rung censored; the ladder bought no fact and no deeper settled line.

## Findings

1. **Arm B's reach prediction ("≈ ply 8") was wrong — measured ply 4.** The
   prediction assumed the ply-4+ number-1 pool stays sparse (the unvisited
   rows only); it ignored that each ply-3 censor exposes ~21 fresh ply-4
   records (~500 after ply 3's 24 visits), so the post-ply-3 visits went to
   the ply-4 fresh pool, not the sparse rows. The pre-registered decision
   rule (facts/rungs/depth, §4.5) is unaffected — but layer caps ration
   *breadth per layer*, they do not by themselves force depth progress; a
   future plan wanting depth must ration by ply *front* (e.g. cap fresh
   exposures per layer, not only visits).
2. **The eligibility trigger works and is not vacuous.** Eight nodes were
   rung-eligible at session start (root + seven ply-1 nodes whose ply-2
   children batches 1–2 had fully censored) and one flipped mid-session
   (c2c3) — exactly the "rung-eligible = censored and no virgin child"
   semantics, measured end to end.
3. **The reserve is the ladder's real bound.** 9 rungs / 80.0M evals (the
   final rung overshot the 75M reserve by one budget, per the
   caps-not-quotas rule); eligibility never bound (13+ candidates existed).
   A future ladder experiment should tune `reserve_share`, not
   `max_rung_passes`.
4. **The standing ledger advance drops arm A's censor knowledge** (75
   additional ply-2 nodes visited by A/A′ are back at passes 0 in C's
   ledger). Pre-registered (§4: the standing ledger advances to the
   winning arm); consequence: the next session re-censors those nodes at
   4M instead of resuming at pass 2 — accepted, noted here for the next
   batch's sizing.
