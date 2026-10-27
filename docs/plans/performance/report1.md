# Performance Report 1: cheap DF-PN probe gate before the 3-man preflight

Plan: [`plan1.md`](plan1.md) — initiative `performance`, backlog **item 1**
(report §10, "Preflight cost gate / cheap-search probe").
Measurements: [`measurements/plan1/`](measurements/plan1/) (`driver.py`,
`ab_wall.py`, `compare.py`, `state/*.json`, `env.json`).
Date: 2026-10-27. Baseline: pristine `HEAD` `1208411` (built from
`git archive HEAD`).

## Verdict

**GO — all eight pre-registered bars hold.** A bounded DF-PN probe with a
2,000-child-eval cap, run on a scratch `Search`/scratch TT before the
pre-phase, classifies every easy 3-man win in the measured corpus
(1–293 evals) and leaves every draw, hard win, terminal root, and
non-gated root on its previous path. Easy wins go from **~0.78 s / 5.83M
evals** to **~0.011 s / 2–586 evals** (≈76× wall, ≥10,000× child evals);
draws/hard wins pay **+2,000–2,019 evals (≤ 0.087%)** and **≤ +2.5% wall**.

## D-a. Calibrated cap and margin

`CHEAP_PROBE_EVAL_CAP = 2_000`, `CHEAP_PROBE_TT_MB = 4`,
`CHEAP_PROBE_TIMEOUT_SECS = 1` — unchanged from the plan. Phase 0
(`state/probe_phase0.json`, the `--tt-size 4 --budget 2000 --no-preflight`
stand-in) reproduced the plan's table on the current binary and extended it
with every preflight fixture:

| class | case | probe result | evals |
|---|---|---:|
| easy-win | `easy_qf1` KQ, Qf1 | win in 3 | 189 |
| easy-win | `easy_qd1` KQ, Qd1 | win in 3 | 193 |
| easy-win | `easy_qa1` KQ, Qa1 | win in 3 | 293 |
| easy-win | `easy_qd8` KQ+Qd8 vs Ke8 | win in 1 | 1 |
| hard-win | `hard_ladder_root` (win in 15) | draw (budget) | 2,002 |
| hard-win | `hard_ladder_step3` (win in 9) | draw (budget) | 2,003 |
| draw | `draw_kr` / `draw_kn` / `draw_kb` | draw (budget) | 2,000 / 2,006 / 2,003 |
| draw | `draw_cyclic_rook` | draw (budget) | 2,019 |
| draw | `draw_adjacency` | draw (budget) | 2,000 |
| terminal | mate / stalemate / extinction | decided by the search itself | 0 |

**Separating margin: 293 → 2,000 (6.8×).** No draw or hard case finished
inside the cap, so the plan's pre-registered lowering to 1,000 was not
triggered (it would be safe — 293 × 3.4 — but adds risk to unmeasured easy
classes for no measurable benefit: the probe's Draw cost is 0.04% of a
closure). The cap is kept at the pre-registered 2,000.

## Full corpus table, before → after

Production configuration `--tt-size 256 --timeout 2 --first-outcome`;
walls are the **interleaved** A/B medians (15 reps/arm, `state/ab_wall.json`),
evals are deterministic values from `state/{baseline,after}.json`.

