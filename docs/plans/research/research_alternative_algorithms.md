# Research: Whole-algorithm alternatives to DF-PN+ — PDS, PN², PDS-PN (`research` backlog #8)

Mined 2026-09-20 by `plan6.md` (literature target #8: PDS / PN²
algorithm-swap scaling vs DF-PN+). Selected in Phase 0 per
`measurements/plan6/shortlist.md`; query log in
`measurements/plan6/query_log.md`. Vendored sources:
`measurements/plan6/vanherik_winands_pnchapter.pdf` (primary),
`winands2002_pdspn.pdf`, `kishimoto2012_icga_survey.pdf`; the Pawlewicz &
Lew 2007 paper is vendored in-repo at `docs/plans/dfpn/epsilon.pdf`.

Primary source: **H. J. van den Herik, M. H. M. Winands, *Proof-Number
Search and its Variants* (chapter), §§2–8** — the only obtainable source
containing the precise PDS threshold rules (Eqs. (8)–(9)), the NegaPDS /
NegaPDS-PN pseudo-code, the PN² two-level construction (Eqs. (10)–(11)),
the complete LOA head-to-head tables (Tables 3–9), and the df-pn-vs-PDS
§7 (Pawlewicz & Lew's Table 10). Corroborators: **Pawlewicz & Lew 2007**
§4 (the primary quantified df-pn-vs-PDS data; already vendored in-repo —
mined for the 1+ε trick by `dfpn/research_epsilon.md`, which did **not**
cover the §4 PDS comparison), **Winands, Uiterwijk, van den Herik 2002**
(CG 2002; the chapter's §4–6 in shorter form, plus the GHI note and the
PN² memory curve), **Kishimoto, Winands, Müller, Saito 2012** (ICGA
Journal 35(3); §§5.2, 7.3, conclusion), and **Sakuta & Iida 2001**
(secondhand via the chapter's overhead remark). Nagai's 2002 dissertation
remains unobtainable (plan5 Phase 0; re-confirmed); its PDS content is
reproduced verbatim in the obtained sources.

Scope note: plan5 (`research_child_termination.md`) classified PDS's
*cutoff rule* (both thresholds exceeded simultaneously) as subsumed by the
threshold recursion. This extraction addresses only what plan5 explicitly
carved out: the **whole-algorithm structure** — PDS's per-level threshold
schedule, PN²'s two-level decomposition, and the scaling evidence for
swapping DF-PN+ outright.

## 1. Summary

- **PDS** (Nagai) is a depth-first AND/OR search that replaces df-pn's
  re-entry-driven threshold recursion with *multiple iterative deepening*:
  every interior node stores a (pn, dn) **threshold pair** (initialized to
  the node's own (pn, dn) at the start of each iteration), and the
  subtree is searched until *both* the node's pn and dn have reached their
  thresholds. A child's proof threshold is set to
  `max(proof, disproofMin(n))`, and per visit the selected child's own
  (pn or dn) counter is incremented by 1 — the direction chosen by
  Nagai's proof-like/disproof-like heuristic (Eqs. (8)–(9) below). Expanded
  nodes are stored in a TwoBig transposition table with their (pn, dn)
  bounds. In solver terms: **the same (pn, dn, threshold) recursion our
  frames implement, with the threshold schedule moved from the parent's
  re-entry arithmetic into a stored per-node counter incremented by 1 per
  visit**, plus *delayed evaluation* (children are generated but priced
  only when expanded).
- **PN²** (Allis, Breuker) is two stacked best-first PNS levels: the
  level-1 PN search calls a bounded level-2 PN search to evaluate the
  most-proving node. The level-2 tree may hold
  `y = min(x·f(x), N − x)` nodes in memory, where `x` is the level-1 tree
  size and `f(x) = 1/(1+e^{(a−x)/b})` is a logistic growth function with
  tuned parameters (a, b). Level-1 uses delayed evaluation, level-2
  immediate evaluation. In solver terms: **a best-first frontier held
  outside the transposition table, sized as a fraction of the search** —
  the mechanism by which plain PN's memory blow-up is tamed.
- **PDS-PN** (Winands et al. 2002) is the hybrid: PDS at level 1 (TT-backed,
  depth-first, memory-safe), plain best-first PN at level 2 (bounded by the
  same `y` formula, with `x` = TT occupancy); after the level-2 search
  completes, only its root's (pn, dn) are stored in the TT — the level-2
  tree is discarded and rebuilt on the next visit. In solver terms:
  **PDS frames whose unexpanded children are priced by a bounded,
  repeatedly-rebuilt best-first sub-search instead of (1,1)/TT-bounds**.

## 2. Background: the published forms

### 2.1 PDS's threshold schedule (chapter §2.4, §5.2)

- Per interior node `n` with parent `p`, at iteration start:
  `pnt(n) = pn(n)`, `dnt(n) = dn(n)`. The subtree search at `n` stops when
  `pn(n) ≥ pnt(n)` **and** `dn(n) ≥ dnt(n)` (or the node is (dis)proven) —
  the both-thresholds stopping condition plan5 classified as subsumed.
- Selection: the child with the lowest `max(disproof_child, proof)` is
  expanded (ties: lowest proof number) — chapter Fig. 6 `selectChild`.
- Increment direction — Nagai's proof-like heuristic. Interior OR node `n`
  with parent `p` is *proof-like* iff
  `pnt(p) > pn(p) AND (pn(n) ≤ dn(n) OR dnt(p) ≤ dn(p))` (Eq. (8));
  mirrored for AND nodes (Eq. (9)). Proof-like ⇒ increment the child's
  proof counter; else the disproof counter. At the root, the smaller of
  pn/dn is incremented each outer iteration.
- Engineering constants as published: delayed evaluation ("only expanded
  nodes are evaluated"); TwoBig TT; children generated before the loop
  ("avoid cycles" `putInTT(n)`); **GHI ignored** — chapter §2.4/§5.2:
  "dependent on its history a node can be a draw or can have a different
  value. However, in the current PDS algorithm we ignore this problem."
- The chapter states PDS "is a depth-first search algorithm but behaves
  like a best-first search algorithm. In most cases PDS selects the same
  node for expansion as PN search" — i.e., PDS and df-pn share the
  most-proving-node asymptotics (Nagai's equivalence result); the
  behavioral differences are the threshold schedule granularity, the
  delayed-evaluation policy, and the TT-stored per-node counters.

### 2.2 PN² and PDS-PN (chapter §§2.2, 5.3; ICGA-2012 §5.2)

- Level-2 node budget `y = min(x·f(x), N − x)` (chapter Eqs. (10)–(11);
  `a` = transition point, `b` = S-shape; PDS-PN's tuned (a,b) = (450K,
  300K) on LOA; PN² used (1800K, 240K) per Breuker). After a level-2
  search, children of its root are preserved (PN²) or discarded (PDS-PN
  stores only the root in the TT); subtrees are deleted either way.
- PN²'s memory behavior is the published weak point: with working memory
  halved repeatedly from 1M nodes, "the solving performance rapidly
  decreases for PN²" (chapter Fig. 7; Winands 2002 Fig. 3) — "PN² will
  not be able to solve really hard problems since it will run out of
  working memory". PDS-PN stays stable down to ~10K nodes because its
  level-1 is TT-backed.
- PDS-PN's cost over PDS: "PDS-PN searches 3.7 times more nodes than PDS
  but is still 3 times faster than PDS in CPU time … since the PN²-search
  tree is repeatedly rebuilt and removed" (chapter §6.2 Third Comparison) —
  and PDS-PN searched 1.4× more nodes than PN².

### 2.3 The df-pn-vs-PDS head-to-head (chapter §7; P&L 2007 §4)

Pawlewicz & Lew (2007) ran the only published direct comparison
(four implementations, same hardware/data structures, 3 GHz Pentium 4,
TwoBig TT 2²⁰ nodes, no game-specific enhancements):

- **Hard LOA, 286 positions** (P&L Table 3): solved within 30 min —
  df-pn+1+ε **286**, plain df-pn **285**, PDS+1+ε **278**, plain PDS
  **274**. Geometric mean of solving-time ratios (Table 4): df-pn+1+ε is
  **4.17× faster than PDS+1+ε and 4.46× faster than plain PDS**; plain
  df-pn is 2.64×/2.83× faster.
- **Easy LOA, 488 positions** (P&L Table 2): same ordering at every time
  limit (30 s: 468 / 457 / 430 / 425).
- **Atari Go 6×6, TT-size sweep 2¹²–2²²** (P&L Table 1): df-pn+1+ε fastest
  at *every* TT size; the advantage grows as the TT shrinks — in the
  "search space significantly exceeds the TT" regime the enhanced df-pn
  solved with 256-node and even 32-node TTs where PDS failed entirely
  below 2¹⁴–2¹⁶.
- The chapter's synthesis: "df-pn was solving the set of hard problems 4
  times faster than PDS … we may still conclude that df-pn is an
  interesting alternative to PDS-PN", and — the recorded gap — "Up to now
  there has not been a direct comparison between df-pn with PDS-PN using
  the same hardware and the same data-structure implementation."
- Regime statement (ICGA-2012 conclusion): "If sufficient memory is
  available, PNS or PN² may already be quite efficient in performing the
  job. For larger problems df-pn may be the best choice" — with
  future-work item (1) conceding that a comprehensive empirical variant
  study "is sorely missing". The tsume-shogi record reinforces this: the
  high-performance solvers cited (Nagai 2002, Kishimoto 2005/2010, Kaneko
  2010, Okabe 2005) are all df-pn-family; the two-level/best-first
  variants do not appear among them.
- PDS's overhead mechanism is quantified twice: the chapter measured PDS
  "generated nodes 7 to 8 times slower than PN²" in LOA, "in agreement
  with experiments performed in Othello and Tsume-Shogi [Sakuta & Iida
  2001]" — attributed to delayed evaluation (nodes generated ≫ nodes
  expanded). Note the reverse side: on **node counts** PDS builds the
  *smallest* trees of the LOA field (Table 8: PDS 498.5M vs PN² 1275.2M
  vs PDS-PN 1845.4M on the 457-position common subset).

### 2.4 What is *not* in the record

- No head-to-head includes repetition handling: the LOA comparisons run
  with threefold-repetition draws present but explicitly ignored by PDS
  ("we ignore this problem, since we believe that it is less relevant for
  the game of LOA"); no repetition-dominated domain was ever compared.
- No node-count-only df-pn-vs-PDS comparison exists (P&L report times;
  Winands reports nodes only against PN/PN²/PDS-PN, not df-pn).
- Nagai's Othello results (PDS faster than plain PN) are flagged by the
  chapter as an initialization artifact: Nagai's PN lacked the mobility
  init, which "is not fair"; PDS's child-count initialization *is* the
  mobility enhancement.

## 3. Head-to-head evidence table (normalized)

Winner = algorithm family favored; "resembles stress class" = deep
proof, TT much smaller than search tree, repetition handling load-bearing.

| # | Setting | Metric | Winner | Margin | Resembles stress class? |
|---|---------|--------|--------|--------|--------------------------|
| 1 | Atari Go 6×6 crosscut, TT 2¹²–2²² (P&L T1) | solving time | df-pn+1+ε | fastest at every TT size; ~9× over PDS at 2²⁰ (38 s vs 353 s); solves where PDS fails | **Yes** (tree ≫ TT); but no repetition rule in Atari Go |
| 2 | 488 easy LOA, TT 2²⁰ (P&L T2) | # solved vs time limit | df-pn+1+ε | 468 vs 425 (PDS) at 30 s | No (shallow) |
| 3 | 286 hard LOA, TT 2²⁰ (P&L T3/T4) | # solved / time ratio | df-pn+1+ε | 286 vs 274 at 30 min; 4.17× geomean vs PDS+1+ε | Depth class yes ("longer distance to the final position"); repetitions ignored by PDS |
| 4 | 488 LOA, 50M node limit (Winands 2002 T1 / ch. T3) | nodes & time on common subset | split: PDS fewest nodes, PN² fastest | PDS 118.3M nodes vs PN² 139.3M; PDS 6.2× slower | No (vs PN/PN², not df-pn) |
| 5 | 463-position subset (Winands 2002 T2 / ch. T4) | nodes / time | PDS nodes, PN² time | PDS 2.6× fewer nodes; PN² 3× faster | No (not df-pn) |
| 6 | 457-position subset, 50M limit (ch. T8) | nodes / time | PDS nodes; PN²/PDS-PN time | PDS 498M vs PN² 1275M vs PDS-PN 1845M nodes | No (not df-pn) |
| 7 | 286 hard LOA, 500M limit (Winands 2002 T5 / ch. T9) | # solved | PDS-PN | 276 vs 265 (PN²) — attributed to PN²'s memory wall | Depth class yes; but the winner's mechanism is memory-durability, not scaling |
| 8 | df-pn vs PDS-PN | — | — | **no published comparison** (chapter §7, explicit) | gap |
| 9 | Any repetition-dominated domain (tsume-shogi etc.) | — | — | **no PDS/PN²-vs-df-pn comparison exists**; ICGA-2012 concedes the comprehensive study is "sorely missing" | gap |

Every measured row either favors the df-pn family or compares PDS-family
algorithms against PN/PN² (never df-pn). No row favors PDS, PN², or
PDS-PN over df-pn in any regime, and rows 8–9 show the comparisons that
would matter most for this solver were never run.

## 4. Mapping to this solver

All sites verified by reading the files this session.

- **PDS.** The dfpn frame is already PDS's skeleton:
  `src/search/dfpn/core.rs::dfpn` carries `(th_pn, th_dn)` per frame and
  exits when both bounds reach them (`if (th_pn != INF && pn >= th_pn) ||
  (th_dn != INF && dn >= th_dn) { break }`) — PDS's stopping condition
  with OR semantics on the thresholds. The differences a PDS driver would
  introduce: (1) the child increment schedule — replace the re-entry
  arithmetic `(np, nd) = (min(th_pn, epsilon_ceil(second_pn)), …)` with
  PDS's `+1`-per-visit counters driven by the proof-like test
  (Eq. (8)/(9), needing the parent frame's `pn, dn, th_pn, th_dn`, all in
  scope at that site); (2) storing per-node thresholds in the TT instead
  of recomputing from the parent — `src/search/tt/` unsolved-bounds
  entries already store `(pn, dn)`; thresholds are path-independent
  derivatives, so no entry-shape change is forced; (3) delayed evaluation
  — `src/search/dfpn/children.rs::evaluate_all_children` /
  `evaluate_child` would stop pre-pricing the unselected tail (each
  child's (1,1) neutral init or TT probe is exactly what PDS defers).
  Selection (`src/search/dfpn/selection.rs::select_from_children` /
  `best_and_second_unsolved`) already implements min-disproof with
  tie-breaking; `is_solved_by_children` unchanged. **The mapping is
  real but small**: points (1)–(3) are the entire delta, and (1) is
  exactly the threshold-increment schedule space plan4 closed (δ>1,
  dynamic δ, 1+ε; +1 = the ε→0 corner already implemented as the
  fallback of `epsilon_ceil`), while (3) is the published 7–8×
  node-generation overhead.
- **PN² / PDS-PN.** No call-site mapping exists without breaking the
  architecture: the level-2 best-first frontier is a persistent
  non-TT structure (worker-like tree with parent pointers, deletion of
  solved subtrees, a second memory manager), which the search CLI's
  "RAM = TT only" contract and `Search`'s TT-only state forbid. In
  PDS-PN shape it would additionally sit inside the frame loop at
  `core.rs` (replacing the recursive `dfpn` call for unexpanded
  children) and rebuild per visit — the published 3.7× node inflation.

## 5. Contract check

| Contract | PDS | PN² | PDS-PN |
|---|---|---|---|
| GHI first-player-loss shortcut | Retrofit needed: published PDS **ignores GHI** (quoted above); its TT stores per-node (pn, dn) plus implicit threshold state — the first-player-loss shortcut and the never-cache-repetition-dependent-results rule would carry over unchanged in principle, but no published PDS implementation ever validated loop handling (LOA: "we ignore this problem") | Level-2 best-first tree caches subtree values across paths; the Breuker et al. 2001 GHI solution for best-first search exists (TCS 252) but is a separate proof-tree machinery | Same as PN² at level 2 |
| TT path-independence | Compliant with the existing guard set (bounds only, thresholds recomputed per visit) | n/a (level 2 is not TT-based) | Level 1 compliant; level 2 stores only the root |
| Per-run repetition cache | Unaffected | Unaffected at level 1; level-2 subtree evaluation must re-check path repetitions per descent (a best-first sub-tree has no path context — the exact problem the GHI best-first solution exists for) | Same |
| `ProofEvent` emission | Unchanged emission sites; PDS proves the same nodes | Same | Same |
| **RAM = TT only** | **Compliant** (depth-first + TT) | **Fatal**: level-2 frontier of `y = min(x·f(x), N−x)` nodes outside the TT; published memory-halving curves show solving collapse under restriction | **Fatal** at level 2 (same frontier, bounded but non-TT); level 1 compliant |
| Refinement rounds / `--refine-cap` / child-eval budget | PDS's per-node iteration is itself an iterative-deepening schedule; the bounded-round work caps would need re-deriving — the schedule space this interacts with was closed by plan4 (no depth/clock/regional structure) | The level-2 budget *is* the `y` formula — a second, independent work/memory knob to tune | Both problems |

## 6. Phase 2 classification

| Variant | Class | Reasoning |
|---|---|---|
| **PDS (whole algorithm)** | **(d) evidence-against** | The only two head-to-heads (Atari Go TT-sweep; hard LOA) favor df-pn by 2.6–4.5×, with the largest margins exactly in the "tree ≫ TT" regime that defines our stress class; and the swap's structural deltas are (1) the +1 threshold schedule — the ε→0 corner of the already-implemented `epsilon_ceil` (plan5: subsumed; plan4: schedule space closed), (2) delayed evaluation — the published 7–8× node-generation overhead, (3) TT-stored thresholds — a constant-factor layout change. No published result suggests a node-count win on our metric of record; the df-pn-vs-PDS node counts were never published, and the MPN-selection equivalence bounds any structural node-count difference to re-search granularity. |
| **PN²** | **(c) contract-breaking** | RAM = TT only is fatal: the level-2 best-first frontier is non-TT memory by construction; the published record independently shows PN² collapsing under memory restriction and the ICGA-2012 regime statement ("for larger problems df-pn may be the best choice") ruling it out for exactly our class. |
| **PDS-PN** | **(c) contract-breaking + (d) evidence-absent** | Level-2 frontier breaks the RAM contract; node inflation vs PDS 3.7×; and the one comparison that could justify it (vs df-pn, same hardware) has never been run — the chapter records the gap explicitly. Its win over PN² (Table 9) is a memory-durability effect, which our contract makes irrelevant by construction. |
| **DFPN-PN** (the chapter's own future-work suggestion: df-pn level 1 + PN level 2) | **(c) contract-breaking, unpublished** | Same level-2 memory problem; no data anywhere. |
| **df-pn+ with 1+ε** (the incumbent) | **(a) implemented** | `core.rs::epsilon_ceil`; mined at `dfpn/research_epsilon.md`; ε scheduling closed by plan4. |
| Two-level TT replacement schemes (context) | **(d) evidence-against** | ICGA-2012 §5.1: the shogi-community consensus is that two-level TTs "are ineffective for PNS variants" — corroborates the single-table + effort-based-preference design in `src/search/tt/`. |

**Verdict: no variant lands in class (b). H0 holds — backlog #8's
algorithm-swap question closes with the lock-in cost quantified: the
published record favors DF-PN+ in every measured regime, including the
ones resembling ours, and the comparisons that could overturn this (df-pn
vs PDS-PN; anything repetition-dominated) were never run.**

## 7. Sizing sketch (for the record — no POC proposed)

Had PDS survived as (b), the POC contract would be: env-gated alternative
driver (`ATOMIC_PDS=1` selecting a PDS-mode threshold schedule inside
`dfpn` — a ~100-line delta at the §4 sites: increment direction from
Eqs. (8)–(9), `epsilon_ceil` bypass, delayed tail evaluation behind a
flag), gate object = stress case first-outcome, controls = `m22_white`,
`dec13`, `dec10`, bit-identical gate-off anchor. Honest cost: **2–3
sessions**, above the usual 1–2-session POC bar — a second driver means a
second GHI/repetition surface to validate, and plan4's closure predicts
the schedule change alone is inert-to-negative. Not recommended; archived
with the (d) classification.
