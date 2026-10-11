# plan19 — Item #19 (fourth and final arm): TT-eviction V2 re-score under the pinned gate

Initiative: `research` (re-opened 2026-10-09). Executes reopened backlog
item **#19** (re-score the unjudged closures), the *TT-eviction arm V2*
closure — research plan7's Phase-1 replacement-priority arm
([`report7.md`](report7.md),
[`measurements/plan7/summary.md`](measurements/plan7/summary.md)), the POC
#12 record owned by `lean`. This is the **last remaining #19 arm** (plan17
closed the clock-budget arms, plan18 the history/killer arms); with it,
#19 is complete and the §1 re-examination table has no unjudged rows left.
Self-contained. Independent of #20 (salted restarts), #21 (in-context child
results), #22 (fringe).

Structural template: plan18 (the constants-arm re-score), with the hook
changed to the **plan7 eviction variant** and the following simplifications:

- **One candidate arm, not two.** plan7 Phase 1 ran V1 and V2; V1 is
  excluded outright (D2 — its readings are catastrophic, not band-edge).
  Rollout shape is therefore **264 runs** (22 cases × 6 salts × {base, v2}),
  not 396.
- **The adopted-claim sub-study is dropped as inapplicable** (D3(c4) — same
  rationale as plan18 D3(e): the eviction policy computes every stored value
  exactly; it only chooses which entries survive, so the soundness surface
  is the outcome invariants).
- **Fidelity anchors exist as recorded scalars, not archived stdout.**
  plan7's Phase-1 stress/m22 raw runs were not committed (only
  `quick_v2.json` and `summary.md` survive), so the anchors are the
  recorded numbers — with an explicit erratum clause for a mismatch (D3(c3)).

## Motivation

`reexamination.md` §1 lists "TT eviction arm V2 (`report7.md`)" among the
closures that were single-draw comparisons below the noise floor —
*unjudged, not won*. The plan7 record (2026-09-20, three weeks before the
salt rollout existed) closed #12 on exactly that evidence class:

1. **The V2 readings sit at or inside the pinned noise band.** stress
   344,866,966 evals = **1.382×** baseline (quiet band top: 1.316 — barely
   outside, one draw); m22 17,507,076 = **1.237×** — **inside the band
   [0.760, 1.316], formally unjudged at the gate's own noise floor**. The
   "the incumbent `(live, solved, work, generation)` priority is a measured
   local optimum" claim rests on these two draws.
2. **The eviction surface is the one closure where the noise channel is the
   lever's own substrate.** The salt works by remapping which keys share a
   bucket — "and therefore the eviction pattern — moves"
   (`reexamination.md` §1). plan7's Phase-0 occupancy data shows why this
   matters: at the 128 MB default the arm only fires where buckets fill
   (stress 95.4% full, m22 4.6%, dec13 0.25%, dec10 0.09%), and the salt
   changes the collision density that decides exactly that. A salt-0 draw
   of V2-vs-baseline measures one collision pattern, not the surface.
