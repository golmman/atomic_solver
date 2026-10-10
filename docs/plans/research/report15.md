# report15 — plan15 (item #18, Phase 0: the m22 collapse diagnosis)

Date: 2026-10-11. Initiative: `research`. Evidence:
[`measurements/plan15/`](measurements/plan15/) (drivers + parsed state +
`env.json`; raw transcripts and traces uncommitted, regenerable via
`probe15.py`). Instrumentation: a hand-port of the archived reexam
`probe.patch` (`bb19e7b`) to `d7de659`/`b09ff7a` — `src/search/probe.rs`
(counters C1–C8 plus the plan15 additions: children-per-frame histogram,
evals-by-ply buckets, bounded 2²⁶-slot unique-key set), hooks in
`children.rs`/`core.rs`/`dfpn/mod.rs`/`search/mod.rs`, and
`examples/probe_solve.rs`. All temporary; **no product change remains**
(`git diff --exit-code` clean; release build re-verified; salt-0 anchors
exact before and after).

## Verdict: **GO-plan16** (pre-registered rule fired on two sub-arms)

| arm | stress | dec13 | dec10 | m22 s0 / s1 / s2 | dir. win | m22 ≤1.5× uncens. | GO |
| --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 249.5 M | 3.82 M | 4.26 M | 14.16 M / 23.65 M / 15.82 M | — | — | — |
| mob (naive) | **0.235** | **0.160** | 1.83 | cens / cens / cens | yes | 0/3 | no |
| **mob-or** | **0.285** | **0.127** | 1.21 | **1.12 / 0.50 / 0.90** | yes | 3/3 | **yes** |
| mob-and | cens ≥1.20 | 2.19 | 1.27 | 9.15 / 2.02 / 5.89 | no | 0/3 | no |
| **mobcap8** | **0.396** | **0.133** | 1.35 | **1.06 / 0.54 / 0.85** | yes | 3/3 | **yes** |
| moblog | 0.853 | 0.457 | 1.31 | cens / cens / cens | yes | 0/3 | no |

Ratios are first-outcome child evals vs the salt-0 baseline (per-salt m22
baselines re-measured: 23,653,031 / 15,815,489 — matching the reexam
values). `mob-or` (OR-parent children `(n, 1)` only) and `mobcap8` (both
sides `min(n, 8)`) satisfy the pre-registered GO rule: a stress-or-dec13
direction win (ratio ≤ 0.9) **and** m22 ≤ 1.5× baseline uncensored on ≥ 2 of
3 salts — both hold on 3/3 salts. `mob-and` and `moblog` are retired. All
readings remain below the pinned gate's noise floor on the quiet band
[0.76, 1.32]; they nominate candidates, plan16 re-measures under the
paired-salt rule. No HALT: every soundness invariant passed (below).

## Soundness invariants (all pass)

- **(a) Probe-disabled identity:** pristine product CLI at HEAD reproduces
  the plan12 salt-0 counts exactly — m22 14,156,269 / dec13 3,822,602 /
  dec10 4,262,128 / dec15 1,077,420 — before *and* after the instrumented
  phase; stdout byte-identical (stderr differs only in wall-clock
  `elapsed=`/`nps=` annotations).
- **(b) Probe-enabled non-perturbation:** the counter-only probe arm
  (counters + unique-key set on, init/trace off) reproduces the same four
  counts exactly.
- **(c) Outcomes:** every uncensored run of every arm reports `win`; every
  censored run is `draw` + `BudgetExhausted` (no-result sentinel).
- **(d) Determinism:** reruns of (m22, base) and (m22, mob-or, salt 0) are
  identical bit-for-bit across all counters (modulo `wall_s`).
- **(e) Revert:** instrumentation files deleted, four edited files restored;
  `git diff --exit-code` clean; release build re-verified; anchors re-run
  exact.

## Mechanism classification (P1–P3)

**The decomposition is clean: the win and the collapse live in different
halves of the naive arm.**

