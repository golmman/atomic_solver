# Parallel search design space — architecture selection note

`parallel` backlog #2 deliverable (plan 2). Decides, with measurements
and the pre-registered rules in `plan2.md`:

1. whether an opt-in nondeterministic parallel mode ships at all
   (lean trigger 1, consciously deferred to this note);
2. which **shape** — a harness example over unmodified solver processes
   (option C), a CLI `--threads N` mode (option A), both, or neither;
3. **GO / MARGINAL / NO-GO** for backlog #3 (staged implementation).

Sizing-spike evidence: `measurements/plan2/` (harness script, raw
captures, README with the command table). The spike drives the release
binary as a black box; nothing in `src/` or `examples/` changed.

Related: `initiative.md` (constraints, normative), `plan1.md` /
`report1.md` (reading round), `research_cizek2025.md`,
`research_jlpns.md`, `research_solrex.md`, `../lean/report7.md`
(deterministic no-go, OR/AND split), `../../dfpn/research_ghi_journal.md`
(journal contract), `../../research/structural_floor.md` §2, §9.

---

## 0. What option C means concretely on this solver

The root of a solve is an OR node: the first root child proven winning
proves the root. The simplest C instance is therefore a **root-child
race over process-per-child workers**:

1. enumerate root moves (`list_legal`);
2. derive each child FEN (`replay <fen> <move>` prints `fen:`), noting
   already-terminal children (immediately proven);
3. run each child as its own solver process (`--timeout <cap>
   --first-outcome`), first proven child stops everything; workers are
   the *unmodified* sequential solver.

**Constraints 1/2 are vacuous at the worker level**: each worker is
today's solver, bit-identical semantics, private TT, no sharing of
anything mutable. The master combines only decisive facts — a rule-
sound composition (child `loss` ⇒ root move proven; child terminal ⇒
root move proven; child `win` ⇒ root move eliminated; child `draw`
⇒ unproven), not cross-worker *reuse*. The plan10 ban is about caching
results into a TT/proof state; a scheduler observing worker outcomes is
not reuse.

**Perspective mapping** (the classic bug source; encoded explicitly in
the harness and cross-validated against `find_winning_child` on two
shallow decisive-suite FENs, exact agreement, `measurements/plan2/README.md`):

| child solve result (child side to move) | consequence for the root move |
|---|---|
| already terminal (replay `outcome:` ≠ None) | **proven** |
| `loss` | **proven** |
| `win` | eliminated (root move loses for the root side) |
| `draw` (incl. budget timeout) | unproven, consumed as such |

A root is **disproven** only if all children return `win`/eliminated —
not a spike goal; the harness reports it when observed (it never was).

**Master-side re-direction** (PPN₂-style partial pn/dn feedback) is out
of spike scope; see §5 (enrichment path).

## 1. Options A–D × constraints 1–4 matrix

Constraints (initiative.md, normative): **C1** sequential path
bit-identical; **C2** no false decisive outcomes (journal-GHI contract
binds for shared mutable TT); **C3** budget-determinism contract
preserved or explicitly re-documented; **C4** RAM accounting restated,
not silently broken.

