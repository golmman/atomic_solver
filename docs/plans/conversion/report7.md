# Report: Partial-Sum Sweep Short-Circuit — Phase 1 Gated Spike (Plan 7)

Executed 2026-09-18 per `plan7.md`. The mechanism (partial-sum sweep
short-circuit at summed-bound thresholds, behind `CONV7_SPIKE=1`) was
implemented, measured, and **fully reverted** — `src/` and `examples/` are
byte-identical to the pre-spike tree (`git diff` empty; post-revert stress FO
output md5 `ffd3d015e9aa1a99efc1a3dad6831ae8`, matching the HEAD capture;
fast gate green). Raw artifacts under
`docs/plans/conversion/measurements/plan7/`.

## Verdict

**NO-GO.** The 74.2% first-order surface from `report6.md` is real, but the
realized mechanism is a large **net loss**: weaker stored bounds convert the
search into re-entry churn that outweighs the in-sweep savings by a wide
margin, and — the decisive failure — the early cut can sit *between* the
sorted children prefix and a decisive child later in the list, so wins the
full sweep would prove are deferred past the entire budget.

| case (mode) | baseline child evals | spike-on child evals | ratio | outcome | wall |
| --- | --- | --- | --- | --- | --- |
| stress FO | 249,480,478 | 817,976,676 (120 s timeout) | **3.28× worse** | **win 477 plies → draw (resource-cut)** | 47 s → 120 s |
| stress default | 338,094,183 | 794,858,304 (120 s timeout) | **2.35× worse** | **win 129 plies → draw (resource-cut)** | 62 s → 120 s |
| m22_white (FO, control) | 14,156,269 | 30,603,544 | 2.16× worse | win kept (95 → 75 plies) | 2.6 s → 4.8 s (1.83×) |
| m23_white (FO) | 9,673,403 | 22,305,542 | 2.31× worse | win kept (33 → 49 plies) | 1.8 s → 3.2 s |
| m24_white (FO) | 406,737 | 1,101,351 | 2.71× worse | win kept (15 → 15 plies) | 0.08 s → 0.16 s |

The stress outcome regression is *not* a false outcome (the 120 s runs ended
in a resource-cut `Draw`, stored bounds only, soundness intact) — it is the
bound-weakening offset dominating exactly as `report6.md` risk #1 warned,
with the missed-decisive-child asymmetry on top.

## Step 0 — identity anchors (gate off, spike code present)

All five HEAD baselines reproduced exactly before any spike-on run: stress FO
249,480,478 / 13,907,467 / 477 plies (PV byte-identical); stress default
338,094,183 / 19,943,731 / 129; m22 14,156,269 / 858,117 / 95; m23
9,673,403 / 553,100 / 33; m24 406,737 / 22,739 / 15. The gate-off build is
therefore bit-identical to HEAD (the plan6 anchor protocol).

## Mechanism as implemented

Exactly the plan's sketch, plus one soundness guard the plan's contract
implied but the sketch did not spell out:

- `evaluate_all_children` received `th_pn`/`th_dn` and accumulated the
  running summed bound (AND: Σ `child.pn`; OR: Σ `child.dn`, saturating,
  `min(INF, …)` — mirroring `select_from_children`'s arithmetic exactly),
  breaking when the sum reached the frame's corresponding *finite*
  threshold; the frame then exited through the existing threshold-cut path
  over the partial table (unsolved bounds `(partial_sum.max(1), …)`, no
  `NodeProven` events, identical TT store path).
- **All-solved-prefix guard (essential).** A solved-Draw child contributes
  `pn = INF` at an AND frame, so a naive crossing can fire on a partial
  table in which every present child is solved; `is_solved_by_children`
  would then "prove" a Draw (or Loss) from a subset while unevaluated
  children exist — an unsound outcome. The crossing was therefore allowed
  to fire only when the partial table contained at least one *unsolved*
  child. With the guard, every early-cut frame is provably one the full
  sweep also threshold-cuts (the full table still contains that unsolved
  child ⇒ no solved outcome; full Σ ≥ partial Σ ≥ threshold ⇒ the loop's
  cut test fires), so frame-level outcomes are preserved and only the
  stored bound weakens. Measured guard activity is tiny (the false-proof
  configuration is rare): 4,470 blocked crossings on stress FO vs 89.4M
  early cuts — the guard costs nothing and was never the limiting factor.
- Decisive-child early exit unchanged and outranking the crossing check
  (a Loss child breaks the sweep before the sum is accumulated).
- Prefix property: the partial table is a prefix of the sorted full move
  list, so `store_best_child` indices keep their full-table meaning — the
  plan's indexing audit concern is resolved by construction.
- Spike counters (temporary `spike7.rs` + hooks + a temporary
  `examples/solve_stats.rs` runner, the plan6 pattern): early-cut frames,
  in-sweep savings, per-position re-entry map with **own-eval attribution**
  (delta-0 verified on every run), skipped-children and crossing-overshoot
  histograms, stored-bound-vs-pre-entry-TT-bound comparison.

## Phase 1 spike-on anatomy (why it fails)

- **In-sweep savings are as predicted.** 89.4M early cuts on stress FO
  skipping a gross 1.29B child evaluations (OR concentrated at 16–31
  skipped children, AND at 8–15 — matching report6's per-frame
  distribution), with crossing overshoot ≈ 0 in ~99% of cuts (the sweep
  stops at the minimum prefix).
- **Re-entry churn eats the savings and then some.** Positions with ≥ 1
  early cut: 14.16M of 14.62M mapped positions (96.9%); evals spent at
  their re-entries: **787.8M = 96.3% of the entire 818M-eval run**. The
  re-sweeps themselves are cheap (TT-resolved children cost 1 eval), but
  the weaker stored bounds re-price the whole threshold lattice: parents
  keep descending into children whose partial bounds look cheap, and each
  re-entry extends the sweep by ~1 threshold increment instead of
  re-cutting at a full-sum bound.
- **The stored bounds themselves look healthy** (46–49% same, 47–49%
  stronger vs the pre-entry TT bound, only 1–4% weaker) — the damage is
  not bound collapse but the *missing full-sum information*: today's cut
  frame stores `Σ` over all children, which lets the parent cut
  immediately on re-entry; the partial sum cannot.
- **Missed decisive children.** An early cut between the crossing point and
  a later Loss child forfeits the win proof at that frame (the existing
  threshold cut cannot do this — its full sweep always finds decisive
  children first). This is the mechanism that turned the stress win into a
  120 s timeout: with the mechanism active, the first-outcome phase never
  re-derives the 477-ply proof at any budget it reached.

## Go/no-go bars, one by one

1. **Stress FO net saving ≥ 10%** — **FAIL**: net −228% (3.28× more evals)
   and the win is lost within 120 s.
2. **Stress default net saving ≥ 5%** — **FAIL**: net −135% (2.35× more),
   win lost within 120 s.
3. **m22 control: win found, wall ≤ 2× baseline** — **passes the letter**
   (win found, 1.83× wall) but shows the same 2.16× eval inflation; the
   destabilization canary is flashing, not green.
4. **Quick suite: 59/59 outcomes unchanged, no case > 2× evals** —
   **FAIL**: 2/59 outcome changes (`dec01` and `m23_white`, win → draw
   within the suite timeout), 28/59 cases regress > 2× evals, median ratio
   1.96×, worst 92.8× (`dec42`); total quick-suite evals 2.38×.
5. **Cyclic-rook repetition gate** — green (`test_repetition
   --include-ignored` with the mechanism active).

**NO-GO** per the plan's pre-registered rule (bars 1 and 2 alone decide it).

## Revert verification

- `src/search/dfpn/spike7.rs` and `examples/solve_stats.rs` deleted;
  `children.rs` / `core.rs` / `mod.rs` restored — `git diff` empty on
  `src/` and `examples/` (byte-identical to HEAD).
- Post-revert rebuild: stress FO output (outcome/pv/pv_status) md5
  `ffd3d015e9aa1a99efc1a3dad6831ae8`, identical to the HEAD capture taken
  before the spike (and to `plan6/stress_fo_head.txt`'s content).
- `make test` green (31 suites, 0 failures).

## Backlog #6 closure

Backlog #6 (threshold-cut-frame pricing) **closes as a measured no-go** on
the combined evidence: `report6.md` (the surface exists — 74.2% first-order
— and classes A/B/C are closed) and this report (the one surviving mechanism
realizes a 2.2–3.3× net loss with outcome regressions). The pricing lane is
exhausted at the diagnostic level: cut-frame mass is structural (full sweeps
of re-priced thresholds), not recoverable by stopping sweeps early. The
initiative's remaining levers re-rank: #4 (parallel, jointly owned with
`lean` #2) is the only lever of scale left; #5 (c)/(e) reading items and the
parked #2a ordering-guidance lane follow.

## Tools / examples used

- Temporary env-gated `src/search/dfpn/spike7.rs` (gate + counters +
  per-position own-eval map + dump) with hooks in `children.rs`
  (crossing test, eval attribution), `core.rs` (frame-entry/-exit hooks)
  and `mod.rs` — all reverted; tree byte-identical to HEAD.
- Temporary `examples/solve_stats.rs` runner (`--fen/--timeout/
  --first-outcome/--tt-size`, prints the run summary + spike dump) —
  deleted (the plan6 `solve_stats` pattern).
- Measurement scripts: plain `bash` + the `benchmark --json` outputs,
  compared with a short `python3` script (kept inline in the session only).
- Raw artifacts: `docs/plans/conversion/measurements/plan7/*.txt|json`
  (gate-off anchors, spike-on runs, quick-suite gate-off/spike-on JSON).

## Problems encountered

1. **First instrumentation round had inflated attribution**: the
   per-position map initially recorded per-frame *subtree* evals
   (`child_evals` deltas include descendants), so the map total exceeded
   `child_evals` (no delta-0). Fixed mid-session to the spike6 own-eval
   method (`frame_begin`/`count_eval`/`on_frame_exit` slots per active
   depth); all reported numbers are from the corrected build, delta-0
   verified on every run.
2. The stress spike-on runs hit the 120 s wall-clock cap (that *is* the
   finding); their eval counts are trajectory samples of a resource-cut
   run, not converged results — the no-go does not depend on them
   converging.
3. plan6's `stress_fo_head.txt` captured stdout only, while a casual
   `> file 2>&1` mixes in the chunk log; the identity comparison needs the
   deterministic stdout lines (or dedup) — noted for the next spike author.

## Missing tests

None retained — with the spike reverted there is no feature surface to
test. The identity anchors (gate-off baseline reproduction, post-revert
byte-identity, delta-0 attribution) live in this report and the raw
artifacts only. Had the verdict been a go, Phase 2's unit-test list
(crossing-at-first-child, INF-threshold inertness, AND/OR symmetry,
re-entry resumption, no-solved-outcome stores, partial-table `best_child`
indexing) would have applied to the surviving mechanism.

## Next steps

- Backlog #6 closed (see above); `initiative.md` row and History updated;
  `docs/plans/README.md` `conversion` row updated.
- The initiative's live surface is now #4 (parallel spike, with `lean` #2)
  plus the #5 (c)/(e) reading items; #2a stays parked. The dormancy
  question for the initiative (all levers closed → parallel spike is the
  last one of scale) is a decision for the next session, not acted on
  here.
