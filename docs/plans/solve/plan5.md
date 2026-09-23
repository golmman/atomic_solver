# Plan 5: Campaign architecture design spike — GHI-correct job-level DF-PN (+ two-job prototype)

Initiative: `solve`. Executes the Phase-1 intellectual gate of the
2026-09-23 roadmap (`initiative.md`): the plan1–4 campaign line closed,
and the return path runs through a campaign architecture whose
soundness and economics are *designed and measured*, not assumed. The
open question from the initiative record — design-only vs. with a
small prototype — is resolved here per the recommendation on record:
**with prototype**. The prototype is what converts the design from a
paper into a pre-registered feasibility measurement, and it is the
only way to answer the one number the design owes:
`parallel/report2.md` measured the degenerate corner of this
architecture family (process-per-child race, no retention, no
feedback) at **0.27× wall / 15.5× work inflation**; the job-level
shape with persistent workers and master feedback claims that corner
is unrepresentative — the prototype must show it.

**No product changes.** The design doc and all prototype code are
campaign-side (`examples/campaign*`, `measurements/plan5/`); the CLI,
the sequential path's contracts, and the benchmark/drift gates are
untouched. Per repo convention the final task is `report5.md`.

This plan is independent of plan6 (off-sandbox resource sizing); the
two gates are combined only at plan7's threshold.

## 1. Background (self-contained)

**Where the campaign stands.** plan1: quiet positions censor at a
flat ~20.5–46.6M-node / 120 s plateau at every ply; the only
uncensored class is cheap tactical proofs. plan2: cross-run TT
substrate measured empty (M1 gate NO-GO, median share 0.0000%) —
censored-run solved sections are search-local; SSFP's cross-run
anchor mechanism is dead. plan3: a quiet root (d4d5-p2) does **not**
finish at 2 h with either TT size — work is *linear* (~198k
nodes/s), which is precisely what makes a bigger-machine pricing run
(plan6) meaningful. plan4: the sharp class saturates; the plan4
artifact (95 validated proofs, 10,177-key union) froze as the interim
deliverable, and its census **proved the position-keyed composition
contract** (421 cross-tree key pairs deduped; position-true keys via
the fixed `pt_keys`).

