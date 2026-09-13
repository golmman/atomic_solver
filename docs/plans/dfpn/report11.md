# Report: Plan 10 Resurrection via Decoupled Reuse — Phase 0 No-Go (Plan 11)

## Summary

Executed `plan11.md`'s Phase 0: a seven-arm measurement matrix attempting to
decouple plan10's cross-clock solved-entry reuse (−37.7% stress child evals,
`report10.md`) from the DF-PN threshold destabilization that killed it. The
verdict is a second, sharper no-go:

- **The two proposed mechanisms fail in complementary, now precisely
  localized ways.** Mechanism B (site-2 direction split with quarantine)
  fails in every topology: quarantining AND-parent Win facts kills the
  stress win (B1: timeout, +110% evals), adopting them solved kills the
  m22 control (B2: stress −36.1% but control timeout with 10.55M
  AND-side folds). Mechanism A (frame-entry adoption with TT store-back)
  is the only arm that is simultaneously sound, control-safe, and
  drift-free — but delivers only **−8.8%** first-outcome / **−2.6%**
  default-mode on the stress case, below the plan's ≥10% gate, and the
  default-mode number is decisive: `make stress` runs default mode.
- **The knowledge advance over report10**: its "the benefit and the
  pathology are one mechanism" is now pinned down. The pathology is
  specifically **adopted-Win facts completing Loss claims at AND parents**
  (the direction that carries 85% of the adoption mass — and the stress
  win). Every mechanism that lets that mass complete proofs destabilizes
  the control; every mechanism that suppresses it kills the stress win.
  Five decoupling designs (quarantine topologies ×3, frame-entry channel,
  prefetch channel) all land on one horn or the other.
- Per the working agreement, **all spike instrumentation was reverted**;
  the restored binary reproduces the post-plan9 stress baseline exactly
  (identical chunk log, 13,907,467 nodes, 477-ply line). Backlog #2 should
  be **closed permanently**; the remaining levers are parallelism,
  Expected-Work-Search, clock-pressure ordering, and refinement-after-
  cap-cut.

## What was built (all reverted)

Temporary, env-gated spike instrumentation (plan10's pattern):

- `src/search/dfpn/spike11.rs` (deleted): arm parsing
  (`DFPN11_ARM=PLAN10|A|B1|B2|B3|AB|A2`, `DFPN11_SPIKE_OFF=1`, default
  Off — normal runs and tests bit-identical), the cross-clock index
  (`HashMap<u64, (Move, Outcome, u32)>` keyed by the board-only
  repetition key, unbounded in the spike), and adoption/counter stats.
- Store site (`core.rs` frame exit): identical to plan10 — fully solved
  Win/Loss frame results only; draws (incl. GHI-suppressed) and unsolved
  bounds never enter; terminal-branch stores excluded.
- Site 1 (`core.rs`, arms A/AB): frame-entry adoption with TT store-back —
  on a hit passing `rule50 + d ≤ 100`, `d ≤ max_depth`, and the one-ply
  guard, store the solved result at the current full key, emit
  `NodeProven`, return the outcome. The parent learns the fact only
  through the pre-existing exact-key `evaluate_child` path (plan11.md
  Fact 1).
- Site 2 (`children.rs`, arms PLAN10/B1/B2/B3/AB/A2): probe when the
  exact-key `resolved` is `None`, under the same adoption rule. Direction
  split per arm: Loss facts always solved; Win facts solved (PLAN10),
  quarantined (B1; B2/B3/AB per parent type — `outcome: None`,
  `explored: true`, bounds `(INF, 1)` at OR / `(1, INF)` at AND), left
  live (AB at AND parents), or prefetch-store-back (A2).
- Temporary `examples/solve_stats.rs` runner. All removed;
  `git restore src/ examples/` returned the tree to HEAD.

Two spike-build validation anchors held exactly: `DFPN11_SPIKE_OFF=1`
reproduced the post-plan9 baselines bit-for-bit, and `DFPN11_ARM=PLAN10`
reproduced report10's full-lever run exactly (8,544,142 nodes /
155,394,450 evals / 303-ply line / 169,978 index entries; realized saving
94,086,028 = report10's "94.1M").

Methodology notes: (a) this spike's site-2 adoption counter reads 491k vs
report10's 1,111,158 — a counter-definition difference only (this spike
counts post-rule, post-guard successful adoptions per `evaluate_child`
call); behavioral equivalence is proven by the exact match of nodes,
evals, PV length, and index entries. (b) The AB arm initially came back
byte-identical to B2, which exposed a spike bug: the direction-split
match had no "leave live" outcome, so AB's AND-parent Win facts silently
fell through to solved adoption. Fixed to a three-way
`Solved|Quarantine|Skip` decision; the AB row below is the corrected arm.
A lesson for future spikes: an arm whose output is byte-identical to
another arm is itself the regression test.

