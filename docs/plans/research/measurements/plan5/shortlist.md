# Plan 5 — Phase 0 shortlist and selection

Selection criteria (from `plan5.md`): (1) child/subtree-granularity early
termination in the PNS/DF-PN family, exact and sound, no learned heuristics;
(2) reproducible rule/pseudo-code; (3) obtainable full text; (4) not already
mined in a `research_*.md`; (5) applicability to a repetition-handling,
TT-backed solver arguable before the deep read.

## Shortlist table

| Cluster | Source | Mechanism (one sentence) | Availability | Why (not) selected |
|---|---|---|---|---|
| df-pn cutoff variants | Nagai 2002 dissertation (PDS) | Multiple-iterative-deepening thresholds per node; child thresholds = current node numbers, incremented when proof-like/disproof-like | Not clearly obtainable (U-Tokyo repository serves the 2002 equivalence paper only) | PDS's cutoff rule precisely documented in the obtained secondary sources instead (#4/#5); algorithm-swap is backlog #8; the rule itself maps to the threshold-increment family |
| df-pn cutoff variants | Winands, Uiterwijk, van den Herik 2002, *PDS-PN* (CG 2002) | Two-level: PDS first level + best-first PN second level; threshold assignment per PDS | Obtained (Maastricht author copy) | Second level is a best-first PN²-style layer — algorithm-swap territory (#8); no child-level cutoff rule beyond PDS's |
| df-pn cutoff variants | van den Herik & Winands, *Proof-Number Search and its Variants* (chapter) | Survey: PN, PN², PDS, df-pn formulas (3)–(6), 1+ε as Eq. (7) | Obtained (Maastricht author copy) | Confirms df-pn threshold recursion + 1+ε are the entire child-level surface it knows; both already implemented/mined here |
| df-pn cutoff variants | Kishimoto, Winands, Müller, Saito 2012, *Game-Tree Search Using Proof Numbers: The First Twenty Years* (ICGA Journal 35(3)) | Survey §7: heuristic init, correlated-sibling counting, dynamic widening, threshold control (δ>1, dynamic δ, 1+ε), heuristic-threshold PNS, simulation, early terminal detection, df-pn(r)/TCA | Obtained (Müller author copy) | Not selected as the single mined source (survey, not a mechanism source), but used as the classification backbone — its §7 enumeration is the evidence that the surveyed child-level surface is closed |
| threshold-increment family | Nagai 1999a/2002 df-pn+ (δ>1); Kishimoto 2005 (dynamic δ = avg of children's heuristic-init values); Pawlewicz & Lew 2007 (1+ε) | Child threshold raised by δ over the second-best sibling bound | Mixed (survey-precise) | 1+ε implemented (`core.rs::epsilon_ceil`); dynamic δ degenerates to δ=1 without heuristic initialization (backlog #6); ε scheduling closed by plan4 |
| partial/selective expansion | **Yoshizoe 2008**, *A New Proof-Number Calculation Technique for PNS* (CG 2008, Springer ch. 13) | Dynamic widening: compute pn/dn from top-k (or 1/k) children only; correctness guaranteed by revelation dynamics, unlike naive forward pruning | **Closed access** (Springer; no OA copy — Semantic Scholar/Unpaywall checked) | Criterion 3 fails; mechanism describable only via the survey's three sentences and Gao 2017's summary — classified by equivalence to the mined FDFPN rule instead |
| partial/selective expansion | **Henderson 2010**, *Playing and Solving the Game of Hex*, Ph.D. thesis, UAlberta — **SELECTED** | Focused DF-PN: child limit `l = base + ⌈fraction × live children⌉`; SelectChild/ΔMin/ΦSum iterate over the first l live children; proven-lost children pruned, limit recomputed | **Obtained, open** (UAlberta repository; vendored: `measurements/plan5/henderson2010_playing_solving_hex.pdf`) | Meets all five criteria: exact and learning-free, full pseudocode (Figs. 5.2–5.3) + Observations 1–3 + tuning data, original source of the mechanism, not previously mined, mapping arguable |
| partial/selective expansion | Gao, Müller, Hayward 2017, *Focused DF-PN using CNNs for Hex* (IJCAI-17) | External precise statement of FDFPN's Eq. (1); CNN-driven widening size `l(s) = base + ⌈min(µ, 1+vθ(s))·|live|⌉` | Obtained (Hayward author copy; vendored) | The cutoff rule is knowledge-free but the paper's contribution is learned (policy/value NNs = backlog #6); used as corroborating secondary source, not the mined source |
| correlated siblings | Seo 1995 (tsume-shogi df-pn solvers); also Kaneko et al. 2004, Miwa et al. 2004 (ML variants) | At AND/OR nodes count only one representative of k "highly correlated" children in the Σ | Not obtained (Japanese/primary sources); rule precise only via ICGA-2012 survey | Premise is a domain-dependent equivalence (interposing piece drops); no verifiable atomic-chess analogue — unsound here without one |
| solver-engineering cutoffs | Tanase 2000 Killer-Tree Heuristic (applied to LOA in Sakuta et al. 2003) | Reuse proof trees of similar positions for move ordering (−20% nodes for PDS in LOA) | Survey-precise only | Ordering lever, not child-level termination; ordering is `lean` #10 territory |
| heuristic-threshold PNS | Schaeffer et al. 2005 (checkers) | Do not expand leaves with heuristic score beyond threshold th; iterate th upward; exact only in the final th=∞ pass | Survey-precise | Requires a heuristic evaluator with proven-bound properties — no such component exists here; dependency class = backlog #6/#7 |
| λ-search family | Allis threat-space search; Soeda et al. 2006 dual λ; Yoshizoe et al. 2007 λdf-pn (LDFPN paper obtained) | Threat-sequence restriction of move sets inside df-pn | LDFPN obtained | Excluded per plan (backlog #7 overlap: recognizers), unless framed as a df-pn-internal cutoff — LDFPN frames it as an algorithm integration |
| cyclic-graph control | Kishimoto & Müller df-pn(r); Kishimoto 2010 TCA; Nagai 2010 threshold bumping on stall | Ignore old same-distance children at AND/OR Σ; raise thresholds when no new leaves expand | Survey-precise (+ open Kishimoto 2005 dissertation) | Targets the infinite-loop/overestimation problem, not churn; the premise (cyclic pn/dn overestimation) is already prevented here by the first-player-loss GHI shortcut + unsolved-bounds reuse guard |

## Selection

**Mined source: Henderson 2010 (FDFPN child-limit), §5.2.2–5.2.5.** The
mechanism is the one surveyed candidate that is (i) a genuine child-sweep
termination rule, (ii) exact and knowledge-free, (iii) documented with
reproducible pseudocode and analysis, (iv) obtainable in full text, and
(v) not previously mined. Yoshizoe 2008 — the other candidate in its class —
is closed-access; its mechanism is covered by classifying it against the
FDFPN extraction.

Evidence base for the extraction: `measurements/plan5/`
`henderson2010_playing_solving_hex.pdf` (§5.2.3 pp. 59–62, §5.2.4 pp. 62–63,
§5.2.5 pp. 63–64, Figures 5.2–5.5), `gao2017_fdfpncnn_ijcai17.pdf` (§1–2,
Eqs. (1)–(3)), ICGA-2012 survey §7.2–7.3 (URL in `query_log.md`).
