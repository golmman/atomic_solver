# Research: Young & Hayward 2016 — *A Reverse Hex Solver* (Solrex)

Mining round for `parallel` backlog #1 (plan1). Terminology reused from
`dfpn/research_parallel.md` (virtual numbers, congestion term, stop-set,
TT discipline); Kaneko cross-references point at its §9 figures.

## 0. Version read

- **Primary**: K. Young, R. B. Hayward, *A Reverse Hex Solver*, CG 2016;
  arXiv:1707.00627v1 (26 Apr 2017), 13 pages. Vendored at
  `docs/theory/solrex-2016/solrex-2016.pdf`. Open access.
- Cross-referenced: Čížek et al. 2025 Related Work (for the SPDFPN
  scaling figure); `dfpn/research_parallel.md` §9 (Kaneko's 3.6×/8
  threads, <15% overhead).

## 1. What Solrex is, and what the paper does and does not specify

Solrex solves Reverse Hex (misère Hex) by minimax search over game
states using **Scalable Parallel Depth-First Proof-Number Search
(SPDFPN)** — Pawlewicz & Hayward's 2014 (CG 2013, LNCS 8427) enhanced
parallel variant of Focussed DF-PN (Arneson/Hayward/Henderson's
Solhex), on the same code base (Benzene). Enhancements are domain
theorems: Rex-specific inferior-move pruning and early win detection
via pairing strategies (§2–4); the strongest by far is H-search
(knockout test: omitting it costs **89.83×** on 5×5, §6).

**Concurrency detail is thin in this paper.** What it states about the
parallel engine (§5): "Search follows the Scalable Parallel Depth First
variant of Proof Number Search, with the search focusing only on a
limited number of children (as ranked by the usual electric resistance
model) at one time [14]" — i.e. focussed df-pn: a bounded set of
children held in memory per node, ranked by Anshelevich resistance.
The TT is queried for "the resulting state win/loss value ... either
because we previously solved, or because of color symmetry" — at leaf
level, sequentially described. All locking/sharding/stop mechanics are
delegated to the SPDFPN paper, which is **not mined in this round**
(flagged in §7 as the missing primary source for option A's mechanism).

## 2. The concurrency model — what can be established

- **Threads on one host** (shared memory), SPDFPN: the closest
  published thing to "parallel DF-PN over a shared TT in practice, on
  few threads". Per Čížek 2025's Related Work (independent corroboration
  of the same paper), SPDFPN reached **11.8× on 16 cores** — the
  strongest published shared-memory DF-PN figure, vs Kaneko's
  **3.58× on 8 cores** (`dfpn/research_parallel.md` §9).
- **What is shared**: the TT (solved win/loss values of states), per
  SPDFPN's design; the focussed-children discipline limits per-node
  memory. The paper's own text confirms TT reuse of solved values and
  color-symmetry shortcuts, but carries no per-entry lock/shard
  discussion — the mechanism-level comparison with Kaneko's per-node
  locks + stop-set (virtual pn/dn congestion, Mark/Unmark) **cannot be
  completed from this paper alone**; SPDFPN's own description is
  required. What the *combination* of sources supports: SPDFPN is the
  only shared-memory DF-PN variant with a published double-digit
  core-count speedup, and its changes vs Kaneko are (per its title and
  the focussed-search citation) memory-focussing + scalability
  engineering, not a different cooperation protocol.
- **Difference from Kaneko that is visible here**: resistance-ranked
  child focusing (few children kept per node — Kaneko's parallel loop
  reads *all* children's entries each visit, `dfpn/research_parallel.md`
  §5.7) and a modern (2014) sharded-TT implementation. The economics of
  the extra focusing under concurrency are exactly what the small-pool
  numbers below measure.

## 3. Measured overhead and speedup at low core counts (§6)

- **Hardware**: Torrington, quad-core i7-860 2.8 GHz with
  hyper-threading (8 pseudo-cores).
- **The few-thread measurement**: all 18 (up to symmetry) 1-move 6×6
  openings — 134,635 s **single-thread** vs 25,900 s on **four threads**
  ⇒ **≈5.2× on 4 threads**. The hardest 6×6 opening solves in under
  4 hours on four threads. (5×5 suite, all 24 replies to the acute
  corner: 13.2 s.)
- **Superlinear caveat**: 5.2× on 4 threads exceeds perfect scaling.
  The paper does not remark on it; plausible causes are cache/TT-memory
  effects (each thread touches a smaller working set) or HT — it does
  not report a 2-thread point, so no clean curve exists.
- **Search-work inflation vs the sequential solver (our secondary
  metric): not reported.** The 1-thread vs 4-thread comparison is wall
  time only; no node/child-eval counts, no Kaneko-style "<15%
  overhead" figure. The paper's ablations are all *feature* knockouts
  (H-search 89.83×, inferior pruning 2.30×, capture fillin 2.02×,
  resistance ordering 1.62× on 5×5) — parallelism itself is never
  ablated against its own 1-thread configuration with work counts.
- **Comparison with Kaneko's figure**: 5.2×/4 threads is *better per
  thread* than Kaneko's 3.6×/8 (≈0.45×/core per-thread efficiency
  above 1 — but the superlinearity forbids a precise per-core
  reading), and the domain (deep, pruning-heavy Hex solving with a
  large TT) is closer to our workload shape than shogi tsumes.
  It matches the optimistic end of lean report7's "realistic wall
  expectation: ~1.5–3× at 4 threads" and beats it — while being
  consistent with lazy-SMP economics *only if* the superlinear bonus is
  discounted, which the paper's data does not allow.