| | C1 sequential bit-identical | C2 no false decisive (GHI) | C3 budget determinism | C4 RAM | Effort | Verdict |
|---|---|---|---|---|---|---|
| **A** thread-level shared-TT lazy-SMP (`--threads N`) | holds only because the mode is opt-in (drift gate runs at N=1) — but the CLI surface itself changes | **load-bearing**: a shared mutable TT makes the journal contract bind at every TT site (`dfpn/research_ghi_journal.md` §5); no published soundness argument for repetitions under concurrency exists in the whole 2025-era literature round (plan1 finding) | renegotiated: budget advisory under N>1 (`../lean/report7.md` §2 table) | TT shared, but per-thread history/pools; restatement modest | L–XL (inert TT-concurrency refactor first) | real but **second**; only after mining Pawlewicz & Hayward 2014 (bibliography: **Open**); best few-thread figure published ≈5.2×/4, superlinear caveat, no inflation figure (`research_solrex.md`) |
| **B** process portfolio (N parameterizations racing one root) | vacuous (no CLI change) | vacuous per worker | vacuous | N×TT | S | **adjunct only, not the main lever**: JLPNS/PPN₂ is not a portfolio — central re-direction is where its speedup comes from; Čížek's ablation: discarding worker state between jobs falls *below* sequential (`research_cizek2025.md` §2, `research_jlpns.md`) |
| **C** root/child split, per-worker TTs (harness over unmodified processes) | **vacuous by construction** (no CLI option exists) | **vacuous** (private TTs; master combines decisive facts only — rule-sound, not reuse) | vacuous per worker; master-side caps are its own documented surface | **N×TT** — restatement required (§4) | S–M | **strongest survivor on soundness & shape** (plan1); the sizing spike measures whether the wall-time economics survive our tree shape (§3) |
| **D** two-level shared-worker protocols (Čížek 2025) | second level = A's problems | shared-DB layer has **no safe payload here**: the paper's DB carries rule-path-independent Grundy numbers; our only rule-path-independent class (decisive Win/Loss) is plan10-banned from cross-worker reuse, draws are path-dependent (`research_cizek2025.md` §3, §6) | as A | as A | XL | **dead as a distinct option**: reduces to C (first level) + A (second level, idle below 128 cores in Čížek's own data) |

Every A/D cell rests on plan 1's mining (cited research files) — this
note owns those verdicts and re-checked them against the matrix; the
spike adds the missing measurement for C.

## 2. Ship-shape decision space (stated before measurements)

- **`examples/parallel_solve`-style harness (process race)**: CLI
  untouched — C1 holds by construction ("with the option off, every
  stdout byte matches today" is trivially true because there *is* no
  option); RAM restated in the *example's* docs as N×TT, not in the
  search CLI's contract; no proof-event surface. Worker soundness
  inherited; the master's decisive composition is the only new logic.
- **CLI `--threads N`**: touches `src/main.rs` + help text, the drift
  gate, the RAM=TT contract, and the budget-determinism surface (C3) —
  only justifiable if A advances past its gate (mine Pawlewicz &
  Hayward 2014; inert TT-concurrency refactor first).
- **Both**: the harness first, A behind it — plan 1's recommendation.
- **Neither**: the consumer story decides this. The external
  optimizer's `benchmark` harness is explicitly sequential-
  deterministic (`docs/spec/optimizer_interface.md`); the repo's own
  validation methodology (bit-identical drift gates, reproducible TT
  snapshots, dual-build oracle) rests on determinism. The realistic
  consumer of a parallel mode is **a human analyst on a multicore box
  wanting a faster single solve**, accepting nondeterministic *which*
  proof wins — never a false one.

Working hypothesis (overturnable by the data): **ship as an example
harness, never as a CLI flag**; A remains a separate, gated future
stage.

## 3. Sizing spike — measured cells (m22, 4-core reference container)

Hardware envelope: 4 cores (cgroup `cpu.max` quota, matching `nproc`)
/ **8 GiB enforced cgroup memory limit** (the plan assumed 16 GB; the
corrected envelope matters only for §4's RAM restatement — all N ≤ 4
wall runs fit comfortably). Wall speedups meaningful at N ≤ 4;
N = 8/16 rows are **deterministic schedule simulations** over the
measured per-child (wall, nodes) distribution — labeled as such.

Primary quantitative case **m22_white** (`4r2k/3p4/2pB2p1/p6p/5pPP/
2N1PP2/P1PP4/1R4RK w - - 0 22`): its sequential root solve (~2.6 s,
858k nodes, first-outcome) is ~18× cheaper than shuffle-win's (47.8 s,
13.9M nodes), so per-child budgets (120 s cap) close within the spike
budget; shuffle-win serves as the qualitative probe (task (i)).

Sequential root baselines (release binary, `--first-outcome`, default
TT/ε/refine-cap):

| case | outcome | wall | nodes (work proxy) | PV length |
|---|---|---|---|---|
| m22_white | win | 2.63 s | 858,117 | 95 plies |
| shuffle-win | win | 47.83 s | 13,907,467 | 477 plies |

