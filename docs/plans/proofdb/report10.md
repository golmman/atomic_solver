# Report 10 — the tier sweep's 6 facts (the pilot pre-registration falsified, §9 re-scope executed); the `g1f3` 6B rung censors at the climb's TT size (next rung 18B)

Executes `plan10.md` as amended by §9 (2026-10-02). All deliverables landed;
all pre-registered gates **pass** under the re-scoped contracts. The batch's
central event is the **falsification of the plan-session pilot as arm A's
pre-registered baseline**: arm A's official run found **6 facts / 1,074
censors**, not the pilot's 3/1,077, and the root cause is measured (the
pilot's head probes warmed the harvest session's cross-job private TT). Arm
A was re-baselined by a full in-session replay per §4.2 and reproduced
**byte-identically** (job lines, post ledger, all 6 fact shards, grown
manifest). Arm B (unchanged by the amendment) ran the three `g1f3` defenses
at their 6B rung **at TT 1024 MB** — all three censored, 0 facts, work ≈ 9B
each, next rung **18B**. The §4.3 completion branch did **not** fire; the
flip analysis reports **0 flips** over the grown DB (and the pre-run standing
DB); the handover contingency is **not triggered**. Docs + `examples/`-side
code only; the product solver (`src/`), DB schema, and spec are untouched;
decision 10 ("flips stay derived") stands.

**Standing layer after plan10** (all digests recomputed after the advance):
`data/proofdb.db` sha `0d929f4c3c62e4bbb87e9b043b582716297b23e2f7332a36e82763b1486070b8`
(55,703 nodes = 1 root + 55,360 overlay + 342 ancestors; proven 55,628, open
75), manifest `docs/plans/proofdb/shards/manifest.json` sha
`e91ad57b44008fee65ab0b124fab51365c17d08f891777cde525cfecb61fb5fd` (262
entries), `data/proofdb_work.json` sha
`de690bc1880a44c2bce75be705c3d30b9c151271d909d739d967ce350534b054` (8,446
records = 7,113 fresh + 250 pass-1 + 1,029 pass-2 + 49 pass-3 + 5 pass-4).
Post-batch census: job set 1,077 = 0 fresh + 1,077 censored (the 6 refuted
replies left the pool); active rows still 49; frontier C1 75 / C2 951,429 /
C3 1,724.

## D1 — the per-job budget filter (§2/§3.1)

`--and-close-max-budget <evals>` (0 = unlimited, the default and the
pre-plan10 behavior): jobs whose resolved ladder budget exceeds the cap are
dropped from the sequence; the census lines are unchanged (they describe the
full job set), and the new echo `and-close-filter: max-budget <n>, jobs
<kept> of <total>` describes what actually runs (empty when unlimited).
~20 lines in `and_close/driver.rs` (`apply_budget_filter`, 7.8 KB) +
`harvest_args.rs` plumbing + `proofdb_harvest.rs` field; `and_close.rs`
untouched (stays ~10.1 KB). Tests in `tests/proofdb.rs`
(`and_close_budget_filter_plan10_d1`: unlimited no-op, cap between the
fresh/ledger classes keeps exactly the cheaper ones with the exact echo,
cap above everything drops nothing). `make test` green (incl. the new
test), clippy/fmt/doc clean.

## §9 — the falsification event (the batch's main finding)

The plan-session census probe unintentionally ran the whole batch at true
budgets and was committed as the pilot (`measurements/plan10/probe/`),
pre-registering arm A as "exactly the pilot's outcome, byte-for-byte". Arm
A's official run falsified that: **6 facts / 1,074 censors / 9,693,439,558
evals** vs the pilot's 3/1,077/9,695,668,462 on the shared 1,080-job set.

- **What stayed identical:** the sequence order, every job's budget, pass,
  and `work_before` (the strict-growth ladder is bookkeeping-deterministic —
  0 divergences on all shared jobs), the session-start census, the filter
  echo, and the no-exposure signature (0 new records, bumps only, decisive
  records untouched).
