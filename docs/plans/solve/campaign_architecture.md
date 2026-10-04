# Campaign architecture — GHI-correct job-level DF-PN: soundness contract (plan5 D1)

Status: **normative for plan7** (campaign product surface). Written before the
plan5 prototype ran (pre-registration discipline); the prototype
(`examples/campaign_master`, `examples/campaign_worker`) is this contract's
conformance test, and §9 records where the v0 prototype deliberately
implements a subset.

Scope: the *campaign* computational object — a months-scale, throughput-
oriented decomposition of one root's proof over persistent worker processes —
not the product CLI's sequential solve, and not the closed product-mode
parallel options (`lean` plan7, `parallel` plan2). Nothing here changes the
CLI, the sequential path's contracts, or the benchmark/drift gates.

Theory anchors (all vendored + mined, see `docs/bibliography.md` and the
`research_*.md` files in this initiative and in `docs/plans/parallel/`):
job assignment and retention from PNS-PDFPN (Čížek et al. 2025,
`docs/theory/pns-pdfpn-2025`); master-side re-direction and locked leaves
from PPN₂ (Saffidine et al. 2011, `docs/theory/ppn2-2011`); congestion
feedback from SPDFPN (Kaneko 2010, `docs/theory/pdfpn-2010`); the repetition
discipline from the GHI journal (Kishimoto & Müller 2005,
`docs/theory/ghi-journal-2005`) as already instantiated by this repo's
solver (`src/search/dfpn/research_ghi_journal.md`,
`research/structural_floor.md` §plan10).

---

## 0. The one design risk, stated first

No mined paper treats repetitions under parallelism. Every decisive fact this
architecture exchanges therefore carries a repetition hazard: a judgment that
is valid under the path it was computed on may be invalid under another path.
The whole contract below is organized around one invariant:

> **CI (context identity).** A worker's proof output is merged only at the
> exact path (root FEN → move sequence) it was computed for. Cross-path reuse
> of a decisive fact happens exclusively through replay-validated channels:
> the master's finalize + validate pass (§3) or a future anchor store's
> re-validation-on-use policy (§4).

CI is what makes "no repetition-dependent fact crosses a boundary" (§2)
mechanical: a fact crosses *with its path attached*, and the master verifies
it *under that path* before trusting it anywhere.

## 1. Job definition

A **job** is `(job_id, subposition, direction, budget, context)`:

- `job_id` — session-unique, encodes the dispatching master and a monotonic
  sequence; used as the durable store key.
- `subposition` — reached by replaying `context` from the campaign root FEN.
  The campaign root FEN is fixed for the lifetime of the session (mirroring
  the product solver's fixed-root rule). The worker never receives a bare
  FEN as the source of truth: it receives the path and replays it, so the
  halfmove clock and castling/EP state are derived, not trusted.
- `direction` ∈ {prove, disprove, evaluate}: v0 uses `evaluate` only — the
  worker searches unrestricted and reports the decisive outcome found; the
  master decides favorability. `prove`/`disprove` (targeted outcome search)
  is a plan7 refinement, not a soundness-bearing element.
- `budget` — a **deterministic work cap**: cumulative child evaluations
  enforced by `Search::set_child_eval_budget` (budget-exhausted →
  `Outcome::Draw` + `ExitReason::BudgetExhausted`, never wall clock). Wall
  time appears in the worker only as a safety net that must be strictly
  looser than any budget the master relies on for correctness accounting.
- `context` — the full move path (UCI) from the campaign root to the
  subposition. This is the repetition-relevant context: the worker rebuilds
  the position *and* the repetition-key prefix from it and seeds the search
  with `search_depth_with_prefix`, so every repetition judgment inside the
  job is made under the global path's ancestor set (the same discipline the
  reconstruct walker and `verify_ppv` use).

## 2. Split contract — what crosses the worker boundary

Only three fact classes cross, each tagged with its provenance class:

| class | content | trust at receipt |
|---|---|---|
| **rule-derived** | static terminal classifications, legal-move sets, replayed position state | trusted (the master derives them itself by replay; a worker never needs to send them) |
| **worker-decisive** | a proven `Win`/`Loss` at the job root, exported as a **proof-event stream** (the worker's `NodeProven` events: `(path, outcome, depth)`), never as a bare outcome | **not trusted**: merged only after §3 verification |
| **advisory progress** | partial pn/dn of the job root, work counters, exit reasons | never trusted for composition; ranking/bookkeeping only |

**No repetition-dependent fact crosses a boundary.** Operationally:

- The solver never caches repetition-dependent results as path-independent
  (first-player-loss GHI shortcut), and never emits `Draw` proof events, so
  nothing repetition-dependent *can* appear in a proof-event stream.
- A worker's Draw outcome (budget-cut or exhausted) is **not** a fact and is
  not exported; it stays inside the worker as advisory progress.
- A worker proof that rests on a first-player-loss repetition judgment valid
  at the job root is valid *for that job path* and is merged only at that
  path (CI). The master never re-anchors such a subtree elsewhere.

Per-worker GHI quality is unchanged product semantics: each worker is an
in-process `Search` with its own TT; `begin_run` per job clears the
repetition cache; TT base entries are path-independent; unsolved pn/dn
bounds are advisory within a worker and never exported.

## 3. Merge contract — global-context replay verification

The master composes a borrowed decisive result into its proof state in two
places, both mandatory:

1. **Per-fact on receipt.** The master replays the job path from the
   campaign root (legality-checked, clock-derived), then replays the
   incoming event stream incrementally in DFS order: every move is
   legality-checked against the replayed position, every node's Zobrist
   hash is recomputed in global context, and the job root's claimed outcome
   must match the root event. An event that fails replay rejects the *whole
   result* (not patched, not partially merged) and the master re-queues the
   job or marks the leaf failed. Cost: O(total stream length) replays,
   bounded by the artifact size — paid under the combined metric
   (initiative constraint 3).
2. **Always again at artifact finalization.** The master's aggregated tree
   is finalized (canonical subtrees copied onto unexpanded transpositions —
   the only place a fact is ever re-anchored at a different path) and then
   validated by `proof_tree::validate`'s replay validator, which re-plays
   every root-to-node path on a real `Position` and checks outcome/terminal/
   completeness/depth/hash/cycle rules. A violation fails the artifact;
   there is no "repair" path.

Why this catches the repetition hazard: the validator replays every path
from the campaign root, so a decisive fact whose justification depended on a
repetition that does not occur under the replayed path either (a) manifests
structurally (a `CycleLeaf` defect, a terminal mismatch, or a
`LossIncomplete` where the "missing" reply was wrongly judged a repetition
draw) and is rejected, or (b) replays identically because the merged path is
the job path (CI) and the judgment carries over. The residual risk that no
structural validator can see — a semantically repetition-flavored Win whose
justification happens to replay cleanly — is bounded by the same property
that bounds it in the sequential solver: repetition-dependent *results* are
never cached, so the only repetition inputs a worker proof can have are its
path ancestors, which the merge preserves verbatim. This residual is
recorded as **GAP-1** and is the standing reason the offline validator, the
dual-check audit, and (future) EGTB cross-validation stay mandatory gates;
it is not closed by this architecture, it is *managed* by it.

Merge bookkeeping (what the master does with a verified result):

- Favorable decisive result → the master's proof obligation at that node
  resolves (OR child proven / AND reply covered); the event stream grafts
  into the global proof tree.
- Unfavorable decisive result (e.g. a `Loss` where a `Win` was needed) →
  the parent obligation is **refuted**; the master prunes that branch and
  updates its selection state. A refutation is also a decisive fact and is
  verified identically.
- Advisory result → selection-state update only.

## 4. Anchor-key clock safety (for any future anchor store)

Any anchor store (verified subtrees, EGTB layers) keys by
`(position, halfmove-clock)` at minimum — in this solver's terms, by the full
Zobrist key including the rule50 key (`src/zobrist.rs`), never by the
board-only repetition key. Fact classes:

- **Never preloadable:** budget/timeout-derived anything (an unsolved
  pn/dn bound is advice, not a fact); repetition-flavored results of any
  kind; any fact whose provenance is not one of the two classes below.
- **Preloadable with re-validation on use:** replay-verified proof subtrees
  (each use re-replays in the consuming path's context — a use that cannot
  replay is discarded, never patched).
- **Preloadable unconditionally:** rule-derived terminal facts (they are
  functions of the replayed position, clock included).

plan2's M1 result stands unchanged: anchors are a **within-campaign deposit
mechanism** (a run deposits its verified sections for its own later
checkpoint/restore and for sibling workers of the same session), not a
cross-run substrate; cross-run reuse measured empty (median value share
0.0000%).

## 5. Worker state & GHI

- A worker is a process running an in-process `Search` (the lib crate,
  unmodified semantics) with a private TT **retained across jobs for the
  whole session**: no `new_generation()` is ever triggered between jobs, so
  solved base entries and unsolved bounds stay probe-able (this is exactly
  the retention mechanism PNS-PDFPN's ablation shows is load-bearing).
- Soundness of retention is *existing solver semantics*, unchanged: TT base
  entries are path-independent; repetition-dependent results are never
  cached; the per-run repetition cache is cleared in `begin_run`; the
  journal contract (`dfpn/research_ghi_journal.md`) binds within every
  worker exactly as in the sequential solver.
- Retention is a **performance** contract, not a correctness one: zeroing a
  worker's TT between jobs may cost work (measured: plan5 arm C2-nr) but
  never soundness. Correctness-relevant state never lives in a worker TT —
  every fact that matters is re-verified at the master (§3).
- Worker liveness: workers are replaceable processes. Death of a worker
  loses only its private TT (performance state), never proof state (the
  master holds the proof state; §7).

## 6. Master proof state

v0 shape (what plan5 implements):

- An AND/OR proof state over **position keys** (replayed paths + recomputed
  Zobrist hashes), rooted at the campaign FEN.
- pn/dn bookkeeping in PNS semantics over the master's frontier:
  OR node `pn = min(children pn)`, `dn = Σ children dn`; AND node
  `pn = Σ children pn`, `dn = min(children dn)`; open leaves `(1, 1)`;
  resolved leaves `(0, INF)` / `(INF, 0)`. The master itself performs
  rule-derived expansion of AND nodes (opponent replies) — movegen is cheap
  and keeps the master's proof state ahead of the workers' results.
- **Pseudo-MPN job selection:** dispatch idle workers to the open leaf with
  minimal proof cost estimate, where the estimate mixes (a) the worker-fed
  advisory pn/dn of that leaf's last job, (b) accumulated work spent on it,
  and (c) a static `StaticAtomicScorer` prior. This is PPN₂'s master-side
  re-direction with Čížek's job-cap granularity.
- **Locked-leaf reservation:** one in-flight job per open leaf
  (PPN₂ locking); re-queues of the same leaf prefer the same worker
  (TT affinity).
- **Partial pn/dn updates (Čížek `updates`):** each bounded job's result
  carries the job root's post-run TT bounds; the master refreshes the leaf's
  estimate and re-ranks before the next dispatch. Congestion damping
  (Kaneko's `T(n,c)`) is **recorded, not required for v0**: if worker
  feedback oscillates, the damping term is the first planned addition.
- The master never trusts advisory numbers for composition (§2); they steer
  *where to search*, never *what is proven*.

## 7. Checkpointability (initiative constraint 4)

All correctness-relevant state is durable; nothing lives only in one
process's RAM:

- **Master proof state** — periodically dumped (JSON: selection state,
  obligation/resolution statuses, advisory numbers) and restorable on
  restart; the authoritative artifact is the finalized proof-tree dump
  (`proof_tree.bin`), written at session completion.
- **Durable job store** — jobs and results are files in the session
  directory; a result is durable before the master acts on it; the store
  survives master/worker restarts (at-least-once dispatch; jobs are
  idempotent — re-running a job yields the same-or-refuted decisive facts
  under the same path).
- **Per-worker TT snapshots** — the existing `tt_snapshot` format per worker
  on demand (`--tt-dump-path` semantics), for offline reconstruction and
  post-mortem; not required for correctness.

## 8. Cluster readiness (initiative constraint 5) and message schemas

Software shape: one master process + N worker processes; communication is
**messages only** (files in v0; same schemas over any durable queue later).
No shared memory beyond one node; scaling = adding workers. The v0 file
schemas (JSON lines, UTF-8):

```
session.json            { root_fen, config { tt_mb, slice_budget, ... } }
jobs/<job_id>.json      { job_id, worker, path_uci: [...], direction,
                          budget_evals, created }
  claim protocol: worker renames the file to <job_id>.claim (atomic)
results/<job_id>.json  { job_id, outcome: win|loss|draw, exit_reason,
                          nodes, child_evals, wall_s,
                          root_pn, root_dn,          // advisory
                          events: [ { path_uci, outcome, depth } ] }  // decisive only
stop                    master ⇒ workers: drain and exit
```

`events[].path_uci` is **job-relative**; the master prefixes it with the job
path at verification (§3). `outcome`/`depth` are the worker's proven values;
`root_pn`/`root_dn` are advisory (§2). Workers are stateless with respect to
the master apart from their private TT (§5): any worker can run any job.

## 9. Work-inflation mechanism table (plan2 failure modes → design elements → falsifying measurement)

| plan2 failure mode | mechanism | measured by |
|---|---|---|
| **Isolation subsidy** (sequential root proves m22 in 858k nodes; the winning child alone needs 8.59M isolated, ~10×) | master-side AND expansion at reply granularity: no worker ever solves a whole root child; jobs are bounded leaves, re-selected continuously (pseudo-MPN), so the interleaving subsidy is preserved at the master's proof state | plan5 §4: work inflation C2 vs S; attribution arms C2-nf (no re-selection) |
| **Killed-work waste** (first proven child stops everything; losers' work discarded) | master only needs ONE OR-child; refutations are cheap decisive facts that prune; locked-leaf + advisory pn/dn steer workers away from refuted/hopeless branches *before* they burn budgets | plan5 §4: inflation(C2) < inflation(C2-nf); leaf/job work histograms in the plan5 state |
| **Restart-from-zero** (fresh worker per job: plan2's 15.5× corner) | persistent workers with retained TTs (§5); re-queues of a leaf keep their TT context | plan5 §4: attribution arm C2-nr (TT reset per job) — Čížek Fig. 4 analog |

Falsification rule: if the §4 ablations do not separate
(inflation(C2) ≥ inflation(C2-nr) or ≥ inflation(C2-nf)), the design element
claimed above does **not** pay and the architecture doc must be amended to
record which mechanism actually paid before any plan7 work (plan5 gate:
attribution is gating for GO, not merely supporting).

## 10. v0 subset implemented by the plan5 prototype (conformance notes)

- Job decomposition depth: master expands root OR-children and their AND
  replies; leaf jobs solve the reply position (worker-internal depth
  unbounded below that). Deeper master-side expansion is plan7 scope.
- Direction: `evaluate` only; master decides favorability (§1).
- Verification: per-fact incremental replay at receipt + full
  `validate_proof_tree` at finalize (§3); dual-check audit on top (plan5 §4
  SOUND gate).
- File-based job store v0 (§8); checkpoint dumps implemented; restart
  exercised manually, not in the timed arms.
- No shared key database between workers (Čížek's shared TT layer is out of
  scope — constraint 5 forbids redesigning around shared memory; a durable
  shared solved-set store is plan7 scope on GO).
- No EGTB anchors, no congestion damping, no prove/disprove targeted search.

## 11. Amendments discovered by the prototype (plan5)

These amend §2/§3 above; they were discovered by running the prototype, not
assumed. They are binding for plan7.

- **A1 — worker exports must be self-contained (strengthens §2/§3).** A raw
  DF-PN event stream from a worker with a retained TT is *not* always a
  complete proof: a decisive search can resolve a node via a TT entry proven
  under an earlier job whose event stream was discarded (Draw results never
  export events), leaving holes that finalize cannot repair — the rebuilt
  subtree then contains depth-0 "leaf-ified" interior wins that the
  product's own validator flags (`terminal_mismatch`). The prototype
  measured this at a substantial rate under retention. Therefore the worker
  export path is: TT snapshot → `reconstruct` (deterministic hole-filling
  over the solved map, the product's own offline pipeline) →
  `validate_proof_tree`; only a validator-clean subtree crosses the
  boundary. On export failure the worker downgrades the result to
  unresolved (advisory only) and resets its TT, so the master's re-queue
  re-proves the leaf self-contained. Retention stays sound (§5); it is the
  *export*, not the search, that needs this discipline.
- **A2 — master merge is a hard tripwire (§3 made mechanical).** A result
  that fails the master's global-context replay + structural verification
  aborts the session with `verify_failed` (exit 3): rejected, never
  patched, never retried on the same terms. In the prototype's runs this
  tripwire never fired after A1 (it fired repeatedly before).
- **A3 — GAP-1 concretized: reconstruction fills are job-local (§4
  residual).** `reconstruct`'s hole-filling searches see only the job
  subtree's repetition context (the walk is rooted at the subposition FEN),
  not the global path prefix. A filled hole whose value depends on a
  repetition of a *pre-subposition* ancestor would be structurally valid
  and replay-clean, yet wrong globally — invisible to every structural
  validator. plan7 must either (a) give fills the global repetition prefix
  (product-side `reconstruct` extension), or (b) restrict fills to depths
  where the hazard is provably empty, or (c) re-derive filled nodes under
  global context at merge time. The dual-check audit (≥ 20 facts re-derived
  by the sequential solver) is the prototype's compensating control and
  stays mandatory.
- **A4 — the advisory pn/dn channel needs a read-only accessor.** Worker
  progress feedback reads the job root's post-run TT bounds via the new
  public `TtEntry::advisory_pn_dn()` (read-only, behavior-neutral; no
  search logic uses it). Recorded here because it is the only lib-surface
  addition the prototype needed.
- **A5 — plan6 scheduler/budget attribution: no registered feedback shape
  meets the GO band at 2 workers; the abandon trigger as registered is
  structurally inert.** plan6 ran the pre-registered variants on the plan5
  harness (shuffle, 2 workers, 5 reps each, 600 s arm cap): V0 = the
  C2-nf shape (flat 8M jobs), V1 = one-step ladder (4M→8M, feedback on),
  V2 = two-step ladder (2M→8M), V3 = V0 + the registered abandon triggers
  (leaf resolved elsewhere / root child resolved-refuted at a merge
  boundary). Results (proven reps; S = 45.95 s / 249.5M child evals):
  V0 2/5 at 0.90× wall, 1.91× inflation; **V1 5/5 at 0.63× wall, 2.64×**
  (the completeness clause exposed that its proven count is bought with
  work, not wall); V2 3/5 at 0.95× wall, 1.57×; V3 2/5 at 0.30×, 7.3×.
  Censored reps burn 9–13B evals in every arm — the bimodality plan5
  recorded persists. Findings, binding for any future campaign work:
  (1) **the abandon triggers never fired** (0 abandonments in 30 reps):
  on these positions proofs come exclusively from an all-replies-won
  child resolution — no leaf ever loses (children only refute via
  terminal classification, already known at build) and locked leaves
  cannot be resolved by another worker — so the registered trigger shape
  is provably inert and must not be shipped; a useful trigger would need
  a different deadness signal (e.g. master-side pn bounds), which is
  *not* validated by any measurement here. (2) The pre-registered ECON
  gate (5/5 proven, wall ≥ 1.2×, inflation ≤ 3×) fails for the winner
  (V1) on the wall band → **ECON NO-GO**; MARGINAL fails too (wall
  < 1.0×). (3) C4′ (V1 at 4 workers) failed both scaling criteria
  (4/5, 0.18× wall, ~26× inflation); plan5's C4 (1.29× / 1.98×) stands
  as the upper tail of a high-variance process, not a reproducible
  operating point. (4) The plan5 diagnosis ("which jobs get which
  budgets") is refined: budget ladders change the *price* of losing the
  coverage race over the root's ~41 children / 640+ open leaves (static
  priors do not identify the proof-bearing child), never its *outcome*;
  no scheduling policy within the registered family fixes that, which is
  why the iteration fired negative and plan7 is not draftable.
- **A6 — plan9 checkpoint-resume attribution: the machinery is sound, the
  accumulated state converts into zero marginal proof progress (ρ = 0);
  the pre-registered discriminator fires the resume-mechanics mode, with a
  recorded budget-cap resolution limit.** plan9 built the constraint-4
  checkpoint/resume machinery examples/-side (master-state dump v2 with
  per-leaf depth/last_worker and per-child depth/synthesized; `--resume`
  with durable-results re-drain rebuilding the fresh proof tree and
  job-locks dropped; `--job-seed` namespacing; worker `--tt-load`/
  `--tt-dump` over the public snapshot primitives, `best_child` carried
  unset — a registered fidelity limitation affecting an ordering hint
  only) and ran the four registered arms on d4d5-p2 at the incumbent V1
  shape (S1 fresh 1 h, S2/S3 warm-resume 1 h each, RC cold-resume 1 h).
  Measured: Δfacts = 30/0/0/0 across closes (leaves_won + leaves_lost +
  children_resolved), i.e. **ρ = ρ_2 = ρ_3 = ρ_cold = 0** — the item-8
  verdict (with m2 = 2.8176 locked in GO band) is **NEGATIVE**, `solve`
  dormant per the status rule. The machinery itself validated cleanly at
  every boundary: SMOKE round-trip passed all seven checks; every warm
  worker restored 2.33–2.43M solved + 1.76–1.86M unsolved records with
  table counts equal to file counts and a 100-record probe at 0
  mismatches; the re-drain re-verified ~7.2k prior-session results per
  resumed session with **0 verify failures** (A2 never fired); worker TT
  checkpoints ≈ 113–123 MB each (≈ 460 MB per close). Substrate metric
  (work-to-censor on the 12 leaves common to all four sessions' job
  sets): warm ≈ fresh per-job child-evals (ratio 1.00; both sides
  ≈ 100% pinned at the 8M-eval budget; median advisory root_dn 37k–48k
  ≪ budget) — no warm discount, which by the plan9 §4 letter is the
  *resume-mechanics* mode. Recorded caveat: the fresh S1 jobs on those
  same leaves are already 99.7% budget-pinned, so a discount could only
  manifest below the cap and the metric's resolution is bounded there;
  the positive signals (clean restore, clean re-drain, monotone state)
  show the substrate *survives* the boundary while nothing *converts*.
  The frozen-frontier degeneracy is separately confirmed on the fresh
  side (S1 = 30 facts then flat, reproducing plan8 §3 at 1 h). Reopener
  record: accumulation-as-such has no measured substrate at job-scale
  budgets on this root; the two mechanism directions that remain are
  (a) the A5.4 coverage race (master-side targeting of the proof-bearing
  child, not session structure) and (b) budget-aware early-censor
  certificates (exploit restored advisory bounds as a censor proof to
  cut re-queue cost — unvalidated; would need a worker-side change with
  its own soundness argument). Session structure (checkpoint/resume,
  job-store durability) is measured sound and stays the campaign's
  constraint-4 implementation regardless.
- **A7 — plan10 full-frontier attribution: the dispatcher-biased record
  is superseded; budget escalation is inert on the full frontier; the
  advisory-dn channel and a below-cap warm discount are measured (n=2).**
  plan10's Stage-0 sweep bypassed the master dispatcher by construction
  (driver-written jobs, 4 workers, per-leaf budget ladder 8M/32M/128M
  child-evals) and measured the full 698-leaf frontier of plan9's frozen
  state. Facts that amend the A5/A6 attribution record: (1) the
  incumbent dispatch policy touches only 104/698 open leaves (one
  affinity leaf per worker per child), and **all 38 sweep conversions
  are on never-searched leaves** — the touched 104 produced zero
  conversions at 4× budget; the cheap class is a coverage problem
  (C(8M) = 36 facts at a median ≈ 1.3k evals sit in the dispatcher's
  blind spot), the rest of the frontier a censoring problem (100%/
  100%/99.85% cap-pinned at the three rungs; ΔC = 2 over 110.9 G
  child-evals). (2) The Stage-0 gate fired **CENSORING GO**: budget
  escalation is refuted as a conversion mechanism; the registered next
  lever is Stage 2 (A6b early-censor certificates), whose substrate is
  now measured but thin — pre-rung advisory root_dn separated the two
  rung-resolved leaves from the censored mass (AUC 0.87/0.99, combined
  0.99/1.00) and the cold-control arm measured a warm-state discount
  below cap (warm marginal / cold spend 0.72/0.86; one conversion
  unreachable cold at its resolving budget) — exactly the regime the A6
  caveat recorded as unresolvable at 8M. Qualification: n = 1 positive
  per rung; any certificate design must validate the signal at larger n
  and carry the pre-registered path-scoped soundness argument. (3) M4
  remains 0/27 root-level conversions (c1g5's two open leaves censor at
  268M/260M cumulative evals); the dispatcher's per-child targeting
  (A5.4) is not gate-selected but the coverage fact above is its
  measured motivation for the cheap class. Session structure and
  machinery unchanged: the sweep ran the plan9 binaries with 0 job
  errors, 0 verify failures (A2), and 0 deviations across 2,023 jobs.
- **A6b — plan11 Stage-2a attribution: the advisory-dn censor-signal has
  no positives to validate at larger n on this frontier; Stage 2
  (A6b early-censor certificates) halts at the M2 gate — and the
  pre-registered certificate design + path-scoped soundness contract are
  recorded here as the stage's gate material, not as a plan12 input.**
  plan11 ran the registered stratified corpus over the plan10 merged
  close's 660 open leaves (H = 24 top committed-advisory-dn, force
  including the c1g5 near-misses; M = 12 and L = 12 at the touched set's
  ~55–75th / ~15–35th committed-dn percentiles; U = 8 untouched control)
  through a warm budget ladder 256M / 1G / 2G child-evals per leaf per
  rung (R1′ fresh, R2′/R3′ warm `--tt-load`; the contingent-R3′ rule
  fired at 1.14 h elapsed + 2.03 h projected ≤ 4.5 h; locked κ = 27.5,
  R_camp = 558,529; observed κ ≈ 26.7–26.9, ≈ 670k nodes/s). Measured:
  **P = 0 positives at the full ladder** — all 56 leaves censored at
  every rung, 100% cap-pinned at all three budgets (the A6b lever's
  ≈100% re-burn premise re-confirmed on the certificate mechanism's
  actual target population), 182.3 G child-evals ≈ 6.6 G nodes in 2.81 h
  ladder wall, 0 job errors, 0 deviations, 0 oom, restore-probe 0
  mismatches, SMOKE 8 leaves × 100k × 2 phases clean. The registered M2
  gate fired **SUBSTRATE_EMPTY**: no AUC/θ/S(θ) is computable, the n=2
  substrate of A7 is not validated, and plan12 (the certificate
  implementation) is not draftable. Two covariate findings for any
  future revisit: (i) the two A7 positives converted at ≤ 128M warm
  budgets; at plan11's deeper ladder their budget class is already
  resolved out of the open frame, and the remaining frontier converted
  nothing in 182 G evals — consistent with A7's ~1 conversion per
  ≈ 169 G projection, i.e. the predicted-positive mass is too sparse to
  validate any predicate at affordable n; (ii) in the warm regime (the
  certificate-consumption regime) the committed-dn stratification
  partially flattens (R3′ pre-rung medians: H 70k vs M 29k / L 25k /
  U 32k — M/L/U overlap), so the advisory channel does not preserve the
  committed-dn ordering that strata selection relied on. Stage 1
  (A5.4 coverage-race targeting) remains the initiative's registered
  direction; any future certificate revival must start from a
  conversion-rate step-change, not from re-measuring this corpus.
  **Pre-registered certificate (v1) + path-scoped soundness contract
  (written before the run; recorded verbatim as the halt record):** a
  worker-produced record attached only to *censored* results for job
  (global path P, budget B) — path_digest (SHA-256 of the canonical
  global path, replay-verifiable), budget_evals, pre_advisory (pn, dn
  restored at job start), post_advisory, evals_spent, lineage_digest
  (SHA-256 of the worker's TT dump at the session boundary the job ran
  in), session_id/worker_id/job_seed, outcome = Censored. It may license
  exactly one thing: the master's spend-aware re-queue may skip
  re-queuing leaf L at budget ≤ B on lineage (w, ℓ) when a certificate
  at (L, B, w, ℓ) exists and the plan12-validated predicate holds; it
  may not change L's status class, compose into proof state, gate any
  A2-verified fact, suppress escalated budgets (certified leaves are
  first in the escalated queue), or apply across workers/lineages; it
  claims nothing about L's value, budgets > B, other lineages, or other
  paths. Soundness argument: (1) composition surface unchanged —
  certificates gate work allocation only; the artifact pipeline
  (per-fact A2 verification at merge; final replay validation) never
  sees one and no new fact class exists; (2) path scope — the claim is
  indexed by the exact job path P (campaign root → leaf line, clock and
  repetition context included), re-derived by replay at consumption,
  rejected on mismatch; no repetition-dependent result crosses any
  boundary (none is produced); (3) lineage scope — the prediction is
  conditioned on the worker's TT state at lineage ℓ; session-boundary TT
  dumps are content-addressed and a consumed certificate must match the
  consuming session's restored dump digest; cross-job TT pollution
  inside ℓ is deliberately included in the claim's scope (the claim is
  about the lineage as a whole — what a warm re-queue actually runs on),
  which is why the scope must not be widened; (4) GHI — journals keep
  the standing contract, certificates cache no search result, the skip
  is a scheduling decision; the hazard class is exactly the plan9
  machinery's, measured sound; (5) fail-open always — any verification
  failure, missing field, digest mismatch, or unknown-certificate
  condition re-queues the leaf normally (suppression is never the
  default); (6) yield risk (not soundness) — false censor handled by
  the M2 gate, plan12's mandatory on-line probe (every 16th consumption
  not honored, runs at full budget, auto-disable above 1/16), and
  escalation priority. Verifier (would-be plan12 deliverable): path
  replay legality from the campaign root (A2 discipline, reused);
  recorded pre_advisory equals the master's per-leaf advisory record;
  digest checks; outcome class = Censored and evals_spent ≤ budget;
  session/worker/seed well-formed; any failure → rejected, fail-open;
  mandatory corrupted-certificate tests (altered advisory dn, altered
  path/digest, altered budget or spent-evals overclaim, lineage-digest
  mismatch) each asserting rejection.
