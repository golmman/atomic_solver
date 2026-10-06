# Lean Report 10 — #10 history/killer constant re-tune (env-gated sweep)

Sweep executed 2026-10-06 per `plan10.md`. **No `src/` changes landed**; the
temporary `LEAN10_*` env-gated constant overrides and the `LEAN10_STATS`
diagnostic were applied on top of HEAD, measured, and fully reverted —
`src/` is byte-identical to the pre-spike tree (`git diff` empty) and the
post-revert m22 first-outcome stdout md5 is `ea72f7ea…`, matching the
step-0 HEAD capture and the report8/report9 anchors. `make test` green.
Artifacts under `measurements/plan10/` (`arms.csv` is the parsed record;
raw transcripts deleted per AGENTS.md).

## Decision

**Backlog #10 (history/killer constant re-tune): NO-GO, closed.** No
finalist passes the pre-registered gates — specifically **gate 2** (no
quick case regresses > +10% `child_evals`, zero outcome flips, zero new
timeout-status flips) fails for every arm with a material aggregate win,
and even the best flip-free arm fails it. The report below records the
measured constant sensitivity as decision evidence.

**With #10 closed, the `lean` backlog is empty** — dormancy implication at
the end of this report.

## Non-perturbation gate (step 0 → spike binary)

With no env vars set, the spike binary reproduced the HEAD behavior
exactly: m22 first-outcome stdout md5 `ea72f7eae2437867c986dbf71268223f`
(byte-identical) and the quick-suite JSON with all 59 cases identical in
`nodes`/`child_evals`/outcomes (total 38,974,090; only wall-time fields
differ). An earlier identical result was a stale-example-binary artifact —
`benchmark` must be rebuilt with `cargo build --release --examples` after
lib changes; the hook was then verified live via extreme probes (`k0`,
`hb1`, `ag1` all move the totals far off baseline).

## Phase 0 — baselines and diagnostic

- Quick FO baseline (`--timeout 3`): **38,974,090** child evals, 59/59 ok,
  0 timeouts, 0 wrong. Thorough FO baseline (`--timeout 5`): 60 ok + 5
  timeout cases (m20_white/m20_black/m21_white/m21_black/m22_black).
- m22 first-outcome anchor: md5 `ea72f7ea…` (matches report8/plan9).
- **Optional diagnostic (`LEAN10_STATS=1`, quick suite)**: the
  history+killer layer changes the static-only top-1 pick in
  **482,956 / 2,477,336 `sort_moves` calls = 19.5%** — far from the ~0
  kill-switch threshold, so the full phase-1 screen ran. The surface is
  *active* (history/killers do reorder picks constantly); it is the
  *outcome* of re-tuning that is harmful.

## Phase 1 — OFAT screen (quick FO, total child evals vs 38,974,090)

| axis | arm | total | Δ | ok/timeouts/wrong | worst >+10% regression | gate 2 |
| --- | --- | --- | --- | --- | --- | --- |
| hist scale (BONUS+MAX) | 0.5× (50/5000) | 37,886,759 | −2.79% | 59/0/0 | dec44 5.19×, dec05 3.90× | **fail** |
| hist scale | 2× (200/20000) | 37,841,090 | −2.91% | 59/0/0 | dec31 2.80× | **fail** |
| killer | 10,000 | 39,124,984 | +0.39% | 59/0/0 | dec14 1.48× | fail |
| killer | 200,000 | 38,974,090 | ±0.00% | 59/0/0 | — (byte-identical) | n/a |
| aging | 2,500 | 44,990,871 | +15.44% | 59/0/0 | dec14 1.57× | fail |
| aging | 50,000 | 45,990,478 | +18.00% | 59/0/0 | dec01 1.95× | fail |
| cap ratio | 50 (MAX 5000) | 41,475,429 | +6.42% | 59/0/0 | m23_white 1.32× | fail |
| cap ratio | 200 (MAX 20000) | 39,890,425 | +2.35% | 59/0/0 | dec13 1.19× | fail |

Additional observations from the hook probes and a finer aging sweep
(refinement, phase 0/2 labels in `arms.csv`):

- `SCORE_KILLER=0` (+9.2%) is far worse — the killer bonus is genuinely
  load-bearing; `200_000` is byte-identical to 50,000 (no tie-adjacent
  reorderings at suite scale). The killer axis is effectively a dead
  degree of freedom between 50k and 200k.
- **The aging axis is violently non-monotonic**: age-every-frame (1) =
  −19.2%, 5 = −1.9%, 10 = −18.6% (1 timeout), 100 = −1.8%, 500 = −24.0%
  (1 timeout), 1000 = +8.9%, 2000 = −5.8% (2 timeouts), 2500 = +15.4%,
  10000 (default) = 0, 50000 = +18.0%.

## Phase 2 — cross-product (hist scale × aging)

