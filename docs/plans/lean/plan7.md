# Lean Plan 7 — #2 parallel search: determinism design spike (no-go/go)

Backlog row **#2** ("Parallel search", L–XL) is the only remaining
multiplicative lever after plan6, and the backlog row itself gates it:
*"needs a determinism design spike for the `child_eval_budget` contract
first"*. This plan **is** that spike. It is a design session: **no `src/`
changes land** (temporary instrumentation is allowed under the lean
spike pattern — plan6 phase 0 is the precedent — and must be fully
reverted). The deliverable is a decision: an architecture choice with a
staged implementation sketch, or a **no-go** that demotes #2 with
numbers.

Sized to one session. One lever: the *decision about parallelism*; no
other backlog item is touched.

## Why now (from the post-plan6 profile, 2026-09-12)

Every sequential micro-lever below the ~3% bar is exhausted: #12a/#13/#14
done, #17 and #12b spiked-and-demoted, make/unmake (31.9%) is the
measured-tight upstream core, the child-eval loop (26.9%) is the
algorithm itself. The remaining single-cluster micro-lever (#8 static
scoring, 5.5% share) is bounded, while #2 is the only item with a 2×+
ceiling. Before spending M–XL implementation effort, the ceilings and
contract conflicts must be quantified.

## Grounding facts (measured or in-code; the spike verifies, not re-derives)

