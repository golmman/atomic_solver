# Plan 8: Item 8, stage 2 — campaign arms at 4 workers on the d4d5-p2 root (R_camp, m2, memory check)

Initiative: `solve`. Executes **item 8 (resource sizing), stage 2 of 3**,
per the 2026-09-24 rescope recorded in `initiative.md`: three ≤ 6 h
in-sandbox stages — sequential d4d5-p2 ladder (**plan7, executed
2026-09-24, `report7.md`**: gate **LINEAR**, R_seq certified) → campaign
arms at 4 workers (this plan) → multi-session checkpoint-resume
accumulation (plan9). Stage 2 is a **sizing measurement, not a mechanism
search**: it measures the campaign's aggregate work rate on the
registered representative quiet root at the stage-2 design point
(4 workers), under the plan7 §4 combination rule, with the memory check
mandatory (report6 §5.1's OOM precedent). No scheduler/budget variants —
the A5 attribution falsified that family, and this plan does not
re-tread it; the arms run the recorded incumbent shape (plan6 winner V1)
unchanged.

**No code changes at all.** Black-box driver over the unmodified release
binaries (product path unchanged since the plan4 `pt_keys` fix;
`examples/campaign_{master,worker}` unchanged since plan6; binary
SHA-256s recorded in `env.json` per the plan6 convention), harness under
`measurements/plan8/`, following the plan1–plan7 conventions. Per repo
convention the final task is `report8.md`.

## 1. Background (self-contained)

**Stage-1 locked inputs** (report7 §3, binding): R_seq = **198,229
nodes/s** (mean of L1/M2/L4; spread ±0.6%) on the d4d5-p2 root at the
reference config (default 128 MB TT, `--first-outcome`), certified over
1–4 h; η = 1.0081 (work grows linearly with wall); N_floor = **2.87 G
nodes** (L4 censoring floor); max-RSS flat at 234.7 MB; TT fill point
≤ 1 h (128 MB table at capacity, solved/unsolved split churning —
capacity-capped TT keys are **not** a progress metric, plan7 §1).

**Stage-2 obligations** (plan7 §4, binding): deliver **R_camp** (the
aggregate campaign rate at 4 workers on this root), **m2 = R_camp/R_seq**,
the **Σ-worker-nodes/hour** figure alongside child evals with the
**nodes↔child-evals conversion fixed once**, any **floor extension**,
and the **memory check** — the stage's own local gate: memory-checked
4-worker feasibility in the 8 GiB cgroup, GO-band m2 ≥ 1 vs the A5.4
prior.

**Why m2 ≥ 1 is plausible despite A5.** A5's "campaign < 1× sequential"
was measured as *wall-to-completion on finishable positions* (shuffle),
where censored reps grind non-proof-bearing leaves. The economics differ
on a *censored quiet root*: sequential churns a full 128 MB TT
(capacity-capped at ≤ 1 h), while campaign workers solve shallow leaf
jobs in private 128 MB TTs that never approach capacity. The one
on-record datapoint agrees: plan5's exploratory d4d5 C2 rep
(2 workers, 1800 s, the old 1M/8M ladder, `../plan5/state/
d4d5_C2_rep0_summary.json`) aggregated **637.2M worker nodes in 1800 s =
354k nodes/s → m2 ≈ 1.79** — with 0/27 children resolved and 707 open
leaves (censored, as expected). That is a single 2-worker datapoint on a
different budget shape; stage 2 is the registered 4-worker measurement
at the incumbent shape. It is corroboration only — no admission trick is
needed or used (both stage-2 arms are fresh; the budget fits, §7).

**Shape (registered, not a variant).** V1 = the plan6 winner:
`--slice 4000000 --max-slice 8000000`, feedback on (no `--nf`),
retention on (default; the plan5-confirmed win), **no `--abandon`**
(A5: structurally inert, 0 fires in 30 reps). Recorded caveat carried to
the report: R_camp is conditional on this shape; if a different recorded
shape has a higher quiet-root rate, that is a follow-up observation, not
a registered comparison (there is no compute for it).

**Unit.** dfpn node expansions, the plan7 §4 registered unit: sequential
side black-box via `pre_exit: nodes=`; campaign side via the master
summary's `worker_nodes` accumulator (schema on record:
`../plan5/state/d4d5_C2_rep0_summary.json`), cross-checked by summing
per-job result JSONs. Child evals are reported alongside (continuity
with plan5/plan6 metrics) but never substitute for nodes.

## 2. Stage-2 arms

Root: d4d5 p2 (`rnbqkbnr/ppp1pppp/8/3p4/3P4/8/PPP1PPPP/RNBQKBNR w KQkq
d6 0 2`), the registered representative quiet root. All arms strictly
sequential in the order below, nothing else running concurrently, stdin
from DEVNULL, raw captures under `logs/`, state JSONs under `state/`,
max-RSS per process via `os.wait4` ru_maxrss, per-arm resumability
(completed arm = state-file exists; plan7 convention), session dirs under
`/tmp`, tags carry arm **and worker count** (plan6's tag-collision
lesson: `<arm>w<workers>_rep<k>`).

