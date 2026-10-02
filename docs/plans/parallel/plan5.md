# Plan 5 — SPDFPN prototype, stage 2a: `--threads N` mechanism (implementation + smoke)

Executes `parallel` backlog **#4, stage 2a**. Sized for one sitting.

**Numbering note (deviation from the backlog text, declared up front):**
stage 2 as scoped (full SPDFPN mechanism *and* the interleaved wall
campaign) is L–XL and does not fit one session, so it is split:
**plan5 = this plan** (mechanism implementation + 2/4-thread smoke) and
**plan5b** (measurement campaign, pre-registered GO bands, the
revert-if-missed verdict). The backlog's "final task = report5.md" lands
with **plan5b**, which reports the whole stage; this plan's final task is
`report5a.md`. Reference design: `research_spdfpn.md` (§1 mechanism,
§3 soundness mapping, §4 bill of materials, §5 staging). Prerequisite:
plan4b landed (tax pinned ≤ +7 %; `report4b.md`).

Prerequisite reading: `parallel/initiative.md` (design constraints 1–4,
backlog #4), `research_spdfpn.md` (whole document — the pre-registered
reference design), `../dfpn/research_ghi_journal.md` §5 (why the
repetition contract is load-bearing), `../dfpn/research_parallel.md`
(Kaneko — the contrast the paper itself draws), `plan4b.md` /
`report4b.md` (the shared-TT foundation this builds on).

## Goal

An **opt-in** SPDFPN-style parallel DF-PN behind `--threads N` (N ≥ 1,
default 1): N workers over one shared sharded TT, work-threshold
interruptible jobs with a same-child resume clause, virtual win/loss
steering via a virtual TT + job lock + `TRYRUNJOB` assignment,
thread-local repetition caches, serialized `ProofEvent`s. With `--threads
1` the solver is **exactly today's code path** (byte-identical surface);
with N > 1 the run is nondeterministic in *which* valid proof wins and in
work counts — **never a false decisive outcome** (the accepted
nondeterminism envelope, owner premise 2026-10-02).

## Pre-registered design decisions

1. **CLI**: `--threads N` in `main.rs` (self-contained parsing per the
   module header; unknown options still exit with an error). Help text
   documents: N = 1 is the deterministic sequential solver; N > 1 is
   nondeterministic (which valid proof wins, work counts, event
   ordering), and `child_eval_budget` exhaustion → `Draw` +
   `ExitReason::BudgetExhausted` becomes **advisory** under N > 1
   (constraint 3 restated, not silently broken).
2. **N = 1 bit-identity**: the parallel machinery (scoped threads,
   coordinator, virtual TT, job lock) is constructed **only when N > 1**;
   the sequential path touches none of it. The sharded TT itself is
   already unconditional (plan4b) — its deterministic surface is the
   drift gate, its wall tax the accepted budget.
3. **Work threshold `W` (MaxWorkPerJob)**: a `dfpn::parallel` constant,
   unit = **child_evals** (our cumulative TT `work` — deterministic, the
   optimizer's metric; not the paper's DFPN calls). Initial W = 1000;
   the exact value is swept and locked in plan5b (paper used 100–500
   calls; our nodes are cheaper, so a larger multiple is expected —
   research §4 risk 1).
