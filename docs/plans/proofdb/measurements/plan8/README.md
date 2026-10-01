# plan8 measurements — the `and-close` completion gradient: the deep-probe session and the fresh-tail screen (2026-10-01)

Executes `../../plan8.md`: the first facts of the initiative. Two arms over
the standing post-plan7 state (root = startpos; staging copies; fresh TT;
cap 3B child-evals each; policy `and-close` = plan8 D1, no mid-run tuning).

## Provenance

| file | content |
| --- | --- |
| `assemble_census.py` | the driver: parses the transcripts + replays, runs the H1/H5/union/H2 checks, writes the census JSONs + `verdicts.json` |
| `census_arm_1.json` / `census_arm_2.json` | per-arm job records + metrics + session-start census + H5 replay result |
| `verdicts.json` | combined gate verdicts + the §4.3 standing-mode switch |
| `ledger_snapshot_arm1.json` / `ledger_snapshot_arm2.json` | per-arm post-run ledgers (shas `fd08019c…` / `1444d87b…`) |
| `ledger_union.json` | the standing advance: union(arm1_post, arm2_post), 8,214 records (sha `462467d6…`) |
| `flip_analysis.json` | the §4.5 implied-outcome analysis over the grown DB (0 flips; completion branch did not fire) |
| `env.json` | environment + rev |

Input state (all digest-verified before the runs): standing DB
`data/proofdb.db` sha `670e19e3…` (35,055 nodes); standing manifest
`docs/plans/proofdb/shards/manifest.json` sha `37c1f5b2…` (197 entries);
standing ledger `data/proofdb_work.json` sha `0d0a86ee…` (7,547 records).

## Command table (in run order; binaries `target/release/examples/…`;
raw transcripts stay in `/tmp/plan8/` — regenerable, gitignored)

1. **Arm 2 (fresh-tail screen)**: `proofdb_harvest --policy and-close
   --and-close-order fresh --ledger ledger.json --budget-evals 4000000
   --max-total-evals 3000000000` → 784 jobs, **34 facts**, 750 censored,
   3,001,870,921 evals, stop=budget, manifest 197 → 231 entries
   (`cd84fac7…`).
2. **Arm 1 (deep probes)**: `proofdb_harvest --policy and-close
   --and-close-order completion --ledger ledger.json --budget-evals
   1000000000 --max-total-evals 3000000000 --tt-mb 1024` → exactly the
   pinned sequence `g1f3 d7d6`, `g1f3 e7e5`, `g1f3 f7f6`, each
   **censored at 1,000,000,0xx child-evals** (BudgetExhausted), 0 facts,
   cap fired, manifest unchanged.
3. **H5 replays**: both arms re-run from fresh staging copies → job
   sequences identical (excl `wall_s`), post-run ledgers byte-identical.
4. **Union advance (§4.4)**: `proofdb_ledger_union --out union/ledger.json
   --expect <arm1>=<sha> --expect <arm2>=<sha> arm1/ledger.json
   arm2/ledger.json` → 8,214 records (arm 2: +667 new paths, 80 sole pass
   upgrades); reversed inputs → byte-identical (`462467d6…`);
   idempotence → byte-identical; N=1 normalization of arm2_post →
   byte-identical. The three deep-probe paths carry arm 1's work (pass 1,
   ~1B) over arm 2's 4M bumps — the monotone-budget rule preserved.
5. **H2**: `proofdb_merge` ×2 over the grown manifest → 231/231 shards
   replay-validated; DB `186d0e89…` (38,779 nodes) and dumps byte-identical
   across runs (and a third run against the promoted manifest).
6. **Flip analysis (§4.5)**: `proofdb_flip --db grown1.db --manifest
   arm2/manifest.json --out flip_analysis.json` → 0 flips (no open row
   reached AND-completeness), 0 claims to verify, root undecided; sanity:
   also 0 flips over the pre-run standing DB.
7. **Promotion + standing advance**: grown manifest →
   `docs/plans/proofdb/shards/manifest.json` (231 entries); grown DB →
   `data/proofdb.db` (sha `186d0e89…`); union ledger →
   `data/proofdb_work.json` (sha `462467d6…`, 8,214 records = 1,101
   censored + 7,113 fresh).

## Result summary

| arm | jobs | facts | censored | evals | post-run ledger |
| --- | ---: | ---: | ---: | ---: | --- |
| 1 — deep probes (`g1f3`'s 3 defenses at 1B) | 3 | **0** | 3 | 3,000,000,060 | `fd08019c…` (7,547 records; 3 bumps to pass 1, work ~1B) |
| 2 — fresh-tail screen (4M each) | 784 | **34** | 750 | 3,001,870,921 | `1444d87b…` (8,214 records; +667 new, 83 bumps; no-exposure exact) |

Arm 2's 34 facts: 26 at ply 10, 1 at ply 11, 2 at ply 12, 2 at ply 14,
2 at ply 17, 1 at ply 18 — all `win` from the reply's side-to-move
perspective, i.e. all refutation steps toward AND-completion of their
parent rows (none completes a row yet). The 12 ply-2 jobs (`g1f3`'s 3 +
`g1h3`'s 9 defenses) all censored at 4M — their first in-harness probes,
confirming the head rows quiet at the screen scale.

## Verdict table

| gate / expectation | verdict |
| --- | --- |
| H1 integrity | **pass** — pinned census (active rows 49, replies 1142), gradient head (`g1f3:3 e2e3:7 e2e4:7 g1h3:9 root:13`), exclusions 26/0/0, budgets = base; **plan-model finding:** the §1 pinned split 970/172 is not reproducible from the standing ledger (958 no-record / 83 pass-0 / 101 censored; §2's fresh definition gives 1041/101) — see report8 finding 1 |
| H1 no-exposure (arm 2) | **pass (exact)** — post = start ∪ censored paths (667 new + 83 in-place bumps, all censors, max pass 1) |
| H2 merge determinism | **pass** — 231/231 shards validated; ×2 (+1 vs promoted) byte-identical DB `186d0e89…` + dumps |
| H3/H4 no-regression / spot-checks | **pass** — 34 facts, every shard replay-validated by the merger; no same-path contradiction between arms (arm 1 had 0 facts) |
| H5 policy determinism | **pass** — both arms: sequences identical excl `wall_s`, ledgers byte-identical; union determinism/idempotence/normalization byte-identical |
| H6 hygiene | **pass** — `make test` green (57 tests in the proofdb target incl. 7 new and-close tests), clippy/fmt/doc clean, no `src/` changes, `data/` ignored |
| Flip analysis consistency | **pass (vacuous)** — 0 flips; the tool's independent verifier is exercised (it rejected an early buggy OR-claim during development, then 0 claims on both DBs) |
| Arm 1 expectations (≈ 0 facts; cap after the 3 pinned probes) | **confirmed** — 0 facts, measured bounds ≥ 1B evals per defense |
| Arm 2 expectations (≈ 0 facts) | **refuted — 34 facts**; the cold-4M zero-fact precedent is broken at the fresh tail |
| §4.1 completion branch | **did not fire** — no `g1f3` defense decided, no row AND-complete |
| §4.3 standing-mode switch | fires per the "only arm 2 yields" branch: the fresh sweep continues at 4M; the completion head escalates in parallel batches |
