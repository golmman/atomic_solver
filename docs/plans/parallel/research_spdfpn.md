# Research: Pawlewicz & Hayward 2014 — *Scalable Parallel DFPN Search* (SPDFPN)

Mining round for the reopened `parallel` option-A stage. This is the
primary source that plan1 flagged as "not yet mined" (`research_solrex.md`
§1: Solrex's concurrency detail is delegated here) and that the plan2
NO-GO left deliberately open for "a hypothetical future A-stage". It is
being mined now because the determinism precondition has changed: the
product consumer accepts nondeterministic parallel runs (which *which*
valid proof wins, never a false one). Terminology reused from
`dfpn/research_parallel.md` (Kaneko AAAI-10) and
`dfpn/research_epsilon.md` (1+ε).

## 0. Version read

- **Primary**: J. Pawlewicz, R. B. Hayward, *Scalable Parallel DFPN
  Search*, CG 2013, LNCS 8427, pp. 138–150 (2014). **Author copy** from
  Hayward's site (`webdocs.cs.ualberta.ca/~hayward/papers/pawlhayw.pdf`),
  13 pages, Springer paywalled (DOI: 10.1007/978-3-319-09165-5_12).
  Vendored: `docs/theory/spdfpn-2014/spdfpn-2014.pdf` (+ extraction).
- **Bibliography correction**: the entry's `arXiv:1503.07698` pointer is
  wrong — that arXiv id resolves to an unrelated XENON1T detector paper.
  No arXiv version of SPDFPN was found; the entry now points at the DOI
  and the vendored author copy. Fixed in `docs/bibliography.md`.

## 1. The mechanism (what Solrex's paper could not establish)

Serial base = **DFPN with the 1+ε method** (their own ε=0.25) treating
the DAG as a tree over a TT — i.e. *exactly our solver's serial family*
(we run 1+ε with ε=0.125). The parallel design adds four components:

1. **Work threshold `W` (MaxWorkPerJob) — interruptible jobs.** DFPN
   gains a third threshold: the call halts (returning its work) once its
   local work — counted as *number of DFPN calls* — reaches `W`, so a
   thread's job is resumable and load can be rebalanced. Because a
   thread can be interrupted while the best child's pn sits between
   p2 and (1+ε)·p2, `SELECT` gains a "continue the same child" clause:
   on resumption, keep searching child j1 as long as p1 < (1+ε)·p2,
   rather than re-selecting from scratch. This is the modification that
   makes a work-capped dfpn *semantically* a resumable job, not a
   restart. `W` bounds job granularity: large enough for cheap dispatch,
   small enough for even distribution (criterion (iv)).
2. **Work assignment by pn-guided descent.** Candidates are nodes on the
   root→mpn path whose *past work* `n.w` is below `MaxWorkPerJob` —
   "closest to the root" such node: deep enough that DFPN will stay in
   its subtree (no thrashing), shallow enough for load balance. The
   descent (`TRYRUNJOB`, Alg. 4) is a DFPN-shaped recursion reading
   (d)pns from the TT, using virtual numbers when present (§2 below).
   Past work persists in the TT entry, so the criterion is global across
   threads, not per-thread local.
3. **Virtual win/loss + virtual (d)pns — the cooperation protocol.**
   When thread α is assigned node A, A temporarily gets a virtual win or
   loss (by p≤d), and virtual (d)pns are re-propagated along the path to
   the root (Coulom's idea, the same one JLPNS/PPN₂ uses). Other threads
   select children through these virtual numbers, so they are *steered
   away* from α's subtree — different threads always work on different
   subtrees (criteria (i)–(iii)). Virtual (d)pns live in a **virtual
   TT**: per-ply arrays of at most #threads entries (a node is entered
   at most once per thread), guarded by one **job lock** held only
   during candidate-finding; it is released when the assigned thread
   starts solving and re-taken for bookkeeping after. Virtual entries
   are removed when the job finishes and true (d)pns re-propagate.
4. **Shared TT discipline.** Multi-reader/single-writer locks on the
   shared TT. Two write-safety rules: never overwrite an entry another
   thread has solved during your processing (following Kaneko), and
   computer-chess-style replacement with k=4 collision probing that
   overwrites the slot whose job did the *least work* — chosen because
   1+ε "works best with advanced tt collision resolution" and they need
   good behavior as the TT fills ("searches effectively even as the
   transposition table becomes almost full" — search spaces reached
   ~100,000× TT size with continuous progress on the hardest 9×9 Hex
   opening).

The paper is explicit that the virtual assignment can be *wrong* (an
inaccurate virtual win/loss makes SPDFPN diverge from serial DFPN,
violating their criterion (iii)) and accepts this — the point of the
design is that divergence costs efficiency, not correctness: correctness
is carried by the underlying (F)DFPN's PN-maintaining rules, not by the
cooperation protocol.

### vs Kaneko (their §5.5; Kaneko's mining: `dfpn/research_parallel.md`)

Kaneko's congestion term augments a child's pn by the number of threads
searching it — *discouraging* but not preventing re-selection of the
same child; near the tree bottom sibling pns are all small and similar,
so Kaneko's threads are likely to pile onto the same child near the
leaves (their observed behavior). SPDFPN replaces the soft penalty with
a hard guarantee (virtual numbers make the assigned node look decided)
and makes the diversion *depth* an explicit, tunable parameter
(`MaxWorkPerJob`). This is the measured difference: Kaneko 3.58×/8
threads, SPDFPN ≈5.9×/8 (run 1: 6.4×/8) on a comparable-core regime.

## 2. Measured results (Hex, Benzene/Fuego, Focussed DFPN)

- **Scalability** (Suite 2, 8 hardest 8×8 openings + 8 Olympiad
  positions; 24-core machine, 16 threads used; their Table 2):
  efficiency 0.94/2 threads → 0.85/4 → 0.80/8 → 0.65/12 → **0.74/16
  (11.8×)** in run 2 (MaxWorkPerJob 500/100); run 1 (100/20): 9.4×/16
  (0.59). Decomposition: threading-overhead factor f_t ≤ 1.22
  (shared-TT access + hardware), extra-work factor f_s ≤ 1.40 (extra
  leaf expands from divergence) — **work inflation only 5–40% at 16
  threads**. (Geometric-mean speedups; the Solrex superlinearity caveat
  — clock scaling between serial and parallel hosts — applies to some
  suite-1 rows, not to Table 2, which is one machine.)
- **Suite 1** (24 threads of a hyperthreaded 12-core): all 13 previously
  intractable 9×9 openings plus the first-ever 10×10 opening (63 d);
  hardest 9×9 opening 111 d. The old 8-thread solver solved *none* of
  these in 480 h. For hard openings they raised ε from 0.25 to 0.5 to
  reduce TT-lookup failures (§6.1) — an ε/TT-pressure interaction worth
  remembering (we have the same knob).
- They attribute Hex's better parallel speedups (vs tsume-shogi's) to
  larger branching factor ⇒ smaller node-expansion rate — i.e. **more
  time per node amortizes locking/coordination**. Domain-transfer risk
  flagged in §4 below.

