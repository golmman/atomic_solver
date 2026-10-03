# Plan 7 — Process-level portfolio racing (sequential racers, take-first decisive)

**Status: DRAFT (plan session, 2026-10-05) — pending owner acceptance; on
acceptance this re-opens `parallel` with a scoped pivot (see Scope
boundary).** Final task: `report7.md`.

## Motivation

The in-search parallelism space is measured out (plan2 option C, lean plan7,
SPDFPN plan5b/plan6 — all NO-GO). This plan tests a *different* mechanism
that shares none of the failing machinery: run **K independent sequential
solver processes concurrently** (one per idle core), each with a
deterministically different trajectory, and take the **first decisive
outcome**. No shared TT, no coordinator perturbation, no work inflation —
soundness is trivial because every racer *is* the unmodified product solver.

The motivating observation is plan6's own data: the shuffle-win t4 run
distribution is **bimodal** (5/8 runs healthy at 20–39 s, 3/8 in a
bad-trajectory tail at 90–100 s with 5.7–6.3× eval inflation). If the
single-run trajectory is path-dependent enough that independent
configurations diverge this way, a take-first portfolio should trim the bad
tail: P(first of 4 finishes early) can beat the single-run median even
though every racer is slower per-core than the 4-thread SPDFPN attempt was
fast. The reference container has 4 CPUs and the product solver is
single-core — 3 cores idle.

**This is not a re-open of the SPDFPN NO-GO.** The no-go record stands
verbatim; this plan tests a usage pattern (shell-level process portfolio),
not an in-search mechanism. No product code changes: the solver binary is
consumed as-is. Deliverables are driver scripts + measurements, per the
measurement conventions in AGENTS.md.

## Scope boundary

- Product solver (`src/`, CLI surface) unchanged. Byte-identity spot checks
  in stage 0 must pass unchanged from the plan6 record.
- All code lives under `docs/plans/parallel/measurements/plan7/` (driver
  `portfolio7.py`, analyzer `analyze_portfolio7.py`, `seq_ref` reuse from
  plan6 where needed).
- One-machine, 4-CPU reality: a 4-racer race monopolizes the container for
  the race's duration. Wall measurements at 4 concurrent single-threaded
  processes are real (plan6 convention: N ≤ 4 real, N ≥ 8 simulated).
- `--refine-cap` / `--epsilon` are documented, sound-for-any-value CLI
  parameters; varying them does not change outcome soundness (verified per
  run by the outcome-agreement gate below).

## Hypothesis under test (pre-registered)

**H (portfolio tail-trim):** for cases whose single-run wall distribution is
long-tailed/bimodal under trajectory-affecting configuration, racing 4
diverse sequential configurations and taking the first decisive outcome
yields a lower *median* wall-to-first-decisive than a single default-config
run, at ≤ 4× CPU cost (accepted: the cores are otherwise idle).

Predictions (falsifiable):

- **H1 (bimodal case benefits):** on shuffle-win, median `W_first` (first
  decisive among the 4 racers) ≤ 0.75× the sequential baseline median, and
  the 8-race max ≤ the 8-baseline-run max.
