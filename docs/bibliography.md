# Bibliography

Index of literature referenced by the solver's plans and initiatives. This
is a pointer file, not a summary: the deep-dive extractions live in the
initiative directories as `research_*.md` (created by the plan that mines
the paper — do not preemptively write one). When a plan mines an entry,
update its **Status** here.

Status values:

- **mined** — a full extraction exists; the link points to it.
- **cited** — used inside a plan/report without a dedicated extraction.
- **open** — identified as relevant, not yet mined; the feeding backlog
  item is named.

## Proof-number search foundations

- L. V. Allis, M. van der Meulen, H. J. van den Herik (1994). *Proof-Number
  Search*. Artificial Intelligence 66(1). — Original PNS. **Cited**
  (`dfpn/research_parallel.md`).
- A. Nagai (2002). *Df-pn Algorithm for Searching AND/OR Trees and Its
  Applications*. Ph.D. dissertation, University of Tokyo. — DF-PN and
  DF-PN+; source of the PDS variant. **Cited**
  (`dfpn/research_ghi.md`, `dfpn/research_epsilon.md`).
- J. Pawlewicz, L. Lew (2007). *Improving Depth-first PN-Search: 1 + ε
  Trick*. Warsaw University. — Implemented in the solver. §4 is the primary
  df-pn-vs-PDS head-to-head (Atari Go TT-size sweep; 488 easy + 286 hard LOA;
  df-pn+1+ε 4.17× faster than PDS+1+ε); the 1+ε extraction covers the trick
  only. Copy vendored: `docs/plans/dfpn/epsilon.pdf`. **Mined**
  (`dfpn/research_epsilon.md`; §4 PDS comparison mined by
  `research/research_alternative_algorithms.md`).
