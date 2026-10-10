# plan17 — Item #19 (second arm): clock-budget solved-entry reuse re-score under the pinned gate

Initiative: `research` (re-opened 2026-10-09). Executes reopened backlog
item **#19** (re-score the unjudged closures), the *clock-budget
solved-entry reuse* closure (`dfpn` #2, dfpn plans 10/11) — the highest-value
remaining unjudged closure per `report16.md`'s closure block. Self-contained.
Independent of the other remaining #19 arms (`lean` plan10 history/killer
arms, `lean` plan7 eviction V2) and of #20/#21/#22; each gets its own plan.

Unlike plan13's ε arms (single option change, no `src/` changes), this lever
family is **behavior-changing search semantics**: the mechanisms were
implemented, measured, and reverted inside `dfpn` (plans 10/11), so the
re-score runs them through a fresh temporary env-gated hook
(plan16's pattern), with dfpn's own soundness contract carried over as
pre-registered hard gates — including the plan10-style adopted-claim
re-verification sub-study.

## Motivation

`reexamination.md` §1 lists the closure "clock-budget solved-entry reuse,
−37.7% on stress but m22 collapse (`dfpn` #2, plans 10/11)" among the gates
that were single-draw comparisons below the noise floor — *unjudged, not
won* (and, symmetrically, not lost). Two readings are on the table:

1. **plan10's full lever** (cross-clock shadow index, solved adoption at
   both probe sites): stress first-outcome **155,394,450 child evals
   (−37.7%)** vs the 249,480,478 baseline, but the m22_white control
   collapsed (120 s timeout, 714.6 M evals) and 15/59 quick cases regressed.
   plan11's seven-arm matrix could not separate the win from the
   destabilization (the decisive diagnostic: 84.8% of adoption mass is
   AND-parent Win facts — simultaneously the stress-win driver and the m22
   killer); arm C (bounded verification) skipped on a negative prior.
2. **plan11 arm A** (frame-entry adoption with TT store-back — the sound,
   drift-free decoupling candidate): stress FO **227,597,289 (−8.8%)**,
   m22 **18,139,173 (1.28×)**, quick suite byte-identical — closed as
   *below the 10% gate*. A ratio of 0.912 sits **inside the pinned quiet
   noise band [0.760, 1.316]**: arm A's negative reading is exactly the
   class #19 exists to re-judge.

The re-examination's regression-to-the-mean hypothesis (sixty plans tuned
against the salt-0 draw) makes a per-case salt distribution over the frozen
plan12 corpus the first grade at which either reading becomes evidence.
This plan is also the first re-score of a lever whose mechanism itself
re-routes search work (not just a threshold dial), so the gate's
soundness invariants are load-bearing here in a way they were definitional
in plan13.

## Objective

1. Fresh paired runs of both candidate arms and the shipped baseline over
   the frozen plan12 corpus (22 cases × 6 salts × 3 arms = 396 runs).
2. Per-case verdicts per pinned `gate_methodology.md` v1.0 applied
   verbatim.
3. The plan10 soundness contract re-established on the hook build and
   verified at gate grade: adopted-claim re-verification sub-study + the
   report7 cyclic-rook invariant under both arms.
4. The observed paired-ratio dispersion recorded as the comparison rule's
   third real-arm calibration data point.
5. **No product change remains** — the hook is added, measured, and
   reverted; `git diff --exit-code` clean before the report.

## Pre-registered scope decisions

- **D1 — corpus, caps, salt set (inherited verbatim, frozen).** The 22
  plan12 fixtures and per-case caps (stress 2.5 B, m20_white 2 B, others
  1 B; `--timeout 600` secondary); salts {0, 1, 2, 3, 4, 5}; salt 0 always
  runs. Extensions pre-registered only. Corpus and caps as pinned in
  [`measurements/plan12/driver.py`](measurements/plan12/driver.py).
