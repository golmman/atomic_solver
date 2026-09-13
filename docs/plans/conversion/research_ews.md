# Research: Expected Work Search (`conversion` backlog #5a)

Mined 2026-09-13 by `plan3.md` (docs-only reading round). Source: O. Randall,
M. Müller, T.-H. Wei, R. Hayward, *Expected Work Search: Combining Win Rate
and Proof Size Estimation*, [arXiv:2405.05594v1](https://arxiv.org/abs/2405.05594)
(submitted 2024-05-09; 9-page manuscript, §1–7 plus Appendix A). This is the
only public version at mining time; no corrections or retractions were found
in a check of the paper's public citation record. Section references below
are to the paper's own numbering. This is also the paper EWS cite in the CG
community as of 2026 (per `initiative.md` backlog #5a).

Headline results claimed by the paper: first solution of the empty 5×5 Go
board under the *positional superko* ruleset (a repetition-dominated ruleset;
5.25 h, 2.6 B nodes, §5.2.1) and empty 8×8 Hex in under 4 minutes with
domain-specific knowledge (§5.3).

## 1. The algorithm in the paper's terms

EWS is a **best-first** framework in the MCTS mold — selection, expansion,
backpropagation over an in-memory tree (§3) — not a depth-first threshold
search. It is designed for **binary** outcomes ("our implementation of EWS
solves binary outcome problems", §2; multi-outcome "could be solved using
multiple searches or extended bookkeeping"). Every node stores a win-rate
estimate `WR(n)` and two Expected Work estimates `EW_loss`, `EW_win`.

### 1.1 The Expected Work definition (§3.1)

For a node `X` with children `C_0 … C_{n-1}` in current search order, with
`WR(Y)` the estimated probability that `Y` is a win (EWS uses a negamax
win/loss-from-mover formulation):

```
EW_loss(X) := Σ_i        EW_win(C_i)                                    (1)
EW_win(X)  := Σ_i (EW_loss(C_i) · Π_{j<i} WR(C_j))                      (2)
```

Reading: a losing node must solve *all* children (Eq. 1 — the PNS AND-node
sum), while a winning node solves children in order until the first child
proven lost for the opponent; the `Π_{j<i} WR(C_j)` factor is the estimated
probability that the search *reaches* child `C_i` at all, i.e. that no
earlier child was already a refutation (Eq. 2). With `WR ≡ 0` for every
child, Eq. 2 degenerates to `min_i EW_loss(C_i)` — the paper notes this is
exactly the PNS OR-node optimistic minimum (§3.1).

Three stated assumptions (§3.1): (i) win rates correlate with the position
actually being won/lost; (ii) the child ordering stays fixed between
re-computations (violated in practice — EWS reorders continually); (iii)
sibling win rates / EW values are independent — "a strong assumption,
violated for example in a DAG", defended only empirically (§5).

### 1.2 Selection, expansion, backpropagation (§3.2–3.4)

Selection descends from the root **always to the first-ordered child** until
a leaf is reached (no exploration term at all). Expansion adds all
non-terminal children, evaluating each new leaf's win rate and EW by
heuristics (§3.3). Backpropagation removes children solved as wins, declares
a loss when no unsolved children remain, re-sorts the unsolved children by

```
A < B  ⟺  EW_loss(A)/(1 − WR(A)) < EW_loss(B)/(1 − WR(B))              (3)
```

and recomputes `WR` and both EW values (§3.4). Appendix A proves that
ordering (3) minimizes `EW_win` at every node, by an exchange argument
(swap two adjacent children; the difference factors into
`p·(1−WR(A))·(1−WR(B))·(f(A)−f(B)) > 0`). The proof requires
`EW_loss ≥ 0` and `0 < WR < 1` strictly (their `wins=1, visits=2`
initialization guarantees the latter, §4.1).

Exploration is achieved purely by **optimistic EW initialization** (§4.2):
a new leaf's EW is set from the rollout used for its win-rate estimate,
`EW(X) := Σ_{i<m} b(P_i)` — the sum of branching factors `b(·)` over the `m`
positions visited by the random rollout. Underestimating new nodes makes
them grow during early visits (the PNS-analogue of optimistic `pn/dn = 1`).

### 1.3 Where the win rate comes from (§4.1)

The paper's implementation uses **random rollouts** (wins initialized to 1,
visits to 2). It explicitly does *not* prescribe the estimator: "It is
possible to use other methods of win rate estimation with EWS such as a
heuristic policy to guide simulations, or a machine learned evaluation
function" (§4.1, citing AlphaZero-style evals and Proof Cost Networks
[31]). The paper proves nothing about estimator calibration; assumption (i)
of §3.1 is argued, not established.

### 1.4 Generic refinements (§4.3)

Domain-agnostic machinery: transposition table of solved outcomes with
Zobrist hashing + enhanced transposition cutoffs; symmetry reduction;
relevancy zones (Shih et al. AAAI-21) for move pruning; a conjectured
winner halves the bookkeeping; and the 1+ε trick (Pawlewicz & Lew) "if the
search is observed to oscillate rapidly among a small number of children".

**GHI handling (§4.3, the part relevant to this initiative):** "we implement
the GHI solution proposed by Kishimoto et al. [11]. Transposition entries
are simulated to ensure that the stored outcome can be reached legally
within the rules of the game before they are used to replace search."
`[11]` is the AAAI-04 Kishimoto–Müller paper (same reference as
`dfpn/research_ghi.md`). So: **everything solved is cached**, and
cross-path reuse is *verified per use* by simulation rather than restricted
by construction. The paper reports no repetition-specific cost profile and
no ablation isolating the GHI machinery; the only repetition-cost data point
it cites is inherited background — MIGOS solving 4×4 Go jumped from 14.8 s
(Japanese rules) to ~1.1 h (situational superko), more than two orders of
magnitude (§2).

## 2. The 5×5 Go positional-superko experiment (§5.2.1) — what actually won

The headline question of `plan3.md`: they solved a repetition-dominated
game — did the win come from selection, from caching, or from the
estimators?

- **Setup:** empty 5×5 Go, positional superko (forbids repeating *any*
  previous board, regardless of mover), 24.5 komi, first-player full-board
  win. Solved in 5.252 h / 2,605,781,360 nodes, naively parallelized as 6
  processes over the 6 symmetrically unique replies to the center opening
  (§5.2.1). Baselines (Go-Solver αβ on Fuego, EWS-WR, EWS-PS) all exceeded
  24 h.
- **The ablations answer the question.** EWS-WR replaces Eq. 2 with
  `EW_win := min_i EW_loss(C_i)` and sets all `WR ≡ 0` — the paper calls
  the result "a negamax PNS algorithm which prioritizes positions closest
  to being solved" (§5.2). EWS-PS replaces the ordering with UCT
  (MCTS-Solver-like). On 5×5 Go: full EWS 5.25 h; **both ablations » 24 h**
  (Table 2). On the 600-position 6×6 Go dataset, EWS's average solve time
  is 9.5× better than Go-Solver, 8.5× better than EWS-WR, with EWS-PS the
  fastest per-position but the most timeouts (Table 1, Fig. 2).
- **Conclusion:** the win is attributable to the *combination* — the
  win-rate-weighted expected-work selection fed by calibrated rollouts —
  not to anything in the caching layer. The caching layer is standard
  (TT + Kishimoto–Müller simulation verification + ETC + symmetry +
  relevancy zones). Nothing in the paper changed *what is cacheable*; it
  pays for cross-path reuse of solved results with per-reuse simulation.
- **Hex cross-check (§5.3):** without Hex knowledge EWS needs 93.9 M nodes
  for 6×6; with basic virtual-connection knowledge it takes 26 nodes for
  6×6 and solves 8×8 in < 4 min. The paper's own reading: domain knowledge
  dominates at scale. There is no Hex repetition regime (Hex is acyclic).

## 3. Threshold interaction: does EWS sit inside df-pn or replace it?

EWS **replaces** df-pn threshold descent entirely. There are no `(pn, dn)`
thresholds: the descent always follows the first-ordered child (§3.2),
re-search is replaced by re-sorting (§3.4), and exploration/termination are
governed by optimistic EW initialization rather than threshold budgets. The
only contact point with df-pn machinery is cosmetic — the 1+ε trick is
listed as an anti-oscillation refinement *inside* EWS's reordering loop
(§4.3), and EWS-WR is characterized as "negamax PNS" (§5.2).

There is therefore no reading of EWS as a pure ordering/selection layer on
top of our existing child evaluation: adopting it means abandoning the
depth-first threshold skeleton (`src/search/dfpn/core.rs` `dfpn` loop,
`selection.rs::select_from_children`, the ε-threshold arithmetic of
`core.rs::epsilon_ceil`) in favor of a best-first in-memory tree — which is
exactly the memory profile DF-PN exists to avoid (Nagai's motivation;
`dfpn/research_epsilon.md` §Background). The paper's own experiments keep
whole proof trees in memory (§5.2.1's 2.6 B-node tree is per-process
in-memory state; the 6-way parallelization was needed to fit it).

## 4. What this means for this solver

### 4.1 Transfer blocker: the win-rate estimate

`atomic_solver` is a pure solver: no evaluation function, no rollouts
(`AGENTS.md` architecture; `src/search/ordering.rs` `StaticAtomicScorer` is
a move-*ordering* scorer, not a position evaluation). EWS's benefit is
concentrated in the `WR` term of Eq. 2/3 — the paper's own ablation
isolates it: removing WR (EWS-WR) costs 8.5× on average (§5.2, Table 1) and
turns 5×5 Go from 5.25 h into » 24 h. Evaluating the degenerate alternatives
named in `plan3.md`:

- **`WR ∝ 1/pn`-style surrogates:** any monotone surrogate with
  `WR → 1` for near-proven children collapses the `Π_{j<i} WR(C_j)`
  products in Eq. 2 to ~0 after the first plausible child, reducing
  `EW_win` to (approximately) `min_i EW_loss(C_i)` — i.e. the EWS-WR
  degenerate form the paper itself uses as its PNS baseline. This is not a
  discovered shortcut; it is the measured-weak end of the paper's own
  ablation.
- **AND-side "defense-survival" WR from `StaticAtomicScorer`:** this would
  be our invention, not the paper's — §4.1 offers only rollouts, learned
  policies, or learned evals, and §3.1's assumption (i) (WR correlates with
  truth) is exactly what an uncalibrated static scorer cannot establish.
  On top of that, the AND-side surface where a WR-flavored ordering signal
  could act was measured nearly empty by `conversion` plan4 (`report4.md`):
  high-clock AND frames are 0.011–0.15 % of nodes, the refuting child is
  already at rank 0 in 152/153 (FO) and 817/818 (default) of proven
  high-clock AND frames.

