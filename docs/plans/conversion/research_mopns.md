# Research: Multiple-Outcome Proof Number Search (`conversion` backlog #5b)

Mined 2026-09-13 by `plan3.md` (docs-only reading round).

> **Bibliographic correction (found during this mining).** The initiative and
> `docs/bibliography.md` attributed MOPNS to "A. Kishimoto, IJCAI-11". That
> paper does not exist. The paper is **A. Saffidine, T. Cazenave, *Multiple-
> Outcome Proof Number Search*, ECAI 2012, pp. 708–713, DOI
> 10.3233/978-1-61499-098-7-708** (author copy:
> `lamsade.dauphine.fr/~cazenave/papers/mopns.pdf`; OpenAlex knows exactly
> one work with this title, and the complete IJCAI-11 proceedings index
> contains no such paper — the only Kishimoto IJCAI-11 entry is
> "Evaluations of Hash Distributed A\*" with Kobayashi and Watanabe). The
> EWS paper (`research_ews.md`) also cites it as Saffidine & Cazenave, ECAI
> 2012. All paper claims below cite the ECAI-12 paper's own numbering
> (§1–6, Figures 1–7, Propositions 1–5, Tables 1–3).

Terminology is reused from `dfpn/research_ghi.md` (GHI, twin, simulation,
base/twin entries, first-player-loss) rather than redefined; the GHI
background there applies verbatim to this paper's cyclic-graph discussion
(§6).

## 1. The framework in the paper's terms

### 1.1 Setting (§2)

Two-player zero-sum finite games with a **linearly ordered outcome set**
`O = {o_1 < … < o_m}` (Max's preference; Min's is reversed). The paper
assumes games that are "finite, acyclic, sequential and deterministic"
(§2) — repetitions and path-dependent outcomes are **out of scope by
assumption** (see §4 below). Solving = computing the minimax value
`real(n) ∈ O`. Chess/draughts/Connect Four are named as three-outcome
games; Woodpush has `2×S×(S+1)` outcomes.

### 1.2 Effort numbers (§4.1–4.2)

Where PNS carries two numbers per node, MOPNS carries **`2m` effort
numbers**: for each outcome `o`,

- `G(n, o)` — estimated node expansions needed to prove `real(n) ≥ o`
  ("greater number");
- `S(n, o)` — estimated node expansions needed to prove `real(n) ≤ o`
  ("smaller number").

A node is solved with value `o` exactly when `G(n,o) = S(n,o) = 0`.
Backup rules (Figure 3a) are the PNS sum/min applied per outcome, with the
AND/OR roles flipped between the two families:

```
Max node:  G(n,o) = min_c G(c,o)     S(n,o) = Σ_c S(c,o)
Min node:  G(n,o) = Σ_c G(c,o)       S(n,o) = min_c S(c,o)
```

Terminal nodes (Figure 3b): `G(n,o) = 0` for all `o ≤ real(n)` and
`S(n,o) = 0` for all `o ≥ real(n)` — a terminal solved at value `v` has a
*staircase* of zeros across the outcome axis, not a single `(0, ∞)` pair.
Leaves initialize all numbers to 1 (mobility initialization discussed in
§4.6).

**Monotonicity (§4.3):** if `o_i ≤ o_j` then `G(n,o_i) ≤ G(n,o_j)` and
`S(n,o_i) ≥ S(n,o_j)` — the better the outcome, the harder to prove "≥ it",
the easier to prove "≤ it". Two propositions pin the permanent (0/∞) values
together:

- **Prop. 1:** `G(n,o) = 0 ⟹ ∀o' < o: S(n,o') = ∞` (and dual).
- **Prop. 2:** `G(n,o) = ∞ ⟺ S(n,o) = 0` within a node (and dual) —
  proving "cannot reach ≥ o" is the same fact as "is ≤ o".

### 1.3 Descent policy (§4.4)