- **D2 — arms (fixed) and the hook.** Baseline: shipped behavior, env
  unset. Candidates:
  - **`plan10`** — the full lever (dfpn plan10 §Design): shadow index
    `HashMap<u64, (Move, Outcome, u32)>` keyed by the board-only
    repetition key, payload (best_move, outcome, proven depth); capacity
    `1 << 18`, inserts dropped when full; persists across `begin_run` like
    the TT; never serialized. Store at the frame-exit TT store in
    `core.rs::dfpn`, only for `store_outcome == Some(Win | Loss)` (draws
    never stored). Probe at **both** sites: dfpn node entry (site 1, after
    the plan9 repetition-cache probe, before `sort_moves`) and
    `evaluate_child` (site 2, when the exact-key `resolved` is `None`).
    Adoption rule on a hit: `rule50 + d ≤ 100`, `d ≤ max_depth`, the
    one-ply `best_move_repeats_path` guard on `best_move`; adopted facts
    are classified as exact-key resolved hits would be (including the
    proof-event emission and the one `child_evals` increment).
  - **`arma`** — plan11 mechanism A: same index, same store site, same
    adoption rule, probe at **site 1 only**; on a hit the frame emits
    `NodeProven(pos, outcome, d)`, stores the solved result into the TT at
    the current full key (`remaining_depth = u32::MAX`, `depth = d`,
    `work = 0`), and returns the outcome — the parent then learns the fact
    through the pre-existing exact-key path (dfpn plan11 §Mechanism A).
  - Selection via env `ATOMIC_CLOCKREUSE=plan10|arma`; unset ⇒ shipped
    behavior, bit-identical (the plan16 hook pattern). Command line per
    run: `atomic_solver --fen <FEN> --tt-size 128 --first-outcome
    --outcome-only --salt S --budget C --timeout 600` (`evals:` on
    stderr); candidate arms additionally set the env var. No other flag
    changes.
