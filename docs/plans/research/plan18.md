# plan18 — Item #19 (third arm): `lean` plan10 history/killer arms re-score under the pinned gate

Initiative: `research` (re-opened 2026-10-09). Executes reopened backlog
item **#19** (re-score the unjudged closures), the *`lean` plan10
history/killer arms* closure (`lean` #10, lean plan10/report10) — the
remaining #19 arm with the largest killed aggregate readings. Self-contained.
Independent of the last remaining #19 arm (`lean` plan7 TT-eviction V2 — its
own plan) and of #20/#21/#22.

Structural template: plan17 (the first behavior-changing re-score), with two
deliberate simplifications: the hook is the *lean plan10 constant-override
pattern* (fields defaulting to the consts — far smaller than plan17's
cross-clock mechanism port), and the plan17 adopted-claim sub-study is
**dropped as inapplicable** (D3(e) below — a constant re-tune adds no
claim-bearing mechanism; every outcome is still computed exactly by the
search, so the soundness surface is the outcome invariants, not claim
verification).

## Motivation

`reexamination.md` §1 lists "the `lean` plan10 history/killer arms (−11 to
−20% quick totals, killed by per-case flips)" among the gates that were
single-draw comparisons below the noise floor — *unjudged, not won*. The
lean plan10 record (2026-10-06, one day before the re-examination's salt
rollout existed; [`report10.md`](../lean/report10.md),
[`arms.csv`](../lean/measurements/plan10/arms.csv)) closed #10 as no-go on
exactly that evidence class:

1. **The strong aggregate winners were killed by per-case flips measured
   at a 3 s wall cap.** hs2-ag1 (−19.57%) and hs05-ag1 (−11.14%) each
   flipped *one* previously-solving quick case into a 3 s timeout
   (m23_white: 9.67 M evals baseline → >12.3 M under the arm) — and the
   gate-2 rule treated that flip as fatal. Under the pinned gate's budgets
   (1 B evals, 600 s secondary), a 1.27× blowup on a 9.67 M case is nowhere
   near censoring: **the specific per-case flips that killed these arms are
   wall-clock artifacts of the quick suite's `--timeout 3`, not eval-budget
   facts.** This is precisely the class the salt distribution re-judges.
2. **The tail regressions are the real risk and were also single draws.**
   dec05 at 5.19× (both finalists), dec44 5.19× (the hs05 OFAT arm), dec14
   1.59× (hs05-ag100): each a salt-0 reading in a regime where one neutral
   draw moves a hard case by ≥ 3× (`reexamination.md` §1). Whether these
   replicate across salts is unknown.
3. **The chaos evidence itself is salt-blind.** The aging series'
   non-monotonicity (1 → −19.2%, 5 → −1.9%, 10 → −18.6%, 500 → −24.0%) was
   measured on one draw per arm. A distribution-grade re-score either
   confirms "chaotically coupled, no robustness" or overturns it.

Unlike plan17's lever, this family changes *only move ordering* (constants
feed `sort_moves` scores; no values, no claims, no TT semantics change), so
the plan17 machinery applies with a lighter soundness contract.

## Objective

1. Fresh paired runs of both candidate arms and the shipped baseline over
   the frozen plan12 corpus (22 cases × 6 salts × 3 arms = 396 runs).
2. Per-case verdicts per pinned `gate_methodology.md` v1.0 applied
   verbatim.
3. The lean plan10 archive re-anchored at current HEAD: per-case
   quick-suite fidelity for both arms plus the counter-bearing diagnostic —
   with the report11-erratum lesson (plan17 §"D3(c) deviation") applied to
   the quick-suite byte-identity sub-anchor.
4. The observed paired-ratio dispersion recorded as the comparison rule's
   fourth real-arm calibration data point.
5. **No product change remains** — the hook is added, measured, and
   reverted; `git diff --exit-code` clean before the report.

## Pre-registered scope decisions

- **D1 — corpus, caps, salt set (inherited verbatim, frozen).** The 22
  plan12 fixtures and per-case caps (stress 2.5 B, m20_white 2 B, others
  1 B; `--timeout 600` secondary); salts {0, 1, 2, 3, 4, 5}; salt 0 always
  runs. Extensions pre-registered only. Corpus and caps as pinned in
  [`measurements/plan12/driver.py`](measurements/plan12/driver.py).
