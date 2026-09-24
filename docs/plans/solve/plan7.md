# Plan 7: Item 8, stage 1 — sequential d4d5-p2 sizing ladder + the item-8 combination rule (pre-registered)

Initiative: `solve`. Executes **item 8 (resource sizing), stage 1 of 3**,
per the 2026-09-24 rescope recorded in `initiative.md`: the off-sandbox
20–24 h single run is replaced by three ≤ 6 h in-sandbox stages —
sequential d4d5-p2 ladder (this plan) → campaign arms at 4 workers
(plan8) → multi-session checkpoint-resume accumulation (plan9). The
sandbox (4 CPUs, 8 GiB cgroup) is the reference environment; the
offered step-up envelope (8 CPUs, 16 GB, 12 h) is the first scale-up
target, not a prerequisite. Item 8 is the initiative's remaining lever:
`solve` falls to **dormant** only if plan6 (fired Not-GO) *and* the
item-8 stages jointly fire negative (the status rule).

plan6 closed the campaign line (A5: budget ladders price but cannot win
the coverage race; item 7 cancelled — the old plan7 name is retired
with it). What no measurement has produced yet is a **number**: every
quiet-root probe to date censored and the record says "bottomless".
Stage 1 converts that into the measured work rate and censoring floor
on the root hardware, at 4× the longest measured wall; §4 fixes —
before any stage runs — how the three stages combine into the item-8
verdict.

**No product changes.** Black-box driver over the unmodified release
binaries (`measurements/plan7/`), following the plan1–plan3
conventions. Per repo convention the final task is `report7.md`.

## 1. Background (self-contained summary)

- Root: d4d5 p2 (`rnbqkbnr/ppp1pppp/8/3p4/3P4/8/PPP1PPPP/RNBQKBNR w
  KQkq d6 0 2`) — the registered representative quiet root (plan2
  pilot, plan3 Task A), censored at every budget on record.
- plan1: 120 s cost run censors at 23.4M nodes (195.0k nodes/s); the
  plateau is flat across depth and lines (~186k/s plan-wide).
- plan3 Task A: 2 h arms. A1 (default 128 MB TT) censored at
  **1.429 G nodes / 7202.5 s = 198.4k nodes/s — 1.02× the root's own
  120 s rate**: clean linear work, no end in sight. A2 (1 GB TT)
  censored at 1.211 G nodes, rate 0.845× A1 (mild locality cost, not
  thrash) — finishability is not memory-mediated. Verdict: NOT
  FINISHABLE at the registered budget.
- plan3 Task A also fixed the TT-capacity fact this plan must respect:
  A1's 2 h snapshot held 2.455M solved + 1.739M unsolved =
  4,194,304 keys = **exactly the 128 MB table's capacity** — the table
  is full and churning long before 2 h. Consequence (registered):
  **TT solved-key counts are capacity-capped and are not a usable
  progress metric across arms**; the fill point itself is a datapoint
  (§2, secondary metric) and the accumulation metric of stage 3 must
  live on the campaign master proof state, not on TT keys.
- report6/A5: the campaign's open problem is the coverage race over
  the root's ~41 children; at the 600 s scale no registered mechanism
  beat sequential wall. Whether that holds at the 2–4 h scale on this
  root is precisely stage 2's question (§4 obligations).

## 2. Stage 1 — the ladder (primary)

Three wall-clock arms at the reference config (default 128 MB TT,
`--first-outcome`, TT snapshot dump — command-identical to plan3 A1
modulo `--timeout`, so every recorded datapoint is comparable):

| arm | command (plus `--fen`, `--first-outcome`, `--tt-dump-path`) | status |
| --- | --- | --- |
| L1 | `--timeout 3600` | fresh (this plan) |
| M2 | `--timeout 7200` | **on record: plan3 A1** (1.429 G nodes, 198.4k/s) |
| L4 | `--timeout 14400` | fresh (this plan) |

**Why the 2 h midpoint is reused, not re-run.** Three fresh arms = 7 h
of compute > the item-8 hard 6 h cap. plan3 A1 is the same root, same
flags, same default config, same sandbox, on a product path unchanged
since (plan5/plan6 touched only `examples/` campaign code). Its
admission into the ladder fit is gated by the comparability check
below — if the fresh arms' rates drift, A1 is demoted to
corroboration and the fit degrades to two points. No re-run is
registered (it would breach the cap); the drift itself is reported.