**Sequential-coupling ceilings from prior measurements** (the `nn`
oracle-floor work and the #5 sizing, see initiative Motivation):

- **90.6% of OR-node work is spent on the decisive child.** Any scheme
  that parallelizes *sibling OR children* has an Amdahl ceiling of
  ≈ 1/(0.906) ≈ **1.10×** — near-worthless. This alone likely kills
  architectures A/D-OR below unless the measurement is re-interpreted.
- **AND-node replies concentrate**: median max child-share 52.9% →
  perfect reply-parallel AND evaluation ceilings at ≈ **1.9×** per AND
  node, before overheads.
- **Root fan-out** is 41 on the shuffle-win case, but the sequential
  solver stops at the first decisive child, so root-parallel solving
  multiplies total work by the number of children tried — a wall-only
  trade against the `child_evals` efficiency metric.

**Shared mutable state on `Search`** (the inventory the refactor cost
estimate starts from; all in `src/search/dfpn/`):

| State | Location | Sharing obstacle |
| --- | --- | --- |
| TT (owned, `&mut` store) | `search/tt/table.rs` | generation counter + max-work merge + solved-overwrite rules; needs sharding/atomics or per-thread tables |
| history / killers | `dfpn/mod.rs:169-171` | per-thread is fine (lazy-SMP convention) but changes trajectories |
| `path_stack`, `prefix_path` | `dfpn` | inherently per-search; `path_contains` scans are per-thread |
| `repetition_cache` | `dfpn/repetition_cache.rs` | keyed per run; per-thread OK, cross-thread sharing needs concurrency |
| `precompute_pool`, child vectors, sort buffers | `dfpn/children.rs` | per-depth slots — must be per-thread by construction |
| clock sampler `Cell<Instant>` | `dfpn/mod.rs:183-185` | makes `Search` `!Sync` today; per-thread is natural |
| `child_evals` / `nodes` counters | `dfpn` | the *deterministic budget contract reads these* |
| proof-event sender | `set_proof_event_sender` | the proof-tree worker's twin selection ties on "first created" — interleaved emission makes tree construction nondeterministic |

**The contracts any parallel mode must satisfy or explicitly renegotiate:**

1. **Drift gate** (working agreement): quick suite `child_evals`
   bit-identical; m22 first-outcome stdout byte-identical; m22
   default-mode chunk trajectory bit-identical. Precedent for deliberate
   change exists (plan4) but requires suite validation and a documented
   redefinition.
2. **`child_eval_budget` / `ExitReason::BudgetExhausted`** (AGENTS.md,
   lean non-goals): the deterministic budget "must keep working exactly
   as documented" **for the sequential path**. An opt-in parallel mode
   may have different semantics only if the sequential path is
   untouched.
3. **TT snapshot chain**: `--tt-dump-path` feeds offline proof
   reconstruction (`reconstruct_pt`) and the dual-build oracle;
   run-to-run reproducible dumps are what makes those experiments
   comparable.
4. **`ProofEvent` protocol**: worker twin tie-break is "first created";
   parallel emission order changes the finalized tree.

## Phase 0 — Architecture candidates and their ceiling math (desk work)

Evaluate, against the grounding facts and the contracts:

- **A. Sequential-equivalent parallel child evaluation** — fixed work
  decomposition, children evaluated concurrently, merged in sequential
  order so TT store order and trajectory are bit-identical. Ceiling =
  Amdahl over the per-node critical path; likely ≪ 1.5× given the 90.6%
  OR concentration. Deterministic: **all gates intact**.
- **B. Parallel AND-reply evaluation with early disproof exit** —
  deterministic merge (first disproof in fixed reply order wins, others
  cancelled). Ceiling ≈ 1.9× on AND-heavy positions; OR-heavy work
  (90.6% figure) dilutes the aggregate. Deterministic if reply order is
  fixed.
- **C. Lazy-SMP over a shared TT** — N threads search the same root with
  diversified ordering; first decisive result wins. Nondeterministic:
  breaks gates 1–3 as written, changes the proof-event stream. Highest
  wall potential on hard cases.
- **D. Parallel refinement roots / phase-2 parallelism** — the
  refinement rounds are sequentially dependent (bound = f(previous PV));
  intra-round parallelism collapses to A. Likely no-go on structure.
- **E. Process-level parallelism outside the solver** (suite cases,
  reconstruct experiments) — already possible; note it as the
  baseline "free" parallelism the solver gets no credit for.

For each: ceiling, determinism verdict per gate, refactor cost (from the
Phase-0 inventory), and what a minimal PoC would look like.

## Phase 1 — Ceiling quantification (desk work; one instrumented run only if needed)

The 90.6% / 52.9% figures came from different measurement campaigns and
may not compose. If Phase 0's math is ambiguous on the *aggregate*
OR/AND work split, run **one** temporary-instrumentation pass on the two
validation cases (m22_white first-outcome `--timeout 30`; shuffle-win
first-outcome `--timeout 100`) recording per-node-class child-eval
totals, then **revert completely**. Decision rule: if the best
deterministic architecture's ceiling is < 1.5× aggregate, the spike
trends no-go regardless of contract work.

## Phase 2 — Contract negotiation (the actual spike question)

For the surviving 1–2 architectures, write down precisely:

- Which gates break, and the concrete redefinition (e.g. "parallel mode
  is opt-in via `--threads N`; drift protocol runs with `--threads 1`;
  `child_eval_budget` in parallel mode is per-thread or becomes
  best-effort with `BudgetExhausted` reserved for sequential").
- TT snapshot reproducibility policy (e.g. dumps only defined for
  single-thread runs).
- Proof-event serialization policy (per-thread channels drained in
  fixed order, or parallel mode refuses a proof-event sender).
- Memory budget: N × TT or sharded shared TT — the RAM = TT contract of
  the search CLI.

## Phase 3 — Decision and staging

- **Go**: name the architecture, the gate redefinitions, the validation
  protocol (which suite replaces the drift gate for the parallel mode),
  and the staged plan sequence (expected: plan8 = TT concurrency /
  `Search` Send refactor measured inert, plan9 = parallel mode behind
  `--threads`, plan10 = tuning). Each stage independently revertible.
- **No-go**: demote #2 to "spiked, below bar / contract-infeasible"
  with the ceiling numbers and the contract inventory; the initiative's
  next lever falls back to #8 (static scoring) and the `dfpn`-side
  algorithmic items.

## Acceptance criteria

- A decision (go/no-go) backed by numbers, recorded in `report7.md`.
- The state-inventory table and per-architecture matrix land in the
  report (or a `research_parallel.md` next to it, report-referenced).
- `src/` is byte-identical before/after the session (`git status`/`git
  diff` clean apart from `docs/plans/lean/`).
- No drift-protocol run is invalidated; if instrumentation was used, a
  post-revert sanity run (m22 first-outcome, short timeout) matches the
  pre-spike stdout.

## Final task

Write `report7.md` in this directory (findings, decision, next-plan
proposal), and update backlog row #2's status line accordingly.
