# Plan 5: within-layer selection for breadth-pns — the decision, and the second batch (ladder-reachability control)

Initiative: `proofdb`. Follows `report4.md` finding 3 ("a next-generation
selector would need work-aware numbers or a per-layer budget split to make
progress past ply 3–4") and next-steps 1 ("the current rule is
pre-registered, so changing it is a plan-level decision"). This plan does
two things: **(a) decides** the within-layer selection question — work-aware
effective numbers vs a per-layer budget split — and pre-registers the
decision here; **(b) pre-registers and runs the second batch** over the
exposed frontier as the **control measurement** that converts "the ladder
can never fire in-session" from arithmetic into a measured result.

Deliberately a **no-code-change plan**: batch 2 runs the unchanged, still
pre-registered plan4 policy. The decided selection change is implemented in
plan6 (batch 3), A/B'd against this control. Docs + `examples/`-side
measurements only; the product solver, DB schema, and spec are untouched.
Per repo convention the final task is `report5.md`.

## 1. Background (self-contained)

**State after plan4.** The standing shard dir holds 197 validated shards;
the derived DB (`data/proofdb.db`, 35,055 nodes) is unchanged since batch 1
(manifest digest `37c1f5b2…`). The sidecar ledger
(`data/proofdb_work.json`, verified byte-identical to the committed plan4
snapshot, sha256 `c15661d5…`) holds **1,443 records**: 75 censored
(`passes_failed` 1, ≈ 4.0M `work_done` each; plies 0/1/2 = 1/20/54) and
1,368 freshly exposed (`passes_failed` 0; plies 2/3 = 289/1,079). Batch 1
yielded 0 facts / 300.0M evals with a strictly breadth-first ply sweep
(0 → 1 → 2), and the first ladder rung never fired in-session.

**Session-start census (pinned by a read-only probe on throwaway copies,
2026-10-01).** `pns: rows 35055 (open 75), ledger records 1434 …; jobs
1483 (rows 49, ledger 1434)`. Exact job universe at batch-2 start:

- **1,483 jobs** = 49 open rows + 1,434 ledger nodes. (Of the 1,443 ledger
  records, 9 are merged into their open row nodes by the lineage gate —
  `pns/build.rs` keeps the censor history on an open row; no work state is
  lost, `ledger_records` counts tree *nodes*.)
- **Number-1 pool: 1,408** = 1,368 fresh ledger records + 40 never-visited
  open rows (plies 3–31).
- **Number-2: 75** = 66 censored ledger children + 9 censored rows
  (batch-1 visits; pass state merged into the row nodes).

Note: `report4` finding 3's "49 + 1,368 = 1,417 number-1 jobs" counted the
9 already-censored rows at number 1; the measured pool is 1,408 number-1 +
75 number-2 (1,483 total). The correction is in the conservative direction:
the pool is *bigger* than stated. Batch 2 pre-registers the measured
census, not the 1,417 figure.

**Layer arithmetic (the pathology, quantified).** One 300M session affords
**75 visits** at the 4M base budget. The first fresh layer in queue order
(ply 2, 289 nodes) costs 289 × 4M ≈ **1.16B**; the ply-3 layer (1,079
nodes) ≈ **4.32B**. So a factless session spends its entire cap on ~26% of
one ply-2 sub-layer — and each censored visit **exposes ≈ 18.2 new
number-1 records** (batch 1: 1,368 / 75) while removing at most one node
(itself) from the pool.

## 2. The decision (pre-registered): per-layer budget split with a scheduled ladder reserve

Two lemmas fix the decision before any batch could:

- **Lemma 1 (work-aware degeneracy).** Every member of the number-1 pool
  has `passes_failed = 0` and `work_done = 0`, and its structural role
  number is 1 (§2 of plan4's induction; measured: batch-1 census "all
  number 1"). Any selection key built from (structural numbers, ledger
  state) is therefore *constant on the pool* and collapses to the current
  `(number, ply, path)` — a work-aware effective number is provably a
  no-op on exactly the pool that defines finding 3. It only re-ranks
  already-censored nodes (finer ladder granularity), which is irrelevant
  while the ladder is unreachable.
- **Lemma 2 (pool growth).** A censored visit removes ≤ 1 node from the
  number-1 pool and adds ≈ 18.2. A factless session's pool grows
  monotonically; even a modest fact yield cannot drain it (drainage needs
  ≈ 1,400 decisive facts against ≈ 75 visits/session). The ladder's firing
  condition — number-1 pool empty — is **unreachable in-session at this
  frontier breadth under any ordering** built from the data the selector
  has. The 1,408 × 4M ≈ 5.6B pool cost vs the 300M cap says the same thing
  statically.

Consequence: within a fresh layer there is *nothing to order* — the only
real levers are budget policy. Therefore:

**Decision: the per-layer budget split** (report4's second option).
Concretely, for the plan6 selector:

1. The fresh-queue order stays `(number, ply, path)`.
2. Each ply layer receives a bounded share of the session cap (per-layer
   visit/eval allocation; plan6 fixes the constant) — a wide layer can no
   longer consume the whole session, so a session *reaches deeper plies*
   (where plan2/plan3 measured the cheap facts: plies 4–27 and the ply-5
   sibling class) instead of dying at ~26% of ply 2.
3. A pre-registered reserve fraction of the session cap is spent as
   **scheduled ladder rungs** (revisits of the cheapest-effective-number
   censored nodes), interleaved with fresh visits (exact interleave rule
   fixed in plan6; interleaved preferred — facts on invested lines land
   earlier). The ladder stops being emergent (Lemma 2 shows it can never
   be) and becomes a scheduled, measurable part of every session.

Rejected: **work-aware effective numbers** — Lemma 1 makes them
unmeasurable here; they would be a code change with a provably
batch-identical outcome.

**Fallback (retained, not decided):** the shallow-subtree-aware tiebreak
from report4 finding 3. Legal-child counts are already computed at expand
time (zero new movegen cost) and are the only data-backed key that *can*
reorder a fresh layer. Plan6 may A/B it against the split if the split's
first batch disappoints.

**Implementation scope (plan6, not this plan):** `pns/selector.rs` (+
census fields, one CLI flag), tests via `tests/proofdb.rs`. No `src/`
changes, no schema/spec change, ledger format unchanged.

## 3. Deliverables

- **D1 — batch 2** (the control run) under `measurements/plan5/`:
  `census_breadth-pns.json` (job records as plan4), `ledger_snapshot.json`
  (post-batch escalation state), `env.json`, `README.md` (provenance,
  command table, the measured verdicts on E1–E5 below). No grown-DB copy
  needed if 0 facts (post-batch DB byte-identical to input, as in plan4).
- **D2 — `report5.md`**: the measured ladder-reachability verdict, the
  pool-growth rate measured on batch 2, the ply-2 fresh pool's yield,
  gate verdicts, and the plan6 kickoff (split implementation + batch 3).

## 4. Pre-registered decisions (fixed before the run)

1. **Batch 2 runs the unchanged plan4 policy** — `breadth-pns`, base 4M,
   ladder `2^(k-1) × 4M`, cap 300M, root = startpos, ledger =
   `data/proofdb_work.json` (must equal the plan4 snapshot byte-for-byte
   at session start; abort on drift). It is the *control*: the decided
   split is not in effect and must not leak in.
2. **Expectations (pre-registered):**
   - **E1 (queue shape):** every visit comes from the ply-2 fresh
     sublayer (289 available > 75 visits); the job ply sequence is
     constant 2; the cap fires before the sublayer drains, so ply 3 is
     never reached.
   - **E2 (yield ≈ 0 facts):** plan2/plan3 precedent puts cheap facts at
     ply ≥ 4 (ply-5 sibling class, plies 4–27 ladder lines); the ply-2
     fresh pool is replies of quiet 4M-censored nodes. Any fact is
     reported as the first ply-2-frontier yield data point, not a gate
     failure.
   - **E3 (ladder never fires):** no visit at `pass ≥ 2` — Lemma 2. This
     is the plan's goal measurement: the expected result is a measured
     **no**, and that is a success outcome (it converts finding 3's
     arithmetic into data and is the justification for §2's decision).
   - **E4 (ledger growth):** 1,443 → ≈ 2,890 records (75 censored bumps +
     ≈ 75 × 18.2 fresh ply-3 records; accepted range 2,600–3,200), all
     new records at ply 3.
   - **E5 (determinism):** full replay from the same input DB + initial
     ledger → identical job sequence and byte-identical post-batch ledger
     (plan4 H5 shape).
3. **Contingency (decision re-open rule):** if E3 is violated (a ladder
   rung fires) or E2 yields > 10 facts, the §2 decision re-opens before
   plan6: the pool is not as inert as measured, and the split's reserve
   may need re-sizing (or the work-aware ladder re-ranking becomes
   relevant after all).
4. **No selection-code changes in this plan** (§2's decision is plan6
   work); the legacy gradients stay selectable but are not run.
5. **Census at session start must match §1's pinned numbers** (1,483
   jobs / 1,408 number-1 / 75 number-2); a mismatch is a model defect —
   stop and investigate (plan4 finding 1 precedent).

## 5. Pre-registered gates (fixed before any run)

Plan4's H1–H6, adapted to a 0-fact-expected run:

- **H1 integrity:** unchanged (lineage gate, exclusion census, job-path
  check); plus decision 5's census match.
- **H2 merge determinism:** with 0 new shards the merger re-run must
  reproduce the DB **byte-identically** (a real residue check on a
  censored session, not a vacuous pass). With facts, plan4 H2 verbatim.
- **H3 no-regression / H4 spot-checks:** vacuous at 0 facts; verbatim
  otherwise.
- **H5 policy determinism:** E5's full replay.
- **H6 hygiene:** `make test` green; clippy/fmt clean (no code changed —
  the gate verifies that claim); `git status` confirms `data/` ignored.

A gate failure is a defect in the tool or this plan's model — stop and
investigate, do not loosen the gate.

## 6. Non-goals

No selection-key or budget code changes (plan6); no schema/spec change; no
`src/` changes; no work-proportional numbers (rejected for this frontier
by Lemma 1); no in-session re-propagation; no parallel harvesters (item
6); no subtree scoping (item 7); no website handoff (item 5); no
DTM-upgrade pass (item 3); no new facts expected, none forced.

## 7. Tasks

1. Verify session-start state (ledger sha256 = plan4 snapshot; DB/manifest
   digest unchanged) and the census match (decision 5).
2. Run batch 2 (decision 1 budgets); write the post-batch ledger snapshot.
3. Check gates H1–H6 (E5 replay included); assemble D1
   (`measurements/plan5/`).
4. Write D2 (`report5.md`): E1–E5 verdicts, measured pool-growth rate,
   ladder-reachability verdict, plan6 kickoff.

## 8. Budget

One execution session, no code changes. Compute: ≈ 75 jobs × 4M ≈ 300M
child-evals ≈ 65–70 s wall (batch-1 rate 4.6M/s) + the E5 replay double-run
(+ ≈ 70 s) + one merger byte-identity run (≈ 1 min). Fits easily in one
sitting.

## SESSION COMPLETE

- `docs/plans/proofdb/plan5.md` written (plan session, docs-only;
  read-only probe on throwaway `/tmp` copies pinned the batch-2 census:
  1,483 jobs / 1,408 number-1 / 75 number-2, superseding report4's
  1,417 figure). §2 pre-registers the within-layer decision: **per-layer
  budget split with a scheduled ladder reserve**; work-aware effective
  numbers rejected by the degeneracy lemma (constant on the fresh pool);
  shallow-subtree-aware tiebreak retained as fallback. Batch 2 is
  pre-registered as the unchanged-policy control measuring ladder
  reachability (expected verdict: never fires in-session — a success
  outcome). No code changed; `data/` untouched (probe ran on copies).
Follow-up options:
1. Kickoff prompt: "Execute docs/plans/proofdb/plan5.md (second batch,
   unchanged breadth-pns policy as the ladder-reachability control):
   verify ledger/DB state, run batch 2 over the 1,483-job frontier
   (cap 300M, base 4M), gates H1–H6 incl. the E5 replay, report5.md with
   the E1–E5 verdicts and the plan6 kickoff (per-layer split)."
2. Alternative: skip the control and go straight to plan6 (implement the
   split + batch 3) — faster to the decided policy, but loses the
   baseline that makes batch 3's A/B interpretable; not recommended.
