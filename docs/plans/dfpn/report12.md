# Report 12: KQvK ladder — repetition-semantics resolution + work-mass reduction (CLOSED as evidence-based no-go)

Plan: `plan12.md` (backlog #5). Executed as **Session A only** (Phase 0) per
the plan's two-session protocol; the user checkpoint confirmed the no-go on
2026-09-15, so Session B (Phase 1) was never started and no solver code
changed. The full Phase 0 substance — T0 reproduction, T1 semantics
resolution, T2 work-mass diagnostics, G1/G2 gate outcomes, the R1
attribution table, deviations, and problems — is `report12_phase0.md` in
this directory; raw logs and the archived spike sources are under
`measurements/plan12/`.

## Outcome

- **G1: GO.** All four ladder positions are genuine wins under the solver's
  path-repetition semantics (T1a exhaustive strategy certificate + T1b
  independent repetition-aware prover, exact DTM 15/13/11/9).
- **G2: FAIL.** Attributed cacheable mass on the gate object (bounded
  `search_depth(·,15)` root run) = **8.08%** of the ~2.83B-child-eval gap
  vs the ≥ 50% gate. All three pre-registered arms fail: Arm A2's lemma is
  vacuous on the near-strongly-connected 3-man graph, Arm D's mechanism
  measured 0 events, Arm C fails its "cheaply" precondition (T1b root proof
  ≈ the solver's own failing cost).
- **Closure: evidence-based no-go** (the plan10/plan11 pattern). The
  dominant mass (91.3% class-3) is unsolved frontier churn — the search
  never resolves anything inside the horizon — which no
  repetition-semantics change can touch.

## Transferable results

- The KQvK ladder is a characterized benchmark class; the addendum's
  baselines are chunk-boundary node counts and reproduce bit-for-bit
  (ground truth: `docs/plans/egtb/measurements/plan1/egtb3-q.bin`).
- The repetition machinery (first-player-loss shortcut, plan9 cache) is
  measured inert on this class — a negative result that scopes all future
  repetition plans away from it.
- The successor lever, recorded in `initiative.md` backlog #5's closure
  entry: **bounded-search horizon/threshold pricing** — why a layered
  AND/OR fixpoint decides the whole ~420k-position region in ~6 s while
  the bounded DF-PN search resolves nothing in 300 s. Candidate plan13
  material (layered bounded fixpoint pre-phase; region-closure draw
  proofs as a by-product), with `measurements/plan12/plan12_t1.rs` as a
  reusable independent oracle.

## Deviations, problems, missing tests

See `report12_phase0.md` (deviations 1–5, problems, missing tests). Nothing
added since: no code landed, the tree is byte-identical to the pre-plan12
state, `make test` is green, and no fixture change was triggered (the
ladder is not fixed, so no regression case is proposed).

## Next steps

- Re-rank the `dfpn` backlog (done in `initiative.md`; #3 remains the only
  open item).
- Draft the successor plan (horizon/threshold pricing) in a fresh session
  from this report's data.
