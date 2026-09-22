# Plan 3: Quiet-root finishability probe (2 h Arm A) + ±1-ply substrate measurement

Initiative: `solve`. Follows `report2.md` §9 and the 2026-09-22 session
discussion. This plan tests the one assumption every campaign
reorganization (including the user's step-by-step proof/disproof
narrowing scheme) silently rests on but that no measurement so far has
touched:

> **Is a single quiet root finishable at all, given a ~10× budget over
> the plan1 censor plateau?**

It also measures the one substrate case plan2 left open: parent→child
overlap at exactly **±1 ply along a principal line** (plan2's ladder
pairs were ±2 plies; the discussion bounded the ±1 case at
~1/#children ≈ 3–10%, straddling the old 5% GO threshold).

**No product changes.** Zero source modifications; the whole plan is a
black-box driver (`measurements/plan3/`) over the unmodified release
binaries, following the plan1/plan2 conventions. Per repo convention
the final task is `report3.md`.

## 1. Background (self-contained summary)

- plan1: quiet positions censor at a flat ~20.5–46.6M-node / 120 s
  plateau regardless of depth; the 8 self-play ladders show the
  plateau at *every* ply along each line (hardness does not thin out
  with depth). 26 of 27 uncensored points are cheap tactical proofs
  (≤ 5k nodes, ≤ 9 plies).
- plan2 (report2.md): the pre-registered M1 substrate gate fired
  NO-GO — censored runs' TT solved sections are search-local (median
  cross-system value share 0.0000%; even ±2-ply same-system pairs
  ~0.1–0.4%). SSFP's cross-run anchor mechanism is measured empty;
  its product hooks were never landed. The 2 h pilot never ran.
- Discussion (2026-09-22): a step-by-step scheme — prove/disprove
  children one ply at a time, composing each validated subtree into a
  global artifact — is the correct bookkeeping of proof obligations
  and composes with the existing verify pipeline, but it only pays if
  (a) the residual subproblems are *finishable* given enough
  per-problem budget, and (b) parent-run deposits actually overlap the
  child's search space at ±1 ply. Both are open.

## 2. Task A — finishability probe (primary)

Root: d4d5 p2 (`rnbqkbnr/ppp1pppp/8/3p4/3P4/8/PPP1PPPP/RNBQKBNR w KQkq
d6 0 2`), censored at 23.4M nodes / 120 s in plan1 — the same root the
plan2 pilot was registered on.

Two arms, strictly sequential, defaults otherwise:

| arm | command (plus `--fen`, `--first-outcome`, `--tt-dump-path`) | rationale |
| --- | --- | --- |
| A1 | `--timeout 7200` (TT default 128 MB) | comparable to the plan1 plateau and the never-run plan2 Arm A |
| A2 | `--timeout 7200 --tt-size 1024` | the plan1 1 GB addendum cut nodes 22.6% at p30; a 2 h run at ~10× nodes may be TT-thrash-bound, and A1 censoring with A2 completing would mean finishability is *memory-mediated* — itself a finding |

Protocol (both arms): stdin from DEVNULL, raw stdout/stderr captured,
max-RSS recorded (`/usr/bin/time -v` when available), snapshot kept
until the report then pruned to counts + `.keep` markers (plan2
convention). On a **decisive completion**: `reconstruct_pt --snapshot …
--out …` must print `validate: ok` for the result to count as an
artifact; record the combined metric wall(find) + wall(verify)
(initiative constraint 3).

### Pre-registered gate (fixed before any run)

- **FINISHABLE** — either arm completes decisively with
  `validate: ok`: the residual core is finishable at ≤ 10× plateau
  budget. Record which arm, wall, nodes, combined ratio. The
  step-by-step narrowing scheme earns a pilot (its core assumption
  holds); sizing questions (per-step budget, chain depth) are answered
  from the measured cost, not guessed.