| combo | total | Δ | timeouts | gate 2 |
| --- | --- | --- | --- | --- |
| hs2-ag1 (BONUS 200, MAX 20000, age 1) | 31,345,410 | −19.57% | 1 (m23_white ok→timeout) | **fail** |
| hs2-ag5 | 31,303,032 | −19.68% | 1 (dec01 ok→timeout) | **fail** |
| hs05-ag1 | 34,631,157 | −11.14% | 1 (m23_white ok→timeout) | fail |
| hs05-ag5 | 39,547,250 | +1.47% | 0 | fail (dec05 5.19×) |
| hs05-ag100 | 37,679,579 | −3.32% | 0 | **fail** (dec14 1.59×, m24_white 1.33×) |
| hs2-ag100 | 40,719,587 | +4.48% | 0 | fail (dec05 3.23×) |

## Why the no-go is real, not a missed tuning sweet spot

1. **The wins are trajectory luck, not ordering quality.** The strong
   aggregate winners (−11% to −20%) all do it while flipping a
   previously-solving suite case into a 3 s timeout (`m23_white`: 9.67 M →
   >12.3 M evals, `dec01`: 5.71 M → >16.2 M) — exactly the
   "trajectory chaos between neighboring constants" failure mode gate 1's
   3% bar was designed to filter out. The neighboring constants of each
   winner swing +15%/+18% or introduce timeouts (see the aging series).
2. **The tail cases that gate 2 protects blow up in almost every arm.**
   `dec05` (5.19–5.38×), `dec14` (1.6×), `dec44` (5.19× under hs05),
   `m24_white` (1.32–1.33×) regress across the board. These are the
   deep/repetition-heavy cases where DF-PN threshold lattices are
   sensitive to move ordering churn — the same backfire class the plan's
   stress-case gate anticipated. Only *one* arm of 23 measured is
   flip-free with a >3% aggregate win (hs05-ag100, −3.3%), and it still
   regresses dec14 by 59%.
3. **Gate 2 is the right gate and it is unambiguous**: every arm violates
   it; there is no finalist to advance to the thorough/stress validation,
   so gates 3–5 never came into play.

Measured sensitivity summary (the decision evidence the plan requires):
quick-suite totals across all 23 arms span 29.6 M–46.0 M (−24% to +18%),
with per-case regressions >10% in 22 of 23 arms and outcome flips in 5 —
the constants are *impactful* (the 19.5% reorder fraction confirms) but
*chaotically* coupled; no neighboring-constant robustness exists, which is
what a hardcodable, durable re-tune would need.

## F5 — "side-aware killers" retired by analysis (no code needed)

`sort_moves` and `update_killers` both key on `depth = path_stack.len()`,
the ply index from the root. The root position is fixed for the lifetime
of the search (AGENTS.md), there are no passes or null moves, so
side-to-move parity is a pure function of ply: all frames at a given
depth have the same side to move. The depth-indexed killer table is
therefore *already implicitly side-aware*, and the history table is
explicitly side-indexed (`[[i32; 64]; 64; 2]`). Doubling the killer table
to `[side][depth]` indexing would be a structural no-op. The backlog
item's second mechanism is retired on this argument; the constants sweep
above was the entire remaining lever.

## Landing / revert

- Spike code (env overrides in `Search::new`, parameterized
  `update_history_entry`/`age_history`/`killer_bonus`, `LEAN10_STATS`
  counters, `main.rs` stderr hook) fully reverted; `git diff -- src/`
  empty; constants unchanged (`history.rs`: BONUS 100, MAX 10_000,
  AGE_INTERVAL 10_000, SCORE_KILLER 50_000).
- Post-revert: m22 first-outcome stdout md5 `ea72f7ea…` (byte-identical to
  the step-0 anchor); `make test` green (all suites, 0 failures).
  `make test-full` not required — `src/` is byte-identical to HEAD.

## Method notes and caveats

- **Stale example binary trap**: `cargo build --release` does not rebuild
  `examples/`; the first (all-identical) arm sweep ran a pre-spike
  `benchmark`. Re-ran everything after `cargo build --release
  --examples`. The committed `arms.csv` contains only post-rebuild runs.
- Arms are single deterministic runs (deterministic search ⇒ no run-to-run
  noise); the "noise" the gates guard against is *neighboring-constant*
  chaos, not run variance — hence gate 1's 3% bar and gate 2's per-case
  protection.
- The thorough baseline's 5 timeout cases are the m20/m21/m22-class hard
  positions (expected at `--timeout 5`); no finalist reached thorough
  validation because no finalist passed gate 2 on quick.
- The stress FO case was not re-baselined (its re-capture mattered only
  for gate 5 of an adopt; no finalist advanced).

## Follow-ups

- **Backlog #10 is closed** in `initiative.md`; the `lean` backlog table
  has no open items left.
- **Dormancy implication**: every ordering-adjacent item in the backlog is
  now measured-closed (#5 plan9, #10 plan10) and the nn-oracle-floor
  record bounds anything that only reorders the winning OR child. The
  remaining levers are structural: the `dfpn` algorithmic items
  (threshold dynamics own 99.7–99.9% of AND work per plan9) and the parked
  `parallel` initiative. The `lean` initiative itself has no further
  measurable in-scope lever at HEAD; a fresh profile should either spawn a
  new initiative or let this one go dormant.
- `tests/fixtures/move_order_positions.txt`-class regressions under
  constant changes (dec05/dec14 blow-ups) are worth a note for any future
  ordering proposal: a suite-total win without per-case gates is not
  adoptable evidence.
