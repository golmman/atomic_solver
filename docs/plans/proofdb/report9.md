# Report 9 — the standing shape's first batch: the fresh sweep's 25 facts exhaust its pool; the strict-growth ladder escalates the head (0 facts, plateau holds)

Executes `plan9.md`. All deliverables landed; all pre-registered gates
**pass**. Both arm expectations held: **arm A exceeded its 5–15-fact
forecast with 25 facts** (the arm-2 prior sharpened: the deepest tail is
the yielding territory, 9.7 % of jobs vs the screen's 4.3 %), and **arm B
confirmed ≈ 0 facts** on the completion head — the three `g1f3` defenses
censored at the pre-registered **2B rung** (exactly `2 × work_done`, the
D1 strict-growth fix working as designed), and the 26-job 8M
sub-plateau tier plus 32 deeper probes all censored. The §4.3 completion
branch did **not** fire (no defense decided, no row AND-complete); the
§4.6 flip analysis reports **0 flips** over the grown DB; the handover
contingency is **not triggered**. Docs + `examples/`-side code only; the
product solver (`src/`), DB schema, and spec are untouched; decision 10
("flips stay derived") stands.

## D1 — the strict-growth ladder (§2)

The plan-session discovery was real and the fix is in: plan8 §2's
monotone floor (`max(2^(k−1) × base, work_done)`) revisits a pass-1
4M-standing record at **exactly its accumulated work** — for `g1f3`'s
defenses, a byte-identical deterministic repeat of the 1B probe (~3B
pure waste per completion arm). `ladder_budget` now floors at
`2 × work_done` for every pass ≥ 1 (fresh pass-0 records stay at the
base — their work is 0 by construction, verified over the standing
ledger). One-line change + doc comments (plan9 §2 cited alongside plan8
§2) + test updates in `tests/proofdb.rs`:

- the monotone-rule expectations became strict-growth expectations
  (`(1, 100M)` → 200M, `(2, 20M)` → 40M — the old rule under-budgeted
  both);
- the **equal-work regression case** added: pass 1 at work = `2^(k−1) ×
  base` must yield `2 × work` (16M), not work (8M) — the defect case;
- the `g1f3` case pinned with the standing base: `(1, 1,000,000,060,
  4M)` → `2,000,000,120`;
- the integration test's sequence budgets updated (`c2c4` 200M, `d2d4`
  8M = `max(8M rung, 2 × 4M)` — plan8's behavior preserved where the old
  floor was strictly exceeded).

No other policy code changed; `breadth-pns` and the censor hook are
untouched. `make test` green (7 and-close tests pass), clippy/fmt/doc
clean. `examples/proofdb/and_close.rs` grew to ~10.1 KB (doc-comment
growth; size justification added to the module header per convention).

## The arms (root = startpos; staging copies; no mid-run tuning)

| arm | config | jobs | facts | censored | evals | stop |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| A — fresh sweep | `fresh`, base 4M, TT 128 MB, cap 1.1B | 257 | **25** | 232 | 931,601,887 | **exhausted** |
| B — completion head | `completion`, base 4M, TT 1024 MB, cap 6.5B | 56 | **0** | 56 | 6,504,002,872 | budget |