The **attracting outcome** `o*(n)` of a node is the unproven outcome
minimizing `G(n,o) + S(n,o)` (the cheapest outcome still to prove); the
**distracting outcome** `o'(n)` is the adjacent outcome the opponent would
deviate toward. The root descent (Algorithm 1) picks `o* = o*(root)` once
per descent, then descends choosing at each Max node the child minimizing
`G(c, o*)` and at each Min node the child minimizing `S(c, o')` — prove the
attracting outcome against the mover, disprove it against the defender.
**Prop. 3:** for finite two-outcome games MOPNS develops exactly the PNS
tree (`G(n,Win) = PN(n)`, `S(n,Lose) = DN(n)`).

### 1.4 Pruning (§4.5)

Each node carries interval bounds `pess(n) = argmax{o : G(n,o) = 0}` and
`opti(n) = argmin{o : S(n,o) = 0)}`, with `pess(n) ≤ real(n) ≤ opti(n)`
(the Score-Bounded-MCTS bounds). These propagate into **relevancy bounds**
`α/β` exactly like alpha-beta windows (`α(n) = max(α(f), pess(n))` etc.).
**Prop. 4:** `β(n) ≤ α(n)` ⟹ the subtree of `n` can be discarded. **Prop.
5:** the descent path is never prunable (`α(n) < o* ≤ β(n)`), so pruning
never changes the number of descents — it only frees memory of off-path
subtrees, "crucial as lack of memory is one of the main bottlenecks of PNS
and MOPNS".

### 1.5 Depth-first variant and classical improvements (§4.6)

The paper sketches, without experiments: PN²-style nested searches;
mobility initialization (`S = |children|` at the mover's nodes); a **depth-
first MOPNS** via Nagai's transformation with "two threshold numbers … one
threshold for the greater number of the current attractive outcome at the
root and one for the smaller number of the distractive outcome"; plus
Nagai's garbage collection, **Kishimoto & Müller's GHI solution**, and the
1+ε trick as drop-in refinements.

### 1.6 Experiments (§5)

Best-first MOPNS vs best-first PNS, **no transposition detection** in the
prototype, acyclic testbeds only (Connect Four, Woodpush). Protocol favors
PNS: for each position only PNS's two best runs (the ones that prove the
final value) are counted — no binary search overhead.

| Suite | MOPNS nodes | PNS nodes | MOPNS time | PNS time | Fewer-nodes wins |
|---|---|---|---|---|---|
| Connect Four 4×5, 256 pos | 16,947,536 | 20,175,238 | 99 s | 85 s | 227 vs 13 |
| Connect Four 5×5, 625 pos | 1,557,490,694 | 1,757,370,222 | 11,230 s | 9,055 s | 406 vs 140 |
| Woodpush (8,3), 99 pos | 31,328,178 | 34,869,213 | 718 s | 702 s | 76 vs 23 |
| Woodpush (13,2), 256 pos | 155,756,022 | 174,285,199 | 4,796 s | 4,573 s | 205 vs 51 |

Reading: MOPNS creates **fewer nodes on most positions** (the two-outcome
wins are identical by Prop. 3) but is **not faster in wall time** in any
aggregate — the per-node cost of maintaining `2m` numbers and re-sorting
outweighs the shared-tree saving at these outcome counts.

### 1.7 Cyclic graphs (§6, footnote 3)

The conclusions state the theory holds for DAGs with adapted relevancy
bounds, and that "the problems encountered by MOPNS in cyclic graphs are
similar to that of PNS and DFPN in cyclic graphs. Fortunately, it should be
straightforward to adapt Kishimoto and Müller's ideas [7] from DFPN to a
depth-first version of MOPNS." Footnote 3 observes that chess with the
50-move rule is a DAG but "this rule is usually abstracted away, resulting
in a cyclic structure". `[7]` is the **journal** GHI paper (Information
Sciences 175(4), 2005) — not the AAAI-04 version.

## 2. Draw semantics in MOPNS

