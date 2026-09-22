# Plan 2 Report — Design-space note + option-C sizing spike (architecture selection)

Executed 2026-09-21 per `plan2.md`. **No `src/` or `examples/` change**:
the spike drove the existing release binary as a black box; `git status`
shows only `docs/bibliography.md`, `docs/plans/README.md`, and new files
under `docs/plans/parallel/`. A post-spike m22 first-outcome capture is
byte-identical across reruns (`measurements/plan2/m22_drift_capture_*.txt`)
— the spike left no residue (nothing in the build changed, and the run
proves it).

## Decision

**NO-GO** on the pre-registered Phase 2 rules (m22, N=4, median of 5
runs). Consequences:

1. **No opt-in nondeterministic parallel mode ships** (lean trigger 1
   was never satisfied; the spike removed the last economic argument).
2. **Ship shape (recorded, moot):** if anything ever ships, it is the
   `examples/` harness over unmodified solver processes (option C);
   a CLI `--threads N` (option A) stays gated as before. Neither ships now.
3. **Backlog #2 closed** as measured no-go; **backlog #3 closed**
   (unblocking condition failed). The initiative moves toward closure;
   `docs/plans/README.md` row updated (pivot/close event).
4. The Pawlewicz & Hayward 2014 bibliography entry was added as
   **Open** and stays open — mining is only owed if a future A-stage
   is ever justified.

## Measured numbers (m22 unless stated; raw data + command table in `measurements/plan2/`)

Sequential root baselines (`--first-outcome`, defaults): m22 win,
**2.63 s / 858,117 nodes**, 95-ply informational PV; shuffle-win win,
**47.83 s / 13,907,467 nodes**, 477-ply PV.

Root-child race over process-per-child workers (cap 120 s per child,
first proven child stops all; winning child `a2a4` is 2nd in
enumeration order, so these races are already at the most favorable
scheduling shape):

| config | median wall | speedup | agg worker nodes | inflation |
|---|---|---|---|---|
| sequential root | 2.63 s | 1.00× | 858 k | 1.0× |
| race N=2 (5 reps) | 9.05 s | 0.29× | 9.93 M | 11.6× |
| race N=3 (5 reps) | 9.38 s | 0.28× | 11.64 M | 13.6× |
| race N=4 (5 reps) | 9.75 s | **0.27×** | 13.33 M | **15.5×** |
| race N=4, winner-first order (3 reps) | 9.56 s | 0.28× | 21.52 M | 25.1× |
| **simulation** N=8 / N=16 | 8.704 s | 0.30× | 88 M / 179 M | 103× / 208× |

Agreement/soundness: **zero decisive disagreements and zero false
proven roots** in all 18 timed race runs; the proven move set was
identical in every rep (deterministic solver; nondeterminism touches
only kill timing). Perspective mapping validated against
`find_winning_child` with exact agreement on two shallow decisive-suite
FENs (dec08: `f4e5`, dec03: `b5d3`). Shuffle-win probe: the N=4 race
consumed all 41 children in 1320 s with **no proven child** while the
sequential root proves in 47.8 s. RAM: peak RSS ≈ **230 MB per worker**
at the default 128 MB TT (≈1.8× table size) → practical max N ≈ 35
against the container's enforced 8 GiB cgroup limit.

## The finding that closes the door

The deficit is **structural, not a tuning or scheduling problem**:

- The sequential root proves the win in 858 k nodes **total**; the
  winning child (`a2a4`) needs 8.59 M nodes **in isolation** — a ≈10×
  subsidy from the root search's cross-child TT transposition sharing
  and best-first proof interleaving across children.
- No scheduler can beat a worker's own isolated cost: even an oracle
  schedule (winner first) has a floor of 8.7 s = 0.30×. The N=8/16
  simulations are N- and order-insensitive for exactly this reason.
- `g4h5` — the first move of the root's informational PV — does not
  prove in isolation at all within 120 s (98 M nodes): root-level work
  distribution is not decomposable into per-child jobs without paying
  the isolation cost.
- More workers only add abandoned-children partial work: inflation
  grows monotonically with N (11.6× → 15.5× → 208× simulated).