- **What diverged:** `child_evals` on 1,053/1,080 shared jobs (same budgets,
  different within-job search), and 3 censored jobs flipped to wins
  (`exit_reason` `BudgetExhausted` → `Complete` on the same 3 paths).
- **Root cause (measured):** the harvest session retains one private TT
  across jobs (`session.rs` module doc). The pilot ran all 1,083 jobs in one
  session — its three 6B head probes ran *first* and warmed the 128 MB TT
  for the 1,080 tier jobs; arm A drops those probes (`--and-close-max-budget
  100M`), so its tier jobs run under a different TT history. §1.3's premise
  "arm A's config is pilot-matched (… identical sequence)" was false in the
  decisive respect. Cross-checks: two fresh-staging 30-job prefix runs on
  independent builds are byte-identical (job lines excl `wall_s` +
  post ledger; LTO vs no-LTO byte-identical — `det1`/`det2`/`nolto`), and a
  single-job cold run of the first diverging job (`e2e3 c7c6`, 24M budget)
  reproduces arm A's `child_evals` (24,000,080), not the pilot's
  (24,000,085) (`skip1`) — the pilot's values are reachable only from a
  warmed TT.
- **Consequences executed per §9:** arm A re-baselined by a full in-session
  replay (passed byte-exact everywhere, including all 6 fact shards and the
  262-entry grown manifest `e91ad57b…`); the union target moved to the arms'
  own digest-pinned post ledgers; the H2 manifest target moved to arm A
  run 1's; the H3 pilot shard byte-equality was dropped (arm A's 3
  shared-name shards carry the pilot's *names* — path-hashed tags — but
  different *bytes*: the proof-subtree export depends on the job's TT
  snapshot; two sizes match, the 27-ply one differs 28,642 vs 29,710 B).
  The pilot artifacts stay committed, re-labeled a falsified
  pre-registration in `probe/README.md`.
- **Process lesson (normative going forward):** a cross-session artifact can
  serve as a determinism baseline only if the *entire session* is
  sequence-identical — the plan8/9 per-arm in-session replay rule is the
  only sound determinism gate whenever any part of the job set is filtered
  or reordered relative to the baseline's session. This also means: a
  **censor record is scheduling state, not a soundness artifact** — the
  pilot's "measured bounds ≥ 6B at 128 MB" were correct descriptions of
  those runs but not invariant properties of the positions; soundness lives
  only in the decisive, replay-validated shards.

## The arms (root = startpos; staging copies of all mutable files; no mid-run tuning)

| arm | config | jobs | facts | censored | evals | stop |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| A — tier sweep ≤ 100M | `completion`, filter 100M, base 4M, TT 128 MB, cap 10.5B | 1,080 | **6** | 1,074 | 9,693,439,558 | **exhausted** |
| B — `g1f3` 6B rung | `completion`, no filter, `--max-jobs 3`, base 4M, TT 1024 MB, cap 18.5B | 3 | **0** | 3 | 18,000,000,517 | max-jobs |

**Arm A** ran its full 1,080-job set (the 1,083-census minus the three
filtered head probes) and stopped `exhausted` — the 10.5B cap never fired
(9.69B spent). Tier table (official run; the pilot's tier predictions in
parentheses where they differ):

| tier | jobs | facts (pilot) | censored |
| --- | ---: | ---: | ---: |
| 8M | 1,027 | **4** (1) | 1,024 |
| 24M | 48 | **2** (2) | 45 |
| 72M | 5 | **0** (0) | 5 |
| 6B head | 0 — filtered | — (0) | — |

The 6 facts (all merged `validate: ok`, all replay-validated by the merger):

