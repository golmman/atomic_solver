# Research: Čížek, Balko, Schmid 2025 — *Massively Parallel Proof-Number Search for Impartial Games and Beyond*

Mining round for `parallel` backlog #1 (plan1). Terminology (virtual pn/dn,
congestion term, stop-set, worker, TT discipline) is reused from
`dfpn/research_parallel.md` (Kaneko AAAI-10), not redefined. The GHI
soundness template is `dfpn/research_ghi_journal.md`. The plan5 full-text
check (recorded in `docs/bibliography.md`) — no child-level
cutoff/widening content, parallel-layer only — is confirmed by this
mining: the Grundy-number machinery is domain-level, all parallel
content is in §"PNS-PDFPN Algorithm".

## 0. Version read

- **Primary**: T. Čížek, M. Balko, M. Schmid, *Massively Parallel
  Proof-Number Search for Impartial Games and Beyond*, arXiv:2511.10339
  **v2** (9 Feb 2026), 19 pages; copyright line AAAI-26 (published there).
  Vendored at `docs/theory/pns-pdfpn-2025/pns-pdfpn-2025.pdf`. Open access.
- Companion code: `github.com/cizektom/spots` (C++20 core, Python/Ray
  first-level distribution, pybind11 bridge) — the paper's §"Code and
  Data"; not mined source-level, but the paper's parameter appendix
  covers what the experiments used.

## 1. The two-level decomposition (§"PNS-PDFPN Algorithm")

The algorithm, **PNS-PDFPN**, parallelizes Proof-Number Search at two
independent levels:

**First level (distributed memory, job-level).** A **master** process
maintains the current proof state — a best-first PNS tree in the paper's
NAND (negamax AND/OR) formulation — and repeatedly assigns **jobs** to
**workers** over a cluster interconnect. A job is a **pseudo-MPN leaf**
ℓ of the master tree; the worker processes ℓ asynchronously until it is
solved or a per-job expansion cap (`iterations`) is reached. The worker
returns the proof/disproof numbers of ℓ **and its children**; the master
expands ℓ in the master tree from that payload. Workers send periodic
updates of `pn(ℓ)`/`dn(ℓ)` every `updates` expansions so the master tree
stays fresh. Assigned leaves are **locked** (never re-assigned), and the
pn/dn definition is "slightly adjusted to avoid misleading attraction to
the locked leaves" (§ First-Level; the paper delegates the exact
adjustment to Wu et al. 2013 — the job-level virtual-loss lineage). The
job loop is explicitly built on Job-Level PNS (Wu et al. 2011/2013) "as
used by Schaeffer et al. (2007)" (Checkers) and Saffidine et al. 2012
(PPN₂).

**Second level (shared memory, within one worker).** Each worker runs
**PDFPN — Kaneko's parallel DFPN** (§ Second-Level, cited as "the
parallel DFPN algorithm by Kaneko (2010)"): one thread per core,
each thread an independent DFPN search in the subtree of the worker's
assigned leaf ℓ, sharing a **lock-protected transposition table of
proof and disproof numbers**. Redundancy control is exactly Kaneko's:
disproof numbers are **virtually increased during child selection by the
number of threads currently computing in the corresponding subtree**
(the congestion term `T` of `dfpn/research_parallel.md` §4), applied as
`th(w)` subtracted from the `dn(w′)+1` term of the threshold formula.
Solved-position notification (the stop-set analog): "if a thread solves
a position currently computed by a different one, the other thread is
notified to backtrack to that position."

## 2. The "shared worker info" (§ First-Level, § Retaining and Sharing)

There are two distinct shared-information mechanisms, at different
levels:

