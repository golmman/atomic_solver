# Plan 3: Mine SPDFPN 2014 — option-A reassessment under accepted nondeterminism

Initiative: `parallel`, new backlog #4. **Docs-only plan**: no production
code, no benchmark runs, no drift protocol (nothing to drift).

**Premise change (owner decision, 2026-10-02):** the consumer accepts
nondeterministic parallel runs — nondeterminism confined to *which*
valid proof wins, never a false decisive outcome. This reopens option A
(thread-level shared-TT parallel DF-PN), which plan2's NO-GO deliberately
left behind an unmined source: Pawlewicz & Hayward 2014, *Scalable
Parallel DFPN Search* (CG 2013, LNCS 8427) — the mechanism paper behind
Solrex's concurrency model, and the best few-thread shared-TT figures
published (≈5.2×/4, 11.8×/16 per Čížek's citation). The plan2 verdict on
option C (process-per-child, 0.27× wall, 15.5× work inflation) stands.

Prerequisite reading: `parallel/initiative.md` (constraints, backlog),
`parallel/design_space.md` (option matrix; §1 row A's gate item "mine
Pawlewicz & Hayward 2014"), `research_solrex.md` (what its thin §5
delegates to this paper), `../dfpn/research_parallel.md` (Kaneko — the
paper's own §5.5 comparison target), `../dfpn/research_ghi_journal.md`
(soundness template), `../research/structural_floor.md` §9 (parallelism
as the only multiplicative lever).

## Goal

Answer, from the primary source, a fixed question set, then synthesize
into a GO/NO-GO input for the A-stage and a staged plan skeleton:

1. **Mechanism**: what exactly makes SPDFPN's threads cooperate — work
   threshold (`MaxWorkPerJob`), virtual win/loss + virtual (d)pns +
   per-ply virtual TT + job lock, `TRYRUNJOB` assignment, shared-TT
   discipline (locking, overwrite rules, replacement policy) — and how
   it differs from Kaneko's congestion-term protocol.
2. **Measurements**: parallel efficiency vs thread count, and the
   overhead decomposition (threading overhead vs extra work/work
   inflation) — the numbers option C lacked.
3. **Soundness**: what the paper does and does not say about repetitions
   under concurrency; what our path-independent TT payload + thread-local
   repetition cache already covers; the residual contracts we must write.
4. **Mapping**: a component-by-component bill of materials against this
   solver (serial 1+ε base, work capping, TT `work` field, TT
   concurrency, ProofEvent ordering, C1–C4), with effort sizing and the
   domain-transfer risks that a prototype spike must measure.

## Deliverables

- `docs/theory/spdfpn-2014/` — vendored PDF + extraction (find the real
  paper: the bibliography's arXiv pointer was expected to be wrong; the
  DOI + an author copy are the fallbacks).
- `research_spdfpn.md` in this directory — the mining analysis with the
  question-set answers, the option-A verdict, and the staged plan
  skeleton (drift-gated stages, pre-registered kill points).
- `docs/bibliography.md` — entry corrected (arXiv pointer removed) and
  flipped **Open → Mined**; `docs/theory/README.md` row added.
- `parallel/initiative.md` — reopened: premise change recorded in
  Status/History, backlog #4 opened.
- `docs/plans/README.md` — `parallel` row updated (reopen event).
- `report3.md` in this directory (final task).

## Non-goals

- No code changes; the inert TT-concurrency refactor and the prototype
  are *future* plans (plan4/plan5), gated on this round's GO input.
- Not re-litigating plan2 (option C no-go) or lean plan7 (deterministic
  no-go); the premise change is the only reopen condition.