This is the quantitative confirmation of the initiative's own fear
("work-wasteful vs the sequential early-exit for single-root solving")
and of lean report7's OR-early-exit tail risk — but the tail turned out
to be the *smaller* half of the problem; the floor is the bigger one.

## Discrepancies vs the published bands

- The literature band for job-level parallel PNS at small pools is
  near-linear (Čížek 2025: 2.4×/2, 4.3×/4, 16.2×/16; PPN₂ 7.2×/16) with
  work inflation 1.0–2.5×. We measured **0.27–0.30×** (i.e. a 3.3–3.7×
  slowdown) with **11.6–15.5× inflation**. The discrepancy is fully
  explained by job granularity and domain shape: Čížek's jobs are
  10³–10⁵ expansions on slow-expanding, position-monotone domains where
  each job's work is dominated by its own subtree, and worker TTs
  persist across jobs; our "jobs" (root children) are siblings whose
  searches redundantly re-derive what the root's shared TT already
  interleaved, and a process harness forfeits retention by design.
- The published near-linear small-pool numbers come from job-level
  parallelism over persistent private-state workers — our spike
  confirms the mechanism transfers only when jobs are *large and
  independent*. Root children are neither.

## Problems encountered

1. **Harness bugs (fixed during the spike, all measurement-affecting
   ones re-run clean):** (a) the initial race spawn loop checked a
   never-grown list and spawned **all 43 children simultaneously**;
   combined with (b) below this produced OOM kills that initially
   masqueraded as solver behavior — races "ending" at the cap with no
   proven child; (b) the container's cgroup enforces
   **`memory.max = 8 GiB`** (the plan assumed 16 GB) — 43 workers ×
   ≈230 MB plus ~7 GiB page cache hit the limit and the kernel killed
   workers mid-search (99 oom_kills recorded); (c) `--cap` passed as
   float made the solver reject `--timeout 5.0` (validation caught it);
   (d) race-condition-prone stdout parsing fixed by joining drain
   threads on worker exit; (e) aggregate-node accounting initially
   missed replaced (finished) workers. All final numbers come from
   post-fix runs.
2. **CPU-quota vs nproc honesty:** `cpu.max` gives exactly 4 CPUs — the
   wall measurements at N ≤ 4 are valid; N = 8/16 were simulated
   (labeled), per the plan's pre-registration.
3. **`find_winning_child` budget mismatch:** its 5 s timeout is
   hardcoded, so the full-proven-set comparison at matched 120 s
   budgets on m22 is impossible with the stock binary; the per-child
   table serves as the sequential reference instead, and the exact-
   agreement check ran on shallow FENs. Recorded as a spike limitation.
4. **Bash-tool time limits** killed two long race invocations; their
   orphaned solver processes were reaped and the affected runs redone.
   The shuffle-win probe's rep 2 was cut after rep 0 exhausted the
   queue (no information lost).

## Missing tests / gaps (what a real implementation would have needed)

- The harness (`race_harness.py`) is a spike artifact validated only by
  task 4 (shallow-FEN agreement) plus consistency checks; a real
  `examples/parallel_solve` would need: a unit test for the
  perspective mapping (terminal/loss/win/draw × proven/eliminated/
  unproven), a property test that the master's decisive composition
  never inverts a child verdict, and a race/regression test for the
  kill-and-reschedule path (the OOM/SIGKILL interactions this spike
  hit are exactly where an implementation would be fragile).
- Work accounting for killed workers is a lower bound (last chunk-log
  `nodes=`); a real implementation should expose an exact
  on-kill node count (a solver-side counter flush) before trusting
  inflation figures at the percent level.
- The 8/16-worker rows are simulations by design; on a bigger host they
  should be replaced by wall runs (nothing in the decision logic
  changes).

## Next steps

- Initiative `parallel`: moves toward closure — update the
  `docs/plans/README.md` status at the next initiative audit if any
  residue remains (row already updated as a close event).
- If a real consumer for fast single-position solves ever appears:
  start from `design_space.md` §5–6 and the reopen conditions there —
  the A-stage gate (mine SPDFPN 2014, inert TT-concurrency refactor,
  explicit renegotiation of constraints 2–4) is unchanged, and C is now
  measured out, not just argued.
- No plan 3: backlog #3 is closed with its gate unmet.