## The histogram report10 named as its missing diagnostic

Adoption direction × parent type on the stress case (arm PLAN10,
first-outcome; 491,031 adoptions):

| direction × parent | count | share |
|---|---|---|
| Loss @ OR parent (winning move found) | 4,842 | 1.0% |
| Loss @ AND parent | 2 | ~0% |
| Win @ OR parent (refuted attacker move) | 69,702 | 14.2% |
| **Win @ AND parent (proof-completing attacker win)** | **416,485** | **84.8%** |

report10's differential table had already shown Loss-fact adoption is
control-safe (m22 3.1 s, 13.4M) — but Loss facts are only 1% of the mass.
The stress win lives almost entirely in AND-parent Win facts, which is
exactly the direction report10's control case dies from (this spike
measured 10,553,135 AND-side folds on the m22 run under B2).

## Measurements

All runs: 128 MB TT, default ε, reference host, first-outcome mode unless
noted. Baselines re-verified bit-for-bit immediately before measuring.

| arm | stress FO child evals (Δ vs 249,480,478) | stress FO verdict | m22 control |
|---|---|---|---|
| baseline (SPIKE_OFF) | 249,480,478 | 13,907,467 nodes / 53.8 s / 477 plies | win 3.0 s / 14,156,269 |
| PLAN10 (validation) | 155,394,450 (−37.7%) | win, 303 plies | timeout (report10 replica) |
| B1 (Win quarantined both parents) | timeout (524,405,831) | fail | timeout (132,474,691 @ 30 s) |
| B2 (Win solved @ AND, quarantined @ OR) | **159,489,840 (−36.1%)** | win, 93 plies | **timeout** (179,860,933 @ 30 s; 10,553,135 AND-side folds) |
| B3 (Win solved @ OR, quarantined @ AND) | timeout (522,824,528) | fail | timeout (151,939,652 @ 30 s) |
| **A (frame-entry + TT store-back)** | **227,597,289 (−8.8%)** | win, 301 plies | **win 3.9 s / 18,139,173 (1.28×)** |
| AB (A + OR-parent quarantine) | 250,830,277 (+0.5%) | win, 189 plies | win 3.6 s / 16,704,497 (1.18×) |
| A2 (site-2 prefetch store-back) | timeout (711,647,455) | fail | win 4.8 s / 24,373,971 (1.72×) |

### Arm A's full profile (the near-miss)

- Stress FO: 227,597,289 evals (−8.8%), 49.8 s; 58,206 site-1 adoptions.
- Stress default mode: 329,231,213 total (first-outcome phase 227.6M),
  **−2.6% vs the 338,094,183 baseline** — the refinement tail barely
  benefits. `make stress` runs default mode, so A does not address the
  actual complaint.
- Quick suite (`benchmark --suite quick --json --first-outcome`, 59
  cases): **byte-identical** — every per-case `child_evals` equal, zero
  outcome flips. A's drift surface is exactly the deep-clock-variant
  class.
- Cyclic rook (`8/8/8/8/2k5/8/8/4KR2 w - - 0 1` and `… w - - 90 1`): Draw
  at both clocks, never a Win; the draw-only search never even populates
  the index (0 entries).
- Index working set: 255,971 entries (stress FO) / 272,337 (stress
  default) — the spike index was unbounded; production would need the
  `1 << 18` cap raised (`1 << 19`), unlike plan9's cache whose working
  set sat two orders of magnitude below its cap.

## Why each arm fails: the mechanism map

1. **Quarantine topology (B1/B3)** — AND-parent Win facts are what let an
   AND node claim Loss from clock-variant proofs; suppressing them
   (explored, neutral bounds) stalls proof completion at every clock
   variant and the stress case re-searches everything (+110% evals).
   OR-parent quarantine alone adds frame-exhaustion churn (27k
   breaks-with-quarantined under AB) worth more than it saves.
