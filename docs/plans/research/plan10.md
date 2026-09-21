# Plan 10: Consolidating deliverable — `structural_floor.md`, the
# authoritative no-go record

Initiative: `research` closing deliverable **#16** (opened by the
2026-09-21 re-scope; the documented successor of plan9, `report9.md`).
With #15 closed, every literature target (#5, #8, #15) and every POC
candidate (#10–#14) is answered or closed, and #6/#7/#9/#13 carry recorded
blockers. The node-count program is closed (plan8); what remains is the
initiative's final purpose: **characterize the structural floor** — one
document that states, with evidence pointers, what the solver is locked
into and why.

This is a consolidation plan, not a mining or POC plan: no new measurements,
no `src/` changes, no new hypotheses. Its one discovery is the document
itself. Nothing in it may rest on a claim that does not already have a
report or `research_*.md` behind it.

## Scope decision (recorded up front)

**Placement: `docs/plans/research/structural_floor.md`.** The document is
inherently a process record — the re-scope mandates that "every claim links
to its report or `research_*.md`", which are repo-internal process
artifacts. `docs/spec/` forbids exactly that (AGENTS.md: no references to
`docs/plans/`, reports, or process vocabulary), so a spec placement would
force either dangling references or a rewrite that loses the evidence
chain. If an external consumer ever needs the distilled contract, a
standalone spec distillation can be carved out later as its own small task;
it is not part of this plan.

**The document does not open work.** Any thread it surfaces as genuinely
unexamined must already be pre-weakened with a recorded blocker (open
threads: `conversion` #4 parallel spike, `lean` #2/#10, Gao 2021 #5e — all
owned elsewhere). If drafting reveals a claim with *no* traceable
evidence, the GAP gate fires (below); the document records the gap, it
never fills one with new research.

## Goal

Produce `docs/plans/research/structural_floor.md`: one authoritative,
self-contained document consolidating the initiative's no-go record, such
that a reader outside this initiative can trace every locked-in commitment
to its measured or mined evidence, and the initiative can close with
nothing left open in its own backlog.

## Content contract (from the re-scope, expanded by plan outcomes)

The document must cover, at minimum — each section stating the lock-in,
the mechanism, *why* (the measured or evidence-based reason), and the
pointer chain:

1. **The DF-PN+ commitment and its measured cost.** plan6's head-to-head
   evidence table (df-pn dominates PDS variants 2.6–4.5× in the
   tree-≫-TT regime; PN² RAM-fatal; PDS-PN/DFPN-PN never run against
   df-pn — the gap recorded); the branch commit is a local optimum by
   published evidence, not by fiat (`report6.md`,
   `research_alternative_algorithms.md`).
2. **The 1+ε threshold mechanism and the four ε closure legs.** No global
   constant improves Pareto (plan4 Phase 0; ε=0.375 spin-off note), no
   depth/clock schedule (plan4 Phase 1), no regional structure —
   trajectory chaos (plan4), no exploitable node-local signal (plan8).
   The seesaw framing from `report9.md`: search-order guidance is confined
   to position-only signals and path-only thresholds.
3. **GHI and the path-independent TT.** First-player-loss shortcut;
   repetition-dependent results never cached; the consequence that every
   path-derived valuation mechanism is excluded (the plan9 crux: any
   path-depth-keyed leaf value poisons the frame-exit stored bounds —
   `report9.md`, `research_deep_dfpn.md`).
4. **RAM = TT only and the best-first exclusion.** PN²/DeepPN's open
   frontier is contract-fatal (plan6 classification (c)); the two
   documented bounded exceptions (region closure, reconstruct-side memory
   limit) stated precisely so the floor is not overclaimed.
5. **Ordering and TT-eviction local optima.** `lean` plan9's oracle-floor
   measurement (ordering headroom slim; AND-side refuter already at rank
   0) and research plan7's Phase 0/1 (priority replacement already
   present; harmful churn 0–0.146% of stores; both policy arms fail).