- **D3 — soundness and fidelity invariants (hard gates, any violation
  halts).**
  - (a) **Pristine pre-hook identity:** the unhooked build at HEAD
    reproduces the plan12 salt-0 values exactly (stress 249,480,478; m22
    14,156,269; m23 9,673,403; dec13 3,822,602; dec10 4,262,128; m20
    censored) — run before the hook is applied.
  - (b) **Baseline-arm identity:** every fresh baseline cell reproduces
    plan11/plan12's recorded per-salt value exactly, all 132 cells — the
    null is degenerate, any mismatch is an identity failure, not noise.
  - (c) **Hook fidelity anchors (checked before any rollout; a mismatch
    halts for hook debugging, not a verdict):** at salt 0 the hook build
    must reproduce dfpn report11's spike numbers exactly — `arma`: stress
    FO 227,597,289 and m22 18,139,173; `plan10`: stress FO 155,394,450.
    The archived m22 `plan10` reading (120 s timeout, 714.6 M evals) used
    different caps, so m22/`plan10` is recorded as informational
    (censoring pattern per salt), not an anchor. Additionally `arma`'s
    quick-suite byte-identity anchor: `benchmark --suite quick --json
    --first-outcome` at default settings (salt 0) must show all 59
    per-case `child_evals` equal to the unhooked run (dfpn report11's
    measured drift-free property).
  - (d) **Outcome invariants:** zero decisive-outcome conflicts within any
    arm across salts, and every uncensored run of every arm matches its
    fixture `expected` outcome. For the candidate arms this is a
    substantive gate on the shift lemma (plan10 §Soundness contract 2:
    Win/Loss proof trees are repetition-robust, leaves clock-independent,
    `rule50 + d ≤ 100` bounds every interior node's clock) — a conflict is
    a soundness defect, halt and diagnose, never noise.
  - (e) **Adopted-claim re-verification sub-study** (plan10's 444/444
    pattern, scaled down): per candidate arm, sample ≥ 100 adopted claims
    (deterministically, spread over cases/salts; the hook dumps a bounded
    sample of `(uci path from root, proven depth d, outcome)` at the
    adoption site). A temporary verifier replays each path to the node
    (via the solver's own replay helper / `examples/replay`), then runs
    `solve_depth_limited` at `max_depth = d` **with the hook off** and
    requires the identical decisive outcome. Zero disagreements required;
    any disagreement is a soundness defect → HALT.
  - (f) **Cyclic-rook invariant per arm:** `8/8/8/8/2k5/8/8/4KR2 w - - 0 1`
    must never claim `Win` under either arm. Run with `--no-preflight`
    (the position is 3 men, so the pre-phase closure claims Draw before
    the search runs and the spot-check would be vacuous). Post-revert
    sanity: `cargo test --release --test test_repetition --
    --include-ignored` green (product unchanged; cheap).
  - (g) **Revert:** hook + temporary helpers deleted; `git diff
    --exit-code` clean; release build re-verified; (a)/(c) salt-0 anchors
    re-run exact after the revert.
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
  recomputed. One extension per arm; a second defer stands.
- **D6 — hand-off rule (pre-registered).** An **adopt** arm becomes a
  sized hand-off note to `dfpn` (the closure's owner): the per-case paired
  tables, the hook specification as the implementation seed, and the
  proposal to reopen `dfpn` under trigger (a) — the re-examination's
  noise-floor finding is a *measured* search-semantics diagnostic, and
  dfpn plan11's post-go implementation tasks (§Tasks: `cross_clock.rs`,
  tests, drift/soundness battery) are the sized follow-up. **Reject**
  arms are recorded with their tables and the #19 clock-budget closure is
  upgraded from "unjudged" to gate grade; `structural_floor.md`'s
  relevant claim gains the gate-grade pointer.
- **D7 — dec10 caveat and calibration (pinned rule verbatim).** A
  non-hard-class regression that is a case's *sole* negative and sits at
  the quiet-band edge is recorded as a methodology-calibration *question*,
  never an applied override (plan16 D6 precedent). Paired-ratio dispersion
  recorded as the rule's third real-arm calibration data point; this
  family is expected strongly bimodal (plan16: 93%/90% of pairs outside
  [0.9, 1.1] — the cross-clock lever re-routes whole descents, so its
  dispersion should be at least as heavy). No methodology revision is
  proposed from within this plan; any revision needs stated data per the
  standing rule.

## Method

- **Phase 0 — tooling.** `measurements/plan17/driver.py` + `parse.py` +
  `verdicts.py` adapted from plan13/plan16 (D1–D2 constants, arm dimension
  with the env-var hook, resumable, ≤ 3 concurrent processes, transcripts
  to a non-committed RAW dir). The hook is implemented in a working tree
  first and validated against D3(a)–(c) before the rollout.
- **Phase 1 — rollout.** 396 runs; baseline arm first and identity-checked
  (D3(a)/(b)) before the candidate arms; parse to `state/runs.json` +
  `state/summary.json`; run the D3(d) checks.
- **Phase 2 — verification + verdicts.** Adopted-claim sampling dump
  collected during the rollout (D3(e)); the verifier runs after the
  rollout on the dump; then `verdicts.py` → `state/verdicts.json` (D4
  tables per case per arm + paired-ratio dispersion summary).
- **Phase 3 — report.** D6 hand-off if any; revert and D3(g); write
  `report17.md` (final task); refresh the `docs/plans/README.md` research
  row only if an adopt hand-off or a methodology question triggers it.

## Gates

This plan produces verdicts, not a GO/NO-GO:

- **adopt** → hand-off to `dfpn`; the #19 clock-budget closure resolves
  positively and the −37.7%/−8.8% readings are superseded at gate grade.
- **reject / defer** → recorded with tables; the closure is upgraded to
  gate grade either way. If `plan10` rejects on m22 censoring while
  `arma` wins, the record states the trade-off explicitly (plan11's
  finding at distribution grade).
- **HALT** → any D3 invariant violated: soundness or fidelity
  investigation, not a measurement. A fidelity-anchor mismatch (c) is
  debugged before any candidate number is read.

## Out of scope

- plan11's quarantine arms B1–B3 and contingency arm C: their single-draw
  readings were catastrophic (timeouts, +110%, m22 fold-kill), not
  band-edge readings; a re-score is only indicated if this plan's
  `plan10` arm *overturns* its own catastrophic m22 reading — noted in the
  report's next steps, never silently dropped.
- The remaining #19 closures (`lean` plan10 history/killer arms, `lean`
  plan7 eviction V2) — later plans, each one goal.
- #18 (closed measured-out), #20 salted restarts, #21 in-context child
  results, #22 fringe.
- Productization of the lever (dfpn plan11's `cross_clock.rs` task list is
  the owner's, post-adopt); AGENTS.md updates; benchmark/optimizer
  interface; proof-tree layer; methodology revisions.

## Budget envelope

plan13/plan16 measured 396 runs ≈ 2.2–2.5 h CPU-serial (≤ ~1 h wall at
3-way concurrency). Additions: hook build + D3(a)–(c) checks ≈ 30 min; the
`plan10` arm may add censored 1-B m22 runs (≤ 6 × ~2.5 min, cap-absorbed);
the verification sub-study ≈ ≤ 1 h (bounded by the sample size and
`max_depth = d` searches, which are cheap by construction — `d` is a
*proven* depth). Total ≈ 4–5 h CPU; no expected wall explosion.

## Final task

Write `report17.md` in this directory: deliverables, per-arm verdict
tables with basin counts and censoring patterns, adopted-claim
verification results, paired-ratio dispersion summary, problems
encountered, missing tests, next steps (kickoff prompt: remaining #19 arms
— history/killer or eviction V2 — or the `dfpn` hand-off execution if an
arm adopts).
