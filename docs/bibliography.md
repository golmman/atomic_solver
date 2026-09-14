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
  Trick*. Warsaw University. — Implemented in the solver. **Mined**
  (`dfpn/research_epsilon.md`).
- C. Gao (2021). *On Computation Complexity of True Proof Number Search*.
  [arXiv:2102.04907](https://arxiv.org/abs/2102.04907). — True pn/dn in
  arbitrary DAGs is NP-hard; framing for why DAG-aware pn/dn must remain
  heuristic. **Open** (`conversion` backlog #5e).

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
  Kaneko-10's scaling assumptions. **Open** (`conversion` backlog #4/#5c).

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
