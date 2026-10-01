# The PNS selection mechanism: provenance, theory linkage, and the considered alternatives

Plan6 analytical note (D3). Written after the A/B; the normative mechanism
spec is `plan6.md` §2, the implementation is
`examples/proofdb/pns/{config,pacing}.rs` + `pns/selector/{decision,rung}.rs`,
and the measured verdict is `measurements/plan6/verdicts.json` +
`measurements/plan6/README.md`. This file mines the vendored theory under
`docs/theory/` (nothing vendored inside this directory, per the theory
library conventions) and records the provenance chain.

## 1. Provenance chain (how the mechanism came to be)

1. **report3 → the breadth-pns pivot.** plan3's gradient A/B measured the
   per-visit fixed-budget screen pass over ranked frontier classes; the
   pivot to a live PNS priority queue (plan4) came from the observation
   that fixed-class ordering cannot adapt to censor outcomes.
2. **report4 finding 3 — the layer-cost pathology.** Batch 1 (the plan4
   batch) measured the emergent dynamic: every censored visit exposes a
   fresh number-1 sublayer (18.2 children/visit), so the queue's number-1
   pool never drains within a session and the ladder (the geometric
   revisit budget) never fires. Layer cost grows ~27× per ply against a
   fixed session cap — an unrationed session dies inside one ply layer.
3. **plan5 — the two lemmas + the batch-2 control.** L1: no selection key
   built from (structural numbers, ledger state) can order a fresh pool —
   all its ledger state is zero — so *work-aware* ordering is impossible
   for exactly the nodes an expansion policy wants to reach. L2: a
   factless policy's number-1 pool grows monotonically (≈ +21.2 exposed
   per censor vs ≤ 1 removed), so **emergent escalation is unreachable
   in-session** — measured as E3 in batch 2 (75 visits, all pass 1, all
   ply 2, no rung). report5's verdict: escalation must be scheduled, not
   emergent.
4. **The design dialogue (2026-10-01) — three insights.**
   (a) *Expansion is the parent's proof work* (AND: refuting every reply
   is literally the parent's proof; OR: the proof must come through some
   child) → the eligibility trigger: a censored node earns a revisit only
   when its whole exposed line is opened — `rung-eligible = censored and
   no virgin child`. (b) *Rungs must be scheduled* (L2) → the reserve
   share + interleave pacing. (c) *Expansion itself is rationed per ply
   layer* (report4 finding 3; the cheap facts measured at plies 4–27 in
   plan2/plan3) → per-ply-layer visit caps.
5. **plan6 §2 — the mechanism**, implemented and A/B'd (arms A/A′/B/C);
   verdict: **ladder no-go** — see §4 below and the measurements README.

## 2. Theory linkage

- **PN-search breadth explosion.** PN-search is native best-first and
  keeps the whole frontier in memory; the vendored extraction states the
  two classical problems verbatim: "PN-search uses a large amount of
  memory space because it is a best-first algorithm", and efficiency
  suffers from the frequent proof/disproof-number updates
  (`docs/theory/deep-dfpn-2017/deep-dfpn-2017.md`, §1/§2.1; original
  Allis et al. 1994, bibliography). Our harvest setting turns this into
  a *compute* rather than memory pathology: the number-1 pool × 4M base
  budget against a 300M session cap (plan5's 5.6B-vs-300M arithmetic) —
  the same monotone-frontier behavior, priced in child-evals.
- **DF-PN threshold escalation (Nagai 2002).** df-pn's answer to the
  breadth problem is depth-first re-search with thresholds raised across
  iterations — the vendored extraction: df-pn is "(1) selecting the
  most-proving node, (2) updating thresholds of proof number or
  disproof number in a transposition table, and (3) multiple iterative
  deepening until the ending condition is satisfied"
  (`deep-dfpn-2017.md` §1; cf. `pdfpn-2010.md` "node in df-pn is
  controlled by the thresholds of proof number and disproof number").
  The plan6 rung mechanism is this escalation in harvest-policy
  clothing: the revisit ladder budget `2^(k-1) × base` plays the
  threshold's role (a raised work bound per pass), the ledger's
  `passes_failed` plays the TT-threshold's role (path-dependent state,
  session-frozen — deliberately the plan4 simplification), and the
  rotation ("fewest-passes") is a fairness variant of the
  most-proving-node selection.
- **What is ours (no literature precedent found):** the eligibility
  trigger — a revisit only when the node's whole exposed line is opened.
  It is justified by the proof-work argument (§1.4a), not by numbers:
  while a virgin child exists, expanding it is strictly better than
  re-searching the parent, because the parent's proof must pass through
  it. The hybrid (breadth-first frontier expansion + scheduled,
  eligibility-gated, reserve-rationed escalation) is new as far as the
  surveyed literature goes; the escalation half is Nagai's, the
  eligibility gate and the rationing are this initiative's.
- **Nagai's thesis is not vendored** (entry status stays **Cited**): the
  vendoring attempt (2026-10-01) found no retrievable copy — the author
  site's Wayback snapshots carry no thesis PDF, scholar.archive.org and
  the search engines challenge-block scripted access. The linkage above
  is mined from the vendored secondary sources (`deep-dfpn-2017`,
  `pdfpn-2010`), which state the df-pn mechanics directly.

## 3. Considered-and-rejected alternatives

- **Pure expansion** (no rungs) — carried as *arm C* and measured; it
  **won** the pre-registered decision rule (facts 0 = 0, max ply 5 vs 4,
  the ladder's 80M evals bought nothing). As of plan6 it IS the shipped
  default; the rung machinery stays config-reachable (`reserve_share`)
  for later batches.
- **Pure / emergent ladder** (the pre-plan6 selector: let escalation
  happen when the queue orders it) — rejected by plan5 L2 and measured
  unreachable in batch 2 (E3); with the layer caps it would additionally
  never see a censored node at the queue top (fresh exposure always
  re-seeds number 1).
- **Work-aware ordering** (effective numbers including measured work) —
  rejected for the fresh pool by L1 (all ledger state zero there — no
  key can order it); re-rankable later *among rungs only* (where ledger
  state exists) if a future A/B motivates it — untested in plan6.
- **In-session number propagation** (recompute pn/dn after every visit)
  — out of scope (plan4 simplification, plan6 §6 non-goal): the census
  is order-dependent already by TT retention; re-propagation would make
  the session's selection state much heavier for a gain the A/B says is
  not there (facts come from cheap expansion layers, not revisits).

## 4. The measured verdict (closing the question plan5 opened)

Arms B vs C over the identical post-batch-2 state (300M each, staging
copies): B ran 55 expansions (plies 2/3/4: 24/24/7) + 9 rungs (root pass
2 and 3; seven ply-1 nodes pass 2; the rotation and mid-session
eligibility flip verified from the census `kind` trace) — all rungs
censored, 0 facts, max ply 4. C ran 75 expansions (plies 2/3/4/5:
24/24/24/3), 0 facts, max ply 5. Per the §4.5 rule (B ≤ C on facts; all
rungs censor; no depth dividend): **the ladder is a measured no-go at
this frontier** — the reserve share's default becomes 0 (arm C's
config), the mechanism ships as rationed expansion only, and the
eligibility trigger + pacing + growth knobs remain exposed for later
batches. The one open caveat: the eligibility trigger itself was
*exercised but never paid* — it selected exactly the nodes a
deeper/cleaner frontier would reward, so a future batch that grows the
cheap-fact plies (plies 4+, plan2/plan3's class) can re-test the ladder
by config alone, no code.
