# plan14 — Item #23: seeded move-order tie-break noise channel

Initiative: `research` (re-opened 2026-10-09). Executes reopened backlog
item **#23** (opened 2026-10-10). Self-contained: a fresh session can
execute this without reading the full record. Item #17 is done
(`gate_methodology.md` pinned v1.0); this plan extends its noise-channel
family, it does not re-score any lever (#19 re-scores remain **plan13**
per report12).

## Motivation

Two plan12 findings motivate a second, mechanically different noise
channel:

1. **The basin ceiling.** Per-case draw diversity is bounded by trajectory
   structure, and salt 5 did not produce a clean fifth channel on the
   low-diversity cases (dec13 = 2 basins, dec14 = 3, dec10 = 4). The salt
   perturbs a trajectory *only* through TT bucket collisions; wherever TT
   interaction is sparse it is silent. A tie-break perturbation acts at
   **every** `sort_moves` call, independent of TT interaction — the one
   lever that can widen basins where the salt cannot.
2. **The standing caveat.** Report12 lists "salt-channel independence from
   other noise sources (tie-breaking, history aging)" as unmeasured.
   Making tie-breaking a seeded, controlled channel converts that confound
   into a measured property.

Read-only opening-session probe (`move_order_debug`, m20/m22/m23 fixtures,
history/killers at zero): root ordering totals tie in large blocks —
m20_white: 4 rook moves at 560, 4 at 1580/3070-family, 3 king steps at
110, 6 moves at 0; m22_white/m23_white similar. So ties are plentiful
where the dynamic bonuses are unpopulated. **Unmeasured and decisive for
this plan: tie prevalence mid-search**, where history and killer bonuses
have accumulated. That is the Phase 0 question.

## Objective

1. **Phase 0 (temporary instrumentation, reverted):** measure in-search tie
   prevalence — per `sort_moves` call, the tie-group structure of the
   composite score — with a pre-registered kill gate.
2. **Product knob:** `--seed <u64>` CLI option + `Search::set_seed`,
   mirroring the `--salt` plumbing. Seed 0 = shipped behavior,
   bit-identical (salt-0 identity contract preserved at every salt).
3. **Calibration rollout** over the plan12 corpus: soundness audit, basin
   counts under the seed channel, and a **cross-channel correlation arm**
   (seed draws vs. salt draws — same basins or new ones?).
4. **Conditional methodology amendment:** pin the seed channel into
   `gate_methodology.md` (v1.0 → v1.1) only if it is sound, non-degenerate,
   and adds basins; otherwise record it as measured-out and update the
   caveat.

## Pre-registered scope decisions

- **D1 — mechanism (fixed).** The tie-break is a **stateless hash**, never
  a stateful RNG: `tiebreak(m) = splitmix64_mix(seed, node_zobrist_key,
  move_bits)`, used as the *secondary* sort key
  `(Reverse(score), tiebreak)` in `sort_moves` (`history.rs`). Rationale:
  (a) a consumed-RNG stream makes the tie-break visit-order-dependent,
  breaking the path-independence philosophy maintained everywhere else
  (same position always ties the same way regardless of the path); (b) the
  node Zobrist key is already in hand (`&Position`), so the cost is ~2
  multiplies per scored move. Non-tied comparisons are bit-identical to
  the shipped ordering — **tie-break only, never score noise** (the
  `ScorerParams`/optimizer contract stays intact).
- **D2 — seed-0 identity (hard).** With seed 0 the tie-break key is not
  computed at all and the existing exact code path (stable sort on the
  composite score alone) runs unchanged: shipped trajectories, all
  plan11/plan12 salt-0 reproduction targets, and every existing test are
  untouched. The TT-hint front-slot promotion (`best_from_tt`) is applied
  after sorting, exactly as today, under every seed.
- **D3 — scope (fixed).** `sort_moves` only. AND-node ordering goes through
  the same call site; `selection.rs` node-selection ties and history-aging
  jitter are explicitly out of scope (future items if the channel
  calibrates).
- **D4 — Phase 0 probe design + kill gate (pre-registered).** Env-gated
  counters (temporary `src/` instrumentation, e.g. `ATOMIC_TIE_PROBE=1`),
  per `sort_moves` call recording: moves, size of the tie-group at the
  *top* score, total moves in tie-groups, whether the top-2 tie. Probe
  cases: the quick suite plus m22_white, m20_white, dec13, dec10 at short
  bounded solves. **Kill gate:** proceed to productization only if nodes
  with a top-2 tie are ≥ 10% of all `sort_moves` calls on ≥ 2 of the 4
  cases; otherwise #23 closes measured-out (H0) — a tie-break channel with
  no ties to break cannot perturb anything. Probe hygiene: quick suite
  bit-identical with the probe disabled; `git diff --exit-code` after the
  revert.
