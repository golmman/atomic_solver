# Research: Saffidine, Jouandeau, Cazenave 2011 — *Solving Breakthrough with Race Patterns and Job-Level Proof Number Search* (JLPNS/PPN₂)

Mining round for `parallel` backlog #1 (plan1). Terminology reused from
`dfpn/research_parallel.md` (virtual numbers, worker, TT) and
`dfpn/research_ghi_journal.md` (path-dependence, journal contract).

## 0. Version read

- **Primary**: A. Saffidine, N. Jouandeau, T. Cazenave, *Solving
  breakthrough with Race Patterns and Job-Level Proof Number Search*,
  13th International Conference on Advances in Computer Games (ACG 2011,
  Tilburg), LNCS, pp. 196–207. **Author copy** via HAL
  (`hal.science/hal-01499675`, deposited 31 Mar 2017); the Springer
  version is paywalled. Vendored at `docs/theory/ppn2-2011/ppn2-2011.pdf`
  (HAL cover page + 12 content pages). Version note: the plan and
  bibliography call this paper "JLPNS (ACG-13)"; the paper itself does
  not propose JLPNS — that is Wu et al. (Connect6, CG 2010 / IEEE
  TCIAIG 2013), which it extends. What it proposes is **PPN₂**
  (Parallel PN²), "an extension of the parallel algorithm JLPNS when a
  PN search is used as the underlying job". This distinction matters
  for the portfolio question (§4).

## 1. The job definition and ownership model (§2.2–2.3, Alg. 1–2)

- **Base scheme (JLPNS, Wu et al.)**: a main PNS tree on a server;
  instead of plain leaves, a **solver is called at each leaf** on a
  remote client. Clients are prevented from duplicating work by a
  **virtual-loss mechanism**: a leaf sent to a client is "temporarily
  assumed to be proved a loss until the client returns a meaningful
  result". The paper's own diagnosis: under virtual loss, "0 and ∞ are
  no longer attractor values for the proof and disproof numbers" — a
  node can sit marked losing and later revert to unsolved.
