# plan14 — GHI-safe cut-context table: Phase 0 prototype and go/no-go

Initiative: `dfpn` (reopened from dormant; trigger (a): a new measured
search-semantics diagnostic for the deep, repetition-dominated class).
Executes one new backlog item: **context-keyed threshold-cut reuse** — the
intersection of the three families named by the 2026-10-29 brainstorm
(child `pn`/`dn`, threshold-cut context, GHI-safe transposition context).
This plan is Phase 0 only: a temporary, env-gated prototype plus a
current-HEAD control matrix. No product behavior changes. If the pre-registered
GO gate passes, `plan15.md` is the productionization + full-corpus gate rollout;
if it fails, the item closes as measured-out.

## Why this mechanism is the surviving candidate

The three brainstorm families were checked against the code and the recorded
closures before probing:

- **Child `pn`/`dn`.** The solver already reuses unsolved child `pn`/`dn` from
  the TT (`ChildInfo::unsolved_bounds`), stores the selected child
  (`best_move`/`best_child`) and child work, and has the TT pre-probe. The
  closed variants are leaf initialization (`research` plans 15/16),
  Deep df-pn depth-keyed leaf values (`research` plan9), and cross-clock
  solved-fact reuse (`dfpn` plans 10/11). A plain “TT cutoff when stored
  `pn`/`dn` exceed the current thresholds” was measured in this plan session
  and **regresses** both hard cases (see below).
- **Threshold-cut context.** The TT currently stores only the frame's
  resulting `pn`/`dn`; the *thresholds at which the frame cut* are lost. PDS
  stores per-node thresholds in its table (mined as a “constant-factor layout
  change” in `research/research_alternative_algorithms.md`), but whole-PDS is
  evidence-against. The untested residue is a small side table of cut
  contexts, not a search-algorithm swap.
- **GHI-safe transposition context.** Decisive Win/Loss proofs are
  path-independent by rule; repetition-dependent Draws are suppressed and
  handled by the exact-context plan9 cache. Unsolved bounds may be
  context-dependent but are only heuristic. The safe shape is to *add* a
  side table keyed by the exact ancestor-context hash, never to remove or
  override the existing path-independent TT reuse, and never to store a
  decisive outcome.
- **Combination.** A cut-context entry keyed by
  `(position hash, order-independent ancestor-context hash)` records the
  thresholds and bounds of a previous threshold cut. On re-entry under the
  same context with thresholds dominated by the stored ones, the frame can
  return unsolved immediately (exactly what a threshold cut would have done,
  with no work). This is the one mechanism that measured a large, repeatable
  win on the hard class.

## Plan-session read-only probes (2026-10-29, HEAD `4466d1a`)

All probes used an isolated source copy with temporary instrumentation
(`/workspace/probe14`; product tree untouched), a release build, salt 0,
`--first-outcome --outcome-only`, 128/160 MB main TT or 256/320 MB where
noted. The probe build's `TtEntry` was enlarged to 80 B, so the main-table
labels were chosen to keep the **same bucket count** as the product table
(160 MB probe ≙ 128 MB product; 320 MB probe ≙ 256 MB product).

Current-HEAD clean baselines (product `TtEntry` 56 B):

| case | TT | child evals |
| --- | ---: | ---: |
| stress `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21` | 128 | 305,404,068 |
| m22 `4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22` | 128 | 19,111,111 |
| stress | 256 | 287,872,755 |
| m22 | 256 | 15,023,911 |

Archived plan12 numbers no longer hold at HEAD (dec01, for example, is
censored at 120 s on the clean HEAD binary even at 128 MB, where the plan12
row recorded 5.76 M). **Plan15 must re-baseline the gate corpus before
applying gate v1.0.**

Probe results, all on the fields that later became the cut-context entry
(`(key, context hash, cut thresholds, stored bounds)`), direct-mapped table:

| arm | main TT capacity | context table | stress | m22 |
| --- | ---: | ---: | ---: | ---: |
| full cut-context skip (bounded), main128 | 128 MB equiv | 2²² slots / 128 MB | **169,746,499** | **17,086,349** |
| full cut-context skip (bounded), main256 | 256 MB equiv | 2²² / 128 MB | **188,937,258** | **15,647,876** |
| full cut-context skip, no TT-entry fallback | 256 MB equiv | 2²² / 128 MB | 226,365,247 | 15,647,876 |
| AND-node-only skip | 128 MB equiv | 2²² / 128 MB | 162,368,337 | 17,433,179 |
| repetition-flagged frames only | 128 MB equiv | 2²² / 128 MB | 221,593,025 | censored |
| superceded unrestricted HashMap skip | 128 MB equiv | unbounded | 192,061,798 | 16,046,409 |

The bounded direct table reaches the same class of win without unbounded
memory; 2²¹ slots was insufficient (stress 155.8 M on main256 but m22
34.6 M, a 2.3× collapse), so the context table size is load-bearing.
The **bounds-fallback** variant is the implementable one: when the TT entry
is absent, the cut-context entry restores its stored unsolved bounds instead
of relying on a live TT entry. It kept the stress win (188.9 M) while making
the mechanism self-contained; without it the TT-required variant lost most
of the win (274.0 M stress, +2.3% m22).

Salt-0 controls for the bounds-fallback full table at main256 + 2²² context
slots:

| case | clean base | cut-context | ratio |
| --- | ---: | ---: | ---: |
| stress | 287,872,755 | 188,937,258 | **0.656** |
| m22 | 15,023,911 | 15,647,876 | 1.042 |
| dec10 | 3,901,847 | 2,292,227 | **0.587** |
| dec06 | 1,507,482 | 1,374,808 | **0.912** |
| dec13 | 5,030,497 | 5,072,982 | 1.008 |
| dec02 | 138,124 | 143,794 | 1.041 |
| m23 | 11,621,376 | 9,626,650 | **0.828** |
| dec01 | censored | censored (fewer evals) | — |

Thus the identified surface is not stress-only, and the main hard-class
control (m22) sits inside the noise band rather than collapsing. The small
dec02/dec13/dec06 changes are below or near gate-v1.0’s per-case thresholds
and must be re-measured over salts in plan15.

Rejected variants measured in the same probes (do not re-propose):

- plain TT cutoff on stored `pn`/`dn` (no threshold context): stress
  321,998,826 (+5.4%), m22 20,604,056 (+7.8%);
- cut-context skip without exact ancestor-context match: stress
  387,472,914 / m22 20,742,462;
- high-water threshold resume (raise current thresholds to the stored cut
  thresholds): stress 897,095,962, m22 18,559,188; OR-only stress censored;
- restricting unsolved-bound reuse to exact context (removing cross-context
  reuse): m22 censored at 120 s, stress censored at 300 s — cross-context
  bounds are load-bearing;
- AND-only cut-context (despite stress 162.4 M): dec02 +10.5%, dec06 +15.4%
  regressions, so the full-table shape is the candidate.

## Mechanism to prototype (normative)

Add a per-run, fixed-size, direct-mapped **cut-context table** alongside the
TT. It is a heuristic side table, never a source of solved outcomes.

### Entry

```
key          u64   // full position hash (includes rule50)
context      u64   // order-independent ancestor repetition-key context hash
cut_pn       u32   // thresholds of the threshold cut, saturated encoding
cut_dn       u32
pn           u32   // unsolved bounds stored by that cut frame
dn           u32
```

32 B per slot. Encode `u64 -> u32` as `min(x, u32::MAX)` with
`u32::MAX` acting as “infinite/≥”; decode for comparison as
`u32::MAX -> INF`. The saturation direction must be checked in the spike
(for cut thresholds it only *loses* some skips; for bounds it can only make
the guidance larger, never prove anything).

### Index

`idx = mix(key ^ context.rotate_left(17)) & (slots - 1)` using the existing
`crate::zobrist::mix`. Direct-mapped overwrite; a collision is a lost entry,
never a correctness issue. Full `(key, context)` equality is required on
probe.

### Store site