- **Draw as first-class outcome:** a draw is simply an interior element of
  the ordered outcome set. A node is *proven drawn* when
  `G(n,Draw) = 0 ∧ S(n,Draw) = 0`: the mover can force at least a draw
  (some child proven `≥ Draw`, since `G(Max,·) = min_c G(c,·)`) *and* the
  mover cannot win (all children proven `≤ Draw`, since
  `S(Max,·) = Σ_c S(c,·)`). Draw children contribute to the parent's
  numbers through the same per-outcome sum/min as any other outcome — no
  special-casing.
- **Draw vs the two-outcome view:** the pn/dn pair we run is the projection
  `G(·,Win)`/`S(·,Lose)` (Prop. 3). In that projection a proven draw is
  *invisible*: `G(n,Win) > 0` and `S(n,Lose) > 0` — a draw node is an
  unsolved node with finite numbers. Making draws visible requires the
  `G/S` pair **at the Draw threshold**, which is exactly the machinery our
  ad-hoc `Outcome::Draw` propagation replaces with explicit solved-outcome
  bookkeeping.
- **Path-dependent draws (repetitions): not handled.** The framework
  assumes acyclicity (§2); footnote 3 acknowledges the 50-move rule creates
  cycles and notes it is "usually abstracted away"; §6 defers cyclic graphs
  wholesale to Kishimoto & Müller. **There is no monotonicity or validity
  argument for caching repetition-dependent results anywhere in the paper.**
  The published counterpart to `conversion` backlog #1's self-made lemma
  does not exist here; the citation trail points at the journal GHI paper
  (`conversion` #5d) as the only candidate source.

## 3. Our draw machinery against the paper

Walking the solver's draw path (`plan3.md` §Research 2 item 3):

- **Terminal order** (`src/search/dfpn/children.rs::evaluate_child`):
  commoner-extinction branches, then (if `rule50 < 100`) path-repetition →
  `Draw` with `repetition_seen = true`, then rule50 expiry, then TT probe,
  then the existence-query classification (moves-empty → checkmate/stalemate
  via the checkers bit; `occupied == 2` → draw). MOPNS says nothing about
  terminal precedence; our order encodes the game-rule precedence
  (checkmate/stalemate end the game before rule50 can) and is a
  solver/domain concern, not a framework gap.
- **Draw proof rule** (`src/search/dfpn/selection.rs::
  is_solved_by_children`): a parent is `Draw` iff all children are solved,
  none is a winning child, and at least one is a draw child. **This is
  exactly MOPNS's `G(n,Draw) = 0 ∧ S(n,Draw) = 0` condition**: "all
  children proven `≤ Draw` and some child proven `≥ Draw`" — under our
  negamax perspective flip (`Outcome::pn_dn_for`), "no winning child" is
  "all children ≤ Draw" and "some draw child" is "some child ≥ Draw".
  Our propagation is the **two-outcome slice of MOPNS evaluated at the
  Draw threshold**, expressed inductively over solved children instead of
  as effort numbers. No case exists that MOPNS handles and we handle
  differently at the level of provable outcomes.
- **`suppress_draw` + repetition cache** (`src/search/dfpn/core.rs`
  store site; `src/search/dfpn/repetition_cache.rs`): repetition-dependent
  draws are stored unsolved `(1,1)` in the TT and as per-search
  `(tt_key, context_hash) → depth` entries outside it. MOPNS has no
  counterpart — it never reaches this question (acyclic assumption), and
  §6 hands cyclic graphs to GHI. Our design is therefore **not a special
  case of MOPNS and not a deviation from it**; it lives in the space MOPNS
  explicitly delegates to the GHI literature.
- **`repetition_seen` propagation** (`selection.rs` tie-breaks,
  `core.rs` suppress condition): flags which draw proofs consumed a
  path-repetition edge. MOPNS's permanent-value invariants (Props 1–2) play
  the analogous role of keeping contradictory facts out of a node; our
  unsolved-bounds degeneracy guard (`children.rs`: reuse only when
  `pn > 0 && dn > 0` and depth-usable) is the practical counterpart on the
  TT side. Consistent, no gap.
- **Multiple draw sources:** we deliberately do not distinguish
  stalemate / `occupied == 2` / rule50 / repetition draws in the
  `Outcome` type — only the `repetition_seen` flag marks path-dependence.
  Rule50 draws stay TT-cacheable because the halfmove clock is part of the
  Zobrist key (`src/zobrist.rs`), making the clock-expired node a distinct
  tree node. MOPNS footnote 3 is a published endorsement of this modeling
  choice (keep the clock, don't abstract the rule away into cycles).

Concrete differences found, with position classes:

1. **Draw-vs-draw comparisons (PV only).** Our selection picks the
   *longest* draw child (`selection.rs::is_solved_by_children`,
   `draw_depth` rule, with a prefer-non-repetition tie-break at equal
   depth). MOPNS has no depth notion — best-first descent has no concept
   of "most resistant line". The difference affects only the informational
   PV / refinement (`Search::pv_status`), never the proven outcome. No
   action.
2. **Half-proven draw facts (interval bounds).** MOPNS carries `pess/opti`
   intervals that narrow before a node is fully solved; our TT stores only
   a full solved `Outcome` or unsolved `(pn, dn)` bounds. A position class
   where this matters: a defender chain where "the attacker cannot win"
   (all children proven `≤ Draw`) completes long before "the attacker can
   hold the draw" (some child proven `≥ Draw`) — MOPNS would cache the
   `opti = Draw` half. Measured surface in *our* profile: thin. The
   repetition-cache spike (`dfpn/research_repetition_cache.md` §2) shows
   first proofs of repetition-draw chains sum to 1,597 child evals on the
   stress case — the two halves complete essentially simultaneously one
   eval apart from the path-repetition leaves; the cost is the re-descent
   churn around uncacheable chains, not half-proof asymmetry. A
   draws-only interval cache would additionally hit only *before* the
   plan9 full-Draw cache does (a half-proof is a sub-proof of the same
   `f(P, A)` value), and inherits the identical soundness question with no
   published answer (§2 above). No-go; see §5.
3. **Root-level outcome targeting.** MOPNS's attracting/distracting descent
   picks *which outcome to prove* from effort numbers; our solve picks
   outcome targets structurally (first decisive outcome, then shortest-PV
   refinement; `dfpn/mod.rs` iterative rounds). A depth-first MOPNS
   (§4.6 sketch: threshold on `G(·,o*)` + threshold on `S(·,o')`) would
   formalize our threshold arithmetic into a two-outcome-per-descent
   scheme — but that is a threshold-semantics change (see §5) and changes
   neither cacheability nor, on our measured profile, the line source.

## 4. Relation to the 2005 journal GHI algorithm (feeds `conversion` #5d)

- MOPNS **cites the journal version** (Information Sciences 175(4), 2005)
  as its GHI reference `[7]` — useful confirmation that the journal paper
  is the canonical algorithm reference for anything multi-outcome or
  depth-first built on df-pn.
- MOPNS **consumes GHI, does not extend it**: §4.6 lists the GHI solution
  as a directly applicable refinement for a depth-first MOPNS; §6 states
  adapting it "should be straightforward" and that cyclic-graph problems
  for MOPNS "are similar to that of PNS and DFPN". No MOPNS-specific GHI
  machinery exists. The later #5d mining needs nothing from this file
  except the pointer: whatever soundness argument exists for reusing
  repetition-dependent results lives in the journal GHI paper (twin
  entries + Kawano simulation, `dfpn/research_ghi.md` §3).

## 5. What this means for this solver

### 5.1 Initiative-constraint check

Structural constraint (`initiative.md`): change *what is cacheable* or
*where the line comes from* — or clear the plan10 hazard explicitly.

- **What is cacheable:** the only candidate mechanism extracted is the
  MOPNS-style interval (`pess/opti`) bound as a cacheable partial fact
  (§3 item 2). It changes what is cacheable in principle (half-proven
  draw facts instead of full draws) — but the measured surface on the
  stress profile is thin (half-proofs complete within ~1 child eval of
  full proofs; `research_repetition_cache.md` §2), it adds hits only
  earlier in the same `f(P, A)` computation, and it inherits the exact
  soundness question the plan9 cache answered with the exact-context
  identity argument — a half-fact has the same answer: reuse only under
  the identical `(P, A)`, whereupon the full proof is one eval away
  anyway.
- **Where the line comes from:** the attracting-outcome descent would
  change which outcome the search aims at per descent. On the stress case
  the root outcome is a Win and OR-side pn thresholds already drive the
  line; a df-MOPNS threshold pair (`G` at `o*`, `S` at `o'`) is a
  re-parameterization of the same threshold arithmetic.
- **Plan10 hazard:** a df-MOPNS *is* threshold arithmetic (`core.rs`
  threshold update, `selection.rs::select_from_children` folding). The
  plan10 failure mode (solved-mass folding destabilizing root `dn`
  schedules, `dfpn/report10.md` §"Why the go fails") applies unchanged —
  nothing in MOPNS decouples the benefit from the folding: its backup
  rules are the same sum/min per outcome, so a solved draw child folds
  into an unsolved parent's `Σ S(c,Draw)` exactly as it folds into our
  current `dn` today. The paper provides no mechanism-level reason the
  hazard would not apply; per the plan's non-goals, that withholds the
  go.

### 5.2 Verdict and summary row

**Expected outcome (i) of `plan3.md` obtained:** no missing machinery —
our draw propagation is the documented two-outcome slice of MOPNS at the
Draw threshold (§3), and our repetition handling lives in the space MOPNS
explicitly defers to GHI (§2, §4). `conversion` backlog #5b **closes as a
valuable negative result**; no new backlog item is opened.

| Mechanism | Lives in | Cacheable? | Line source? | Thresholds? | Plan10 hazard | Effort | Verdict |
|---|---|---|---|---|---|---|---|
| MOPNS: `2m` effort numbers `G/S` per outcome, per-outcome sum/min backup, attracting/distracting descent, `pess/opti` interval pruning (§4.1–4.5) | `selection.rs` (selection/backup), `core.rs` (thresholds), TT (interval outcome bounds) | Only candidate: cacheable `opti = Draw` half-facts — measured-thin surface (§3 item 2), same exact-context soundness shape as plan9 | Only via the descent policy's outcome targeting — no measured line-source change on the stress profile | Yes — df-MOPNS is a threshold re-parameterization | **Applies** — per-outcome sum/min backup folds solved children into unsolved parents exactly like today's `(pn, dn)`; paper gives no decoupling mechanism | M | **No-go** (thresholds + hazard); #5b closes: our propagation is the documented two-outcome slice |

### 5.3 What remains valuable from this mining

- The negative result itself: the solver's `Outcome`-based draw propagation
  is formally the MOPNS Draw-threshold slice, so there is no "missing
  three-outcome machinery" to import — the ad-hoc-looking code
  (`is_solved_by_children`'s all-solved/no-win/some-draw rule) is a
  published-correct special case.
- A published data point for the record: even in MOPNS's *friendly*
  regime (acyclic, no transpositions, PNS-favoring protocol), fewer node
  creations did **not** convert into wall-time wins at 3 outcomes
  (§1.6) — a caution for any future multi-outcome bookkeeping proposal.
- Two pointers for open backlogs: MOPNS cites the **journal** GHI paper as
  its cyclic-graph reference (strengthens `conversion` #5d / `dfpn` #4 as
  the mining target for any caching soundness argument), and its footnote
  3 endorses keeping the rule50 clock in the node identity rather than
  abstracting the rule away (our `src/zobrist.rs` design, dfpn non-goal to
  remove it).
- Bibliographic hygiene: the Kishimoto-IJCAI-11 attribution in
  `initiative.md`/`docs/bibliography.md` is corrected to Saffidine &
  Cazenave, ECAI 2012 (header note above).