| arm | command (campaign harness) | budget | purpose |
| --- | --- | --- | --- |
| SM | master `--workers 4 --slice 4000000 --max-slice 8000000 --max-wall 600 --tt-mb 128 --pt-mb 512` + 4 workers (`--tt-mb 128`, retention on) | 600 s | harness validation, early 4-worker rate, memory profile |
| S600 | `campaign_master --mode seq --fen <root> --seq-timeout 600` | 600 s | sequential-side nodes↔child-evals conversion on this root (the CLI does not print evals; the `--mode seq` summary reports both) |
| C4 | master + 4 workers as SM, `--max-wall 14400` | 4 h | **headline arm**: R_camp, m2 at the stage-2 design point, memory check at 4× SM |
| C1 | master + 4 workers as SM, `--max-wall 3600` | 1 h | rate-stability point: m2(C1) vs m2(C4) over the same 4× span as stage 1 |

**Arms note (deviation from the initiative wording, pre-registered).**
`initiative.md` scoped stage 2 as "2 h / 4 h caps". 2 h + 4 h = 6 h of
compute, which leaves nothing for the mandatory memory check (SM), the
conversion run (S600), or analysis inside the hard 6 h stage cap. The
1 h/4 h pair keeps the 4× span of the stage-1 ladder (L1/L4), so the
rate-stability question ("does the campaign rate decay with session
length, the way sequential's does not?") is answered on the same span
with the same arithmetic. The initiative wording is amended at this
plan's execution close.

**Primary metrics per campaign arm** (SM through C1):

1. **m2(T) = (Σ worker_nodes / wall) / R_seq**, R_seq = 198,229 nodes/s
   locked. On censored arms Σ worker_nodes is pure search nodes (no
   proven jobs → no reconstruction fills); if the COMPLETED branch fires,
   fill work is included and flagged (§3).
2. **Child-eval rate** and the **conversion** κ = Σ child_evals / Σ
   nodes per arm, plus the sequential-side κ from S600. The "fixed once"
   deliverable is the d4d5-p2 root's κ quoted from both sides (plan5
   datapoint prior: κ ≈ 28 job-level; sequential shuffle S was ≈ 17.9 —
   κ is class-dependent, hence per-root).
3. **Memory**: per-process max-RSS + a tree-RSS sampler (sum of
   `/proc/<pid>/statm` RSS over master + workers, 10 s cadence).
   **Registered abort rule**: tree-RSS > 7.0 GiB on two consecutive
   samples, or any single process > 3.5 GiB → SIGTERM the session and
   record the INFEASIBLE signal; an OOM kill (child vanishes with
   SIGKILL / cgroup `oom_kill`) is the same signal. An aborted arm is
   **not** retried at 4 workers (§7).
4. **Job-level health**: jobs/h, mean job wall, slice-ladder
   distribution (from worker job lines) — annotates whether 4 workers
   change the job mix vs plan5's 2-worker row.
5. **Master proof-state counts at censor** (`children_total/resolved`,
   `leaves_open/won/lost` from the master summary) — observational
   substrate datapoint for plan9's ρ, **not** a stage-2 verdict metric.

## 3. Pre-registered stage-2 gate (fixed before any run)

Headline m2 = m2 of the longest censoring arm (C4; C1 only if C4 did not
run, §7). COMPLETED overrides everything else.

- **COMPLETED** — any campaign arm resolves the root: the composed
  artifact must pass `inspect_pt --validate` and the root-outcome
  cross-check; the report gives combined wall(find) + wall(verify)
  (initiative constraint 3). Prior ≈ 0 (plan5: 0/27 children at 1800 s),
  but if it fires, the floor extends to the completed work and the
  item-8 question collapses for this root.