At the existing `dfpn` frame exit, *only* when the loop terminated by the
existing threshold-cut branch (`threshold_cut`), insert

```
(key, context_hash, th_pn, th_dn, store_pn, store_dn)
```

where `context_hash` is the frame-entry ancestor context already computed for
the plan9 repetition cache, and `store_pn/store_dn` are the unsolved bounds
written to the TT. Terminal/solved/first-player-loss-suppressed stores must
**not** insert cut contexts (a suppressed Draw is not a threshold cut).

### Probe site

After the frame-entry TT solved-result check (and after the path-repetition
return) and before the repetition-cache probe:

1. If a live TT entry is solved, do nothing (solved values always win).
2. Look up `(tt_key, context_hash)`. On a hit, require
   `th_pn <= stored.cut_pn` **and** `th_dn <= stored.cut_dn`.
3. If the TT entry is absent, store the cut-context `(pn, dn)` as an
   unsolved entry (with `best_move = Move::NONE`, `work = 0`,
   `remaining_depth = max_depth`) before returning. If a live unsolved TT
   entry exists, keep it; do not overwrite it with older table bounds.
4. Return `Outcome::Draw` without generating moves or evaluating children.
   This is exactly the existing threshold-cut return shape: no `NodeProven`
   event, no solved store, no call into the repetition cache.

### Termination guard

The skip can repeat while the parent keeps selecting the same child with
unchanged bounds. In normal work-chunked runs the existing “unchanged child
bounds ⇒ `explored = true`” rule breaks that loop. The mechanism must be
disabled when `restart_slice == u64::MAX` (no such marker exists) —
documented as a behavior-preserving guard. A unit test must run a
`RestartSchedule::NEVER` search with the feature on and prove termination;
the intended safe behavior is bit-identical to feature-off there.

### Soundness contract (normative, restate in the report)

- The table stores only unsolved bounds and cut thresholds. It never stores
  an `Outcome`; every hit returns `Draw` exactly like a threshold cut.
  Therefore it cannot manufacture a false decisive outcome; the worst defect
  is a delayed/false draw (a completeness/performance defect, not a proof
  defect).
- Exact ancestor-context match (64-bit hash, same convention as the plan9
  repetition cache) is required. Cross-context reuse is forbidden.
- A stored cut at thresholds `(C_pn, C_dn)` proves only “this subtree was
  already searched to at least these thresholds under this context and did
  not resolve”. The reuse condition `th_pn <= C_pn ∧ th_dn <= C_dn` is the
  monotone direction: a lower threshold cannot resolve more.
- The table is per-run, cleared in `begin_run`, never serialized into TT
  snapshots or proof artifacts, and never consulted by preflight/reconstruct.
- No solved fact, no repetition-cache payload, and no cross-clock/eviction
  change is involved (plan10/11 hazard structurally absent).
- The deterministic budget contract is unchanged: a hit consumes no child
  evals, exactly like a path-repetition/TT-solved frame; no wall clock enters
  a decision.

### Memory / configuration proposal (decide in Phase 0)