## 3. Repetitions under concurrency — the silence, and why it matters less here

As with all four previously mined parallel-PNS papers (plan1 finding),
SPDFPN is silent on GHI/repetition hazards: Hex is position-monotone.
The journal-GHI contract (`dfpn/research_ghi_journal.md` §5) therefore
remains load-bearing — but this paper's mechanism lets us carry it
cheaply, because of what our TT *already* does:

- Our TT stores only **path-independent base entries**; repetition-
  dependent results are *never cached* (first-player-loss GHI shortcut;
  they stay unsolved (1,1)). The TT payload shared across threads is
  therefore path-independent by construction, and cross-thread reuse of
  a decisive entry is the *same* reuse the sequential solver already
  performs at the same key — no new soundness class is introduced by
  sharing it. The plan10 cross-worker reuse ban is a different context
  (a global store across separate solves); it does not cover
  same-run thread sharing of path-independent entries.
- The **per-run repetition-draw cache** (`repetition_cache.rs`, keyed by
  position hash + ancestor repetition set) is path-dependent *by
  design* — it must stay **thread-local** (per-worker private caches,
  never shared, never merged). Sharing it across threads with different
  paths is the one sharp false-result edge in an otherwise
  path-independent payload.
- Stale-(d)pn hazards from TT overwrites (their §5.2) already exist
  sequentially in milder form; the TT's own rules (never overwrite a
  solved base entry with unsolved bounds) plus per-shard critical
  sections preserve them under concurrency.

The resulting soundness statement for an SPDFPN-style mode here:
*nondeterministic in which valid proof is found and in work counts;
never a false decisive outcome* — worst-case defect of a wrong virtual
assignment is wasted work, exactly the paper's failure mode. That is
precisely the nondeterminism envelope the consumer now accepts.