### 4.2 The repetition angle: their regime pays, ours refuses

The structural contrast with our solver, and the answer to the headline
question of the plan:

- EWS **caches everything** (solved outcomes in a TT) and handles GHI by
  **simulation-verified reuse** (§4.3) — the Kishimoto–Müller scheme mined
  in `dfpn/research_ghi.md`. In their regime, "what is cacheable" is
  decided *at reuse time by verification*, not by construction.
- Our solver made the opposite choice after plan5's false win
  (`dfpn/research_repetition_cache.md` §4): repetition-dependent draws are
  never cached as solved TT facts; the plan9 repetition cache
  (`src/search/dfpn/repetition_cache.rs`) reuses only exact
  `(position, ancestor-set)` contexts where the reuse is a semantic
  identity — no simulation, no cross-context reuse.
- EWS therefore offers **no new mechanism for what is cacheable**. Its
  regime demonstrates that unrestricted caching + per-reuse verification
  can win on a repetition-dominated game — which is evidence for the lever
  already parked as `dfpn` backlog #4 (bounded cross-path verification over
  a TT snapshot), whose reference implementation is the *journal* GHI
  algorithm (`conversion` backlog #5d) — not for anything EWS-specific.
  Notably, their positional-superko experiment did not change their
  cacheability rules relative to their non-superko experiments; the same
  TT + simulation stack serves both.

### 4.3 Initiative-constraint check

The `conversion` initiative's structural constraint (`initiative.md`
Motivation, "Why plain PNS variants are not the answer"): a candidate
mechanism must change *what is cacheable* or *where the line comes from* —
or, if it touches thresholds/child selection, state why the plan10 failure
mode (folding solved mass into unsolved parents' `(pn, dn)`,
`dfpn/report10.md`) does not apply.

