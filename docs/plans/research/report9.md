# Plan 9 Report: literature mine — Deep df-pn 2017 (seesaw reduction via depth-dependent unsolved-leaf pn/dn)

## Summary

plan9 executed the desk mining of literature target #15 per `plan9.md`:
Phase 1 extracted Deep df-pn (Zhang, Iida, van den Herik, ACG 2017) to
`research_deep_dfpn.md` following the house template; Phase 2 classified the
mechanism and every in-passing variant against the solver's contracts, with
all code sites verified by reading the files (quoted excerpts in
`measurements/plan9/code-sites.md`, classification table in
`measurements/plan9/classification.md`).

The expected outcome held. **No variant lands in class (b): the gate
decision is CLOSED (H0), and backlog #15 closes with this record.**

## What the mechanism is

Deep df-pn changes exactly one thing relative to df-pn: the unsolved-leaf
value. Where df-pn assigns 1, Deep df-pn assigns
`D_dfpn(depth) = E^(D−depth)` (Definition 1; E = "threshold of branch
size", D = "threshold of depth"), propagating through the standard min/sum
folds. Expanding a node with fewer than E children *shrinks* its folded
value, so the search keeps descending instead of returning to an ancestor —
the stay-deeper bias against the seesaw effect (Theorem 1). The paper is
explicit that this is the same behavioral dial as the 1+ε trick, controlled
from a different site (§4: "two methods"); Theorem 1 assumes a pure tree
(no TT, no transpositions, no repetitions) and E′ < E.

## Why it does not transfer

1. **The faithful mapping is contract-breaking at a named line.** A
   path-depth-keyed leaf value makes the unsolved bounds stored at frame
   exit — `core.rs`: `(store_pn, store_dn) = (pn.max(1), dn.max(1))` —
   path-relative: the same position reached by different lines stores
   different `(pn, dn)` in `TtEntry` (`src/search/tt/entry.rs`; no
   path-depth field exists). That breaks the documented path-independent
   base-entry contract, and the probe side (`children.rs::evaluate_child`,
   which reuses `(e.pn, e.dn)` under a remaining-depth-only guard) imports
   foreign-path valuations — voiding Theorem 1's own consistency premise,
   i.e. the mechanism's intended effect does not survive its own caching.
   The crux question of plan Context 2 resolved to: a depth source *does*
   exist at the leaf sites (`path_stack.len()` is already read in
   `evaluate_child`), so the failure is not "no depth proxy" but
   "any path-derived proxy poisons the stored bounds".

2. **Every path-independent workaround is measured- or evidence-fatal.**
   Never storing deep-derived bounds discards the unsolved-bounds reuse
   that is the search's working set (direct analog: plan7 V1 — stress
   timeout, m22 +3400%). Path-depth-tagged entries need a layout change and
   collapse transposition reuse in the 14 M–250 M-eval regime. Position-only
   proxies (halfmove clock, material) mutate the mechanism and point the
   wrong way (the clock marks quiet *deep* lines); no published evidence.

3. **The behavioral target is already covered and its dial measured-closed.**
   The 1+ε threshold recursion is the paper's method (1) for the same
   stay-deeper effect; the ε surface has four closure legs (plan4 constant/
   schedule/chaos, plan8 node-local signal). Deep df-pn would be a second
   knob on a dial whose response this solver measured as trajectory chaos.

4. **The evidence does not transfer.** Eight Connect6 positions (mostly
   4-move openings) at ≤ 500 k-node cutoffs with a relevance-zone/VCDT
   sub-solver and top-5 attacker widening confounding every number;
   per-position best-of-300 parameter tuning with winning (E, D) spread over
   E ∈ [3, 18], D ∈ [2, 6] — no transferable default; and in the paper's own
   head-to-head (Table 2), best-tuned 1+ε matches or beats Deep df-pn on
   node count in 4 of 8 positions (the average favors Deep via outlier
   positions 5 and 7). Our gate objects (m22 14.2 M, stress 249.5 M
   first-outcome child evals, repetition-dominated, tree ≫ TT) are 2–3
   orders of magnitude outside the tested regime, and the authors themselves
   restrict the claim's comfort zone in §7.

