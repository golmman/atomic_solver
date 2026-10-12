# Plan 12 — backlog #7 groundwork: Phase-0 noise probe, selective SPDFPN re-land, per-phase work accounting

Executes `parallel` backlog **#7 (any-worker-can-finish SPDFPN), groundwork
stage**. Backlog #7 is sequenced behind #5 (executed: plan8 ADOPT — the
salt portfolio is the family's adopted mechanism and #7's incumbent
comparator), so the item is unblocked. This plan is the item's
pre-registered prerequisite ("re-read plan6's bad-trajectory tails under
plan8's distributions before building") plus the two enablers any
mechanism session needs; the mechanism build + campaign is a separate
**plan13**, gated on this plan's decision point D1. Final task:
`report12.md`.

**Why groundwork is its own plan.** Three things changed since the #7 row
was written and none of them is a code change:

1. **The re-land procedure is stale.** plan6's stage-0 re-land
   (`git checkout b714ef6 --` over the revert inventory) predates the
   `facts` module (plan10/11), lean's 4-slot TT buckets (plan14) and the
   `with_mb` power-of-two rounding (plan15). A verbatim checkout of
   `b714ef6`'s `src/search/tt/` would revert lean's landed TT work and the
   facts-era hook sites. The re-land must be a **selective re-application**
   onto the current tree (stage 1 below), which is more than mechanical —
   it needs its own plan stage with conflict surfaces named up front.
2. **The measurement environment is now characterized.** plan11 measured
   the environment's 4-way contention floor (fresh facts-free pure-R0
   processes: 1.24–1.25× solo wall on dec13) and the overlay-filesystem
   writeback confound (15–23 % on solo sub-second runs; measurement dirs
   belong on tmpfs — `measurements/plan11/env.json`). Any plan13 campaign
   inherits both; this plan records them as normative inputs.