- C. Gao (2021). *On Computation Complexity of True Proof Number Search*.
  [arXiv:2102.04907](https://arxiv.org/abs/2102.04907). — True pn/dn in
  arbitrary DAGs is NP-hard; framing for why DAG-aware pn/dn must remain
  heuristic. **Open** (`conversion` backlog #5e).

## PNS variants and child-level termination

- H. J. van den Herik, M. H. M. Winands. *Proof-Number Search and its
  Variants* (chapter). — Survey of PN, PN², PDS, df-pn (threshold formulas
  (3)–(6), 1+ε as Eq. (7)); PDS's proof-like/disproof-like rules (Eqs. (8)–(9))
  and NegaPDS/NegaPDS-PN pseudo-code; PN² construction (Eqs. (10)–(11)); LOA
  comparison tables (Tables 3–9) and §7's df-pn-vs-PDS ratios with the recorded
  df-pn-vs-PDS-PN comparison gap. Author copy:
  `dke.maastrichtuniversity.nl/m.winands/documents/pnchapter.pdf`; copy
  vendored: `docs/plans/research/measurements/plan6/vanherik_winands_pnchapter.pdf`.
  **Mined** (`research/research_alternative_algorithms.md` — backlog #8
  closed H0: no PDS/PN²-family variant beats df-pn in any published
  comparison; PDS (d), PN² (c, RAM), PDS-PN (c+d)).
- M. H. M. Winands, J. W. H. M. Uiterwijk, H. J. van den Herik (2002).
  *PDS-PN: A New Proof-Number Search Algorithm — Application to Lines of
  Action*. CG 2002. — Two-level PDS + best-first PN; Tables 1–5 head-to-heads;
  PDS's GHI is explicitly ignored ("in the current PDS algorithm this problem
  is ignored"); PN² memory-degradation curve. Author copy:
  `dke.maastrichtuniversity.nl/m.winands/documents/PDSPNCG2002.pdf`; copy
  vendored: `docs/plans/research/measurements/plan6/winands2002_pdspn.pdf`.
  **Cited** (`research/research_child_termination.md`; corroborating source
  in `research/research_alternative_algorithms.md`).
- M. Sakuta, H. Iida (2001). *The Performance of PN*, PDS and PN Search on
  6×6 Othello and Tsume-Shogi*. Advances in Computer Games 9. — Independent
  measurement of PDS's node-generation overhead (7–8× slower than PN);
  cited secondhand via the PNS-variants chapter (not obtained). **Cited**
  (`research/research_alternative_algorithms.md`).
- M. H. M. Winands, J. W. H. M. Uiterwijk, H. J. van den Herik (2002).
  *PDS-PN: A New Proof-Number Search Algorithm — Application to Lines of
  Action*. CG 2002. — Two-level PDS + best-first PN; no child-level cutoff
  rule beyond PDS's. Author copy:
  `dke.maastrichtuniversity.nl/m.winands/documents/PDSPNCG2002.pdf`.
  **Cited** (`research/research_child_termination.md`).
- A. Kishimoto, M. H. M. Winands, M. Müller, J.-T. Saito (2012). *Game-Tree
  Search Using Proof Numbers: The First Twenty Years*. ICGA Journal 35(3).
  — Survey; §7 catalogues the PNS-variant enhancement space (heuristic init,
  correlated siblings, dynamic widening, threshold control, heuristic
  threshold, simulation, early terminal detection). Conclusion: "For larger
  problems df-pn may be the best choice"; a comprehensive variant comparison
  "is sorely missing". Author copy:
  `webdocs.cs.ualberta.ca/~mmueller/ps/ICGA2012PNS.pdf`; copy vendored:
  `docs/plans/research/measurements/plan6/kishimoto2012_icga_survey.pdf`.
  **Cited** (`research/research_child_termination.md`; corroborating source
  and regime statement in `research/research_alternative_algorithms.md`).
- P. T. Henderson (2010). *Playing and Solving the Game of Hex*. Ph.D.
  dissertation, University of Alberta. — Source of Focused DFPN (FDFPN):
  child limit `l = base + ⌈fraction × |live children|⌉`, frontier dynamics,
  Observations 1–3 (§5.2.2–5.2.5). Copy vendored:
  `docs/plans/research/measurements/plan5/henderson2010_playing_solving_hex.pdf`.
  **Mined** (`research/research_child_termination.md`) — classified as
  structurally equivalent to the closed partial-sum sweep lever for this
  solver; backlog #5 closed.
- K. Yoshizoe (2008). *A New Proof-Number Calculation Technique for
  Proof-Number Search*. CG 2008 (Springer LNCS 5131, ch. 13,
  DOI 10.1007/978-3-540-87608-3_13). — Dynamic widening (top-k / 1/k
  children) with correctness guarantee. Closed access; no OA copy found
  (Semantic Scholar/Unpaywall checked 2026-09-20). **Cited**
  (`research/research_child_termination.md`).
- C. Gao, M. Müller, R. Hayward (2017). *Focused Depth-first Proof Number
  Search using Convolutional Neural Networks for the Game of Hex*. IJCAI-17.
  — Restates FDFPN's child-limit rule externally (Eq. (1)); CNN-driven
  widening size and policy ordering (learned components = backlog #6
  territory). Author copy vendored:
  `docs/plans/research/measurements/plan5/gao2017_fdfpncnn_ijcai17.pdf`.
  **Cited** (`research/research_child_termination.md`).

## GHI / repetitions

- A. Kishimoto, M. Müller (2004). *A General Solution to the Graph History
  Interaction Problem*. AAAI-04. — PDF in repo: `docs/plans/dfpn/ghi.pdf`.
  **Mined** (`dfpn/research_ghi.md`).
- A. Kishimoto, M. Müller (2005). *A Solution to the GHI Problem for
  Depth-First Proof-Number Search*. Information Sciences 175(4), pp.
  296–314. [DOI 10.1016/j.ins.2004.04.012](https://doi.org/10.1016/j.ins.2004.04.012)
  (author copy vendored: `dfpn/ghi_journal.pdf`, from
  `www.cs.ualberta.ca/~mmueller/ps/kishimoto-mueller-infsci-ghi.pdf`). —
  Journal version of the above: complete literature review, df-pn
  pseudo-code, Theorems 3.1/3.2 with proofs, DUP/SIM/NOCYCLE ablation. The
  soundness theorems assume a sound verification oracle; no step-by-step
  simulation procedure and no ancestor-context carrying — the remaining
  implementation-level source is Kishimoto's 2005 Ph.D. dissertation (open
  access, UAlberta repository). **Mined** (`dfpn/research_ghi_journal.md`).
  The bounded cross-path verification lever it feeds is closed as an
  evidence-based no-go (`dfpn` backlog #4); the extracted soundness contract
  lives on as a design constraint for the parallel-search spike
  (`conversion` backlog #4 / `lean` #2).
- Y. Kawano (1996). *Using Similar Positions to Search Game Trees*. Games
  of No Chance, MSRI. — Kawano simulation; the plan5 twin mechanism and
  `conversion` backlog #2 (root-scale candidate verification) both derive
  from it. **Cited** (`dfpn/research_ghi.md`).
- A. Kishimoto (2005). *Correct and Efficient Search Algorithms in the
  Presence of Repetitions*. Ph.D. dissertation, University of Alberta. —
  **Cited** (`dfpn/research_parallel.md`).
- A. Saffidine, T. Cazenave (2012). *Multiple-Outcome Proof Number
  Search*. ECAI 2012, pp. 708–713.
  [DOI 10.3233/978-1-61499-098-7-708](https://doi.org/10.3233/978-1-61499-098-7-708)
  (author copy: `lamsade.dauphine.fr/~cazenave/papers/mopns.pdf`). —
  Formal multi-outcome PNS (`G/S` effort numbers per outcome,
  attracting/distracting descent, `pess/opti` interval pruning). Earlier
  misattributed to "Kishimoto, IJCAI-11" — no such paper exists; corrected
  during plan3. **Mined** (`conversion/research_mopns.md`).

## Parallelism

- T. Kaneko (2010). *Parallel Depth First Proof Number Search*. AAAI-10. —
  PDF in repo: `docs/plans/dfpn/parallel.pdf`. **Mined**
  (`dfpn/research_parallel.md`).
- A. Saffidine, N. Jouandeau, T. Cazenave (2011). *Solving Breakthrough
  with Race Patterns and Job-Level Proof Number Search*. ACG 13. —
  Job-level parallel PNS. **Open** (`conversion` backlog #4).
- K. Young, R. B. Hayward (2016). *A Reverse Hex Solver*. CG 2016.
  [arXiv:1707.00627](https://arxiv.org/abs/1707.00627). — Scalable parallel
  DF-PN in practice (Solrex). **Open** (`conversion` backlog #4).
- T. Čížek, M. Balko, M. Schmid (2025). *Massively Parallel Proof-Number
  Search for Impartial Games and Beyond*.
  [arXiv:2511.10339](https://arxiv.org/abs/2511.10339). — Two-level
  parallelization + shared worker info; 333× on 1024 cores; supersedes
  Kaneko-10's scaling assumptions. Full-text check (plan5): no child-level
  cutoff/widening content — parallel-layer only. **Open** (`conversion`
  backlog #4/#5c).

## Recent solving paradigms

- O. Randall, M. Müller, T.-H. Wei, R. Hayward (2024). *Expected Work
  Search: Combining Win Rate and Proof Size Estimation*.
  [arXiv:2405.05594](https://arxiv.org/abs/2405.05594). — Work-minimizing
  selection; solved 5×5 Go under positional superko (repetition-dominated)
  and 8×8 Hex. **Mined** (`conversion/research_ews.md`; no-go for a pure
  solver — the win-rate estimator is the transfer blocker).

## Adjacent (play, not exact solving)

- J. Kowalski, D. J. N. J. Soemers, S. Kosakowski, M. H. M. Winands
  (2025). *Generalized Proof-Number Monte-Carlo Tree Search*.
  [arXiv:2506.13249](https://arxiv.org/abs/2506.13249). — PNS-biased MCTS
  for move decisions, not proofs; background only. **Cited**
  (`conversion/initiative.md` Motivation).

## Domain tools

- Fairy-Stockfish (F. Fichter) — variant-capable alpha-beta engine with
  NNUE support; the intended line oracle for `conversion` backlog #2.
  Not literature; listed because the backlog depends on it.
