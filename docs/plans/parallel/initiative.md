# Initiative: `parallel` — parallel proof search

## Status

Opened 2026-09-21 as the consolidated home for the parallelization effort,
absorbing `conversion` backlog #4 and `lean` backlog #2 (joint ownership
split across two initiatives until now). The next plan number is **plan1**.

**2026-10-03, owner decision — gate re-registered; initiative reopened for
plan4b/plan5.** The plan4 kill verdict stands unchanged as the measured
record; the owner consciously accepts the measured TT-concurrency tax
(+4.4 % / +6.0 % hard-class wall) as the entry fee for the A-stage — the
only remaining multiplicative lever (all others measured no-go). Three
conditions are pre-registered with the acceptance:

1. **Conditional acceptance (revert-if-missed).** The tax is accepted
   *only while* plan5 proves out: if the SPDFPN prototype misses its GO
   bands (≥ 2.0×/4 threads, work inflation ≤ 2×, zero soundness
   violations), the refactor is reverted again and the tax goes with it.
2. **Explicit gate, not a waiver.** The plan4 gate (≤ +2 % wall) is
   replaced by a hard budget: **≤ +7 % median hard-class wall** (m22 /
   shuffle-win first-outcome, vs the unsharded baseline) with the
   byte-identity surface unchanged (quick-suite `child_evals`, stdout,
   snapshot bytes). Future TT work may not erode the sequential path
   silently beyond this budget.
3. **Measurement honesty.** The reference container exposes 4 CPUs: wall
   measurements at 2–4 threads are real; N ≥ 8 are simulated only. The
   conservative band at 4 threads (~1.8×) sits at the GO-band edge — a
   marginal result is a realistic outcome, not a surprise.

Next: **plan4b done (stage 1b re-landed, tax pinned +5.36 %/+4.61 % vs
the ≤ 7 % budget, `report4b.md`)**; **plan5 done (stage 2a mechanism built,
smoke sound but 1.42×/4 threads — `report5a.md`)**; **plan5b next** (W sweep,
campaign, GO/NO-GO + condition-1 revert-if-missed).

**2026-10-03, plan4 kill point fired — initiative closed (backlog #4 NO-GO).**
The inert TT-concurrency refactor measured +6.0 % (m22) / +4.4 %
(shuffle-win) first-outcome wall vs the pre-registered ≤ +2 % gate —
with zero deterministic drift — and no in-scope mitigation recovered it
(profiled; three variants tried; seqlock redesign scoped and rejected).
The refactor was reverted; the A-stage dies at the cheap kill point as
pre-registered. Details: `report4.md`, `measurements/plan4/`.

**2026-10-02, premise change — initiative reopened for option A.** The
product consumer now accepts nondeterministic parallel runs
(nondeterminism confined to *which* valid proof wins, never a false
decisive outcome). This satisfies the precondition that plan2's NO-GO
deliberately deferred: option A (thread-level shared-TT parallel DF-PN)
was gated on mining Pawlewicz & Hayward 2014, now mined (plan3,
`research_spdfpn.md`): mechanism fully specified (W-threshold
interruptible jobs, virtual win/loss steering over a per-ply virtual TT,
shared-TT discipline), serial base identical to ours (dfpn 1+ε),
measured 0.74 efficiency /16 threads with work inflation ≤1.40× — the
exact gap that killed option C. The option-C no-go and the lean plan7
deterministic no-go stand untouched. Next: backlog #4 (plan4 inert
TT-concurrency refactor = cheap kill point; plan5 prototype with
pre-registered GO bands).

