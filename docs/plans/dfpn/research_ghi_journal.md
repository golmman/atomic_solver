# Research: The journal GHI paper — Kishimoto & Müller, *Information Sciences* 175(4), 2005

Mining round for `conversion` backlog #5d (plan5), feeding `dfpn` backlog #4
(bounded cross-path verification). This extraction extends — and where marked,
**corrects** — `research_ghi.md` (the AAAI-04 mining). Terminology (base/twin,
simulation, first-player-loss, current-player-loss) is reused from
`research_ghi.md`, not redefined.

## 0. Version read

- **Primary**: A. Kishimoto, M. Müller, *A solution to the GHI problem for
  depth-first proof-number search*, Information Sciences 175(4), pp. 296–314
  (2005), DOI `10.1016/j.ins.2004.04.012` (copyright 2004, accepted
  26 April 2004). **Author copy PDF** (19 pages, publisher pagination), still
  served from the first author's co-author's Alberta page:
  `http://www.cs.ualberta.ca/~mmueller/ps/kishimoto-mueller-infsci-ghi.pdf`
  (located via OpenAlex location record `oai:CiteSeerX.psu:10.1.1.69.818`,
  which preserves the raw source URL). A copy is vendored in this directory as
  `ghi_journal.pdf` (same precedent as `ghi.pdf`). Paywalled at the publisher;
  OpenAlex/Semantic Scholar/Unpaywall all report `oa_status: closed`.
- **Cross-check**: the AAAI-04 version, `ghi.pdf` (in this directory), 6 pages,
  already mined in `research_ghi.md`.
- Text-extraction note: the journal PDF's symbol font subset carries a broken
  ToUnicode map (the ∞ glyph extracts as `1`, the minus as an unmapped `/C01`).
  The root-threshold sentences below were decoded from glyph outlines
  (`∞ − 1`, not `1`); §2.2 and §3.5 of the paper are affected.

