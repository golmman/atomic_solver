# Report 13 — Phase 0 (Session A): bounded-search pre-phase architecture bake-off

Plan: `plan13.md` (backlog #6). Date: 2026-09-16. Raw logs and spike sources
under `docs/plans/dfpn/measurements/plan13/`. Session B (Phase 1) was **not**
started; per the plan's execution protocol this report ends with the G-A/G-B
outcomes, the architecture recommendation (R), and the R1–R5 review status for
the user checkpoint. `src/` is byte-identical to HEAD (verified below).

## TL;DR

- **G-A: GO — via candidate R only.** The **region-closure fixpoint (R)**
  decides the ladder root as a replay-verified, cycle-free WIN at
  **5,827,440 child evals / 0.67 s** (table-free), with exact DTM 15 —
  0.29× of the 20M-eval bar and 1.1% of the 60 s bar — and decides all four
  ladder positions consistently (dtm 15/13/11/9). At horizon dtm−1 it
  correctly returns UNKNOWN; at dtm+2 the cost is unchanged (no explosion).
- **P (PN bounded prover): measured no-go.** Certifies step3 (dtm 9) at
  81,411 child evals / 0.03 s — a 370× improvement over plan12 T1b's
  table-guided DFS at the same depth — but explodes in *tree-node memory*
  from dtm 11 up (8M-node cap hit on step1, step2, and the root at every
  horizon; a 16M-node root probe also fails). The bottleneck is memory, not
  child evals.
- **L (layered certificate fixpoint): measured no-go.** ~5–25× per-ply layer
  growth; the root is eval-cap-censored at layer 8 (400M evals), and on the
  G-B oracle it *claimed* WIN on all 85 certifiable win samples but **zero of
  those claims produced a replay-verifiable certificate** (all safely
  downgraded) — the certificate-first context-free fact encoding is defeated
  by strategy board-set growth.
- **G-B: PASS.** 240 sampled KQvK placements (120 table-Win / 120 table-Draw)
  against `egtb3-q.bin`: **zero contradictions** for every candidate; R
  additionally proved all 120 draws correctly and 0 region-vs-table
  mismatches; every emitted WIN certificate replayed cycle-free.
- **Recommendation: proceed to the Session-B checkpoint with R** as the
  Phase 1 architecture (plan13 Phase 1, R1–R5 as drafted; no overrides
  needed).

## T0 — baseline confirmation (light)

Clean HEAD build (3339541), temp runner (`plan13_run`, archived), 128 MB TT,
default ε. Log: `measurements/plan13/t0_baseline.log`. The plan13 T0 scope is
steps 1–3 bit-for-bit plus the root bounded-15 as a band; all reproduce:

| FEN | mode | metric | this run | plan12 T0 | status |
| --- | --- | --- | ---: | ---: | --- |
| step3 `8/5K2/…/4Q3 w - - 6 4` | default | nodes | 741,940 | 741,940 | **bit-for-bit** |
| | | child evals | 2,047,932 | 2,047,932 | bit-for-bit |
| step2 `8/8/2k1K3/… w - - 4 3` | default | nodes | 5,395,869 | 5,395,869 | **bit-for-bit** |
| | | child evals | 39,383,178 | 39,383,178 | bit-for-bit |
| step1 `8/1k1K4/… w - - 2 2` | default | nodes | 65,460,435 | 65,460,435 | **bit-for-bit** |
| | | child evals | 322,826,558 | 322,826,558 | bit-for-bit |
| root `8/2K5/k7/… w - - 0 1` | bounded-15, 300 s | last chunk boundary | **796,353,729** | 796,353,729 | **bit-for-bit** |
| | | total nodes | 1,138,040,832 | 1,105,641,472 | band ✓ (R7; host ~3% faster) |
| | | child evals | 2,915,855,192 | 2,832,550,718 | band ✓ |

Root default (600 s) was not re-run (plan12's 5,396,315,458-eval number is
the baseline of record; the bounded-15 boundary match reconfirms the
deterministic work schedule unchanged). Post-revert, the clean binary
reproduces every work-chunk boundary of steps 1–3 bit-for-bit again
(`measurements/plan13/post_revert_verify.log`).

## T1 — bake-off (table-free)

Instrument: temp example `plan13_t1` (archived). All three candidates share
the solver's terminal classification, real `atomic_movegen` movegen, the
path-repetition cut (a position on the current line is a Draw), and one
shared **replay verifier**: a WIN claim only counts if the extracted full
attacking strategy, replayed from the root with the defender branching over
*all* replies, mates within the horizon bound on *every* line with *no board
repeating anywhere on the line*. The egtb q-table is loaded only as a
validation oracle (`table_mismatches` counter, `--oracle` sampling) — the
table-free record runs produce byte-identical search numbers
(`r_ladder.log` contains both).

Metric convention: primary = child evals (one clone+do_move per generated
child — the solver's `child_evals` analogue). R additionally reports
*fixpoint scans* (child-edge consultations during relabeling passes) and
*region* (closure size) separately.

### R — region-closure fixpoint (`r_ladder.log`)

Forward BFS closure within a 500k-position budget, AND/OR fixpoint with exact
ranks (plan12 T1a machinery, table-free), rank-decreasing strategy; the
horizon is the certificate line-length bound. The fixpoint runs once per
position; per-horizon work is a replay only (≤ 0.01 s).

| position | h=dtm−1 | h=dtm | h=dtm+2 | region | child evals | scans | wall |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| root (dtm 15) | UNKNOWN (cert-fail) | **WIN_CERTIFIED** | WIN_CERTIFIED | 420,532 | **5,827,440** | 117.2M | 0.67 s |
| step1 (dtm 13) | UNKNOWN (cert-fail) | **WIN_CERTIFIED** | WIN_CERTIFIED | 420,532 | 5,827,440 | 117.1M | 0.66 s |
| step2 (dtm 11) | UNKNOWN (cert-fail) | **WIN_CERTIFIED** | WIN_CERTIFIED | 420,532 | 5,827,440 | 111.0M | 0.66 s |
| step3 (dtm 9) | UNKNOWN (cert-fail) | **WIN_CERTIFIED** | WIN_CERTIFIED | 420,532 | 5,827,440 | 110.8M | 0.67 s |

Certificates (replay-verified): 2,815 lines / 1,655 mates / max plies 15
(root), 800/472/13, 477/282/11, 23/12/9 — **identical line counts to plan12
T1a's rank-decreasing strategy**, cycle-free (0 repeats, 0 bad-strategy
exits), max plies = exact dtm on every root, dtm matching plan12's 15/13/11/9.
Region-vs-table cross-check: **0 mismatches** over all 420,532 decided
positions × 4 roots. All four roots share the same 420,532-position closure —
consistent with plan12's near-strongly-connected 3-man graph finding. h=dtm−1
fails via budget-exit in the replay and is **downgraded to UNKNOWN, never a
claim**.

### P — PN bounded prover (`p_ladder.log`, `p_root_8M.log`, `p_root_16M.log`)

Proof-number/disproof-number search to a fixed depth horizon, path
repetition cut, subset-transfer memo (T1b lemmas), PN selection replacing
both DF-PN thresholds and T1b's table guidance. Node cap 8M (~3.5 GB),
eval cap 2B, wall 240 s:

| position | h | result | child evals | tree nodes | wall |
| --- | --- | --- | ---: | ---: | ---: |
| root (dtm 15) | 14 | UNKNOWN (node-cap) | 10,260,539 | 8,000,001 | 20.7 s |
| root | 15 | UNKNOWN (node-cap) | 9,443,090 | 8,000,001 | 22.3 s |
| root | 17 | UNKNOWN (node-cap) | 9,712,672 | 8,000,001 | 23.1 s |
| step1 (dtm 13) | 12 / 13 | UNKNOWN (node-cap) | 9.42M / 9.24M | 8,000,001 | ~19 s |
| step2 (dtm 11) | 10 / 11 | UNKNOWN (node-cap) | 9.02M / 8.79M | 8,000,001 | ~17 s |
| step3 (dtm 9) | 8 | UNKNOWN (node-cap) | 9,520,984 | 8,000,001 | 19.5 s |
| step3 | 9 | **WIN_CERTIFIED** | **81,411** | 11,071 | **0.03 s** |
| step3 | 11 | WIN_CERTIFIED | 120,173 | 16,765 | 0.07 s |

Extended-memory probe: root, h=15, node cap 16M (~7 GB) → still
UNKNOWN (17,819,690 evals / 16,000,001 nodes / 40.3 s,
`p_root_16M.log`). Diagnosis: PN's search object is a *tree of paths*; with
path-repetition cuts the unproven frontier cannot be shared across paths, so
node memory (not child evals) explodes — the same structural behavior T1b
measured with DFS ordering (1.02B nodes at the root), now measured under
least-work-first pricing (68k–135k repetition cuts per root run; the subset-
transfer memo does record tens of thousands of proven facts but cannot make
the *unproven* frontier shareable). The step3→step2 cliff (11k → >8M nodes
for dtm 9→11) is ~2.5 orders of magnitude for two plies.

The hypothesis under test — PN's pricing avoids the poisoned-line
re-verification that made T1b depth-sensitive — is **falsified for the
root**: at dtm=dtm+2 on step3 it is spectacularly true (370× better than
T1b), but the tree memory explodes beyond dtm ~10 regardless of pricing.

### L — layered certificate fixpoint (`l_root.log`, `l_step{1,2,3}.log`)

Iterated depth layers d = 1..=h with a persistent fact set; WIN facts stored
certificate-first (chosen move + strategy board set, context-free by
construction), falling back to T1b subset-transfer when a fact's board set
exceeds 4,096 boards. Layers accumulate; eval cap 400M, wall 240 s:

| position | layers completed (layer evals) | outcome at cap |
| --- | --- | --- |
| root | d=1..8: 29 / 147 / 3,552 / 21,587 / 507,783 / 2,743,403 / 62,417,378 / 334,306,124 | eval-cap at d=8 (cumulative 400M, 228 s) |
| step1 | eval-cap at d=8 (cumulative 400M, 218 s) | UNKNOWN all horizons |
| step2 | wall-cap at d≈9 (286.6M evals) | UNKNOWN all horizons |
| step3 | eval-cap at d=8 (non-contended run: 88.2M cumulative at d=7, 47.5 s) | UNKNOWN all horizons |

Root growth is ~5–25× per ply with no fact support: at d=8 only 1,077
context-free WIN facts exist against **77,034 path-bound fallbacks** (the
strategy board sets exceed the cap almost immediately above the horizon, and
the fallback propagates upward), and fact hits are 31 — the layers re-search
nearly everything. Extrapolating the growth curve, the root (dtm 15) would
need ≥ 10¹⁰ evals. **The hypothesis under test — persistence across layers
removes re-exploration without subset tests — is refuted as implemented:**
certificate-first facts are too large to store, and their path-bound
fallback reintroduces exactly the ancestor-set subset machinery the
architecture was meant to avoid. The oracle (below) makes this vivid: on all
85 certifiable win samples, L *claimed* WIN but 85/85 certificate
extractions failed replay.

### Gates

- **G-A (go): PASS via R.** Root decided as a certified WIN at 5,827,440
  child evals and 0.67 s (bar: ≤ 20M / ≤ 60 s), table-free, all four ladder
  positions decided consistently with dtm-matching certificate depths
  (15/13/11/9). P and L individually fail by wide margins (censored at
  node memory / eval budget), but the gate is "at least one candidate".
- **G-B (soundness, hard): PASS.** `--oracle` sampled 240 legal
  attacker-to-move KQvK placements (LCG-sharded; side-not-to-move not in
  check; ≥ 200 required): 120 table-Win + 120 table-Draw. Per candidate,
  horizon h=6 (or the R-derived exact dtm when ≤ 6), eval cap 3M, wall 8 s:
  **0 contradictions** across all 720 candidate-runs (no WIN on a draw
  sample, no DRAW on a win sample, no NOT_WIN at h = dtm on a win sample, no
  uncertified claim emitted as decisive). R: 85/120 win samples certified,
  **all 120 draw samples proven DRAW** correctly, 0 region-vs-table
  mismatches. P: 85/120 win samples certified (26 dtm-1, 37 dtm-3, 22 dtm-5),
  0 downgrades. L: 0 certified, 85 claims safely downgraded. Logs:
  `oracle_shard{0..3}.log` (per-sample lines included).

## Architecture recommendation

**R — the region-closure fixpoint — for plan13 Phase 1.** It passes G-A with
a 3.4× margin on evals and ~90× on wall, its certificates are exact-rank
(hence provably minimal PV length — relevant to R1 below), it proves draws
for free as the fixpoint remainder (a *measured* by-product: 120/120 oracle
draws, no extra cost), and its region (420,532 positions, ~5.8M child-eval
closure build) is far below the detector's small-space gate. Its determinism
is total: closure BFS + delta-free relabeling passes are order-independent,
no memoization, no selection heuristics.

P and L are closed as measured no-gos for this class, with their failure
modes recorded (tree-of-paths memory explosion; per-layer re-search without
sound shareable facts). Per plan11's rule, no second-architecture iteration
was attempted within the session.

## What the bake-off teaches about general DF-PN threshold pricing

The lean9-class lever (reduce the 99.7–99.9% AND-frame mass in threshold-cut
frames on the *general* class):

1. The pre-phase's advantage is **not** "better ordering" — it is that a
   closure fixpoint prices every node exactly once (monotone relabeling with
   exact ranks), while bounded DF-PN re-walks the horizon with threshold-cut
   frames that resolve nothing (plan12's 91.3% class-3 churn). Any general
   lever must move toward *proven-fact accumulation per unit work*, not
   toward smarter DFS.
2. P's result is the sharpest data point: **pricing was never the whole
   problem** — PN with perfect least-work-first ordering still explodes
   because the bounded proof object (a tree of paths under path-dependent
   repetition cuts) has no DAG sharing for its unproven frontier. A general
   DF-PN change would inherit the same object; the small-space gate is what
   makes the fixpoint representation affordable.
3. L's failure identifies what a *sound* cross-context fact needs: a
   certificate-anchored board set, which grows with the strategy subtree —
   unaffordable beyond a few thousand boards. Context-free fact reuse in the
   general search (plan10/11 territory) has the same size wall.

## R1–R5 resolutions (as reviewed; no overrides requested)

- **R1 (PvStatus):** stands as decided — `PvStatus::PreflightProof` (final
  name at implementation). With R winning, the certificate carries exact
  ranks, so the Phase 1 PV will be provably length-minimal; the new variant
  still beats overloading `ProvenShortest` (honest under any future
  architecture, keeps the refinement-loop semantics of the existing flag).
- **R2 (no ProofEvent emission in round 1):** stands; nothing in the
  bake-off changes the asymmetry argument. Reversibility unchanged.
- **R3 (detector predicate, occupied ≤ 3 men, no pawns/castling):** stands.
  Measured support: all four ladder roots close over the same 420,532-
  position region; R's region-budget gate is the natural second-level guard
  (it cleanly turned UNKNOWN on the 35 oracle samples whose regions exceeded
  500k). Defense-in-depth (replay) confirmed: cyclic regions cannot yield
  cycle-free certificates (0 repeats observed across all certified runs).
- **R4 (budget accounting):** stands; Phase 0 used free-standing budgets per
  candidate; integration numbers (5.83M evals root) leave ample headroom
  under any plausible `child_eval_budget`.
- **R5 (CLI/integration):** stands; `--no-preflight`, the `preflight:` line,
  and the `Search::solve`/`search_depth` hook points need no adjustment
  after these measurements.

## Deviations from the plan

1. **R's horizon knob is the certificate bound.** R has no native search
   horizon (the closure fixpoint is horizon-free); the per-horizon gate is
   implemented as the plan's certificate line-length bound, which is exactly
   the semantics the gate table wants: h = dtm−1 fails replay and downgrades
   to UNKNOWN, h = dtm certifies minimally, h = dtm+2 costs nothing.
2. **L needed the subset-transfer fallback** (planned as "no subset tests at
   all"): strategy board sets overflow the 4,096-entry context-free
   encoding, and the fallback is what the oracle's 85 downgraded L claims
   exercised. Reported as measured.
3. **P's memo at expansion:** the T1b-style memo is probed both at node
   expansion and for child evaluation at expansion time (the PN analogue of
   T1b's node-entry probe). WIN-fact reuse measured 0 hits on the ladder
   (the tree never revisits a (key, depth) with a compatible path often
   enough to matter at these budgets).
4. **Oracle R draw-region budget:** 35/120 win samples' regions exceeded the
   500k-position oracle budget and were left UNKNOWN (never DRAW — plan12
   R8 hygiene preserved). The ladder roots' regions are 420,532 and close
   well within budget.
5. **Two instrument bugs found and fixed during the session** (spike-internal
   only, documented here because they shaped the measurements): a PN backup
   that failed to push proven leaves' numbers into the parent's child entry
   (PN selection spun on stale (1,1) entries), and a replay verifier ply
   off-by-one (initial `plies=1`), which had first made P's correct step3
   proof look uncertified. Both fixed and re-measured before any bake-off
   numbers were recorded.

## Soundness notes (spike only; nothing landed)

- All instrumentation was temporary and is **fully reverted**: the two spike
  examples are deleted, sources archived (`plan13_t1.rs`, `plan13_run.rs`),
  `git status` shows only the new `measurements/plan13/` directory, `git
  diff HEAD` is empty — `src/` and everything else byte-identical to HEAD
  (3339541).
- Post-revert verification: the clean release binary reproduces the T0
  work-chunk boundaries bit-for-bit on steps 1–3 (741,940 / 5,395,869 /
  65,460,435; every intermediate boundary identical), i.e. the deterministic
  work schedule is untouched (`post_revert_verify.log`).
- Since no solver code changed, quick-suite drift / m22 / stress-case gates
  are vacuously satisfied for Session A.
- The ladder-root **draw-proof** side (R's fixpoint remainder) was exercised
  by the oracle only; the cyclic-rook 4-men regression class is outside the
  detector and untouched.

## Problems encountered

- Example binaries build to `target/release/examples/`, not
  `target/release/` — the first T0 script run no-op'd until fixed (cosmetic;
  re-run cleanly).
- `Duration::from_secs_f64(f64::MAX)` panics — the oracle's "no wall cap"
  sentinel needed an explicit finite/size guard.
- P's PN search needed a stall guard (progress key per iteration) in
  addition to the eval/wall/node caps; with the backup fix it never fired in
  the recorded runs.
- CPU contention during the concurrent oracle/L runs inflated some wall
  numbers (noted in the table above); all deterministic counters are
  unaffected, and every G-A-relevant wall figure (R's 0.67 s, P's step3
  0.03 s) was measured uncontended.

## Missing tests

- No solver code changed, so no test gaps were introduced. The ladder
  integration test (`#[ignore = "slow: ..."]`) is a Phase 1 deliverable and
  not triggered in Session A (per plan13 Phase 1).

## Next steps

1. **User checkpoint**: confirm architecture R and the R1–R5 assumptions
   (R1 is binding: new `PvStatus::PreflightProof` variant; R2–R5 veto-able).
   The no-go branches did not trigger, so the plan proceeds to Session B.
2. **Session B (plan13 Phase 1)**: implement R as
   `src/search/preflight.rs` (+ detector + certificate verifier + budget
   accounting per R4), the R5 CLI surface, the replay-verifier unit tests
   with negative cases, the ladder-root slow integration test, and the full
   validation battery (quick-suite drift byte-identical, `test_repetition
   --include-ignored`, m22/stress exact-baseline, G-B oracle re-run on the
   release binary, `make test-full`).
3. Recorded for future plans: (a) **closure draw proofs are a real
   by-product** — R proved all 120 sampled draws at zero extra cost; a
   detector-gated pre-phase that can also return certified Draw (not just
   defer) is worth designing deliberately in Phase 1 (it changes what the
   pre-phase may claim; round 1 scope per plan is root decision only).
   (b) A popcount-4 tier (KRvK+P) stays plausible — the closure budget gate
   is the safety mechanism; measure before building. (c) PN's failure mode
   (path-tree memory) is evidence against any general "PNS instead of
   DF-PN" rework on the big class without a DAG/RCN-style shared frontier.
4. `report13.md` (final, subsuming this Phase 0 report) is written after
   Session B per repo convention.

## Checkpoint record (2026-09-16, user-confirmed) — Session B entry point

The user confirmed the Phase 0 recommendation (architecture R) and resolved
the checkpoint decisions as proposed. Session B must implement against
**this record + `plan13.md` Phase 1**; this section is self-contained.

### Confirmed architecture

R — region-closure fixpoint, exactly as measured in Phase 0: forward BFS
closure (position budget) → AND/OR fixpoint with exact ranks (table-free) →
post-fixpoint rank guard (D3) → rank-decreasing strategy extraction →
replay-verified certificate (the Phase 0 verifier verbatim, incl. negative
cases in unit tests) → outcome + certificate PV returned directly from
`Search::solve` / `search_depth` before the DF-PN loop (A3); on anything
else (detector off, budget abort, guard fail, replay fail) defer and run
the existing search bit-identically.

### Decisions (user-confirmed)

- **D1 = (a) new `PvStatus::PreflightProof`** (binding R1 as decided).
  Doc: PV extracted from a replay-verified pre-phase certificate — a
  winning, cycle-free line with exact ranks (length = exact DTM for the
  region-closure architecture).
- **D2 = (b) WIN + DRAW at root.** The fixpoint remainder is a certified
  board-Draw ⇒ repetition-semantics Draw by the monotonicity lemma (clock-
  independent, no D3 guard needed on the draw side). `Outcome::Draw` is a
  possible pre-phase return; the `preflight:` line carries it. Budget
  abort ⇒ defer, never a claim.
- **D3 = (a) post-fixpoint rank guard for WIN claims:**
  `halfmove + root_rank ≤ 99` else defer. The rule50 boundary (100
  halfmoves ⇒ draw) must be verified against `atomic_movegen` semantics in
  a unit test. Draw claims are exempt (D2).
- **D4 = (a) region budget default 1,000,000**, abort ⇒ defer. Covers the
  entire 3-man legal space (ladder region 420,532; larger components
  observed in the oracle). Transient closure memory ~100–120 MB must be
  documented in `main.rs` as a bounded pre-phase exception to "RAM = TT
  only".

### Informed assumptions (proceed unless re-vetoed; from this report)

A1 no `ProofEvent` emission (R2); A2 pre-phase evals count into
`child_evals` / `child_eval_budget` (R4); A3 hook both `solve` and
`search_depth`, `--no-preflight`, one `preflight: decided|deferred
reason=… evals=…` line in non-`--outcome-only` mode, benchmark JSON
untouched, certificate bound = `max_depth` under `search_depth`; A4 no
refine-cap interaction (pre-phase wins return directly — no refinement
runs); A5 PV = rank-decreasing principal line (attacker: child dist exactly
d−1; defender: max-dist child; deterministic tie-break by region BFS
order), length = exact DTM; A6 module `src/search/preflight.rs`, split
before 10 KB; A7 ladder-root integration test as
`#[ignore = "slow: ..."]`, no fixture change; A8 stop flag checked between
pre-phase phases.

### Session B validation battery (unchanged from plan13 Phase 1)

`cargo fmt` / `clippy` / `make test`; `test_repetition --include-ignored`
(cyclic rook stays Draw); drift protocol `benchmark --suite quick --json
--first-outcome` byte-identical (detector never fires on the suite —
verify by counter); m22_white and the stress case exactly baseline; the
Phase 0 oracle protocol re-run on the release binary (0 contradictions);
`make test-full` before closing. Final deliverable: `report13.md`
(subsuming this Phase 0 report), backlog re-rank, History update.