- **What is cacheable:** unchanged by EWS (§2 above).
- **Where the line comes from:** changed — but the line source is a
  win-rate estimator the solver does not have, and every estimator-free
  surrogate collapses to the EWS-WR ablation (measured 8.5× worse) or to
  unproven invention (§4.1).
- **Plan10 hazard:** structurally N/A — EWS has no `(pn, dn)` thresholds
  to pollute; solved children are *removed* from the child list rather
  than folded into bounds (§3.4). The hazard test passes, but vacuously:
  the mechanism does not plug into the existing search at all.

### 4.4 Verdict and summary row

**No-go** for opening a backlog item, on three independent grounds:

1. **Transfer blocker (paper-fact):** the mechanism's measured benefit
   requires a win-rate estimator; the paper's own ablation shows the
   estimator-free degenerate form (EWS-WR ≡ negamax PNS) is the weak
   configuration (8.5× average, » 24 h on 5×5 Go).
2. **Structural constraint:** EWS changes neither cacheability nor the
   line source in a way usable by a pure solver; the cacheability lesson
   it does demonstrate (verify-at-reuse) is already owned by `dfpn` #4 /
   #5d.
3. **Effort class:** adoption is a best-first rewrite (L) that discards
   the depth-first skeleton, the plan9 repetition cache, and the measured
   ordering stack, to chase a benefit whose precondition we cannot supply.
   The ordering-layer salvage (EW formula as a `sort_moves` signal) is
   blocked by `lean`'s OR-side oracle-floor measurement and plan4's empty
   AND-side surface.

