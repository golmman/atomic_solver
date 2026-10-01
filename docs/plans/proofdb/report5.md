# Report 5 — batch 2: the unchanged-policy control; the ladder measured unreachable

Executes `plan5.md` (the batch-2 control + the §2 selection decision,
pre-registered before the run). All deliverables landed; all pre-registered
gates **pass**; all five expectations **confirmed**. The measured yield is
**0 facts at 300.0M child-evals — exactly the pre-registered expectation**
(E2). The plan's goal measurement (E3) came back as expected: **the ladder
never fired** — 75 visits, all at pass 1, all at ply 2 — converting plan5
Lemma 2's arithmetic into data. The contingency triggers (§4 decision 3)
are all clear, so **the §2 decision stands as pre-registered**: the
within-layer selection change is the per-layer budget split with a
scheduled ladder reserve, implemented in plan6 and A/B'd against this run
as its byte-match control.

## Deliverables

- **D1** `measurements/plan5/`: `census_breadth-pns.json` (75 job records +
  summary + the decision-5 census match + E1–E4 verdict records),
  `ledger_snapshot.json` (the post-batch escalation state, 3,033 records),
  `assemble_census.py` (driver + H1/E1–E5/H5 checks), `env.json`,
  `README.md` (provenance, command table, verdict table). No grown-DB copy:
  0 new shards and 0 facts → the post-batch DB is byte-identical to the
  input (sha256 `670e19e3…` confirmed after the batch; the harvest tool
  never writes the DB).
- **D2** this report. No code changed anywhere (the plan is
  no-code-change); the gate verifies that claim (H6).

## The batch (root = startpos; decision 1 budgets, no mid-run tuning)

| metric | value |
| --- | --- |
| session-start census | `pns: rows 35055 (open 75), ledger records 1434 (1434 new, 0 dropped as decided); exclusions proven-ancestor r26/l0, implied-win r0/l0, implied-loss r0/l0; jobs 1483 (rows 49, ledger 1434); base budget 4000000` — **byte-matches the §1-pinned line** (decision 5) |
| number split (measured from ledger + batch-1 census) | **1,408 number-1** (1,368 fresh ledger + 40 unvisited rows) / **75 number-2** (66 censored ledger + 9 censored rows) = 1,483 |
| jobs run | 75 — all class L (fresh ledger children), all censored (`BudgetExhausted`), 0 decisive |
| census triple | every job `(pass, number, work_before) = (1, 1, 0)` |
| visited plies | 2 only — 75 of the 289 fresh ply-2 records (**job ply sequence constant**, E1) |
| evals | 300,001,045 child-evals (cap 300M, between-jobs check) |
| wall time | 65.6 s (job-time sum 63.9 s, ≈ 4.7M evals/s) |
| post-batch ledger | 1,443 → **3,033 records**: 150 censored (all at `passes_failed` 1, ≈ 4.0M each — batch 1's 75 and batch 2's 75 first visits; **no node has yet earned a second visit**) + 2,883 fresh |

## Expectation verdicts (§4 decision 2)

| expectation | verdict | notes |
| --- | --- | --- |
| **E1** queue shape | **confirmed** | every visit from the ply-2 fresh sublayer; 75 < 289 available, so the cap fired before the sublayer drained; ply 3 never reached |
| **E2** yield ≈ 0 | **confirmed** | 0 facts / 300.0M — the ply-2 fresh pool (replies of quiet 4M-censored nodes) is yieldless at the base budget, consistent with plan2/plan3's "cheap facts at ply ≥ 4" |
| **E3** ladder never fires | **confirmed** (the goal measurement) | max pass = 1 across all 75 jobs; no rung at pass ≥ 2. A measured no is the success outcome |
| **E4** ledger growth | **confirmed** | 1,443 → 3,033 (accepted 2,600–3,200); 1,590 new records, **all at ply 3**; exactly 75 bumps (`passes_failed` 0→1); exposure **21.2** new number-1 records per censored visit (batch 1: 18.2) |
| **E5** determinism | **confirmed** | full replay from the same input DB + initial ledger: job records identical on every field except wall_s; identical job sequence; **byte-identical post-batch ledger** (sha256 `4f168105…` both); identical `pns:` census line |

**Contingency (decision 3): not triggered** — E3 not violated, E2 = 0 ≤ 10
facts. The §2 decision stands; no re-open.

## The measured verdicts (D2's required content)

**Ladder reachability: measured no.** The number-1 pool at session start
costs 1,408 × 4M ≈ 5.6B child-evals against the 300M cap; a factless
session affords 75 visits; each censored visit exposed 21.2 new number-1
records while removing exactly one node (itself). Under the unchanged
policy the ladder is not merely *unlikely* — it is structurally
unreachable: after the batch, the ledger holds 150 censored nodes all at
pass 1 and 2,883 fresh number-1 records, so the pool is larger than at the
session's start and the queue's next 75 pops are again all number-1 ply-2/3
records. This is the data version of Lemma 2 and the justification for the
§2 decision.

**Pool-growth rate (measured on batch 2).** Net number-1 pool change:
1,408 → ≈ 2,923 derived for the next session (fresh 2,883 + 40 unvisited
rows), i.e. **+20.2 per censored visit** (21.2 exposed − 1 visited). Next
session's census arithmetic: 3,073 jobs ≈ 2,923 number-1 + 150 number-2
(derived from the snapshot; not measured in-session). Drainage by facts
would need ≈ 2,900 decisive visits against 75 visits/session — the
monotone-growth premise holds.

