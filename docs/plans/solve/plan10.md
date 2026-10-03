# Plan 10: Phase 3, Stage 0 — frontier budget-completion sweep (scheduling-vs-censoring diagnostic)

Initiative: `solve`. Executes **Phase 3, Stage 0** of the 2026-10-03 owner
decision (recorded in `initiative.md` Status): the **frontier
budget-completion diagnostic** over the plan9 frozen frontier — "decides
whether scheduling (A5.4 coverage race) suffices or censoring (A6) is
required, and prices the report9 budget-cap resolution caveat". Stage 0 is
registered as **S** (small): it is a measurement, not a mechanism — the
harness is the existing measured-sound plan9 campaign machinery driven in a
new way, with **no solver, lib, or campaign-code changes** (the only new
file is the driver under `measurements/plan10/`, Python 3 stdlib, per the
plan8/9 precedent).

Per repo convention the final task is `report10.md`.

## 1. Background (self-contained)

**The A5/A6 reopener record.** plan6 (A5) falsified the registered
scheduler/budget family: budget ladders price but cannot win the coverage
race over the root's children. plan9 (A6) built and validated the
constraint-4 checkpoint/resume machinery and measured **ρ = 0.0**
(Δfacts 30/0/0/0): accumulated state survives boundaries but converts into
zero marginal proof progress. The pre-registered discriminator fired the
*resume-mechanics* mode (no warm discount; both sides ≈ 100% budget-pinned
at 8M evals on the common re-queued leaves) **with a recorded resolution
caveat**: fresh jobs on those leaves were already 99.7% pinned, so a
discount could only manifest below-cap and the metric could not resolve
there. The two recorded mechanism directions are (a) **A5.4** — master-side
targeting of the proof-bearing child (scheduling) and (b) **A6b** —
budget-aware early-censor certificates (worker-side, unvalidated). Stage 0
prices both before either is built.

**New plan-session audit of the frozen frontier** (mined from the committed
`measurements/plan9/state/master_state_s1.json`, plan9's fresh S1 close;
the state carries per-leaf status/pn/dn/work/slices for all 728 leaves):

- **Coverage is 104/698 open leaves — 85% of the frontier is never
  touched.** The incumbent dispatch policy (`select_leaf`: key =
  (child pn-sum, TT affinity, leaf dn, static rank)) locks each worker onto
  exactly one affinity leaf per child (4 workers × 27 children ≈ 104–108
  leaves; observed: 4 touched leaves per child, one per worker). A leaf
  once censored is re-queued to its last worker forever (affinity = 0
  dominates the grown dn), so session length does not broaden coverage —
  the "698 open leaves" of report8/9 is a *dispatcher-biased sample*, and
  the 594 untouched leaves sit at the build prior (pn = dn = 1, work = 0).
- **Spend is extremely concentrated**: of the 56.8 G child-evals spent on
  touched leaves in one fresh hour, `1.e4` absorbed 35.1 G and `1.e3`
  18.5 G (94% in two children); 8 leaves exceeded 1 G evals each (max
  8.84 G ≈ 44 min of one worker on one leaf); 27 leaves pinned at exactly
  28M (the V1 ladder's 7-slice ceiling).
- **`c1g5` is two leaf-wins from root-child resolution** (24 replies Won,
  2 open: f7f6 at 68M historical evals, g8f6 at 52M). A resolved child
  would be the first root-level conversion ever measured on this root
  (children_resolved frozen at 0/27 since plan8).
- **Advisory bounds are far below budget** on touched leaves (median
  root_pn 521, root_dn 4,522 ≪ 8M) and did not predict censoring in plan9 —
  the A6b certificate question (do advisory bounds predict conversion?) is
  open and measurable at larger budgets.

These facts reframe Stage 0: the sweep is the **first full-frontier
measurement** — it bypasses the incumbent dispatcher by construction
(driver-written jobs), so it measures the frontier the dispatcher never
reaches, including the 594 never-searched leaves.

**What exists to build on** (code audit, this session): the worker binary
is job-driven and dispatcher-independent — it claims jobs by `w{w}_` prefix
from the session's `jobs/` dir (atomic rename), replays the 2-ply path from
the campaign root (global repetition prefix per the context contract), runs
`search_depth_with_prefix` under the job's `budget_evals` (child-eval
budget; `begin_run()` resets `nodes`/`child_evals` per job, so per-job
marginal accounting is exact), and for decisive outcomes exports a
validator-clean subtree (TT snapshot → `reconstruct` →
`validate_proof_tree`, A1) — a decisive sweep result is self-validating at
the worker boundary. The stop condition (STOP file or `--max-runtime`)
triggers the plan9 `--tt-dump` before exit; `--tt-load` restores at startup
(measured sound end-to-end in plan9: SMOKE 7/7, 8/8 clean restore cells,
0 verify failures). The master's `--resume` re-drains durable results with
global-context replay verification (A2) and the report9 `find_loc_by_path`
fallback merges driver-written job ids by path.