- **H2 (unimodal cases don't materially regress):** on m22 and rem12,
  median `W_first` ≤ 1.05× the baseline median (small contention/ordering
  tax allowed, no regression beyond noise).
- **H3 (soundness, hard gate):** zero soundness violations — every racer
  that finishes decisively reports the case's known outcome (Win, matching
  plan6's 64/64 record), zero panics, zero cap-hits among racers counted
  separately like plan6.

If H1 fails, portfolio racing is measured NO-GO on its motivating case → the
idea joins the no-go record; if H2 fails, the mechanism is rejected as
non-selective (it only helps where it also hurts). Both failure modes close
the question; on full GO the follow-up decision (ship a tiny
`examples/portfolio` driver vs. document the pattern only) is recorded in
the report, not implemented here.

## Fixed parameters

Sequential references (plan6 record, release build, defaults): m22 3.263 s,
rem12 16.886 s (in-session 17.09 s), shuffle-win 47.83 s. TT 128 MB (RAM =
TT only → 4 racers × 128 MB = 512 MB, fits), ε 0.125, refine-cap 0.25,
`--first-outcome --outcome-only`.

Cases (identical to plan6 stage 3, for direct comparability):

| case | FEN | cap (s) |
| --- | --- | --- |
| m22 | `4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22` | 30 |
| rem12 | `rnbqkbnr/8/6pp/pppppp1B/3PPP2/N5PN/PPP4P/R1BQ1RK1 w kq - 0 10` | 60 |
| shuffle_win | `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21` | 100 |

Racer portfolio (4 configurations; diversity via trajectory-affecting CLI
knobs, chosen before measurement, no post-selection swaps):

| racer | `--refine-cap` | `--epsilon` | rationale |
| --- | --- | --- | --- |
| P0 | 0.25 (default) | 0.125 (default) | the product racer — bounds the portfolio from below by one plain run (modulo contention) |
| P1 | 0 | 0.125 | uncapped refinement rounds → maximally different round structure |
| P2 | 1.0 | 0.125 | loose cap → different truncation points |
| P3 | 0.25 | 0.25 | shifted DF-PN+ threshold → different pn/dn propagation |

Not used as diversity sources: `--config` TOML perturbations (untuned
ordering shifts would confound per-node cost with trajectory diversity —
noted as a stage-2 extension only if H1 fails narrowly), `--tt-size`
(memory contract).

## Stages

1. **Stage 0 — drift gate (cheap).** Release build; re-run m22 (30 s cap)
   and shuffle-win (100 s cap) once with defaults; stdout sha256 must match
   the plan6 stage-0 record (`b7c74f17…`, `64129ef0…`). Any mismatch aborts
   — the measurement runs on the wrong code state.
2. **Stage 1 — driver.** `portfolio7.py`, reusing the `campaign6.py` shape:
   per case, 8 interleaved rounds; each round = (a) one baseline sequential
   default-config run **alone on the machine**, then (b) one race — P0–P3
   launched concurrently, wall recorded per racer, `W_first` = first
   decisive finish. Per run: wall, outcome, child_evals if printed, cap-hit
   flag. One JSON line per run → `state/portfolio7_raw.jsonl`; stderr under
   `logs/` (gitignored).
3. **Stage 2 — pilot (2 rounds, shuffle-win only).** Sanity: 4 concurrent
   processes launch and fit memory, racers terminate ≤ cap, `W_first`
   extraction works, no CPU oversubscription artifacts (nothing else
   running). Caps or harness fixed here if broken; campaign not started
   until the pilot is clean.
4. **Stage 3 — campaign.** 8 rounds × 3 cases (pilot rounds do not count).
   Interleaving order: cases round-robin, baseline-then-race per round.
   Estimated compute: ≈ (48 + 100) × 8 (shuffle-win) + (17 + 60) × 8
   (rem12) + (3 + 30) × 8 (m22) ≈ 30–40 min worst case.
5. **Stage 4 — analysis.** `analyze_portfolio7.py`: per case, baseline
   median/max vs `W_first` median/max, full 8-point distributions (the
   plan5b lesson: report shapes, not only medians), CPU cost = Σ racer
   core-seconds per decisive answer, H1–H3 evaluated mechanically.
   Results → `state/portfolio7_results.json` + `README.md` provenance table
   (drivers, env, results committed; raw transcripts never committed).
6. **Final task — report.** `report7.md`: verdict against the pre-registered
   gate, distributions, cost accounting, deviations; update
   `parallel/initiative.md` (portfolio item row) and the
   `docs/plans/README.md` `parallel` row (re-opened → re-closed NO-GO, or
   re-opened with the documented-pattern follow-up).

## Deviation policy

Any stage-2 harness fix is recorded; racer configs, cases, caps, rep count
and the H1–H3 bands are frozen from this document — no post-hoc adjustment.
If the machine is shared during the campaign (another process visible), the
affected rounds are re-run, not excused.

## Measurement conventions

Per AGENTS.md: commit `README.md` (provenance), `env.json`, `portfolio7.py`,
`analyze_portfolio7.py`, `state/*.json(l)`; never commit `logs/`, `*.out`,
`*.err`. Clean `__pycache__` (`sys.dont_write_bytecode` in drivers) and
verify with `git add --dry-run` before committing.