**The ply-2 fresh pool's yield: 0.** The first direct data point on the
fresh ply-2 frontier: 75 visits at 4M each produced no fact and 21.2
children per visit. Combined with batch 1 (54 ply-2 jobs, also 0), the
ply-2 frontier is now 129/289 visited at ≈ 4M, all censored — a quiet
class, as pre-registered.

## Gate verdicts

| gate | verdict | notes |
| --- | --- | --- |
| H1 job/shard integrity | **pass** | decision-5 census match (the pinned §1 line reproduced byte-for-byte, incl. the corrected 1,483-job universe); lineage gate ran (0 dropped as decided, 0 illegal); exclusion census per reason; every job path is a fresh in-tree ledger child at ply 2 with the (1, 1, 0) triple (checked in `assemble_census.py`) |
| H2 merge-after-batch determinism | **pass** | two full merger runs over the unchanged manifest → DB `670e19e3…` and dump `e3060d14…` byte-identical both times and to the input DB (a real residue check on a censored session) |
| H3 no-regression vs input DB | **pass (vacuous + bytewise)** | 0 new shards → post-batch DB byte-identical to the input (sha256 after the batch unchanged) |
| H4 new-fact spot-checks | **pass (vacuous)** | no new facts |
| H5 policy determinism | **pass** | E5's full replay (see above) |
| H6 hygiene | **pass** | `make test` green (all test binaries, 0 failures); `cargo clippy --release --all-targets` 0 warnings; `cargo fmt --check` clean; no `src/` or `examples/` changes; `git status` confirms `data/` ignored and only the new `measurements/plan5/` untracked |

## Findings

1. **Exposure rate drifted up: 18.2 → 21.2 children/visit.** Batch 1's
   censored nodes were plies 0–2 (row/root positions); batch 2's are fresh
   ply-2 ledger children — slightly wider. The accepted E4 range (2,600–
   3,200) still held with margin; at this rate plan6's batch 3 (75 visits
   over a now ≈ 2,923-node number-1 pool) would add ≈ 1,590 records again
   if it repeats the base budget — worth remembering when sizing the
   per-layer caps (a layer that only exposes new breadth must not get the
   whole session).
2. **The number-2 pool is inert by construction, and now doubly measured.**
   After two sessions no node has ever had a second visit (all 150
   censored at pass 1). The ladder reserve in plan6's design is therefore
   not a small correction — it is the *only* mechanism that will ever
   revisit an invested line, which supports §2's "scheduled, measurable"
   framing over the emergent alternative.
3. **The census triple is a strong H1 instrument.** The (1, 1, 0) invariant
   on all 75 jobs plus the byte-matched census line pinned the session
   start exactly; no model defect surfaced this time (decision 5's
   mismatch branch never fired).

## Problems encountered

- Minor assembly-script defect: the batch-1 census stores job paths as
  space-joined strings while the ledger stores path arrays — the first
  cross-check run failed on the type mismatch; fixed by normalizing both
  to path tuples. No measurement impact.
- A wording error in the first `README.md` draft (it described the 150
  censored records as "75 at pass 1 + 75 at pass 2") was caught by
  re-reading the snapshot against the report text: all 150 are at
  `passes_failed` 1. Corrected before commit; the census JSON was never
  wrong.

## Unresolved / next steps

1. **Plan6 (batch 3): implement the decided selector.** Eligibility
   (censored revisit only when all children visited), pacing (interleaved
   slots), rationing (reserve share of the session cap; per-ply-layer
   visit caps), exposed through a TOML config surface; pre-registered A/B
   arms in `plan6.md` §2 — arm A′ replays this batch under the degenerate
   config and must byte-match it (initial ledger = the plan4 snapshot
   `c15661d5…`; the post-batch anchor is this run's ledger sha256
   `4f168105…`).
2. Website handoff (item 5), independent harvesters (item 6), propagation
   pass, DTM upgrade: unchanged, deferred.

## Tools used

- `cargo build/test/clippy/fmt` (release), `python3` (census assembly +
  ledger/census probes), the merger/harvest binaries. No new external
  tooling. All probes and the replay ran against throwaway copies in
  `/tmp/plan5/`; the standing shard set, manifest, and `data/` ledger were
  touched only by the real batch.

SESSION COMPLETE
- `report5.md` written; `measurements/plan5/` complete (census,
  ledger_snapshot, env.json, README); gate result: all gates pass, all
  five pre-registered expectations E1–E5 confirmed, 0 facts as
  pre-registered; the ladder measured unreachable (E3, the goal
  measurement). plan5.md §2 decision stands; plan6 drafted in the same
  sitting (docs-only, design dialogue).
Follow-up options:
  1. Kickoff prompt: "Execute docs/plans/proofdb/plan6.md: implement the
     selection mechanism (eligibility + pacing + rationing, `--pns-config`)
     and run the four-arm A/B over the post-batch-2 state; gates H1–H6
     including the A′≡A equivalence replay; report6.md with the ladder's
     marginal-value verdict."
  2. Alternative: review plan6's constants (reserve_share 0.25,
     layer_visit_cap 24, interleave_k 4, max_rung_passes 3) against the
     measured post-batch-2 census before committing the A/B — safer if the
     standing census deviates from the pre-registered ranges.
