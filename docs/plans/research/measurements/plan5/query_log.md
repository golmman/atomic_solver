# Plan 5 — Phase 0 query log (bounded survey, 2026-09-20)

Queries run (plan-defined plus follow-ups on the two live clusters). arXiv
API, DuckDuckGo Lite, Semantic Scholar API, Unpaywall API; PDFs fetched
directly. Time-boxed: no deep reads during Phase 0.

| # | Query / endpoint | Result relevant to #5 |
|---|------------------|-----------------------|
| 1 | arXiv API `all:"proof number search"` (relevance, 40) | 10 hits; none on child-level termination (complexity, AO*, MCTS hybrids, parallel — all out of scope). |
| 2 | `depth-first proof number search child cutoff threshold` (web) | Generic hits; no new mechanism. |
| 3 | `PDS proof number disproof number search Nagai threshold` (web) | → PDS-PN paper (Winands, Uiterwijk, van den Herik) and chessprogramming/Wikipedia surveys. |
| 4 | `Winands PDS-PN new proof-number search algorithm pdf` | **Obtained**: `dke.maastrichtuniversity.nl/m.winands/documents/PDSPNCG2002.pdf` (CG 2002) + `pnchapter.pdf` ("Proof-Number Search and its Variants" chapter, van den Herik & Winands). |
| 5 | `Kishimoto Winands Müller Saito depth-first proof number search ICGA Journal pdf` | **Obtained**: "Game-Tree Search Using Proof Numbers: The First Twenty Years", ICGA Journal 35(3), 2012 — `webdocs.cs.ualberta.ca/~mmueller/ps/ICGA2012PNS.pdf`. §7 is the mechanism catalogue. |
| 6 | `partial expansion proof number search` (web) | No PNS-relevant hits (noise). |
| 7 | `selective expansion proof number search df-pn` (web) | Kaneko parallel df-pn, "Deep df-pn and Its Efficient Implementations" (Springer, closed), Čížek 2025 (already tracked). |
| 8 | `Yoshizoe proof number search dynamic widening 2008 pdf` | Yoshizoe 2008 "A New Proof-Number Calculation Technique for Proof-Number Search" (CG 2008, Springer ch. 13): **closed access** — Semantic Scholar `openAccessPdf: CLOSED`, Unpaywall `is_oa: None`, no OA locations. First hit led to the LDFPN paper (λ-search — excluded per plan, backlog #7 overlap). |
| 9 | `Nagai df-pn algorithm AND/OR trees dissertation pdf` | Nagai dissertation not clearly obtainable (U-Tokyo repository serves the 2002 equivalence-proof paper, not the dissertation); PDS cutoff rule available via sources #4/#5 instead. |
| 10 | `Kishimoto 1974 github publications` (author page) | Open IJCAI/AAAI PDFs only; "Deep df-pn" not open. |
| 11 | `Henderson 2010 MoHex solver df-pn widening factor hex thesis pdf` | **Obtained**: P. T. Henderson, *Playing and Solving the Game of Hex*, Ph.D. thesis, UAlberta 2010 — open at `ualberta.scholaris.ca` (item 8d2ae989). Contains the full FDFPN treatment (§5.2.3–5.2.5, Eq. 5.2, Observations 1–3, pseudocode Figs. 5.2–5.3, tuning Fig. 5.5). |
| 12 | (from #5's §7.2 pointer) `webdocs.cs.ualberta.ca/~hayward/papers/fdfpnscnnhex.pdf` | **Obtained**: Gao, Müller, Hayward, "Focused Depth-first Proof Number Search using CNNs for Hex" (IJCAI-17). Documents FDFPN's rule externally (Eq. (1)) and the CNN re-parameterization (Eqs. (2)–(3), learned — backlog #6, out of scope). |
| 13 | arXiv Čížek 2025 (2511.10339v2) full text grep | No child-level cutoff/widening content (parallel-layer only). Confirms no new mechanism in the already-tracked source. |

PDFs vendored under `measurements/plan5/`: `henderson2010_playing_solving_hex.pdf`
(#11), `gao2017_fdfpncnn_ijcai17.pdf` (#12). Others referenced by URL.