- **D2 — arms (fixed) and the hook.** Three arms, plan17's 396-run shape
  (baseline arm first and identity-checked before the candidates):
  - **`base`** — shipped behavior, env unset.
  - **`hs2ag1`** — `LEAN10_HISTORY_BONUS=200 LEAN10_HISTORY_MAX=20000
    LEAN10_HISTORY_AGE_INTERVAL=1` (lean plan10 cross-product finalist,
    best total −19.57%; archived per-case anchor
    [`quick_arm_hs2-ag1.json`](../lean/measurements/plan10/results/quick_arm_hs2-ag1.json)).
  - **`hs05ag1`** — `LEAN10_HISTORY_BONUS=50 LEAN10_HISTORY_MAX=5000
    LEAN10_HISTORY_AGE_INTERVAL=1` (−11.14%; archived anchor
    [`quick_arm_hs05-ag1.json`](../lean/measurements/plan10/results/quick_arm_hs05-ag1.json)).
  Arm-selection rationale (recorded because lean plan10 measured 23 arms
  and the re-score cannot carry them all): `hs2ag1`/`hs05ag1` are the two
  arms whose aggregate wins exceed the gate's own case-win threshold
  (median ratio ≤ 0.9) **and** whose kills were per-case flips — the exact
  unjudged class of #19; they differ in exactly one factor (history scale
  at fixed aggressive aging), a clean one-factor contrast at gate grade.
  Excluded: `hs2ag5` (near-twin of `hs2ag1`, −19.68% vs −19.57%, differs
  only in AGE 1→5; it becomes the pre-registered D5 extension arm if a
  defer fires); `hs05ag100` (the only flip-free >3% arm, but a −3.3%
  aggregate cannot produce a case-level win — median ≤ 0.9 per case — so
  it could never adopt, at best defer; re-indicated only if a candidate
  adopt makes the scale×aging interaction load-bearing); the killer axis
  (`k200` byte-identical to default — a dead degree of freedom; `k10`
  fails at salt 0; `SCORE_KILLER` therefore gets **no** env override in
  this hook — a recorded deviation from lean plan10's four-var hook that
  cannot affect any anchor, since no anchor uses it).
  - **Hook (lean plan10 pattern, minimal):** `Search::new` reads
    `LEAN10_HISTORY_BONUS` / `LEAN10_HISTORY_MAX` /
    `LEAN10_HISTORY_AGE_INTERVAL` once into `Search` fields defaulting to
    the `history.rs` consts; `update_history_entry`, `age_history`, and the
    cap arithmetic take the fields. Env unset ⇒ the fields equal the
    consts ⇒ every arithmetic path identical ⇒ bit-identical search. The
    temporary `LEAN10_STATS=1` stderr counter (sort_calls /
    reorder-top-1-picks) is re-added for the D3(c2) counter anchor; it is
    side-band (never affects scoring) and only used in Phase 0. Command
    line per run: `atomic_solver --fen <FEN> --tt-size 128
    --first-outcome --outcome-only --salt S --budget C --timeout 600`
    (`evals:` on stderr); candidate arms additionally set the env vars. No
    other flag changes.