Protocol (both fresh arms, plan3 verbatim): strictly sequential
(L1 first, then L4; both registered before any run), nothing else
running concurrently; stdin from DEVNULL; **no `--outcome-only`** (the
`pre_exit: nodes=` line is the primary metric); raw stdout/stderr
captured under `logs/`; max-RSS via `os.wait4` ru_maxrss; state JSONs
under `state/`; snapshot kept until the report then pruned to counts +
`.keep` markers. `probe.py`-style per-arm resumability so an
interrupted arm can be relaunched.

Primary metrics per arm:

1. **Node rate** (nodes/s from `pre_exit:`) — the work-rate law.
2. **Censoring floor** — nodes at censoring (if an arm completes,
   §3's COMPLETED branch fires instead).
3. **max-RSS** — memory stability at 4× the measured span (plan9
   checkpoint-sizing input; the 8 GiB cgroup OOM precedent is
   report6 §5.1).

Secondary metric (cheap, one number per arm):

4. **TT occupancy** — solved/unsolved key counts from the snapshot:
   locates the time-to-fill point (already ≤ 2 h per plan3; is it ≤
   1 h?) and annotates snapshot churn for plan9's checkpoint-cadence
   sizing. Registered as **not** a progress metric (capacity-capped,
   §1).

## 3. Pre-registered stage-1 gate (fixed before any run)

Let η = log(N_L4/N_L1)/log 4 (fresh two-point work-growth exponent;
three-point fit over L1/M2/L4 if the comparability check passes).
Comparability check: L1's and L4's rates both within ±10% of A1's
198.4k/s → A1 admitted as the ladder midpoint.

- **COMPLETED** — any arm finishes decisively: `reconstruct_pt
  --snapshot … --out …` must print `validate: ok` for the result to
  count; record the combined metric wall(find) + wall(verify)
  (constraint 3). Prior ≈ 0 (plan3), but if it fires, the item-8
  question collapses for this root and the artifact is the deliverable.
- **LINEAR** — both arms censor, η ∈ [0.9, 1.1], monotone floors
  N_L1 < N_M2 < N_L4: long sessions buy proportional work at a stable
  rate. Expected under plan3: N_L1 ≈ 0.7 G, N_L4 ≈ 2.9 G nodes at
  ~198k/s. This certifies R_seq (§4) at the reference scale and is the
  branch stage 2/3 are priced against.
- **DEGRADING** — both arms censor, η < 0.9: work per wall-hour falls
  with budget (churn/overhead decay at 4 h — a new finding; the only
  recorded decline is in TT size, A2's 0.845×, not in T). Sequential
  long-session reach is discounted in §4's table; plan9's accumulation
  question becomes decisive (see §4 NEGATIVE).
- **SUPERLINEAR** — both arms censor, η > 1.1: floors grow faster than
  effort — recorded as an anomaly (a two/three-point censoring fit
  cannot be over-interpreted; check residuals first). Leans negative
  in §4; stages 2/3 proceed only on user confirmation.

## 4. The item-8 combination rule (pre-registered now; binding for plan8/plan9)

**Unit: dfpn node expansions** on the d4d5-p2 root (black-box
available sequentially via `pre_exit: nodes=`). plan8 must report the
campaign arms' aggregate Σ-worker-nodes/hour on the same root alongside
child evals, fixing the nodes↔child-evals conversion once.

Quantities, each measured by one stage:

- **R_seq** (stage 1, this plan) — sequential work rate on the root at
  the reference config, certified over 1–4 h by the §3 gate.
- **N_floor** — the largest censoring floor measured on the root
  (stage 1: N_L4; later stages may extend it). Honest caveat carried
  from report1/report3: censoring bounds N_finish from below only;
  N_finish is not estimable from above by any registered mechanism —
  the horizon table below is therefore conditional on W.
- **R_camp, m2** (stage 2, plan8) — aggregate campaign rate at 4
  workers on this root (2 h / 4 h caps, memory-checked:
  report6 §5.1's OOM precedent makes the memory check mandatory);
  m2 = R_camp/R_seq.
- **ρ** (stage 3, plan9) — the accumulation ratio: marginal
  master-proof-state progress of a resume session continuing a
  censored checkpoint vs. a fresh session of equal wall (defined
  precisely in plan9; measured on the campaign master proof state,
  never on TT keys — §1's capacity fact).

**Deliverable number.** The horizon table T(W) = W / R_best for
W ∈ {10^14, 10^15, 10^17} nodes — plan1's GO/RETHINK calibration
anchors — with R_best = R_camp if m2 ≥ 1 else R_seq, plus an envelope
row (8 workers ≈ 2× R_camp optimistically-linear, 12 h sessions) and
each row flagged single-session vs accumulated (per ρ). This is the
"bottomless → number" conversion item 8 exists for.

**Item-8 verdict (fired at plan9's report):**

- **POSITIVE** — ρ ≥ 0.7 **and** m2 ≥ 1: accumulation stacks across
  sessions and the campaign at 4 workers does not lose to sequential
  on this root at sizing scale. The horizon table becomes the
  campaign go/no-go input (expected to hinge on T(10^15) — the
  initiative's own calibration says the target is decade-scale; the
  user owns that call with the table on record).
- **NEGATIVE (dormant per the status rule)** — ρ < 0.4: no measured
  mechanism makes progress survive session boundaries, so total
  reachable effort on the root is bounded by single-session wall
  (≤ 12 h envelope ≈ single-digit G nodes ≈ within an order of
  magnitude of the stage-1 floor) while the floor grows with every
  effort increase by construction. "Bottomless" then carries to the
  hardware question: the available resources cannot converge on this
  root by scaling, and `solve` goes dormant (reopeners: mechanism
  innovation or a resource step-change, per `initiative.md`).
- **MARGINAL** — ρ ∈ [0.4, 0.7): judgment call with the table on
  record. A follow-up accumulation-improvement proposal is entertainable
  **only** with a mechanism hypothesis for why ρ is partial; the
  registered scheduler/budget family is falsified (A5) and must not be
  re-treaded.

Stage obligations fixed by this rule: plan8 delivers R_camp, m2, the
unit conversion, and any floor extension on this root (its own local
gate: memory-checked 4-worker feasibility in the 8 GiB cgroup, GO-band
m2 ≥ 1 vs the A5.4 prior); plan9 delivers ρ and the accumulated floor
point, then fires the verdict above using stage 1–3 inputs.

## 5. Tasks

1. Harness `measurements/plan7/` (driver adapted from plan3's
   `probe.py`: arms `ladder1` / `ladder4` / `status`, per-arm
   resumability, plan2/3 snapshot reader reused verbatim; README
   command table; `env.json`).
2. Run L1, then L4, strictly sequential; apply the §3 gate.
3. Analysis: rate law + η fit (A1 admission per the comparability
   check), floors, RSS profile, TT occupancy/fill point.
4. Fill the §4 horizon table's stage-1 rows (R_seq measured; campaign
   and envelope rows marked pending plan8/plan9).
5. Write `report7.md`: gate verdict, the measured number, the locked
   stage-1 inputs to §4, deviations, snapshot pruning per convention.

## 6. Non-goals

- No product changes; no campaign runs (plan8); no accumulation
  (plan9); no `--tt-load-path` re-land; no startpos-value claim —
  d4d5-p2 is the registered representative quiet root and a censor is
  a budget lower bound, not an impossibility proof.
- No re-litigation of closed lines: M1 substrate (plan2), scheduler/
  budget family (plan6/A5), narrowing/anchors (report3 §3).

## 7. Budget

Compute (the cap counts compute wall, report6 convention): L1 1 h +
L4 4 h = 5 h; snapshot dumps and analyses ≈ 15 min → **≈ 5.25 h of the
hard 6 h cap**. Contingency: an arm interrupted > 30 min in is not
re-run (a re-run cannot fit the cap); the stage reports with what
completed (L1 + the three plan1/plan3 datapoints) and L4 re-slots into
the plan8 session preamble. Report writing follows the compute cap and
does not count against it.