**Arm A** swept the *entire* fresh pool (257 jobs — the 257
never-visited replies left by arm 2's ply-ascending screen; H1 verified
every budget 4M, every before-bump pass 0) in 218 s and stopped
`exhausted` — the 1.1B cap never fired. The 25 facts cost 3,598,470
evals total (~144k avg, max 2.7M) = **0.39 % of the arm's budget**;
the other 928M went into 232 censored screens. Distribution: 1 fact
each at plies 20/21/22/25, 3 at 24, 3 at 26, **15 at ply 27** — the
deepest class the arm-2 screen censored at its 3B cap is exactly the
yielding one. 17 facts are odd-ply jobs (an AND-row's reply refuted:
root player wins after it — AND-completion steps), 8 are even-ply jobs
(an OR-row's move refuted: the replier wins). All promoted rows are
root-player wins at bound 1–15 plies; all 25 shards replay-validated by
the merger.

**Arm B** ran exactly the pinned sequence: the census-expected head
`g1f3 d7d6`, `g1f3 e7e5`, `g1f3 f7f6` at **budgets 2,000,000,048 /
2,000,000,050 / 2,000,000,022 = precisely 2 × work_before** (the
strict-growth formula reading the standing ledger — the report8
pre-registered 2B rung reached without touching the base), each
censored at its full budget (~470 s wall each, ~4.2M evals/s); then the
26-job 8M tier (`e2e3` ×7, `e2e4` ×7, `g1h3` ×9, plus the 10-missing
row's first 3 replies), then 22 more 8M probes and the 5 pre-existing
pass-2 records at 24M until the 6.5B cap fired (56 jobs). Budget
classes: 3 × 2B, 48 × 8M, 5 × 24M. Every budget ≥ `2 × work_before`
(strict growth, H1-verified over all 56 records).

## The plateau climb's state (§4.4's first application)

The **factless-batch escalation policy** fired for the first time and
the two tiers took their pre-registered paths:

- **Fresh tier**: arm A *yielded* (25 facts) → the ×10 rule never
  fired, and it never will: arm A **exhausted the fresh pool**
  (`stop=exhausted`) — after this batch every missing reply of every
  active row is ledger-censored, so the and-close job set is pure
  censor tail. The standing shape's future batches are ladder climbs;
  the fresh-tier rule retains meaning only if new rows appear (they
  cannot: facts only prove replies, the pool only shrinks).
- **Censor tier**: the ladder escalates automatically.
  `g1f3`'s defenses now pass 2, work ≈ 3.0B each (1B measured + 2B this
  batch) → **next rung 6B** (2 × cumulative), then 12B — the
  pre-registered rung ladder 1B → 2B → 6B → 12B climbing toward the
  7–15B-equiv plateau band. The sub-plateau tier (`e2e3`/`e2e4`/`g1h3`
  and the deeper rows' replies) now pass 2, work ≈ 12M → next visit
  24M. Arm A's 232 new censors are pass 1 (work ≈ 4M) → next visit 8M.
  The standing ledger after the union: 8,446 records = 7,113 fresh
  (pass 0, work 0) + 1,274 pass-1 + 53 pass-2 + 6 pass-3.

Next-batch sizing (for plan10): the ~1,270 remaining pass-1 records at
their 8M rungs ≈ 10B evals ≈ ~40 min wall — or a targeted slice (the
completion order ranks rows by missing-count; the 12–37-censored-reply
rows above each fact-parent are the completion-critical ones). The
`g1f3` 6B rung (3 probes ≈ 18B evals ≈ ~70 min) is its own
pre-registered batch.

## The union advance + promotion (§4.5–4.6)

- **Promotion**: arm A's 25 validated shards + the grown manifest →
  `docs/plans/proofdb/shards/manifest.json` (231 → 256 entries, sha
  `9992dba4…`); every entry `validate: ok`.
- **Standing advance**: `data/proofdb_work.json` = union(armA_post,
  armB_post) via `proofdb_ledger_union` — the mechanism's second
  production use — `a696610b…`, 8,446 records. Inputs digest-pinned
  (`--expect`); the plan7 §2 gates re-run byte-level on the real inputs
  (reversed-order determinism, idempotence, N=1 normalization ×2 — all
  byte-identical). Arm B's 56 visits merged as pure pass upgrades (0 new
  paths); arm A's as 232 new censors (0 upgrades) — the sets disjoint by
  construction, as planned.
- **Merger**: 256/256 shards replay-validated; grown DB →
  `data/proofdb.db` (`6c724928…`, 41,459 nodes = +2,680; proven 41,384,
  open 75); all 256 manifest paths resolve in the grown DB.
- **Flip analysis (§4.6)**: `proofdb_flip` over the grown DB → **0
  flips**, root undecided (fixpoint 1 round); sanity: 0 flips over the
  pre-run standing DB. The near-miss is instructive: the 15-fact parent
  row stores 16/16 proven children, but the flip tool's fresh movegen
  finds **12 more legal replies** (ledger-censored at 4M, never stored)
  — no row reached AND-completeness. Every touched row still has
  12–37 censored replies outstanding. **The handover contingency is not
  triggered.**

## Gate verdicts

| gate | verdict |
| --- | --- |
| H1 integrity | **pass** — the §1 pins reproduced first as a committed probe (census 1,108 = 257 fresh + 851 censored, gradient head, exclusions 26/0/0, the fresh-order head at ply 19); per-arm session-start census re-pinned; armA: every budget 4M, every before-bump pass 0; armB: head exactly `g1f3 d7d6`/`g1f3 e7e5`/`g1f3 f7f6` at budgets == 2 × work_before, then the 26-job 8M tier, every budget ≥ 2 × work_before; no job path is a standing manifest path or standing DB row; **no-exposure check exact** (armA: +232 new censors, 0 bumps; armB: 56 bumps, 0 new paths) |
| H2 merge determinism | **pass** — merger ×2 (+1) over the grown manifest → 256/256 shards validated; DB `6c724928…` + dumps byte-identical across all runs |
| H3/H4 no-regression / spot-checks | **pass** — 25 facts, every shard replay-validated; no same-path outcome contradiction between arms (armB had 0 facts; armA's fact paths disjoint from every standing manifest path) |
| H5 policy determinism | **pass** — both arms replayed from fresh staging copies: job sequences identical (excl `wall_s`), post-run ledgers byte-identical — including armB's 6.5B-eval session (~26 min × 2, the plan8 precedent); **plus union determinism** on the real advance inputs, byte-level |
| H6 hygiene | **pass** — `make test` green (incl. the updated strict-growth ladder tests + the equal-work regression case), `cargo clippy --release --all-targets` 0 warnings, `cargo fmt --check` / `cargo doc` clean, no `src/` changes, new/edited example files ≤ 10 KB (`and_close.rs` ~10.1 KB with header justification), `git status` confirms `data/` ignored and the shards/ commit list is exactly the manifest + 25 shard bins |
| Flip-analysis consistency | **pass (vacuous)** — 0 flips over the grown DB and the pre-run standing DB; the tool's completeness check exercised for real (it correctly rejected the 15-fact parent's near-completeness: 12 censored replies outstanding) |

## Findings

1. **The strict-growth fix is confirmed by its target application**: the
   three `g1f3` probes ran at budgets *exactly* `2 × work_before`
   (2,000,000,048 = 2 × 1,000,000,024 etc.) — the report8 pre-registered
   2B rung reached with the standing base 4M, where plan8's formula
   would have re-run 1B probes byte-identically. The equal-work case is
   now structurally impossible and test-pinned.
2. **The fresh-tier economics sharpened, then closed**: arm A's yield
   rate (25/257 ≈ 9.7 %) is 2.3× the arm-2 screen's (34/784 ≈ 4.3 %),
   and the facts cluster at plies 24–27 — the screen's censor tail was
   hiding the cheapest remaining class. But the pool is now *empty*
   (`stop=exhausted`): the standing shape's next batches are ladder
   climbs only. The "cheap facts" class of the initiative is, for now,
   harvested out.
3. **The plateau band is confirmed from above a second time**: 6.5B
   child-evals across 56 completion-head probes, 0 facts; the three
   `g1f3` defenses now carry measured bounds ≥ 3B each (1B + 2B
   cumulative). The rung ladder (6B → 12B) sits exactly at the band's
   lower edge — the climb's next two rungs may suffice or may confirm
   the upper half.
4. **AND-completeness needs the censor tail, not just the fresh tail**:
   the 15-fact parent's 16/16-proven-children state (with 12 censored
   replies outstanding) demonstrates that completing a row requires
   refuting *every* censored reply of it — completion-critical rows are
   those with few censored replies remaining, and the completion order
   (missing-count asc) does not rank by that. A future plan may want a
   "completion-critical" order (rank rows by censored-replies-outstanding
   asc); noted as a candidate, not a scope change here.
