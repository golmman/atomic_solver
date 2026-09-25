# Plan 9: Item 8, stage 3 — multi-session checkpoint-resume accumulation (ρ) and the item-8 verdict

Initiative: `solve`. Executes **item 8 (resource sizing), stage 3 of 3**,
per the 2026-09-24 rescope recorded in `initiative.md`: three ≤ 6 h
in-sandbox stages — sequential d4d5-p2 ladder (**plan7, executed
2026-09-24, `report7.md`**: gate **LINEAR**, R_seq = 198,229 nodes/s
certified) → campaign arms at 4 workers (**plan8, executed 2026-09-25,
`report8.md`**: gate **FEASIBLE, GO-band**, m2 = 2.8176, R_camp =
558,529 nodes/s, memory check passed) → multi-session checkpoint-resume
accumulation (this plan). **m2 is locked in GO band, so the item-8
verdict (plan7 §4 bands) hinges on ρ alone.**

Stage 3 is the verdict stage of the item-8 ladder, and it is also the
first stage that requires campaign-side code: the plan5 prototype has
**no resume path** — the master dumps `master_state.json` on a timer but
never loads it, and workers cannot carry their private TTs across a
session boundary. Building that machinery (examples/-side only; no lib
or product changes — §2) is the stage's instrument, not a mechanism
search: it is the minimal faithful implementation of initiative
constraint 4 (checkpointability) needed to ask the registered question.
The arms then measure ρ per the plan7 §4 definition ("defined precisely
in plan9" — §3 fixes it), and the report fires the item-8 verdict.

Per repo convention the final task is `report9.md`.

## 1. Background (self-contained)

**Locked stage-1/2 inputs** (plan7 §4 combination rule, as extended by
report8 §5): R_seq = 198,229 nodes/s; R_camp = 558,529 nodes/s at 4
workers (m2 = 2.8176, no DECAY, conditional on the registered V1 shape);
κ ≈ 27.5 child evals/node (both sides, ±0.4%); N_floor′ = 8.043 G nodes
(C4 censor); memory profile: 4-worker campaign ≈ 1.0 GiB tree-RSS vs the
7.0 GiB abort watermark (≈ 8× headroom for checkpoint stores);
product binaries unchanged since plan6 (report8 verified SHA-256 match).

**The frozen-frontier prior (shapes this plan's definitions).** report8
§3: the master proof state is frozen from ≤ 600 s onward — children 0/27
resolved, 698 open leaves, 30 leaf wins, identical at 600 s, 1 h and
4 h. All measured proof progress on the d4d5-p2 root is the early cheap
class; beyond it the frontier churns censored-draw jobs (every completed
job < 1M nodes, the budget ladder structurally non-binding). ρ is
therefore expected to be measured in a regime where *fresh* sessions
also make zero marginal progress — the §4 gate pre-registers how the
degenerate case is handled instead of burning the stage on a 0/0
surprise.

**Why worker TT restore is part of the faithful mechanism.** Within a
session, retention accumulates per-leaf search state in worker TTs
(plan5's C2-nr ablation: 0/10 proven without retention, 227×/25×
inflation — the retention mechanism is load-bearing). A "checkpoint"
that drops worker TTs at the session boundary measures the strawman: it
confounds "accumulation does not work" with "we threw the accumulation
substrate away". The architecture doc (§7) already assigns per-worker TT
snapshots to the checkpointability requirement as the performance-state
level; §5 classifies retention as a performance contract, never a
correctness one — **restore is retention across a process boundary**, so
no new fact class crosses any boundary: worker-decisive results still
export via the A1 self-contained pipeline (TT snapshot → `reconstruct` →
validator), still pass the A2 master replay tripwire, and the A3
job-local-fill residual + dual-check audit discipline carry over
unchanged. Restored solved entries are path-independent by the TT store
contract; restored unsolved bounds are advisory-only within the worker.
Job paths are fixed 2-ply leaf paths from the same campaign root, so a
restored bound for a leaf's subtree was derived under the identical
global context — the reuse hazard profile is exactly retention's.

**What exists to build on** (code audit, 2026-09-25): the lib snapshot
primitives are public and sufficient — `read_tt_snapshot` yields
`SolvedRecord {key, outcome, depth, best_move}` and `UnsolvedRecord
{key, pn, dn, depth, remaining_depth, best_move, work}`, and
`Search::tt_mut()` + `TranspositionTable::store` can rebuild a table
from them (best_child is not in the snapshot format; restore carries the
unset sentinel `u8::MAX` — an advisory ordering hint is lost, no fact is
affected; registered fidelity limitation). The master's
`master_state.json` (dump_state) carries per-leaf status/pn/dn/work/
slices but omits `depth`/`last_worker` (and per-child
`depth`/`synthesized`) — a v2 dump format is required for a faithful
resume. The fresh proof tree on resume is rebuilt by re-draining the
durable `results/*.json` (the job store is already at-least-once and
jobs idempotent): verification re-runs, events re-emit into the fresh
tree — the artifact rebuilds by construction, which is also what makes a
COMPLETED branch on a resumed session trustworthy.

## 2. Checkpoint/resume machinery (campaign-side only; the stage's instrument)

All changes confined to `examples/campaign*` (initiative non-goal scope:
"campaign code lives outside the product surface"). No lib changes; the
product binary must remain **byte-identical to plan8's** (registered
check in `env.json`; campaign binaries will differ — recorded as
expected).

1. **master_state v2** — additive dump extension: per-leaf `depth` and
   `last_worker`, per-child `depth` and `synthesized`, a `version`
   field. `--resume`: load the v2 state (statuses, advisory pn/dn, work,
   slices), drop all job locks (workers are gone), skip re-emission for
   already-synthesized children, and start with an empty `processed`
   list so the durable results are re-drained (re-verified, re-merged,
   events re-emitted into the fresh proof tree). Counters restart at 0;
   **all report metrics are marginals by subtraction of prior session
   closes** (registered accounting rule; the in-flight stragglers of
   session k − 1 drain at the start of session k and count toward
   session k — correct and noted).
2. **Job-id namespacing** — resumed masters take `--job-seed <n>`
   (default 0) so `w{w}_{seed}_{n}` ids cannot collide with earlier
   sessions' result files in the shared session directory.
3. **Worker TT checkpoint/restore** — `--tt-dump <path>`: on observing
   the stop condition (STOP file or `--max-runtime`), write the TT
   snapshot before exit; `--tt-load <path>`: at startup, read the
   snapshot and store every record into the fresh table via
   `tt_mut().store` (solved: outcome + depth + best_move; unsolved:
   bounds + work; best_child unset). Restore integrity is driver-checked
   (§3.3).
4. **Checkpointed close protocol** — master writes STOP; workers finish
   their in-flight job, dump their TT, exit; the **driver** (which owns
   the worker processes) waits bounded (300 s) for worker exit and the
   complete set of TT files, then versions the checkpoint artifacts
   (`master_state_s<k>.json`, `worker<w>_s<k>.tt`) so later sessions
   cannot overwrite a checkpoint an attribution arm still needs.
5. **Verification** — unit tests alongside the existing campaign-module
   tests (dump→load round-trip of the selection state; job-seed
   uniqueness), following the plan5 precedent; `make test` green
   (product path untouched, drift gates untouched); clippy/fmt clean;
   behavior in practice validated by the SMOKE arm (§3).

## 3. Stage-3 arms

Root: d4d5 p2 (`rnbqkbnr/ppp1pppp/8/3p4/3P4/8/PPP1PPPP/RNBQKBNR w KQkq
d6 0 2`). Shape: the registered incumbent V1, unchanged (`--slice
4000000 --max-slice 8000000`, feedback on, retention on, no `--abandon`,
4 workers, `--tt-mb 128 --pt-mb 512`) — plan8's binaries are superseded
only by the §2 resume machinery. All arms strictly sequential in the
order below, nothing else running concurrently, stdin from DEVNULL, raw
captures under `logs/`, state JSONs under `state/`, per-arm resumability,
session dirs under `/tmp` (≈ 600 MB of checkpoint TT files per
checkpoint fits the 4.1 GB free), `env.json` with SHA-256s (including
the product-binary identity check) and the arm table.

| arm | wall | protocol | purpose |
| --- | --- | --- | --- |
| SMOKE | 10 min | 5 min fresh → checkpoint → 5 min warm-resume | harness + machinery validation (round-trip, restore integrity, re-drain); **not a datapoint**; a driver defect is fixed measurement-side and SMOKE re-run |
| S1 | 3600 s | fresh + checkpoint at close | fresh-session control **on the new binaries** (Δfacts(S1) expected ≈ 30 per plan8 C1/SM/C4); the chain's base checkpoint |
| S2 | 3600 s | **warm resume** from S1 (state v2 + 4 worker TTs) | primary ρ measurement at the first boundary |
| S3 | 3600 s | warm resume from S2 | second boundary: does accumulation stack (ρ_3 vs ρ_2)? |
| RC | 3600 s | **cold resume** from S1 (state v2, fresh workers, no TT restore) | attribution control isolating the worker-TT-restore contribution (C2-nr analog) |

Primary metrics:

1. **Δfacts per session** = Δ(leaves_won + leaves_lost) + Δ
   (children_resolved), master proof state at close vs the prior close —
   new verified facts only (the checkpoint restores prior facts; deltas
   are taken over the restored state, so a resumed session cannot
   re-count the harvest).
2. **ρ per plan7 §4, fixed here**: ρ_k = Δfacts(S_k) / Δfacts(S1) for
   k ∈ {2, 3} (RC analog: ρ_cold = Δfacts(RC) / Δfacts(S1)); headline
   **ρ = (Δfacts(S2) + Δfacts(S3)) / (2 · Δfacts(S1))**. The denominator
   is the registered reading of plan7 §4's "a fresh session of equal
   wall": the marginal proof progress one fresh wall-hour produces from
   scratch on this root, measured on the new binaries by S1 and
   corroborated by plan8 C1 (30). The numerator asks the accumulation
   question directly: does warm accumulated state convert into new
   facts at ≥ the rate a fresh hour harvests the cheap class?

Secondary (observational, no verdict role):

3. **Restore integrity**: per worker, restored solved/unsolved record
   counts vs snapshot counts; a spot-check replays 100 sampled restored
   solved keys through a fresh `Position` — every replayed outcome must
   match the record (a mismatch is a machinery defect: abort the
   dependent arm, flag, do not re-run within the cap).
4. **Work-to-censor discount**: per re-queued leaf, mean per-job
   child_evals and advisory root_dn warm (S2/S3/RC) vs the same leaf's
   fresh jobs (S1). Distinguishes the two NEGATIVE failure modes of §4.
5. **Continuity/health**: jobs/h, job_errors (0 required),
   verify_failures (**0 required — a verify failure on re-drain is a
   resume-machinery defect: abort the arm, flag**), oom_kill delta 0,
   per-process max-RSS and the plan8 tree-RSS sampler with the §3 abort
   rule of plan8 (7.0 GiB / 3.5 GiB watermarks) throughout.

## 4. Pre-registered stage-3 gate and the item-8 verdict (fixed before any run)

- **COMPLETED** — any arm proves or refutes the root: the composed
  artifact must pass `reconstruct_pt`/`inspect_pt --validate` and the
  root-outcome cross-check; report wall(find) + wall(verify) combined
  (constraint 3). Prior ≈ 0 (report8: 0/27 at 4 h), but if it fires the
  item-8 question collapses for this root and the artifact is the
  deliverable. (A COMPLETED resumed session is trustworthy exactly
  because the fresh proof tree was rebuilt from the re-drained durable
  results — §1.)
- **Verdict bands (plan7 §4, binding; fired at report9):**
  - **POSITIVE** — ρ ≥ 0.7 ∧ m2 ≥ 1 (m2 = 2.8176 locked): accumulation
    stacks across session boundaries; the horizon table's accumulated
    rows fill at the measured constant; `solve` stays **active** with
    the table as the campaign go/no-go input (the user owns the
    decade-scale call with the table on record).
  - **NEGATIVE** — ρ < 0.4: `solve` goes **dormant** per the status rule
    (plan6 Not-GO + item-8 negative). The report must attribute *which*
    failure mode fired, via the §3.4 substrate metric:
    - *Frozen-frontier mode*: restore demonstrably works (integrity
      clean, warm discount present) yet Δfacts ≈ 0 — the frontier itself
      is inert at job-scale budgets, so accumulation is moot: progress
      ≈ 0 at any session structure. Reopener note: mechanism innovation
      targeting the frontier (the coverage race, A5.4), not
      accumulation.
    - *Resume-mechanics mode*: no warm discount (warm ≈ fresh work-to-
      censor) — search state does not survive the boundary usefully,
      extending plan2's locality finding to same-leaf resume. Reopener
      note: accumulation has no substrate even before the frontier
      question.
    Both modes are NEGATIVE (ρ = 0); the distinction goes into the
    reopener record and the mandatory `campaign_architecture.md` §11
    amendment (A6, plan6's attribution-amendment precedent).
  - **MARGINAL** — ρ ∈ [0.4, 0.7): judgment call with the table on
    record; a follow-up is entertainable **only** with a mechanism
    hypothesis for why ρ is partial; the scheduler/budget family (A5)
    must not be re-treaded.
- **Degenerate rules (pre-registered now, motivated by the §1
  frozen-frontier prior):**
  - Δfacts(S1) = 0 — contradicts plan8's on-record 30; a protocol or
    environment anomaly, **not a verdict**: halt, investigate; the
    verdict fires only from a corrected re-run within the cap or is
    deferred to the next session with the anomaly documented.
  - Δfacts(S1) > 0 with zero resumed numerators — ρ = 0 is a *measured*
    value, not a 0/0: the denominator exists (the fresh hour harvests
    the cheap class), the numerator is measured zero (no mechanism
    converts accumulated state into new facts). This fires **NEGATIVE**
    honestly, with the failure-mode attribution above. It is recorded
    explicitly that the limitation demonstrated is the root's frozen
    frontier plus absent conversion — not that session-boundary
    accumulation was falsified in the abstract.

**Deliverables.** The completed plan7 §4 horizon table: single-session
rows stand from report8 §4; the accumulated rows fill at ρ (POSITIVE/
MARGINAL) or are struck with the measured reason (NEGATIVE). The item-8
verdict. `campaign_architecture.md` §11 amendment A6. The checkpoint/
resume machinery itself remains in `examples/` as the campaign's
constraint-4 implementation regardless of verdict.

## 5. Tasks

1. Implement the §2 machinery (examples/ only); `make test` green;
   clippy/fmt clean; product binary byte-identity check.
2. Harness `measurements/plan9/`: `resume.py` adapted from plan8's
   `campaign.py` (chain orchestration, checkpoint versioning, marginal
   computation from state files, restore-integrity spot-check, tree-RSS
   sampler, per-arm resumability, plan7/8 env/state conventions;
   `env.json` with binary SHA-256s; README command table).
3. Run SMOKE; validate the round-trip end-to-end (v2 fields, TT file
   counts, re-drain, restore probe). A machinery defect found here is
   fixed and SMOKE re-run (restarts are not datapoints).
4. Run S1 → S2 → S3 → RC strictly sequential; plan8's abort rule
   throughout; apply §4.
5. Analysis: ρ per arm, §4 verdict with failure-mode attribution,
   substrate metrics, completed horizon table.
6. Write `report9.md` (verdict, arms × metrics table, deviations,
   cleanup per the measurement conventions: session dirs removed,
   checkpoint TT files deleted after the report, committed record =
   drivers + state JSONs + `analysis.json` + `env.json` + this plan's
   report); amend `campaign_architecture.md` §11; at close update
   `initiative.md` (status block + History) and the `solve` row of
   `docs/plans/README.md` (stale since report8 §7's note; stage 3's
   close changes the initiative's status disposition — active or
   dormant — so the row is updated at this close either way).

## 6. Non-goals

- No product changes, no lib changes; the resume machinery stays
  examples/-side. No `--tt-load-path` on the product CLI (item 7 stays
  cancelled; the campaign-side restore here is a measurement
  instrument, not a product surface).
- No scheduler/budget variants, no new shapes, no `--abandon`: V1 is
  the recorded incumbent, run unchanged; the registered scheduler/
  budget family is falsified (A5) and must not be re-treaded.
- No mechanism search: if the verdict is NEGATIVE, mechanism innovation
  is a reopener note, not work done here.
- No EGTB anchoring, no deeper master-side expansion, no
  prove/disprove directions (architecture doc's plan7-scope refinements
  have no registered owner anymore; not this plan's).
- No startpos-value claims: d4d5-p2 is the registered representative
  quiet root; a censor is a budget lower bound, not an impossibility
  proof.
- No re-litigation of closed lines: M1 substrate (plan2), quiet-root
  finishability at 2 h (plan3), scheduler/budget family (plan6/A5),
  coverage-race mechanism (report6 §6).

## 7. Budget

Compute (the cap counts compute wall, report6 convention): SMOKE 10 m +
S1 1 h + S2 1 h + S3 1 h + RC 1 h = 4 h 10 m; checkpoint/TT-dump and
re-drain overhead ≈ 10 m; analysis ≈ 30 m → **≈ 4.8 h of the hard 6 h
stage cap**.

Contingencies (registered):

- An arm interrupted > 30 min in is **not** re-run (plan7/8 pattern);
  the stage reports what completed.
- §4 degenerate rule: Δfacts(S1) = 0 → halt-and-investigate; no verdict
  from a broken denominator.
- A missing/incomplete worker TT file at a checkpoint close → the
  dependent resume arm degrades to cold-resume and is flagged (not
  re-run); if S1's own checkpoint is incomplete, S2/S3/RC all degrade —
  report the protocol deviation, fire no warm-ρ claim.
- If slippage leaves < 3.2 h at the S2 start, drop S3 (single-boundary
  measurement; RC keeps the attribution control); if < 2.2 h remain,
  S1 + RC only, with the verdict computed on ρ_cold and flagged as the
  short-arm value.
- SMOKE re-runs for harness/machinery reasons do not count toward the
  cap's datapoint arithmetic (only accepted arms do).
