# Plan 10: the ladder's first full sweep — the tier rungs (8M/24M/72M) and the `g1f3` 6B rung

Initiative: `proofdb`. Executes report9's next-step 1 (the ladder's next
batch, cheap tier first) and resolves its next-step 2 (the
completion-critical-order candidate — resolved as subsumed, §4.4). The plan
is a **two-arm batch** over the standing shape:

1. **Arm A — the tier sweep** (`completion` order + the new
   `--and-close-max-budget` filter at 100M): the 1,080 censored replies at
   their 8M/24M/72M rungs — the completion work's bulk (every active row's
   remaining missing replies);
2. **Arm B — the `g1f3` 6B rung** (`--max-jobs 3`, TT 1024 MB): the three
   head defenses at `2 × work_done ≈ 6B` each — the first rung of the climb
   not measured at the climb's TT size (the pilot's 6B censors ran at 128 MB,
   §1.3).

A plan-session census probe unintentionally executed the **entire batch** on
throwaway staging copies at the true budgets (§1.3 — the `--budget-evals
1000` trick met the strict-growth floor) and measured it end-to-end: 27.7B
evals, 3 facts, 1,080 censors, bookkeeping exact. The pilot is committed
under `measurements/plan10/probe/` and serves as the **pre-registered
cross-run determinism baseline** (§4.1): arm A's official run must reproduce
the pilot's 1,080 tail job lines byte-for-byte, and the batch's union ledger
advance must reproduce the pilot's post-run ledger (§4.6). The official run
is the batch's canonical execution — gates, promotion, and the report belong
to the execution session.

Docs + `examples/`-side code only; the product solver (`src/`), DB schema,
and spec are untouched; decision 10 ("flips stay derived") stands; the
handover contingency (plan8 §4.6, carried verbatim in §4.7) is unchanged.
Per repo convention the final task is `report10.md`.

## 1. Background and pinned state (committed probe output)

**Standing layer after plan9 (verified 2026-10-01, digests recomputed; see
§1.3 for the plan-session incident and its restoration).**
`data/proofdb.db` sha256 `6c724928603a28c35aeb68736ae2021d16cb32c2817be9a0ef309b66afb294d4`
(41,459 nodes), manifest `docs/plans/proofdb/shards/manifest.json` sha256
`9992dba4e4abc800a90d90a954f85d073766d38ae8383ca91d2f2982a72fd916`
(256 entries), `data/proofdb_work.json` sha256
`a696610bbf4c6d6b530088652e20e3279a51ef17fbc6a900d49c6222aac77590`
(8,446 records = 7,113 fresh + 1,274 pass-1 + 53 pass-2 + 6 pass-3).

**Committed census pin** (probe on a throwaway staging copy of exactly these
digests, 2026-10-01, stop-file census, verbatim lines):

```
harvest: db proofdb.db (41459 nodes, built_from 9992dba4e4abc800) — policy and-close over frontier C1 75 / C2 689878 / C3 1730 (AND-checks 20738)
and-close: active rows 49, replies 1083 (fresh 0, ledger-censored 1083); order completion; base budget 4000000
and-close-gradient: g1f3:3 e2e3:7 e2e4:7 g1h3:9 a2a3 a7a6 b2b3 a6a5 c2c3 a5a4 d2d3 a4b3 e2e3:10 d2d4 a7a6 a2a3 a6a5 b2b3 a5a4 c2c3 a4b3 e2e3:11 d2d4 d7d5 a2a3 a7a6 b2b3 a6a5 c2c3 a5a4 e2e3 a4b3 f2f3 b7b6 g2g3 b6b5 h2h3 b5b4 a3a4 b4b3 c3c4 b3b2 e3e4 b2a1q f3f4 d5c4 g3g4 c7c6:12 root:13 a2a3 a7a6 b2b3 a6a5 c2c3 a5a4:17 d2d4:18
and-close-excluded: open rows 75 (active 49); proven-ancestor 26, implied-win 0, implied-loss 0
```