- **PPN₂'s job**: a most-proving, not-reserved **leaf of the server's
  PNS tree** plus a **descent threshold N**. The client develops a full
  **PN search** rooted at that leaf ("PN² at the leaves, on a remote
  client") until N descents or solution. Because the underlying job is
  a PN search, **partial results** — the current `pn`/`dn` of the job
  root — are sent back every `p` descents; the server folds them into
  the master tree so they "influence the next searches of the other
  clients" (Alg. 1–2).
- **Ownership**: the server exclusively owns and mutates the master
  proof tree; each client owns its private search tree. **Workers never
  mutate a shared proof structure** and never see each other. Reserved
  leaves carry a **flag** (not virtual loss): "a node is never
  considered to be losing unless it has actually been proved to be
  losing, thus 0 and ∞ remain attractors"; the server picks tasks from
  the set of most-proving nodes, skipping reserved ones.
- **What is communicated back**: partial/final `pn`/`dn` of the job
  root (the remote tree itself stays on the client and is eventually
  discarded). No shared TT exists at any point — client trees are
  private and bounded by N ("memory resources in the clients were
  never a problem").

## 2. Load balancing and idleness (§4.1)

- Assignment: server-side backtracking over the most-proving-node set
  to find a not-reserved leaf (Alg. 1). Dynamic work stealing from the
  frontier, no static partitioning.
- **The tail problem is not discussed.** The paper never says what
  happens when few unreserved most-proving leaves remain — the situation
  our OR-node early exit aggravates (lean report7: the sequential
  solver already stops at the first refuting child, so frontier jobs
  near a proof collapse to few candidates). No worker-utilization
  metric is reported; the closest proxies are the node counts (§5).
- Partial-result frequency sweep (Table 4): best at every 100 descents
  (186 s vs 263 s with none, on the 4×5 board, 64 clients); too-frequent
  updates cost more in communication than they gain in guidance.

## 3. The portfolio question (option B vs C)

The plan asks: is job-level PNS a process portfolio in disguise
(independent jobs, no shared mutable proof state)?

- **It is not option B.** A portfolio (initiative option B) races N
  *independent full solves* of the same root with different
  parameterizations; there is no coordination and no decomposition.
  PPN₂'s clients do not race: they solve *different subtrees* chosen by
  a central master from a single, growing proof structure, and their
  partial results feed back into that structure. Coordination is the
  point.
- **It is option C with dynamic job assignment and a central bookkeeper**
  — a root/frontier split where each worker owns a private state and a
  private search tree, and the master combines returned pn/dn into its
  own tree. No shared mutable proof state ever exists; per-worker
  soundness is inherited from the underlying sequential solver.
- **The load-bearing evidence for the low-soundness-risk family**: this
  architecture achieves **18.3× with 64 clients, 7.2× with 16** on a
  small cluster of quad-core boxes over gigabit Ethernet (Table 3) with
  **no shared TT**. So a job-level decomposition with *no* shared TT
  gets competitive speedups on a small pool — supporting options B/C as
  the low-risk path, *while refuting the "independent jobs" reading of
  B*: the measured speedup depends on the master's continuous
  re-direction of workers (partial results improved scaling; state
  resets would kill it — Čížek 2025's ablation,
  `research_cizek2025.md` §4).
- Soundness in our setting: a client job is a full solver run of an
  assigned subtree from its own root path. Its decisive outcome is
  path-independent by rule (journal value claim,
  `dfpn/research_ghi_journal.md` §5) and the master's AND-side
  combination (all children Win ⇒ Loss) is rule-sound for decisive
  facts. A client **Draw** is path-dependent and must be consumed as
  "unproven here" — never stored as a shared key. This is exactly the
  master-level discipline Čížek's design needs too; the difference is
  PPN₂'s payload (pn/dn of the job root) rather than a shared database.

## 4. Scaling regime (§4)

- **Hardware**: 17 Linux computers, 3.2 GHz Intel i5 quad-core, 4 GB
  RAM, gigabit switches; master alone on one machine; up to 64 clients.
  Distributed memory, commodity LAN — the closest published analogue to
  a 4–16 process pool on one multicore host.
- **Measured speedups** (4×5 board, 1k descents/job, Table 3): 1 →
  3397 s; 4 → 2.2×; 8 → 4.2×; 16 → 7.2×; 32 → 11.1×; 64 → 18.3×.
- **Work inflation**: expanded server nodes grow 107k (1 client) →
  232k (64); touched 915k → 1930k — roughly **2× more tree work at 64
  clients** ("running many clients in parallel makes it harder to avoid
  unnecessary work"). The expanded-node count also equals the number of
  jobs sent, so the master tree stays small (memory concentrated in
  touched nodes).
- **Small-pool extrapolation for this solver**: at 4–16 clients the
  curve is near-linear (≈0.55×/client), with ~2× work inflation already
  latent. Our domain diverges where the early exit bites: at OR nodes
  the sequential solver stops at the first refuting child, so racing
  children (option C's OR-side) wastes exactly the work the sequential
  solver never does — the same mechanism lean report7 measured as the
  1.47× deterministic ceiling. PPN₂'s jobs, however, are *sequential
  subtree solves*, not sibling races, so the AND-side combination
  (all children needed) is where its shape fits us; the AND share of
  our work is ~60% (lean report7 phase 1), but each AND child is a
  full solve, so job-level parallelism over AND children is the natural
  fit.

## 5. The repetition question

Unaddressed, and the domain cannot pose it: Breakthrough pawns never
move backward (position-monotone, no cycles, no draws — the paper's
state-space bound and retrograde-analysis section assume it). No GHI
discussion exists. **Finding:** silence — the parallel repetition
question is unaddressed by the paper; the journal contract
(`dfpn/research_ghi_journal.md` §5) remains the load-bearing argument
for our master-side combination rules (§3).

## 6. Mapping section — option B/C verdict

- **Mechanism.** Master keeps a frontier (the root's AND-children or
  deeper pseudo-MPN leaves); N worker *processes* (today's sequential
  binary, own TT, own budget) solve assigned subtrees; results flow
  back as outcomes/bounds; master combines (AND: all-Win ⇒ Loss;
  OR: any-Win ⇒ Win) and re-assigns from the frontier.
- **Where it would live in our code.** Entirely outside the solver
  core: a harness in `examples/` orchestrating N invocations of the
  existing binary (`play_and_solve` / `replay` already demonstrate the
  per-job primitive). No `src/search/` change ⇒ the sequential path
  stays bit-identical by construction (constraint 1 vacuous).
- **Expected surface.** Harness-level `--workers N`; per-job
  `--timeout`/budget; aggregate `child_evals` accounting across
  workers; no CLI change to the solver itself. RAM = N×TT + master
  bookkeeping — constraint 4 must be restated, not broken (document
  N×TT explicitly).
- **Budget determinism.** Each worker's `child_eval_budget` exhaustion
  is a local `Draw` + `BudgetExhausted`; the master must treat that as
  "unproven here" (never a global Draw claim) — the contract survives
  per-worker, and the master's aggregation rule is new surface to
  document.
- **Effort class.** S–M (harness + combination rules + validation); no
  TT concurrency work. This is the cheapest of A–D by far.
- **Go/no-go input for the architecture note.** **GO for option C**
  (job-level decomposition, persistent private-state workers, no shared
  TT) as the first implementation stage — direct published evidence at
  4–16 clients with ~2× work inflation and no soundness exposure beyond
  the master-side decisive-fact rule. **Option B as a cheap
  pre-stage/parallel adjunct only** (it is not what JLPNS/PPN₂ does,
  and its wins are trajectory-lottery wins, not work division);
  **partial-result feedback and any shared structure: out of scope**
  for the first stage (they reintroduce coordination surfaces the
  soundness note would have to carry, for gains the Čížek data says
  only matter far above 16 workers).

## 7. Summary table row (plan1 synthesis)

| Mechanism | Option fed | Where it lives in our code | Soundness-contract verdict | Expected 4–16 core speedup class | Effort | Go/no-go input |
|---|---|---|---|---|---|---|
| PPN₂-style job-level decomposition: master-owned frontier tree, private client searches, partial pn/dn feedback, reservation flags (no virtual loss, no shared TT) | C (with B's infrastructure as a degenerate case) | Harness in `examples/` orchestrating N unmodified solver processes; no `src/search/` change | Absent-and-monotone (domain silence); master-side decisive composition rule-sound (journal value claim), worker Draws consumed as unproven; journal contract carried by us at one well-defined site | ≈0.5×/worker to 16 workers measured on a cluster; here capped by the OR-early-exit tail; AND-side (~60% of work, lean report7) is the fit | S–M | **GO for option C** as first stage; B only as degenerate adjunct; partial-result feedback out of scope for stage 1 |

## References

- A. Saffidine, N. Jouandeau, T. Cazenave, *Solving breakthrough with
  Race Patterns and Job-Level Proof Number Search*, ACG 2011, LNCS,
  pp. 196–207. Author copy vendored: `docs/theory/ppn2-2011/ppn2-2011.pdf`
  (HAL hal-01499675).
- I.-C. Wu, H.-H. Lin, P.-H. Lin, D.-J. Sun, Y.-C. Chan, B.-T. Chen,
  *Job-Level Proof Number Search for Connect6*, CG 2010, LNCS 6515
  (and the TCIAIG 2013 journal version) — the actual JLPNS.
- T. Čížek, M. Balko, M. Schmid, arXiv:2511.10339v2. Mined:
  `research_cizek2025.md` (this directory) — its first level is the
  descendant of this paper's architecture.
- A. Kishimoto, M. Müller, *Information Sciences* 175(4), 2005. Mined:
  `dfpn/research_ghi_journal.md`.
- `lean/report7.md` — the OR/AND work split and deterministic ceiling
  this mapping leans on.