**The architecture hypothesis under test** (initiative.md, "Current
best architecture hypothesis"): job-level PNS over **persistent
workers** — each worker an (almost) unmodified sequential solver
instance with its own TT, retained across jobs for the whole run —
plus a **master** holding the proof state and feeding partial
pn/dn feedback, anchored where possible by verified EGTB layers,
with offline verification of the aggregated artifact. The plan2
spike's negative result is *not* evidence against this shape: it
measured the degenerate one-job-per-worker corner (fresh worker per
root child, no retention, no master re-selection).

**Mined theory** (all vendored + mined; no new reading round needed):

- `docs/theory/pns-pdfpn-2025` (Čížek et al., AAAI-26): PNS-PDFPN —
  master assigns pseudo-MPN leaf jobs; workers run (P)DFPN with
  caps, report pn/dn back, keep their TT across jobs; shared key
  database; 332.9× on 1024 cores. Its ablation (Fig. 4/5) is the
  direct answer to the plan2 failure: discarding worker state
  between jobs falls *below* sequential; retention + sharing +
  grouping recover scaling. Tunables: `iterations` (job cap),
  `updates` (feedback frequency), `grouping`.
- `docs/theory/pdfpn-2010` (Kaneko): shared-memory PDFPN — virtual
  proof numbers `vpn(n,c) = pn(c) + cost + T(n,c)` as congestion
  (pheromone); stop-set notification when a node resolves; 3.58×/8
  threads, ~11% overhead. Relevant for the second level *if ever*;
  out of scope here (constraint 5: scale by adding worker processes,
  not shared memory).
- `docs/theory/ppn2-2011` (Saffidine et al.): PPN₂ — two-level
  PNS/PNS, master-side re-direction with locked leaves; 7.2–18.3× on
  16–64 cores. The master feedback loop's reference shape.
- `docs/theory/ghi-journal-2005` (Kishimoto & Müller) and the repo's
  standing contract (`dfpn/research_ghi_journal.md`,
  `research/structural_floor.md` §plan10): repetition-dependent
  results are never cached as path-independent; the solver's
  first-player-loss shortcut. **No mined paper treats repetitions
  under parallelism** — this is the genuinely open design risk, and
  the reason this plan is a *GHI-correct* design spike.

**The one structural threat to answer head-on.** The plan2 spike's
mechanism finding: the sequential root proves m22 in 858k nodes while
the winning child alone needs 8.59M nodes isolated (~10× cross-child
transposition/interleaving subsidy), and no process-per-child
schedule recovers it. Job-level decomposition avoids this only if
(i) jobs are *bounded* so the master re-selects continuously (the
interleaving subsidy is preserved at the master's proof state), and
(ii) workers retain state so repeated visits to a subtree do not
restart from zero. Both mechanisms are measurable; the prototype
measures them (ablations, §4).

## 2. Deliverable D1 — the soundness contract (`campaign_architecture.md`)

A design document in this directory (no code), normative for plan7.
It must fix, in writing, before the prototype runs:

1. **Job definition.** A job is `(subposition, direction, budget,
   context)` where subposition is a FEN including the halfmove clock
   (Zobrist keys include it — `src/zobrist.rs`), direction ∈ {prove,
   disprove, expand} (or the PNS-native "evaluate pseudo-MPN leaf"),
   budget is a **deterministic work cap** (child-eval budget /
   expansion cap, never wall clock — the solver's
   `set_child_eval_budget` semantics), and context carries the
   repetition-relevant path information the worker needs.
2. **Split contract — what crosses the worker boundary.** Only:
   (a) rule-derived terminal facts; (b) worker-decisive results,
   exported as **proof-event streams / proof subtrees**, not as bare
   outcomes; (c) advisory pn/dn progress numbers, never trusted for
   composition. **No repetition-dependent fact crosses a boundary.**
3. **Merge contract — global-context replay verification.** The
   master composes a borrowed decisive result into its proof state
   only after **replay verification in global path context** (the
   existing `proof_tree::validate` discipline: apply incoming moves,
   re-evaluate). A worker proof that rests on a first-player-loss
   repetition judgment valid at the job root can fail replay under a
   different global path — it is then rejected, not patched. This is
   constraint 1 ("no in-flight fact is trusted") made mechanical;
   its cost is re-verification wall, paid under the combined metric
   (constraint 3). The document must state the verification policy:
   per-fact on receipt, and always again at artifact finalization.
4. **Anchor-key clock safety.** Any future anchor store (verified
   subtrees, EGTB layers) keys by `(position, halfmove-clock)` at
   minimum; the document enumerates which fact classes are
   clock-safe to preload (replay-verified subtrees re-validated on
   use; rule-derived terminals) and which are never preloadable
   (budget/timeout-derived anything; repetition-flavored results).
   plan2's M1 result stands: anchors are a *within-campaign*
   deposit mechanism, not a cross-run substrate.
5. **Worker state & GHI.** Each worker is an in-process `Search`
   instance (lib crate) with its own TT retained across jobs;
   retention is sound because TT base entries are path-independent
   and repetition-dependent results are never cached (existing
   solver semantics, unchanged). The journal contract binds within
   every worker exactly as in the sequential solver.
6. **Master proof state.** AND/OR (NAND) proof state over position
   keys; pseudo-MPN selection; locked-leaf reservation (PPN₂);
   partial pn/dn updates from running workers (Čížek `updates`);
   congestion damping if needed (Kaneko's `T(n,c)` — recorded, not
   required for v0).
7. **Checkpointability** (constraint 4): master proof-state dump,
   durable job store, per-worker TT snapshots — all restartable; no
   correctness-relevant state in one process's RAM only.
8. **Cluster readiness** (constraint 5): the worker/master software
   must run unchanged at 2 workers in the sandbox and at N workers
   on a cluster; communication = job/result messages only (no
   shared memory beyond one node). The document states the message
   schemas.
9. **Work-inflation mechanism table**: for each of the plan2 failure
   modes (isolation subsidy, killed-work waste, restart-from-zero),
   the design element that addresses it and the §4 measurement that
   falsifies the claim.

## 3. Deliverable D2 — two-job prototype (`examples/campaign*`)

Minimal campaign harness, campaign code only (initiative non-goal
explicitly allows this: "campaign code lives outside the product
surface"):

- `examples/campaign_master`: proof state v0 (NAND pn/dn over
  position keys), pseudo-MPN job selection, locked leaves, partial
  pn/dn updates, checkpoint dump/restore.
- `examples/campaign_worker`: persistent worker process — loads jobs
  from the master (stdin/pipe or file-based store v0), runs an
  in-process `Search` with retained TT under deterministic budgets,
  emits decisive results as proof-event streams + progress pn/dn.
- `measurements/plan5/` drivers + README command table, per the
  measurement conventions (env.json, parsed state committed; raw
  transcripts not).

Scope guard: this is a **two-job-feasibility** prototype (master + 2
workers, one position, one session of runs) — not the plan7 product
surface, not a durable multi-node store, not EGTB anchoring.

## 4. Prototype measurement — arms, metrics, pre-registered gates

**Positions (pre-registered; finishable so baselines exist).** The
quiet class cannot serve: its roots censor (plan3), so no sequential
baseline completes. Mechanism economics are measured on:

- **Primary: shuffle-win** (`--name shuffle-win` in the decisive
  suite; sequential baseline 47.8 s / 13.9M nodes, `parallel/design_space.md` §3
  — re-run once on this build for drift).
- **Secondary: m22_white** (2.6 s / 858k nodes) — sanity scale.

**Arms** (each ≤ 30 min, strictly sequential across arms; 5 reps for
wall medians):

| arm | config | what it isolates |
| --- | --- | --- |
| S | sequential solve (baseline) | reference wall/work |
| C2 | campaign, 2 persistent workers, feedback on | the design point |
| C4 | campaign, 4 workers, feedback on | scaling slope at the sandbox ceiling |
| C2-nr | C2 with worker TT reset between jobs (fresh worker per job) | retention value (Čížek Fig. 4 analog) |
| C2-nf | C2 with master feedback off (jobs run to completion, PNS-PNS style) | feedback/interleaving value (PPN₂ analog) |

**Metrics:** wall speedup vs S (median of 5); **work inflation** =
aggregate worker work (child evals / nodes) ÷ S work; retention
benefit (C2 vs C2-nr); feedback benefit (C2 vs C2-nf); **soundness
audit** (below). A bounded exploratory run on d4d5-p2 (equal 30-min
budgets, S vs C2, no gate) records whether the campaign shape shows
master-tree progress where the sequential run censors — reported as
observation only.

### Pre-registered gates (fixed before any run)

- **SOUND (hard, correctness first):** zero false decisive facts end
  to end. Concretely: the composed prototype artifact for C2/C4
  passes `reconstruct_pt --validate` with the sequential run's
  outcome; every composed fact is replay-verified in global context
  (per §2.3); a spot dual-check (≥ 20 exported facts re-derived
  independently by the sequential solver) shows zero contradictions.
  **Any violation = NO-GO regardless of economics.**
- **ECON (primary position, C2):** GO = wall ≥ **1.2×** AND work
  inflation ≤ **3.0×**. MARGINAL = wall ≥ 1.0× AND inflation ≤ 5×.
  NO-GO otherwise. Calibration: published 2-worker figures are
  1.47× (Kaneko, shared memory) and ~1.87× (Čížek PNS-DFPN) with
  ~6–20% work overhead; the plan2 race measured 0.27× / 15.5×. The
  GO band sits deliberately below the published figures because D2
  is an *untuned v0 harness* — Čížek's own parameter table shows
  untuned configurations undershoot his best numbers substantially —
  while still strictly above sequential: a parallel shape that does
  not beat the sequential solver at 2 workers has no campaign.
- **C4 scaling:** GO requires C4 wall ≥ C2 wall (no regression with
  more workers) and inflation ≤ 3.5×.
- **Mechanism attribution (gating for GO, not just supporting):** the
  design's central claim — retention + feedback are what separate
  this shape from the measured-dead corner — must be *demonstrated*:
  inflation(C2) < inflation(C2-nr) and inflation(C2) < inflation(C2-nf).
  Economics that clear the GO band without this separation yield
  MARGINAL at best (the design works but not for the stated reason,
  and plan7 would be drafted on an unattributed result); if the
  ablations do not separate, the architecture doc must record which
  mechanism actually paid before any plan7 work.
- **Verdict semantics:** GO (econ, with SOUND) → plan7 is draftable
  (campaign product surface + one-quiet-system pilot). MARGINAL →
  one bounded iteration on the recorded diagnosis before any plan7
  work. NO-GO (econ or sound) → no plan7; `solve` falls to dormant
  if plan6 also fires negative (initiative status rule).

## 5. Tasks

1. Write `campaign_architecture.md` (D1) — all §2 items; this is the
   gate for starting D2 (the prototype is the contract's
   conformance test, so the contract exists first).
2. Build the prototype (D2) + `measurements/plan5/` harness.
3. Baselines: S runs (fresh, this build) for both positions.
4. Run C2 / C4 / C2-nr / C2-nf (5 reps each, medians), the
   exploratory d4d5-p2 equal-budget pair, and the soundness audit
   (artifact validation + dual-check). Apply the §4 gates.
5. Write `report5.md`: gate verdicts, arms × metrics tables, the
   attribution result, deviations, the concrete recommendation
   (plan7 draft / iterate / dormant per plan6), and any contract
   amendments discovered by the prototype. Prune snapshots per
   convention.

## 6. Non-goals

- No product CLI changes (`--tt-load-path`, `--frontier-dump`,
  resume flags land only in plan7, and only on GO, to the §2 spec —
  the plan2 early-implementation-and-revert lesson).
- No startpos-value claim; no full campaign; no EGTB anchoring;
  no multi-node store; no second-level (shared-memory) parallelism.
- No change to the product-mode parallel no-gos (`lean` plan7,
  `parallel` plan2) — their scope stays the CLI; this plan measures
  campaign-side software only.
- No dependence on plan6; the two combine only at plan7's threshold.

## 7. Budget

Design doc + prototype build ≈ one session. Runs: baselines ~2 min;
C2/C4/ablations 5 reps × 4 arms × ≤ 5 min ≈ ≤ 2 h; d4d5-p2 pair
60 min; audits ≈ minutes. Total ≤ ~4 h of sandbox compute. Nothing
else runs concurrently during timed arms.