(`order fresh` prints the identical census and stops `exhausted` with 0 jobs —
**the fresh pool is empty**, arm A(plan9)'s 257-job sweep drained it. Every
missing reply of every active row is ledger-censored; this resolves report9's
finding 4 / unresolved 2 — see §4.4.)

Notes:

- The job set is **1,083, all ledger-censored** (0 fresh). 1,333 pass-≥1
  ledger records exist; 250 are non-jobs (children of excluded rows or
  non-reply paths). The three `g1f3` defenses sit at the gradient head
  (missing 3 < e2e3's 7).
- Frontier C2/C3 moved from plan9 §1's 641,521/1,755 because the DB grew by
  promotion; C1/exclusions are unchanged. The current numbers are the pins.

**§1.3 — the pilot (plan-session finding, owned transparently).** The plan
session's census probe passed `--budget-evals 1000` intending near-free
1,000-eval jobs, with the per-job budget echoes doubling as the tier table.
The strict-growth floor made `2 × work_done` dominate for every pass-≥1
record, so all 1,083 jobs ran at their **true base-4M budgets** — the probe
became a full-cost run of the whole batch (27,695,668,981 child-evals,
~98 min wall ≈ 4.7M evals/s). Verified afterwards: **0 budget divergences**
between the probe's budgets and the base-4M formula over all 1,083 jobs, so
the pilot's per-job data is exactly what a base-4M run produces. The pilot's
stdout (`job:` records), post-run ledger, and the 3 fact shards are committed
under `measurements/plan10/probe/` (digests in its README).

**Incident and restoration.** The probe passed the *standing manifest path*
as `--manifest`; the session's `rewrite_manifest` added its 3 fact entries to
the standing manifest (259 entries, digest `af49fce3…`) before the plan
session noticed. The DB and ledger were never touched (the harvest CLI does
not write them); the standing manifest was restored from `git show HEAD:`
back to the 256-entry `9992dba4…` state and all three standing digests were
re-verified (§1's values). **Lesson (normative for future probes): a probe
must copy the manifest alongside the DB and ledger** — report8 finding 1's
"probes commit their pins" now extends to *all three* mutable files the
harvest CLI touches. The pilot's 3 shards were saved to
`measurements/plan10/probe/probe_shards/` before restoration; they are
deterministic (`tag_for_path` is content-hashed; the export pipeline is
deterministic given the TT size) and arm A's official run must re-derive them
byte-identically (§5 H2/H3).

**The pilot's tier table** (probe budgets ≡ base-4M budgets; `pass` =
visit number = ledger `passes_failed` + 1; censors consume the full budget):

| tier | jobs | facts | censored | child-evals |
| --- | ---: | ---: | ---: | ---: |
| 6B (`g1f3` ×3, TT 128 MB) | 3 | 0 | 3 | 18,000,000,519 |
| 24M | 48 | 2 | 46 | 1,122,935,498 |
| 72M | 5 | 0 | 5 | 360,001,245 |
| 8M | 1,027 | 1 | 1,026 | 8,212,731,719 |
| **total** | **1,083** | **3** | **1,080** | **27,695,668,981** |

Head probes pinned (`path | budget | outcome | pass | work_before`):
`g1f3 d7d6 | 6000000190 | censored | 3 | 3000000095`, `g1f3 e7e5 |
6000000194 | censored | 3 | 3000000097`, `g1f3 f7f6 | 6000000102 | censored |
3 | 3000000051` — each censored at exactly `2 × work_before` (the
strict-growth floor working; canonical digest `3e7cf3c7…` in the probe
README). The three defenses now carry measured bounds ≥ 6B each *at TT
128 MB*; the climb's TT-size question is arm B's reason to exist (§4.2).

**The pilot's 3 facts** (all `validate: ok`; deterministic re-derivation
required by H2/H3):