4. **Same-child resume clause** (SPDFPN §1 item 1): a job interrupted at
   its W cap may resume; on resumption `SELECT` keeps child j1 while
   `p1 < (1 + ε) · p2` (ε = the solver's 0.125) instead of re-selecting
   from scratch — the modification that makes a work-capped dfpn a
   *resumable job*, not a restart.
5. **Virtual TT + job lock + `TRYRUNJOB`** (§1 items 2–3): per-ply
   arrays of ≤ N virtual entries (node entered at most once per thread —
   O(depth × N) memory); one **job Mutex** held only during
   candidate-finding and end-of-job bookkeeping, released while the
   assigned thread solves. Assignment candidates: nodes on the
   root→mpn path whose past work `n.w < W` (the TT `work` field),
   closest to the root. A wrong virtual win/loss costs **work only** —
   correctness is carried by the underlying 1+ε dfpn rules, exactly the
   paper's failure mode.
6. **Thread-local vs shared state** (the soundness core; research §3):
   - *Shared*: the sharded TT — whose payload is path-independent by
     construction (repetition-dependent results are never cached;
     first-player-loss GHI shortcut). Cross-thread reuse of a decisive
     entry within one run is the *same* reuse the sequential solver
     performs at that key; the plan10 cross-worker ban is about a
     global store across separate solves and does not cover in-run
     thread sharing. Write safety: per-shard critical section
     (solved-never-downgraded, work monotone — plan4's invariant tests).
   - *Thread-local (never shared, never merged)*: the per-run
     repetition-draw cache (path-dependent by design — sharing across
     paths is the one sharp false-result edge), history, killers,
     pooled movegen slots.
7. **ProofEvents**: the sink is wrapped in a Mutex — emissions are
   serialized; **ordering is nondeterministic under N > 1** (documented).
   The proof-tree worker/reconstruct contract is untouched mechanically
   (offline pipeline; tree building stays out of the search CLI).
8. **Timeout / stop**: a shared `AtomicBool` stop flag checked on the
   existing deadline checkpoints; wall-clock timeout remains wall-clock
   (it is not a deterministic surface today either).
9. **RAM (constraint 4)**: one shared TT (unchanged size); per-thread
   state is N × small (caches, killers, pools). "RAM = TT only" is
   restated in the `main.rs` header as "RAM = TT + O(N) small state".
10. **Shape**: `src/search/dfpn/parallel/` submodule(s) — `mod.rs`
    (coordinator/worker loop), `jobs.rs` (W-cap frames, resume state,
    `TRYRUNJOB`), `virtual_tt.rs` (virtual entries + overlay reads);
    each file < ~10 KB. `std::thread::scope` borrowing `&Search` —
    **no `Arc`, no public API change** beyond the flag. Files stay under
    the size convention; the per-run repetition cache gains a
    per-worker instantiation (its module doc updated).

## Implementation steps (drift-gated, bisectable)

1. **Baselines** (post-plan4b build, release): quick-suite `--json`,
   m22 + shuffle-win first-outcome stdout, snapshot sha256 — under
   `measurements/plan5/`.
2. **CLI flag + scaffolding** (`--threads`, parse/validation, N = 1
   dispatch is a no-op). Drift check: full byte-identity + `make test`.
3. **W-threshold + resume clause** in the dfpn frame/selection (dead
   code at N = 1). Drift check: byte-identity (proves the dead code is
   inert on the sequential path).
4. **Virtual TT + `TRYRUNJOB` + coordinator + worker loop**. Drift
   check at N = 1 unchanged; first 2-thread m22 run completes with the
   sequential decisive outcome.
5. **Thread-local worker state + serialized events**. Smoke: 5 × m22 at
   2 and 4 threads — outcome agreement with sequential every run, no
   panic, plausible per-thread work split (record under
   `measurements/plan5/`; *not* the GO campaign — that is plan5b).
6. **A fast-tier integration test** (`tests/`): small case, 2 threads,
   outcome agreement + no-torn-state invariants (~1 s; slow-tier
   stress variants if needed are `#[ignore = "slow: …"]`, never
   profile-gated).
7. **Docs**: `dfpn/parallel` module doc = the soundness contract
   (decisions 5–6 written out: why no false decisive outcome, what is
   shared vs thread-local, the budget/timeout restatements);
   `main.rs` help + header; `AGENTS.md` architecture lines gain the
   `--threads` mention only at stage completion (report5a task).
8. `cargo fmt`, `cargo clippy`, `cargo doc` clean; `make test` green.

## Non-goals

- No N ≥ 8 *real* measurements ever (condition 3 — simulated only, and
  only in plan5b); no wall-speedup claims from this plan.
- No replacement-policy change (the paper's k=4 least-work collision
  probing stays out of scope; our k=2 per-shard stands — if TT pressure
  shows up in plan5b, that is a finding, not this plan's work).
- No sharding-count retuning; no W tuning (plan5b sweeps, then locks).
- No proof-tree/pipeline changes beyond the Mutex-wrapped sink.
- No `--first-outcome`/`--outcome-only`/`--tt-dump-path` semantics
  change under N > 1 (they remain valid; snapshot dump order stays
  deterministic via `for_each_entry`).
- No default-on parallelism: the sequential solver remains the product.

## Risks

- **Cheap nodes vs W**: atomic-chess nodes are much cheaper than Hex's —
  coordination overhead weighs more; expect efficiency below the paper's
  0.74/16 and possibly below the GO band at 4 threads. That is plan5b's
  measurement, not a reason to pre-tune here.
- **AND-node virtual-win mapping**: the p≤d virtual rule must be
  perspective-checked at AND nodes (research §4 risk 3); divergence
  costs work only, but a *perspective inversion* could steer workers
  onto the same subtree — the smoke's per-thread work split is the
  early detector.
- **Job-lock contention/livelock**: the lock is held only during
  candidate-finding; the wall timeout bounds any pathology.
- **W too small** → fragmentation; **too large** → tail imbalance. The
  resume clause absorbs part of this; plan5b's sweep quantifies it.

## Kill points

- **N = 1 drift cannot be restored** after a bounded bisect: fix or kill
  stage 2a. (The landed TT itself stays — it is owned by plan4b's ≤ 7 %
  budget, not by stage 2a.)
- **Any decisive disagreement vs sequential in the smoke, or any
  panic/torn state**: that is a soundness violation — stop; the
  prototype is dead under the band rules regardless of speedup. Record
  the failing case in `report5a.md`; plan4b's re-land then falls to the
  revert-if-missed clause (executed in plan5b).
- **Unresolvable deadlock/livelock** in the coordinator: kill.

## Deliverables

- Code: `src/search/dfpn/parallel/` (+ minimal dfpn hooks), `main.rs`
  `--threads`, repetition-cache per-worker instantiation, integration
  test.
- `measurements/plan5/` — provenance README, `env.json`, baselines +
  smoke results (`state/*.json`; logs gitignored).
- `initiative.md` — backlog #4 row: stage 2a built (smoke numbers).
- `docs/plans/README.md` `parallel` row — updated **only** on a kill or
  a stage-2 soundness kill; a clean pass needs no row edit.
- `report5a.md` in this directory (final task): what landed, smoke
  results, deviations from the pre-registered design, plan5b kickoff.