1. **Master-level key-result database.** The master "maintains a
   database of key computed results shared with all workers to reduce
   search overhead" (pattern: Liang/Wei/Wu's JL-UCT). New results are
   sent to workers *with each assigned job*; workers report newly
   derived results back with each update. Workers grouped on one node
   (`grouping` parameter) share a local copy of the database. For
   impartial games the **key results are Grundy numbers**: "we use all
   computed Grundy numbers as the key shared results, since they are
   highly reusable and inexpensive to share." Consistency guarantee:
   a Grundy number is a domain-theoretic invariant of a position
   (Sprague–Grundy theorem; the paper's Theorem 1) — it is
   **path-independent by construction**, so concurrent sharing needs no
   soundness argument beyond the domain theory. The paper never frames
   it this way, but this is the load-bearing property of the whole
   sharing design.
2. **Worker-level TT.** The lock-protected pn/dn TT shared by a
   worker's threads (PDFPN), plus the `th(w)` congestion counters — the
   same per-node lock + virtual-number structure as Kaneko (see
   `dfpn/research_parallel.md` §4–5). Crucially, the paper's ablation
   (§ Retaining, Fig. 4/8) shows worker TT **retention across jobs** is
   a first-order effect: PN₂-style workers that discard their search
   tree after each job ("PNS-PNS") scale to 64 cores then fall **below
   the sequential DFPN baseline**; retaining computed key results
   (Grundy numbers) recovers large gains; retaining pn/dn via DFPN
   workers adds a further, *smaller* gain; sharing (synchronizing)
   Grundy numbers adds the biggest one.

Comparison with Kaneko: the second level *is* Kaneko's design,
unchanged (per-node locks, congestion term, solve notification). The
novelty is entirely at the first level — a coordinator distributing
frontier jobs to stateful workers, plus a shared database whose payload
is restricted to path-independent facts.

## 3. The repetition question

Unaddressed — and structurally absent from the domain. The benchmark
games (Sprouts; impartial games under normal play) "end after a finite
number of moves, and there is no draw" (§ Sprouts Game); positions are
monotone (spots/lines only accumulate), so no repetition semantics
exists. No GHI discussion appears anywhere in the paper; the shared
database is safe precisely because its payload (Grundy numbers) is
path-independent by the Sprague–Grundy theorem.

**Finding (load-bearing):** the paper offers no soundness argument for
concurrent caching of path-dependent results under cross-worker
composition, because its domain has none. The journal-GHI contract
(`dfpn/research_ghi_journal.md` §5: with multiple workers sharing one
TT, cross-worker path contexts differ *by construction*) remains ours to
carry. In particular, the *shape* of Čížek's shared database — share
only results that are path-independent by rule — transfers, but our
only rule-level path-independent class (decisive Win/Loss facts) is
excluded from cross-worker reuse by the initiative's non-goal (plan10
hazard class), and our draws are path-dependent. So in our domain the
database has no safe payload; see §6.

## 4. The scaling curve and the 4–16 core regime

Headline (§ Experiments, Fig. 5, Table 1): **332.97±26.8× on 1024
cores** with the child-ordering heuristic, **208.67±9.17×** without,
relative to sequential DFPN (5·10⁷ TT) on the 47-spot position. Prior
best: PPN₂ 18.3×/64 cores; SPDFPN 11.8×/16 cores; Kaneko PDFPN
3.58×/8 cores (all as quoted in the paper's Related Work).

Overhead sources and ablations (Fig. 4–5, 8–11, Tables 1, 3):

- **State resets are catastrophic.** PNS-PNS (worker state discarded per
  job) is outperformed by *sequential* DFPN despite parallelism — search
  overhead from recomputation dominates everything. This is the
  single most transferable negative result.
- **Sharing path-independent key results is the largest win** (GN
  retention + sync), then pn/dn retention (DFPN workers), then grouping
  (master-overhead relief), then second-level threads.
- **Master saturation.** "Beyond a certain point, adding more workers
  stops being beneficial, as the overhead on the master grows
  significantly" (visible as low worker utilization; fixed by
  `grouping`, which lets same-node workers share a local database and
  deduplicate reported results).
- **Search-work inflation** (node-expansion ratio vs sequential DFPN):
  ≈0.94 at 1 core → **2.50 at 1024 cores** (Table 3, heuristic variant);
  worker utilization stays >99% throughout.