| path (reply bolded by position) | plies | side to move at job | outcome | effect |
| --- | ---: | --- | --- | --- |
| `a2a3 a7a6 b2b3 a6a5 c2c3 a5a4 d2d3 a4b3 e2e3` + **`g7g6`** | 10 | White | `win` | reply refuted (AND-completion step); row `…e2e3` missing 10 → 8 |
| `a2a3 a7a6 b2b3 a6a5 c2c3 a5a4 d2d3 a4b3 e2e3` + **`h7h5`** | 10 | White | `win` | reply refuted; same row (→ 8) |
| `d2d4 d7d5 a2a3 a7a6 b2b3 a6a5 c2c3 a5a4 e2e3 a4b3 f2f3 b7b6 g2g3 b6b5 h2h3 b5b4 a3a4 b4b3 c3c4 b3b2 e3e4 b2a1q f3f4 d5c4 g3g4 c7c6` + **`e1e2`** | 27 | Black | `win` | the OR-row's candidate `e1e2` refuted (missing 12 → 11) |

Outcomes are side-to-move (spec §schema): the two 10-ply facts are White-win
subtrees (the replier's reply is refuted); the 27-ply `e1e2` fact is a
**Black-win subtree — the DB's first opponent-win (loss-side) fact class**: it
refutes a candidate move of an even-ply OR-row and documents a root-loss line
after `e1e2`. It does **not** decide the row (a candidate elimination, not a
completion) and implies nothing upward by itself; the flip analysis verifies
every implication recursively (§4.7). The proven-outcome tripwire covers this
class like any other (a same-path contradiction aborts the merge).

**Bookkeeping verification (pilot).** For every one of the 1,083 jobs:
censored ⇒ post `work_done = work_before + child_evals` and `passes_failed =
pass`; decisive ⇒ the ledger record is left in place. Verified exact; the
post-run ledger is 8,446 records (bumps in place, 0 new records), sha
`d0653c3cff0d2807fb76c33077385e721a9ded858b4d83790ee650a7ee6f2e72` (committed
as `measurements/plan10/probe/post_ledger.json`).

## 2. Deliverables

- **D1 — the budget filter** (§3.1): `--and-close-max-budget <evals>` (0 =
  unlimited, the default and today's behavior) + the `and-close-filter:` echo
  line + tests. ~20 lines in `and_close/driver.rs` (6.5 KB, room) +
  `harvest_args.rs` (4.2 KB); `and_close.rs` untouched (10.1 KB stays).
- **D2 — the measurement** under `measurements/plan10/`: per-arm job lines,
  post-run ledger snapshots, the union ledger (the standing advance),
  `flip_analysis.json`, `env.json`, `verdicts.json`, `README.md` (provenance,
  command table, verdicts), and the assemble driver. The plan-session probe
  artifacts (`probe/`) predate D2 and stay as committed inputs.
- **D3 — `report10.md`**: arm verdicts, the ladder's state after the sweep
  (next-rung table), the `g1f3` climb's state (bounds ≥ 6B at 128 MB, ≥ 6B or
  decided at 1024 MB), the first loss-side fact class, gate verdicts,
  findings, and the handover assessment if triggered.

## 3. Pre-registered arms and decisions (fixed before any run)

Both arms run on **fresh staging copies** of the standing state (§1 digests
verified first), root = startpos, `first_outcome_only` on, heavy tier
disabled (`--heavy-sample 0`), manifest/DB/ledger all copied per §1.3's
lesson. Facts are arm-independent truth and promote after the verdict; a
same-path outcome contradiction between arms (or against the standing layer)
aborts the session (the standing tripwire). Arm A runs first (cheap), then
arm B — the plan8 §4.3 ordering rule.

1. **Arm A — the tier sweep.** `--policy and-close --and-close-order
   completion --and-close-max-budget 100000000 --budget-evals 4000000
   --max-total-evals 10500000000` (TT 128 MB = the pilot's config). Census
   expectation (H1): the session-start census equals §1's pin verbatim (base
   budget 4000000), plus the new echo `and-close-filter: max-budget 100000000,
   jobs 1080 of 1083`. The 100M threshold keeps every tier job (the largest
   tier budget is 72,000,330; nothing exists between 72M and the head's 6B)
   and drops exactly the three head probes. Expectation: **exactly the
   pilot's outcome** — 3 facts (the §1.3 table's three paths, byte-identical
   shards), 1,077 censors, ≈ 9,695,668,462 child-evals, stop `exhausted` (the
   cap is headroom; decisive jobs cost less). Any deviation from the pilot's
   tail job lines is a determinism defect: stop and investigate (§4.1).
   *(Falsified 2026-10-02: the official run found 6 facts / 1,074 censors —
   §9 re-scopes the expectation and the baseline.)*
2. **Arm B — the `g1f3` 6B rung.** `--policy and-close --and-close-order
   completion --max-jobs 3 --budget-evals 4000000 --max-total-evals
   18500000000 --tt-mb 1024`. Census expectation (H1): the §1 pin verbatim,
   **no filter echo** (max-budget unset = unlimited); the sequence head is
   exactly the §1.3 head pins (three probes at budgets 6,000,000,190/194/102,
   visit 3, work_before ≈ 3.0B). The run is the first ≥ 2B climb measurement
   at the climb's TT size (1024 MB, the plan8/9 precedent): the pilot's 6B
   censors were 128 MB runs, valid but config-mismatched bounds. Expectation
   (the band prior): ≈ 0 facts, each probe censored at ≈ its 6B budget
   (`child_evals` within [6.0B, 6.1B]; not byte-pinned — §4.2), work after ≈
   9.0B each. **0 facts is a valid measured result** (the band's lower edge
   is 7–15B-equiv); each censor advances the defenses' cumulative work to
   ≈ 9B → the next rung is **18B** (§4.3).
3. **The completion branch** (plan8 §4.6 via plan9 §4.3, verbatim): if any
   `g1f3` defense decides `'win'` and the row's replies are all proven
   root-player-wins, `g1f3` is AND-complete → the flip analysis reports the
   implied outcome + bound and the **handover assessment fires**; a reply
   deciding `'loss'` flips its row to `'win'` and implies the root loss —
   reported the same way. Flips stay derived (decision 10); the conservative
   handover trigger (all root children resolved by facts) is unchanged.
4. **The plan9 prose-ladder correction (normative note).** Plan9 §2/§4.4
   pinned the rung ladder "1B → 2B → 6B → 12B"; the strict-growth formula
   (the code is normative) gives after the 6B rung: work = 3B + 6B = 9B →
   next budget `max(2³ × base, 2 × 9B) = 18B`. The ladder is **1B → 2B → 6B
   → 18B**; plan9's "12B" was a prose slip (2 × the last rung, not 2 × the
   cumulative work). This plan adopts 18B as the pre-registered next rung.