**Compute constants** (plan7/8 certified): R_seq = 198,229 nodes/s;
campaign rate at 4 workers R_camp = 558,529 nodes/s (m2 = 2.8176);
κ ≈ 27.5 child-evals/node. Budget arithmetic: 8M evals ≈ 291k nodes ≈ 1.5 s
sequential; the full ladder worst case is bounded in §7.

## 2. Sweep harness (driver-only; no solver, lib, or campaign-code changes)

All sweep logic lives in `measurements/plan10/sweep.py` (Python 3 stdlib
only, `os.wait4`-based process control, adapted from plan9's `resume.py`
conventions). Campaign binaries are the plan9 build, unchanged; the
**product binary must remain byte-identical to plan9's record**
(SHA-256 `1b70b32f46d9218e…`, registered in `env.json`).

1. **Leaf population**: the 698 `Open` leaves of
   `measurements/plan9/state/master_state_s1.json` (path = child `mv` +
   reply `mv`, both UCI). Registered sanity check: the driver recomputes
   the counts (728 leaves / 30 Won / 698 Open, 27 children) from the state
   file; a worker replay error on any path is a machinery defect (abort).
2. **Session skeleton** (driver-written, no master during the sweep): the
   driver creates `/tmp/plan10_sweep/session.json`
   (`root_fen` = d4d5 p2 = `rnbqkbnr/ppp1pppp/8/3p4/3P4/8/PPP1PPPP/RNBQKBNR
   w KQkq d6 0 2`, tt_mb 128 — the worker reads only `root_fen`) and writes
   job files directly (`w{w}_{rung}_{n}.json`: `path_uci` = the 2-ply path,
   `budget_evals` = the rung budget, `direction` = "evaluate"). Leaves are
   partitioned round-robin over 4 workers. **Bypassing the master's
   dispatcher is deliberate and is part of the design**: the incumbent
   selection policy is itself a measured object (§1), and Stage 0 must
   observe the full frontier, not the dispatcher's 15% sample.
3. **Rung ladder with warm continuation**: rungs R1 = 8M, R2 = 32M,
   R3 = 128M child-evals per job. R1 runs on fresh workers (no
   `--tt-load`); after each rung the driver writes STOP, waits bounded
   (300 s) for worker exits + the complete TT-dump set (plan9 close
   protocol), then relaunches workers with `--tt-load` for the next rung —
   **warm cumulative semantics**: each worker's retained TT carries its own
   rungs' state, so a leaf's cumulative sweep spend mirrors the campaign's
   warm re-queue reality (retention is measured load-bearing, C2-nr). A
   leaf with a decisive result at any rung exits the ladder (its result is
   final); only unresolved leaves get the next rung's jobs.
   **Contingent R4 = 512M** on the R3 survivors, fired iff at the R3 close
   `elapsed + K·(512M/κ)/R_camp ≤ 5.0 h` (K = survivor count; projected
   before launch, recorded either way).
4. **Per-rung barrier + accounting**: the driver waits until every rung job
   has a result file (claim files cleaned by the worker), then records per
   job: outcome, exit_reason, per-job `child_evals` (marginal —
   `begin_run()` resets), `nodes`, wall, advisory `root_pn`/`root_dn`,
   event count. Per-leaf cumulative spend = Σ per-job marginals.
5. **Conditional cold-control arm**: fired iff ΔC > 0 (§4) — up to 30
   leaves resolved at R2/R3 (stratified sample) re-run **cold** (fresh
   workers, no TT restore) at their resolving rung's budget, giving the
   warm-vs-cold spend ratio at budgets where a discount can manifest
   (directly pricing the report9 A6 caveat at last). Not fired if the
   frontier stays inert (nothing to discount).
6. **Health**: job_errors must be 0; worker stderr captured under `logs/`;
   cgroup `memory.events` oom_kill delta 0 required; per-worker max-RSS
   recorded. TT dumps kept only for the latest rung (≈ 480 MB/set; disk
   bound ≈ 1 GB under `/tmp`), deleted at close.
7. **No master during the sweep** ⇒ **no fact crosses any artifact
   boundary**: sweep outcomes are diagnostic measurements. The single
   soundness gate on the decision metric is the §3 merge pass.

## 3. Merge pass (the A2 discipline on the decision metric)

