# plan10 measurements — the tier sweep (arm A) and the `g1f3` 6B rung at the climb's TT size (arm B) (2026-10-02)

Executes `../../plan10.md` as amended by §9 (2026-10-02): the two-arm batch
over the standing post-plan9 state (root = startpos; staging copies; fresh
TT per session; policy `and-close`, order `completion`; no mid-run tuning).
D1 = the **per-job budget filter** (`--and-close-max-budget`, 0 = unlimited)
+ the `and-close-filter:` echo.

**§9 in one line:** the plan-session pilot (probe/, committed) was arm A's
pre-registered cross-run baseline; arm A's official run falsified it
(6 facts / 1,074 censors vs the pilot's 3/1,077) — the pilot's three 6B head
probes ran first in the same session and warmed the harvest session's
cross-job private TT, so its tier jobs ran under a different TT history than
arm A's. Arm A was re-baselined by a full in-session replay per §4.2; the
pilot stays committed as a falsified pre-registration. Arm B is unchanged.

## Provenance

| file | content |
| --- | --- |
| `assemble_census.py` | the driver: parses the transcripts + replays, runs the H1/§9-replay/no-exposure/union/H2/flip checks, quantifies the pilot falsification, writes the census JSONs + `verdicts.json` |
| `census_armA.json` / `census_armB.json` | per-arm job records + metrics + session-start census + §9/H5 replay result |
| `verdicts.json` | combined gate verdicts + the falsification record (`pilot_falsification`) |
| `ledger_union.json` | the standing advance: union(armA_post, armB_post), 8,446 records (sha `de690bc1…`); the /tmp union outputs (reversed, idempotent, N=1 ×2) verified byte-identical by the driver |
| `flip_analysis.json` / `flip_prerun.json` | the §3.7 analysis over the grown DB and the pre-run standing DB (both: 0 flips, root Null, fixpoint 1 round) |
| `env.json` | environment + rev + input/output state |
| `probe/` | the pilot artifacts (committed at plan time) — now labeled a **falsified pre-registration** (see its README header) |

Input state (all digests re-verified before the runs; staging copies of DB +
manifest + ledger + **all three mutable files' directories** per the §1.3
lesson): standing DB `6c724928…` (41,459 nodes); standing manifest
`9992dba4…` (256 entries); standing ledger `a696610b…` (8,446 records = 7,113
fresh + 1,274 pass-1 + 53 pass-2 + 6 pass-3). The §1 census pin was
reproduced verbatim at every session start (both arms + the replay).

## Command table (in run order; binaries `target/release/examples/…`; raw
transcripts stay in `/tmp/plan10/` — regenerable, gitignored)

0. **Arm A run 1** (executed pre-amendment; the §9 falsification event):
   `proofdb_harvest --db proofdb.db --manifest manifest.json --shard-dir
   shards --ledger ledger.json --policy and-close --and-close-order
   completion --and-close-max-budget 100000000 --budget-evals 4000000
   --max-total-evals 10500000000 --heavy-sample 0` (TT 128 MB default) →
   **1,080 jobs, 6 facts, 1,074 censors, 9,693,439,558 evals, stop
   exhausted**, wall 2097.7 s; manifest 256 → 262 entries (`e91ad57b…`);
   post ledger `9c060103…`. Census + filter echo `jobs 1080 of 1083` exactly
   per §5 H1; no-exposure exact (0 new records, 1,074 bumps, decisive
   records untouched).
1. **Arm A replay (§9 re-baseline, ~34 min):** same command, fresh staging
   copies → job lines byte-identical (excl `wall_s`), post ledger
   byte-identical (`9c060103…`), all 6 fact shards byte-identical, grown
   manifest byte-identical (`e91ad57b…`).
2. **Arm B (the `g1f3` 6B rung):** `proofdb_harvest … --policy and-close
   --and-close-order completion --max-jobs 3 --budget-evals 4000000
   --max-total-evals 18500000000 --tt-mb 1024 --heavy-sample 0` (no filter)
   → exactly the pinned head `g1f3 d7d6` / `g1f3 e7e5` / `g1f3 f7f6` at
   budgets 6,000,000,190 / 194 / 102 (pass 3, work_before ≈ 3.0B), **3
   censors at ≈ 6B each, 0 facts**, 18,000,000,517 evals, stop `max-jobs`,
   wall 4066.2 s; manifest unchanged; post ledger `4e14f46f…`.
3. **Arm B replay (§4.2, ~66 min):** same command, fresh staging copies →
   job lines byte-identical (excl `wall_s`, `child_evals` included:
   6,000,000,191 / 6,000,000,204 / 6,000,000,122), post ledger
   byte-identical (`4e14f46f…`).
4. **Union advance (§3.6/§4.3 re-scoped):** `proofdb_ledger_union --out
   union/ledger.json --expect armA/ledger.json=9c060103… --expect
   armB/ledger.json=4e14f46f… armA/ledger.json armB/ledger.json` → 8,446
   records (0 new paths; armA: 1,074 sole pass upgrades; armB: 3); reversed
   inputs, idempotence, and N=1 normalization of both arms all
   byte-identical (`de690bc1…`).
5. **Promotion:** arm A's 6 validated shards + grown manifest →
   `docs/plans/proofdb/shards/` (262 entries, `e91ad57b…`).
