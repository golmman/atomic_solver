# Lean Report 7 — #2 parallel search determinism design spike

Spike executed 2026-09-14 per `plan7.md`. **No `src/` changes landed**;
temporary instrumentation (4 counters + 1 print) was applied, measured,
and fully reverted; the post-revert m22 first-outcome stdout is
byte-identical to the instrumented run (instrumentation touched only
counter increments).

## Decision

1. **Deterministic parallelism (architectures A, B, D): NO-GO, final.**
   Measured ceiling **1.47–1.48×** on both validation cases — below the
   1.5× bar set by the plan, with zero overhead assumed. Demoted with
   numbers.
2. **Lazy-SMP over a shared TT (architecture C): feasible only as a
   strictly opt-in `--threads N` mode; economics doubtful. Parked
   dormant** with documented reopen triggers (below). Not opened as
   plan8.
3. **Next lean lever falls back to #8** (cheaper static scoring, 5.5%
   of the post-plan6 pie) and/or #10; algorithmic levers remain in the
   `dfpn` initiative.

## Phase 0 — desk findings

### The AND-node early exit already exists (kills architecture B)

`selection.rs::solve_child_outcome` applies one uniform rule at both
node classes: a child `Loss` ⇒ parent `Win`; all children `Win` ⇒
parent `Loss`. `children.rs::evaluate_all_children` breaks on the first
`Loss` child **at OR and AND nodes alike** — at an AND node that break
is exactly the first-refuter disproof exit. Consequences:

- Sequential AND-node wall is already "replies up to and including the
  refuter". Parallel reply evaluation can only *overlap* the
  pre-refuter replies (the median 47.1% complement of the 52.9% max
  share) against the refuter's subtree; there is nothing else to save.
- Worse, speculative parallel replies do **more** work than sequential
  (they run past the refuter or are cancelled mid-flight), so
  architecture B trades a `child_evals` regression — the initiative's
  preferred efficiency metric — for a ≤ 1.9× median node-level ceiling
  that the aggregate math below erases anyway.

### Where the deep work actually is

`evaluate_child` never recurses (verified: no `dfpn(` call in
`children.rs`). The deep recursion is the *single selected child* per
frame (`core.rs:290-303`), once per dfpn-loop iteration. "Parallel
siblings" therefore means parallelizing what the selection loop does
not pick — at OR nodes, the 9.4% non-decisive remainder from the `nn`
oracle-floor measurement. This is the mechanism behind the 90.6%
critical-path figure, now confirmed structurally.

### State inventory (verified in code)

As tabulated in `plan7.md`: owned `&mut` TT with generation counter and
max-work/solved-overwrite merge rules (`tt/table.rs::store`), per-thread
history/killers, per-frame pooled slots (`precompute_pool`,
`child_pool`, `eval_state_pool` — per-thread by construction),
`Cell<Instant>` clock sampler (makes `Search: !Sync` today),
`path_stack`/`prefix_path`/`repetition_cache` per-search, and the
proof-event sender whose consumer ties twins on "first created"
(`proof_tree/worker.rs`).

## Phase 1 — measured OR/AND work split (instrumented, reverted)

Method: four temporary `u64` counters on `Search` — `child_evals` and
dfpn entries split by the parent frame's `is_or_node` — plus one stderr
print in `main.rs`. Release build, `--first-outcome --outcome-only`.

| case | or_child_evals | and_child_evals | OR share | or_nodes | and_nodes |
| --- | --- | --- | --- | --- | --- |
| m22_white (`--timeout 30`) | 5,510,656 | 8,645,613 | 38.9% | 156,596 | 701,521 |
| shuffle-win (`--timeout 100`) | 100,877,680 | 148,602,798 | 40.4% | 2,845,278 | 11,062,189 |

The split is stable across both cases: **~40% OR / ~60% AND** by
child-evals (AND frames are 4.5× more numerous but cheaper per frame).

Deterministic sibling-parallel ceiling
`= 1 / (f_OR·0.906 + (1−f_OR)·0.529)`:

- m22: `1 / (0.389·0.906 + 0.611·0.529)` = **1.48×**
- shuffle-win: `1 / (0.404·0.906 + 0.596·0.529)` = **1.47×**