1. **The stress/dec13 win is carried by the OR-side `(n, 1)` init** — the
   half H-A named. On stress, `mob-or` alone reaches 0.285× (full mob:
   0.235×); on dec13 it *beats* the full arm (0.486 M vs 0.613 M). Its
   anatomy (`anatomy.json`): the first-sweep-cut mass transfers from AND
   frames to OR frames (stress OR 27.1 %→60.3 %, AND 56.4 %→9.0 %; dec13 OR
   29.0 %→62.5 %, AND 58.5 %→1.4 %) and fresh-frame thrash drops (stress
   fresh 91.1 %→77.2 %) — the search stops paying to discover branching
   factors through wasted fresh expansions and pays slightly more per frame
   (evals/frame 17.95→23.29) for far fewer frames (13.9 M→2.97 M on stress).
2. **The collapse (and the losses on stress/dec13) is carried by the
   AND-side `(1, n)` init** — the half H-B named. `mob-and` alone is
   catastrophic everywhere: stress censored at 300 M, dec13 2.19×, dec10
   1.27×, m22 9.15×/2.02×/5.89×. Its m22 anatomy is thrash-shaped: unique
   share 13.8 %, stale-eval share 63.8 %, explored marks 10.5 M (12× frames).
3. **The m22-specific amplifier under the full arm is deep repetition-region
   thrash, not honest expansion.** Under naive mob the m22 trajectory
   immediately enters a >90-ply maneuvering region — 99.7 % of all 208 M
   frames sit at ply > 90 (baseline: 69 frames), the P2 selection trace
   cycles at ply ~144–152 within its first million recursions (baseline
   stays at ply 6–12) — and that region holds only **176,385 distinct
   positions**: unique share collapses 83.3 %→**0.085 %**, ~1 `explored`
   mark per frame, 4.8 evals/frame, first-sweep cuts no longer fresh
   (0.037 % vs 92.3 %), INF-bound reuses 11.2 M. Censored at 1 B on all
   salts (systematic, 3 salts here + reexam's 5). **H-D (honest expansion)
   is refuted** — the distinct-node share falls instead of rising.
4. **Neither pre-registered full-run signature matched the observed full-mob
   anatomy** (H-A predicted OR-side cuts drop — observed; but H-B predicted
   AND-side cuts collapse while OR-side stays near baseline — instead the
   AND-side share stayed 57.4 % vs 58.0 % baseline and the OR-side collapsed
   22.7 %→0.2 %). The two halves interact; the causal attribution comes from
   the P3 split, and it is unambiguous. **H-C's pre-registered counter
   signature did not separate** (m22-mob stale-eval share 11.6 % vs
   stress-mob 10.7 %; suppressed-draw stores 4,752 vs 2,144), but the
   thrash *shape* (tiny distinct set, stale re-derivations, deep-ply
   cycling) is real and is the amplifier on m22.
5. **P2 divergence:** the base and mob selection traces diverge at the
   **first recursion** (index 0) on both m22 and dec13 — the mob root
   selection picks a different child immediately (with `(n, 1)`-style
   bounds, min-pn no longer ties at 1), and the trajectories never rejoin.
   m22-mob's trace hit the 2²⁴-record cap while the baseline trace has 858 K
   records total.

**Best single-sentence mechanism:** the `(n, 1)` init on defender-to-move
children makes OR frames' first-sweep cuts *informative* (the parent's own
bound stops being deceptively 1) and transfers cut mass to where it is
cheap; the `(1, n)` init on attacker-to-move children destroys the AND-side
cut regime, and on m22 the steered trajectory dives into the deep
repetition-cycle region of its own 95-ply win, where a 176 K-position set is
re-derived ~1,200× over.

## Nominated plan16 candidates

- **`mob-or`** — OR-parent children `(n, 1)`, `(1, 1)` elsewhere. Primary
  candidate: strongest direction wins, m22 ≤ 1.12× on all salts.
- **`mobcap8`** — both sides `min(n, 8)`. Secondary: slightly weaker on
  stress (0.396×) but also m22-clean; its AND-side half is conservative.