| case | class | base wall | after wall | ratio | base evals | after evals | delta |
|---|---|---:|---:|---:|---:|---:|---:|
| easy_qf1 | easy-win | 0.7965 | 0.0105 | 0.013 | 5,827,506 | 378 | −5,827,128 |
| easy_qd1 | easy-win | 0.7786 | 0.0109 | 0.014 | 5,827,510 | 386 | −5,827,124 |
| easy_qa1 | easy-win | 0.7603 | 0.0111 | 0.015 | 5,827,518 | 586 | −5,826,932 |
| easy_qd8 | easy-win | 0.7479 | 0.0102 | 0.014 | 5,827,491 | 2 | −5,827,489 |
| hard_ladder_root | hard-win | 0.7843 | 0.7751 | 0.988 | 5,866,690 | 5,868,692 | +2,002 (0.034%) |
| hard_ladder_step3 | hard-win | 0.8086 | 0.8257 | 1.021 | 5,827,778 | 5,829,781 | +2,003 (0.034%) |
| draw_kr | draw | 0.6057 | 0.6064 | 1.001 | 5,217,752 | 5,219,752 | +2,000 (0.038%) |
| draw_kn | draw | 0.5705 | 0.5845 | 1.025 | 4,109,608 | 4,111,614 | +2,006 (0.049%) |
| draw_kb | draw | 0.2824 | 0.2878 | 1.019 | 2,299,948 | 2,301,951 | +2,003 (0.087%) |
| draw_cyclic_rook | draw | 0.5691 | 0.5748 | 1.010 | 5,217,752 | 5,219,771 | +2,019 (0.039%) |
| draw_adjacency | draw | 0.7664 | 0.7685 | 1.003 | 5,827,440 | 5,829,440 | +2,000 (0.034%) |
| term_mate | terminal | 0.0091 | 0.0087 | 0.956 | 0 | 0 | 0 |
| term_stalemate | terminal | 0.0090 | 0.0078 | 0.867 | 0 | 0 | 0 |
| term_extinct | terminal | 0.0092 | 0.0084 | 0.913 | 0 | 0 | 0 |
| control_4men | detector-miss | 0.0085 | 0.0084 | 0.988 | 30 | 30 | 0 |

Outcomes and PV lengths are identical everywhere; the only status changes are
the intended easy-win ones (`preflight-proof` → the ordinary search's
`first-outcome` / `proven-shortest`):

| case | base | after |
|---|---|---|
| easy_qf1/qd1/qa1 | `decided outcome=win … rank=3`, `pv_status: preflight-proof` | `deferred reason=probe-solved evals=189/193/293`, `pv_status: first-outcome` |
| easy_qd8 | `decided outcome=win region=420534 rank=1` | `deferred reason=probe-solved evals=1`, `pv_status: proven-shortest` |
| draws / hard wins | `decided … region>0` | `decided … region>0`, `evals` + probe |
| terminal roots | `decided … region=1` | `decided … region=1 evals=0` (probe skipped) |

Peak RSS (`ru_maxrss` of the child, `state/*.json`): easy wins
270,700 → 235,808 KB (the ~35 MB closure allocation is skipped); draw cases
unchanged within ±3 MB (251,580 → 254,484 KB worst case), as expected from a
transient 4 MB scratch table.

## Easy-win latency distribution

After-change walls, interleaved arm (`state/ab_wall.json`), 15 reps per case:
`easy_qf1` median 0.0105 s (min 0.0094, max 0.0227), `easy_qd1` 0.0109,
`easy_qa1` 0.0111, `easy_qd8` 0.0102. All are ~10× below the 50 ms bar and
~72–80× faster than the baseline's ~0.73–0.87 s. The remaining ~9–10 ms is
dominated by the 256 MB TT allocation that the CLI does before any search
(report §4 item), not by the probe (1–293 evals, < 1 ms).

## Draw/hard extra-eval cost

The probe's deferred cost is its own sweep-exhausted child-eval count:
2,000 (`draw_kr`, `draw_adjacency`), 2,006 (`draw_kn`), 2,003 (`draw_kb`),
2,019 (`draw_cyclic_rook`), 2,002 (`hard_ladder_root`), 2,003
(`hard_ladder_step3`) — i.e. the cap plus at most 19 evals of sweep
overshoot. Against the closures (2.30M–5.87M evals) that is **0.034–0.087%**,
inside the ≤ 0.1% bar. Interleaved wall ratios on the same cases: 1.001,
1.003, 1.010, 1.019, 1.025, 1.021, 0.988 → all within the +5% bar (and the
worst, `draw_kn` +2.5%, is at the measurement floor: +2,006 evals is
~0.2 ms of closure time).

## Budget/accounting evidence

