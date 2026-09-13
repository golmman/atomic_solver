# Plan 3: Research reading round — Expected Work Search and MOPNS

Initiative: `conversion` backlog #5, items (a) and (b) only. This is a
**docs-only plan**: no production code, no search-behavior change, no drift
protocol. The deliverables are two literature extractions
(`research_ews.md`, `research_mopns.md`), a synthesis that maps each paper's
machinery onto the solver's repetition-dominated surface, and a re-ranked
backlog with explicit go/no-go recommendations for follow-up implementation
plans. Items (c)–(e) of backlog #5 (parallel PNS 2025, Kishimoto & Müller
journal GHI, Gao complexity) stay open and are not mined here — (c) feeds
backlog #4/`lean` #2 and (d) feeds `dfpn` #4, both of which have owners.

Prerequisite reading: `initiative.md` (Motivation "Why plain PNS variants are
not the answer", non-goals), `dfpn/report10.md` (the measured threshold-
destabilization no-go any candidate mechanism must structurally avoid),
`dfpn/research_repetition_cache.md` (what the repetition-dominated work
profile actually consists of: 96% re-proofs, cost in the churn around
uncacheable draw chains, not in the proofs themselves).

## Goal

Answer, from the primary sources, one question per paper:

1. **EWS (Randall, Müller, Wei, Hayward 2024, arXiv:2405.05594):** does
   expected-work child selection replace or complement DF-PN threshold
   selection in a regime where most of the tree is repetition-dependent
   draw chains, and — critically — did their positional-superko
   implementation (5×5 Go, explicitly repetition-dominated) change *what is
   cacheable* in their transposition/repetition handling in a way that
   transfers to our first-player-loss shortcut?
2. **MOPNS (Kishimoto, IJCAI-11):** does the formal three-outcome
   (win/loss/draw) proof-number framework expose machinery our draw
   propagation lacks — i.e. is our `suppress_draw` shortcut +
   repetition-cache + path-terminal handling a special case of MOPNS, or a
   deviation from it with measurable cost — and does MOPNS provide a
   soundness argument (or a needed correction) for caching repetition-
   dependent draws?

