# Plan 9: Literature mine — Deep df-pn (Zhang et al. 2017): seesaw reduction
# via depth-dependent unsolved-leaf pn/dn

Initiative: `research` literature target **#15** (opened by the 2026-09-21
re-scope; the documented successor of plan8, `report8.md`). After plan8 the
initiative's node-count program was closed and the goal pivoted to
*characterizing the structural floor*. As a final cheap lever before the
consolidation, this plan mines the one remaining unexamined mechanism class:
**the seesaw-effect reducers** (DeepPN 2015, Deep df-pn 2017), whose full
texts are already in-repo (`docs/theory/deep-pns-2015/`,
`docs/theory/deep-dfpn-2017/`) — no acquisition survey is needed, which is
what makes this a half-session desk exercise.

This is a mining plan per the working agreement: one discovery, no `src/`
changes, no POC. The deliverable is an extraction plus an applicability
verdict; only if a sound, contract-compatible mapping survives does a sized
POC proposal follow as a separate plan.

## Scope decision (recorded up front)

**DeepPN 2015 is NOT mined separately.** It is best-first PN-search with an
open frontier — the family plan6 closed (RAM = TT only fatal, same
classification as PN²) — and the Deep df-pn paper's own §1 already records
DeepPN's two drawbacks (storage cost, update cost). It gets a bibliography
row (`Cited`, background to the 2017 paper) and nothing else.

**Deep df-pn 2017 IS mined**, because it passes the "no re-running closed
levers" filter narrowly: it changes the *unsolved-leaf pn/dn values*
(`D_dfpn(depth) = E^(D−depth)`) rather than the thresholds — a different
site than the 1+ε trick implemented here. Narrow, because the prior
economics are negative (see Context 4).

## Goal