3. **The incumbent comparator moved.** The salt portfolio (#5/plan8/
   plan9) is adopted, and plan10/plan11 confirmed its one structural
   weakness: a 1-in-5 winnerless censored tail on m20_white (all racers
   budget-censored in some rounds). #7's shared-TT pool is the one shape
   that could fix that (cross-worker subtree dedup — the channel plan10/
   plan11's H1 measured at 0.575× evals on m20_white) — but only at the
   cost of the ~5 % sequential TT tax, `src/` soundness risk, and the
   plan5/plan6 coordinator-perturbation pathology. plan6's next-steps bar
   stands: a re-open must bring a *mechanism* lever targeting the
   bad-tail pathology, not another measurement campaign. This plan
   therefore builds the lever's diagnostic ground truth first.

## Hypothesis under test (pre-registered)

**H0 (noise-floor):** plan6's shuffle-win t4 bad-trajectory tail
(3/8 runs at 90–100 s, 1.4–1.6 G evals ≈ 5.7–6.3× sequential, cap-hit
timeouts) is an ordinary realization draw compared against a lucky salt-0
sequential denominator — i.e. it sits inside the sequential cross-salt
noise band scaled by the measured per-run parallel overhead, and the
plan5/5b/6 "coordinator-perturbation pathology" is not a parallelism-specific
effect. The re-examination addendum's observation motivating this: the
sequential salt noise band on shuffle-win ("stress") is loud (3.04×
spread) while plan6's parallel runs were never salt-controlled.

If H0 holds, the "trim the bad tail" motivation of #7 is dead: the
min-selection it would add is already harvested by the adopted salt
portfolio (zero `src/` risk), and #7 closes NO-GO on the probe alone.

## Stage 0 — Phase-0 noise probe (read-only, no code changes)

Data sources (all committed):

- `measurements/plan6/state/campaign6_raw.jsonl` — 8 rounds ×
  N ∈ {1..4} × {m22, rem12, shuffle_win}, with per-run
  `helper_evals` / `coordinator_evals` / `helper_nodes` / wall /
  outcome. The helper/coordinator decomposition per run is the key
  asset: bad-tail inflation can be attributed to coordinator
  perturbation vs helper over-work *without any new run*.
- `../research/measurements/plan12/state/runs.json` — the 22-case × 6-salt
  sequential rollout. shuffle_win = corpus case `stress` (3 of 6 salt
  draws uncensored, spread 3.04×). rem12 is **not** in the plan12 corpus.
- Fresh runs (allowed read-only probe): rem12 at the 6 canonical salts
  (`--salt {0..5}`, first-outcome, 1 B budget, ε 0.125 — the shipped
  defaults) to build its sequential realization distribution (~2 min of
  solver time). Recorded under `measurements/plan12/`.

Method (falsifiable):

1. **Magnitude.** For each plan6 parallel run r:
   `inflation_r = (helper_evals_r + coordinator_evals_r) / evals_seq_salt0`.
   Model: `inflation_r ≈ ratio_r × overhead_r` where `ratio_r` is the
   sequential realization ratio (cross-salt band [0.760, 1.316] quiet;
   shuffle-win loud 3.04×) and `overhead_r` is the measured parallel
   work overhead. Test: is the observed max bad-tail inflation
   (5.7–6.3×) explainable as `max uncensored cross-salt ratio ×
   max measured overhead_r`? If the observed inflation exceeds every
   such product by a clear margin (> 20 %), the tail is **not**
   realization noise. Same test for rem12 (loud spread only 1.31× —
   its I4 = 3.03× median inflation is the sharper test: if rem12's
   median inflation exceeds `1.31 × median overhead` clearly, the
   overhead is parallelism-specific, not realization luck).
2. **Frequency.** plan6 tail frequency (2–3/8 on shuffle-win t2/t4)
   vs the sequential cross-salt tail frequency (fraction of the 6 salt
   draws with ratio ≥ 2×; 3 stress draws are censored — report the
   censoring, do not impute). With 6 draws the frequency test is weak:
   it can veto (tail far more frequent than any salt draw) but never
   confirm; state the limit.
3. **Decomposition.** For the bad-tail runs: is the excess in
   `coordinator_evals` (perturbation of the finishing trajectory — the
   plan5/5b diagnosis) or in `helper_evals` (wasted helper work — a
   different pathology that any-worker-can-finish would *convert* into
   progress)? This directly informs plan13's mechanism design.
4. **Contingency on H0.** If both magnitude and decomposition say the
   tails are inside the noise model, H0 holds → D1 NO-GO branch.

Output: `measurements/plan12/probe_noise.json` + the H0 verdict paragraph
in `report12.md`.

## Decision point D1 (mechanical, after stage 0)

- **GO → write `plan13.md` (mechanism + campaign):** H0 rejected at the
  magnitude test on ≥ 1 of {shuffle-win, rem12} (a parallelism-specific
  inflation component survives), **and** the stage-1 tax pin passes
  (≤ +7 %), **and** the stage-2 accounting smoke is sound (V = 0,
  zero panics).
- **NO-GO → close #7:** H0 holds (tails fully inside the noise model,
  magnitude and frequency) → the bad-tail premise is dead; the adopted
  salt portfolio remains the family's only mechanism; #7 closes NO-GO
  with the probe as the record (re-land reverted in stage 3).
- **Borderline (mixed cases or within 20 % of the noise model):** owner
  decision with the full probe data; no mechanism session starts without it.

## Stage 1 — selective re-land (only on the GO branch; executed after the probe)

The plan4/4b sharded-TT refactor + plan5a `--threads N` mechanism,
re-applied onto the current tree. Sources:

- Sharded TT: `measurements/plan4/sharded_tt_attempt.patch` +
  `shard_rs_attempt.rs` (preserved from plan4; re-landed verbatim once
  before, plan4b).
- Parallel mechanism: commit `b714ef6` — `src/search/dfpn/parallel/
  {mod,jobs}.rs`, the `src/cli.rs`/`src/main.rs`/`children.rs`/`core.rs`
  wiring, `tests/test_parallel.rs`.
- W: `MAX_WORK_PER_JOB = 10_000` (plan5b short-class winner) with
  plan6's finding recorded: W is case-dependent (long class prefers
  20 000); plan13 pre-registers per-case W, this plan changes nothing.

Named conflict surfaces (resolve by hand, keeping the current tree's
landed work — never revert it):

- `src/search/tt/table.rs` — lean plan14 (4-slot buckets) and plan15
  (`with_mb` rounds down to a power of two) must survive; the plan4
  patch was written against 2-slot buckets. The sharding layer goes
  *around* the current layout, not through it.
