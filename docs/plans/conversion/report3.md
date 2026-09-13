# Report: Research Reading Round — EWS and MOPNS (Plan 3)

## Summary

Docs-only reading round per `plan3.md`: both backlog #5 papers were
fetched, read end-to-end, and mined into `research_ews.md` and
`research_mopns.md`. Both mappings end in a **no-go**: no backlog item is
opened, and no production code, benchmark, or drift protocol was touched
(the plan had nothing to drift). Two valuable results came out anyway:
(1) EWS is closed with a *paper-grounded* transfer blocker (the win-rate
estimator) instead of the prior structural suspicion; (2) MOPNS — closed as
the planned "valuable negative result": the solver's `Outcome`-based draw
propagation is formally the MOPNS Draw-threshold slice, and MOPNS
explicitly defers repetitions to the GHI literature, so no published
caching-soundness argument was waiting here. One material discrepancy was
found and corrected: **MOPNS is Saffidine & Cazenave, ECAI 2012 — not
"Kishimoto, IJCAI-11"**; no such Kishimoto paper exists. Backlog #5 items
(c)–(e) remain open with their owning backlogs.

## Papers actually read

| Paper | Version read | Extent | Notes |
|---|---|---|---|
| Randall, Müller, Wei, Hayward, *Expected Work Search: Combining Win Rate and Proof Size Estimation* | [arXiv:2405.05594v1](https://arxiv.org/abs/2405.05594) (submitted 2024-05-09), HTML (LaTeXML) rendering cross-checked against the PDF | 9-page manuscript, §1–7 + Appendix A | Only public version at mining time; a check of the paper's public citation record surfaced no corrections or retractions. The 5×5 Go positional-superko experiment (§5.2.1) and the EWS-WR/EWS-PS ablations (§5.2) are the load-bearing sections for our questions. |
| Saffidine, Cazenave, *Multiple-Outcome Proof Number Search* | Published ECAI 2012 PDF (author copy, `lamsade.dauphine.fr/~cazenave/papers/mopns.pdf`), pp. 708–713, DOI 10.3233/978-1-61499-098-7-708 | 6 pages, §1–6, Figures 1–7, Propositions 1–5, Tables 1–3 | Full text extracted and mined. Cites the *journal* GHI paper (Information Sciences 175(4), 2005) as its cyclic-graph reference — confirming #5d as the right mining target. |

Both papers were downloaded to `/tmp/opencode` (`ews.pdf`, `mopns.pdf`;
text extracted with `pypdf` — the container has no `pdftotext`). The EWS
appendix (optimality proof of the child-ordering rule) was recovered from
the arXiv HTML rendering.

## Discrepancies between the papers and our assumptions

1. **MOPNS authorship/venue (material, corrected).** `initiative.md`
   backlog #5b and `docs/bibliography.md` attributed MOPNS to
   "A. Kishimoto, IJCAI-11" and framed it as "the same author's
   formalization of three-outcome PNS" relative to the mined GHI material.
   Verification: OpenAlex knows exactly one work with this title
   (Saffidine & Cazenave, ECAI 2012); the complete IJCAI-11 proceedings
   index (fetched end-to-end, page-continuity checked) contains no MOPNS
   paper and no Kishimoto game-solving paper beyond "Evaluations of Hash
   Distributed A\*" (with Kobayashi, Watanabe, paper 105); the ACM DL
   record found by web search is the same Saffidine–Cazenave paper; and
   the EWS paper itself cites it as Saffidine & Cazenave, ECAI 2012. The
   bibliography and initiative were corrected. Consequence for the plan's
   question 2: the "same author's formalization" cross-reference premise
   was wrong — MOPNS is by different authors and cites GHI rather than
   extending it. The extraction answers were unaffected in substance (the
   paper is still the formal multi-outcome PNS framework).
2. **"EWS solved 5×5 Go despite repetitions via selection, caching, or
   estimators?"** Our assumption going in (initiative backlog #5a
   description) was that the repetition-dominated win might encode a new
   cacheable surface. It does not: the GHI handling is the known
   Kishimoto–Müller *simulation-verified reuse* of a fully-populated TT
   (§4.3), and the paper's own ablations attribute the win to the
   win-rate-weighted expected-work selection (both ablations » 24 h on
   5×5 vs 5.25 h; EWS-WR 8.5× worse on average on 6×6 Go). No
   repetition-specific cost profile is reported anywhere in the paper.
3. **"MOPNS might provide the missing monotonicity/validity argument for
   caching repetition-dependent draws."** It does not, and could not: the
   framework assumes finite acyclic games (§2), acknowledges that the
   50-move rule creates cycles and that the rule is "usually abstracted
   away" (footnote 3), and defers cyclic graphs to Kishimoto & Müller
   (§6). The published counterpart to backlog #1's lemma does not exist
   outside the GHI literature.
4. **Minor, favorable:** our modeling choice of keeping the halfmove clock
   in the node identity (rule50 in the Zobrist key; dfpn non-goal to
   remove it) is independently endorsed by MOPNS footnote 3's advice to
   treat clock-affected positions as distinct nodes rather than abstract
   the rule into cycles.

## Go/no-go verdicts