- `gate.evals` is added to `self.child_evals` immediately after the probe, so
  the documented budget contract is unchanged. Observed cases:
  - budget 1,000 on the ladder root: probe consumes the budget
    (1,026 evals incl. overshoot); the pre-phase then runs with a zero
    remaining budget and defers `eval-budget`; `evals: 1026`,
    `budget exhausted`, `exit_reason: BudgetExhausted` — never `Timeout`.
  - budget 10,000: probe 2,000 + closure 8,000 = `evals: 10000`,
    `preflight: deferred reason=eval-budget evals=10000`.
  - budget 0: the gate is skipped (`remaining == 0`), the pre-phase aborts
    with `eval-budget`, and `child_eval_budget_zero_causes_immediate_exit`
    still passes.
- `PreflightReport::evals` now means the run's whole pre-phase cost (probe +
  closure/certificate). It is folded in through `preflight_phase(...,
  prior_evals)`; `search_depth` passes `0` (that entry point runs no probe, so
  the certificate-bound and reconstruct behaviors are untouched). The
  `preflight_eval_budget_defers_then_budget_contract_holds` invariant
  `child_evaluations() == report.evals` still holds, because the probe evals
  are part of the reported pre-phase cost.
- Determinism: the probe reuses the production epsilon, scorer,
  `max_ply`, restart schedule and TT salt; the search is deterministic for a
  fixed `(FEN, config, cap)`. The 1 s timeout is a safety net that never fired
  in any measured run (the cap binds first in every drawing case; the easy
  cases finish in < 300 evals).

## TT isolation

- Fast unit test `cheap_probe_gate_defers_on_draw_and_leaves_production_tt_untouched`
  (dfpn): a deferred probe on the KR draw leaves `tt_stats()` **bit-identical**
  and `child_evaluations() == 0` (the gate only *reports* its cost; the caller
  accounts it).
- Pre-phase claims still store nothing: `preflight_certifies_ladder_root_win`
  (`live == 0, solved == 0`) and the terminal-claim unit test pass unchanged.
- The probe cannot emit `ProofEvent`s: it is a fresh `Search` with no sender,
  so the documented R2 gap is not widened. `preflight_claim_emits_no_proof_events_r2_gap`
  still passes.
- The ordinary search after a `probe-solved` deferral starts from the same
  empty production TT (verified by evals: probe 189 + ordinary 189 = 378 for
  `easy_qf1`, i.e. the ordinary search costs exactly what the probe did).

## Test and gate results

- New fast unit tests (`src/search/dfpn/tests.rs`): gate reports decisive on
  an easy root; deferred gate on a draw leaves the production TT untouched
  and respects the cap (+ sweep overshoot); the gate is skipped on terminal
  roots, detector misses, `set_preflight_enabled(false)`, and a zero budget.
- New fast unit tests (`src/search/preflight/tests.rs`): probe-solved
  end-to-end report (`reason == "probe-solved"`, `region == 0`,
  `child_evaluations() > report.evals`, non-`PreflightProof` status);
  `--no-preflight` disables the probe (`reason == "disabled"`, fewer evals
  than the enabled run); terminal claims report `evals == 0` (probe skipped);
  the adjacency-draw full-closure claim still decides with the probe's evals
  folded into `report.evals`.
- **Changed test expectation (one, pre-authorized class):**
  `tests/test_cli.rs::cli_prints_pv_status_preflight_proof_for_mate_in_one`
  → `cli_easy_three_man_root_is_probe_solved`. The mate-in-1 fixture
  (`4k3/8/8/8/8/8/8/4R1K1 w`) is a non-terminal 3-man easy position, which is
  exactly the class whose `pv_status` the plan pre-authorized changing: it now
  prints `pv_status: proven-shortest` (the ordinary search) and, in non
  `--outcome-only` mode, `preflight: deferred reason=probe-solved evals=<n>`.
  `preflight-proof` CLI coverage is retained by
  `cli_no_preflight_disables_prephase_on_gated_position` (the ladder root, a
  hard win, still claims).