- `src/search/dfpn/mod.rs`, `children.rs` — the facts
  `FactExporter`/`FactImporter` hooks (plan11) and refine-cap plumbing
  must survive.
- `src/main.rs`, `src/cli.rs` — the `--facts-out`/`--facts-in` flags
  must survive alongside the re-landed `--threads N`.
- `AGENTS.md` — module doc lines for the parallel submodule re-added;
  facts lines kept.

Verification before any parallel measurement (the N = 1 gate never
relaxes):

1. Fresh pre-re-land capture at current HEAD: quick suite
   (59/59 `child_evals`), m22 + shuffle-win + rem12 stdout sha256,
   TT-snapshot sha256 (`--tt-dump-path`, 195 B). Compare against
   `baseline_quick_pre5.json` and the plan6 hash record; if any stale
   (facts/lean landed since), the fresh capture is the new anchor and
   the staleness is declared in the report.
2. Re-land identity: all captured surfaces byte-identical to step 1.
3. `make test`, `cargo fmt --check`, `cargo clippy --all-targets` green;
   the re-landed `tests/test_parallel.rs` plus a concurrency-invariant
   smoke (zero decisive disagreements, zero panics/torn state on the
   m22 t4 smoke) pass.
4. **Tax pin** (owner condition 2, re-registered): interleaved A/B
   medians, N = 1 first-outcome wall, unsharded (step 1 build) vs
   sharded (this stage), m22 10 pairs + shuffle-win 3 pairs. Gate:
   ≤ +7 % median on both. On miss: revert (stage 3 procedure) and D1
   falls to NO-GO (the entry fee is part of #7's premise).

## Stage 2 — per-phase work accounting (report6's unresolved first diagnostic)

Instrumentation only, on the re-landed build — the counters plan6 named
as the first diagnostic any re-open must build, and the ground truth
plan13's mechanism verdict reads:

- Per-run counters separating **helper pre-warm quality** (distinct
  useful entries stored, entry re-store rate, jobs abandoned at the W
  bound) from **coordinator perturbation** (coordinator TT-read hits on
  helper-stored entries, coordinator trajectory decisions flipped vs
  the N = 1 baseline trajectory, per-chunk progress rate).
- Emitted as a side-channel (log/env-gated), never on the decisive
  stdout path; N = 1 byte-identity re-verified after instrumentation
  (same surface as stage 1 step 1).
- A single instrumented campaign cell (shuffle-win t4, 8 reps, W =
  20 000) recorded as the *accounting smoke*: V = 0 decisive
  disagreements, zero panics, per-run decomposition stable enough to
  read (this is smoke-grade, not the plan13 campaign — no verdict).

Note: the landed plan5a shape is **coordinator-only-finish** (report5a
deviation 1) — helpers pre-warm, only the coordinator can prove the
root. Stage 2 measures exactly the pathology that
any-worker-can-finish targets; it does not build the new shape.

## Stage 3 — revert procedure (NO-GO branch or tax-pin miss)

The `report5.md`/`report6.md` mechanical revert, adapted: restore
current-HEAD content over the re-landed files (the re-land inventory in
reverse), delete `src/search/dfpn/parallel/{mod,jobs}.rs`,
`src/search/tt/shard.rs`, `tests/test_parallel.rs`; re-verify the stage 1
step 1 capture byte-identical and `make test`/fmt/clippy green. The tax
is refunded; instrumentation goes with it.

## Preview of plan13 (non-normative — plan13 pre-registers its own gate)

What the mechanism session would contain, so this plan's GO branch is
concrete:

- **Mechanism**: extend the re-landed helper loop so a worker whose
  job proves the root (or detects a completed proof) can finalize and
  emit the decisive outcome — every worker can finish, first decisive
  result wins. Guards: the coordinator's chunk loop stays as the
  progress guarantee against the repetition-region stall (report5a's
  pure-pool failure: TT-guided `TRYRUNJOB` descents are blind in
  repetition-heavy regions — first-player-loss GHI shortcut leaves
  nothing to steer on); serialized `ProofEvent`s from whichever worker
  finishes; the GHI journal contract (design constraint 2) re-proven by
  outcome-agreement + the torn-state stress test under `make test-full`
  load (the test report5a/6 never wrote — carried into plan13).
- **Campaign**: 8 interleaved reps × N ∈ {2,3,4} × {m22 (control),
  rem12, shuffle-win}, per-case W (10 000 / 20 000); priority-1
  surface: m20_white at the plan12 2 B budget, K = 4 workers vs the
  fresh portfolio anchor (plan8's 95.89 s `W_first` / 791 M evals,
  re-baselined onto the current binary — plan10 found the old anchors
  stale); contention-floor accounting (plan11: fresh A0 control,
  1.24–1.25× floor; measurement dirs on tmpfs, never the overlay tree).
- **Adopt bar (preview)**: the pool must beat the *adopted salt
  portfolio*, not merely sequential N = 1 — on m20_white
  (`W_first` ≤ the portfolio anchor, and the winnerless censored tail
  reduced) and ≤ 1.1× wall elsewhere — plus the plan6-style mechanism
  bands (S4 ≥ 2.0×, work inflation ≤ 2.0×, cap-hit rate ≤ 1/8,
  V = 0). Missing the portfolio bar closes #7 even with a fast pool:
  the portfolio is 4× CPU with zero `src/` risk and no sequential tax;
  a shared-TT pool that cannot out-convert it has no reason to exist.
- **RAM restatement** (design constraint 4): one shared sharded TT —
  RAM = TT + per-worker repetition caches/stacks, documented in the
  module header (vs the portfolio's N × TT).
- **Docs** (design constraint 3): `child_eval_budget` exhaustion
  re-documented as nondeterministic under `--threads > 1`; `--threads`
  help text documents the long-solves-only recommendation.

## Non-goals

- No default-on parallelism; the sequential path stays byte-identical
  (stage 1 step 2) or the re-land is reverted.
- No cross-worker reuse of solved Win/Loss *facts* across process
  boundaries (plan10 hazard class, non-goals unchanged) — the shared TT
  is in-process and bound by the existing path-independence contract.
- No proof-tree/pipeline work; no new benchmarks beyond the stage 0
  rem12 salt runs and the stage 1 tax-pin references.
- No re-tuning of W or any scorer constant in this plan.
- No wall claims from N ≥ 8 (4-CPU container; plan11 floor applies).

## Risks

- **The probe's noise model is under-determined**: 6 salt draws, 3 of
  them censored on shuffle-win. The magnitude test is the load-bearing
  one; the frequency test can only veto. If the verdict is borderline,
  D1 hands it to the owner rather than over-claiming.
- **The selective re-land is not mechanical** (four named conflict
  surfaces, a patch written against a 2-slot TT). Budget half the
  session for stage 1; if the TT conflict resolution turns into a
  redesign, stop, record the blocker, and fall back to the NO-GO/defer
  branch — do not improvise a new TT architecture inside this plan.
- **Session budget**: stage 0 ≈ 30 min (analysis + 6 rem12 salt runs),
  stage 1 ≈ 2–3 h incl. `make test` and the tax pin, stage 2 ≈ 1–2 h.
  If over budget: cut stage 2 to the accounting smoke's design only
  (recorded in the report), never the stage 1 verification.
- **The tax pin could regress** (the 4-slot bucket layout interacts
  with the plan4 sharding differently than the 2-slot one did). It is a
  gate, not a hope: miss → revert + NO-GO.

## Deliverables

- `measurements/plan12/` — `README.md` (provenance), `env.json`
  (tmpfs note if any measurement writes files), probe driver + outputs
  (`probe_noise.json`), rem12 salt-run results, tax-pin + accounting
  state (`state/*.json|csv`); raw transcripts and any `.tt` snapshots
  gitignored per the measurement layout (prune to empty `*.tt.keep`
  tombstones, delete `__pycache__`, confirm with `git add --dry-run`).
- `initiative.md` — backlog #7 row updated at execution start
  (plan12 drafted/in execution) and at closure; no other rows.
- `docs/plans/README.md` `parallel` row — updated only on a #7
  close/pivot event (the initiative is already active; a groundwork
  plan is not an open/pivot/close event).
- **`report12.md` in this directory (final task)**: H0 verdict, D1
  outcome, re-land + tax-pin results, accounting smoke, problems,
  unresolved parts, missing tests, next steps (plan13 kickoff prompt on
  GO; closure block on NO-GO).