3. **The mechanism is a victim-coin-flip upgrade, not a values change.**
   The shipped score's `generation` term is inert within a run
   (`new_generation()` is never called by the search; all live entries share
   the current generation — plan7 context #4, re-confirmed in the code), so
   among two live unsolved victims with equal `work` the shipped policy
   falls through to slot order (`>=` ⇒ bucket[0] evicted). V2 replaces that
   slot-order coin flip with a `remaining_depth` tiebreak. Every stored
   value is computed exactly either way — outcome-invariant by construction,
   like plan18's reordering family.

The re-score question: do the stress/m22 regressions replicate as
distributions over salts, does V2 win anywhere on the corpus, and does the
local-optimum claim survive at gate grade?

## Objective

1. Fresh paired runs of the V2 arm and the shipped baseline over the frozen
   plan12 corpus (22 cases × 6 salts × 2 arms = **264 runs**).
2. Per-case verdicts per pinned `gate_methodology.md` v1.0 applied
   verbatim.
3. The plan7 Phase-1 archive re-anchored at current HEAD: bit-exact salt-0
   fidelity for the V2 arm (stress, m22, dec13, dec10, quick suite) — with
   the report11-erratum lesson (plan17/plan18 pattern) applied to the
   recorded-scalar anchors.
4. The observed paired-ratio dispersion recorded as the comparison rule's
   **fifth** real-arm calibration data point.
5. **No product change remains** — the hook is added, measured, and
   reverted; `git diff --exit-code` clean before the report.
6. With this arm executed, **#19 is complete**: the report states so and
   the `initiative.md` #19 row is refreshed accordingly.

## Pre-registered scope decisions

- **D1 — corpus, caps, salt set (inherited verbatim, frozen).** The 22
  plan12 fixtures and per-case caps (stress 2.5 B, m20_white 2 B, others
  1 B; `--timeout 600` secondary); salts {0, 1, 2, 3, 4, 5}; salt 0 always
  runs. Extensions pre-registered only. Corpus and caps as pinned in
  [`measurements/plan12/driver.py`](measurements/plan12/driver.py).
- **D2 — arms (fixed) and the hook.** Two arms, the 264-run shape (baseline
  arm first and identity-checked before the candidate):
  - **`base`** — shipped behavior, env unset.
  - **`v2`** — steeper work priority, per the plan7 record verbatim
    ("unsolved-vs-unsolved victim scoring gains a `remaining_depth`
    tiebreak before `generation` — deep high-work subtrees protected over
    shallow churn"): in
    `src/search/tt/table.rs::insert_new`, the victim score among live
    unsolved slots becomes the lexicographic
    `(live, solved, work, remaining_depth, generation)` with **larger
    `remaining_depth` kept** (primary direction: `remaining_depth` at
    unsolved stores is the current `max_depth` bound and `u32::MAX` at
    solved stores — core.rs store sites — so a larger bound = a deeper
    search horizon protected, matching the recorded wording; the solved bit
    already outranks unsolved, so the tiebreak never crosses the
    solved/unsolved line). Env gate `ATOMIC_TT_REPL=v2` (plan7's original
    env name), unset ⇒ shipped score ⇒ bit-identical; implementation: one
    `bool` field defaulted at TT construction from the env, consumed only
    in `insert_new`'s score comparison. No layout changes — two entries per
    bucket stays, per RAM = TT only.
  - **Anchor-arbitrated direction clause.** Both tiebreak directions fit
    the one-line plan7 record; the archived stress scalar is definitive:
    if the primary direction misses the D3(c) anchors, the **opposite**
    direction (smaller `remaining_depth` kept) is the pre-registered first
    re-implementation step, and the direction that reproduces is recorded.
    A miss on both is debugged, then classified per D3(c3).
  - **V1 exclusion (recorded, not silent).** plan7's V1 readings were a
    600 s timeout at 3.95 B evals on stress (13×+ the baseline, structurally
    fatal — dropped unsolved stores are the search's working set) and
    +3400% on m22. Nothing there is band-edge; a distribution re-score
    cannot change the verdict. Re-opening V1 would need a new mechanism
    hypothesis (a layout that protects solved slots without dropping
    unsolved stores), which the two-slot layout forbids — the binding
    constraint plan7 already recorded.
  - Command line per run:
    `atomic_solver --fen <FEN> --tt-size 128 --first-outcome --outcome-only
    --salt S --budget C --timeout 600` (`evals:` on stderr); the candidate
    arm additionally sets `ATOMIC_TT_REPL=v2`. No other flag changes.
- **D3 — soundness and fidelity invariants (hard gates; a violation halts).
  Liveness-first ordering per the report11-erratum lesson: every identity
  claim is recorded only after hook liveness is demonstrated (D3(c0)).**
  - (a) **Pristine pre-hook identity:** the unhooked build at HEAD
    reproduces the plan12 salt-0 values exactly (stress 249,480,478; m22
    14,156,269; m23 9,673,403; dec13 3,822,602; dec10 4,262,128; m20
    censored) — run before the hook is applied. **Partially pre-verified in
    this plan session (2026-10-11, read-only):** m22 14,156,269 and stress
    249,480,478 reproduce on the existing release build with a clean tree
    (`git diff --exit-code`); the execution session re-runs the full (a)
    set on its own build.
  - (b) **Baseline-arm identity:** every fresh baseline cell reproduces
    plan12's recorded per-salt value exactly, all 132 cells — the null is
    degenerate, any mismatch is an identity failure, not noise.
  - (c) **Hook fidelity anchors (checked before any rollout; a mismatch
    halts for hook debugging, not a verdict):**
    - (c0) **Liveness first.** An identity or fidelity claim is void unless
      the hook is proven live on the *same binary* that produces it.
      Liveness proof: `ATOMIC_TT_REPL=v2` at salt 0 reproduces the archived
      stress anchor (344,866,966 — D3(c1)); a run matching the *baseline*
      count under a set env var is the stale-binary/false-identity
      signature → HALT. Standing procedure (plan18 operational finding):
      `cargo build --release --examples` explicitly after every lib
      change — the command does not relink the `atomic_solver` bin by
      itself.
    - (c1) **Archived plan7 salt-0 anchors (bit-exact).** V2 at salt 0:
      stress 344,866,966 **and** first win 97 plies (`pv_status:
      first-outcome` — vs the baseline's 477; the win length is part of the
      anchor); m22 17,507,076; dec13 3,822,602 and dec10 4,262,128
      **bit-identical to their baselines** (the arm provably never fires at
      the salt-0 occupancies plan7 measured — 0.25% / 0.09% full — so any
      delta is a hook defect, not noise); quick suite per-case
      `child_evals`/status equal to the archived
      [`quick_v2.json`](measurements/plan7/quick_v2.json) (the arm never
      fires there either).
    - (c2) **Quick-suite byte-identity sub-anchor (env unset).** The hook
      build at default settings, `benchmark --suite quick --json
      --first-outcome --timeout 3`, must show all 59 per-case
      `child_evals` and statuses equal to the unhooked step-0 run
      (archived total 38,974,090). Recorded only after (c0) passes; a
      failure under proven liveness and env unset is a **hook defect** →
      HALT and debug, never absorbed.
    - (c3) **Erratum clause (recorded-scalar limitation).** plan7's
      Phase-1 stress/m22 raw outputs were not archived; the anchors are
      the recorded scalars in `summary.md`/`report7.md`. If an anchor
      fails although (c0)-style liveness is proven by the *other* anchors
      (e.g. stress reproduces, m22 does not), the deviation is debugged to
      a definitive conclusion and recorded as an **erratum candidate in
      the research plan7 record** — never silently absorbed, never assumed
      a hook defect without diagnosis. Both-directions-miss under a
      liveness-verified build ⇒ HALT (the mechanism spec or the record is
      wrong; no verdicts are read).
    - (c4) **No adopted-claim sub-study (recorded inapplicability).** The
      eviction policy asserts nothing: every entry it keeps or drops
      carries values the search computed exactly. The soundness surface is
      (d); the fidelity budget is spent on (c1)'s five anchors instead.
  - (d) **Outcome invariants (substantive).** Zero decisive-outcome
    conflicts within any arm across salts, and every uncensored run of
    every arm matches its fixture `expected` outcome. The policy only
    chooses victims — solved values are computed exactly as before — so
    any outcome flip is a soundness defect (halt and diagnose, never
    noise). This is the family's whole soundness surface.
  - (e) **Cyclic-rook invariant per arm:**
    `8/8/8/8/2k5/8/8/4KR2 w - - 0 1` must never claim `Win` under either
    arm, run with `--no-preflight` (3 men — the pre-phase closure would
    claim Draw first and make the spot-check vacuous). Post-revert
    sanity: `cargo test --release --test test_repetition --
    --include-ignored` green.
  - (f) **Revert:** hook deleted; `git diff --exit-code` clean; release
    build re-verified; (a)/(c2) salt-0 anchors re-run exact after the
    revert.
- **D4 — verdict machinery (pinned v1.0, applied verbatim).** Per case:
  per-salt paired ratios `v2_s / base_s` over pairs where both sides are
  uncensored; censoring pattern per salt; baseline basin count from
  plan12. Verdicts: censored-win / censored-loss / win (median ≤ 0.9 ∧
  ≥ 75% of ratios < 1) / regression (median ≥ 1.1 ∧ ≥ 75% > 1) / unclear.
  Lever verdict: **adopt** = ≥ 1 censored-win or win on a hard-class case
  (stress, m20_white, m22_white, m23_white) ∧ no regression /
  censored-loss anywhere; **reject** = any regression or censored-loss;
  otherwise **defer**. Low-diversity baselines (basin count < 3) cannot
  solely support an adopt. Under-budget rule: > 3 of 6 censored salts on a
  case for either arm ⇒ case unresolved, contributes nothing.
- **D5 — defer extension rule (pre-registered).** A defer is re-judged only
  after extending the salt set **by {6, 7} on the affected cases, both
  arms**, fresh runs at the added salts; the pooled verdict is then
  recomputed. One extension; a second defer stands. **No third arm exists**
  (plan7 had only V1/V2; V1 is excluded per D2), so — a deviation from
  plan17/plan18's arm-extension options — a defer on *mechanism grounds*
  cannot be resolved by adding an arm and stands as recorded.
- **D6 — hand-off rule (pre-registered).** The closure's owner is `lean`
  (the eviction surface was POC #12's recorded owner; the TT eviction
  record lives in `research` plan7). An **adopt** becomes a sized proposal
  to `lean`: the per-case paired tables, the hook specification as the
  implementation seed (hardcoded policy in `insert_new`, env hook
  deleted), and the landing contract (`cargo fmt`/`clippy`, `make test`,
  `make test-full` — TT-behavior change; `tests/` TT unit fixtures in
  `src/search/tt/tests.rs` exercise `insert_new` directly and must be
  updated to the new policy). A **reject** is recorded with its tables and
  the #19 eviction closure is upgraded from "unjudged" to gate grade; the
  `reexamination.md` §1 "TT eviction arm V2" bullet gains the gate-grade
  pointer (as plan17/plan18 did for their bullets). Either way the plan7
  closure's standing claim is re-graded explicitly: reject ⇒ "the incumbent
  priority is a measured local optimum" upgraded from two draws to a
  distribution-grade statement; adopt ⇒ the claim is overturned at gate
  grade.
- **D7 — dispersion prediction (pinned rule verbatim, falsifiable).**
  Paired-ratio dispersion recorded as the rule's fifth calibration data
  point. Unlike the constants family (plan18) or the clock family (plan17),
  V2 fires **only** on full buckets whose two slots are live and unsolved —
  plan7 Phase-0 occupancy (salt 0): stress 95.4%, m22 4.6%, dec13 0.25%,
  dec10 0.09% full — and the salt changes collision density, so the arm can
  fire under salts where its salt-0 footprint is zero. **Pre-registered
  prediction:** sparse-effect shape (plan17-like, not plan18-like):
  frac-outside [0.9, 1.1] ≈ **20–50%**, exact-null mass (ratio exactly 1.0)
  ≥ **40%**, tails concentrated in the hard class (stress / m20 / m22 /
  m23) plus dec-class cases where a salt raises collision density. Also
  pre-registered: the plan7 salt-0 readings (stress 1.382, m22 1.237) are
  the quantities being replaced by distributions — m22's 1.237 is inside
  the quiet band, so if the rollout shows m22 unclear rather than
  regression, that is the gate correcting a noise-floor misread, not a
  contradiction. A non-hard-class regression that is a case's sole
  negative at the quiet-band edge is recorded as a calibration *question*,
  never an applied override. No methodology revision is proposed from
  within this plan.

## Method

- **Phase 0 — tooling.** `measurements/plan19/driver.py` + `parse.py` +
  `verdicts.py` adapted from plan18 (D1–D2 constants, arm dimension via the
  env hook, resumable, ≤ 3 concurrent processes, transcripts to a
  non-committed RAW dir). The hook is implemented in a working tree first
  and validated against D3(c0)–(c1) **in that order** (liveness before
  identity) before the rollout; (c2) runs on the hook build before any
  rollout run.
- **Phase 1 — rollout.** 264 runs; baseline arm first and identity-checked
  (D3(a)/(b)) before the candidate arm; parse to `state/runs.json` +
  `state/summary.json`; run the D3(d) checks.
- **Phase 2 — verdicts.** `verdicts.py` → `state/verdicts.json` (D4 tables
  per case + paired-ratio dispersion summary).
- **Phase 3 — report.** D6 hand-off if any; revert and D3(f); write
  `report19.md` (final task); annotate the `reexamination.md` §1
  eviction-V2 bullet on a reject; refresh the #19 row in `initiative.md`
  (and the `docs/plans/README.md` research row) — with this plan the
  #19 item **closes**; add the `initiative.md` history entry.

## Gates

This plan produces verdicts, not a GO/NO-GO:

- **adopt** → sized proposal to `lean` (D6); plan7's local-optimum claim is
  overturned at gate grade.
- **reject / defer** → recorded with tables; the closure is upgraded to
  gate grade either way. On a reject, the report states that plan7's no-go
  stands at distribution grade (or, if m22's in-band reading dissolves
  into `unclear` while stress still rejects, that the stress regression
  alone carries the rejection — the honest distribution-grade form).
- **HALT** → any D3 invariant violated: soundness or fidelity
  investigation, not a measurement. Fidelity mismatches are debugged before
  any candidate number is read (plan17 §"Problems" order of operations).

## Out of scope

- **V1** (excluded per D2, with rationale); any layout change (bucket size,
  entry fields, snapshot format — the RAM = TT only contract and the
  snapshot/import contract are fixed); TT-capacity re-litigation (the 32 MB
  pressure point is *not* re-run — plan7 recorded it; capacity is `lean` #16
  territory and the post-plan9 solver's TT sensitivity is already on
  record).
- #20 salted restarts, #21 in-context child results, #22 fringe (the
  remaining open reopened-backlog items after #19 closes).
- Productization (the `lean` landing contract is the owner's, post-adopt);
  proof-tree layer; `ProofEvent`; optimizer interface; methodology
  revisions.

## Budget envelope

264 runs at ≤ 3 concurrent processes: the baseline arm re-runs plan12's
measured ~1.2 h grid; the v2 arm adds ≈ 1.38× on stress-class runs and is
byte-identical (near-free) on the low-occupancy tail — plan7 measured v2
stress 344.9 M ≈ 1.38× baseline evals, so worst-case run-time inflation is
proportional. Additions: hook build + D3(c0)–(c2) checks ≈ 30 min (the
quick anchors are minutes). Total ≈ 3–4 h CPU; no expected wall explosion;
censoring bounded by the caps as in plan17/plan18.

## Final task

Write `report19.md` in this directory: deliverables, per-case verdict table
with basin counts and censoring patterns, fidelity-anchor results
(including the anchor-arbitrated tiebreak direction), paired-ratio
dispersion summary with the D7 prediction's fate, problems encountered,
missing tests, next steps (kickoff prompt: the first open reopened-backlog
item — #20 salted restarts or #21 in-context child results, per owner
choice — or, on an adopt, the `lean` eviction reopen execution). Refresh
the #19 row in `initiative.md` (item closes), add the history entry, and
update the `docs/plans/README.md` research row if the index summary changes.
