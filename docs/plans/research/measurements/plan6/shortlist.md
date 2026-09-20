# Plan 6 — Phase 0 shortlist and selection

Selection criteria (from `plan6.md`): (1) measured df-pn-vs-(PDS/PN²) head-to-
head evidence **or** precise pseudo-code for a PDS/PN²-family variant
applicable to AND/OR trees with repetition handling; (2) reproducible
pseudo-code or quantified comparison (not anecdote); (3) obtainable full
text; (4) not already mined in a `research_*.md`; (5) applicability to a
repetition-handling, TT-backed, RAM-bounded solver arguable before the deep
read.

## Shortlist table

| Cluster | Source | Contribution (one sentence) | Availability | Why (not) selected |
|---|---|---|---|---|
| PDS proper | Nagai 2002, *Df-pn Algorithm for Searching AND/OR Trees and Its Applications* (dissertation) | Original PDS and df-pn definitions; the equivalence of most-proving-node selection | Not obtainable (plan5 Phase 0; re-confirmed this plan) | Criterion 3 fails; algorithm content covered via sources below (their NegaPDS pseudo-code is reproduced from Nagai) |
| PDS proper / two-level | Winands, Uiterwijk, van den Herik 2002, *PDS-PN* (CG 2002) | NegaPDS + NegaPDS-PN pseudo-code (appendix); Tables 1–5: PN/PN²/PDS/αβ/PDS-PN head-to-heads on 488 + 286 hard LOA positions; PN² memory-degradation curve; explicit "PDS ignores GHI" note | **Obtained** (Maastricht author copy; vendored `winands2002_pdspn.pdf`) | Content overlaps the chapter (below) which adds the df-pn section; used as corroborator |
| PDS / PN² / PDS-PN / df-pn in one place | van den Herik & Winands, *Proof-Number Search and its Variants* (chapter) — **SELECTED** | PDS threshold rules (Eqs. (8)–(9)), step-by-step PDS example, NegaPDS-PN pseudo-code, PN² construction (Eqs. (10)–(11)), complete LOA comparison tables (Tables 3–9), and §7's df-pn-vs-PDS ratios (Table 10) with the recorded df-pn-vs-PDS-PN evidence gap | **Obtained** (Maastricht author copy; vendored `vanherik_winands_pnchapter.pdf`) | Meets all five criteria: whole-algorithm scope, exact pseudo-code + quantified comparisons, obtainable, not previously mined (plan5 cited it only as a survey pointer), mapping arguable |
| PDS / PN² / PDS-PN (same authors, 2004 journal version) | Winands et al. 2004, ICGA Journal 26(1) *The PDS-PN search algorithm* | Journal extension of the CG-2002 paper | Not fetched | Subsumed by the chapter for this plan's question; adds no variant or comparison the chapter lacks |
| PN² proper | Breuker 1998, *Memory versus Search in Games* (dissertation); Breuker et al. 2001 (AG9) | PN² elaboration, logistic growth limit, chess experiments | Not obtainable (dissertation); AG9 paper not fetched | Criterion 3 fails; the chapter + ICGA-2012 survey reproduce the construction (Eqs. (10)–(11)) and all cited measurements |
| df-pn vs PDS head-to-head | **Pawlewicz & Lew 2007, *Improving Depth-first PN-Search: 1+ε Trick*, §4** | The primary quantified comparison: Atari Go TT-size sweep (Table 1), 488 easy + 286 hard LOA solved-counts vs time limit (Tables 2–3), geometric-mean speed ratios (Table 4: enhanced df-pn 4.17× faster than enhanced PDS, 4.46× than PDS) | **In repo** (`docs/plans/dfpn/epsilon.pdf`, vendored pre-plan5) | Not selectable as the deep source — already mined (`dfpn/research_epsilon.md`, criterion 4); used as the evidence-table anchor instead. Its §4 PDS comparison was not covered by that extraction |
| GHI / loop handling across variants | Kishimoto, Winands, Müller, Saito 2012, ICGA Journal 35(3) | §5.2 PN²/PDS-PN memory analysis; §6 df-pn DCG solutions; §7.3 threshold control; conclusion: "For larger problems df-pn may be the best choice"; future work: comprehensive variant comparison "sorely missing" | **Obtained** (Müller author copy; vendored `kishimoto2012_icga_survey.pdf`) | Survey, not a mechanism source; the classification backbone and the regime statement, used as corroborator |
| Othello/tsume-shogi PDS overhead | Sakuta & Iida 2001, *The performance of PN*, PDS and PN search on 6×6 Othello and Tsume-Shogi* (AG9) | Independent measurement of PDS's node-generation overhead (7–8× slower than PN, quoted via the chapter) | Not obtained | Secondhand-precise only (cited with that confidence level); consistent with the chapter's own LOA overhead figure |
| Recent PNS-variant applications (post-2012) | arXiv recent-first sweep (Čížek 2025, EWS 2024, GPN-MCTS 2023/25, AO*/PNS 2021, 7-in-a-row 2021) | No measured PDS/PN²-vs-df-pn comparison in any | Checked (`query_log.md` #7) | No new evidence; the ICGA-2012 future-work item independently confirms the gap |

## Selection

**Mined source: van den Herik & Winands, *Proof-Number Search and its
Variants* (chapter), §2–§8** — the one obtainable source covering all three
replacement-algorithm variants (PDS, PN², PDS-PN) with reproducible
pseudo-code, plus the df-pn head-to-head (its §7) and the explicit record of
the df-pn-vs-PDS-PN comparison gap. Head-to-head evidence table additionally
cites Pawlewicz & Lew 2007 §4 (primary ratios, in-repo), Winands 2002
(Tables 1–5), Sakuta & Iida 2001 (secondhand overhead figure), and the
ICGA-2012 survey (regime statement + missing-study confirmation).

Had this chapter not been obtainable, Phase 0 would have ended DEFER-leaning:
no other source contains both the pseudo-code and the comparisons
(Nagai 2002 and Breuker 1998 unobtainable; P&L 2007 already mined; the
survey is a catalogue without the full PDS rules).