After the ladder: the driver copies plan9's `master_state_s1.json` into a
fresh merge session dir as `master_state.json` and copies **only the
decisive sweep results** (≤ 1 per leaf by ladder construction) into its
`results/`, then runs `campaign_master --resume --job-seed 10 --max-wall 60
--slice 4000000 --max-slice 8000000 --workers 4 --tt-mb 128 --pt-mb 512`
(the plan8/9 incumbent shape). The resume re-drain re-verifies every
decisive result under the **global context** (A2 hard tripwire; the
`find_loc_by_path` fallback merges driver-written job ids) and merges the
facts into the proof state — the merged close state (dumped via
`--state-every`) reports the post-sweep `children_resolved`, `leaves_won`,
and per-leaf statuses. Registered containments: the drain completes in the
master's first loop iteration before any dispatch; the ≤ 60 s dispatch
slop adds at most 4 × 8M-eval jobs whose results arrive after exit and are
ignored; **a verify failure here is a machinery defect (abort, flag) — the
affected leaf's resolution is invalidated and excluded from the decision
metric**, not patched.

## 4. Metrics and the pre-registered Stage-0 gate (fixed before any run)

Root: d4d5 p2 (the registered representative quiet root; as in plan9, a
censor is a budget lower bound, not an impossibility proof).

Primary metrics:

1. **M1 — completion curve**: C(B) = # of the 698 open leaves with a
   validator-clean decisive result at cumulative budget B ∈ {8M, 32M,
   128M(, 512M)}, split Won/Lost. Sub-reading: C(8M) on the 594 untouched
   leaves = the cheap harvest the incumbent dispatcher leaves on the table.
2. **M2 — censor-depth distribution**: per rung, work-to-censor for
   unresolved leaves (marginal per-job evals): cap-pinned fraction, median,
   deciles. The report9 "≈ 100% pinned at 8M" datum re-measured at every
   rung.
3. **M3 — advisory-signal predictivity** (A6b viability): for leaves
   entering R2/R3 (warm bounds available), does the pre-rung advisory
   (root_pn, root_dn, prior cumulative work) separate rung-resolved from
   rung-censored? Reported as best-Youden 2×2 over each signal and the
   combined prior, plus AUC. **No separation ⇒ certificates built on
   advisory bounds have no measured substrate.**
4. **M4 — root-level conversion**: post-merge `children_resolved` and the
   per-child "open wins needed" table. **M4 > 0 is a headline structural
   finding reported regardless of the bands** (it breaks the 0/27
   frozen-frontier record) and identifies which children are within reach
   at what spend — Stage 1's targeting data.
5. **M5 — cumulative-spend ledger**: per-leaf record (path, untouched/
   touched flag, historical work = max over the committed plan9 session
   states S1/S2/S3/RC [sessions are independent runs; plan8 states carry
   aggregates only — registered limitation], per-rung marginal spend and
   outcome, cumulative sweep spend, merged status) →
   `measurements/plan10/state/ledger.json` + summary CSV.
6. **M6 — frontier pricing**: total sweep spend, per-rung spend, and — if
   conversion is real — the projected budget to drain the reachable
   frontier within the measured ladder, as the input to any Stage 1
   pricing.

**Decision gate (pre-registered; ΔC = C(128M) − C(8M), the budget-scaling
conversion signal on the full frontier):**

- **SCHEDULING GO** — ΔC ≥ 25 leaves (≥ 3.6% of the frontier): higher
  budgets measurably extract facts the incumbent budgets never see; the
  frontier is budget-starved, and master-side targeting + spend-aware
  re-queue (Stage 1, A5.4) is the registered next lever. Stage-1 design
  inputs: M4's within-reach children, M5's per-leaf costs, the resolved
  leaves' identity.
- **CENSORING GO** — ΔC ≤ 5: budget escalation is refuted on this frontier
  (spending more per job extracts nothing); the only remaining lever is
  cutting repeat waste → Stage 2 (A6b early-censor certificates), **gated
  on M3**: a GO with no M3 signal downgrades to *MARGINAL-with-no-
  substrate* (the certificate soundness argument would need a non-advisory
  substrate — recorded, not invented here).
- **MARGINAL** — 5 < ΔC < 25: judgment call with M1–M6 on record; both
  stage designs priced, no A5-family re-tread.
- **Sharpened-negative branch** — ΔC ≤ 5 ∧ M3 shows no separation: both
  registered levers lack a measured substrate on this frontier; the
  initiative returns to dormant with the sharpened reopener record (the
  resource step-change — the 8-CPU/16 GB envelope — remains the only
  pricing lever, and it re-prices rather than reopens). This is a valid,
  reportable completion.

Registered sanity cross-checks (observational): sweep R1 work-to-censor on
the 12 plan9-common leaves vs plan9's fresh values (dispatcher-independent
replication); R1 outcome on the 30 already-Won leaves' siblings (no
double-resolution expected — they are excluded); job_errors 0.

## 5. Tasks

