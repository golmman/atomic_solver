# plan7 measurements — batch 3: the shipped-default control vs the ledger-union arm (2026-10-02)

Executes `../../plan7.md`: the two-arm A/B over the standing post-plan6 state
(root = startpos; staging copies; fresh TT; cap 300M, base 4M; policy
`breadth-pns` = the shipped default, no `--pns-config`; no mid-run tuning),
plus the D1 ledger-union tool's gate checks on the real inputs.

## Provenance

| file | content |
| --- | --- |
| `assemble_census.py` | the driver: parses the transcripts, runs the H1/H5/§4.3 checks, writes the census JSONs + `union.json` + `verdicts.json` |
| `census_arm_1.json` / `census_arm_2.json` | per-arm job records + metrics + session-start census + H5 replay result |
| `union.json` | the union artifact record: pinned input digests, composition, per-input attribution, gate results |
| `verdicts.json` | combined gate verdicts, the re-censor metric, the §4.5 decision rule, the §4.6 closure data |
| `ledger_union.json` | the union ledger itself (5,950 records; sha `896cdd4d…`) |
| `ledger_snapshot_arm1.json` / `ledger_snapshot_arm2.json` | per-arm post-run ledgers (shas `61097620…` / `0d0a86ee…`) |
| `env.json` | environment + rev |

Input state (all digest-verified before the runs): standing DB
`data/proofdb.db` sha `670e19e3…` (35,055 nodes; unchanged since plan3);
standing ledger `data/proofdb_work.json` sha `058a6202…` (4,569 records =
plan6 arm C's post-run state; byte-identical to
`../plan6/ledger_snapshot_C.json`); plan6 arm snapshots A `7fd92ea2…` /
B `9aa0c326…`.

## Command table (in run order; binaries `target/release/examples/…`;
raw transcripts stay in `/tmp/plan7/` — regenerable, gitignored)

1. **Union build** (`D1` on the real inputs): `proofdb_ledger_union --out
   /tmp/plan7/union/ledger.json --expect <standing=058a6202…> --expect
   <snapshot_A=7fd92ea2…> --expect <snapshot_B=9aa0c326…> standing A B` →
   5,950 records (the pinned expectation; a deviation would have been a
   model defect).
2. **Union gates (H5-union, shell-level)**: re-run with reversed input
   order → byte-identical (`896cdd4d…`); idempotence
   `union(u, standing, A, B)` → byte-identical; N=1 normalization of the
   standing ledger → byte-identical to the input (`058a6202…` — it was
   already in canonical form). §2.2 coverage + the §2 rule checked over
   the real artifacts in the driver.
3. **Arm 1 (control)**: staging copy of DB + manifest + the standing
   ledger + symlinked shards; `proofdb_harvest --db db.db --manifest
   manifest.json --shard-dir shards --policy breadth-pns --ledger
   ledger.json --max-total-evals 300000000` → 75 jobs, 0 facts,
   300,001,000 evals, stop=budget.
4. **Arm 2 (union)**: same, `--ledger` = the union ledger → 75 jobs,
   0 facts, 300,001,020 evals, stop=budget.
5. **H5 replays**: both arms re-run from fresh staging copies → job
   sequences identical (excl `wall_s`), post-run ledgers byte-identical.
6. **H1**: the pinned session-start census lines matched verbatim
   (arm 1: `ledger records 4559 … jobs 4608`; arm 2: `5938 … 5987`;
   otherwise identical incl. the cfg echo of the compiled defaults);
   job paths in the post-run ledgers; census quadruple consistent;
   manifests unchanged.
7. **H2**: `proofdb_merge` twice over the standing manifest/shards →
   DB `670e19e3…` both times, = the input DB (0 promoted facts — the
   censored-session residue check); dumps byte-identical.
8. **Standing-ledger advance (§4.5 side effect)**:
   `cp /tmp/plan7/arm2/ledger.json data/proofdb_work.json` → sha
   `0d0a86ee…`, 7,547 records (301 censored / 7,246 fresh). The standing
   DB, manifest, and shard set untouched (0 facts → promotion vacuous).

## Verdict table

| gate / expectation | verdict |
| --- | --- |
| H1 integrity (pinned census, job paths, quadruple) | **pass** — both arms, cfg echo included |
| H2 merge determinism | **pass** — ×2 byte-identical, = input DB at 0 facts |
| H3/H4 no-regression / spot-checks | **pass (vacuous)** — 0 facts in both arms |
| H5 policy determinism (per-arm replay) | **pass** — both arms |
| H5 union determinism / idempotence / normalization / coverage | **pass** — byte-level (see `union.json`) |
| H6 hygiene | **pass** — `make test` green (incl. the 5 new union tests; 50 in the proofdb target), clippy/fmt/doc clean, no `src/` changes, union files ≤ 10 KiB, `data/` ignored |
| Arm 1 expectations (75 visits; 24/24/24/3; re-censors 20–24; ≈ 0 facts; ledger +1,400–1,600) | **confirmed** — 24 re-censors (all ply-2 visits land on A-censored nodes), waste 96,000,242 evals = **32.0% of the cap** (the plan's ≤ 96M estimate, measured dead-on); 0 facts; ledger +1,658 (slightly above the +1,400–1,600 range; see findings) |
| Arm 2 expectations (75 visits; 24/24/24/3; **0 re-censors**; ≈ 0 facts; ledger +1,400–1,600, all new knowledge) | **confirmed** — 0 re-censors, 0 waste; 0 facts; ledger +1,597, all genuinely new paths |
| **§4.5 decision rule (the union's marginal value)** | **fires on all three** — re-censors 24 → 0, facts 0 ≥ 0, max ply 5 ≥ 5. **The union mechanism ships**: the standing ledger advanced to arm 2's post-run state; `proofdb_ledger_union` is the standing-state merge tool and the N-way primitive item 6 builds on |
| **§4.6 closure rule** | **fires** — sixth consecutive 0-fact 300M session; **plan8 must change the yield outlook** (depth-rationing lever or fact-yield-oriented selection change); another identical default batch is not a valid plan8 |

## Result summary

| arm | jobs | facts | plies visited (jobs) | re-censors (waste) | post-run ledger |
| --- | --- | --- | --- | --- | --- |
| 1 — control (standing ledger) | 75 (75 expand) | 0 | 2: 24, 3: 24, 4: 24, 5: 3 | **24** (96.0M = 32.0% of cap) | `61097620…` (6,227 records = 300 censored + 5,927 fresh) |
| 2 — union (5,950-record base) | 75 (75 expand) | 0 | 2: 24, 3: 24, 4: 24, 5: 3 | **0** (0) | `0d0a86ee…` (7,547 records = 351 censored + 7,196 fresh) — **the standing state now** |

Both arms ran the identical ply shape 24/24/24/3 — the layer-visit caps
bound identically; the difference is entirely *what* the ply-2 ration was
spent on: arm 1 re-censored 24 A-known-quiet nodes at 4M each (96.0M
evals, 32.0% of the session cap — finding 4 made measurable and matching
the plan's ≤ 96M estimate exactly), arm 2 censored 24 genuinely virgin
ply-2 nodes and the whole 300M budget bought new knowledge. The recovered
A-knowledge (1,381 paths + 57 pass upgrades) rode along at zero cost.