6. **The seesaw thread.** Leaf-value site path-dependent at its core
   (#15/plan9); the stay-deeper dial implemented (1+ε) and measured-closed;
   published head-to-head does not favor the leaf-value site even at home.
7. **Child-level termination surface.** plan5's mapping: threshold
   increments implemented; count-based limits equivalent to the closed
   partial-sum lever; correlation pruning unsound without an evaluator;
   FDFPN structurally equivalent and inverted relative to the churn mass
   (`report5.md`, `research_child_termination.md`).
8. **The structural characterization of the hard class.** What the outlier
   family *is* (m20–m23, 20 men, pawns, repetition-dominated, tree ≫ TT)
   and the negative space (no simplification: 99.9%+ frame-eval mass at
   ≥9 men, zero harvestable subgames — plan2/plan3; pre-flight fires at
   the roots).
9. **Known open threads after closure.** The pre-weakened rows (#6/#7
   no-heuristic-component blocker; #9/#13), each with its blocker and
   owner — recorded as "closed for now, reopen trigger X", not as
   recommendations.

Every claim carries a pointer to its `reportN.md` or `research_*.md`
(relative links). Where a claim cites code, cite the module, not line
numbers (they drift).

## Hypotheses

This plan has no H1/H0 in the experimental sense. Its equivalent:

- **COMPLETE**: every section above traces to existing evidence; the
  initiative closes cleanly (status → closed in `docs/plans/README.md`).
- **GAP**: a section's claim cannot be traced — the document records the
  gap explicitly in an "evidence gaps" subsection, the initiative still
  closes (the floor document describes the record as it stands), but the
  gap is flagged in `report10.md` for any future reopener.

## Method

### Phase 0 — evidence inventory (measurements/plan10/claims.md)

Enumerate every claim the document will make as a table: claim → primary
pointer → secondary pointers → verified (link target exists and actually
supports the claim, by opening it). This is the audit that makes the
document trustworthy; it also catches stale numbers (e.g. baselines moved
post-dfpn-plan9 — use the post-plan9 conventions in `initiative.md`:
stress 249,480,478, m22 ~14.2 M first-outcome child evals).

### Phase 1 — write the document

`docs/plans/research/structural_floor.md`, structured per the content list
above. House style: assertions first, evidence pointers immediately
following, no new claims. Include a short preamble stating what the
document is (the consolidated no-go record of the `research` initiative),
what it is not (a spec; a promise that no lever exists), and the date.

### Phase 2 — cross-check and close

- Every pointer resolves and supports its claim (re-walk the Phase 0
  table against the finished text).
- The document is self-contained for a reader who has never seen this
  initiative: initiative-level jargon (class (b), GO/NO-GO, gate objects)
  either spelled out or defined on first use.
- Backlog and History updates in `initiative.md`; initiative row moved to
  Closed in `docs/plans/README.md` with the successor note (open threads
  → `conversion` #4 / `lean` #2, already recorded there).

## Decision gates

| Gate | Criterion | Consequence |
|---|---|---|
| **COMPLETE** | All content sections present; Phase 0 table fully verified; cross-check clean | Write `report10.md`; close the initiative (backlog #16 → done, `initiative.md` status, `docs/plans/README.md` row → Closed). |
| **GAP** | ≥1 claim untraceable to existing evidence | Document the gap in an "evidence gaps" subsection; still close the initiative (the floor is characterized by the record as it stands); list the gap in `report10.md` as a reopen trigger. |
| **DEFER** | A section turns out to require a *new* measurement or mining to state honestly | Do not fabricate; mark the section "blocked on <missing evidence>" and surface it in `report10.md` as a potential new initiative, not as research-backlog work. |

## Deliverables

- `docs/plans/research/structural_floor.md` — the consolidating record
  (the deliverable itself).
- `docs/plans/research/measurements/plan10/claims.md` — the evidence
  inventory / verification table (Phase 0).
- `docs/plans/research/report10.md` — closure report: what the document
  covers, gate decision, any gaps, the initiative's closing statement.
- `docs/plans/research/initiative.md` — backlog #16 status, Closing
  deliverables table updated, History entry, status → closed.
- `docs/plans/README.md` — `research` row moved from Active to Closed.

## Verification

- No `src/` or `examples/` changes: `git diff --exit-code` clean at close;
  no benchmark runs; no claim may rest on a new measurement.
- Link check: every relative link in `structural_floor.md` resolves
  (`grep -o` over the markdown + a shell existence check; recorded in
  `measurements/plan10/`).
- Numbers cross-checked against the source reports, not this initiative's
  summary rows (the summary rows are the map, the reports are the
  territory).
- Bibliography: no new entries expected; verify no `Open` row points *only*
  at this initiative (Saffidine 2011, Young 2016, Čížek 2025, Gao 2021
  point at `conversion` — they stay as-is; note this in the report).
- Housekeeping: `cargo fmt --check`, `cargo clippy --release
  --all-targets`, `make test` green (hygiene check per the Boy Scout
  principle).

## Non-goals

- No new measurements, mining, or POCs — the one-discovery rule; the
  discovery here is the document.
- No placement under `docs/spec/` (scope decision above); no external
  standalone distillation.
- No re-litigation of any closed lever; where a closure is contested
  during writing, the document cites the existing record verbatim and
  moves on.
- No reopening of `conversion` #4, `lean` #2/#10, or the Gao 2021 DAG
  thread — they are named as open threads with owners, not worked here.
- No changes to the proof-tree layer, `ProofEvent` protocol, or the
  optimizer interface (`docs/spec/optimizer_interface.md`).

## Final task

Write `docs/plans/research/report10.md` (what the document covers, gate
decision, evidence-gap list if any, closing statement for the
initiative), move the `research` row to Closed in
`docs/plans/README.md`, and update `initiative.md` (backlog #16, Closing
deliverables table, History).
