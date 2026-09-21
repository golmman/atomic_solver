# Plan 10 Report: closing deliverable #16 — `structural_floor.md`, the authoritative no-go record

Executed 2026-09-21 per `plan10.md`. Consolidation plan: no new
measurements, no `src/` changes, no new hypotheses — the one discovery is
the document. Docs-only throughout.

## Gate decision: **COMPLETE**

All nine content sections present and evidence-linked; the Phase 0
inventory is fully verified (33/33 claims trace to existing reports or
`research_*.md` extractions); the Phase 2 cross-check is clean. Neither the
GAP nor the DEFER gate fired.

## What the document covers

[`structural_floor.md`](structural_floor.md) consolidates the initiative's
no-go record into one self-contained document, per the plan's content
contract:

1. **DF-PN+ commitment and its measured cost** — plan6's 9-row head-to-head
   evidence table (df-pn 2.6–4.5× over PDS variants, largest in the
   tree-≫-TT regime; PN² RAM-fatal; the recorded never-run comparisons).
2. **The 1+ε mechanism and the four ε closure legs** — no constant, no
   schedule, no regional structure (trajectory chaos), no node-local
   signal (plan4/plan4 Phase 1/plan8); the seesaw framing from report9.
3. **GHI and the path-independent TT** — first-player-loss shortcut,
   never-cache-repetition-dependent-results, and the plan9 crux (any
   path-derived leaf value poisons the frame-exit stored bounds).
4. **RAM = TT only and the best-first exclusion** — PN²/DeepPN
   contract-fatal; the two documented bounded exceptions (preflight region
   closure, reconstruct-side memory limit) stated precisely so the floor is
   not overclaimed.
5. **Ordering and TT-eviction local optima** — `lean` plan9's oracle floor
   (refuter at rank 0 in 100% of refuted AND frames) and plan7's eviction
   arms both failing against the incumbent priority replacement.
6. **The seesaw thread** — leaf-value site path-dependent at its core
   (#15/plan9); stay-deeper dial implemented (1+ε) and measured-closed;
   published head-to-head does not favor the leaf-value site even at home.
7. **Child-level termination surface** — plan5's four-family mapping;
   count-based limits inverted relative to the churn mass; correlation
   pruning unsound without an evaluator.
8. **The hard class** — m20–m23 (20 men, pawns, repetition-dominated,
   tree ≫ TT) plus the negative space (no simplification: zero frames
   below 6 men, 99.93–99.97% of frame-eval mass at ≥9 men, zero
   harvestable subgames, no preflight misses at the roots).
9. **Open threads after closure** — pre-weakened rows (#6/#7 blockers,
   #9/#13) plus the externally-owned threads (`conversion` #4 with
   `lean` #2, `lean` #10, Gao 2021 `conversion` #5e), each as "closed for
   now, reopen trigger, owner".

Initiative-level jargon (class (b), GO/NO-GO, gate object/controls) is
defined on first use in the preamble; every claim carries relative-link
pointers; code is cited by module, never line numbers.

## Evidence gaps

**None.** The Phase 0 audit
([`measurements/plan10/claims.md`](measurements/plan10/claims.md)) verified
every claim row against its target by opening it this session. The two
honest *limits of the record* (the never-run df-pn-vs-PDS-PN and
repetition-dominated comparisons from plan6; the metric-population caveat
on lean report9's OR work-share figures) are stated in place in §1/§5 as
closed threads, not flagged as gaps — both are already recorded by their
owning reports.

## Verification

- No `src/`/`examples/` changes: `git diff --exit-code` clean at close; no
  benchmark runs; no claim rests on a new measurement (one-discovery rule
  held throughout).
- Link check: all relative links in `structural_floor.md` (19 targets) and
  in `measurements/plan10/claims.md` (19 targets) resolve — recorded in
  [`measurements/plan10/linkcheck.md`](measurements/plan10/linkcheck.md).
- Numbers cross-checked against source reports, not summary rows: stress
  249,480,478 and m22 14,156,269 first-outcome child evals (post-plan9
  conventions), the plan4/plan7/plan8 arm tables, plan1/plan3 mass splits,
  and plan6's head-to-head margins all match their originating reports.
- Bibliography: no new entries; verified no **Open** row points only at
  this initiative — Saffidine 2011, Young 2016, Čížek 2025 point at
  `conversion` #4/#5c and Gao 2021 at `conversion` #5e; they stay as-is.
- Housekeeping: `cargo fmt --check`, `cargo clippy --release
  --all-targets`, `make test` — green (docs-only change; Boy Scout hygiene
  check).

## Problems encountered

- Initial link targets in `measurements/plan10/claims.md` were written one
  directory level too shallow; caught by the plan's own link-check step and
  fixed in-session (`linkcheck.md` records the correction).
- No other blockers. The plan's scope decision (document lives under
  `docs/plans/research/`, not `docs/spec/`) required no revisiting: the
  evidence chain references process records throughout.

## Unresolved parts / missing tests

- None for this plan. The document's §1/§5 record the comparisons the
  published literature never ran; that is the record's own boundary, not a
  missing test here.

## Closing statement for the initiative

The `research` initiative opened 2026-09-19 to find node-count levers
outside the current implementation. It closes 2026-09-21 having answered
that question exhaustively: plans 1–3 measured *where the work is* (>98%
unsolved child evals, ~80% threshold-cut mass, no simplification, one
outlier position family); plans 4–8 measured every accessible lever class
closed (ε constants/schedules/regional/node-local, TT eviction, conditioned
ε); plans 5/6/9 mined every unmined mechanism class in the literature and
found no class-(b) candidate. `structural_floor.md` is the residue: a
solver locked into DF-PN+ with a path-independent TT, a TT-only RAM
contract, position-only search-order signals, and a hard class whose cost
is priced-in threshold churn — each lock measured or mined, each with its
evidence pointer intact. The live threads it surfaced are owned elsewhere;
the record is the hand-off.