| Item | Verdict | One-line reason |
|---|---|---|
| #5a EWS child selection | **No-go** (documented, no backlog opened) | The measured benefit requires a win-rate estimator (`WR` term in Eqs. 2/3); the paper's own estimator-free ablation EWS-WR ≡ "negamax PNS" is 8.5× worse on average and »24 h on 5×5 Go; every estimator-free surrogate (`WR ∝ 1/pn`, static-scorer "defense survival") either collapses to that ablation or is uncalibrated invention; EWS replaces (not extends) the df-pn skeleton (L effort); the ordering-layer salvage is blocked by `lean`'s OR oracle floor and plan4's measured-empty AND surface. Plan10 hazard: vacuously clear (no thresholds to pollute — a different framework). |
| #5b MOPNS machinery | **No-go** (closed as valuable negative result) | Our draw propagation (`selection.rs::is_solved_by_children` all-children-solved / no-win / some-draw) is exactly MOPNS's `G(n,Draw)=0 ∧ S(n,Draw)=0` — no missing machinery; the only cacheable-surface candidate (MOPNS `pess/opti` half-facts) has a measured-thin stress surface (repetition-draw chain first proofs sum to 1,597 child evals — both halves complete ~1 eval apart; `dfpn/research_repetition_cache.md` §2) and no published soundness argument; df-MOPNS threshold targeting is plan10-hazard class (same per-outcome sum/min folding, no decoupling mechanism in the paper). |

Structural-constraint check (initiative): neither mechanism clears the
"changes what is cacheable or where the line comes from" bar in a usable
way — EWS's line source is an estimator we cannot supply; MOPNS's
cacheable candidate adds no hits beyond the plan9 cache's own surface.

## Backlogs opened / closed

- **Closed:** `conversion` backlog #5a (EWS — documented no-go) and #5b
  (MOPNS — valuable negative result, the plan's expected outcome (i)).
- **Opened:** none (both mappings ended in no-go, per plan task 4).
- **Unchanged / re-pointed:** #5c feeds #4/`lean` #2, #5d feeds `dfpn` #4,
  #5e stays open. Two sharpened pointers from this round: EWS's
  repetition win came from *caching-everything + simulation-verified
  reuse* — i.e. the journal-GHI lever `dfpn` #4/#5d already tracks — and
  MOPNS cites that same journal paper as its cyclic-graph reference.

## Files changed (docs only)

- `docs/plans/conversion/research_ews.md` (new) — extraction with §4.4
  summary row.
- `docs/plans/conversion/research_mopns.md` (new) — extraction with §5.2
  summary row, GHI-terminology reuse per plan, #5d hand-off note.
- `docs/plans/conversion/initiative.md` — backlog #5 row updated
  ((a)+(b) closed with verdicts; MOPNS attribution corrected in the item
  description); History entry added.
- `docs/bibliography.md` — EWS entry **Open → Mined**; MOPNS entry
  corrected to Saffidine & Cazenave ECAI-12 and **Open → Mined**; the
  #5d/#5e open links are preserved.
- `docs/plans/conversion/report3.md` (this file).
- `src/`, `tests/`, examples: untouched (`git status` verified).

## Tools/examples used

- Web fetching: arXiv abs/HTML/PDF for EWS; IJCAI-11 proceedings index
  (for the attribution verification); OpenAlex API (title search, single
  hit); DuckDuckGo lite (author-copy PDF discovery); `curl` + `pypdf` for
  PDF text extraction (no `pdftotext` in the container).
- No repo tools, benchmarks, or examples were run — nothing to measure.

## Problems encountered

1. The plan's MOPNS sourcing note ("Open-access via IJCAI proceedings;
   if unreachable, Kishimoto's Alberta page hosts an author copy")
   dead-ended: the paper is not in the IJCAI-11 proceedings at all. The
   trail (ACM DL hit → lamsade author copy → OpenAlex → proceedings
   page-continuity check) is documented in `research_mopns.md`'s header
   note; future readers should cite ECAI-12.
2. The first attempt to delegate output-file triage to a subagent hit the
   session's subagent-depth limit; resolved by grepping/reading the saved
   tool outputs directly (no repo impact).
3. `webfetch` on the IJCAI proceedings page silently summarized the
   middle of the listing; a page-gap analysis over the extracted page
   numbers confirmed the listing was complete and MOPNS genuinely absent
   (rather than lost in truncation).

## Unresolved questions

- EWS: no estimator-free variant was studied in the paper beyond the
  EWS-WR/EWS-PS ablations; whether a *learned* win-rate or Proof-Cost-
  Network-style estimator could ever be supplied for atomic chess is
  out of scope for this initiative (a pure solver by charter) — noted,
  not pursued.
- MOPNS: the depth-first variant (§4.6) and the DAG-adapted relevancy
  bounds (§6) are sketches without experiments; if a future lever ever
  needs per-outcome effort numbers, that sketch — not this paper's
  best-first experiments — is the reference, and it must clear the
  plan10 hazard explicitly.
- The repetition-soundness question for any cross-context reuse remains
  owned by the GHI literature (#5d / `dfpn` #4); neither paper adds to
  it.
- The EWS journal/conference follow-up status (beyond arXiv v1) was not
  exhaustively tracked; no corrections were found in the public citation
  record at mining time.

## Next steps

- `docs/plans/README.md` status index: no row change needed (the
  `conversion` initiative row reflects an open initiative; plan3 is an
  internal step of it).
- The next ranked levers are unchanged: #4/`lean` #2 (parallel search
  spike, fed by #5c when mined) and `dfpn` #4 (bounded cross-path
  verification, fed by #5d) — now with two paper-grounded data points
  (EWS §4.3, MOPNS §6) pointing at the journal GHI algorithm as the
  reference for the latter.
- The next plan number in this initiative remains **plan5** (per the
  initiative's Status note); per the plan's non-goals, no implementation
  plan was drafted for EWS or MOPNS.
