# Research: Focused DF-PN (FDFPN) child-limit — Henderson 2010 (`research` backlog #5)

Mined 2026-09-20 by `plan5.md` (literature target #5: child-level early
termination in DF-PN). Selected in Phase 0 per `measurements/plan5/
shortlist.md`; full-text copies of the primary and corroborating sources are
vendored as `measurements/plan5/henderson2010_playing_solving_hex.pdf` and
`measurements/plan5/gao2017_fdfpncnn_ijcai17.pdf`.

Primary source: **P. T. Henderson, *Playing and Solving the Game of Hex*,
Ph.D. thesis, University of Alberta, 2010** — §5.2.2–5.2.5 (the MoHex solver's
Focused DFPN), Figures 5.2–5.5, Equation (5.2), Observations 1–3. Corroborating
secondary sources: Gao, Müller, Hayward, *Focused Depth-first Proof Number
Search using Convolutional Neural Networks for the Game of Hex* (IJCAI-17,
Eqs. (1)–(3) restate the rule externally); Kishimoto, Winands, Müller, Saito,
*Game-Tree Search Using Proof Numbers: The First Twenty Years* (ICGA Journal
35(3), 2012), §7.2 "Dynamic Widening" (three-sentence description of Yoshizoe
2008, the closed-access sibling of the same mechanism family). All paper
claims below cite the thesis's own section/figure numbering.

## 1. Summary