5. **The job pool is now finite and ladder-bound** (1,333 censored
   records, strictly growing budgets): each record's next-visit budget
   is pinned by the ladder; the total remaining work to exhaust the
   current rung set is ~1,270 × 8M + 53 × 24M + 6 × (56–72M) + 3 × 6B ≈
   29B evals ≈ ~2 h wall. The standing shape is approaching the point
   where the DB's job set is fully enumerated at a measured bound.

## Problems encountered

- The session-start manifest snapshot had to be taken from `git show
  HEAD:` (the working-copy manifest was already the promoted 256-entry
  one by the time the driver ran); the driver reads the snapshot for the
  "no job is a manifest path" check. Noted in the measurement README.
- Two of my ad-hoc probe scripts had string/list handling bugs (the job
  record's `path` is a space-joined string, not a list) that produced
  one false H1 alarm ("job is a manifest path": actually the *post-run*
  manifest containing arm A's own 25 facts) and one false "censored
  kids 0" (a wrong token-strip). Both were re-run correctly and pass;
  the false alarms cost ~15 min of investigation, no data impact. The
  flip tool's 0-flips verdict — initially suspicious against the
  16/16-proven-children parent — was independently confirmed by the
  corrected ledger count (12 censored replies outstanding).
- `/usr/bin/time` still unavailable (plan8 nit); shell wall times used.

## Unresolved / next steps

1. **Next batch (plan10, pre-register then run)**: the ladder's cheapest
   tier first — the ~1,270 pass-1 records at their 8M rungs (completion
   order; cap ~10B; the yield prior is low but nonzero, and the
   no-exposure signature keeps the cost pure bookkeeping) — plus, as its
   own batch, the `g1f3` 6B rung (3 probes ≈ 18B evals ≈ ~70 min).
   The plan8 §4.3 ordering rule (cheap arm first) applies.
2. **Completion-critical ordering** (finding 4): a candidate
   `--and-close-order` value ranking rows by censored-replies-outstanding
   asc before the next big ladder spend; small D1 (one order arm),
   pre-registered separately.
3. Items 3 (DTM-upgrade pass), 5 (website handoff), 6 (parallel
   harvesters; base-stamp prerequisite), 7 (subtree scoping) unchanged.
   `make test-full` not required by this session (no `src/` or search
   changes) but stands before any release.

## Tools used

`cargo build/test/clippy/fmt/doc` (release), `python3` (census assembly +
gate checks), the `proofdb_harvest`/`proofdb_ledger_union`/
`proofdb_merge`/`proofdb_flip` binaries, `git` (read-only), `sqlite3`
(via python, read-only row checks). All runs against staging copies in
`/tmp/plan9/`; the standing `data/` layer was touched only by the
pre-registered promotion/advance (§4.5); the transcripts live in
`/tmp/plan9/` (regenerable per the README command table; the parsed
census is the record).

SESSION COMPLETE
- `report9.md` written; `measurements/plan9/` complete (2 arm censuses
  with all 313 job records, 2 post-run ledger snapshots, the union
  ledger, flip_analysis.json + pre-run sanity, verdicts.json, env.json,
  README, assemble driver — all H1/H2/H3/H4/H5/union/H6/flip gates
  asserted green in the driver). D1 strict-growth ladder implemented +
  tested (equal-work regression case pinned; H6 pass). **25 facts
  promoted**; standing layer advanced: DB `6c724928…` (41,459 nodes),
  manifest `9992dba4…` (256 entries), ledger `a696610b…` (8,446
  records). **The completion branch did not fire** (0 flips, root
  undecided, no handover); arm A exhausted the fresh pool (stop=
  exhausted), arm B confirmed the plateau (0 facts in 6.5B evals; g1f3
  bounds ≥ 3B, next rung 6B).
Follow-up options:
1. Kickoff prompt: "Draft plan10 for the proofdb initiative (plan
   session): the ladder's next batch — size the pass-1 8M-rung tier
   (~1,270 × 8M ≈ 10B evals, completion order) against the g1f3 6B rung
   (3 × 6B ≈ 18B evals); consider the completion-critical order
   candidate (rank rows by censored-replies-outstanding asc) as a small
   D1; pre-register census pins with committed probe output."
2. Alternative: close the standing shape for a while and pull item 6's
   base-stamp field or item 3's DTM-upgrade pass — worthwhile only if
   the ladder's ~29B-eval rung set is deferred; the plateau climb has
   priority while compute is available.