The measured configuration that matters is “main-table capacity ≈ today's
default plus a 2²²-slot context table”. Because `224 MB` is an exact
56 B-entry label (`224 MB / 224 B = 2^20` buckets, the same bucket count as
today's 256 MB default), the proposed product shape is:

- main TT `224 MB` (same bucket count as the current `256 MB` default);
- cut-context table `2^22 × 32 B = 128 MB`;
- total default label `352 MB` (or a new `--cut-context-size` knob with
  `0` = off, default 128 MB).

This is +96 MB over the current 256 MB default with no main-table capacity
loss. If the owner prefers strict “RAM = TT only”, express the space as part
of the TT reservation (e.g. `--tt-size 352` split 224/128) and update the
TT module docs and AGENTS.md accordingly. The Phase 0 spike measures
2²¹ vs 2²² to pin the lower bound; 2²¹ was materially insufficient in the
plan-session probes.

## Phase 0 tasks (one execution session)

1. **Baseline.** Clean HEAD build; record salt-0 `child_evals` for stress,
   m22, dec10, dec06, dec13, dec02, m23, dec01 at 128 and 256 MB. Confirm
   the archived plan12 baselines are stale and record the current numbers as
   the only comparisons used in this plan.
2. **Spike.** Implement the table and the two call sites behind
   `DFPN_CUTCTX=1`, `DFPN_CUTCTX_SLOTS=2^b`, `DFPN_CUTCTX_STATS=1`
   (temporary only). Start from the plan-session probe shape; do not
   productionize yet.
3. **Identity/liveness.** With `DFPN_CUTCTX` unset, reproduce every baseline
   exactly. With it set, confirm hits > 0, outcomes unchanged, and no
   `ProofEvent`/snapshot interaction. Count hits, misses, collision
   overwrites, bounds-fallback stores, and skipped child evals.
4. **Salt-0 matrix.** Main-capacity-matched runs for stress, m22, dec10,
   dec06, dec13, dec02, m23, dec01 at context slots 2²¹ and 2²². Also run
   the feature at the current 256 MB main label for the absolute comparison.
5. **Quick suite.** `cargo run --release --example benchmark -- --suite
   quick --json --first-outcome` with feature off and on; zero outcome
   flips is the hard gate (child-eval changes on the affected surface are
   expected). Record per-case ratios.
6. **Soundness.** `DFPN_CUTCTX=1 cargo test --release --test test_repetition
   -- --include-ignored` (cyclic rook never `Win`); a `RestartSchedule::NEVER`
   termination test; `make test` with the temporary code still in the tree
   (or after reverting the hook, as the plan decides). Run `cargo fmt` /
   `clippy`.
7. **Revert + report.** Remove the spike, `git diff --exit-code` clean,
   re-run the salt-0 anchors exact. Write `report14.md` with the matrix,
   liveness/soundness evidence, the chosen memory split, and the GO/NO-GO.
   Update the `dfpn` initiative row/History and, only on a GO, draft
   `plan15.md`.

## Pre-registered gates

**GO to plan15** iff all hold on the Phase 0 matrix:

1. stress first-outcome improvement ≥ 10% (plan-session probe: 0.656× at
   main256 + 2²² slots);
2. m22 ≤ 1.05× baseline (probe 1.042×) and not censored where baseline
   solves;
3. every hard-class case (stress, m20/m22/m23) has no censored-loss and no
   ratio > 1.10; dec10 and dec06 ≤ 1.05×; dec13 and dec02 ≤ 1.10×;
4. quick suite 59/59 outcomes unchanged, zero decisive-outcome conflicts,
   cyclic-rook/repetition gate green;
5. liveness (hits > 0) and the `RestartSchedule::NEVER` termination test
   green;
6. no unexplained memory beyond the declared split.

**NO-GO** on any outcome flip, cyclic-rook violation, m22 collapse, stress
< 10%, or any hard-class regression/ censored-loss. Then close the item and
record the three-family surface as measured out (the probes above plus this
plan are the closure evidence).

Plan15 (only on GO) is the productionization and full-corpus gate rollout:
fresh current-HEAD baseline over the canonical 22 cases × 6 salts, then the
feature arm, gate v1.0 applied verbatim with the re-baselined numbers, plus a
`make test-full` run. The plan-session probe numbers are evidence, not gate
verdicts.

## Out of scope

- Full-corpus/salt rollout (plan15).
- Solved-result or Draw caching (plan9 plan/plan21 classes).
- Leaf initialization, Deep df-pn, cross-clock reuse, TT eviction, ordering,
  parallelism, preflight, proof-tree/PV changes.
- Any change to the path-independent TT store semantics or the first-player-
  loss shortcut.

## Final task

Write `report14.md` in this directory: the clean current-HEAD baselines, the
Phase 0 matrix (all arms, hit/collision/fallback counters), quick-suite
outcome table, soundness/termination evidence, the memory-split decision, and
the GO/NO-GO. Record the probe artifacts/commands in the report; if GO, the
report ends with the `plan15.md` kickoff prompt.