| fact path (last moves) | plies | tier | effect |
| --- | ---: | --- | --- |
| `…d2d3 a4b3 e2e3` + `g7g6` | 10 | 24M | pilot's fact 1 re-derived (work 8.5M ≪ 24M) |
| `…d2d3 a4b3 e2e3` + `h7h5` | 10 | 24M | pilot's fact 2 re-derived (work 13.0M) |
| `d2d4 a7a6 a2a3 a6a5 b2b3 a5a4 c2c3 a4b3 e2e3` + `g7g6` | 10 | 24M | **new** — the d2d4-a7a6 row 11 → 10 |
| `d2d4 d7d5 … c3c4 b3b2 e3e4 b2a1q f3f4 d5c4 g3g4 c7c6` + `e1e2` | 27 | 8M | pilot's fact 3 re-derived — the OR-row's candidate refuted, row 12 → 11 |
| `…e3e4 b2a1q f3f4` + `b8d7` | 24 | 8M | **new** — AND-row reply refuted |
| `…h2h3 b5b4 a3a4` + `a8a4` | 18 | 8M | **new** — AND-row reply refuted |

All 6 are White-win refutations of reply/candidate moves. The `e1e2` fact
and the two new 8M classes keep §1.3's **loss-side (opponent-win) fact
class** documented: these facts record root-loss lines (the replier wins
after the refuted move); the flip analysis verifies every implication
recursively, and no same-path contradiction exists (the fact paths are
disjoint from every standing manifest path — pinned in H1).