- Saturation: no saturation up to 1024 cores in the full configuration;
  the authors believe scaling continues as long as second-level
  parallelism can grow (appendix: "PDFPN scales well on up to 64
  cores").

**The 4–16 core regime, explicitly (Table 1):** at small pools the
optimal configuration has **`threads = 1`** — i.e. the second-level
shared-TT machinery (Kaneko's part) is *not engaged* until ≥128 cores
(`threads` 8+ appears at 128 cores, 32 at 1024). Up to 16 cores the
measured speedups are **near-linear from job-level parallelism alone**:
2.36× (2 cores), 4.34× (4), 8.06× (8), 16.22× (16) — with worker TT
retention and Grundy-number sharing, on a domain with slow expansion
(≈100 node expansions per Grundy number) and jobs of 10⁵ iterations.

**What this means for this solver — extrapolation, not quotation.** The
near-linear small-pool numbers are *not* evidence that Kaneko-style
shared-TT search scales: in this regime Čížek's winning configuration is
job distribution over **persistent private-state workers** —
architecturally our option C/B family, with the workers retaining their
TTs between jobs. The parts of the design that make 1024 cores work
(key-result sharing, grouping, second-level threads) are all either
unsound-by-construction for us (§3) or engaged only far above our
regime. A realistic small-pool expectation for a Čížek-style
architecture ported to atomic chess is bounded by (a) the tail/waste
our OR-node early exit creates (lean report7: the sequential solver
already exits on the first refuting child), (b) the 2.5×-at-1024 work
inflation trend starting from ~1.0 at small pools, and (c) master-tree
bookkeeping that has no analog in today's CLI. The honest extrapolation
is "plausibly ≥ Kaneko's 3.6×/8 and PPN₂'s ~7×/16 class, mechanism =
job-level with retained worker TTs, *not* the 333× headline."

## 5. What changed vs the 15-year-old scaling assumptions

- Kaneko (mined in `dfpn/research_parallel.md`) assumed shared memory
  and reported 3.58×/8; the 2025 paper's Related Work confirms that as
  still the shared-memory state of the art *for its own second level* —
  the assumption "parallel DF-PN saturates in the low tens of threads"
  survives, now with a measured explanation (second-level threads only
  pay off at ≥128 workers).
- What changed is the *distributed* picture: prior distributed PNS
  (Checkers-style PNS-DFPN, PPN₂) scaled at 18–21×; the two-level
  design + state retention + path-independent key sharing moves it to
  hundreds. The enablers are architectural (retention, sharing,
  grouping), not a new search algorithm.
- The paper also supersedes the "many workers ⇒ cheap" intuition: its
  own master-saturation analysis says worker count has a sweet spot
  handled by grouping and longer jobs.

## 6. Mapping to option D (two-level shared-worker)

- **Mechanism.** Master holds a frontier tree (root children or deeper
  pseudo-MPN leaves); workers are *full sequential solver instances*
  solving assigned subtrees with `iterations`-capped budgets and
  reporting child outcomes; workers persist across jobs so their TTs
  survive; a shared database distributes path-independent solved facts.
- **Where it would live.** A new coordination layer outside the hot
  path: a `examples/`-style harness (or a thin `src/search/parallel/`
  facade) spawning N solver processes against assigned FENs, plus a
  master bookkeeper for the frontier. `src/search/dfpn/` itself is
  untouched in the first stage; `play_and_solve` is already the
  worker primitive (play a move, solve the resulting position).
- **Surface.** Job assignment FEN + budget; result payload = outcome +
  DTM/PV + `child_evals`; no change to `ProofEvent`, TT semantics, or
  sequential CLI. `--threads N` stays parked; the surface here is a
  harness flag (e.g. `--workers N` at the harness level).
- **Soundness-contract sketch / gap.** Worker outcomes are produced by
  the unmodified sequential solver — per-worker soundness inherited;
  no shared mutable TT, so the journal contract is *not* triggered at
  the worker level. The gap is at the master: combining worker
  outcomes into a root claim must respect constraint 2 — AND-side
  "all children Win ⇒ Loss" composition is rule-sound only for the
  decisive facts (downward-closed in the ancestor set,
  `dfpn/research_ghi_journal.md` §5), and any Draw verdict a worker
  returns is path-dependent and must be treated as "unproven here",
  never cached as a shared key. The key-result database — the paper's
  biggest lever — has **no safe payload** in our domain (decisive reuse
  is plan10-hazard-banned; draws are path-dependent), so our port is
  the paper's architecture *minus its main scaling lever*.
- **Effort class.** M–L for a 4–16 worker harness (no TT concurrency
  refactor needed); the paper's full design (shared DB, grouping,
  two-level) is XL and mostly worthless here (§3).
- **Go/no-go input for the architecture note:** **conditional GO for
  the job-level skeleton** (option C with persistent workers), **NO-GO
  for the shared-database layer** and for the second-level shared-TT
  machinery at our pool sizes. The sizing spike should measure worker
  utilization/tail behavior and work inflation (child_evals ratio) at
  4–16 workers on the m22/shuffle-win class before any commitment.

## 7. Summary table row (plan1 synthesis)

| Mechanism | Option fed | Where it lives in our code | Soundness-contract verdict | Expected 4–16 core speedup class | Effort | Go/no-go input |
|---|---|---|---|---|---|---|
| Two-level PNS-PDFPN: master/frontier jobs + persistent workers; worker-level = Kaneko PDFPN; master-level = shared DB of path-independent key results | C (job distribution, persistent private-state workers); D's shared-DB layer | New harness layer (`examples/`/thin facade) + unmodified solver processes; `src/search/dfpn/`, `search/tt/` untouched | Worker level: absent-and-monotone (domain has no repetitions; per-worker soundness inherited). Master level: absent-and-load-bearing — decisive composition is rule-sound, draws path-dependent; shared DB has no safe payload (plan10-banned decisive reuse) | Near-linear in the paper's own 4–16 core data (2.4–16×) but from job-level + retention, not shared TT; realistic here ~C-class, capped by early-exit tail | M–L (harness); XL for the full design | Conditional GO: job-level skeleton for the plan-2 note; NO-GO: shared DB, second-level threads. Spike: utilization/tail + child_evals inflation at 4–16 workers |

## References

- T. Čížek, M. Balko, M. Schmid, *Massively Parallel Proof-Number
  Search for Impartial Games and Beyond*, arXiv:2511.10339v2 (2026).
  Vendored: `docs/theory/pns-pdfpn-2025/pns-pdfpn-2025.pdf`.
- I.-C. Wu et al., *Job-Level Proof Number Search*, IEEE TCIAIG 5(1),
  2013 (the first-level job/lock mechanics delegated to by the paper).
- T. Kaneko, *Parallel Depth First Proof Number Search*, AAAI-10.
  Mined: `dfpn/research_parallel.md`.
- A. Kishimoto, M. Müller, *Information Sciences* 175(4), 2005. Mined:
  `dfpn/research_ghi_journal.md` (the cross-worker context constraint).