FDFPN (Focused Depth-first Proof Number Search) modifies df-pn so that a
frame considers only a bounded prefix of its statically ordered children: the
`SelectChild`, `ΔMin`, and `ΦSum` helpers iterate over the first `l` **live**
children instead of all children, where `l = base + ⌈fraction × |live
children|⌉`. Children proven lost for the frame's mover are pruned from the
live set (freeing limit budget to reveal the next child); children proven won
for the frame's mover end the frame. The intent is to eliminate work on weak
moves: a winning frame is proven from a subset of its children, and the
(dis)proof-number sums that drive threshold cuts and selection never see the
children beyond the limit. In solver terms it is a **count-based child-sweep
termination rule with subset-sum bounds**, distinct from (a) the threshold-
increment family (Nagai's δ > 1, dynamic δ, the implemented 1+ε trick) and
(b) sum-threshold-based sweep short-circuits.

## 2. Background: the rule as published

### 2.1 Problem addressed (§5.2.3)

"To identify one winning move with certain (dis)proof number bounds, PNS and
its variants must first show that all sibling moves — including weak moves —
cannot be solved with smaller (dis)proof number bounds." Henderson targets
Hex's near-uniform branching, where df-pn degenerates toward breadth-first
behavior in the opening. FDFPN "focuses its effort on the strongest moves,
thereby eliminating work on the weakest moves and dampening the breadth-first
search behaviour."

### 2.2 The rule (§5.2.3, Eq. (5.2), Figs. 5.2–5.3)

- Children of each node are ordered by a **domain-specific move-ordering
  function** (in MoHex: an augmented circuit-resistance evaluation, strengthened
  by inferior-cell and virtual-connection engines; Gao et al. 2017 later
  replaced it with a policy CNN).
- Initially only the first `l` children are **put in the search tree** ("the
  child limit"); unrevealed children are absent from all frame computations.
- `child limit = base + ⌈fraction × live children⌉` with `1 ≤ base`,
  `0 < fraction ≤ 1`; the limit must be ≥ 2 for any node with more than one
  live child (SelectChild needs the second-smallest δ for the threshold
  update). Figure 5.3 marks the changes: `MID` computes `l` and passes it to
  `SelectChild(n, l, …)`, `ΔMin(n, l)`, `ΦSum(n, l)`; the loop condition
  `n.φ > ΔMin(n,l) && n.δ > ΦSum(n,l)` and the selection therefore range over
  the first `l` live children only.
- **Frontier dynamics:** "Whenever a recursive MID call identifies a losing
  child, it prunes the corresponding move and recomputes the child limit."
  Each solved child either keeps the limit constant (introducing the next
  child) or reduces it by one; Figure 5.4 walks the update for
  `base = 1, fraction = 0.5` over six children. A proven-winning child solves
  the node without ever revealing the tail (Figure 5.4, last panel: "Node is
  solved without exploring the 6th move").
- **Stored bounds:** on frame exit, `n.φ ← ΔMin(n)`, `n.δ ← ΦSum(n)` — over
  the *revealed* children only (unrevealed children are never in the search
  tree), then `TTstore`. So an exited frame stores a bound computed from a
  subset of its move list.

### 2.3 What the source proves (§5.2.4)

- **Observation 1** (frontier persistence): a child within the limit at time
  `t` is within the limit at all `T > t` or is no longer live — revealed
  children are never silently dropped.
- **Observation 2** (the intended strength): if `x < n` children are proven
  losing before a winning child is found, then
  `max(0, n − x − b − ⌈f(n − x)⌉)` children **never became accessible**.
  With good ordering, a winning frame examines a strict subset of the
  children df-pn would examine and cannot exceed df-pn's bounds.
- **Observation 3** (the published hazard): if the first `x ≥ b + ⌈f·n⌉`
  children are losing (poor ordering), then at least
  `⌈(x − b − f·n)/(1 − f + ε)⌉` children must be **fully proven losing**
  before the frame can be solved — "a child limit that is too restrictive for
  the quality of the move ordering can force the solving of nodes that would
  normally remain unsolved in DFPN search, potentially imposing large
  inefficiencies." For losing frames generally, "the only difference between
  DFPN and FDFPN … is that the dis(proof) number bounds are different when
  some children are revealed and/or solved. Experimental results suggest that
  this does not cause any significant inefficiencies" — asserted for Hex,
  where the resistance ordering is strong.

### 2.4 Experimental setting (§5.2.5) — and where it differs from ours

Hex capture puzzles on 8×8–10×10 boards; FDFPN with `base = 1, fraction = 0.2`
takes < 60% of DFPN's time (best 55.6% at fraction 0.21); smaller `base`
dominates larger; **excessive pruning worsened solving times, even beyond
plain DFPN**; the optimal fraction increases with `base` (unexplained in the
source). Heuristic leaf initialization, by comparison, never got below 80% of
DFPN's time. Setting differences from this solver: Hex puzzles are shallow
win-proof problems with a highly tuned hand-crafted ordering and a search
tree that fits the TT budget (2²¹ TT entries in the Gao et al. runs); our
hard positions are 60+-ply conversion proofs at 14 M–250 M child evals where
the TT is many times smaller than the tree.

### 2.5 Follow-ups

- **Yoshizoe 2008 (dynamic widening)** — the same class: "only considers
  top-k or a portion of 1/k children nodes during the search", with
  correctness guaranteed "as results obtained by forward pruning were
  occasionally wrong" (Gao et al. 2017, §1–2). Canonical paper closed-access
  (Springer CG 2008 ch. 13; no OA copy — see `shortlist.md`).
- **FDFPN-CNN (Gao, Müller, Hayward, IJCAI-17)** — re-parameterizes the
  widening size as `l(s) = base + ⌈f(s) × |live children|⌉` with
  `f(s) = min{µ, 1 + vθ(s)}` (value CNN) and swaps the ordering for a policy
  CNN. The learned components are backlog #6 territory and out of scope here;
  the *cutoff rule* itself is unchanged and knowledge-free.
- Both sources frame FDFPN as strongest exactly when the ordering function is
  strong, and as fragile otherwise (Observation 3; Gao et al.: "FDFPN is
  fragile to its heuristic move ordering function").

## 3. Mapping to this solver

The solver's frame loop already has every ingredient FDFPN builds on, so the
mechanism maps onto concrete sites (all verified by reading the files):

- **Static child order exists.** `src/search/dfpn/core.rs` (`sort_moves` call
  at frame entry) orders the move list by `StaticAtomicScorer` + history +
  killer + TT-best before `evaluate_all_children` runs
  (`src/search/dfpn/history.rs::sort_moves`). FDFPN's "ordered moves"
  premise is present; its quality is the known weak lever.
- **The sweep.** `src/search/dfpn/children.rs::evaluate_all_children`
  evaluates **every** legal move once per frame (each child priced by
  `evaluate_child`: terminal fast paths, path-repetition draw, TT probe with
  unsolved-bounds reuse, or neutral (1,1) init), with a decisive-child early
  exit that fills the remaining entries as unexplored dummies. FDFPN's
  revelation map would replace this with: evaluate only the first
  `base + ⌈f·|moves|⌉` children; leave the tail unpriced.
- **The fold.** `src/search/dfpn/selection.rs::selection_for_child` folds the
  full table (`OR: pn = min pn, dn = Σ dn` saturating; AND mirrored);
  `is_solved_by_children` derives solved outcomes over the table;
  `best_and_second_unsolved` picks the re-search target. An FDFPN frame would
  fold only the first `l` unexplored children and keep the tail out of both
  the Σ and the selection.
- **The threshold cut.** `src/search/dfpn/core.rs`
  (`if (th_pn != INF && pn >= th_pn) || (th_dn != INF && dn >= th_dn) { break }`)
  exits the frame on the folded bound and stores it (unsolved bounds over the
  table, `store_best_child` index). Under FDFPN the same store would carry
  the subset Σ.
- **The re-search threshold.** `core.rs`'s `epsilon_ceil(second)` (the
  implemented 1+ε rule) already implements the first-child threshold
  recursion FDFPN leaves untouched — the two mechanisms compose on paper.

### 3.1 The structural finding that decides the verdict

In this solver the fold already prices **every** child once per frame visit:
the tail children that FDFPN would leave unrevealed each cost ~1 child eval
(TT-resolved or neutral init) and contribute their real initial bounds to the
Σ. Applying the child limit therefore has exactly two effects, and both are
inverted relative to the churn mass this initiative is attacking:

1. **Cuts fire later, not earlier.** The frame's `Σdn` (OR) threshold cut
   sums all children; excluding the tail lowers the Σ, so the
   `dn >= th_dn` cut — the exit that plan1 measured as ~80% of all
   descendant evals — is *delayed*, and the frame descends further into its
   in-limit best child before cutting. The backlog #5 question ("stop work on
   a child once its bound crosses a hopeless threshold") asks for the
   opposite direction: terminate *more* work, not reveal *less*.
2. **The stored bound loses the tail's real prices.** An FDFPN frame stores
   the subset Σ; the tail contributes nothing. On re-entry the parent sees a
   weaker bound and cannot cut immediately — the identical "missing
   full-sum information" failure that `conversion` report7 measured for the
   sum-based sweep short-circuit (787.8 M re-entry evals = 96.3% of the
   entire spike run on stress FO). Henderson's own frontier dynamic does not
   repair this here: his frontier advances when an in-limit child is
   *proven lost for the mover*, i.e. after a full child refutation — work our
   threshold recursion would defer via TT-stored partial bounds, and the
   tail's (1,1) pricing is what lets the parent cut at all.

FDFPN's win comes from Observation 2's regime: *winning* frames with
excellent ordering solve from a subset. Our profile is dominated by
threshold-cut (losing/drawn) frames, Observation 3's regime, where the
source itself predicts "large inefficiencies" — and our ordering quality is
unproven at exactly the depth FDFPN needs it.

## 4. Soundness check against our contracts

- **TT path-independence / repetition handling.** The limit set is a function
  of the static move order and the live-child set — both path-independent —
  and per-child classification (`evaluate_child`: commoner extinction,
  path-repetition, rule50, TT probe) is unchanged. No repetition-dependent
  fact would be cached: the mechanism only changes *which* children feed the
  fold. No conflict with the first-player-loss GHI shortcut or the per-run
  repetition cache.
- **Subset-solve hazard (new, but of a known shape).** With a subset fold,
  `is_solved_by_children`'s all-solved rule could "prove" a Draw or Loss from
  a partial table while unevaluated children exist — the exact hazard
  `conversion` plan7 solved with the all-solved-prefix guard (an early cut may
  fire only when the partial table contains at least one unsolved child). A
  mapped implementation inherits that guard verbatim; it is a known, handled
  shape, not a blocker.
- **`ProofEvent` neutrality.** Cut frames emit no `NodeProven`; a count-based
  cut changes when frames exit, not what they prove. Compliant.
- **Deferral asymmetry (`conversion` #6/#7 test).** **Fails.** A decisive
  child beyond the limit is invisible to the fold on every visit until enough
  in-limit children are fully refuted to advance the frontier; the
  unrestricted sweep prices every child each visit and its decisive-child
  early exit always finds later Loss children. This is Observation 3 — the
  source itself quantifies the forced extra solving — and it is the same
  asymmetry class that turned the sum-based short-circuit's stress win into a
  resource-cut draw (`conversion` report7: "the early cut can sit between the
  sorted children prefix and a decisive child later in the list").
- **RAM = TT only.** The live-children frontier state is per-frame
  (`ChildInfo` flags), not a new structure. Compliant — though note the
  mechanism's benefit case in Hex rests on a search tree small relative to
  memory, the opposite of our 14 M–250 M-eval regime.

## 5. Phase 2 classification of every surveyed mechanism

| Mechanism | Source | Class | Reasoning |
|---|---|---|---|
| 1+ε second-child threshold (`epsilon_ceil`) | Pawlewicz & Lew 2007 | **(a) implemented** | `core.rs` threshold recursion; mined at `dfpn/research_epsilon.md`; ε scheduling closed by plan4 |
| δ > 1 / dynamic-δ threshold increments (df-pn+, Kishimoto 2005) | Nagai 1999a/2002; ICGA-2012 survey §7.3 | **(d) subsumed** | δ = avg of children's *heuristic-init* values degenerates to 1 without heuristic initialization (backlog #6); with init it is a sibling-statistics-driven member of the increment family whose schedule space plan4 closed (no depth/clock/regional structure); 1+ε is its multiplicative generalization already implemented |
| **FDFPN child limit** | Henderson 2010 §5.2.3 | **(d) structurally equivalent to the closed partial-sum sweep lever** | Same store shape (subset Σ), same deferral asymmetry (Observation 3), same re-entry-churn mechanism (`conversion` report7: weaker stored bounds → 96.3% of run in re-sweeps); and inverted relative to our churn mass — delays threshold cuts instead of accelerating them (§3.1 above) |
| Dynamic widening (top-k / 1/k children) | Yoshizoe 2008 (closed access) | **(d)** | Same class as FDFPN with the identical subset-Σ store and frontier dynamics; classified by the Henderson-based structural argument; additionally fails selection criterion 2/3 (no obtainable pseudo-code) |
| Correlated-sibling representative counting | Seo 1995 (tsume-shogi); Kaneko 2004 / Miwa 2004 (ML) | **(c) unsound here** | Soundness rests on a domain-proven equivalence between "highly correlated" children (interposing piece drops); atomic chess has no cheap verifiable equivalence class, and treating unverified children as one *is* the deferral asymmetry baked into the Σ; ML variants are backlog #6 |
| Heuristic-threshold PNS ("likely win/loss" leaves) | Schaeffer et al. 2005 | **(c) not viable here** | Breaks no repetition/TT contract (final th=∞ pass is exact), but requires a heuristic evaluator with calibrated win/loss thresholds; the dependency class is the same one that closed EWS (`conversion/research_ews.md`: the win-rate estimator is the transfer blocker) — backlog #6/#7, not an independent child-termination mechanism |
| Threat-space search / dual λ / LDFPN | Allis 1994; Soeda 2006; Yoshizoe et al. 2007 | out of scope (per plan) | Backlog #7 (recognizers) overlap; algorithm-integration framing, not a df-pn-internal cutoff |
| df-pn(r) / TCA / stall-threshold bumping | Kishimoto & Müller 2003; Kishimoto 2010; Nagai 2010 | **(a) covered by a different mechanism** | Targets cyclic pn/dn overestimation (infinite loops), not churn; the premise is prevented here by the first-player-loss GHI shortcut, unsolved-bounds reuse guard, and rule50-in-Zobrist node identity (`dfpn/research_ghi*.md`); the bounded cross-path verification lever it feeds is already closed (`dfpn` backlog #4) |
| Early terminal detection | ICGA-2012 survey §7.4.4 | **(a) implemented** | Existence-query classification in `evaluate_child` (no movegen for decisive classification) |
| Kawano simulation / Killer-Tree Heuristic | Kawano 1996; Tanase 2000 | out of scope | Proof-borrowing across similar positions = `conversion` backlog #2 (Kawano already cited); killer-tree reuse is an ordering lever (`lean` #10) |
| PN², PDS-PN two-level, PDS-as-swap | Breuker 1994; Winands 2002; Nagai 2002 | out of scope (per plan) | Backlog #8 (algorithm swaps); no isolated child-level cutoff rule beyond the families above |

**Verdict: no mechanism lands in class (b). H0 holds; the surface is closed.**

## 6. Sizing sketch (for the record — no POC proposed)

Had a class-(b) mechanism survived, the POC contract would be: env-gated
child-limit knob on `evaluate_all_children`/`selection_for_child` (temporary
`solve_stats.rs`-style runner per the plan6/plan7 pattern), gate object =
stress case first-outcome, controls = `m22_white`, `dec13`, `dec10`,
bit-identical gate-off anchor, temporary instrumentation to split
threshold-cut churn into few-heavy-children vs. many-children-once-each
(plan1's open child-level question, which would also settle whether any
child-granularity mechanism could ever pay here). Expected cost 1–2 sessions.
`conversion` report7's skipped-children histograms (OR cuts skipping 16–31
children, AND 8–15) already lean toward the many-light tail, which no
count-based limit helps. **Not recommended; archived with the (d)
classification above.**