- **D3 — soundness and fidelity invariants (hard gates; a violation halts).
  The report11-erratum lesson is applied as an ordering rule: every
  identity claim below is recorded only *after* hook liveness is
  demonstrated (D3(c0)), and every archived anchor is checked per case, at
  the counter level where counters exist.**
  - (a) **Pristine pre-hook identity:** the unhooked build at HEAD
    reproduces the plan12 salt-0 values exactly (stress 249,480,478; m22
    14,156,269; m23 9,673,403; dec13 3,822,602; dec10 4,262,128; m20
    censored) — run before the hook is applied. **Partially pre-verified in
    this plan session (2026-10-11, read-only):** the quick-suite baseline
    reproduces the lean archive bit-exactly (38,974,090 total, 59/59
    per-case `child_evals` and statuses identical) and the m22 anchor md5
    `ea72f7eae2437867c986dbf71268223f` / 14,156,269 evals holds at HEAD —
    the execution session still re-runs the full (a) set on its own build.
  - (b) **Baseline-arm identity:** every fresh baseline cell reproduces
    plan12's recorded per-salt value exactly, all 132 cells — the null is
    degenerate, any mismatch is an identity failure, not noise.
  - (c) **Hook fidelity anchors (checked before any rollout; a mismatch
    halts for hook debugging, not a verdict):**
    - (c0) **Liveness first (the report11-erratum lesson's operative
      form).** An identity or fidelity claim is void unless the hook is
      proven live on the *same binary* that produces it. Liveness proof:
      two archived lean plan10 probe arms reproduced bit-exactly on the
      hook build — `LEAN10_HISTORY_AGE_INTERVAL=1` (archived quick total
      31,493,500; 58 ok / 1 timeout: m23_white) and
      `LEAN10_HISTORY_BONUS=1 LEAN10_HISTORY_MAX=1` (archived 38,373,893;
      58 ok / 1 timeout: dec01). Totals compare over ok cases; a
      timeout-status case is anchored on the status flag only (wall-capped
      evals are not deterministic — same pre-registered limitation as
      (c3)). A total matching baseline under a set env var is the
      stale-binary/false-identity signature (lean report10's trap) → HALT.
      Standing procedure: `cargo build --release --examples` explicitly
      after every lib change.
    - (c1) **Quick-suite byte-identity sub-anchor (env unset).** The hook
      build at default settings, `benchmark --suite quick --json
      --first-outcome --timeout 3`, must show all 59 per-case
      `child_evals` and statuses equal to the unhooked step-0 run
      (archived total 38,974,090). This is the claim dfpn report11 made
      for arm A and plan17 found to be an erratum candidate; here it is
      pre-registered with the lesson applied — (i) recorded only after
      (c0) passes, (ii) a failure under proven liveness and env unset is
      classified a **hook defect** (the search must not depend on env
      parsing when no var is set) → HALT and debug, never absorbed.
    - (c2) **Counter-bearing anchor (this family's adoption-counter
      semantics).** With `LEAN10_STATS=1` (temporary diagnostic, quick
      suite, env unset) the aggregate must reproduce the archived lean
      plan10 diagnostic exactly: **2,477,336 `sort_moves` calls /
      482,956 reorder-top-1 picks**
      ([`diag_quick_summary.json`](../lean/measurements/plan10/state/diag_quick_summary.json)).
      This is the plan17 lesson
      institutionalized — plan17's counter-bearing hook caught a mis-armed
      port immediately (arma reproduced plan10's stress count); here the
      sort-call/reorder counters are the behavioral counters that catch an
      inert, partial, or mis-wired override plumbing.
    - (c3) **Candidate-arm per-case fidelity (salt 0).** `hs2ag1` and
      `hs05ag1` quick-suite runs vs the archived
      `quick_arm_hs2-ag1.json` / `quick_arm_hs05-ag1.json`: every
      **status-ok** case bit-exact (`child_evals` and `outcome`);
      timeout-status cases are anchored on the **status flag only** —
      wall-capped eval counts are not deterministic (pre-registered
      limitation; m23_white must reproduce `timeout: true` under both
      arms). A per-case mismatch on an ok case under a liveness-verified
      build is debugged before any verdict is read.
    - (c4) **Erratum clause (report11 lesson, inverted).** If a
      candidate arm's archived anchor fails although (c0)–(c2) passed,
      the deviation is debugged to a definitive conclusion and recorded
      as an **erratum candidate in the lean plan10 record** (most
      plausible form: a stale example binary in that session) — never
      silently absorbed, never assumed a hook defect without diagnosis.
      The lean plan10 report itself documents one stale-binary incident,
      so its archive is *not* presumed clean.
  - (d) **Outcome invariants (substantive).** Zero decisive-outcome
    conflicts within any arm across salts, and every uncensored run of
    every arm matches its fixture `expected` outcome. A constant re-tune
    reorders children only — solved values are computed exactly as before —
    so any outcome flip is a soundness defect (halt and diagnose, never
    noise). This is the family's whole soundness surface; there is no
    shift-lemma content to gate (contrast plan17 D3(d)).
  - (e) **No adopted-claim sub-study (recorded inapplicability).** plan17's
    D3(e) verified *adopted claims* — facts the lever asserted without
    re-proving. This lever asserts nothing: it reorders `sort_moves` input
    only. The cost is instead spent on (c)'s per-case fidelity, which for a
    reordering lever is the stronger check.
  - (f) **Cyclic-rook invariant per arm:**
    `8/8/8/8/2k5/8/8/4KR2 w - - 0 1` must never claim `Win` under either
    arm, run with `--no-preflight` (3 men — the pre-phase closure would
    claim Draw first and make the spot-check vacuous). Post-revert
    sanity: `cargo test --release --test test_repetition --
    --include-ignored` green.
  - (g) **Revert:** hook + diagnostic deleted; `git diff --exit-code`
    clean; release build re-verified; (a)/(c1) salt-0 anchors re-run exact
    after the revert.
- **D4 — verdict machinery (pinned v1.0, applied verbatim).** Per case:
  per-salt paired ratios `candidate_s / baseline_s` over pairs where both
  sides are uncensored; censoring pattern per salt; baseline basin count
  from plan12. Verdicts: censored-win / censored-loss / win (median ≤ 0.9
  ∧ ≥ 75% of ratios < 1) / regression (median ≥ 1.1 ∧ ≥ 75% > 1) /
  unclear. Lever verdict per arm: **adopt** = ≥ 1 censored-win or win on a
  hard-class case (stress, m20_white, m22_white, m23_white) ∧ no
  regression / censored-loss anywhere; **reject** = any regression or
  censored-loss; otherwise **defer**. Low-diversity baselines (basin count
  < 3) cannot solely support an adopt. Under-budget rule: > 3 of 6
  censored salts on a case for either arm ⇒ case unresolved, contributes
  nothing.
- **D5 — defer extension rule (pre-registered).** A defer is re-judged
  only after extending the salt set **by {6, 7} on the affected cases,
  both arms**, fresh runs at the added salts; the pooled verdict is then
  recomputed. One extension per arm; a second defer stands. If the defer
  is on the *arm choice* rather than salt spread (i.e. `hs2ag1` defers
  with `hs05ag1` decided), the pre-registered third arm is `hs2ag5` (lean
  plan10's other finalist) at the canonical 6 salts.
- **D6 — hand-off rule (pre-registered).** The closure's owner is `lean`
  #10 (closed no-go 2026-10-06; the `lean` backlog is empty). An **adopt**
  arm becomes a sized proposal to reopen `lean` #10: the per-case paired
  tables, the hook specification as the implementation seed (hardcoded
  consts in `src/search/dfpn/history.rs`, env hooks deleted), and lean
  plan10's own landing contract (`cargo fmt`/`clippy`, `make test`,
  **`make test-full` required — move-ordering change**; the Phase-3 list
  of [`lean plan10`](../lean/plan10.md)). A **reject** is
  recorded with its tables and the #19 history/killer closure is upgraded
  from "unjudged" to gate grade; the `reexamination.md` §1 bullet gains
  the gate-grade pointer (as plan17 did for the clock-budget bullet).
  Note the asymmetry with plan17: a reject here does not merely confirm
  lean plan10's no-go — it *upgrades* it from single-draw evidence to
  distribution grade, which is a different (stronger) claim.
- **D7 — dispersion prediction and dec10 caveat (pinned rule verbatim).**
  Paired-ratio dispersion recorded as the rule's fourth calibration data
  point. **Pre-registered prediction (falsifiable, plan17 precedent):**
  unlike plan17's sparse-effect shape (48%/35% frac-outside, large exact
  nulls), a constant change perturbs history values from the first proven
  frame and the aging interval re-times every halving sweep — no
  byte-identical mass is expected; prediction: **frac outside [0.9, 1.1]
  ≥ 90%** (plan16-like), with case-concentrated heavy tails in the dec05 /
  dec44 / dec14 classes (lean plan10's salt-0 kills: dec05 5.19× under
  both arms). Also pre-registered: both arms' salt-0 dec05 readings near
  5.19× would sit far outside the quiet band top 1.316 — if they
  replicate, the arm rejects regardless of its aggregate win; the
  rollout's purpose is to replace exactly these single-draw kills with
  distributions. A non-hard-class regression that is a case's sole
  negative at the quiet-band edge is recorded as a calibration
  *question*, never an applied override. No methodology revision is
  proposed from within this plan.

## Method

- **Phase 0 — tooling.** `measurements/plan18/driver.py` + `parse.py` +
  `verdicts.py` adapted from plan17 (D1–D2 constants, arm dimension via the
  env hook, resumable, ≤ 3 concurrent processes, transcripts to a
  non-committed RAW dir). The hook is implemented in a working tree first
  and validated against D3(c0)–(c3) (in that order) before the rollout.
  The D3(c2) diagnostic is a per-case loop over the quick FEN list (lean
  plan10's diag command shape, `env.json`), aggregated.
- **Phase 1 — rollout.** 396 runs; baseline arm first and identity-checked
  (D3(a)/(b)) before the candidate arms; parse to `state/runs.json` +
  `state/summary.json`; run the D3(d) checks.
- **Phase 2 — verdicts.** `verdicts.py` → `state/verdicts.json` (D4 tables
  per case per arm + paired-ratio dispersion summary).
- **Phase 3 — report.** D6 hand-off if any; revert and D3(g); write
  `report18.md` (final task); annotate the `reexamination.md` §1
  history/killer bullet on a reject; refresh the `docs/plans/README.md`
  research row only if an adopt hand-off or a methodology question
  triggers it; add the `initiative.md` history entry.

## Gates

This plan produces verdicts, not a GO/NO-GO:

- **adopt** → sized proposal to reopen `lean` #10 (D6); the −11%/−20%
  readings are superseded at gate grade.
- **reject / defer** → recorded with tables; the closure is upgraded to
  gate grade either way. If both arms reject on the dec05-class tails, the
  report states that lean plan10's no-go stands at distribution grade with
  the tail regressions confirmed as real, not noise — the strongest
  possible form of the original closure.
- **HALT** → any D3 invariant violated: soundness or fidelity
  investigation, not a measurement. Fidelity mismatches are debugged
  before any candidate number is read (plan17 §"Problems" order of
  operations).

## Out of scope

- The other lean plan10 arms (`hs2ag5`, `hs05ag100`, the OFAT single-axis
  arms, the killer axis) — excluded with rationale in D2; `hs2ag5` is the
  pre-registered D5 third arm only.
- lean plan10's F5 "side-aware killers" retirement — analysis-retired, not
  re-opened (the argument is structural, not a measurement).
- The last remaining #19 arm (`lean` plan7 TT-eviction V2) — its own plan.
- #18 (closed measured-out), #20 salted restarts, #21 in-context child
  results, #22 fringe.
- Productization (the `lean` #10 landing contract is the owner's,
  post-adopt); `ScorerParams`/optimizer interface (lean plan10's scope
  decision stands: history/killer constants are search-layer); proof-tree
  layer; methodology revisions.

## Budget envelope

plan13/plan16/plan17 measured 396 runs ≈ 2.2–2.5 h wall at ≤ 3 concurrent
processes. This family should be at the low end: the candidate arms'
aggregate wins suggest fewer censored runs than plan17's `plan10` arm, but
m20/m22-class censoring under the candidate trajectories is unknown (never
measured — lean plan10 never ran the corpus) and is bounded by the caps as
in plan17. Additions: hook build + D3(c0)–(c3) checks ≈ 30 min (the
(c2)/(c3) quick anchors are minutes: 3 s/case × 59 × ~4 runs). Total
≈ 3–4 h CPU; no expected wall explosion.

## Final task

Write `report18.md` in this directory: deliverables, per-arm verdict
tables with basin counts and censoring patterns, fidelity-anchor results
including the counter anchor, paired-ratio dispersion summary with the D7
prediction's fate, problems encountered, missing tests, next steps
(kickoff prompt: the `lean` plan7 eviction V2 re-score — the last #19 arm
— or the `lean` #10 reopen execution if an arm adopts).