| Mechanism | Lives in | Cacheable? | Line source? | Thresholds? | Plan10 hazard | Effort | Verdict |
|---|---|---|---|---|---|---|---|
| EWS: EW-minimizing best-first selection, `EW_loss=Σ EW_win(C)`, `EW_win=Σ EW_loss(C)·Π WR`, order by `EW_loss/(1−WR)` (§3.1, 3.4, Eq. 1–3) | Replaces `src/search/dfpn/core.rs` + `selection.rs` + `children.rs` (whole framework, not a layer) | No — TT + GHI-simulation exactly as before (§4.3) | Yes — but source is a WR estimator the pure solver lacks (§4.1); surrogates collapse to measured-weak EWS-WR (§5.2) | Eliminated, not modified (best-first, no thresholds) | N/A (no `(pn,dn)` folding exists; solved children are removed, not folded) | L | **No-go** — no estimator (transfer blocker); constraint not met; ordering salvage blocked by `lean` oracle floor / plan4 |

### 4.5 What remains valuable from this mining

- A sharpened framing for `dfpn` backlog #4 / `conversion` #5d: the only
  published repetition-dominated solve of this scale won by
  **caching-everything + simulation-verified reuse**, i.e. the journal-GHI
  lever, not by selection. (Caveat: their game is Go with positional
  superko, where repetition legality depends on the full history — the
  first-player-loss regime our shortcut implements is the *other* branch
  of the same GHI paper, `dfpn/research_ghi.md` §1.)
- A documented negative result closing `conversion` backlog #5a with a
  paper-grounded reason (the estimator), stronger than the pre-existing
  structural suspicion in `initiative.md` backlog #5.
- Corroboration that the 1+ε trick (already implemented, `core.rs::
  epsilon_ceil`) is standard machinery across paradigms (EWS §4.3 lists it
  as a drop-in refinement).
