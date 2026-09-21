# Plan 1: Research reading round — parallel PNS (2025-era)

Initiative: `parallel` backlog #1, all three items. This is a **docs-only
plan**: no production code, no benchmark runs, no drift protocol (there is
nothing to drift). The deliverables are three literature extractions
(`research_cizek2025.md`, `research_jlpns.md`, `research_solrex.md`), a
synthesis that maps each paper onto the initiative's A–D architecture
options and its four design constraints, and enriched inputs for the
backlog #2 architecture note (not drafted here — plan 2 owns it).

Prerequisite reading:

- `parallel/initiative.md` — the design constraints (sequential path
  bit-identical, no false decisive outcomes, budget-determinism, RAM
  accounting) and the A–D option space are normative for every mapping
  section.
- `dfpn/research_parallel.md` — the fully mined Kaneko AAAI-10 baseline
  (virtual pn/dn congestion, Mark/Unmark, shared stop-set, per-node TT
  locks, ~3.6× on 8 threads). Reuse its terminology and section style;
  do not re-derive what it already covers. Its 15-year-old scaling
  assumptions are the thing the 2025-era papers supersede — say
  explicitly, per paper, what changed.
- `lean/report7.md` — the deterministic-parallelism no-go (1.47–1.48×
  sibling ceiling) and the renegotiated reopen triggers. Closed; not
  re-litigated here.
- `conversion/research_ghi_journal.md` — the soundness template. If a
  paper is silent on repetitions under concurrency (expected: Breakthrough,
  Hex, and the mined impartial-game class are position-monotone or the
  treatment is thin), that silence is the finding, and the journal
  contract remains the load-bearing argument we must carry ourselves.
- `conversion/plan5.md` full-text check on Čížek 2025 (recorded in
  `docs/bibliography.md`): no child-level cutoff/widening content —
  parallel-layer only. Start from that, don't rediscover it.

## Goal

Answer, from the primary sources, a fixed question set per paper, then
synthesize into a recommendation input for the plan-2 architecture note:

1. **Čížek, Balko, Schmid 2025 (*Massively Parallel Proof-Number Search
   for Impartial Games and Beyond*, arXiv:2511.10339):** what exactly is
   the two-level decomposition and the "shared worker info", and what does
   the measured scaling curve say about the 4–16 core regime this repo's
   consumers actually run — i.e. how much of the 1024-core design survives
   when the worker pool is two orders of magnitude smaller?
2. **Saffidine, Jouandeau, Cazenave 2011 (JLPNS, ACG-13):** is job-level
   PNS a process-portfolio in disguise (independent jobs, no shared
   mutable proof state), and if so, does it support or refute option B
   (process-level racing) as the low-soundness-risk architecture?
3. **Young, Hayward 2016 (*A Reverse Hex Solver*, arXiv:1707.00627):**
   Solrex is the closest published thing to "parallel DF-PN over a shared
   TT in practice, on few threads" — what concurrency model does it use,
   what overhead did it actually pay, and does its speedup curve match
   Kaneko's 3.6×/8-thread figure or the lazy-SMP economics `lean` plan7
   doubted?

The initiative's structural constraints are normative for every candidate
mechanism: it must state (or admit the absence of) a soundness argument for
cross-worker reuse of repetition-dependent results, and it must preserve
the sequential path bit-identically as a strict opt-in.

## Extraction format

Follow the house style of `dfpn/research_*.md` (and this plan's sibling
`dfpn/research_parallel.md`): standalone markdown, numbered sections,
algorithm descriptions in the paper's terms followed by an explicit
mapping section ("what this means for this solver"). Every claim about a
paper cites its section/figure; every claim about the solver cites the
file/function. Extract machinery, soundness conditions, and measured
results — do not copy papers wholesale.

## Sources

- Čížek 2025: [arXiv:2511.10339](https://arxiv.org/abs/2511.10339) (open
  access). Check the authors' companion implementations/experiments for
  parameter details the paper omits.
- JLPNS: ACG-13 proceedings (Springer LNCS 8885); author copies are on
  Saffidine's and Cazenave's pages. If only the paywalled version is
  reachable, use the author copy and note the version mined.