- **No** draw/ladder/terminal expectation was weakened; all full-closure tests
  run unchanged.
- `make test` green; `make test-full` green (36 test binaries, including the
  60 s regression/stress suites and the ignored full-closure preflight
  tests, 7m22s wall). `cargo fmt --check` and `cargo clippy --all-targets`
  clean.
- Quick suite (`benchmark --suite quick --first-outcome --timeout 3 --json`,
  `state/quick_{base,after}.json`): **eval-identical, 59/59 cases**, 0
  timeouts, 0 wrong, total child evals 42,109,768 in both arms (the suite has
  no ≤3-men non-terminal 3-man case; the probe never fires there).

## GO/NO-GO against the eight bars

| # | bar | result |
|---|---|---|
| 1 | easy-win cases classified `probe-solved`, same decisive outcome | **pass** — 4/4 `probe-solved`, all still `win` with unchanged PV length |
| 2 | draw cases keep a `decided` (Draw) claim with `region > 0`; never a timeout | **pass** — 5/5 decided draws, regions 234,130–481,520, no timeouts |
| 3 | hard-win cases keep the replay-verified claim with unchanged outcome/rank/PV length | **pass** — rank 15 / len 15 and rank 9 / len 9, both `preflight-proof` |
| 4 | terminal roots take the region path (`region == 1`), not the probe | **pass** — 3/3 `region=1`, `evals=0` |
| 5 | easy-wall ≤ 50 ms median and ≥10× faster; draw wall within +5% | **pass** — 10.2–11.1 ms (≈72–80×); draw ratios 1.001–1.025 |
| 6 | extra evals on draw/hard ≤ 0.1% of closure cost (cap ≤ 2,019) | **pass** — +2,000…+2,019 evals, 0.034–0.087% |
| 7 | production TT untouched by a deferred probe | **pass** — `tt_stats()` unchanged unit test; ordinary search costs exactly its own work |
| 8 | `make test` + `make test-full` green; quick suite eval-identical | **pass** — both green; 59/59 identical, 0 wrong |

## Implementation notes (what landed)

- `src/search/dfpn/cheap_probe.rs` — constants, `ProbeGate`, and
  `Search::cheap_probe_gate` (scratch `Search` + 4 MB scratch TT, dropped
  inside the gate).
- `src/search/dfpn/run.rs` — `solve_with_progress` calls the gate after
  `begin_run`, accounts `gate.evals`, reports `probe-solved` and skips
  `preflight_phase` when the probe decided; `preflight_phase` gained
  `prior_evals`, folded into every report; `search_depth` passes `0`.
- `src/search/preflight/mod.rs` — `probe-solved` added to the documented
  deferral reasons, `evals` documented as probe + closure work, module header
  notes the probe's scratch TT and that the no-TT contract is untouched.
- `src/main.rs` — `--no-preflight` documented as disabling the gate too;
  output contract documents the `probe-solved` line and the resulting
  `pv_status`.
- `AGENTS.md` — one clause added to the preflight bullet.

## Findings and follow-ups

- The easy-win *floor* is now the 256 MB TT allocation (~9 ms) plus process
  start, not the search (report §4 item 5, argv backlog #5 "lazy/zero-init TT
  allocation"). That is the next lever for short-solve latency and is already
  in this initiative's backlog.
- The cap is a screen, so a future change to the probe's budget interplay
  (e.g. an overall budget < 2,000) is handled by
  `min(CHEAP_PROBE_EVAL_CAP, remaining)`; the probe then consumes the whole
  remaining budget and the pre-phase defers `eval-budget`. That is intended
  (budget-mode runs are censored by design), but it means a small `--budget`
  gives up the closure entirely — unchanged from the pre-plan behavior, which
  also spent the budget in the closure first.
- `search_depth` (offline/reconstruct path) deliberately runs no probe; if a
  certificate-bound workflow ever wants the screen, it is a one-line change
  with `prior_evals` already in place.
