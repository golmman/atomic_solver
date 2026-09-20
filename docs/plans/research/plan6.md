# Plan 6: Literature mine — PDS / PN² algorithm-swap scaling vs DF-PN+

Initiative: `research` literature target #8. This is the documented
successor of plan5 (`report5.md`): the child-level early-termination surface
closed (H0), and plan5's classification explicitly carved out the
*full-algorithm* question — PDS's cutoff rule was classified subsumed by the
DF-PN+ threshold recursion, so backlog #8's open question is exclusively
whether the **complete replacement algorithms** (Nagai's PDS, Breuker's PN²,
or a descendant) scale better than DF-PN+ on this solver's hard class.

This is a mining plan per the working agreement: one discovery, no `src/`
changes, no POC. The deliverable is an extraction plus an applicability
verdict; only if a variant survives every contract does a sized POC
proposal follow as a separate plan.

## Goal

Answer backlog #8 with data instead of intuition: **does the published
record show PDS, PN², or a descendant scaling better than df-pn/DF-PN+ in
the regime that matters here — deep, repetition-dominated AND/OR solving
where the decisive outcome sits ~30+ full moves out and graph-history
(loop) handling is load-bearing — and can any such variant be mapped onto
this solver's contracts?** For every variant found, the plan records
whether it is already structurally covered here, compatible with the
repetition/TT/memory contracts, and worth a POC — or closes #8 with the
lock-in cost quantified (or honestly recorded as "no published evidence
either way").

## Context

Five prior results shape this plan:

1. **The solver is committed to DF-PN+** (iterative bounded refinement,
   1+ε second-child threshold, GHI first-player-loss shortcut,
   path-independent TT entries). #8 does not propose a swap lightly: its
   stated purpose is to *quantify the lock-in cost* — either the literature
   shows a materially better algorithm for this class (→ POC), or it does
   not (→ the commitment is validated and #8 closes).
2. **plan5's boundary** (`research_child_termination.md`): PDS's
   *stopping condition* (both thresholds must be exceeded simultaneously)
   and its ε interaction are already understood as subsumed by the
   threshold recursion; the epsilon extraction even records the published
   PDS-side ε guidance (`dfpn/research_epsilon.md`). What is *not*
   measured is the whole-algorithm structure: PDS's per-level threshold
   schedule (pn/dn threshold pairs expanding down the tree) versus df-pn's
   re-entry-driven recursion, and PN²'s two-level best-first-versus-proof
   decomposition.
3. **The hard class is repetition-dominated.** The stress case and the
   m20–m23 family are deep-conversion positions where the search spends
   ~80% of frame evals in threshold-cut frames (plan1) and where the
   first-player-loss GHI shortcut plus the per-run repetition cache carry
   the soundness. Historically this is exactly where the PNS variants
   diverged: df-pn displaced PDS/PN² in shogi tsume-sovlers largely
   because df-pn's loop handling (Kishimoto–Müller GHI df-pn) was solvable
   while best-first variants' was painful. Whether that displacement
   generalizes to *this* class, with a GHI shortcut already in place, is
   the core question.
4. **The memory contract is a hard filter.** The search CLI's contract is
   "RAM = TT only" (AGENTS.md). PN²'s outer best-first level holds an
   entire frontier in memory — structurally in conflict. PDS and df-pn are
   both depth-first and compatible in principle. Any (b)-class verdict
   must state the memory profile of the variant explicitly.
5. **Availability is a known constraint.** plan5's Phase 0 recorded that
   Nagai's 2002 dissertation was not obtainable; the PDS/PN² record must
   be mined from what is obtainable (Winands PDS-PN 2002 author copy, the
   PNS-variants chapter with threshold formulas (3)–(6), the Kishimoto
   2012 ICGA survey, Breuker's PN² publications) or from secondary
   sources — with the resulting confidence level stated in the report.

## Hypotheses

- **H1 (swap-worthy surface)**: at least one PDS/PN²-family variant has
  published head-to-head evidence of better scaling than df-pn/DF-PN+ in a
  repetition-dominated or deep-conversion regime, survives the GHI /
  path-independence / RAM contracts with a named mapping onto
  `src/search/dfpn/`, and can be POC'd as an env-gated alternative driver
  at a bounded (2–3 session) cost. If H1 holds, a sized POC proposal
  follows (plan7 candidate).
- **H0 (lock-in validated)**: the published record either favors df-pn in
  this regime, shows the PDS/PN² advantages concentrated elsewhere
  (shallow proofs, memory-cheap best-first, first-visit efficiency), or is
  absent/unclear — with no variant surviving the contracts regardless.
  Backlog #8 closes with the lock-in cost documented as "no evidence of a
  better algorithm for this class."

## Method

### Phase 0 — bounded survey and single-source selection

A time-boxed survey (≤ half a session; no deep reads). Candidate clusters:

| Cluster | Anchors | Prior disposition |
|---|---|---|
| PDS proper | Nagai 2002 (dissertation — recorded unobtainable); secondary statements in the ICGA-2012 survey and the PNS-variants chapter | unmeasured as full algorithm |
| PDS-PN and two-level hybrids | Winands 2002 (author copy available) | unmeasured |
| PN² | Breuker 1994/1996 (Allis-era Utrecht line); PNS-variants chapter | unmeasured; memory filter likely decisive |
| df-pn vs PDS head-to-head reports | Kishimoto 2012 ICGA survey; tsume-shogi / Othello solver competition reports; Kishimoto 2005 dissertation (open) | unmeasured — the scaling-evidence core of #8 |
| GHI / loop handling across variants | Kishimoto–Müller 2004/2005 (vendored); df-pn loop handling vs PDS/PN² | contract filter, not a selection target |
| Recent PNS-variant applications | post-2012 solver papers using PDS/PN²/df-pn with measured comparisons | unmeasured |

**Selection criteria** (the plan mines at most ONE deep source; the
head-to-head evidence table may cite several):

1. Contains either (i) measured head-to-head node/depth comparisons
   between PDS (or PN²) and df-pn-family search, or (ii) precise
   pseudo-code for a PDS/PN²-family variant applicable to AND/OR game
   trees with repetition handling;
2. Reproducible pseudo-code or a quantified comparison (not anecdote);
3. Obtainable full text (arXiv/open access, author copy, or vendored);
4. Not already mined in a `research_*.md`;
5. Applicability to a repetition-handling, TT-backed, RAM-bounded solver
   arguable before the deep read.

Record the shortlist table — cluster, source, contribution in one
sentence, availability, why-not-selected — under `measurements/plan6/`.
If no source clears the criteria, that is an H0-leaning data point:
document it and proceed to Phase 2 with the shortlist as evidence.

### Phase 1 — deep extraction

Write `docs/plans/research/research_alternative_algorithms.md`, following
the house extraction template (`research_child_termination.md`,
`conversion/research_ews.md`):

- **Summary** — the variant(s) in solver terms, one paragraph each.
- **Background** — the published form: threshold schedules / level
  decomposition, the exact rules or pseudo-code, what the source proves or
  measures (node counts, depth reached, memory), and *in what regime*
  (game, depth class, repetition density) — flagged against our 14 M–250 M
  eval class.
- **Head-to-head evidence table** — every published comparison found,
  normalized to: setting, metric, winner, margin, and whether the setting
  resembles the stress class. This table survives even if the verdict is
  H0; it is the lock-in-cost record.
- **Mapping to this solver** — where a variant's mechanism would live:
  `src/search/dfpn/core.rs` (frame loop, threshold recursion, refinement
  rounds), `src/search/dfpn/children.rs` (pooled pre-eval, TT reuse),
  `src/search/dfpn/selection.rs`, `src/search/tt/` (path-independence).
  A variant that cannot name call sites is unmappable.
- **Contract check** — GHI first-player-loss shortcut (does the variant's
  loop handling compose with it, or does it need best-first state that the
  shortcut forbids caching?); TT path-independence (repetition-dependent
  results never cached); per-run repetition cache; `ProofEvent` emission
  per proven/disproven node; **RAM = TT only** (frontier memory profile
  stated explicitly — expected fatal for PN²); refinement-round /
  `--refine-cap` and child-eval-budget interaction (the solver's bounded
  rounds are part of its own df-pn structure; a swap must reproduce or
  replace them).
- **Sizing sketch** — for each surviving variant: what a POC would gate
  (stress case gate object, `m22_white` / `dec13` / `dec10` controls), the
  env-gated alternative-driver shape, and an honest 2–3 session cost
  estimate (a full driver is above the usual 1–2 session POC bar; say so).

### Phase 2 — applicability verdict

Classify every variant found:

- **(a) structurally covered** — the variant's advantage is already
  present in DF-PN+ (name the mechanism); #8 gets no credit for it.
- **(b) sound here, evidence of a win, unmapped** — POC proposal
  candidate; rank by expected leverage on the stress class and by POC
  cost.
- **(c) contract-breaking** — say which contract (GHI, path-independence,
  RAM) and why; archived.
- **(d) evidence-absent or evidence-against** — the published record does
  not support (or actively rejects) the swap for this regime.

The H1/H0 verdict follows from the classification: H1 iff at least one
variant lands in (b).

## Decision gates

| Gate | Criterion | Consequence |
|---|---|---|
| **OPEN** | ≥1 variant in class (b), with a named call-site mapping, a contract-check pass, and head-to-head evidence in a comparable regime | Backlog #8 stays open as "mined — POC pending"; the ranked (b) list is the plan7 candidate pool; a formal sized hand-off proposal goes to `dfpn` (re-open trigger: a measured algorithmic alternative — note the reopen trigger is about *soundness diagnostics*, so the hand-off likely targets a new dedicated initiative or `conversion` #4's design spike, decided in the report). |
| **CLOSED** | All variants land in (a), (c), (d) — or Phase 0 finds no qualifying source | Close backlog #8 with the head-to-head evidence table as the lock-in-cost record; the next plan takes the top remaining target (#6 ML priors / #7 recognizers — both with their documented pre-weakening — or POC candidate #12 TT-eviction priority). |
| **DEFER** | The decisive comparison exists but is unobtainable, or no source contains reproducible rules/numbers | Record the blocker under the backlog row; next plan takes the next target; #8 stays open at reduced priority. |

## Deliverables

- `docs/plans/research/research_alternative_algorithms.md` — the
  extraction (Phase 1 template), created only if a source is selected.
- `docs/plans/research/measurements/plan6/` — shortlist table, query log,
  per-source relevance notes, the (a)–(d) classification, and any vendored
  PDFs.
- `docs/plans/research/report6.md` — verdict, gate decision, evidence
  table summary, hand-off or closure record.
- `docs/bibliography.md` — new entries get a row (**mined** with a pointer
  to the extraction, or **cited** if only touched); statuses of Nagai
  2002, Winands 2002, Breuker, and the Kishimoto 2012 survey updated if
  their disposition changes.
- `docs/plans/research/initiative.md` — backlog row #8 status, History
  entry.

## Verification

- No `src/` or `examples/` changes at any point (mining plan): finish with
  `git diff --exit-code` clean; no benchmark runs are required, and none
  of the plan's claims may rest on a new measurement (that is for the POC).
- The extraction, if created, is self-contained: rules, head-to-head
  evidence table, mapping, contract check, and sizing sketch all present;
  every claimed code site verified by reading the file.
- Bibliography and backlog updates cross-checked against the report.
- Housekeeping: `cargo fmt --check`, `cargo clippy --release
  --all-targets`, `make test` still green (nothing changed; the gate is a
  hygiene check per the Boy Scout principle).

## Non-goals

- No POC implementation, env-gated or otherwise — the ranked (b) list
  becomes a *separate* plan per the one-discovery rule.
- No re-litigation of plan5's child-level classification (PDS's cutoff
  rule stays closed as subsumed; this plan only addresses whole-algorithm
  structure and scaling evidence).
- No parallel/massively-parallel PNS designs (#9, owned by `conversion`
  #4 / `lean` #2); Čížek 2025 is in scope only as a head-to-head data
  point, not a parallel design source.
- No NN priors (#6), no mating-net recognizers (#7), no TT-eviction POC
  (#12), no benchmark-suite changes, no changes to the proof-tree layer,
  `ProofEvent` protocol, or the optimizer interface
  (`docs/spec/optimizer_interface.md`).
- No wall-time work; the metric of record remains first-outcome
  `child_evals`.

## Final task

Write `docs/plans/research/report6.md` (verdict, gate decision,
classification and evidence-table summary, hand-off/closure record), update
the backlog row for #8 and the History in `initiative.md`, and update
`docs/bibliography.md` statuses for every source the plan touched.