## 4. Domain transfer risks

- **Hex (and Rex) is position-monotone**: stones are only added, the
  game is finite, and there are no repetitions and no draws. The paper
  offers **no evidence on constraint 2** (no false decisive outcomes
  under cross-worker reuse of repetition-dependent results) — silence
  is the finding, and the journal contract
  (`dfpn/research_ghi_journal.md` §5) stays load-bearing for us.
- **TT contents under our semantics**: everything SPDFPN shares across
  threads — solved win/loss values of positions — is, in our setting,
  exactly the class whose *cross-path* reuse is rule-sound for
  decisive facts but **banned cross-worker by the initiative's
  plan10-hazard non-goal**, and our path-dependent objects (draws,
  repetition-context-dependent results) have no Hex analogue, so the
  paper never had to solve our problem. A shared-TT port would carry
  the journal contract at every store/lookup site, not just at
  combination points.
- Also untransferable: color-symmetry TT shortcuts and pairing-strategy
  win detection are Hex-specific; the resistance model would have to be
  replaced by our `StaticAtomicScorer` ranking (`src/search/ordering.rs`)
  if focussed-children were adopted.

## 5. Mapping section — option A verdict

- **Mechanism.** N threads, one shared (sharded/locked) TT of
  pn/dn/solved values, each thread running DF-PN independently with
  focussed (bounded) children per node; congestion/stop coordination
  per SPDFPN's design (unverified from this paper — §2).
- **Where it would live in our code.** Deep in the solver:
  `src/search/tt/` (sharding, entry locking, concurrent store merge
  rules — today's `tt/table.rs::store` merge semantics under
  concurrency), `src/search/dfpn/` (per-thread history/killers/pools;
  the `Cell<Instant>` clock sampler makes `Search: !Sync` today, lean
  report7 phase 0), a stop flag (already `Arc<AtomicBool>` shape). This
  is the parked `--threads N` surface, and the L-sized inert TT
  concurrency refactor (lean plan8 shape) is the price of admission.
- **Expected surface.** `--threads N` on the CLI; documented
  nondeterminism for N ≥ 2 (drift protocol runs at N=1); TT snapshots
  defined at N=1; per-thread event channels or none.
- **Soundness-contract verdict.** Absent-and-load-bearing: Hex is
  monotone, so constraint 2 is unaddressed; sharing a mutable TT makes
  the journal contract bind at every TT site (cross-worker path
  contexts differ by construction), and the merge rules + repetition
  caching semantics (`src/search/dfpn/repetition_cache.rs` is
  per-search today) would need a per-worker-context redesign — the
  exact place where the plan10 hazard and the first-player-loss
  serialization hazard live.
- **Effort class.** L–XL (TT concurrency refactor is the inert first
  plan; the win only arrives after the second).
- **Go/no-go input for the architecture note.** **Weak GO behind
  C/B**: the empirical economics at 4 threads (≥5×, possibly
  superlinear) are the best published evidence that shared-TT DF-PN
  pays at small pools, but (i) the mechanism detail is in an unmined
  source (SPDFPN 2014 — mine it before any plan-2 commitment to A),
  (ii) no work-inflation figure exists to compare against our
  `child_evals` gate, and (iii) the soundness surface is the largest of
  A–D. A is a second-stage lever, not the first.

## 6. Summary table row (plan1 synthesis)

| Mechanism | Option fed | Where it lives in our code | Soundness-contract verdict | Expected 4–16 core speedup class | Effort | Go/no-go input |
|---|---|---|---|---|---|---|
| SPDFPN: shared-TT threaded DF-PN with focussed (resistance-ranked) children; solved-value TT sharing | A | `src/search/tt/` sharding + merge rules, `src/search/dfpn/` per-thread state, `--threads N` | Absent-and-load-bearing (monotone domain; journal contract binds at every TT site; repetition-cache redesign) | 5.2×/4 threads measured (superlinear caveat); 11.8×/16 for SPDFPN per Čížek's citation — optimistic end of lazy-SMP economics | L–XL | **Weak GO as second stage**: mine Pawlewicz & Hayward 2014 first; keep C/B ahead of it |

## References

- K. Young, R. B. Hayward, *A Reverse Hex Solver*, CG 2016 /
  arXiv:1707.00627. Vendored: `docs/theory/solrex-2016/solrex-2016.pdf`.
- J. Pawlewicz, R. B. Hayward, *Scalable Parallel DFPN Search*, CG 2013,
  LNCS 8427, pp. 138–150 — the concurrency mechanism's primary source.
  **Not mined this round; the next source if option A reopens.**
- B. Arneson, R. B. Hayward, P. Henderson, *Solving Hex: Beyond Humans*,
  CG 2010, LNCS 6515 (Solhex: focussed df-pn, H-search).
- T. Kaneko, *Parallel Depth First Proof Number Search*, AAAI-10.
  Mined: `dfpn/research_parallel.md`.
- T. Čížek, M. Balko, M. Schmid, arXiv:2511.10339v2. Mined:
  `research_cizek2025.md` (this directory) — source of the SPDFPN
  11.8×/16-cores figure used in §2.