**2026-09-21, plan2: architecture selection closed as measured NO-GO.**
The option space is now closed by measurement on both sides: the
shared-state architectures (A/B/D) are excluded by soundness/economics
(plan1 mining; `design_space.md` §1), and the only constraint-satisfying
shape — option C, a harness over unmodified solver processes — measured
**0.27× wall speedup** (a 3.7× slowdown) at 4 workers on m22 with
15.5× work inflation, far outside the pre-registered GO/MARGINAL bands
(`design_space.md` §6, `measurements/plan2/`). The mechanism is
structural: per-child process isolation forfeits the root search's
cross-child transposition sharing and best-first proof interleaving
(858 k nodes for the whole root vs 8.59 M for the winning child alone).
No opt-in parallel mode ships; the initiative moves toward closure
(backlog #3 never unblocks). The Pawlewicz & Hayward 2014 bibliography
entry stays **Open** for a hypothetical future A-stage.

The recorded evidence stands and is not reopened: deterministic parallelism
is a **measured no-go** (`lean` plan7 spike — sibling-parallel ceiling
1.47–1.48×, below the ~1.5× bar; the AND early-exit is already harvested
sequentially). What this initiative opens is the remaining space:
opt-in parallelism whose nondeterminism is bounded and documented, and
architectures that do **not** rely on a shared mutable TT at all.
`lean` report7's reopen triggers are consciously renegotiated by this
opening: trigger 2 (algorithmic levers exhausted without ~2× work
reduction) is now satisfied — `dfpn` is dormant and `conversion` #6/#7
closed as measured no-gos, leaving parallelism as the only multiplicative
lever on the roadmap. Trigger 1 (a consumer accepting nondeterministic
runs) is deferred to the design note: whether an opt-in nondeterministic
mode ships at all is a plan1 output, not a precondition.

## Motivation

The solver is sequential and single-threaded end to end. All other lever
families are measured out at the diagnostic level (`research`/`lean`/
`conversion` no-go records; `research/structural_floor.md` §9). Parallelism
is the one lever with a multiplicative wall-time ceiling (2–8× on
multicore hardware), and the only one that does not touch DF-PN threshold
arithmetic or TT cacheability semantics — its risks are concurrency and
GHI soundness instead.

Prior mining: Kaneko AAAI-10 is fully extracted
(`dfpn/research_parallel.md`, implementation guide with virtual pn/dn
congestion terms, Mark/Unmark agent tracking, shared stop-set, per-node TT
locks; ~3.6× on 8 threads in the paper). Its 15-year-old scaling
assumptions are superseded by the 2025 massively-parallel PNS work and
JLPNS — both unmined (see backlog #1).

## Goal

Make wall time on hard searches scale with available cores without ever
returning a false decisive outcome, by (a) mining the remaining parallel-PNS
literature, (b) selecting an architecture that fits this solver's
soundness contracts, and (c) staged implementation behind an explicit
opt-in. Priorities follow AGENTS.md: correctness first, then performance,
memory, maintainability.

## Design constraints (carried over, normative)

1. **Sequential path stays bit-identical.** Any parallel mode must be a
   strict opt-in (`--threads N` or equivalent); with the option off, every
   stdout byte, TT trajectory, and `child_evals` count matches today.
   The lean drift protocol applies unchanged to the sequential path.
2. **No false decisive outcomes.** Any candidate must state why it can
   never prove a non-proof (same bar as `conversion`'s non-goal).
   Cross-worker TT reuse makes the `dfpn` journal-GHI contract
   (plan10 hazard class; `dfpn/research_ghi_journal.md`) load-bearing:
   repetition-dependent results must never be shared/cached across worker
   contexts unless the reuse argument survives cross-path composition.
3. **Budget determinism contract.** `child_eval_budget` exhaustion →
   `Draw` + `ExitReason::BudgetExhausted` is a documented deterministic
   surface; a parallel mode must either preserve it or explicitly
   re-document it as nondeterministic under `--threads > 1`.
4. **RAM accounting.** The search CLI's "RAM = TT only" contract must be
   restated, not silently broken, if an architecture multiplies TTs
   (e.g. per-process private tables).

## Architecture options (the design-space note, backlog #2)

Not decided yet — plan1/plans decide. The honest option set:

- **A. Thread-level shared-TT lazy-SMP** (`--threads N`): workers run
  DF-PN over one locked/sharded TT. Kaneko's virtual-pn congestion terms
  are the published cooperation protocol. Highest expected speedup on
  paper, highest engineering risk, makes constraint 2 load-bearing, and
  the CLI surface is exactly the parked `--threads N`.
- **B. Process-level portfolio**: N independent solver processes (each
  today's sequential solver, own TT) racing with different parameterizations
  (ε, ordering tweaks, TT size); first decisive result wins. No shared
  mutable state — soundness is inherited per worker, constraint 2 vacuous.
  Nondeterminism confined to *which* valid proof wins, never a false one.
  Memory N×TT; buildable almost entirely outside `src/` (examples/
  harness). Plausible precisely because trajectory chaos makes diverse
  parameterizations fail independently (`conversion` report8: dec01 5.7M
  evals at ε=0.125 vs 210M unfinished at ε=0.375).
- **C. Root/child split, per-worker TTs**: distribute the root's children
  across workers. AND-side (all children needed) combines deterministically;
  OR-side races children (nondeterministic which proof is found). Clean for
  the `find_winning_child` / EGTB workloads; work-wasteful vs the
  sequential early-exit for single-root solving.
- **D. Two-level/shared-worker protocols (Čížek 2025)**: the massively
  parallel PNS design; subject of the reading round before any comparison
  is meaningful.

**Cross-paper comparison (plan1 reading round, 2026-09-21;**
`research_cizek2025.md`, `research_jlpns.md`, `research_solrex.md**):**
All three papers are silent on repetitions under concurrency (Sprouts /
Breakthrough / Hex are position-monotone) — constraint 2 has no published
soundness argument anywhere in this literature; the journal contract
(`dfpn/research_ghi_journal.md` §5) stays load-bearing for every option.
What the evidence does settle: (1) **C is the strongest survivor.** The
only near-linear small-pool (4–16 core) measurements in the literature
(Čížek Table 1: 4.3×/4, 16.2×/16; PPN₂: 7.2×/16) come from job-level
decomposition over *persistent private-state workers with no shared TT*
— in Čížek's own data the shared-TT second level (`threads > 1`) only
engages at ≥128 cores, and PPN₂'s clients never share state at all. This
is exactly our option C's shape, implementable as a harness over
unmodified solver processes (sequential path bit-identical by
construction; constraints 1/2 vacuous at the worker level; the master's
decisive-fact combination is rule-sound, worker Draws consumed as
unproven). (2) **B is a degenerate adjunct, not the main lever**: JLPNS/
PPN₂ is *not* a portfolio — central coordination and re-direction are
where its speedup comes from, and Čížek's ablation shows discarding
worker state between jobs is catastrophic (PNS-PNS falls below
sequential). (3) **A is real but second**: Solrex/SPDFPN delivers the
best few-thread shared-TT figure published (≈5.2×/4 threads, superlinear
caveat; 11.8×/16 per Čížek's citation), but its mechanism lives in an
unmined source (Pawlewicz & Hayward 2014), no work-inflation figure is
reported, and sharing a mutable TT makes the journal contract bind at
every TT site (L–XL effort: the inert TT-concurrency refactor first).
(4) **D is dead as a distinct option**: Čížek's master-level shared
database — its biggest scaling lever — carries path-independent key
results (Grundy numbers); our only rule-path-independent class (decisive
Win/Loss) is banned from cross-worker reuse (plan10 non-goal) and our
draws are path-dependent, so the database has no safe payload here; the
rest of D reduces to C (first level) plus A (second level, idle below
128 cores). Recommended shape for plan 2 to test: **C first (harness,
S–M effort), A behind it as an opt-in second stage**, B only as a cheap
parallel adjunct. **Plan-2 sizing spike, in order**: (i) worker
tail/utilization and `child_evals` inflation (vs sequential) for C at
4–16 workers on m22/shuffle-win — the OR-early-exit tail is the
unmeasured risk; (ii) master-side decisive-composition agreement (does
an isolated worker solve of each root child reproduce the sequential
child outcome — extends the two-solves-in-one-process property test);
(iii) RAM at N×TT (constraint 4 restatement); (iv) only if A advances:
mine SPDFPN 2014 and measure a 2-thread drift-safe prototype.

## Backlog

| # | Item | Mechanism | Potential | Affects | Effort | Status |
|---|------|-----------|-----------|---------|--------|--------|
| 1 | **Reading round: parallel PNS (2025-era)** | Mine the three open bibliography entries into `research_*.md` here: (a) Čížek, Balko, Schmid 2025 (*Massively Parallel Proof-Number Search*, arXiv:2511.10339 — two-level parallelization + shared worker info, 333× on 1024 cores); (b) Saffidine, Jouandeau, Cazenave 2011 (JLPNS, ACG-13); (c) Young, Hayward 2016 (*A Reverse Hex Solver*, scalable parallel DF-PN, Solrex). Extract per paper: cooperation protocol, TT sharing/sharding model, GHI/repetition handling under concurrency, scaling regime (where the gains saturate on 4–16 cores, not 1024) | information (feeds #2) | — | S | **mined 2026-09-21** (plan1): `research_cizek2025.md`, `research_jlpns.md`, `research_solrex.md` (+ vendored PDFs); all three silent on repetitions under concurrency (monotone domains) — the finding; small-pool evidence favors C over A; D's shared-DB layer has no safe payload here |
| 2 | **Design-space note: architecture selection** | Weigh options A–D above against the design constraints; include the API/consumer story (opt-in `--threads N` vs a process-portfolio harness vs both). Sizing spike allowed per lean's cadence (temporary instrumentation, reverted) | selects the 2–8× lever's shape | — | M | **closed 2026-09-21 (plan2): measured NO-GO** — `design_space.md` + `measurements/plan2/`: option C (root-child race over unmodified processes) measured 0.27× wall (3.7× slowdown) at N=4 on m22, 15.5× work inflation, zero soundness violations; deficit structural (forfeited cross-child TT sharing/interleaving); ship-shape verdict recorded (harness-only, moot); no parallel mode ships |
| 3 | **Staged implementation** | Gated on #2 GO; one lever per plan; sequential drift protocol green at every stage; parallel mode documented as nondeterministic (or not shipped, per #2) | the actual speedup | wall | L–XL | **closed 2026-09-21** — unblocking condition (#2 GO) failed on the pre-registered rules; no staged implementation exists to stage |
| 4 | **A-stage: SPDFPN-style shared-TT parallel DF-PN** | Reopened by the 2026-10-02 nondeterminism premise; reference design = `research_spdfpn.md`. Stage 1 (plan4/plan4b): inert TT-concurrency refactor — sharded TT behind interior-mut locking, byte-identical N=1 behavior (gate re-registered 2026-10-03: ≤ +7 % median hard-class wall, byte-identity unchanged). Stage 2 (plan5/plan5b): `--threads N` prototype — W-threshold + same-child resume clause, virtual TT + job lock + `TRYRUNJOB`, thread-local repetition cache, serialized ProofEvents; measured on the hard class (wall speedup, work inflation, outcome agreement) with pre-registered GO bands; **tax acceptance + refactor revert-if-missed** | 2–8× wall on hard solves (expected ≈ 2.9–3.2× at 4 threads; conservative band ~1.8× at the GO edge) | wall | L–XL | **stage 2a built (plan5, 2026-10-04; `report5a.md`)**: `--threads N` landed — coordinator runs the unmodified sequential chunk loop, N−1 SPDFPN helper workers pre-warm the shared TT (declared architecture deviation: the pure worker pool could not converge on the hard class; W = 20 000 child evals, path-derived thresholds, virtual steering, retirement). N = 1 drift zero everywhere (quick suite 59/59, stdout + snapshot hashes identical to plan4b). Smoke (m22, 5 interleaved reps): **outcome agreement 15/15, zero panics**; median speedup **1.42×/4 threads, 0.92×/2 threads**, inflation ~2.2–2.4×, high t2 variance — below the GO bands, verdict belongs to plan5b's campaign. Stage 2b = plan5b (W sweep, campaign, GO/NO-GO, condition-1 revert-if-missed). Measurements: `measurements/plan4/`, `plan4b/`, `plan5/` |

## Non-goals

- **Reopening deterministic parallelism** (measured no-go, `lean` plan7:
  1.47–1.48× ceiling on m22/shuffle-win; the AND-node early exit is
  already harvested sequentially).
- **Cross-worker reuse of solved Win/Loss facts** (plan10 hazard class,
  `research/structural_floor.md` §2) — a parallel variant of the same
  mistake inherits the same no-go.
- **Default-on parallelism**: the sequential solver remains the default;
  the drift-gated product never depends on thread scheduling.
- Unsound speed: any design that cannot state why it never returns a
  false decisive outcome is not implementable here.

## Measurement conventions

- Primary metric: **wall time** to first decisive outcome on the
  m22/shuffle-win class (the same validation cases as `lean` plan7).
- Secondary: total `child_evals` (parallel overhead = search-work inflation
  vs the sequential baseline, the Kaneko paper's "<15% overhead" measure).
- Sequential-path gate: bit-identical stdout on the quick suite and the
  m22 first-outcome/default captures, unchanged from `lean`'s protocol.
- Parallel-mode reporting: N, per-worker work split, wall speedup vs N=1,
  and outcome agreement across repeated runs (no `wrong` ever).

## History

- **2026-10-04 — plan5 complete: stage 2a mechanism built, smoke sound, speed
  below GO bands (`report5a.md`)**: `--threads N` (N > 1) landed over the
  sharded TT — with one declared architecture deviation: the pre-registered
  pure worker pool could not converge on the hard class (instrumented
  diagnosis: fine-grained jobs re-verify saturated regions without advancing
  the global proof-number state), so the landed shape runs the unmodified
  sequential chunk loop on the coordinator while N−1 SPDFPN helper workers
  (W = 20 000 child evals, path-derived thresholds, virtual win/loss
  steering, zero-progress retirement, serialized events, per-worker
  repetition caches) pre-warm the shared TT. N = 1 byte-identity re-proven
  (quick suite 59/59, m22/shuffle stdout + snapshot hashes identical to
  plan4b). Smoke: outcome agreement 15/15, zero panics; 1.42×/4 threads,
  0.92×/2 threads, inflation 2.2–2.4× — below the pre-registered GO bands;
  plan5b's W sweep + interleaved campaign decide the verdict (condition-1
  revert-if-missed stands). Kill-point check: no decisive disagreement, no
  panic/torn state — the prototype stays alive. Measurements:
  `measurements/plan5/`. No `docs/plans/README.md` row edit (initiative
  neither opened, pivoted, nor closed).

- **2026-10-04 — plan4b complete: sharded-TT refactor re-landed, tax pinned, gate PASS**
  (stage 1b of backlog #4; `report4b.md`): the preserved plan4 attempt was
  applied verbatim (`git apply` + recreated `shard.rs`, header stripped);
  `cargo fmt`/`clippy`/`doc` clean, `make test` green, working-tree scope
  exactly the recorded 14 files + `shard.rs`. Drift protocol zero
  everywhere: quick suite 59/59 bit-identical, m22/shuffle-win stdout
  byte-identical, snapshot dump sha256 `eaa5f2b9…`/195 B identical to the
  plan4 record, golden test ok. Tax pinned by interleaved A/B medians:
  **m22 +5.36 % (10 pairs) / shuffle-win +4.61 % (3 pairs)** — inside the
  ≤ +7 % budget, consistent with plan4 (+6.0 %/+4.4 %; Δ ≤ 0.64 pp, no
  profiling trigger). One deviation: the revived 4-thread stress test
  flamed a latent race **in the test itself** (concurrent eviction +
  own-phase-2 refill misread as a downgrade; flaky 4/5 in isolation);
  fixed test-side only, solver code identical to the plan4 attempt.
  Measurements: `measurements/plan4b/`. Next: plan5 (stage 2a mechanism).

- **2026-10-03 (later the same day) — owner decision: A-stage reopened,
  gate re-registered (docs only, no code)**: the owner accepts the
  measured ~5 % TT-concurrency tax as the entry fee for the only
  remaining multiplicative lever, with three pre-registered conditions:
  (1) acceptance is conditional — plan5 missing its GO bands reverts the
  refactor and the tax; (2) the plan4 gate (≤ +2 %) is replaced by an
  explicit ≤ +7 % median hard-class wall budget with the byte-identity
  surface unchanged; (3) wall measurements at 2–4 threads are real (4-CPU
  container), N ≥ 8 simulated only. Speedup estimate table (2–10 threads,
  literature-anchored: SPDFPN 0.74 eff/16, Solrex ≈5.2×/4) recorded in
  the session record; expected ≈ 2.9–3.2×/4 threads, conservative band
  ~1.8× at the GO edge. Backlog #4 row updated; plan4b (re-land + pin the
  tax) and plan5 (prototype) to be written by the next session.

- **2026-10-03** — **plan4 complete: inert TT-concurrency refactor → NO-GO at the
  pre-registered kill point; backlog #4 closed** (code reverted; docs +
  measurements committed): the sharded, interior-mut, `Send + Sync`
  `TranspositionTable` was fully implemented and behaviorally inert
  (quick-suite `child_evals` bit-identical per case, m22/shuffle-win
  stdout byte-identical, snapshot dump byte-identical, `make test`
  green, new concurrency-invariant tests included), but the wall gate
  failed reproducibly: **+6.0 %** m22 / **+4.4 %** shuffle-win median
  first-outcome vs the ≤ +2 % threshold. Profiled per the plan's risk
  protocol (`perf record`): the RMW pair is only 0.7 % — the cost is the
  lock discipline at child-eval granularity (~15–20 probes/node,
  ~15 M probes on m22; 48 B copies, non-forwardable loads, clobber
  re-loads). `#[inline]`, unchecked indexing (probe inlined, no gain)
  and `-Ctarget-cpu=native` (LSE) were each wall-neutral; a seqlock
  redesign (lock-free readers, atomic entry fields) was scoped and
  rejected: still projects ≈ +2–3 % and adds version-retry storms under
  plan5's multi-writer contention — worse for the target workload. The
  refactor was reverted to the byte-identical sequential solver (clean
  `git diff`, gate re-run green); the full attempt is preserved in
  `measurements/plan4/sharded_tt_attempt.patch` + `shard_rs_attempt.rs`.
  Report: `report4.md` (includes the reopen triggers: a consumer-facing
  GO re-weighing of a ~5 % sequential tax against the parallel band, or
  a mechanism redesign with a re-registered gate — owner's call).
  `docs/plans/README.md` row updated (close event).

- **2026-10-02** — **initiative reopened; plan3 complete: SPDFPN 2014
  mined, option-A GO input** (docs only, no code): premise change
  recorded (nondeterministic parallel runs accepted); paper located via
  author copy after the bibliography's arXiv pointer proved wrong
  (arXiv:1503.07698 resolves to an unrelated XENON1T paper — entry
  corrected, flipped **Open → Mined**); extraction vendored under
  `docs/theory/spdfpn-2014/`; `research_spdfpn.md` written (mechanism,
  measurements, soundness mapping, bill of materials, staged plan
  skeleton); backlog #4 opened. `docs/plans/README.md` row updated
  (reopen event).

- **2026-09-21** — **plan2 complete: architecture selection → measured
  NO-GO** (no `src/`/`examples/` change; spike drove the release binary
  as a black box): `design_space.md` written (A–D × constraints matrix,
  ship-shape space, measured cells, decision); sizing spike executed in
  the pre-registered order — perspective-mapping validation vs
  `find_winning_child` (exact agreement, dec08/dec03), sequential
  baselines (m22 2.63 s/858 k nodes; shuffle-win 47.8 s/13.9 M),
  per-child tables (43+41 children, 120 s caps), races at N=2/3/4 ×
  5 reps (0.29×/0.28×/0.27×; 11.6–15.5× inflation; zero decisive
  disagreements), shuffle-win probe (race cannot reproduce the root
  proof), RAM (≈230 MB/worker; N×TT restatement vs the enforced 8 GiB
  cgroup), N=8/16 schedule simulations (0.30×, N- and order-insensitive;
  labeled simulated). Bibliography: Pawlewicz & Hayward 2014 added as
  **Open**. Backlogs #2/#3 closed; `docs/plans/README.md` row updated
  (close event). Finding that closes the door: root-level cross-child
  transposition sharing + best-first interleaving are worth ≈10× on
  m22 and are structurally inaccessible to process-per-child workers.

- **2026-09-21** — **plan1 reading round complete** (docs only, no code):
  backlog #1 mined — `research_cizek2025.md`, `research_jlpns.md`,
  `research_solrex.md` written in this directory (PDFs vendored under
  `docs/theory/`);
  cross-paper comparison paragraph added to the option-space section;
  backlog #2 enriched with the synthesis pointers and spike order.
  Bibliography: Čížek 2025, JLPNS, Young 2016 flipped **Open → Mined**.
- **2026-09-21** — **initiative opened; housekeeping handover** (docs
  only, no code): `conversion` #4 and `lean` #2 (parked dormant after the
  plan7 spike) consolidated here. Bibliography pointers for the three
  open parallelism entries (JLPNS, Young 2016, Čížek 2025) re-targeted to
  backlog #1. Reopen-trigger renegotiation recorded in Status. Kaneko
  mining stays where it was mined (`dfpn/research_parallel.md`); the PDF
  remains at `docs/theory/pdfpn-2010/pdfpn-2010.pdf`.

Per repo convention, every plan ends with the task of writing its
`report<N>.md` in this directory.