## Classification summary

| Variant | Class |
|---|---|
| Faithful Deep df-pn (path-depth leaf values, TT as-is) | (c) contract-breaking |
| + path-depth-tagged TT entries | (c) |
| + never store deep-derived bounds | (c) |
| Position-only proxies (rule50 clock, material) | (d) |
| E = 0 degenerate arm | (d) |
| Behavioral target (stay-deeper bias) | (a) covered — implemented 1+ε; dial closed by plan4/plan8 |
| Evidence transfer to the stress class | (d) |

## Gate decision: CLOSED

Per the plan's decision table: all mechanisms land in (a), (c), (d).
Backlog **#15 closes** with the classification as the seesaw-thread no-go
record. DeepPN 2015 remains not-mined-separately per the recorded scope
decision (bibliography row `Cited`). No POC was proposed; the sizing sketch
in `research_deep_dfpn.md` §7 is archived for the record only (estimated
2–3 sessions, negative expected value).

## Hand-off / closure record

- **#16 (`structural_floor.md`, plan10) is the next and likely last plan.**
  The seesaw thread feeds it as follows: the stay-deeper dial exists in this
  solver at the threshold site (1+ε, implemented and tuned); the leaf-value
  site is path-dependent at its core with no faithful path-independent
  mapping (this report); the published head-to-head does not favor the
  leaf-value site even in its home regime. Suggested framing for the #16
  seesaw item: *the solver's search-order guidance is confined to
  position-only signals (static scorer, history, killers, TT bounds) and
  path-only thresholds (1+ε); every depth-keyed valuation mechanism in the
  literature is either path-dependent (Deep df-pn) or best-first
  (DeepPN/PN²), both excluded by the TT and RAM contracts.*
- No hand-off to implementation initiatives: nothing was measured, nothing
  is exploitable.
- No wall-time work, no `src/`/`examples/` changes, no benchmark runs —
  no claim rests on a new measurement.

## Verification

- `git status --porcelain src examples` — clean (mining plan; docs-only
  changes).
- Every claimed code site verified by reading
  `src/search/dfpn/{children,core,selection,mod}.rs` and
  `src/search/tt/{entry,table,mod}.rs`; the path-dependence argument quotes
  the actual `TtEntry` fields and the `core.rs` frame-exit store
  (`measurements/plan9/code-sites.md`), per the plan's verification
  requirement.
- Bibliography (DeepPN 2015 `Cited`, Deep df-pn 2017 `Mined` with pointer)
  and backlog #15 / History in `initiative.md` updated and cross-checked
  against this report.
- Housekeeping: `cargo fmt --check`, `cargo clippy --release --all-targets`,
  `make test` — green (no source changes; Boy Scout hygiene check).

## Problems encountered

- None material. The OCR extraction (`docs/theory/deep-dfpn-2017/`) is
  complete including Tables 2–3; the "complete algorithm" web PDF referenced
  in the paper's footnote is not vendored, but the paper states the
  mechanism is standard df-pn plus Definition 1, which sufficed.

## Unresolved parts / missing tests

- None blocking. The DEFER gate was not needed: the depth-proxy question
  settled from the in-repo source plus code reading (a proxy exists; the
  failure is at the store/reuse sites).
- Theorem 1's behavior under TT reuse was never formalized anywhere in the
  literature (the paper has no TT in its model); the argument in
  `research_deep_dfpn.md` §5.1 is this repo's reasoning, not a published
  result.

## Next steps

- plan10: #16 `structural_floor.md` consolidation per the 2026-09-21
  re-scope. With #15 closed, every literature target (#5, #8, #15) and every
  POC candidate (#10–#14) is answered/closed; #6/#7/#9/#13 remain
  pre-weakened with recorded blockers.
- After #16, the initiative should close (nothing left open).
