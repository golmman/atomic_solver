# plan16 — Item #18, Phase 1: gated shaped-variant rollout of `mob-or` and `mobcap8`

Initiative: `research` (re-opened 2026-10-09). Executes the pre-registered
Phase 1 of reopened backlog item **#18** (leaf-initialization family): the
gated rollout of the two shaped candidates plan15 nominated — **`mob-or`**
(OR-parent children `(n, 1)`, `(1, 1)` elsewhere) and **`mobcap8`** (both
sides `min(n, 8)`) — over the frozen plan12 corpus (22 cases × 6 salts)
under `gate_methodology.md` v1.0, verdicts applied verbatim. Self-contained.
Prerequisites are done: #17 gate pinned v1.0, #18 Phase 0 diagnosis
(plan15, verdict GO-plan16). This plan issues **lever verdicts**
(adopt / reject / defer per arm), not a GO/NO-GO.

## Motivation

plan15's P3 split micro-arms attributed the naive-mobility win and the m22
collapse to different halves of the init: the OR-side `(n, 1)` init carries
the stress/dec13 wins (first-sweep cut mass transfers from AND frames to OR
frames — the parent's own bound stops being deceptively 1), while the
AND-side `(1, n)` init destroys the AND-side cut regime; on m22 the
collapsing arm additionally dives into a >90-ply repetition region and
re-derives a 176 K-position set ~1,200× over. Two shaped variants satisfy
the pre-registered GO rule (stress-or-dec13 direction win **and** m22 ≤ 1.5×
uncensored on 3/3 salts):

- **`mob-or`**: stress 0.285×, dec13 0.127×, dec10 1.21×, m22
  1.12× / 0.50× / 0.90× (salts 0/1/2);
- **`mobcap8`**: stress 0.396×, dec13 0.133×, dec10 1.35×, m22
  1.06× / 0.54× / 0.85×.

`mob`, `mob-and`, and `moblog` are retired. All readings are single-salt
(or 3-salt m22) and below the pinned noise floor; they nominate, they do
not judge. The one standing caveat both candidates share: a dec10
regression (1.21× / 1.35×, single-draw, inside/at the edge of the quiet
band [0.760, 1.316]) — D6 below pre-registers its treatment. A blend is not
indicated (the AND-side half is the damage carrier; the surviving space is
OR-side-only or conservatively-capped both-sides), so no blend arm runs.

## Objective

1. Fresh paired runs of both candidate arms and the shipped baseline over
   the frozen plan12 corpus (22 cases × 6 salts × 3 arms = 396 runs).
2. Per-case verdicts per the pinned v1.0 comparison rule; lever verdicts
   (adopt / reject / defer) per arm.
3. A decision on the dec10 caveat at gate grade (D6) and the paired-ratio
   dispersion recorded as the comparison rule's second real-arm
   calibration data point.
4. On adopt: a sized hand-off note (D7). No product change remains after
   the session (research convention; temporary env-gated init hook only).

## The arms and their temporary implementation

Both arms are the plan15 P3 definitions, applied at the existing
`(1, 1)` fallback site (`src/search/dfpn/children.rs`, the unsolved-child
initialization when the child has no reusable TT bound; the guard — no
reusable bound, not repetition-flagged — is exactly plan15's):

- `mob-or` — OR-parent children init `(pn, dn) = (n, 1)` (child has the
  defender to move, `n` = child mobility), all other fresh children `(1, 1)`;
- `mobcap8` — both sides init `(min(n, 8), 1)` / `(1, min(n, 8))`.

Implementation is a **temporary env-gated hook** (`ATOMIC_INIT=mob-or|mobcap8`,
unset = shipped behavior), re-derived from report15's P3 definitions — the
plan15 instrumentation was reverted, so the hook is a fresh minimal port
(init only; no counters). Command line per run is plan13's D2 shape:

```
ATOMIC_INIT=<arm|unset> atomic_solver --fen <FEN> --tt-size 128 \
    --first-outcome --outcome-only --salt S --budget C --timeout 600
```

## Pre-registered scope decisions

- **D1 — corpus, caps, salt set (inherited verbatim, frozen).** The 22
  plan12 fixtures and per-case caps (stress 2.5 B, m20_white 2 B, others
  1 B; `--timeout 600` secondary); salts {0, 1, 2, 3, 4, 5}; salt 0 always
  runs. Extensions pre-registered only. Candidate set fixed at
  {`mob-or`, `mobcap8`}; retired arms are not re-run; no blend arms.
- **D2 — soundness invariants (hard gates, any violation halts).**
  (a) **Pristine identity:** the uninstrumented product build at HEAD
  reproduces the plan12 salt-0 counts exactly (stress 249,480,478; m22
  14,156,269; m23 9,673,403; dec13 3,822,602; dec10 4,262,128; m20
  censored) — run *before* applying the hook.
  (b) **Hook non-perturbation:** the instrumented build with `ATOMIC_INIT`
  unset reproduces the same counts exactly (per-salt, all 132 baseline
  cells against plan12's recorded per-salt values).
  (c) **plan15 reproduction anchors:** the init-on candidate cells that
  plan15 measured uncensored reproduce plan15's archived
  `state/arms.json` values exactly — `mob-or`: stress 71,148,597 (cap
  300 M), dec13 486,175, dec10 5,148,137, m22 15,813,483 / 11,859,889 /
  14,223,312 (salts 0/1/2); `mobcap8`: stress 98,874,361 (cap 300 M),
  dec13 509,576, dec10 5,765,793, m22 15,067,529 / 12,825,207 / 13,387,712.
  These 12 deterministic cells pin the init mapping, the
  `child_is_or` ↔ `(n, 1)`/`(1, n)` orientation (report15's named easy
  mistake — a flip would reproduce the archived `mob-and` numbers
  instead), and the port fidelity. If a cell mismatches: debug the port
  first; if the port is faithful and the mismatch persists, the anchor is
  recorded as lost (the rollout's paired fresh runs still carry the
  verdicts) and the discrepancy is reported.
  (d) **Orientation trace (belt-and-braces):** a temporary stderr trace of
  the first fresh-child inits on a small FEN, hand-verified against
  `list_legal` mobilities of the child positions (parent OR ⇒ `(n, 1)`).
  (e) **Outcomes:** zero decisive-outcome conflicts within any arm across
  salts; every uncensored run matches its fixture `expected` outcome. The
  init steers only — solved values are computed exactly as before — so an
  arm-level outcome flip is a soundness defect, not noise.
  (f) A budget-censored `draw` is the no-result sentinel, never a proven
  draw. (g) **Revert:** `git diff --exit-code` clean after the hook is
  removed; release build re-verified; (a) re-run exact.
- **D3 — verdict machinery (pinned v1.0, applied verbatim).** Per case:
  per-salt paired ratios `candidate_s / baseline_s` over pairs where both
  sides are uncensored; censoring pattern per salt; baseline basin count
  from plan12. Verdicts: censored-win / censored-loss / win (median ratio
  ≤ 0.9 ∧ ≥ 75 % of ratios < 1) / regression (median ≥ 1.1 ∧ ≥ 75 % > 1) /
  unclear. Lever verdict per arm: **adopt** = ≥ 1 censored-win or win on a
  hard-class case (stress, m20_white, m22_white, m23_white) ∧ no
  regression / censored-loss anywhere; **reject** = any regression or
  censored-loss; otherwise **defer**. Low-diversity baselines (basin count
  < 3) cannot solely support an adopt. Under-budget rule: > 3 of 6
  censored salts on a case for either arm ⇒ case unresolved, contributes
  nothing.
- **D4 — defer extension rule (pre-registered).** A defer is re-judged only
  after extending the salt set **by {6, 7} on the affected cases, both
  arms**, fresh runs at the added salts; the pooled verdict is recomputed.
  One extension per arm; a second defer stands.
- **D5 — the m22 thrash amplifier.** report15's P1 showed the collapsing
  arm's m22 pathology is deep repetition-region thrash. m22 is hard-class,
  so a candidate m22 regression vetoes at full weight per the pinned rule
  — no special treatment is pre-registered; the P1 anatomy is only context
  for interpreting the tables.
- **D6 — the dec10 caveat (pre-registered).** The pinned rule is applied
  verbatim, including at dec10: a gate-grade dec10 **regression** rejects
  the arm (non-hard-class cases still veto under "no regression on any
  pre-registered case"); an **unclear** dec10 does not. No mid-plan
  amendment, no W_LO/W_HI change. If a candidate's *sole* negative is a
  dec10 regression whose paired ratios sit inside/at the quiet-band edge,
  the report records the tension as a methodology-calibration question for
  the owner — "should a non-hard-class regression within the noise band
  veto a hard-class win?" — as data with a stated reason (the standing
  revision rule), never as an applied rule or a silent override.
- **D7 — hand-off rule (pre-registered).** An **adopt** arm becomes a
  sized hand-off note proposing implementation as a product feature
  (env knob → constructor option, `(1, 1)` default preserved, per-arm unit
  tests on the init mapping and outcome-consistency regressions including
  m22 and the dec10 class — report15's missing-tests list). Recipient:
  `dfpn` (module owner of `src/search/dfpn/children.rs`; the change is
  search semantics, not wall time), i.e. a `dfpn` reopen proposal under
  trigger (a) — a new *measured* search-semantics result for the hard
  class with no home in `lean` (wall time) or `conversion` (conversion
  class). If the owner prefers `lean`'s heuristic charter, that is an
  owner decision recorded in the hand-off, not decided here. **Reject**
  arms are recorded with their tables; the #18 family record closes or
  pivots per the outcome table below.

## Method

- **Phase 0 — anchors + tooling.** Verify HEAD is clean; run D2(a) on the
  pristine build. Port the init hook (minimal: the env gate + the mobility
  computation at the `(1, 1)` fallback site); build release; run D2(b),
  D2(c), D2(d). Write `measurements/plan16/driver.py` + `parse.py` adapted
  from plan13's (arm dimension via the env var, resumable, ≤ 3 concurrent
  processes, transcripts to a non-committed RAW dir) and `verdicts.py`.
- **Phase 1 — rollout.** 396 runs (3 arms × 22 cases × 6 salts); the
  baseline arm duplicates plan12's cells by design (identity audit, D2(b)).
  Parse to `state/runs.json` + `state/summary.json`; run the D2 checks.
- **Phase 2 — verdicts.** `verdicts.py` → `state/verdicts.json` (D3 tables
  per case per arm, censoring patterns, basin counts, paired-ratio
  dispersion summary); apply D4/D6 as pre-registered.
- **Phase 3 — revert + report.** Remove the hook; D2(g); write
  `measurements/plan16/` (drivers, `env.json`, parsed state, `README.md`),
  D7 hand-off note if any; write `report16.md` (final task); update the
  `initiative.md` #18 row (Phase-1 verdict + hand-off pointer or the
  family outcome); refresh the `docs/plans/README.md` research row **only**
  if an adopt hand-off results (plan13 precedent).

## Gates (verdicts, not GO/NO-GO)

- **adopt (either arm)** → D7 hand-off note; #18 Phase 1 resolved
  positively; the non-adopted sibling's verdict recorded alongside.
- **both reject** → #18 closes measured-out: the surviving shapes fail at
  gate grade; the Phase-0 nomination stands as below-noise-floor data.
- **defer** → D4 extension, then re-judge; a standing defer is recorded
  with the dispersion data.
- **HALT** → any D2 invariant violated: soundness/identity investigation,
  not a measurement.

## Out of scope

- Productization of the init variants (D7 is a hand-off note; the research
  initiative stays research-only; the temporary hook is reverted).
- #18's later stages (atomic features: blast / king-zone threats;
  clock-borrowed initialization from another rule50 clock) — a future plan
  if the shaped family adopts or the owner redirects.
- Blend arms, retired arms (`mob`, `mob-and`, `moblog`), #19 remaining
  closures, #20 restarts, #21 in-context child results, #22 fringe.
- `gate_methodology.md` changes (D6 records a calibration question at
  most); benchmark/optimizer interface; proof-tree layer; preflight.

## Budget envelope

Phase 0: hook port + anchors ≈ 30–45 min (the 12 anchor cells are cheap:
dec13/dec10/m22-class runs are seconds; the two stress cells run at the
300 M plan15 cap ≈ 2 × 30 s). Phase 1: 396 runs; the candidate arms are
expected *faster* than baseline on the expensive cases (stress 0.29× /
0.40× salt-0), so ≈ plan13's envelope: ~2.5 h CPU-serial, ≤ ~1 h wall at
3-way concurrency. A candidate arm exploding somewhere unexpected is
absorbed by the caps + under-budget rule (censored cell ≈ cap at ~10 M
evals/s). Phase 2–3 are minutes.

## Final task

Write `report16.md` in this directory: deliverables, per-arm verdict
tables with basin counts and censoring patterns, the dec10 decision (D6)
and the paired-ratio dispersion record, problems encountered, missing
tests, next steps (adopt → hand-off kickoff prompt; reject → #18 closure
record; defer → extension prompt; alternative: the #19 remaining arms).