**Arm B** ran exactly the pre-registered head — the pinned trio at budgets
6,000,000,190 / 6,000,000,194 / 6,000,000,102 (pass 3, work_before ≈ 3.0B
each) — and all three defenses **censored at ≈ 6B each at TT 1024 MB**
(child_evals 6,000,000,191 / 6,000,000,204 / 6,000,000,122; the §3.2 band
[6.0B, 6.1B] holds). This is the climb's first ≥ 2B measurement at the
climb's TT size (the pilot's 6B censors were 128 MB runs and, per §9, are
no longer treated as config-matched bounds anyway). Work after ≈ 9.0B each;
the pre-registered next rung is **18B** (`max(2³ × 4M, 2 × 9B)`, §4.4's
corrected ladder — plan9's "12B" prose slip stands corrected) ≈ 54B evals
≈ 3–4 h at this TT size. 0 facts is the valid measured result (the band's
lower edge is 7–15B-equiv; the rung ladder keeps climbing toward it).

## Gates (all pass; details in `measurements/plan10/verdicts.json`)

| gate | verdict |
| --- | --- |
| H1 integrity | **pass** — §1 census pin reproduced verbatim at every session start (both arms + replay); filter echo exact; head pins exact; strict growth everywhere; no job a standing manifest path; no-exposure exact (0 new records both arms) |
| H2 merge determinism | **pass** — 262/262 shards replay-validated; merger ×2 (+1) byte-identical DB `0d929f4c…` + dumps; grown manifest == arm A run 1's `e91ad57b…` |
| H3/H4 no-regression / spot-checks | **pass (re-scoped §9)** — 6 facts validated; paths disjoint from standing manifest and between arms; pilot shard byte-equality dropped (falsified) |
| H5 policy determinism | **pass (re-scoped §9)** — arm A run 1 ≡ replay and arm B run ≡ replay, byte-exact (job lines excl `wall_s`, ledgers, shards, manifests); union gates byte-exact on the real inputs (reversed order, idempotence, N=1 ×2; `de690bc1…`) |
| H6 hygiene | **pass** — `make test` green (incl. the filter tests), clippy/fmt/doc clean, no `src/` changes, files ≤ 10 KB (driver 7.8 KB, harvest_args 4.6 KB), `data/` confirmed ignored |
| Flip-analysis consistency | **pass (vacuous)** — 0 flips over the grown DB and the pre-run standing DB; no AND-complete row; root undecided; completion branch did not fire |

## The ladder after the sweep (next-rung table; plan11 sizing input)

| class | records | pass | work | next rung | next-rung cost |
| --- | ---: | ---: | ---: | ---: | ---: |
| 8M tier (now 12M work) | 1,024 | 2 | 12M | 24M | ≈ 24.6B |
| 24M tier (36M) | 45 | 3 | 36M | 72M | ≈ 3.3B |
| 72M tier (108M) | 5 | 4 | 108M | 216M | ≈ 1.08B |
| `g1f3` head (9B) | 3 | 3 | ≈ 9B | 18B | ≈ 54B |
| **total** | | | | | **≈ 83B ≈ 5 h** |

The escalation is ~3× per rung; the measured fact yield of a full sweep is
now **0.55 % of jobs** (6/1,080; the pilot's 3/1,083 = 0.28 % was itself
TT-history-dependent). At the measured ~4.6M evals/s a next full sweep costs
≈ 5 h — plan11 must size the 18B `g1f3` rung (the informative one: ≥ the
band's upper edge — either a defense decides or the band estimate is
falsified) against the tier sweep's marginal yield, possibly as a
head-only batch (the tier sweep's 24.6B 8M tier yields ~4 facts/run at
0.55 %; the completion-critical rows now sit at missing 8–10).

## Findings

1. **The falsification (§9)** — the batch's main result: pre-registered
   cross-run baselines are unsound unless the whole session is
   sequence-identical; the per-arm in-session replay (plan8/9 rule) is the
   only sound determinism gate. Censor records are scheduling state, not
   bounds; soundness lives only in replay-validated decisive shards. The
   D1 filter was the trigger (dropping the head probes changed the TT
   history) — but any future job-set change would have been too.
2. **Tier-job outcomes are TT-history-dependent at the margin.** 3 of the
   pilot's censors decide under arm A's TT history. Fact yield at a given
   budget is a property of the run configuration, not the job; the DB's
   facts are unaffected (each is replay-verified), but batch *forecasting*
   should use the re-baselined 0.55 %, not the pilot's numbers.
3. **The `g1f3` climb question survives its first config-matched
   measurement**: 6B at 1024 MB censors all three defenses (work ≈ 9B).
   The 18B rung is the next informative point; nothing between 6B and 18B
   exists in the ladder.
4. **The completion-critical-order candidate is confirmed subsumed**
   (§3.5, measured): fresh pool empty ⇒ censored-ascending ≡ `completion`;
   no new order shipped.
5. **Process:** the amendment (§9) was drafted *before* the replay and arm B
   ran — the falsified baselines were retired as a unit, and no gate was
   loosened mid-run (the §3.1/§4.1/§4.3/H1/H2/H3 re-scopes are all recorded
   in plan10 with the measured evidence). Compute: arm A run 1 + replay
   ≈ 70 min, arm B + replay ≈ 134 min (runs executed concurrently on 4
   cores; byte-identical results), union + promotion + merger ×3 + flips +
   census ≈ 10 min.

## Unresolved / next steps

- The 18B `g1f3` rung (≈ 54B ≈ 3–4 h) — plan11's decision; also whether the
  next batch is head-only or another full sweep (≈ 83B ≈ 5 h).
- Item 5 (website handoff) and item 6 (parallel harvesters) remain open;
  item 6's per-worker partitioning now also needs a TT-history note: worker
  results are sound (shards replay-validated) but per-job yields depend on
  the worker's TT history — budget accounting stays exact.
- The DTM-upgrade pass (item 3) remains untouched; the 6 new facts are
  `bound`-class like all others.

## Artifacts

- `measurements/plan10/`: `README.md` (provenance + command table),
  `assemble_census.py`, `census_armA.json` / `census_armB.json`,
  `ledger_snapshot_armA.json` (`9c060103…`) / `ledger_snapshot_armB.json`
  (`4e14f46f…`), `ledger_union.json` (`de690bc1…`), `flip_analysis.json` /
  `flip_prerun.json`, `verdicts.json`, `env.json`; `probe/` = the pilot
  (falsified pre-registration, header amended). Raw transcripts (`/tmp/plan10/`)
  are regenerable and uncommitted per convention.
- Promoted: 6 shards + the 262-entry manifest → `docs/plans/proofdb/shards/`;
  grown DB + union ledger → `data/` (gitignored, digests above).
