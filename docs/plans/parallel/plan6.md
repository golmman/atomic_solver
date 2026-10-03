# Plan 6 — SPDFPN stage-2 follow-up: length-threshold experiment, re-scoped gate, ship/close decision

**Owner-directed re-measurement (2026-10-04, this conversation).** After
plan5b's NO-GO (`report5.md`) and the executed condition-1 revert, the owner
reviewed the numbers and directed one more measurement round before a final
decision, on the observation that **shuffle-win is promising (S4 = 2.68×,
I4 = 1.41×) while the very short m22 case (S4 = 1.79×, I4 = 2.06×) is a weak
5-rep median**. This plan executes that directive. It is a *new
pre-registration*: the plan5b bands are closed history, not renegotiated
here — the gate below is re-registered for the ship shape this plan tests.

Executes `parallel` backlog **#4, stage-2 follow-up**: on GO the backlog row
closes **won with a scoped ship shape**; on NO-GO the initiative closes on
the strengthened no-go record. Final task: `report6.md`.

Prerequisite reading: `report5.md` (stage-2 verdict + revert + the re-open
levers), `report5a.md` (mechanism + deviations), `plan5b.md` (the campaign
protocol this plan reuses), `initiative.md` (owner conditions, measurement
conventions).

## Hypothesis under test (pre-registered)

**H (length threshold):** the parallel mode's benefit increases with
sequential solve length, because helper TT pre-warm amortizes over total
work while the coordinator-perturbation damage scales with per-node
decisions. Consequence: there exists a length above which `--threads N` is
a net win — and the honest ship shape is an **opt-in mode documented for
long solves** (the mode is per-invocation; the product's priority-1 class is
deep solves), not an unconditional parallel mode.

Predictions (falsifiable):
- H1: shuffle-win's plan5b numbers reproduce at higher rep count
  (S4 ≥ 2.0×, I4 ≤ 1.5× medians).
- H2: a mid-length case (~15–25 s sequential) lands *between* m22 and
  shuffle-win on both S4 and I4 (monotone length trend, Spearman ρ > 0.8
  over the case points).
- H3: m22 at 8 reps does not regress materially (S4 median ≥ 1.5×) — short
  solves stay usable, just not recommended.

If H1 fails, the shuffle-win promise was a 5-rep artifact → NO-GO.
If H1 holds but H2 shows no trend, the mechanism difference is
case-specific, not length-driven → owner decision with the full data.
If H1 + H2 hold → ship-shape gate below.

## Stage 0 — re-land (mechanical, no design changes)

1. Re-apply the stage-2 mechanism from history: commit `b714ef6` content +
   the plan5b W delta (`MAX_WORK_PER_JOB = 10_000` with its sweep doc
   comment). Files per `report5.md`'s revert inventory, in reverse. No
   mechanism changes in this plan (the eviction-discount lever stays a
   follow-up option — measurement first).
2. Verify the N = 1 surface before any parallel run: quick suite 59/59
   `child_evals` vs `baseline_quick_pre5.json`, m22/shuffle-win stdout
   hashes (`b7c74f17…` / `64129ef0…`), snapshot sha `eaa5f2b9…`/195 B,
   `make test` green.
3. Record the re-open in `initiative.md` (backlog #4 row: re-opened by
   owner directive, this plan) and the `docs/plans/README.md` `parallel`
   row (re-open event).

## Stage 1 — case selection (locked before any parallel run)

Goal: one mid-length point between m22 (3.3 s) and shuffle-win (58.2 s) to
test the trend. Selection criterion: **sequential first-outcome wall in
[10, 30] s** at product defaults (128 MB TT, ε 0.125, refine-cap 0.25),
Win-class, from the existing position pool.

- Candidate scan (sequential runs only, ≤ 60 s cap each): the hard/very-hard
  Win cases in `tests/fixtures/move_order_positions.txt` (m23_white ≈ 2.5 s
  — too short; m20_white ≈ 100+ s — too long), plus Win-class FENs in
  `tests/test_deep_outcomes.rs` and the `docs/plans/*` measurement records
  (conversion `dec*` cases are 1.5–5 s class — likely too short).
- Known gap: the move-order fixture is bimodal (≤ 3 s or ≥ 100 s). If no
  candidate lands in [10, 30] s, substitute **m20_white as an ultra-long
  point** (`--timeout 200`, ~5 reps — expected ≈ 100–150 s sequential) and
  test the trend as short vs long vs ultra-long.
- The selected case(s) + their sequential reference walls/evals are recorded
  in `measurements/plan6/README.md` **before** the campaign starts. No
  post-selection swaps.

## Stage 2 — W confirmation on the long class (then locked)