## 4. Mapping onto this solver — bill of materials

| SPDFPN component | This solver's state | Effort |
|---|---|---|
| Serial 1+ε dfpn | already the core (`dfpn/mod.rs`, ε=0.125) | — |
| Work threshold W + same-child resume clause | partial: `call_max_work`/`round_work_cap` exist, work unit = child_evals (keep it — deterministic, our optimizer metric — vs the paper's DFPN calls); resume clause small | S–M |
| `n.w` past-work criterion | TT entries already carry cumulative per-subtree `work` (child_evals) — the candidate test maps directly | S |
| Shared sharded TT (multi-reader/single-writer, k=4-style replacement) | today `&mut self` single owner; needs per-shard locks + atomic generation; `store()`'s read-modify-write in one critical section. This is the "inert TT-concurrency refactor" gate item from `design_space.md` §1 | L |
| Virtual TT + job lock + `TRYRUNJOB` assignment | new; virtual TT is O(depth × threads) — negligible memory | M |
| GHI under concurrency | path-independent TT payload shared; repetition cache thread-local (mandatory); document as contract | S (docs + tests) |
| ProofEvent stream | events become nondeterministically ordered across threads; the proof-tree worker/reconstruct contract needs an explicit decision (serialize emissions behind a lock, or a parallel mode that emits no events) | M |
| Budget determinism (C3) | `child_eval_budget` exhaustion → `Draw`/`BudgetExhausted` becomes advisory under `--threads > 1`; re-document | S |
| RAM (C4) | TT shared (one table, unchanged); per-thread history/killers/pools N× (small); "RAM = TT only" survives with restatement | S |
| Sequential path (C1) | strict opt-in; drift gate runs at N=1 and must stay byte-identical | S |

Domain-transfer risks to measure (not assume):

1. **Node-expansion rate.** Hex's good scaling is credited to time spent
   per node (VC engine). Our hot path is movegen/eval — cheap nodes mean
   locking and assignment overhead weigh more; `W` in child_evals is
   likely to want a *larger* multiple than the paper's 100–500 calls,
   and efficiency may sit below 0.74.
2. **Problem size.** m22 sequential is 2.63 s — far below any parallel
   payoff. The target class is shuffle-win-like (47.8 s/13.9 M evals)
   and the deep-solve workload; the spike must measure there.
3. **Branching shape.** Atomic chess explosions change the AND/OR
   texture vs Hex; the p≤d virtual-win rule at AND nodes needs a
   perspective-checked mapping.
4. **TT pressure.** Our replacement policy vs their least-work k=4; the
   paper's ε=0.5-under-pressure experience suggests measuring ε
   interaction on the hard class.

## 5. Verdict

Under the accepted-nondeterminism premise, SPDFPN is the strongest
published shape for option A and the only remaining multiplicative lever
on the roadmap (`structural_floor.md` §9): mechanism fully specified,
game-independent (no domain heuristics — their abstract's claim, and the
pseudocode bears it out), built on the same serial base we already run,
with a bounded nondeterminism story we can carry ourselves (§3). The
plan2 NO-GO verdict on option C is untouched — its structural deficit
(forfeited cross-child TT sharing, 15.5× work inflation) is exactly what
SPDFPN's shared TT + virtual-number steering solves (their f_s ≤ 1.40).

Recommended staging (each stage drift-gated at N=1):

1. **plan4 — inert TT-concurrency refactor**: sharded TT behind
   interior-mut locks, byte-identical sequential behavior, no public API
   change. Pure prerequisite, no behavior change; justified only by the
   A-stage (state that in the plan).
2. **plan5 — SPDFPN prototype**: W-threshold + resume clause + virtual
   TT + TRYRUNJOB behind `--threads N`; repetition cache thread-local;
   measure wall speedup, work inflation (f_s analog), outcome agreement
   vs sequential on the hard class, with pre-registered GO bands
   (e.g. GO ≥ 2.0×/4 threads with zero decisive disagreements and
   work inflation ≤ 2×; NO-GO otherwise). ProofEvents: serialize
   emissions (single mutex) for the prototype.

Backlog #4 (initiative.md) records items 1–2; plan4/plan5 are written
when their session opens. If plan4's drift gate cannot be satisfied at
reasonable cost, the A-stage dies there — that is the cheap kill point.