- **D5 — soundness invariants (hard gates, any violation halts).**
  (a) seed-0 identity: at salt 0 *and* at every other salt, seed-0 runs
  reproduce the plan11/plan12 baseline counts exactly;
  (b) zero decisive-outcome conflicts across seeds on any case;
  (c) every uncensored outcome matches its fixture `expected` value;
  (d) determinism: a rerun at the same (case, salt, seed) reproduces
  stdout and child evals bit-for-bit (within-(salt, seed) variance = 0);
  (e) a budget-censored `draw` stays the no-result sentinel.
- **D6 — rollout design (pre-registered).** Corpus = plan12's 22 named
  fixtures and per-case budgets (D3 of plan12, inherited verbatim:
  stress 2.5 B, m20 2 B, others 1 B; `--timeout 600` secondary). Matrix:
  **salt 0 × seeds {0, 1, 2, 3, 4, 5}** (132 runs; seed 0 = identity
  anchor). Cross-channel correlation arm on the eval-sensitive cases
  (m-family + dec01/dec10/dec13/dec14 per plan12): **salts {1, 2, 3} ×
  seeds {1, 2, 3}**, 9 extra runs per affected case, no salt-0/seed-0
  duplication (those cells exist in the main matrix).
- **D7 — basin and correlation rules (pre-registered).** Basin granularity
  0.25% of child evals, as pinned. For each eval-sensitive case:
  (i) seed-channel basin count over the salt-0 × seed matrix;
  (ii) cross-channel test — over the 3×3 correlation arm, is the
  (seed fixed, salt varies) spread near-identical (< 10⁻⁴ relative) while
  (salt fixed, seed varies) moves? A seed draw defines a **new basin** if
  it is not within 0.25% of any salt-channel draw of that case.
- **D8 — verdict rules (pre-registered).**
  - **GO** (channel additive): zero D5 violations, and the seed channel
    adds ≥ 1 new basin on ≥ 2 of the low-diversity cases (dec13, dec14,
    dec10) or raises any hard-class basin count. Then
    `gate_methodology.md` is amended to v1.1: canonical seed set pinned
    from the data, the paired comparison extended to (salt, seed) cells,
    basin counts recomputed; every change stated with its data.
  - **H1** (channel sound but non-additive): no D5 violations, but seed
    draws collapse into existing salt basins and widen nothing. #23
    closes as measured-out; the methodology caveat is rewritten from
    "tie-breaking unmeasured" to "tie-breaking measured: non-additive
    (report14 evidence pointer)".
  - **H0** (probe kill): D4's kill gate fires before productization.
  - **HALT**: any D5 invariant violated — soundness investigation, not a
    measurement.

## Method

- **Phase 0 — probe.** Temporary env-gated instrumentation on
  `sort_moves`; run the D4 probe; decide the kill gate; revert (`git diff
  --exit-code`) and archive the counter data + probe summary under
  `measurements/plan14/`.
- **Phase 1 — product knob.** `src/search/dfpn/history.rs` (tie-break key
  + sort path), `src/search/dfpn/mod.rs` (`set_seed`), CLI option
  mirroring `--salt`, help text. Tests: seed-0 identity (bit-identical
  trajectory on a sensitive case), per-seed determinism (same seed twice
  = identical; different seeds allowed to diverge), outcome consistency on
  a small seed set, no behavior change with seed absent. Gate:
  `make test`.
- **Phase 2 — rollout.** Driver + parser adapted from plan12's
  (`measurements/plan12/`); D6 matrix; D5 checks; D7 analysis.
- **Phase 3 — verdict + report.** D8 branch; if GO, the methodology
  v1.1 edit; write `report14.md` (final task); update the
  `docs/plans/README.md` research row **only** on GO (methodology
  amendment) or on H0/H1 (caveat resolution worth surfacing).

## Gates

- **GO** → the channel joins the gate; #23 done; next plan per owner
  (plan13 ε arms under the possibly-amended gate, or further channels).
- **H0 / H1** → #23 closed with data; the gate stands as pinned in v1.0.
- **HALT** → D5 violation: investigate soundness before any measurement
  claim.

## Out of scope

- Any lever measurement (#18 initialization, #19 re-scores, #20 restarts,
  #21 in-context child results).
- `selection.rs` ties, history-aging jitter, score noise, benchmark /
  optimizer interface, proof-tree layer, preflight (its soundness contract
  forbids interaction; the tie-break never runs there).
- The `dfpn` reopen and `parallel` items.

## Budget envelope

Phase 2 worst case ≈ plan12-sized: 132 main-matrix runs (≈ 42 min serial;
m20 salt-channel censoring does not apply here — seed 0 at salt 0
reproduces plan12, and seeds {1..5} may solve or censor at the same caps)
plus ≤ 9 × 8 correlation-arm runs on seconds-to-minutes cases; ≤ 1.5 h
wall with 3-way concurrency. Phase 0 is minutes.

## Final task

Write `report14.md` in this directory: deliverables, gate result (GO/H0/
H1/HALT), tie-prevalence numbers, basin/correlation tables, problems
encountered, missing tests, next steps (kickoff prompt for plan13 or the
follow-on).