`nodes` (the solver's `pre_exit:` count) is the work proxy;
`child_evals` itself is not CLI-visible.

### (i) Tail / utilization and work inflation at N = 2–4 (measured)

Race = N workers over the 43 m22 children (list_legal order, per-child
cap 120 s), first proven child stops everything; 5 reps per N. The
winning child (`a2a4`, isolated solve 8.7 s / 8.59 M nodes) is 2nd in
order, so every race reaches its best case immediately — these numbers
are the **most favorable scheduling shape option C can get** here.

| config | median wall (s) | wall speedup vs sequential root | aggregate worker nodes | work inflation vs root | proven in all reps |
|---|---|---|---|---|---|
| sequential root (N=1) | 2.63 | 1.00 | 858,117 | 1.0× | win |
| race N=2 | 9.05 | **0.29×** | 9,928,244 | **11.6×** | a2a4 (5/5) |
| race N=3 | 9.38 | **0.28×** | 11,638,819 | **13.6×** | a2a4 (5/5) |
| race N=4 | 9.75 | **0.27×** | 13,328,239 | **15.5×** | a2a4 (5/5) |

(Per-rep raw data: `measurements/plan2/m22_race_N{2,3,4}.json`; killed
workers' node counts are chunk-log lower bounds, so inflation is if
anything understated.)

**The tail is not the problem — the floor is.** The OR-early-exit waste
lean report7 flagged is visible (workers burn partial work on children
killed at the winning finish), but the dominant effect is that the
race's best possible wall equals the winning child's *isolated* solve
time (8.7 s), which is already 3.3× the sequential root's total solve
time. Isolated per-child solves are catastrophically more expensive
than the root's own search on this tree:

- the root proves the win in **2.63 s / 858 k nodes** total;
- the cheapest proven-winning child in isolation needs **8.7 s /
  8.59 M nodes** (`a2a4`; the others: `e3f4` 46.0 s / 61.3 M,
  `c3e4` 45.4 s / 71.2 M);
- `g4h5` — the first move of the root's informational 95-ply PV — does
  **not prove in isolation at all within 120 s** (98 M nodes, timeout):
  the root PV is informational, and the root's proof runs through other
  children.

The mechanism: DF-PN's root-level best-first work distribution and the
shared root TT's **cross-child transposition sharing** are precisely
what per-child process isolation forfeits. Adding workers only adds
killed-children partial work (inflation grows monotonically with N),
and a bigger pool cannot help: even an oracle scheduler that puts
`a2a4` first has a lower bound of 8.7 s — 0.30× — because that is the
isolated child's own cost.

### (ii) Decisive-composition agreement (measured)

- **Zero decisive disagreements, zero false proven roots, in all 18
  timed race runs** (5 reps × N ∈ {2,3,4} plus 3 winner-first-order
  reps): every race proved `a2a4`
  and nothing else, matching the sequential per-child reference (child
  `loss` for `a2a4`, `e3f4`, `c3e4` — all also proven-loss in the
  per-child table). No child was proven one way in the race and the
  other way sequentially.
- Repeated-run agreement: the proven move set is identical across all
  15 pre-registered runs (the solver is deterministic; race
  nondeterminism affects only *when* workers are killed, not the
  proven set).
- Perspective-mapping validation vs `find_winning_child`: exact
  agreement on both shallow decisive-suite FENs (dec08, dec03;
  `measurements/plan2/validation_*.json`).
- Stock `find_winning_child` cannot serve as the full-set reference on
  m22 at matched budgets: its timeout is hardcoded at 5 s, far below
  m22's per-child costs — the per-child table (120 s caps) is the
  sequential reference instead. Recorded as a spike limitation.
- Shuffle-win: the N=4 race (120 s caps) consumed all 41 children in
  1320 s with **no proven child** and no contradiction — the sequential
  root proves win in 47.8 s. Under the pre-registered accounting this
  is a budget-mismatch failure of the *race* to reproduce the root
  proof, i.e. decisive-vs-undecided divergence caused by per-child
  budget isolation (documented, not a soundness violation).

### (iii) RAM at N×TT (measured)

Peak RSS per worker (VmHWM, 20 ms sampling) at the default 128 MB TT:
**226–230 MB** across all race runs (`workers[].peak_rss_mb`), i.e.
≈ 1.8× the table size — movegen pools, history/killer tables, path
stacks and allocator slack add ≈ 100 MB over the table alone. The
measured practical maximum N against the container's real 8 GiB cgroup
limit is ≈ 8192/230 ≈ **35 workers** — comfortably above the 4-core
ceiling, so RAM is not the binding constraint at this hardware (CPU
is); on a 16 GB-class host with more cores the same formula applies.
Note the honest correction: the plan assumed 16 GB RAM, but the cgroup
enforces `memory.max = 8 GiB`; the N×TT restatement must be evaluated
against the enforced limit, not the visible `free` total.

### (iv) 8/16-worker projection — **SIMULATION, not measurement**

Deterministic work-conserving schedule over the completed per-child
(wall, nodes) table (43 m22 children, 120 s caps), both in `list_legal`
order and oracle SJF order (shortest proven child first). **These are
simulated numbers** — at 4 cores an oversubscribed wall run would
measure the container, not the architecture.

| N | order | projected wall (s) | projected speedup | aggregate nodes (prorated) | projected inflation |
|---|---|---|---|---|---|
| 8 | list_legal | 8.704 | **0.30×** | 88.3 M | 103× |
| 8 | SJF (oracle) | 8.704 | **0.30×** | 87.5 M | 102× |
| 16 | list_legal | 8.704 | **0.30×** | 178.9 M | 208× |
| 16 | SJF (oracle) | 8.704 | **0.30×** | 178.3 M | 208× |

Sensitivity: the projection is **insensitive to N and to order** — the
projected wall is pinned at the winning child's isolated solve time
(8.704 s), because no scheduler can prove a root child faster than an
isolated solve of that child, and every additional worker only adds
prorated partial work on children that get abandoned. The measured
winner-first race at N=4 (`m22_race_N4_winnerfirst.json`, 9.56 s median)
confirms the simulation's floor directly. More cores make option C
strictly worse in work terms and never better than 0.30× in wall terms
on this case. (`simulate_N8.json`, `simulate_N16.json`; killed
children's work prorated linearly in time — the real partial-work
profile is unknown, so the inflation figures are order-of-magnitude.)

## 4. Constraint-4 RAM restatement (drafted)

> In opt-in parallel mode (N workers), peak RAM ≈ **N × TT +
> per-process overhead** (measured: one worker at the default 128 MB TT
> peaks at ≈ 230 MB RSS, i.e. ≈ 1.8× the table size alone —
> movegen pools, history/killer tables, path stacks and allocator
> slack add the rest). The sequential CLI's "RAM = TT only" contract is
> unchanged; the parallel mode's envelope must be documented at its own
> surface (harness docs). Practical maximum N against the container's
> *enforced* 8 GiB cgroup limit: ≈ 8192/230 ≈ 35 workers — RAM is not
> the binding constraint at 4-core hardware (CPU is), and the N×TT
> accounting must be evaluated against the enforced limit, not the
> `free` total.

## 5. Enrichment path (recorded, not built)

Between this spike's plain root-child race and JLPNS proper lies
PPN₂-style **master-side re-direction**: the master tracks partial
pn/dn of running jobs and re-allocates workers away from hopeless
children (reservation flags, no virtual loss; `research_jlpns.md`).
On our tree the single biggest inefficiency the spike exposes —
measured: the sequential root proves m22 in 858 k nodes while the
winning child alone needs 8.59 M nodes isolated (≈10× cross-child
transposition/interleaving subsidy) — is *structural* for
process-per-child workers and is not
fixable by re-direction; what re-direction can fix is worker
allocation across the *remaining* children after the first
eliminations. Recorded as the next mechanism if a future GO ever
revisits C with persistent workers.

## 6. Decision (Phase 2 rules, pre-registered in plan2.md)

Evaluated on m22 at N=4 (median across 5 runs): **wall speedup ≥ 2.0×**
AND **work inflation ≤ 2.5×** AND zero soundness violations for GO;
MARGINAL at [1.5×, 2.0×) with inflation ≤ 2.5×; NO-GO below 1.5×, above
2.5× inflation, or on any soundness violation.

| rule | threshold | measured | verdict |
|---|---|---|---|
| wall speedup (N=4, median) | ≥ 2.0× (NO-GO < 1.5×) | **0.27×** | NO-GO — fails even MARGINAL by 5.5× |
| aggregate work inflation | ≤ 2.5× | **15.5×** | NO-GO — 6× worse than the worst published Čížek band (2.5× @ 1024 cores) |
| soundness (decisive composition) | zero violations | zero violations (15/15 identical proven sets; perspective mapping validated on dec08/dec03) | pass (moot) |

**Decision: NO-GO.** Option C — the only architecture whose soundness
fits this solver without an L–XL refactor — is measured **3.7× slower
than the sequential solver on its own validation case**, with an
order-of-magnitude work inflation, and the deficit is structural: the
root's cross-child transposition sharing and best-first proof
interleaving (the 858 k-vs-8.59 M node gap) are exactly what
process-per-child workers forfeit, and no worker count, schedule, or
re-direction recovers them. Backlog #3 (staged implementation) never
unblocks. Together with the deterministic-parallelism no-go
(`lean` report7), the parallelism option space closes as measured:
A/B/D excluded by soundness/economics (§1), C by measurement.

**Answers the note owes:**

1. *Does an opt-in nondeterministic parallel mode ship at all?* **No.**
   Lean trigger 1 (a consumer accepting nondeterministic runs) was
   never satisfied, and the spike removes the last economic argument:
   the cheap, sound-by-construction lever is measurably a slowdown, so
   there is nothing worth a consumer's nondeterminism. If the trigger
   is ever satisfied *and* A's gate is cleared (mine Pawlewicz &
   Hayward 2014 — bibliography entry stays **Open** — plus the inert
   TT-concurrency refactor), that is a new decision with new evidence,
   not a reopen of this one.
2. *Which shape?* Moot for shipping; recorded for the record: the only
   shape this solver could ever ship is the **`examples/` harness over
   unmodified solver processes** (C). A CLI `--threads N` (A) remains
   gated exactly as before (mine SPDFPN 2014 first, inert TT
   concurrency refactor, explicit renegotiation of constraints 2–4).
   Both/neither resolves to *neither, now*.
3. *GO/NO-GO for backlog #3?* **NO-GO** — its unblocking condition
   (#2 GO) failed on the pre-registered rules. Backlog #2 closes as a
   measured no-go; the initiative moves toward closure with this note
   and `measurements/plan2/` as the record. `docs/plans/README.md`
   row updated (pivot/close event).