The plan5b sweep ran on m22 only; W was never swept on a long solve. On
shuffle-win, t4: W ∈ {4000, 10000, 20000}, 3 reps each, interleaved with
N = 1 references. Keep 10 000 if it wins the median; otherwise lock the
winner and note that all plan6 campaign numbers use the new W (plan5b
comparability caveat goes in the report). No other tuning, ever.

## Stage 3 — campaign (the plan5b protocol, tightened)

- Cases: shuffle-win (long, `--timeout 100`), selected mid case, m22
  (short, `--timeout 30`, control — measurement only, no gate).
- N ∈ {2, 3, 4} real; **≥ 8 interleaved reps** per (case, N), each round
  running N = 1, 2, 3, 4 back-to-back (N = 1 = in-session denominator).
  m20_white (if used): 5 reps, `--timeout 200`.
- Metrics per (case, N): median wall + speedup (primary), work inflation
  (median total evals / sequential evals from an in-session reference run),
  outcome agreement (decisive runs must equal the sequential outcome),
  **cap-hit rate** (first-class gate item now), per-thread work split,
  panics/lock anomalies (zero expected). Sequential eval references via
  `benchmark --suite move-order --timeout 200 --json` (in-session) or
  per-case equivalent.
- N ∈ {8, 16}: log-fit extrapolations only, clearly labeled, non-gating.
- Session-budget note: a shuffle-win round ≈ 2.5–4 min → 8 rounds ≈ 25–35
  min; mid case ≈ 8 min; m22 ≈ 3 min; W confirmation ≈ 10–15 min. Fits one
  session; if the ultra-long fallback fires, drop its reps to 5 (planned).

## Pre-registered gate (ship shape: opt-in `--threads N`, documented "long solves only", never default)

Let S4(c), I4(c) = medians at 4 threads on case c; V = decisive
disagreements; C = worst per-(case, N) cap-hit rate over gated cases.

- **GO** (all): S4(shuffle-win) ≥ 2.0× **and** I4(shuffle-win) ≤ 1.5×
  **and** V = 0 **and** C ≤ 1/8 **and** mid case S4 ≥ 1.5× with I4 ≤ 2×.
  → Mechanism stays landed; ship shape documented (CLI help + module docs:
  recommended for long solves, keep N = 1 for short ones); backlog #4
  closes **won (scoped)**; owner sign-off recorded in `report6.md`.
- **MARGINAL**: H1 holds but one of the other GO conjuncts misses → owner
  decision with both speedup denominators (vs N = 1 and vs unsharded),
  inflation, cap-hit rates, and the length trend.
- **NO-GO**: H1 fails (shuffle-win S4 < 2.0× or I4 > 1.5× at 8 reps), or
  V ≠ 0 (any false decisive outcome — immediate), or C > 1/8 on a gated
  case, or mid case I4 > 2×. → Revert stage 0 (the same mechanical
  procedure as `report5.md`; N = 1 verification identical) and close the
  initiative on the strengthened record.
- Byte-identity surface re-verified at the end of the session regardless of
  verdict (quick suite, stdout hashes, snapshot sha) — the N = 1 gate never
  relaxes.

## Non-goals

- No mechanism changes beyond the stage-0 re-land and the stage-2 W lock
  (eviction discount / helper caps / chunk steering are follow-up plans,
  only if the owner opens one after this plan's data).
- No default-on parallelism; no proof-tree/pipeline work.
- No wall claims from N ≥ 8; no re-tuning after the campaign starts.
- No new benchmarks beyond the sequential reference runs the case selection
  needs.

## Risks

- **Session budget** (long rounds dominate): rep counts above are sized to
  fit; if over budget, cut m22 reps first (control), never the gated cells.
- **Case-selection gap**: if no mid case exists, the trend rests on 2–3
  points of very different lengths — still decidable, but the report must
  state the weaker trend evidence.
- **Cap-hits concentrate in gated cells**: plan5b saw 2/30 (t2/t3 only);
  the C ≤ 1/8 gate makes the tail risk explicit instead of median-hidden.
- **W re-lock breaks plan5b comparability**: reported as a caveat; the
  gate is self-contained within plan6's own references.

## Deliverables

- `measurements/plan6/` — README (provenance + locked case table),
  `env.json`, drivers (`*.py`), `state/*.json|csv`; logs gitignored.
- `initiative.md` — backlog #4 row (re-opened → final state);
  `docs/plans/README.md` `parallel` row (re-open event at stage 0, final
  state at closure).
- **`report6.md` in this directory (final task)**: H1–H3 results, campaign
  medians, verdict against the gate above, decision executed
  (scoped ship / owner escalation / revert + closure), problems,
  unresolved parts, next steps.