6. **H2:** `proofdb_merge` ×2 (+1) over the grown manifest → 262/262 shards
   replay-validated; DB `0d929f4c…` (55,703 nodes = 1 root + 55,360 overlay
   + 342 ancestors; proven 55,628, open 75) and dumps byte-identical across
   runs.
7. **Flip analysis (§3.7):** `proofdb_flip --db grown1.db --manifest <grown>
   --out flip_analysis.json` → **0 flips** (no open row reached
   AND-completeness), root Null; sanity: also 0 flips over the pre-run
   standing DB. The completion branch (§4.3) did not fire.
8. **Standing advance:** grown DB → `data/proofdb.db` (`0d929f4c…`);
   union ledger → `data/proofdb_work.json` (`de690bc1…`, 8,446 records =
   7,113 fresh + 250 pass-1 + 1,029 pass-2 + 49 pass-3 + 5 pass-4).

## Result summary

| arm | jobs | facts | censored | evals | stop | post-run ledger |
| --- | ---: | ---: | ---: | ---: | --- | --- |
| A — tier sweep ≤ 100M (TT 128 MB) | 1,080 | **6** | 1,074 | 9,693,439,558 | exhausted | `9c060103…` (8,446 records; 1,074 bumps, 0 new; no-exposure exact) |
| B — `g1f3` 6B rung (TT 1024 MB) | 3 | **0** | 3 | 18,000,000,517 | max-jobs | `4e14f46f…` (8,446 records; 3 bumps, 0 new; no-exposure exact) |

Arm A's 6 facts: 2 at the 24M tier (the pilot's two 10-ply `…e2e3` wins,
re-derived at lower work), 4 at the 8M tier (the pilot's 27-ply `e1e2` win
plus three new: `d2d4 a7a6 a2a3 a6a5 b2b3 a5a4 c2c3 a4b3 e2e3 g7g6` (10
plies), `…e3e4 b2a1q f3f4 b8d7` (24 plies), `…h2h3 b5b4 a3a4 a8a4` (18
plies)). All are White-win refutations of reply/candidate moves; the
`e1e2`/`b8d7`/`a8a4` class stays the loss-side (opponent-win) fact class of
§1.3. Fact yield 0.55 % of jobs (vs the pilot's 0.28 % — the falsification
is itself the measurement: tier-job outcomes are TT-history-dependent at
the margin).

Arm B's state after the batch: the three `g1f3` defenses now pass 3, work
≈ 9.0B each (3.0B + 6.0B); the pre-registered next rung is **18B**
(`max(2³ × 4M, 2 × 9B)`, plan10 §4.4's corrected ladder) ≈ 54B evals ≈ 3–4 h
at the climb's TT size. The 1024 MB measurement is config-matched (the
pilot's 6B censors were 128 MB) — 0 facts, the band prior holds from below.

## Verdict table

| gate / expectation | verdict |
| --- | --- |
| H1 integrity | **pass** — pinned census (1,083 = 0 fresh + 1,083 censored, active rows 49, gradient head, exclusions 26/0/0) reproduced at every session start; armA: filter echo `jobs 1080 of 1083` exact, every job budget ≤ 100M and ≥ 2 × work_before; armB: no filter echo, head exactly the pinned trio at budgets 6,000,000,190/194/102 (pass 3), `--max-jobs 3` stops the arm; no job path a standing manifest path; **no-exposure check exact both arms** (0 new records, 0 gone, bumps only, decisive records untouched) |
| §3.1 expectation (exactly the pilot's 3 facts) | **falsified — 6 facts / 1,074 censors**; §9's re-scoped expectation confirmed by the replay |
| H2 merge determinism | **pass** — 262/262 shards replay-validated; ×2 (+1) byte-identical DB `0d929f4c…` + dumps; grown manifest == arm A run 1's `e91ad57b…` (the pilot's 259-entry target is falsified, §9) |
| H3/H4 no-regression / spot-checks | **pass (re-scoped §9)** — 6 promoted facts all replay-validated by the merger; fact paths disjoint from every standing manifest path and between arms; shard byte-equality to the pilot dropped (names match, bytes differ — the export depends on the job's TT snapshot) |
| H5 policy determinism | **pass (re-scoped §9)** — arm A run 1 vs full in-session replay: job sequences identical excl `wall_s`, post-run ledgers byte-identical, all 6 fact shards byte-identical, grown manifests byte-identical; arm B run vs replay likewise (incl. `child_evals`); union determinism/idempotence/N=1 normalization byte-identical on the digest-pinned real inputs |
| H6 hygiene | **pass** — `make test` green (incl. the new `--and-close-max-budget` filter tests), `cargo clippy --release --all-targets` / `cargo fmt --check` / `cargo doc` clean, no `src/` changes, new/edited example files ≤ 10 KB, `git status` confirms `data/` ignored |
| Flip-analysis consistency | **pass (vacuous)** — 0 flips over the grown DB and the pre-run standing DB; no AND-complete row, root undecided; §4.3 completion branch did not fire |
| §4.3 handover contingency | **not triggered** — root undecided |
| §3.5 completion-critical-order candidate | **subsumed (recorded)** — fresh pool empty ⇒ censored-ascending ≡ `completion`; no new order shipped |
| Pilot pre-registration | **falsified, kept as record** — see `verdicts.json: pilot_falsification` and `probe/README.md` |