- **NOT FINISHABLE at 10×** — both arms censor: the hard core is
  bottomless at this scale. Record node-rate degradation vs. the 120 s
  plateau (~186k dfpn-nodes/s): a *sub-linear* rate (thrash) vs. a
  clean ~10× node count (linear work, no end in sight) are recorded
  as separate interpretations. This is a lower-bound statement about
  2 h budgets, **not** an impossibility claim about the position.
  Consequence: no reorganization of runs (narrowing, splitting,
  ordering) can pay at sandbox scale; the recommendation to the user
  is the sharpness-first rescope or closure (report2 §9 options).
- **SPLIT verdict** — A1 censors, A2 completes (or vice versa):
  finishability is TT-mediated; report both, treat as FINISHABLE for
  the mechanism question with the memory caveat recorded.

Order: A1 first, then A2 (A1's outcome does not change A2's
registration; both are fixed here).

## 3. Task B — ±1-ply substrate measurement (secondary, ~25 min)

plan2's ladder pairs were ±2 plies. This task measures the parent→child
case at exactly one ply along a principal line, on the existing quiet
lines.

- **Parents**: for each of the 5 quiet lines (startpos, e4, d4, nf3,
  d4d5), the shallowest ladder position whose 8 s steer solve emitted a
  PV (`steer: "pv"` in `../plan1/state/ladder_<line>.json`); ties
  broken by lower ply. Cap: 5 parents. *Caveat (documented in the
  report): the PV belongs to an 8 s steer search, not the 120 s cost
  run — a censored 120 s run prints no PV. It is still a
  most-proving line of the same position; the measurement is a
  realistic case, not necessarily the best case.*
- **Child** = parent + first steer-PV move (exact ±1 step, clock
  semantics preserved by construction).
- **Runs**: regenerate each parent (120 s cost run, snapshot kept) and
  run each child (120 s, snapshot kept): ≤ 10 runs ≈ 21 min.
- **Metrics** (pre-registered in the 2026-09-22 discussion, now fixed):
  - `avail(P→C) = |P.solved ∩ C.all| / |C.all|` — the fraction of the
    child's visited key space already proven by the parent (the
    anchor-reuse potential).
  - `carry(P→C) = |P.solved ∩ C.all| / |P.solved|` — the fraction of
    the parent's proof the child can consume (bounded by ~1/#children).
  - medians over the pairs; both directions of plan2's vshare reported
    for continuity.
- **Gate**: median `avail ≥ 5%` → the stepping scheme has substrate at
  ±1 and a follow-up anchor-preload test is worth proposing (that test
  needs the `--tt-load-path` hook re-added as *temporary measurement
  instrumentation*, plan1-menhist precedent: applied, measured,
  reverted, drift-captured — **separate user approval required**, not
  part of this plan). `avail < 1%` → the ±1 case is dead like ±2 and
  cross-system; between → judgment call with the numbers on record.

## 4. Tasks

1. Harness `measurements/plan3/` (`probe.py`: env / arma / armb /
   status; reuses the plan2 snapshot reader verbatim; README command
   table).
2. Run Task A (A1 then A2), apply the §2 gate.
3. Run Task B, apply the §3 gate.
4. Write `report3.md`: both gate verdicts, node/wall/RSS profiles,
   combined metric if completed, overlap table, what each outcome
   means for the three candidate mechanisms (narrowing / budget
   splitting / anchors), and a concrete recommendation (close with the
   artifact / sharpness-first rescope / step-by-step pilot with
   sizing). Prune snapshots per convention.

## 5. Non-goals

- No product changes, no `--tt-load-path` re-land, no proof-tree layer
  changes, no benchmark/drift-gate impact.
- No step-by-step campaign pilot (that is the successor plan, and only
  on a FINISHABLE verdict plus user sign-off).
- No claim about the startpos value either way; d4d5 p2 is a probe, and
  a censor is a budget lower bound, not an impossibility proof.

## 6. Budget

Sequential wall: A1 2 h + A2 2 h + Task B ~21 min + analyses
(reconstruction only on completion) → ~4.5 h on the reference sandbox
(4-core quota, 8 GiB cgroup). Nothing else runs concurrently
(censored/timing-sensitive runs must not be disturbed).
