# Plan 6 — Phase 0 query log (bounded survey, 2026-09-20)

Queries run per `plan6.md` Phase 0 (time-boxed, no deep reads). Endpoints:
direct fetches of the plan5-recorded author-copy URLs, Bing web search,
arXiv API (recent-first), in-repo vendored PDFs. Semantic Scholar returned
HTTP 429 (rate-limited, no key); DuckDuckGo returned empty shells.

| # | Query / endpoint | Result relevant to #8 |
|---|------------------|-----------------------|
| 1 | Direct fetch `dke.maastrichtuniversity.nl/m.winands/documents/PDSPNCG2002.pdf` | **Obtained**: Winands, Uiterwijk, van den Herik 2002, *PDS-PN* (CG 2002), 15 pp. — NegaPDS/NegaPDS-PN pseudo-code (appendix), Tables 1–5 (PN, PN², PDS, αβ, PDS-PN head-to-heads on LOA), the explicit GHI note ("in the current PDS algorithm this problem is ignored"), and the PN² memory-restriction experiment (Fig. 3). Vendored: `winands2002_pdspn.pdf`. |
| 2 | Direct fetch `dke.maastrichtuniversity.nl/m.winands/documents/pnchapter.pdf` | **Obtained**: van den Herik & Winands, *Proof-Number Search and its Variants* (chapter), 31 pp. — extended version of #1 plus PDS's proof-like/disproof-like threshold heuristics (Eqs. (8)–(9)), the step-by-step PDS example, and **Section 7: the df-pn vs PDS head-to-head** (Pawlewicz & Lew's Table 10 geometric-mean ratios) with the explicit statement that no direct df-pn vs PDS-PN comparison exists. Vendored: `vanherik_winands_pnchapter.pdf`. |
| 3 | Direct fetch `webdocs.cs.ualberta.ca/~mmueller/ps/ICGA2012PNS.pdf` | **Obtained**: Kishimoto, Winands, Müller, Saito 2012, ICGA Journal 35(3), 26 pp. — §5.2 (PN²/PDS-PN memory analysis), §7.3 (threshold control: δ>1, dynamic δ, 1+ε), conclusion ("If sufficient memory is available, PNS or PN² may already be quite efficient. For larger problems df-pn may be the best choice.") and future-work item (1): a comprehensive empirical comparison of PNS variants "is sorely missing". Vendored: `kishimoto2012_icga_survey.pdf`. |
| 4 | In-repo check: `docs/plans/dfpn/epsilon.pdf` | **Already vendored**: Pawlewicz & Lew 2007, *Improving Depth-first PN-Search: 1+ε Trick*, 12 pp. §4 contains the primary head-to-head data (Atari Go TT-size sweep Table 1; 488 easy + 286 hard LOA solved-counts Tables 2–3; geometric-mean speed-ratio Table 4). Mined in `dfpn/research_epsilon.md` for the trick — the §4 PDS comparison was **not** mined there (the extraction records only the ε recommendations). |
| 5 | Bing: Pawlewicz Lew "Improving Depth-first PN-Search" pdf | Shell results only (JS-rendered); superseded by #4 (in-repo copy). |
| 6 | Semantic Scholar API: same title | HTTP 429 (rate-limited). Superseded by #4. |
| 7 | arXiv API `all:"proof number search"` (recent-first, 40) | No new head-to-head source. Recent hits all already tracked or out of scope: Čížek 2025 (parallel only, plan5 full-text-checked), EWS 2024 (mined, `conversion/research_ews.md`), GPN-MCTS 2023/2025 (play, not proofs), "On AO*, PNS and Minimax" 2021 (theory), 7-in-a-row 2021 (application, no variant comparison). |
| 8 | Nagai 2002 dissertation availability (re-check) | Confirms plan5's Phase 0 record: not obtainable (U-Tokyo repository serves the 2002 equivalence paper only). PDS's algorithm structure covered via sources #1–#3 (NegaPDS pseudo-code is reproduced verbatim in #1 and #2 from Nagai [18/12]). |
| 9 | Breuker 1998 *Memory versus Search in Games* / PN² papers | Not separately fetched: PN²'s algorithm and all published PN² measurements are fully contained in sources #1–#3 (Tables 3–9 + Fig. 3/7 memory curves, Breuker's logistic growth function Eq. (1)/(10)); the chapter supersedes the individual papers for this plan's question. |
| 10 | Post-2012 solver papers using PDS/PN² with measured df-pn comparisons | None found (#7; search engines blocked). The ICGA-2012 survey's future-work item (1) independently confirms no comprehensive variant comparison exists anywhere. |

## Selection

**Mined source: van den Herik & Winands, *Proof-Number Search and its
Variants* (chapter), Sections 2–7.** It is the only obtainable source that
contains, in one place: the precise PDS threshold rules (Eqs. (8)–(9) and the
NegaPDS pseudo-code), the PN² two-level construction with the logistic growth
limit (Eqs. (10)–(11)), the complete LOA head-to-head tables (PN, PN², PDS,
αβ, PDS-PN; Tables 3–9 + memory curves), and the df-pn-vs-PDS Section 7 with
the Pawlewicz & Lew ratios. Criterion 4 holds: plan5 cited the chapter but
mined Henderson 2010; `dfpn/research_epsilon.md` mined the 1+ε trick, not the
PDS comparison. Evidence-table corroborators: Pawlewicz & Lew 2007 §4
(in-repo `docs/plans/dfpn/epsilon.pdf`), ICGA-2012 survey §§5–9, Winands
2002 CG paper (overlapping with the chapter).

## Availability summary

Obtained and vendored under `measurements/plan6/`:
`winands2002_pdspn.pdf`, `vanherik_winands_pnchapter.pdf`,
`kishimoto2012_icga_survey.pdf`. In-repo already: `docs/plans/dfpn/epsilon.pdf`
(Pawlewicz & Lew 2007). Recorded unobtainable: Nagai 2002 dissertation
(plan5 Phase 0; re-confirmed), Breuker 1998 dissertation (superseded by the
chapter for this question), Sakuta & Iida 2001 (AG9; cited secondhand via the
chapter's PDS node-generation-overhead remark).