Generous assumptions baked in: zero synchronization overhead, perfect
load balance, the *median* 52.9% AND max-share holding at every node
(half the nodes are worse), and no extra work from speculative
evaluation past the early-exit point. The realistic deterministic
ceiling is lower. Below the bar ⇒ **no-go for A, B, and D**
(D additionally dies structurally: refinement rounds are sequentially
dependent through the `bound = f(previous PV)` chain).

## Phase 2 — lazy-SMP contract analysis

The only survivor is architecture C. It is **incompatible with all four
contracts as written**, but each has a clean renegotiation *if* the mode
is strictly opt-in (`--threads N`, N ≥ 2):

| Contract | Parallel-mode renegotiation | Sequential path |
| --- | --- | --- |
| Drift gate | Protocol always runs at N=1; N ≥ 2 is documented nondeterministic | untouched |
| `child_eval_budget` / `BudgetExhausted` | Parallel mode: budget becomes advisory (per-thread or aggregated post-hoc); `ExitReason::BudgetExhausted` stays reserved for sequential runs | exact semantics preserved (lean non-goal honored) |
| TT snapshots | Dumps defined only for N=1 (or dumped from the finding thread); reconstruct/oracle chains document N=1 | untouched |
| Proof events | Per-thread channels drained in a fixed deterministic order, or parallel mode refuses a proof-event sender | untouched |

Implementation shape (if ever opened): per-thread `Search` + shared
sharded TT (bucket-level atomics or `RwLock`; the `store` merge rules
must be preserved under concurrency), per-thread history/killers and
pools, shared stop flag (already `Arc<AtomicBool>`). Realistic wall
expectation from the proof-search literature: **~1.5–3× at 4 threads,
2–5× at 8+** — not the 2–8× the backlog row hoped, and it buys wall
only on single hard positions, never work reduction.

## Phase 3 — why parked dormant, not opened

The case against opening plan8 now:

- **The staging is a leap of faith**: plan8 (TT concurrency refactor)
  is L-sized and *inert* — no user-visible win until plan9 lands. Two
  large plans before the first measurable benefit.
- **The cost is not engineering time but operational bifurcation**:
  every other initiative's validation methodology (bit-identical drift
  gates, byte-identical stdout, reproducible TT snapshots, the
  dual-build oracle) rests on determinism. A fast nondeterministic path
  permanently forks the workflow into "correct-by-construction" and
  "fast" modes.
- **The alternative levers are deterministic work reductions on the
  same cases**: the `dfpn` algorithmic items (#1–#4: EWS, MOPNS,
  AND-side ordering, bounded cross-path verification) attack the ~60%
  AND-side work directly; #8 attacks 5.5% of the pie. A 2× *work*
  reduction equals the optimistic 4-thread lazy-SMP wall win, keeps
  every gate intact, and each item is session-sized.

**Reopen triggers (documented, checked when touched):**

1. A real consumer need for interactive wall times on single hard
   positions that accepts nondeterministic runs (e.g. an external
   user of the CLI, not the repo's own research workflow).
2. The `dfpn` algorithmic items exhaust without a ~2× work reduction
   on the m22/shuffle-win class.
3. TT concurrency falls out of another initiative for free (making
   plan8's inert-stage cost collapse).

## Verification

- `git diff` on `src/` after revert: clean; `git status` shows only
  `docs/` changes plus this report and `plan7.md`.
- Post-revert sanity: m22 first-outcome `--timeout 30 --outcome-only`
  stdout byte-identical to the pre-revert run (instrumentation could
  not have altered the trajectory, and did not).
- Build: release, no warnings introduced (instrumentation built clean;
  reverted build clean).

## Tools, problems, gaps

- Tools: temporary counter instrumentation (lean spike pattern, plan6
  phase-0 precedent); no `perf` needed — the split is exact, not
  sampled.
- Problems: none material. Note the absolute node totals differ from
  the 2026-09-11 shuffle-win study (13.9M vs 20.3M nodes here) —
  different host/version; the OR/AND *split* is the spike's subject and
  was stable across both validation cases.
- Missing tests: none applicable (no source change).
- Next steps: re-rank the backlog after this report (row #2 status
  update); choose the next lever from #8/#10 (lean) or the `dfpn`
  algorithmic items per the reopen-trigger logic above.