5. **The completion-critical-order candidate is subsumed (report9 finding 4
   resolved, measured).** With the fresh pool empty, `missing_count ==
   censored-replies-outstanding` for every active row, so a
   censored-replies-ascending order is **identical to `completion`** (the
   census pin proves it: fresh 0). No new order ships; report10 records the
   negative result. If a future batch re-introduces fresh rows (it cannot
   under the standing shape — facts only shrink the pool), the candidate may
   be re-examined.
6. **Promotion and the ledger advance.** Validated facts from both arms
   promote into the standing shard set + manifest; the standing
   `data/proofdb_work.json` advances to **union(armA_post, armB_post)** via
   `proofdb_ledger_union` (inputs digest-pinned by `--expect`; the plan7 §2
   union gates re-run byte-level on the real inputs — the mechanism's third
   production use, N=2). The arms' job sets are disjoint by construction
   (A: budget ≤ 100M; B: the completion head at 6B). Record growth = 0
   (all 1,080 bumped records already exist); the no-exposure signature
   becomes "**0 new records, bumps only**" (A: 1,077; B: 3, if all censor).
7. **The flip analysis** (plan8 §4.5, verbatim): `proofdb_flip` over the
   grown DB after promotion; every reported flip independently verified by
   the tool's recursive verifier; any implied-decided **root** fires the
   handover assessment (§4.3's conservative trigger unchanged). Expected:
   the two `…e2e3` facts move their row to missing 8 (no completion — 8
   censored replies outstanding), the `e1e2` fact removes one OR-row
   candidate (missing 11); **0 flips is the prior**, the tool's verdict is
   the result.

## 4. The cross-run determinism contract (this batch's H5, pre-registered)

1. **Arm A vs the pilot (byte-exact; falsified 2026-10-02 — re-scoped in §9).** Arm A's emitted `job:` lines must
   match the pilot's 1,080 tail lines on `(path, budget, child_evals,
   outcome, exit_reason, pass, work_before)` — the canonical digest
   `f8cb42777b1b94c1a9bb7d24d183fbd545e557c888744a09fa04f921419e25ac` over
   the lines `path|budget|child_evals|outcome|exit_reason|pass|work_before`
   in sequence order (the pilot's order; `wall_s` excluded). This replaces
   the usual in-session double run with a **stronger cross-session
   baseline** (independent staging copies, independent sessions, identical
   results) and is only sound because arm A's config is pilot-matched
   (TT 128 MB, base 4M, identical sequence). A mismatch is a determinism
   defect: stop and investigate — never proceed to promotion.
2. **Arm B (in-session H5 replay, full).** No pilot baseline exists at
   TT 1024 MB, so arm B carries the normative per-arm replay (same staging
   input → identical job sequence, byte-identical post-run ledger) — the
   plan8/9 precedent, ~63–70 min extra accepted. `child_evals` are pinned
   only within the censor range; the byte-identity applies to the ledger
   and the non-`child_evals` line fields.
3. **The union vs the pilot's post ledger.** union(armA_post, armB_post)
   must byte-equal `measurements/plan10/probe/post_ledger.json` on all
   8,443 non-head records; the three head records compare on
   `passes_failed` (exactly 3) and `work_done` (range [9.0e9, 9.1e9] if all
   censor — the 128 MB→1024 MB overshoot delta is the only allowed
   difference). If arm B yields f head facts, those f records instead
   compare equal to arm B's own post state (decisive jobs leave their
   records untouched) and report10 documents the branch.
   *(Falsified 2026-10-02 — re-scoped in §9: the union target is the arms'
   own post ledgers, digest-pinned via `--expect`.)*

## 5. Pre-registered gates (fixed before any run)

- **H1 integrity:** per arm, the session-start census matches §1's committed
  pin (job set 1,083 = 0 fresh + 1,083 censored; active rows 49; the gradient
  head string; exclusions 26/0/0; the standing digests; the frontier line).
  Arm A: the filter echo exactly `jobs 1080 of 1083`; every job budget ≤
  100M; the byte-exact pilot match (§4.1) — 3 facts at the pinned paths,
  1,077 censors. *(Re-scoped §9: in-session replay instead of the pilot
  match; 6 facts / 1,074 censors.)* Arm B: the sequence head exactly the §1.3 head pins (no
  filter echo); `--max-jobs 3` stops the arm; every job path replays
  legally; no job is a standing manifest path. **No-exposure check:** arm A's
  post ledger = standing + 1,077 bumps (0 new records, 0 fresh touches);
  *(re-scoped §9: arm A = 1,074 bumps — its 6 decisive records untouched;*
  *0 new records confirmed on the official run.)*
  arm B's = standing + 3 bumps (0 new records); ledger growth in *records*
  is 0 for both.
- **H2 merge determinism:** merger re-run twice over the grown manifest →
  byte-identical DB + dump; 259/259 shards replay-validated (256 standing +
  arm A's 3; arm B's facts, if any, add). If arm B yields 0 facts (the
  prior), the grown manifest must **byte-equal arm A's own grown manifest**
  `e91ad57b44008fee65ab0b124fab51365c17d08f891777cde525cfecb61fb5fd` (262
  entries — the promotion is deterministic) *(re-scoped §9; the pilot's
  259-entry `af49fce3…` target is falsified)*; a mismatch is a defect: stop.
  If arm B yields facts, the manifest is the 262-entry one plus arm B's
  entries, all `validate: ok`, and report10 documents the branch.
- **H3/H4 no-regression / spot-checks:** the 6 promoted facts'
  shard bins are validated by the merger's replay validator and pinned to
  arm A run 1's bytes via the §9 replay (the pilot byte-equality is
  falsified — §9) *(re-scoped)*; no same-path outcome
  contradiction (the three fact paths are not standing manifest paths —
  pinned); plan4's H3/H4 run over promoted facts.
- **H5 policy determinism:** §4's three-part contract (arm A cross-run
  byte-exact; arm B full in-session replay; the union vs the pilot's post
  ledger with the head-records caveat) + the union tool's own gates
  (reversed-order determinism, idempotence, N-way normalization) on the real
  inputs.
- **H6 hygiene:** `make test` green (incl. the new filter tests);
  `cargo clippy --release --all-targets` / `cargo fmt --check` / `cargo doc`
  clean; no `src/` changes; new/edited example files ≤ ~10 KB with the
  existing header justification where applicable; `git status` confirms
  `data/` ignored.
- **Flip-analysis consistency:** plan8's gate verbatim (AND-completeness
  mandatory; a flip claim without it is a defect — stop).

A gate failure is a defect in the tool or this plan's model — stop and
investigate, do not loosen the gate.

## 6. Non-goals

No `src/` changes; no schema/spec change (v1 stands; flips stay derived); no
pns-machinery changes; no `fresh`-order work (the pool is empty — pinned); no
parallel harvesters (item 6); no subtree scoping (item 7); no website handoff
(item 5); no DTM-upgrade pass (item 3); no next-rung spends beyond this
batch's arms (the 18B head rung and the second-visit tier sweep are plan11
material, pre-registered there); no materialization of flips; no
in-session re-propagation; no ladder *schedule* changes beyond the filter
knob (plan6 §4.5's no-go stands at its budgets); no re-litigation of the
pilot's outcome (it is the baseline, not a hypothesis).

## 7. Tasks

1. Verify the preconditions: the §1 standing digests recomputed; the census
   probe reproduced on a fresh staging copy (all files copied, §1.3's
   lesson) and matched to the pin.
2. Implement D1 (the filter + echo + tests); run H6.
3. Run arm A on fresh staging copies; assert the §4.1 byte-exact pilot match
   (job lines + post ledger) — any mismatch stops the session.
4. Run arm B (TT 1024 MB, `--max-jobs 3`); full in-session replay per §4.2.
5. Check gates H1–H6; promote facts; the union ledger advance (§3.6) with
   the §4.3 comparison; merger runs (H2); the flip analysis (§3.7); assemble
   D2.
6. Write D3 (`report10.md`): the arm verdicts, the ladder's next-rung table
   (§8's outlook), the loss-side fact class, findings.

## 8. Budget

One session. Implementation: the filter + echo + tests ≈ 1 h. Compute: arm A
≈ 9.7B child-evals ≈ 35 min (no replay — §4.1's cross-run baseline replaces
it); arm B ≈ 18.0B ≈ 63–70 min + replay ≈ 63–70 min; union + merger + flip +
census ≈ 10 min. Total ≈ 3 h compute — the fattest batch yet, accepted once:
the pilot's data made the sizing decision with measurements, and the
remaining open question (the climb's TT-size sensitivity) is arm B's alone.
Pre-registered fallback, decided now not mid-run: if the sitting's wall
budget threatens after arm B's run, the replay may be deferred to plan11's
pre-flight probe (which re-runs the arm on fresh staging copies and
byte-compares) and report10 records the deferral explicitly; arm B's cap may
drop to 12B (two probes; the third joins the next batch) — the completion
question then stays open and report10 says so.

## 9. Amendment — 2026-10-02 (execution session): the pilot is a falsified pre-registration; arm A re-baselined by a full in-session replay

Arm A's official run (§7 task 3, executed 2026-10-02 on fresh staging copies at
`/tmp/plan10/armA`) **falsified §3.1's expectation and §4.1's cross-run
baseline**: 1,080 jobs, **6 facts / 1,074 censors**, 9,693,439,558 child-evals,
stop `exhausted`, wall 2097.7 s — not the pilot's 3/1,077. The session-start
census matched §5 H1 exactly (the §1 pin verbatim + the filter echo
`and-close-filter: max-budget 100000000, jobs 1080 of 1083`), and the
no-exposure check held (0 new records, 0 gone; 1,074 bumps, all `passes_failed`
+1; the 6 decisive records untouched). Only the *within-job* search diverged:
`child_evals` differ on 1,053/1,080 shared jobs (same budgets, passes,
`work_before`, same sequence order), and 3 censored jobs flipped to wins.

**Root cause (measured).** The pilot and arm A are not sequence-matched in the
decisive respect: §1.3's premise "arm A's config is pilot-matched … identical
sequence" is false. The harvest session retains one private TT across jobs
(`examples/proofdb/session.rs` module doc); the pilot ran all 1,083 jobs in one
session, so its three 6B head probes ran *first* and warmed the 128 MB TT for
the 1,080 tier jobs, while arm A drops those probes and runs the tier jobs
under a different TT history. TT state changes DF-PN+'s within-job work and,
at the margin, its outcome. Cross-checks: two fresh-staging 30-job prefix runs
on independent builds are byte-identical (job lines excl `wall_s` + post-run
ledger; LTO vs no-LTO byte-identical — `det1`/`det2`/`nolto`), and a single-job
cold run of the first diverging job (`e2e3 c7c6`, 24M budget) reproduces arm
A's `child_evals` (24,000,080), not the pilot's (24,000,085) (`skip1`) — the
pilot's values are reachable only from a warmed TT. Arm A is deterministic and
sound; the pilot's *outcome* predictions are not.

The falsification also invalidates the byte-equality half of H2/H3: arm A's
three shared-path shards carry the pilot's *names* (path-hashed tags) but
different *bytes* — the proof-subtree export depends on the job's TT snapshot.
Fact sizes equal for two of them (8,438 / 8,594 B), the 27-ply one differs
(28,642 vs 29,710 B).

**Re-scope (fixed before the replay and arm B):**

- **§4.1 (re-scoped):** arm A's determinism gate is a **full in-session replay
  per §4.2** — the same staging input re-run from scratch; job lines byte-exact
  excl `wall_s`, post-run ledger byte-identical, the 6 fact shards
  byte-identical, grown manifest byte-identical to run 1. The pilot's canonical
  digests (`f8cb4277…`, `47892f84…`, `5df15da5…`) are falsified baselines.
- **§3.1 (re-scoped expectation):** arm A = 6 facts / 1,074 censors — 24M tier:
  2 facts (the pilot's two `…e2e3` wins); 8M tier: 4 facts (the pilot's `e1e2`
  win plus three new: `d2d4 a7a6 a2a3 a6a5 b2b3 a5a4 c2c3 a4b3 e2e3 g7g6`,
  `…e3e4 b2a1q f3f4 b8d7`, `…h2h3 b5b4 a3a4 a8a4`); 72M tier: 0. All 6 are
  White-win refutations of reply/candidate moves (the `e1e2`/`b8d7`/`a8a4`
  class remains loss-side-documented as in §1.3).
- **§4.3 (re-scoped):** the union target is the arms' own post ledgers
  (arm A's sha `9c060103bd23038a5bd270eb2399494ce6208c6e1b3f16e53d2ed11d91d846e1`,
  arm B's pinned when run), digest-pinned via `--expect`; the pilot's
  `d0653c3c…` post ledger is falsified as a comparison target (the §4.3
  head-record caveat is moot).
- **H2 (re-scoped):** the grown manifest = 256 standing + 6 arm A facts =
  262 entries; byte-equality target is arm A run 1's manifest (`e91ad57b…`);
  262/262 shards replay-validated.
- **H3 (re-scoped):** pilot shard byte-equality is dropped; the 6 promoted
  shards are validated by the merger's replay validator and pinned to arm A
  run 1's bytes via the replay.
- **Arm B unchanged** (§3.2, §4.2 full replay) — its run/replay share the
  identical 3-job sequence and TT history, so the comparison is sound.
- The pilot artifacts under `measurements/plan10/probe/` stay committed,
  re-labeled a **falsified pre-registration**: its per-job *budget*
  bookkeeping remains valid (0 budget divergences, §1.3), its outcome and
  child-eval predictions do not.
- **§8 budget impact:** + ~35 min (the arm A replay the pilot baseline was
  supposed to replace), total ≈ 3.5 h — accepted.

**Next-rung outlook (for report10, not this batch):** after the sweep, the
ladder state is 1,026 records at pass 2/work 12M (next 24M ≈ 24.6B), 46 at
pass 3/work 36M (next 72M ≈ 3.3B), 5 at pass 4/work 108M (next 216M ≈
1.08B), and the `g1f3` defenses at pass 3/work ≈ 9B (next 18B ≈ 54B) — a
next full sweep ≈ 83B evals ≈ 5 h run. The escalation is ~3× per rung; the
informative rungs are now the `g1f3` 18B (≥ the band's upper edge — either
it decides or the band estimate is falsified) and the OR-row/AND-row
completions the tier sweep feeds. Plan11 sizes that trade-off with the
measured 0.28 %-per-rung fact yield (3/1,083) as the prior.

## History

- **2026-10-02 — amended (execution session): §9 added.** Arm A's official run
  falsified the pilot pre-registration (6 facts / 1,074 censors vs 3/1,077;
  root cause: the session's cross-job private TT — the pilot's head probes
  warmed it, arm A drops them). §3.1/§4.1/§4.3/H1/H2/H3 re-scoped in place:
  arm A re-baselined by a full in-session replay per §4.2, the union target
  moved to the arms' own post ledgers, the manifest target to arm A run 1's
  262 entries; arm B unchanged. The pilot artifacts stay committed as a
  falsified pre-registration.

- **2026-10-01 — drafted (docs-only plan session).** The census probe
  unintentionally ran the full batch at true budgets (the strict-growth
  floor vs the `--budget-evals 1000` trick, §1.3) — owned as the pilot; the
  standing manifest was restored after the probe's in-place rewrite (§1.3's
  lesson: probes copy all three mutable files). The pilot's artifacts
  committed under `measurements/plan10/probe/` (README with digests). See
  §1–§8.

## SESSION COMPLETE

- `docs/plans/proofdb/plan10.md` drafted (plan session, docs-only; probes on
  throwaway `/tmp` copies, artifacts committed in
  `measurements/plan10/probe/`). Standing digests re-verified after the
  §1.3 incident + restoration (DB `6c724928…`, manifest `9992dba4…`, ledger
  `a696610b…`); census pinned 1,083 = 0 fresh + 1,083 censored
  (fresh pool empty, `fresh` order stops `exhausted`); the pilot's tier
  table (6B ×3 censored at 128 MB / 24M ×48 → 2 facts / 72M ×5 / 8M ×1,027
  → 1 fact; 27.696B evals total, bookkeeping exact over all 1,083 records).
- Normative content: the two-arm batch (§3: the tier sweep ≤ 100M with the
  new `--and-close-max-budget` filter, cross-run-pinned to the pilot;
  the `g1f3` 6B rung at TT 1024 MB — the climb's first config-matched ≥ 2B
  measurement); the cross-run determinism contract (§4, replacing arm A's
  in-session replay with the pilot baseline + the union-vs-pilot gate); the
  plan9 prose-ladder correction (6B → 18B, §4.4/§3.4); report9's
  completion-critical-order candidate resolved as subsumed (§3.5); the
  first loss-side fact class documented (§1.3); the handover contingency
  verbatim.
- Note: the pilot spent ~98 min of container compute in a plan session —
  flagged in §1.3 as a process cost, with the probe-copy lesson recorded.
Follow-up options:
1. Kickoff prompt: "Execute docs/plans/proofdb/plan10.md: implement the
   --and-close-max-budget filter + echo + tests, run arm A (tier sweep ≤
   100M, TT 128 MB) asserting the byte-exact pilot baseline (3 facts,
   1,077 censors), run arm B (g1f3 6B rung, TT 1024 MB, --max-jobs 3) with
   its full replay, gates H1–H6 incl. the union-vs-pilot comparison,
   promote the 3 facts, advance the ledger, merge, run the flip analysis,
   report10.md with the next-rung outlook."
2. Alternative: skip plan10's arms and go straight to item 5 (website
   handoff) or item 6 (parallel harvesters) — not recommended: the batch is
   fully pinned and pre-validated by the pilot; deferring it leaves the 3
   facts un-promoted and the standing shape's ladder state unadvanced.