Answer backlog #15: **can Deep df-pn's depth-dependent leaf valuation be
mapped onto this solver's DF-PN+ at all — i.e., is there a path-independent,
GHI-safe, RAM-bounded definition of the mechanism at the actual leaf- sites
— and does any of its published evidence transfer to the stress class?**
Expected outcome (and the reason this is worth only half a session): the
classification lands in (c) contract-breaking or (d) evidence-against, and
#15 closes with the reasoning as the final no-go record feeding the
structural-floor document (#16).

## Context

Six prior results shape this plan:

1. **The pivot.** plan8 closed the node-count program (four ε closure legs;
   all backlog rows answered/closed/pre-weakened). This plan runs only
   because the source is in-repo and the desk cost is bounded; it must not
   reopen a POC program unless a genuinely new mechanism survives every
   contract.
2. **The TT path-independence contract** (`src/search/tt/`): base entries
   are path-independent; repetition-dependent results are never cached
   (first-player-loss GHI shortcut). `n.depth` — the quantity `D_dfpn`
   consumes — is a *path-dependent* quantity in df-pn: the same position
   reached by different lines sits at different depths, so the leaf value
   the paper assigns is not a function of the position. Either the TT
   contract breaks (entries become path-relative — GHI-adjacent hazard) or
   depth must be replaced by a path-independent proxy (halfmove clock?
   frame depth from the current refinement root? remaining DTM — unknown
   a priori), which mutates the mechanism. Resolving which is the crux of
   the classification.
3. **The mechanism's own evidence is thin and out-of-regime.** Eight
   Connect6 positions, a relevance-zone/VCDT sub-solver contributing most
   of the selectivity, best-case E/D tuned per position (hill-climbed;
   Table 3), and Theorem 1 is a heuristic argument that ignores
   transpositions and repetitions entirely (the expansion-inequality
   `Σ_children D_dfpn(d+1) < D_dfpn(d)` requires E′ < E, i.e. below-average
   branching — and says nothing about re-visited nodes).
4. **Adjacent closed surface.** The mechanism is a cousin of threshold
   padding ("stay deeper in one subtree"), whose economics this solver has
   closed four ways: no constant ε (plan4 Phase 0), no path schedule
   (plan4 Phase 1), no regional structure (plan4), no node-local
   confidence signal (plan8) — all trajectory chaos. Deep df-pn is not
   formally a re-run (different site), but the prior is negative.
5. **plan6's algorithm-swap closure.** The full-algorithm replacement
   surface (PDS/PN²/hybrids) is closed with evidence favoring df-pn; Deep
   df-pn is a *modification* of df-pn, not a swap, so it was out of plan6's
   scope — this plan is its proper home.
6. **Structural facts to verify in code, not assume.** The plan must read
   `src/search/dfpn/children.rs` (where unsolved children receive
   pn/dn = 1), `src/search/dfpn/selection.rs` and the core threshold
   recursion to establish: (i) whether any depth-like quantity is even
   available at the leaf-assignment sites (the solver's frames may not
   carry an absolute search depth); (ii) how the iterative bounded
   refinement rounds interact (a depth-dependent leaf value would shift
   between rounds, changing which children the threshold math selects —
   is that consistent with round monotonicity and the `--refine-cap` /
   child-eval-budget contracts?); (iii) the interaction with the 1+ε
   second-child threshold, whose δ sums would now contain `E^(D−depth)`
   terms (the paper's own comparison in §6.3 tuned both independently).

## Hypotheses

- **H1 (mechanism survives)**: a path-independent, GHI-safe definition of
  the deep-leaf value exists that preserves the published mechanism's
  direction (stay-deeper bias), survives the contract check at named call
  sites, and the published record shows a win in a regime comparable to the
  stress class. → sized POC proposal as a plan10 candidate.
- **H0 (classification closes #15)**: the mechanism is path-dependent at
  its core with no faithful proxy, or contract-breaking for another
  reason, or the evidence regime is too far from ours to justify a POC.
  → #15 closes; the reasoning feeds structural-floor item #16.

## Method

### Phase 1 — extraction (`research_deep_dfpn.md`)

Follow the house template (`research_alternative_algorithms.md`,
`research_child_termination.md`):

- **Summary** — Deep df-pn in solver terms: replace the unsolved-leaf
  pn/dn = 1 with `D_dfpn(depth) = E^(D−depth)` (Definition 1); Table 1's
  behavior map (E = 0 depth-first, E = 1 or D ≤ 1 plain df-pn, otherwise
  intermediate); selection changes implicitly through the threshold
  recursion; Theorem 1's argument and its assumptions (E′ < E, no TT, no
  repetitions, expansion only).
- **Evidence table** — the paper's own numbers, normalized: 8 Connect6
  positions, node/seesaw counts vs df-pn and vs 1+ε (Table 2), the
  hill-climbing overhead (Table 3), and what the comparisons do *not*
  cover (no TT, no repetition handling, per-position tuning, relevance
  zones doing unmeasured work). This table survives regardless of verdict.
- **Mapping to this solver** — name the exact sites: where unsolved
  children get their initial pn/dn (`children.rs`), where child δ feeds
  the second-child threshold (`core.rs` selection/threshold recursion),
  `selection.rs` best/second search, `tt/` entry contents. Verify in code
  whether a usable depth-like quantity exists at those sites and what it
  means across refinement rounds and re-entries. A mapping that cannot
  name a real depth source is (c) by construction.
- **Contract check** — TT path-independence (the crux, Context 2); GHI
  first-player-loss shortcut (would depth-biased selection change which
  repetition classes are entered?); RAM = TT only (Deep df-pn is
  depth-first — expected compatible, unlike DeepPN); `ProofEvent` emission
  neutrality; refinement-round / `--refine-cap` / child-eval-budget
  consistency; 1+ε interaction (the two mechanisms overlap in the δ-sum
  threshold arithmetic — state whether they compose, conflict, or subsume).
- **Sizing sketch** — only if H1: env-gated leaf-value arm, stress gate
  object with m22/dec13/dec10 controls, honest session estimate.

### Phase 2 — applicability verdict

Classify the mechanism (and any variant found in passing):

- **(a) structurally covered** — e.g. if the refinement rounds' bounded
  thresholds already produce the stay-deeper effect the parameter tuning
  buys.
- **(b) sound here, evidence of a win, unmapped** — POC proposal
  candidate.
- **(c) contract-breaking** — expected: path-dependence vs the TT
  contract; state precisely which contract and which line of reasoning.
- **(d) evidence-absent or evidence-against** — Connect6 relevance-zone
  regime, best-case per-position tuning, and the four ε closure legs.

Verdict H1 iff a variant lands in (b).

## Decision gates

| Gate | Criterion | Consequence |
|---|---|---|
| **OPEN** | ≥1 mechanism in class (b) with a named depth proxy, a contract-check pass, and comparable-regime evidence | #15 stays open as "mined — POC pending"; sized proposal becomes the plan10 candidate, competing with consolidation item #16 for the next slot (decided in the report). |
| **CLOSED** | All mechanisms land in (a), (c), (d) | Close #15 with the classification as the seesaw-thread no-go record; the next (and likely last) plan is #16, the structural-floor consolidation. |
| **DEFER** | A decisive question (e.g. the depth-proxy question) cannot be settled from the in-repo source plus code reading | Record the blocker under #15; proceed to #16; #15 stays open at reduced priority. |

## Deliverables

- `docs/plans/research/research_deep_dfpn.md` — the extraction (Phase 1).
- `docs/plans/research/measurements/plan9/` — notes, the (a)–(d)
  classification, code-site verification notes.
- `docs/plans/research/report9.md` — verdict, gate decision,
  hand-off/closure record.
- `docs/bibliography.md` — DeepPN 2015 row (`Cited`), Deep df-pn 2017 row
  flipped to `Mined` with a pointer to the extraction (or `Cited` if
  DEFER-without-extraction).
- `docs/plans/research/initiative.md` — backlog #15 status, History entry.

## Verification

- No `src/` or `examples/` changes (mining plan): finish with
  `git diff --exit-code` clean; no benchmark runs; no claim may rest on a
  new measurement.
- Every claimed code site verified by reading the file; the
  path-dependence argument must quote the actual TT entry contents
  (`src/search/tt/`) rather than paraphrase the contract.
- Bibliography and backlog updates cross-checked against the report.
- Housekeeping: `cargo fmt --check`, `cargo clippy --release
  --all-targets`, `make test` green (hygiene check per the Boy Scout
  principle).

## Non-goals

- No POC implementation — an (b)-class verdict produces only a sized
  proposal (plan10 candidate), per the one-discovery rule.
- No separate DeepPN 2015 mining (scope decision above).
- No re-litigation of the ε closures (plan4/plan8) or plan6's
  algorithm-swap record; this plan cites them as priors, not targets.
- No parallel-PNS, NN-prior, or recognizer work (#9/#6/#7 remain closed or
  pre-weakened).
- No changes to the proof-tree layer, `ProofEvent` protocol, or the
  optimizer interface (`docs/spec/optimizer_interface.md`).
- No wall-time work; the metric of record remains first-outcome
  `child_evals`.

## Final task

Write `docs/plans/research/report9.md` (verdict, gate decision,
classification summary, closure/hand-off record), update backlog #15 and
the History in `initiative.md`, and update `docs/bibliography.md` for both
papers.
