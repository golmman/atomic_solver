# Plan 7: POC #12 — TT eviction/turnover measurement, conditional replacement-priority POC

Initiative: `research` POC candidate #12 (TT eviction priority). This is the
documented successor of plan6 (`report6.md`): with the algorithm-swap
question closed (H0), the CLOSED consequence names the remaining targets —
literature #6/#7 (both demoted) or POC candidate #12 — and #12 is the only
one not already killed by a direct measurement.

**Premise correction (why this plan is measurement-first).** The backlog row
for #12 reads "protect entries from the current PV or high-work subtrees
*instead of uniform replacement*". That premise is stale: the table has had
priority replacement since before the initiative opened.
`src/search/tt/table.rs::insert_new` scores each live slot lexicographically
by `(live-in-generation, solved, work, generation)` and evicts the
lower-scoring slot; solved entries outrank unsolved ones and higher-work
entries outrank lower-work ones. The candidate as written is therefore
already partially implemented. What is *not* measured is whether the
remaining harmful case — a new **unsolved** store evicting a live **solved**
entry from a full bucket, or high-work unsolved entries churning — occurs at
all, and whether any such churn costs child evals. Phase 0 measures exactly
that, with a cheap kill gate before any replacement-policy POC is built.

Per the working agreement: one discovery, throwaway instrumentation, `src/`
changes reverted before the report, no production hand-off from this plan.

## Goal

Answer backlog #12 with data: **does the transposition table evict entries
that the search would later have reused — in particular live solved entries
— and if so, does a replacement-policy guard convert that churn into a
first-outcome `child_evals` reduction on the hard class?** The plan is
designed to close #12 cheaply if the answer is no (the expected outcome,
given the pre-weakening), and only escalates to a behavior-changing POC if
Phase 0 shows material harmful churn.

## Context

Six prior results shape this plan:

1. **The ε/TT-size invariance study (pre-weakening).** The dfpn position
   study measured the stress case's decisive point invariant across
   TT ∈ {32 MB, 128 MB, 2 GB} — a 64× capacity range. If evictions were
   materially costly, 32 MB should differ from 2 GB. It does not, which is
   strong indirect evidence that eviction churn is not a first-order cost
   on the stress case. Phase 0 converts this indirect inference into a
   direct measurement (eviction counts by victim class), which is the
   missing piece — and also covers m22/dec13/dec10, which the invariance
   study did not.
2. **TT capacity is a solved, separate problem.** `lean` #16 (plan4) found
   the *default* size capacity-starved and raised it 64 → 128 MB
   (m22 first-outcome −49% wall). That was a capacity fix, not a
   replacement-policy fix; #12 asks whether the policy *within* a filled
   table matters beyond capacity. The two must not be conflated: Phase 0
   runs at the current 128 MB default (and 32 MB as a pressure point),
   never re-litigating the default.
3. **The replacement policy already encodes the candidate's own
   suggestion.** "Protect high-work subtrees" is the `work` term in the
   `insert_new` score; "protect solved entries" is the `solved` term. The
   only unprotected transitions are: (a) new-unsolved evicting live-solved
   when both slots hold live solved entries (the new entry replaces the
   lower-scoring *existing* slot regardless of its own score), and
   (b) unsolved-vs-unsolved eviction by `work`, where the victim may still
   be probed later. Phase 0 counts both.
4. **The generation mechanism is inert within a run.** `new_generation()`
   is never called by the search (verified: no callers outside the TT
   module), so there is no wholesale generational invalidation — all churn
   is bucket-local (two entries per bucket). This simplifies the
   instrumentation: every eviction is an `insert_new` into a full,
   same-generation bucket.
5. **plan1's work partition.** >98% of child evals are on unsolved nodes
   and ~80% of frame evals sit in threshold-cut frames. If the TT were
   discarding solved facts that shortcut re-proofs, the churn would show up
   exactly in the re-proof mass that plan4's ε-sweep found trajectory-chaotic.
   A null result here adds a third leg to that closure (no schedule arm, no
   regional structure, no eviction churn to exploit).
6. **The `twin_stats` example** already dumps TT statistics (GHI-sensitive
   positions); Phase 0's counters extend the same surface rather than
   inventing a new one.

## Hypotheses

- **H0 (no churn to exploit)**: at the 128 MB default (and under 32 MB
  pressure), evictions of live solved entries are negligible or absent,
  occupancy stays materially below capacity, and probe-miss cost
  attributable to eviction is ~0 — consistent with the TT-size invariance
  study. Backlog #12 closes with direct eviction data joining the
  invariance study and the plan4 ε-closure as a three-legged no-go record.
- **H1 (material churn, policy can fix it)**: Phase 0 shows material
  harmful churn (live-solved evictions or high-work unsolved churn at
  ≥ a threshold set in Phase 0, e.g. ≥ 1% of stores on the stress case),
  and an env-gated replacement guard yields **≥ 10% first-outcome
  `child_evals` reduction on the stress case** with m22/dec13/dec10 within
  +5% and zero quick-suite outcome changes → hand-off to `lean` as a sized
  backlog item.
- **H2 (material churn, policy cannot fix it)**: Phase 0 shows churn but no
  Phase 1 arm clears the gate — the churn is real but the two-slot layout
  cannot exploit it without structural changes forbidden by the RAM = TT
  contract. #12 closes with the churn characterization as the record.

## Method

### Phase 0 — turnover instrumentation (throwaway, 1 session)

Add eviction/turnover counters to `TranspositionTable` (plain `u64` fields
or atomics; no behavioral change to `store`/`probe` decisions), exposed via
a stats accessor and printed at pre-exit under an env flag (e.g.
`ATOMIC_TT_STATS=1`), following the `twin_stats` precedent. Counters:

- **store classes**: update-existing / insert-into-free-slot /
  evict-stale-victim / evict-live-unsolved-victim / **evict-live-solved-victim**
  (the harmful class), cross-tabulated by new-entry class (solved/unsolved);
- **probe classes**: hit / miss (bucket empty or stale) / miss where a
  same-key entry exists but was evicted earlier this run (approximated by
  keeping a small key-history set, bounded — not part of the shipped table);
- **occupancy**: end-of-run fraction of buckets holding 2 live entries
  (derivable from the existing `stats()`).

Runs (release build, `--first-outcome`, default 128 MB, plus `--tt-size 32`
as the pressure point):

| Case | FEN source | Role |
|---|---|---|
| stress | `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21` | gate object |
| m22_white | `tests/fixtures/move_order_positions.txt` | control |
| dec13 | `tests/fixtures/decisive_positions.txt` | control |
| dec10 | `tests/fixtures/decisive_positions.txt` | control |

**Drift gate for Phase 0**: counters only — `benchmark --suite quick --json
--first-outcome` must be bit-identical before and after instrumentation
(per the measurement conventions; verified before any Phase 0 numbers are
recorded).

**Phase 0 kill gate** (either condition closes #12 after this phase):

- live-solved evictions < 0.1% of store calls **and** end-of-run occupancy
  materially below capacity (< ~80% of buckets full) at 128 MB; or
- harmful churn is nonzero at 32 MB pressure but the *node counts* match
  the 128 MB run within noise (churn present but cost-free — the invariance
  study's mechanism, now directly labeled).

Otherwise proceed to Phase 1 with the measured churn profile as the sizing
basis.

### Phase 1 — conditional replacement-priority POC (only if the kill gate passes)

Env-gated replacement variants in `insert_new` (one `match` on an env var;
no layout changes — two entries per bucket stays, per RAM = TT only):

- **V1 — solved-slot immunity**: a new *unsolved* store never evicts a live
  *solved* slot (the store is dropped, or displaces the unsolved slot if
  one exists). Directly closes harmful transition (a).
- **V2 — steeper work priority**: among live unsolved slots, evict by
  `work` with a depth/remaining-depth tiebreak (protect deep high-work
  subtrees over shallow churn). Targets harmful transition (b).

Gates (pre-registered, mirroring plan4's):

- **GO**: best arm ≥ 10% below the 249,480,478 stress first-outcome
  baseline, m22/dec13/dec10 within +5%, zero quick-suite outcome changes
  (59/59 outcomes preserved; eval deltas expected and allowed — the policy
  change is behavior-altering, so the bit-identical drift gate does not
  apply, but no outcome may flip) → hand-off to `lean` as a sized backlog
  item with the arm table as sizing evidence.
- **NO-GO / H2**: no arm clears the bar → close #12 with the churn
  characterization and the arm table as the record.

### Phase 2 — revert and record

All `src/` instrumentation and POC code reverted; `git diff --exit-code`
verified before the report is written. Counters, run tables, and the
short arm table (if Phase 1 ran) archived under `measurements/plan7/`.

## Decision gates

| Gate | Criterion | Consequence |
|---|---|---|
| **KILL (Phase 0)** | kill-gate conditions above | Close backlog #12 with direct eviction data; three-legged no-go record (eviction counts + TT-size invariance + ε-schedule closure). Next plan: the remaining pre-weakened targets (#6/#7) or re-scope. |
| **GO (Phase 1)** | ≥10% stress FO win, controls within +5%, 0 quick outcome changes | Hand off to `lean` as a sized backlog item; #12 marked mined-with-POC; spike code archived. |
| **NO-GO (Phase 1)** | churn real, no arm clears the bar | Close #12 per H2 with the churn characterization; the two-slot layout is recorded as the binding constraint. Next plan as per KILL. |

## Deliverables

- `docs/plans/research/measurements/plan7/` — counter design, run tables
  (per case × TT size: store/probe classes, occupancy), quick-suite
  bit-identity proof for Phase 0, and the Phase 1 arm table if reached.
- `docs/plans/research/report7.md` — verdict (H0/H1/H2), gate decision,
  churn characterization, hand-off/closure record, premise-correction note.
- `docs/plans/research/initiative.md` — backlog row #12 status, correction
  of the candidate-table premise ("uniform replacement" → priority
  replacement exists; the open question is the harmful transition), and a
  History entry.
- No bibliography changes expected (no new literature).

## Verification

- Phase 0 instrumentation is counter-only: `benchmark --suite quick --json
  --first-outcome` bit-identical before/after (recorded in
  `measurements/plan7/`).
- Phase 1, if run: zero outcome changes on the quick suite and the three
  controls; all raw run outputs archived.
- `git diff --exit-code` clean at plan close (throwaway code reverted).
- Housekeeping: `cargo fmt --check`, `cargo clippy --release
  --all-targets`, `make test` green (hygiene gate; nothing shipped).

## Non-goals

- No production replacement-policy implementation — a GO verdict produces a
  *hand-off item* to `lean`, not a landed change.
- No TT layout changes (bucket size, entry fields, snapshot format) — the
  RAM = TT only contract and the snapshot/import contract are fixed.
- No TT-capacity re-litigation (`lean` #16 owns the default size).
- No #13 (frontier priors), no #4 (frame overhead), no literature mining,
  no proof-tree/`ProofEvent`/optimizer-interface changes, no wall-time-only
  work; the metric of record is first-outcome `child_evals`.

## Final task

Write `docs/plans/research/report7.md` (verdict, gate decision, churn
characterization, closure or hand-off record, premise-correction note),
update backlog row #12 and the History in `initiative.md`, and verify the
revert.