- **FEASIBLE** — arms censor, no memory incident; verdict by headline m2:
  - **GO-band: m2 ≥ 1** — the campaign beats sequential node-throughput
    on this root at sizing scale; **R_best = R_camp = m2 · R_seq**;
    plan7 §4's horizon-table campaign rows fill at R_camp; **floor
    extension**: N_floor′ = max(2.87 G, Σ nodes at censor of C4)
    (prior: m2 ≈ 1.79 would put C4's Σ ≈ 5.1 G > N_floor).
  - **SUB: m2 < 1** — R_best stays R_seq; the campaign column is priced
    at m2 · R_seq (a measured discount); A5.4 extends to throughput
    terms: the registered campaign shape does not beat sequential
    node-throughput on the quiet root at sizing scale either.
- **DECAY (modifier within FEASIBLE)** — m2(C4) < 0.8 · m2(C1) (or vs
  m2(SM) if C1 was skipped): the campaign rate decays with session
  length. Price R_camp at the C4 value (conservative) and flag plan9's
  cadence question; GO-band can still fire on the decayed constant.
- **INFEASIBLE** — a §2.3 abort signal fires (watermark or OOM), or the
  master aborts `verify_failed` (exit 3), or a worker crashes: the
  memory gate is negative at 4 workers. **Registered confirmation run**:
  one 30-min 2-worker session (same shape) to bound the memory ceiling
  and, if it runs clean, to hand plan9 a reduced-width ρ substrate.
  If the confirmation also fails, stage 2 reports INFEASIBLE with no
  campaign ρ substrate — an interface note for plan9 (its registered
  protocol presumes a live campaign master proof state) and a
  stage-2-negative input to the item-8 verdict.

## 4. Stage-2 rows of the plan7 §4 horizon table (deliverable)

Fill at report time, from the §3 verdict:

- **campaign @ 4 workers column**: T(W) = W / R_camp for
  W ∈ {10^14, 10^15, 10^17}, R_camp = m2 · R_seq (GO-band) or the
  discounted m2 · R_seq (SUB); DECAY flags the constant's provenance.
- **8-worker envelope row**: **not measured in this sandbox** — 8
  workers on the 4-CPU cgroup quota is oversubscription, uninformative
  by construction. The row stays the plan7 extrapolation (≈ 2× R_camp
  optimistically-linear, 12 h sessions), flagged as such; the offered
  step-up envelope (8 CPUs / 16 GB / 12 h) remains the first scale-up
  target.
- **Conversion note**: state κ (both sides) so W can be quoted in child
  evals for continuity with the plan5/plan6 record.

## 5. Tasks

1. Harness `measurements/plan8/`: `campaign.py` adapted from plan6's
   `run_arms.py` (single-rep arms, per-arm maxwall, d4d5-specific arm
   tokens, tree-RSS sampler, `os.wait4` ru_maxrss, worker-count tags,
   per-arm resumability; plan7's `ladder.py` env/state conventions;
   `env.json` with binary SHA-256s and the arm table; README command
   table).
2. Run SM; validate harness + memory profile. A driver defect here is
   fixed measurement-side and SM re-run (restarts are not datapoints).
3. Run S600 (sequential-side conversion).
4. Run C4 (headline), then C1 (stability point), strictly sequential;
   apply the §2.3 abort rule throughout.
5. Analysis: m2 per arm, §3 gate verdict, κ both sides, memory profile,
   job health, master proof-state counts, floor extension.
6. Fill the §4 table rows; update the stage-2 inputs in the item-8
   combination rule (R_camp, m2, κ, N_floor′) for plan9.
7. Write `report8.md` (verdict, arms × metrics table, deviations,
   snapshot/session cleanup per convention) and `measurements/plan8/
   README.md`; at close, amend `initiative.md` (stage-2 arms wording per
   §2's note; status block: plan7 executed, plan8 drafted/executed) and
   the `solve` row of `docs/plans/README.md` only if the initiative's
   status changes (it does not: stage 3 still pending).

## 6. Non-goals

- No product or campaign-code changes; no new lib surface.
- No scheduler/budget variants, no new shapes, no `--abandon` (A5 falsified
  the family; V1 is the recorded incumbent, run unchanged).
- No 8-worker in-sandbox run (quota-oversubscribed; §4 keeps the
  envelope extrapolated).
- No plan9 work: no ρ measurement, no checkpoint/resume machinery, no
  TT-snapshot loading (that item is cancelled), no durable store.
- No startpos-value claims: d4d5-p2 is the registered representative
  quiet root; a censor is a budget lower bound, not an impossibility
  proof.
- No re-litigation of closed lines: M1 substrate (plan2), quiet-root
  finishability at 2 h (plan3), scheduler/budget family (plan6/A5),
  coverage-race mechanism (report6 §6).

## 7. Budget

Compute (the cap counts compute wall, report6 convention): SM 10 m +
S600 10 m + C4 4 h + C1 1 h = **5 h 20 m**; analysis and report
≈ 20 m → **≈ 5.7 h of the hard 6 h stage cap**.

Contingencies (registered):

- An arm interrupted > 30 min in is **not** re-run (a re-run cannot fit
  the cap); the stage reports what completed (plan7 §7 pattern).
- INFEASIBLE on C4: skip C1, run the 30-min 2-worker confirmation
  (§3), report — total ≈ 2.5 h.
- If slippage leaves < 5.3 h at the C4 start, C4 runs first and C1 is
  the casualty (headline first); if < 4.3 h remain, C1 runs instead and
  the gate evaluates on m2(C1), flagged as the short-arm value.
- SM or S600 restarting for harness reasons does not count toward the
  cap's datapoint arithmetic (only the accepted runs do).
