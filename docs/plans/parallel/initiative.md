# Initiative: `parallel` — parallel proof search

## Status

Opened 2026-09-21 as the consolidated home for the parallelization effort,
absorbing `conversion` backlog #4 and `lean` backlog #2 (joint ownership
split across two initiatives until now). The next plan number is **plan1**.

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
| 2 | **Design-space note: architecture selection** | Weigh options A–D above against the design constraints; include the API/consumer story (opt-in `--threads N` vs a process-portfolio harness vs both). Sizing spike allowed per lean's cadence (temporary instrumentation, reverted) | selects the 2–8× lever's shape | — | M | open — plan1 synthesis input: C first (harness over unmodified processes), A behind it (mine Pawlewicz & Hayward 2014 first), B as adjunct, D dead (shared-DB payload unsafe here); spike order: C tail/`child_evals` inflation at 4–16 workers on m22/shuffle-win, decisive-composition agreement, N×TT RAM |
| 3 | **Staged implementation** | Gated on #2 GO; one lever per plan; sequential drift protocol green at every stage; parallel mode documented as nondeterministic (or not shipped, per #2) | the actual speedup | wall | L–XL | blocked by #2 |

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