2. **Solved AND-parent adoption (B2/PLAN10)** — the control dies from
   exactly this direction at volume (10.55M folds on m22).
3. **Frame-entry channel at low volume (A)** — safe (the parent only
   learns facts it descended into, through the ordinary exact-key path),
   but its coverage is only ~12% of plan10's adoption mass; −8.8% FO,
   −2.6% default.
4. **Frame-entry coverage raised to evaluation volume (A2)** — the
   store-back then folds at ~plan10's volume and the stress case
   destabilizes worse than any other arm (711.6M, timeout). There is no
   TT representation of "this child is refuted; skip it; don't let it
   claim anything": a TT-resolved entry always folds decisively. The
   quarantine cannot live in the TT, and everything that lives in the TT
   at volume destabilizes.

**Arm C (bounded cross-path verification) was not run — documented
deviation from the plan's contingency.** Rationale: (a) the verification
cost for AND-side facts is precisely the re-proof the lever exists to
avoid — under B2 the m22 run alone would trigger ~10.55M bounded
searches, and even a ~100-eval average exceeds 1B evals on the control;
(b) plan2 (conversion initiative) already measured root-scale
verification of otherwise-unproven claims as a hard no-go on this exact
position family; (c) a nested verification search inside
`evaluate_child` violates its documented never-recurses invariant and is
not spike-scale work. Closing backlog #2 with five measured falsifications
plus this prior is the honest state of the evidence.

## Decision

**Backlog #2 is closed permanently.** The gate outcome: no arm passes
(stress ≥ 10% **and** m22 ≤ 2× wall). Arm A's −8.8% FO is a real,
sound, drift-free measurement — recorded here as a known option — but it
was not promoted: it misses the plan's gate, mildly regresses the
designated control (+28% evals), and does nothing for the default mode
the `make stress` target actually runs. Re-tuning the adoption rule or
quarantine bounds on an unlanded mechanism is a plan non-goal and was
not attempted.

## Tools/Examples Used

- Temporary `examples/solve_stats.rs` runner and env-gated spike
  instrumentation (all reverted; baseline reproduced exactly after).
- `benchmark --suite quick --json --first-outcome` for the drift
  comparison (baseline JSON vs arm-A JSON diffed per case).
- No external tools beyond the repo's own binaries.

## Problems Encountered

1. The spike's first AB implementation silently degraded to B2 (missing
   "leave live" decision) — caught because the run was byte-identical to
   B2, which is itself a useful spike-debugging heuristic.
2. plan10's adoption-count definition could not be reconciled exactly
   (491k vs 1,111,158) without re-deriving its reverted counter;
   behavioral equivalence was instead established via the exact match of
   work counters, nodes, PV, and index size.
3. Arm C was skipped contrary to the plan's contingency clause — the
   rationale is documented above so the deviation is auditable.

## Unresolved Parts

- None on the lever itself: the mechanism map is complete for every
  decoupling design proposed. The unexplored remainder (verification at
  adoption volume, rule re-tuning) is documented with strong negative
  priors and should stay closed absent new research input.

## Missing Tests

- None added; with a no-go there is no feature surface to test. All
  soundness evidence (cyclic rook at two clocks, quick-suite outcomes,
  byte-identical drift) is reproducible from this report and the env-gated
  arms as described.

## Next Steps

- **Close `dfpn` backlog #2 permanently** (initiative updated). Backlog
  #4 (bounded cross-path verification for general TT reuse) stays open
  but its prior dropped: arm C's analysis suggests verification at
  adoption volume is uneconomical on this position family.
- The stress case's remaining levers are the ones identified before this
  plan: **parallelism** (`conversion` #4 / `lean` #2 — the only
  multiplicative lever), the **EWS/MOPNS reading round** (`conversion`
  #5), **clock-pressure ordering** (`conversion` #3), and **refinement
  after cap-cut** (`dfpn` #3, default mode only).
- The sequential solver's node count on this position class should be
  considered measured-optimal for DF-PN+ with the plan9 repetition cache
  under the current repetition semantics (first-player-loss shortcut,
  rule50 in the key).
