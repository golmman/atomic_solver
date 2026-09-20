# Plan 5: Literature mine — child-level early termination in DF-PN

Initiative: `research` literature target #5. This is the documented
successor of plan4 (`report4.md`): the ε-threshold *schedule* surface is
closed, and #5 attacks the same churn mass (plan1: ~80% of frame evals in
threshold-cut frames) from the **child-granularity** side — stopping work on
a child whose bound has crossed a "hopeless" threshold, instead of pricing
whole frames (that side closed as `conversion` #6).

This is a mining plan per the working agreement: one discovery, no `src/`
changes, no POC. The deliverable is an extraction plus an applicability
verdict; any promising mechanism becomes the scope of a later POC plan.

## Goal

Answer backlog #5 with data instead of intuition: **are there published,
sound ways to terminate the work on a child (or child subtree) once its
bound crosses a hopeless threshold, in the DF-PN family, that are distinct
from (a) the already-implemented 1+ε second-child threshold and (b) the
closed partial-sum sweep short-circuit?** For every mechanism found, the
plan records whether it is already implemented here, sound in a
repetition-handling search, and worth a POC — or closes #5 with the
reasoning.

## Context

Four prior results shape this plan:

1. **The implemented child-level baseline is the 1+ε trick**
   (`dfpn/research_epsilon.md`, Pawlewicz & Lew 2007). In an OR frame the
   first child's pn threshold is `min(pt, ceil(p2 · (1+ε)))` (AND mirrored
   on dn; `src/search/dfpn/core.rs`, `epsilon_ceil`). This caps re-entries
   per child at `O(log threshold)` — but it governs only the *first-child*
   threshold recursion. plan4 closed the scheduling of ε; nothing about
   *mechanisms beyond* this formula is measured.
2. **The frame-granularity side closed NO-GO** (`conversion` #6,
   `conversion/report7.md`): the partial-sum sweep short-circuit broke out
   of the child sweep once the running summed bound crossed the frame
   threshold — a 2.2–3.3× net loss with outcome regressions. The decisive
   failure was the **deferral asymmetry**: the early cut could sit between
   the sorted children prefix and a decisive child later in the list, so
   wins the full sweep would have proven were deferred past the entire
   budget. Any child-level mechanism proposed here must be checked against
   this same asymmetry class.
3. **The churn mass is located** (plan1) but its child-level structure is
   not: we know ~80% of frame evals sit in threshold-cut frames, we do not
   know whether that mass is spent re-searching *few* heavy children many
   times (where child-level termination helps) or spread over *many*
   children once each (where it cannot).
4. **The soundness envelope is tight.** The solver's repetition handling
   (first-player-loss GHI shortcut, per-run repetition cache,
   path-independent TT entries — repetition-dependent results are never
   cached) forbids any mechanism that derives child results from
   path-dependent context or caches partial/early-terminated child outcomes
   keyed only by position. The `search/tt/` contract is a hard filter for
   applicability.

## Hypotheses

- **H1 (open surface)**: at least one published sound mechanism terminates
  child work at a hopeless threshold that is *neither* the 1+ε
  second-child threshold *nor* the partial-sum sweep short-circuit, and is
  compatible with the repetition/TT contracts above. If H1 holds, a sized
  POC proposal follows (plan6 candidate).