- Young & Hayward 2016: [arXiv:1707.00627](https://arxiv.org/abs/1707.00627)
  (open access). Cross-reference with Kaneko's figures already extracted
  in `dfpn/research_parallel.md` §9.

## Research 1: Čížek 2025 (`research_cizek2025.md`)

Extract, at minimum:

1. **The two-level decomposition.** What is parallelized at the top level
   (subtree ownership? root children? job queue?) and what at the worker
   level; what precisely is the "shared worker info" (which fields, which
   consistency guarantees, which synchronization primitives), and how it
   compares to Kaneko's per-node locks + stop-set.
2. **The repetition question.** The paper targets impartial games; extract
   how cycles/repeated positions are handled — cached, re-proven, or
   absent from the benchmark set — and whether any soundness argument for
   concurrent caching of path-dependent results is given. If the games
   mined are position-monotone, state that the parallel repetition
   question is *unaddressed by the paper* — that is the load-bearing gap
   our design must close (journal contract, constraint 2).
3. **The scaling curve.** From the reported 333×/1024-core figure and any
   smaller-run data: overhead sources (duplication, synchronization,
   congestion), saturation point, and an explicit extrapolation to 4–16
   workers ("what this means for this solver" must not just quote the
   headline number).
4. **Mapping to option D** (two-level shared-worker): mechanism, where it
   would live in our code (`search/dfpn/`, `search/tt/`, a new worker
   layer), expected surface, soundness-contract sketch or gap, effort
   class, and a go/no-go recommendation for the architecture note.

## Research 2: JLPNS (`research_jlpns.md`)

Extract, at minimum:

1. **The job definition and ownership model.** What a job is (frontier
   subtree proof/disproof?), who owns state during a job, what is
   communicated back (result only? partial proof numbers?), and whether
   workers ever mutate a shared proof structure concurrently.
2. **Load balancing and idleness.** How jobs are assigned, what happens to
   workers when few jobs remain (the tail problem our own OR-node
   early-exit would aggravate), and the measured utilization.
3. **The portfolio question.** Compare their job scheduling to option B
   (process-level racing with independent TTs) and option C (root/child
   split): is JLPNS evidence that a job-level decomposition with *no*
   shared TT gets competitive speedups on a cluster/small pool, or does it
   depend on a shared/central proof structure?
4. **Scaling regime** on the hardware they used (core counts, network if
   distributed), same explicit small-pool extrapolation as above.
5. **Mapping section** ending in an option-B/C verdict with the same
   fields as Research 1's item 4.

## Research 3: Young & Hayward 2016 (`research_solrex.md`)

Extract, at minimum:

1. **The concurrency model.** Threads vs processes, what is shared (TT?
   partial proof numbers? stop flags?), locking or lock-free, and how it
   differs from Kaneko's design — this is the closest empirical test of
   option A's economics.
2. **Measured overhead and speedup** at low core counts (the few-thread
   part of the curve, not the best case), including search-work inflation
   vs the sequential solver (our secondary metric).
3. **Domain transfer risks.** Hex is position-monotone (no repetitions);
   state explicitly that the paper offers no evidence on constraint 2, and
   whether any of its TT contents would be path-dependent in our setting.
4. **Mapping section** ending in an option-A verdict, same fields.

## Tasks

1. Fetch and read the Čížek paper; write `research_cizek2025.md`.
2. Fetch and read the JLPNS paper; write `research_jlpns.md`.
3. Fetch and read the Young & Hayward paper; write `research_solrex.md`.
4. Synthesis pass (each file's mapping section must end with a table row):
   for each paper — mechanism, option (A–D) it feeds, where it would live
   in our code, soundness-contract verdict (given / absent-and-monotone /
   absent-and-load-bearing), expected small-pool (4–16 core) speedup
   class, effort estimate, and a go/no-go input for the architecture
   note. Then append a cross-paper comparison paragraph to
   `parallel/initiative.md`'s option-space section: which of A–D survive
   on the evidence, and what the plan-2 sizing spike should measure first.
   If the evidence favors a hybrid (e.g. B first, A behind it), say so
   with the reasoning; do not pre-decide plan 2.
5. Update `parallel/initiative.md`: backlog #1 status (mined, pointers to
   the three files); enrich the backlog #2 row with the synthesis
   pointers. Do not draft plan 2 in this session — the reading round ends
   at the re-ranked backlog.
6. Update `docs/bibliography.md`: flip the three entries (Čížek 2025,
   JLPNS, Young 2016) from **Open** to **Mined** with pointers to the new
   files (they live under `docs/plans/parallel/`).
7. Sanity-check: no `src/` change (`git status` clean except docs), no
   dangling cross-references from the new files (links resolve to real
   paths), terminology consistent with `dfpn/research_parallel.md` where
   concepts overlap (virtual numbers, stop-set, worker).
8. Write `report1.md` in this directory: papers actually read (versions,
   page counts), discrepancies between the papers' assumptions and ours
   (monotone domains, thread counts, TT sizes), the per-paper verdicts
   and the surviving-option recommendation, backlog changes, unresolved
   questions (e.g. questions the papers don't answer that the plan-2
   spike must), and next steps.

## Non-goals

- Any production code change, env-gated spike, or benchmark run: no
  architecture is implemented or prototyped here.
- Drafting plan 2 (the architecture note) or any implementation plan: the
  output is the re-ranked backlog plus measurement inputs.
- Re-opening deterministic parallelism (`lean` plan7 no-go) or the plan10
  Win/Loss-reuse hazard — mapping sections must check candidates against
  these measured failure modes, and a paper-based "it should work" needs
  a mechanism-level reason, which is exactly what the mapping sections
  must produce or withhold.
- Re-mining Kaneko AAAI-10 — it is already extracted; build on
  `dfpn/research_parallel.md`.