The initiative's structural constraint is normative for every candidate
mechanism the synthesis proposes: it must change **what is cacheable** or
**where the line comes from**, not the threshold arithmetic — or, if it does
touch child selection/thresholds, it must state why the plan10 failure mode
(folding solved mass into unsolved parents' `(pn, dn)`) does not apply.

## Extraction format

Follow the house style of `dfpn/research_*.md` (see
`dfpn/research_epsilon.md` / `dfpn/research_repetition_cache.md`): a
standalone markdown file, sections numbered, algorithm descriptions in the
paper's terms followed by an explicit mapping section ("what this means for
this solver"). Every claim about the paper cites the paper's section/theorem;
every claim about the solver cites the file/function. Do not copy the papers
wholesale — extract the machinery, the soundness conditions, and the
empirical results relevant to repetition-heavy solving.

## Sources

- EWS: [arXiv:2405.05594](https://arxiv.org/abs/2405.05594) (open access).
  Also check the authors' follow-ups/citations for corrections (the paper is
  the EWS cite in the CG community as of 2026). The repetition angle is the
  paper's 5×5 Go positional-superko experiment.
- MOPNS: A. Kishimoto, *Multiple-Outcome Proof Number Search*, IJCAI-11.
  Open-access via IJCAI proceedings (`ijcai.org/Proceedings/11`); if the
  proceedings PDF is unreachable, Kishimoto's Alberta/Université page hosts
  an author copy. Cross-reference with the already-mined GHI material
  (`dfpn/research_ghi.md`) — MOPNS is the same author's formalization of
  three-outcome PNS, so `research_ghi.md`'s terminology should be reused,
  not reinvented.

## Research 1: EWS (`research_ews.md`)

Extract, at minimum:

1. **The work-minimizing selection rule.** EWS picks the child minimizing
   estimated expected work, combining a win-rate estimate with proof-size
   estimates (pn/dn). Give the exact formula, the estimator inputs, and the
   stated assumptions (independence, estimator calibration).
2. **Where the win-rate estimate comes from.** Identify whether EWS requires
   a learned/heuristic evaluation function. If yes, this is the transfer
   blocker for a pure solver with no evaluation: state it explicitly and
   evaluate the degenerate alternatives (e.g. win-rate ∝ 1/pn-style
   surrogates, or EWS restricted to the AND side where "win rate" is a
   defense-survival estimate our static scorer could inform). Distinguish
   what the paper proves/argues from what would be our invention.
3. **The superko handling.** How the 5×5 Go experiment deals with
   repetitions: what is cached, what is re-proven, whether GHI-style
   verification is used, and what their repetition regime cost profile looks
   like compared to ours (§1–2 of `dfpn/research_repetition_cache.md`).
   This is the headline question — they solved a repetition-dominated game;
   find out whether the win came from selection, from caching, or from the
   estimators.
4. **Threshold interaction.** EWS vs. df-pn thresholds: does EWS replace
   `(pn, dn)` threshold descent entirely, or sit inside it? Would adopting
   it touch `selection.rs`/`children.rs` threshold arithmetic (the plan10
   hazard), or is it a pure ordering/selection layer on top of the existing
   child evaluation?
5. **Mapping section** ending in a recommendation: candidate backlog item
   (with the mechanism, expected surface, and soundness contract sketch) or
   a documented no-go, referencing which of the initiative's constraints it
   would violate.

## Research 2: MOPNS (`research_mopns.md`)

Extract, at minimum:

1. **The three-outcome proof-number definitions.** MOPNS assigns proof and
   disproof numbers with respect to each outcome; give the definitions, the
   expansion/backup rules, and the selection rule. Contrast with the
   two-outcome pn/dn we run plus the ad-hoc `Outcome::Draw` propagation.
2. **Draw semantics.** How MOPNS treats draw as a first-class outcome: when
   is a node "drawn" (proven), how do draw children affect parent pn/dn, and
   what does the framework say about *path-dependent* draws (repetitions)?
   The solver's current stance — repetition-dependent draws are never cached
   as solved, first-player-loss shortcut — should be checked against the
   framework: is it consistent with MOPNS, and does MOPNS offer the missing
   monotonicity/validity argument for the repetition cache (backlog #1's
   lemma is our own; a published counterpart would strengthen it).
3. **What our propagation might be missing.** Walk the solver's draw
   machinery against the paper: `evaluate_child`'s terminal order
   (`children.rs`: moves-empty > extinction/`occupied == 2` > rule50 >
   path-repetition), the `suppress_draw` unsolved store, the plan9/plan1
   repetition cache, and `repetition_seen` propagation. List any case the
   paper handles that we handle differently (e.g. draw-vs-draw comparisons,
   multiple draw sources, outcome priority), with a concrete position class
   where the difference matters if one exists.
4. **Relation to the 2005 journal GHI algorithm** (backlog #5d, not mined
   here): note which parts of MOPNS depend on it so the later #5d mining can
   build on this file instead of re-deriving.
5. **Mapping section** ending in a recommendation, same format as above.
   Expected outcomes are: (i) "no missing machinery — our propagation is a
   documented special case" (a valuable negative result, closes #5b), or
   (ii) a concrete missing case → new backlog item with a soundness contract
   under the initiative's working-agreement #5.

## Tasks

1. Fetch and read the EWS paper; write `research_ews.md` (sections above).
2. Fetch and read the MOPNS paper; write `research_mopns.md` (sections
   above). Reuse `dfpn/research_ghi.md` terminology; link to it rather than
   redefining GHI concepts.
3. Synthesis pass (each file's mapping section must end with a table row):
   for each paper — mechanism, where it would live in our code
   (`selection.rs` / `children.rs` / `repetition_cache.rs` / TT), whether it
   changes what is cacheable / where the line comes from / thresholds, the
   plan10-hazard verdict, estimated effort (S/M/L), and a go/no-go
   recommendation for opening a backlog item.
4. Update `initiative.md`: backlog #5 status (a/b mined, c/d/e open), and —
   only if a mechanism earns a **go** — open the corresponding backlog item
   with the soundness-contract sketch from step 3 (do not draft the
   implementation plan in this session; the reading round ends at the
   re-ranked backlog).
5. Update `docs/bibliography.md`: flip both entries from **Open** to
   **Mined** with pointers to the new files (keep the backlog-item links
   that remain open, e.g. #5d/#5e for the GHI journal version and Gao).
6. Sanity-check the docs build: no code changed, so the gate is
   `cargo fmt --check`/`cargo test --release` green (they must be — nothing
   should have touched `src/`) and no stale cross-references from the new
   files into `docs/plans/` internals that the bibliography's spec rules
   would forbid (these are plan-side docs, so cross-references into
   `docs/plans/` are fine here; the `docs/spec/` standalone rule does not
   apply).
7. Write `report3.md` in this directory: papers actually read (versions,
   page counts), discrepancies between paper and our assumptions, the go/
   no-go verdicts, backlogs opened/closed, unresolved questions (e.g. parts
   of EWS that need the follow-up literature), and next steps.

## Non-goals

- Any production code change, env-gated spike, benchmark run, or drift
  protocol (there is nothing to drift).
- Mining backlog #5c (Čížek 2025 parallel PNS), #5d (Kishimoto & Müller
  journal GHI), or #5e (Gao) — each has an owning backlog; do not duplicate.
- Drafting an implementation plan for EWS-style child selection or MOPNS
  corrections: the reading round's output is the re-ranked backlog; the next
  plan number is assigned only to mechanisms that earn a go.
- Re-litigating closed no-gos (plan1 draw-cache v2, plan2 verification
  lever, dfpn plan10 Win/Loss reuse) except to check whether the papers'
  machinery structurally avoids the measured failure modes — a paper-based
  "it should work" against a measured no-go needs a mechanism-level reason,
  which is exactly what the mapping sections must produce or withhold.