What the journal version adds over the AAAI-04 one, precisely (this answers
plan5's "expected: complete proofs, the full simulation procedure,
df-pn-specific threshold details"):

1. The complete literature review (§2.2: Palay, Campbell, Breuker's BTA with
   three documented deficiencies, Nagai's approach with two documented
   drawbacks, Schijf's tree/DAG/DCG methods) — AAAI-04 has none of this.
2. The soundness argument as **two theorems with proofs** (§3.6, Theorems 3.1
   and 3.2) where AAAI-04 states an unproven Theorem 1 and defers the df-pn
   proof to this journal version (AAAI-04, "Correctness of Our Solution":
   "This theorem is proven for the case of df-pn in (Kishimoto & Müller
   2004)").
3. The df-pn pseudo-code (Fig. 2, adapted from Nagai) — AAAI-04 has none.
4. A controlled three-version ablation (DUP / +SIM / +NOCYCLE, Table 1) —
   AAAI-04 reports only IGNORE-GHI vs HANDLE-GHI aggregates for df-pn *and* αβ
   over Go *and* checkers.
5. Appendix A: why BTA must clear possible-draw marks every iteration.
6. Two efficiency sentences on simulation (legality checking instead of
   movegen; no child TT lookups) — §3.3 end.

**What it does not add: a step-by-step simulation procedure.** §3.3 is at the
same abstraction level as the AAAI-04 section; there is no pseudo-code for the
twin/simulation hooks anywhere in either version. The implementation-level
detail the plan hoped for (how a repetition terminal of a borrowed proof tree
is validated under a different path; whether the twin's ancestor set is
carried) is **not in the primary source**. The remaining published source for
that level of detail is Kishimoto's Ph.D. dissertation (Kishimoto 2005,
*Correct and Efficient Search Algorithms in the Presence of Repetitions*,
University of Alberta — openly available on the UAlberta repository;
`docs/bibliography.md` entry currently **Cited**). Not mined in this round;
flagged as the next source if `dfpn` #4 ever reopens.

## 1. The complete algorithm (journal §3, Fig. 2)

### 1.1 Entry structure and twin lifecycle

- A TT entry holds ordinary `pn`/`dn` bounds for *unproven* positions and is
  used as a normal transposition for them, from any path (§3.1 opening).
- When position A is proven via path p, the entry splits into a **base** entry
  and a **twin** entry; the proof is stored in the twin (keyed by p), and the
  base entry's `pn`/`dn` are **re-initialized to 1** (§3.1). A second proof via
  path q creates another twin. Disproofs are handled the same way (§3.1).
- **Twin-creation rule (NOCYCLE, §3.4)**: if a node is (dis)proven *without
  detecting a repetition*, the result is path-independent and is stored
  **directly in the base entry** — no twin. Twins exist only for
  repetition-involved (dis)proofs. The paper never defines "detecting a
  repetition" precisely; our `repetition_seen` propagation through the
  selected-child chain (`src/search/dfpn/selection.rs`, set in
  `evaluate_child` at `src/search/dfpn/children.rs` when the child's
  repetition key is on the path) is a stricter operationalization — it flags
  only repetitions that actually reach the selected child, but any
  repetition seen anywhere in the subtree would also be a sound
  (over-counting) implementation of the paper's condition.
- **Lookup order at node entry** (§3.4, end): "our scheme first looks at the
  path-independent base table entry for a position, and if the (dis)proof is
  found, that position is considered to be (dis)proven. Otherwise, the twin
  table entry is retrieved if available." For a reached node whose base is
  unsolved: a twin whose path equals the current path is reused directly
  (explicit in AAAI-04 §"Duplicating Transposition Table Entries", step 1 of
  `research_ghi.md` §3.1; the journal implies it via §3.1's "when reaching A
  via a path *other than* p, the proofs of the twin table entries are
  simulated"); otherwise each twin is simulated in turn ("they are tried one
  after another", §3.3 — rare, "since most of the proof trees have the same
  shape"); if at least one verifies, its result is used and a new twin for the
  current path is created; if none verifies, "the proof and disproof numbers
  from the unproven base table entry are used in the search" (§3.1).
- **Return site**: every reuse happens at the node, when the search *reaches*
  it (§3.1), i.e. before expansion — never as a child-bound injection from a
  probe at the parent. See §5 below for why this matters.

### 1.2 Path encoding (§3.2)

64-bit Zobrist-style signature of the root-to-node move sequence:
`code(p) = R[m1][1] ⊕ R[m2][2] ⊕ … ⊕ R[mk][k]`, with `R` a table of
`MaxMove × MaxDepth` random 64-bit integers. The depth index makes the code
order-sensitive. 19×19 Go used `MaxMove = 362, MaxDepth = 50` (~140 KB);
games with many moves split a move into 2–3 partial moves. Both published
versions are identical here. The code is a hash — collisions are not
discussed (a probabilistic soundness assumption; 64-bit birthday risk is
negligible per search but *unbounded* over a table's lifetime — same status as
position-key collisions, which our TT already accepts:
`src/search/tt/table.rs`).

### 1.3 Simulation (§3.3) — and what it does *not* specify

Simulation (Kawano 1996) verifies a twin's proof under a new path by
re-walking the stored proof tree: at each interior OR node it borrows the
winning move recorded in the proof tree (retrieved from the TT), at each AND
node it must expand **all** children; terminal nodes must still be proven.
Dual simulation verifies disproofs. Efficiency properties stated (§3.3): no
movegen at OR nodes (legality checks only); no TT lookups for children (a
df-pn node looks up all children every visit — a sentence the AAAI version
lacks); the borrowed tree is typically much smaller than a fresh search tree.

**The ancestor-context question (plan5's core question): the paper does not
carry or reconstruct the twin's original ancestor set.** The path code encodes
the twin's *own* path p — it is the twin's key, not an ancestor-set payload
handed to simulation. Nothing in §3.1–3.3 says how simulation judges a
repetition terminal of the borrowed tree whose repeating position was on p's
path but is not on the current path q. The only mechanisms the paper offers
are: (a) simulation re-walks under the *current* search context (so a
repetition terminal valid under p that is unavailable under q simply fails to
be a proven terminal — the simulation fails and the twin is rejected to base
bounds, §3.1), and (b) the theorems of §3.6, which *assume* the verification
succeeds only when the (dis)proof is valid for the new path (see §2 below).
This confirms — from the primary source — the gap
`research_ghi.md` §9 documents for our implementation: **the gap is in the
paper's own specification, not only in our port of it.** In particular, the
paper's failure handling is exactly the one our plan5/plan7 implementation
had (fall back to base bounds, no bounded re-search); `research_ghi.md` §9's
"Option A" (bounded fresh `dfpn` under the current path, accept on agreement)
is an extension *beyond* the paper, motivated by the plan7 false win, not
something the journal version prescribes.

### 1.4 Df-pn-specific threshold details (§3.5)

- Root thresholds: "In the pseudo-code of the df-pn algorithm (see Fig. 2
  again), the thresholds of proof and disproof numbers are set to 1 at the
  root. This causes the GHI problem, since df-pn saves the thresholds in the
  transposition table before expanding a node." Following Nagai's approach,
  "we initialize both thresholds at the root to **∞ − 1**". A returned
  `(pn, dn) = (0, ∞)` or `(∞, 0)` is a correct (dis)proof; anything else is
  `unknown`.
- **Correction to `research_ghi.md` §4.2**: that file states "In the original
  df-pn, the thresholds at the root are initialized with one large value. In
  the paper's modified version, the root thresholds are initialized to
  (1, 1)". Both published versions say the opposite: the modified scheme
  initializes the root thresholds to the *large* value ∞ − 1 (journal §3.5,
  decoded from glyph outlines; AAAI-04 "Algorithm-Specific Implementation
  Details": "we initialize the thresholds of proof and disproof numbers at
  the root to ∞ − 1, not ∞ as in the original df-pn algorithm"). The two
  versions genuinely disagree only about the *original* df-pn's root
  initialization (journal §2.2.4: "not 1 as in the original df-pn"; AAAI-04:
  "not ∞ as in the original") — the journal is the later, fuller statement;
  Nagai's thesis itself was not consulted for this round. The `research_ghi.md`
  §4.2 rationale (df-pn stores thresholds in the TT before expansion) is
  correct; its direction is inverted. Note this mechanism does not exist in
  our solver: our `dfpn` frames store `pn`/`dn` at **frame exit** only
  (`src/search/dfpn/core.rs`, the single `self.tt.store` call), never
  thresholds before expansion, so the paper's root-GHI hazard cannot arise
  here.
- The (1, 1) base re-initialization on twin store (§3.1) has, beyond
  `research_ghi.md` §4.1's "stale bounds" rationale, a second effect visible
  only in df-pn's architecture: the parent computes thresholds from
  children's TT entries, so a twin's solved `(0, ∞)` would otherwise leak
  into the parent's threshold arithmetic from the child's entry; the
  re-initialized `(1, 1)` base keeps the path-dependent solved fact invisible
  at the parent until the search actually descends into the node, where the
  twin is consulted (node entry) and possibly simulated. The paper does not
  spell this out — and it leaves the converse gap: after a *verified* reuse
  the proof is saved in the (new) twin while the base stays `(1, 1)`, so how
  the proven result propagates to the parent is unspecified. Our
  architecture's natural completion — a verified frame-entry reuse stores
  back into the TT as a normal solved entry — is exactly what plan11 arm A
  measured (see §5).

## 2. The soundness theorem (journal §3.6)

Statement, with the paper's own assumption quoted: "Assume that all proven
and disproven nodes are stored in the transposition table. Although our
proposed solution might compute incorrect proof and disproof numbers for
unproven nodes, the following theorems guarantee correctness of the
solutions."

- **Theorem 3.1** — "Our solution does not suffer from the draw-first case."
  Proof: if proving Y via path (1) involved repetitions related to ancestor X,
  then X and Y are stored "via path (1)" (twins); reaching Y via path (2)
  forces a search below Y and X (the twins are not blindly trusted).
- **Theorem 3.2** — "Our solution does not suffer from the draw-last case."
  Proof by cases: (1) X via (2) proven *without* repetitions → its base-stored
  proof is path-independent and safely reused everywhere; (2a) X via (2)
  proven *with* repetitions but Y via (2) without → X's twin is not retrieved
  under (1), and Y's repetition-free proof is reusable; (2b) both proven with
  repetitions → neither twin is retrieved under (1); X and Y are explored,
  "guaranteeing a correct result".

What the theorems guarantee: every **returned** (dis)proof is a genuine
(dis)proof — no draw-first/draw-last GHI corruption of final answers. What
they do *not* guarantee: completeness, bound quality for unsolved nodes
(explicitly may be wrong), or that every valid reuse is found.

Preconditions, and their status in our solver:

1. **"All proven and disproven nodes are stored in the transposition table"**
   (§3.6 assumption; keeping solved entries is also the experimental setup,
   §4.1, and garbage collection is explicitly left open, §5). Our TT has
   generational replacement (`src/search/tt/table.rs`: `new_generation`,
   stale-slot preference) — the precondition does **not** hold. It is
   immaterial for our current design because we store no path-dependent
   results at all (point 3), but any future twin mechanism on our TT would
   need a solved-entry retention policy — a real cost the paper's 200 MB
   "keep everything" setup avoided.
2. **Verification soundness is assumed, not proven.** Theorem 3.1's proof
   step "a search is performed below Y and X in our algorithm" treats
   simulation as a correct verification oracle; Theorem 3.2 case 2b asserts
   exploration "guarantee[es] a correct result". The paper proves that the
   *twin store/lookup discipline* routes around draw-first/draw-last; the
   semantic load-bearing step — simulation succeeds only if the borrowed
   (dis)proof is valid under the new path — is a hypothesis. For
   first-player-loss tsume-shogi this hypothesis is plausible (see §3); for
   our repetition-as-draw semantics it is *our* obligation (this is exactly
   where the plan7 false win entered, and where the plan1 monotonicity
   lemma / a bounded fresh re-proof would have to carry the argument).
3. **Path-code collision-freedom** (§3.2, 64-bit hashes): undiscussed;
   same probabilistic status as our position keys.
4. **Root thresholds ∞ − 1** (§3.5): avoids threshold-storage GHI at the
   root; not applicable to our frame-exit-store architecture (§1.4).

Which preconditions we already meet structurally: our TT discipline is the
paper's *base-entry-only* world taken to its safe extreme — every stored
`Outcome` is path-independent (`src/search/tt/entry.rs` has no path field;
`TtEntry` = the paper's base entry), repetition-dependent draws are stored
unsolved as `(1, 1)` (the first-player-loss shortcut,
`src/search/dfpn/core.rs::dfpn`, `suppress_draw` branch) and cached only
per-search under an exact `(position, ancestor-set)` key
(`src/search/dfpn/repetition_cache.rs`). With no twins in the table, the
theorems' obligations are vacuously discharged — our current soundness does
not depend on them. They become load-bearing only when cross-path reuse of
solved results is added (`dfpn` #4).

## 3. The first-player-loss specialization → repetition-as-draw (Goal 3)

Where the paper's argument uses "repetition is decisive":

- **First-player-loss** (§1.2): a repetition is a loss for the first player
  (the attacker). "The GHI problem only affects disproofs": a proof (mate)
  can never contain a repetition, so proofs are path-independent; disproofs
  may hinge on a repetition and are the path-dependent objects. Tsume-shogi
  programs exploit this by "not caching disproofs caused by repetitions" —
  the exact shape of our first-player-loss shortcut (with draw-as-disproof
  reduction).
- **Current-player-loss / SSK** (§1.2, §2.2.3, §4.3): a repetition is a loss
  for the player who repeats; *both* proofs and disproofs can be
  path-dependent (the paper's Fig. 6 example: the same Go position is a
  White win via one move sequence and a Black win via another). This is why
  the twin+simulation machinery exists at all — the first-player-loss
  shortcut alone does not suffice.
- **BTA's model** (§2.2.3): "BTA is described for a 3-valued evaluation model
  with values win, loss, and draw. If a draw is considered a disproof as in
  their experiments, this model is the same as the first-player-loss scenario
  in our framework."

Mapping onto our semantics (two-fold repetition as **draw**, neither variant
of the paper):

1. **Decisive facts are path-independent by rule, not by search discipline.**
   In both paper variants a repetition edge evaluates *decisively* (loss for
   someone), which is what creates path-dependent Win/Loss facts needing
   twins. In our solver a repetition edge evaluates `Draw`
   (`evaluate_child`, `repetition_seen`; `path_contains` frames return
   `Draw`), so no decisive proof can route through one — a winning child must
   be a Win; a loss requires *all* children to be wins
   (`research_repetition_cache.md` §4 point 1). Our situation is strictly
   stronger than the paper's first-player-loss scenario: there, disproofs
   are still path-dependent and need twins; here, *no decisive fact is ever
   path-dependent*, so the twin mechanism for decisive facts is never needed
   and Theorems 3.1/3.2 hold vacuously.
2. **The paper's proof structure extends — trivially — for decisive facts;
   it does not provide machinery for our actual path-dependent object.** The
   path-dependent results in our solver are **Draw proofs** (chains above a
   repetition edge). The paper's framework has no analog: under
   first-player-loss a draw is folded into "disproof" (the BTA reduction our
   semantics refuses — draws must stay draws), and under SSK repetition is
   illegal/loss, never draw. So the journal machinery maps cleanly onto the
   *decisive-fact* reuse question (`dfpn` #4) and not at all onto
   *draw-proof* reuse (plan9's cache surface). The `dfpn` #4 backlog
   assumption "the paper's proof structure extends to repetition-as-draw" is
   **checked and holds only in the vacuous direction**; any cross-path reuse
   of draw proofs must be argued from our own semantics — the `f(P, A)`
   identity (`research_repetition_cache.md` §3) for exact contexts, or the
   plan1 monotonicity lemma for widened contexts — not from the paper.
3. **The verification obligation transfers to us.** Because the theorems
   assume a sound verification oracle (§2 point 2), and our repetition edges
   are non-decisive, a Kawano-style simulation borrowed from the paper would
   re-walk a decisive proof whose terminals are all repetition-free — for
   Win/Loss facts the simulation is verifying something our rule already
   guarantees (see §5). For draw facts the paper offers nothing.

## 4. Reuse-site economics and the plan10 hazard (Goal 4)

Journal lookup/return order vs our sites:

- **Journal**: base-entry check → twin (exact path) → simulation per twin →
  adopt or fall back to base bounds — all at node entry, before expansion
  (§3.1, §3.4). A reused result enters the parent's arithmetic only through
  the node's own proven return (the same channel as a freshly proven child).
- **Ours**: the TT solved-result check at `dfpn` frame entry
  (`src/search/dfpn/core.rs::dfpn`, after the `path_contains` local-repetition
  check, gated by the one-ply `best_move_repeats_path` guard) = the journal
  base-entry/twin lookup site. `evaluate_child`
  (`src/search/dfpn/children.rs`) is the *child* probe site our solver adds
  (the parent's only channel to children — the recursive `dfpn` return value
  is discarded; plan11's structural fact), where a resolved entry becomes a
  solved `ChildInfo` folded into `select_from_children`
  (`src/search/dfpn/selection.rs`).

**Plan10-hazard verdict (mechanism-level)**: the journal reuse path cannot
reproduce plan10's failure mode (`dfpn/report10.md`: m22_white 3.1 s →
120 s timeout, root `dn` 9.3k → 503k), for three structural reasons:

1. **Return site.** Reuse is node-entry-only; there is no mechanism returning
   a solved result as a child bound from a probe at the parent. Our plan10
   site-2 (`evaluate_child` adoption) injected solved `(0, INF)`/`(INF, 0)`
   into an unsolved parent's bound computation without ever entering the
   child — the journal scheme has no such site. (Caveat: in Nagai's df-pn the
   parent reads children's entries for thresholds, which is why the (1, 1)
   base re-initialization matters — see §1.4; with it, a twin's solved
   numbers are invisible to the parent's arithmetic until node entry.)
2. **Volume is bounded by verification work.** Each cross-path reuse pays a
   simulation (a search). Plan10's adoptions were O(1) index probes at
   ~1.1–2M per run — 50–100× the exact-key rate — flooding threshold
   arithmetic with free solved mass. The journal mechanism makes reuse
   expensive by construction; NOCYCLE keeps twins rare (Table 1: 356
   simulation calls over 134 problems, vs 239,820 without NOCYCLE).
3. **No stale solved bounds.** Twin stores re-initialize the base to (1, 1)
   (§3.1); plan10's index left the base TT untouched.

But the same three properties are why the mechanism is **economically empty
on our stress class** — the plan10 win *was* the cheap mass folding
(`report10.md`: "the benefit and the pathology are one mechanism"; flooring
the folding rescued m22 and killed the stress win):

- **Win/Loss side**: the journal mechanism's hit surface is a subset of
  plan11 arm A (frame-entry adoption with TT store-back, unverified — arm A
  needs no simulation because decisive facts are path-independent by rule,
  §3 point 1) at strictly higher per-hit cost. Arm A measured **−8.8%**
  stress first-outcome / **−2.6%** default (`dfpn/report11.md`) — below the
  10% gate, useless in default mode. Verification can only shrink the
  surface: it does not change the economics (plan5's question answered:
  no).
- **Draw side**: the re-proofs a cross-path draw cache would save cost
  ≈ **1.1 child evals each** (`research_repetition_cache.md` §2:
  `repeat_proof_work = 34,118` over `31,620` re-proofs; the churn around
  them is the real cost, and that is exactly what plan9's zero-verification
  exact-context cache intercepts). Any verification-based reuse (simulation
  or bounded re-proof) costs at least what it saves; and the residual
  cross-context surface is thin — plan1 measured 267 v2-only hits per
  first-outcome run after plan9's cache already intercepts ~97% of the
  monotone-widening surface (`conversion/report1.md`).

Empirical anchor (journal Table 1, Go one-eye, SSK, 134-problem subset, 200 MB
TT): IGNORE-GHI 134+2 solved with **2 incorrect proof trees**, 3,614,539
nodes, 81 s; DUP (path-split entries, correctness guaranteed) 9,411,063
nodes, 256 s; +SIM 7,015,856 nodes (2,699,556 by simulation), 239,820 SIM
calls (2,956 failed), 175 s; +NOCYCLE 3,689,363 nodes (2,813 by simulation),
**356** SIM calls (156 failed), 82 s. Reading: NOCYCLE cuts simulation calls
by ~674× and makes the correct solver as cheap as the incorrect one (82 s vs
81 s) — the entire overhead question is decided by *how few twins/simulations
you need*, which is a function of how few path-dependent results exist. Our
solver has already driven that count to zero for decisive facts (rule-level)
and to the plan9-cached set for draws.

Also relevant from §2.2.5: Schijf et al.'s DCG method "sometimes results in
wrong disproofs" — a caution that mapping *cyclic* transpositions without
history checks is unsound; our TT maps only acyclic (DAG) transpositions
freely (repetition-key membership is checked against the live path), which is
the DAG method's discipline.

## 5. Soundness contract sketch for `dfpn` #4 (Goal 5)

Shape per `conversion` backlog #1's lemma template (working agreement #5).

- **Value claim (adopted from plan1, retained there as the candidate
  argument)**: under two-fold-repetition-as-draw, decisive outcomes are
  downward-closed in the ancestor repetition-key set and in the rule50 clock
  (a Win/Loss proof is structurally repetition-free — §3 point 1 — and
  clock-independent except that smaller clocks only add defender budget).
  Formally: Win/Loss proven at (P, A, c) holds at any (P, A′, c′) with
  A′ ⊇ A, c′ ≤ c. Draw is *not* monotone in this direction; the only sound
  draw reuse is the exact-context identity f(P, A) (plan9) or the superset-
  plus-clock-monotone widening plan1 measured as thin
  (`conversion/report1.md`).
- **Reuse direction**: decisive facts, adopted at **frame entry** (the
  journal node-entry site, our `dfpn` TT solved-result check), with TT
  store-back so the parent's threshold arithmetic sees exactly what it sees
  for an exact-key solved entry (completing the propagation channel the
  paper leaves unspecified, §1.4). Never as child bounds at
  `evaluate_child` — that is plan10's measured hazard, and the journal
  mechanism does not use it either (§4).
- **Verification option**: not needed for soundness (the value claim is a
  rule-level theorem plus plan1's lemma, corroborated empirically by
  report10's 444/444 adopted-Win re-verifications and plan11's clean
  controls). The paper's simulation is *unavailable* as machinery anyway:
  our TT stores `best_move` chains, not proof trees
  (`src/search/tt/entry.rs`), so Kawano simulation would need a new
  per-node proof-tree store (memory + `src/search/tt/` surface) **and** its
  soundness is assumed in the paper, not proven (§2 point 2). Option A
  (bounded fresh `dfpn` at `max_depth = entry.depth`, accept on agreement —
  `research_ghi.md` §9) is self-sound but costs ≥ the re-proof it would
  replace — profitable only where re-proofs are expensive, which the
  measurements say they are not (≈1.1 evals for draws, §4).
- **Expected hit-surface**: bounded above by plan11 arm A (−8.8% stress
  first-outcome / −2.6% default); the draw-side residual beyond plan9 is
  measured thin (267 v2-only hits/run, `conversion/report1.md`).
- **Mandatory gates (if ever revived)**: repetition soundness gate
  (`cargo test --release --test test_repetition -- --include-ignored`);
  `m22_white` control (the plan10/11 canary — 3.1 s baseline); quick-suite
  drift protocol (`benchmark --suite quick --json --first-outcome`);
  deterministic budget contract (`child_eval_budget` /
  `ExitReason::BudgetExhausted` untouched); the two-solves-in-one-process
  property test (a reuse must never flip a decisive outcome).

**Verdict: no-go** for `dfpn` #4 as a sequential-search performance lever —
not because the mechanism is unsound (it is the soundest shape measured) but
because its safety mechanisms (verification cost, node-entry return, (1, 1)
re-init) are exactly what forfeits the plan10 win, and the measured ceilings
(arm A −8.8%/−2.6%; draw-side ≈1.1-eval re-proofs) are far below any go bar.
The contract is retained for two consumers: (a) as the soundness-argument
template for any future reuse lever; (b) as a **design constraint for the
parallel-search spike** (`conversion` backlog #4 / `lean` #2): with multiple
workers sharing one TT, cross-worker path contexts differ *by construction*
— the plan9 exact-context cache degenerates, the first-player-loss shortcut
silently serializes the repetition-heavy regions, and the journal mechanism
becomes the published reference for keeping shared-TT reuse sound. There,
unlike here, cross-path reuse is not an optimization over an already-cheap
re-proof; it is the difference between parallel and serialized search.

## 6. Mapping section

Summary table row (`conversion` #5d → `dfpn` #4):

| Mechanism | Where it lives in our code | What is cacheable / line source / thresholds | Plan10-hazard verdict | Effort | Go/no-go |
|---|---|---|---|---|---|
| Journal GHI: base/twin entries + path codes + Kawano simulation, node-entry reuse, (1,1) base re-init, NOCYCLE | Nothing remains (twins removed plan7); nearest analogs: TT solved-result check + one-ply guard at `dfpn` frame entry (`core.rs`), `evaluate_child` resolved check (`children.rs`), plan9 cache (`repetition_cache.rs`) | Cross-path reuse of *decisive* facts only (ours are path-independent by rule — the paper's twins are unnecessary here); line source would be a stored proof tree (we store none) or a bounded re-proof (Option A); thresholds untouched (frame-entry return + store-back) | **Avoided structurally** (node-entry return, verification-bounded volume, no stale solved bounds — §4) — but the same properties cap the win at plan11 arm A (−8.8% FO / −2.6% default) and make draw-side reuse cost ≥ the ~1.1-eval re-proofs it saves | M | **No-go** for the sequential lever; close `dfpn` #4 as evidence-based no-go, retain the contract as soundness template + parallel-search design constraint (`conversion` #4 / `lean` #2) |

Corrections to `research_ghi.md` recorded here: §4.2's root-threshold
initialization is inverted (both papers: modified scheme initializes root
thresholds to ∞ − 1; `research_ghi.md` says (1, 1)) — §1.4 above. §9's two
implementation gaps are confirmed as gaps in the paper's own specification,
not defects of our port — §1.3 above. `research_ghi.md` §6's "The paper
proves (Theorem 1)" is precise only for the journal version (AAAI-04 states
Theorem 1 unproven and defers to the journal).

## References

- A. Kishimoto, M. Müller, "A solution to the GHI problem for depth-first
  proof-number search," *Information Sciences* 175(4), pp. 296–314, 2005.
  Author copy vendored: `ghi_journal.pdf` (this directory).
- A. Kishimoto, M. Müller, "A general solution to the graph history
  interaction problem," AAAI-04. Vendored: `ghi.pdf`. Mined:
  `research_ghi.md`.
- A. Kishimoto, *Correct and Efficient Search Algorithms in the Presence of
  Repetitions*, Ph.D. dissertation, University of Alberta, 2005 — the
  remaining implementation-level source (simulation details); open access on
  the UAlberta repository. Not mined this round; next source if `dfpn` #4
  reopens or the parallel spike needs the full procedure.
- A. Nagai, *Df-pn Algorithm for Searching AND/OR Trees and Its
  Applications*, Ph.D. dissertation, University of Tokyo, 2002.
- Y. Kawano, "Using Similar Positions to Search Game Trees," *Games of No
  Chance*, MSRI Publications, 1996.
- D. M. Breuker, H. J. van den Herik, J. W. H. M. Uiterwijk, L. V. Allis,
  "A solution to the GHI problem for best-first search," *Theoretical
  Computer Science* 252(1–2), 2001 (BTA; the journal §2.2.3 critique and
  Appendix A).
- M. Schijf, L. V. Allis, J. W. H. M. Uiterwijk, "Proof-number search and
  transpositions," *ICCA Journal* 17(2), 1994 (tree/DAG/DCG methods).