1. `measurements/plan10/sweep.py`: session skeleton, job writer, worker
   orchestration (rung barriers, close protocol, warm relaunch), accounting
   collector, conditional cold-control arm, merge-pass driver; `env.json`
   with binary SHA-256s incl. the product-byte-identity check; README
   command table (plan7–9 conventions).
2. Run the ladder R1 → R2 → R3 (§2); fire the contingent R4 / cold-control
   arms per §2.5/§2.3 rules; plan8 abort watermarks (7.0 GiB tree /
   3.5 GiB process) watched throughout.
3. Merge pass (§3); collect M1–M6; `state/*.json` per-arm records +
   `analysis.json` + `ledger.json`.
4. **`book.md`** (bounded docs-only side task, Phase 0 of the roadmap,
   pending since 2026-09-23): generate `docs/plans/solve/book.md` — the
   human-readable refutation book — from the committed
   `measurements/plan4/state/p2_*.json` (400 records, each with outcome +
   PV; data complete, verified this session). Structure = report4 §1
   expanded per first move with PVs and node costs; verification = counts
   match report4 §1 (55 decided; 17/13/13/11/1 per-move table).
5. Write `report10.md` (decision-gate verdict, M1–M6 tables, deviations,
   cleanup per the measurement conventions: session dirs + TT dumps
   deleted, committed record = driver + state JSONs + analysis + ledger +
   env + README). Amend `campaign_architecture.md` §11 only if a new
   attribution-level fact emerges (A7 candidate: the full-frontier
   measurement superseding the dispatcher-biased record).
6. Update `initiative.md` (Status + History) and the `solve` row of
   `docs/plans/README.md` at close (row already moved to active at this
   plan session; the close records the Stage-0 verdict and the next stage
   or the sharpened-negative disposition).

## 6. Non-goals

- No product, lib, or campaign-code changes; no new examples binaries
  (the merge pass drives the existing binaries). Product binary
  byte-identity registered.
- No Stage 1/Stage 2 code (no targeting logic, no censor certificates) —
  this plan only decides which of them is drafted next, with gates.
- No scheduler/budget-family variants (A5 stays falsified; the sweep
  bypasses selection rather than tuning it).
- No artifact composition: no sweep fact enters any proof tree or PPV; the
  merge pass is a verification instrument, its output a master-state
  record only.
- No startpos-value claims; d4d5-p2 remains the representative quiet root.
- No re-litigation of closed lines (M1 substrate, plan3 finishability, A5,
  ρ session-structure): the sweep measures the frontier's budget response,
  not session structure.

## 7. Budget

Compute (wall of compute, report6 convention), worst case per rung at
R_camp = 558,529 nodes/s, κ = 27.5, all 698 leaves censoring at every cap:

| stage | worst-case work | wall |
| --- | --- | --- |
| R1 (698 × 8M evals = 5.58 G evals ≈ 203 M nodes) | | ≈ 6 min |
| R2 (698 × 32M ≈ 812 M nodes) | | ≈ 24 min |
| R3 (698 × 128M ≈ 3.25 G nodes) | | ≈ 1 h 37 m |
| R4 contingent (K × 512M evals ≈ K × 18.6 M nodes; all 698 surviving would be ≈ 6.5 h — fired only under the 5.0 h projection rule, so the realized bound is the projection) | K ≤ 698 | ≤ 5.0 h projected (rule-bounded) |
| cold control (≤ 30 × ≤ 168M evals ≈ 183 M nodes) | | ≈ 6 min |
| merge pass | | ≤ 10 min |

Registered ladder total ≈ **2.2 h** worst case (typical less: resolved
leaves exit early); with the contingent R4 the hard 5.0 h projection rule
bounds the session under the 6 h stage cap. Analysis ≈ 30–45 m.

Contingencies (registered):

- An arm interrupted > 30 min in is not re-run; the stage reports what
  completed (plan7/8/9 pattern).
- A worker crash mid-job: its claimed job's result never arrives — the
  driver detects the missing result at the rung barrier, re-writes that
  job for another worker id, and continues (recovery is measurement-side;
  the re-run job is fresh-budget, recorded as a deviation if it fires).
- A missing/incomplete worker TT dump at a rung close → that worker
  continues the next rung cold (plan9 DEGRADED precedent; flagged, not
  re-run).
- If slippage leaves < 1.5 h at the R3 start, R3 is truncated to a
  stratified sample of ≥ 200 leaves (untouched-heavy) and the gate fires
  on the sampled ΔC with the truncation recorded.
- Machinery defects (replay error, verify failure, job_error ≠ 0): abort
  the affected leaf/arm, flag in the report; SMOKE-style re-runs after a
  driver fix do not count toward the cap's datapoint arithmetic.