- Retired: `mob` (collapse), `mob-and` (poison half), `moblog` (m22
  censored on all 3 salts — even a log-shaped AND-side init suffices to
  break m22).
- plan16 scope suggestion: gated rollout of `mob-or` and `mobcap8` over the
  canonical corpus (22 cases × 6 salts, gate v1.0, paired comparison,
  censored-as-censored), with the pre-registered adopt rule from
  `gate_methodology.md`. A blend is **not** indicated: the AND-side half is
  the damage carrier, so the surviving space is OR-side-only or
  conservatively-capped both-sides.

## Problems encountered

- The archived patch does not blind-apply (HEAD moved: `--salt` became a
  product option, chunks became configurable, `last_child_solved` did not
  exist); it was ported by hand. The wpns/chunk0 arms were not ported (out
  of scope).
- The `children_bucket` helper initially mis-sized its >40 bucket (13 vs 14
  slots); caught at compile+review, fixed before any measurement run.
- The 512 MB unique-key set is allocated lazily at the first frame insert
  (unit tests create many `Search` instances; lazy allocation keeps a stray
  instrumented build harmless). Documented as the temporary exception to
  "RAM = TT only", same status as the preflight `REGION_BUDGET`.
- Descendant-eval counters overlap across nesting (reexam caveat, carried
  over): `ply_evals` sums can exceed `child_evals`; the ply table shows
  where deep recursion happens, not additive work. Only the first-sweep-cut
  share is additive.
- Session-clock note: this session's container clock reads 2026-10-10 while
  the prior session's records use 2026-10-11; the initiative history keeps
  the record's internal chronology (2026-10-11).

## Missing tests

- None to add: no product code changed; the existing suite is untouched and
  `make test` was not required for a no-code-change session (per report14
  precedent; the release build was re-verified after the revert, and the
  salt-0 trajectory identity — a stronger check than the smoke tests — was
  run twice).
- If plan16 productizes an init arm, it will need: a `(1, 1)`-identity test
  (default init unchanged), per-arm unit tests on the init mapping
  (`child_is_or` ↔ `(1, n)`/`(n, 1)` orientation is the easy thing to get
  backwards), and outcome-consistency regression cases including m22 and the
  dec10 class.

## Next steps

1. **Draft plan16** (item #18 Phase 1): gated shaped-variant rollout of
   `mob-or` and `mobcap8` over the canonical 22 × 6-salt corpus under
   `gate_methodology.md` v1.0; pre-register the adopt rule and the
   decision on the dec10 regression (both candidates regress dec10
   1.21×/1.35× — inside/at the edge of the quiet band, single-draw).
2. Alternative: defer plan16 and first close #19's remaining re-score arms
   (clock-budget reuse, history/killer, eviction V2) — they are drafted
   next in the queue and independent of #18.

SESSION COMPLETE
- plan15 executed end-to-end: identity anchors exact (a/b), P1 anatomy +
  P2 divergence + P3 micro-arms measured, determinism verified (d),
  instrumentation reverted and re-verified (e); measurements/plan15/
  archived (probe15.py, parse15.py, env.json, state/{anatomy,divergence,
  arms}.json, README.md); report15.md written; initiative.md #18 row +
  history updated (GO-plan16, plan16 pointer); docs/plans/README.md research
  row deliberately not updated (GO is neither a pivot nor a close; the plan
  pre-registered index updates only for pivot/close-worthy outcomes).
- Verdict: GO-plan16 (candidates mob-or, mobcap8; mob-and/moblog retired).
Follow-up options:
  1. Draft plan16 — the gated shaped-variant rollout of mob-or and mobcap8
     (22 cases × 6 salts, gate v1.0, paired comparison): the pre-registered
     successor this plan exists to enable.
  2. Execute the #19 re-score arms first (clock-budget reuse, history/
     killer, eviction V2) if the owner wants the independent queue drained
     before #18's rollout adds candidates to it.