- **H0 (exhausted surface)**: the literature collapses onto mechanisms
  already implemented here (1+ε, standard threshold recursion, AND/OR
  solved-child cutoffs) or structurally equivalent to a closed lever
  (frame-granularity pricing, `conversion` #6). Backlog #5 closes with the
  mapping as evidence.

## Method

### Phase 0 — bounded survey and single-source selection

A time-boxed survey (≤ half a session; no deep reads). Collect candidates
via the defined queries and the repo's own PDF library
(`docs/plans/*/​*.pdf` already vendored: Nagai's dissertation may not be
vendored — availability is a selection criterion, not a given). Suggested
queries: *"depth-first proof number search cutoff"*, *"selective expansion
proof number search"*, *"PDS df-pn threshold"*, *"partial expansion PNS"*,
on arXiv, the Computers and Games / Advances in Computer Games series, and
ICGA Journal. Known candidate clusters to classify:

| Cluster | Anchors | Prior disposition |
|---|---|---|
| df-pn cutoff variants | Nagai 2002 dissertation (PDS: bounded depth-first with a second threshold), Pawlewicz & Lew 2007 follow-ups | 1+ε mined; PDS-as-*algorithm-swap* is backlog #8, but PDS's *cutoff rule* itself is in scope here |
| partial / selective expansion in PNS | whatever the survey surfaces | unmeasured — the core of #5 |
| solver-engineering cutoffs (shogi/chess solvers) | e.g. child-ordering cutoffs in competition reports | unmeasured |
| λ-search / threat-sequence pruning | Wolf 2000 | overlaps backlog #7 (recognizers) — exclude unless the source frames it as a cutoff inside df-pn |
| PN² and other algorithm swaps | backlog #8 — out of scope except where the source isolates a child-level cutoff rule | excluded |

**Selection criteria** (the plan mines exactly ONE source):

1. Addresses child- or subtree-granularity early termination in the PNS /
   DF-PN family, exact and sound (no sampling, no learned heuristics —
   learned priors are backlog #6);
2. Reproducible pseudo-code or precise rule statement;
3. Obtainable full text (arXiv/open access, or vendored);
4. Not already mined in a `research_*.md` (the 1+ε extraction covers
   Pawlewicz & Lew 2007 — its beyond-the-trick sections may still be
   selected if the survey shows they contain a distinct mechanism);
5. Applicability to a repetition-handling, TT-backed solver is arguable
   before the deep read (the deep read decides it).

Record the shortlist table — cluster, source, mechanism in one sentence,
availability, why-not-selected — even for rejected candidates, under
`measurements/plan5/`. If no source clears the criteria, that is already
an H0-leaning data point: document it and proceed to Phase 2 with the
shortlist as the evidence.

### Phase 1 — deep extraction

Write `docs/plans/research/research_child_termination.md`, following the
house extraction template (`conversion/research_ews.md`,
`conversion/research_mopns.md`):

- **Summary** — the mechanism in solver terms, one paragraph.
- **Background** — the published form: what problem it solves, the exact
  rule/pseudo-code, what the source proves about it (soundness, node-count
  claims, experimental setting and sizes — note where they differ from our
  14 M–250 M eval class).
- **Mapping to this solver** — for each distinct rule, the concrete site it
  would touch: `src/search/dfpn/core.rs` (threshold recursion,
  `epsilon_ceil`), `src/search/dfpn/children.rs` (pooled pre-eval,
  existence-query terminal classification, TT reuse),
  `src/search/dfpn/selection.rs` (OR/AND selection, second-best search).
  A mechanism that cannot name a call site is recorded as unmappable.
- **Soundness check against our contracts** — TT path-independence
  (repetition-dependent results never cached), GHI first-player-loss
  shortcut, repetition cache, `ProofEvent` neutrality, and the
  **deferral asymmetry** test from `conversion` #6 (could the mechanism
  defer a decisive child the unrestricted search would prove?).
- **Sizing sketch** — for each surviving mechanism: what a POC would gate
  (`stress` case gate object, `m22_white` / `dec13` / `dec10` controls),
  where temporary instrumentation lands, and the expected one-to-two
  session cost. No implementation.

### Phase 2 — applicability verdict

Classify every mechanism found:

- **(a) implemented** — name the code site; #5 gets no credit for it.
- **(b) sound and unmeasured** — POC proposal candidate; rank by expected
  leverage on the threshold-cut churn mass (plan1's profile is the
  sizing input) and by POC cost.
- **(c) unsound here** — say which contract it breaks and why; archived.
- **(d) structurally equivalent to a closed lever** — name the closed
  report and the structural argument (e.g. carries the same deferral
  asymmetry as `conversion` plan7).

The H1/H0 verdict follows from the classification: H1 iff at least one
mechanism lands in (b).

## Decision gates

| Gate | Criterion | Consequence |
|---|---|---|
| **OPEN** | ≥1 mechanism in class (b), with a named call site and a soundness-argument that survives Phase 1's contract check | Backlog #5 stays open as "mined — POC pending"; the ranked (b) list is the plan6 candidate pool; hand the top mechanism to the owning initiative (`lean`/`conversion`/`dfpn`) as a formal, sized item per the feeding rules. |
| **CLOSED** | All mechanisms land in (a), (c), or (d) — or Phase 0 finds no qualifying source | Close backlog #5 with the mapping as the no-go record; the next plan elevates the top remaining literature target (#6 ML priors, #7 recognizers, #8 PDS/PN² — with the note that if PDS's cutoff rule was the subject here, #8's algorithm-swap question remains genuinely separate). |
| **DEFER** | A qualifying source exists but is not obtainable in full text, or its pseudo-code is too imprecise to extract a rule | Record the blocker under the backlog row; next plan takes the next literature target; #5 stays open at reduced priority. |

## Deliverables

- `docs/plans/research/research_child_termination.md` — the extraction
  (Phase 1 template), created only if a source is selected.
- `docs/plans/research/measurements/plan5/` — shortlist table, query log,
  per-source relevance notes, the (a)–(d) classification.
- `docs/plans/research/report5.md` — verdict, gate decision, hand-off or
  closure record.
- `docs/bibliography.md` — new entries get a row (status **mined** with a
  pointer to the extraction, or **cited** if only touched); mined-source
  statuses updated.
- `docs/plans/research/initiative.md` — backlog row #5 status, History
  entry.

## Verification

- No `src/` or `examples/` changes at any point (mining plan): finish with
  `git diff --exit-code` clean; no benchmark runs are required, and none
  of the plan's claims may rest on a measurement (that is for the POC).
- The extraction, if created, is self-contained: rule, mapping, soundness
  check, and sizing sketch all present; every claimed code site verified
  by reading the file.
- Bibliography and backlog updates cross-checked against the report.
- Housekeeping: `cargo fmt --check`, `cargo clippy --release
  --all-targets`, `make test` still green (nothing changed; the gate is a
  hygiene check per the Boy Scout principle).

## Non-goals

- No POC implementation, env-gated or otherwise — the ranked (b) list
  becomes a *separate* plan per the one-discovery rule.
- No algorithm-swap evaluation (PDS/PN² as full replacements — backlog #8),
  no NN priors (#6), no mating-net recognizers (#7), no parallel designs
  (#9, owned by `conversion` #4).
- No production changes, benchmark-suite changes, or changes to the
  proof-tree layer, `ProofEvent` protocol, or the optimizer interface
  (`docs/spec/optimizer_interface.md`).
- No wall-time work; the metric of record remains first-outcome
  `child_evals`.

## Final task

Write `docs/plans/research/report5.md` (verdict, gate decision,
classification table, hand-off/closure record), update the backlog row for
#5 and the History in `initiative.md`, and update `docs/bibliography.md`
statuses for every source the plan touched.
