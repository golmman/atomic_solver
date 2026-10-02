# Plan 4b — Re-land the sharded-TT refactor, re-run the drift protocol, pin the tax

Executes `parallel` backlog **#4, stage 1b**. Sized for one sitting.

**Owner decision, 2026-10-03 (later the same day; `initiative.md` Status,
`report4.md` addendum):** the plan4 kill verdict stands as the measured
record, but the owner consciously accepts the measured TT-concurrency tax
as the entry fee for the A-stage — the only remaining multiplicative
lever. Three conditions are pre-registered with the acceptance; this plan
executes the re-land under condition 2:

1. **Revert-if-missed** — the tax is accepted only while plan5 proves out
   (GO bands in `plan5b.md`); a miss reverts the refactor and the tax.
2. **Explicit gate, not a waiver** — the plan4 gate (≤ +2 % wall, measured
   dead at +6.0 %/+4.4 %) is replaced by a hard budget: **≤ +7 % median
   hard-class wall** (m22 / shuffle-win first-outcome, vs the unsharded
   baseline), byte-identity surface unchanged.
3. **Measurement honesty** — 2–4 thread wall measurements are real
   (4-CPU container); N ≥ 8 simulated only (plan5b's concern).

## Starting state

- Working tree is the byte-identical sequential solver (plan4 kill
  actions: clean `git diff`, gate re-run green).
- The complete, final refactor is preserved and **revert-verified**:
  `measurements/plan4/sharded_tt_attempt.patch` (14 tracked files) +
  `measurements/plan4/shard_rs_attempt.rs` (the untracked
  `src/search/tt/shard.rs`, outside `git diff`). plan4's recorded scope:
  `src/search/tt/{table.rs,mod.rs,tests.rs}`, `src/reconstruct/{mod.rs,walker.rs,tests.rs}`,
  `src/search/dfpn/{core.rs,children.rs,pv.rs,mod.rs,tests.rs}`,
  `src/tt_snapshot/{mod.rs,tests.rs}`, `examples/campaign_worker.rs`.
- Plan4's measured record: zero deterministic drift everywhere
  (quick-suite `child_evals` bit-identical 55/55, m22/shuffle-win stdout
  byte-identical, snapshot dump sha256 `eaa5f2b9…`/195 B identical,
  `make test` green) — but wall +6.0 % (m22) / +4.4 % (shuffle-win)
  first-outcome median vs the then-gate of ≤ +2 %.

Because the patch itself never drifted and the invariant tests found no
defect, re-landing is mechanical: apply, recreate `shard.rs`, prove the
tree identical to the plan4-era build, then re-run the protocol and pin
the tax against the new budget.

## Goal

`src/search/tt/table.rs` sharded into contiguous power-of-two chunks
(`shards = min(bucket_count, 256)`, each behind a `std::sync::RwLock`),
interior-mut `probe`/`store`/`clear`/`new_generation` on `&self`,
`Send + Sync`, `AtomicU32` generation (policy unchanged), store semantics
verbatim inside one write-locked per-shard critical section, `probe` →
`Option<TtEntry>` by value, `entries()` → `for_each_entry` (native bucket
order — the snapshot byte-identity contract), `Search::tt_mut()` removed.
No other behavior change. The design decisions are pre-registered in
`plan4.md` (§Design decisions) and are not reopened here.

## Implementation steps

1. **Capture the unsharded baseline binary first.** `cargo build
   --release` from the current (pre-land) tree; copy the binary aside
   (e.g. `/tmp/atomic_solver_pre4b`); record its hash. This binary is the
   wall baseline *and* the drift reference for discriminating
   environment drift from real drift. (RUSTFLAGS builds silently
   overwrite `target/release` — keep default flags, hash-verify before
   every comparison, `strings`-check for the sharded build's lock
   symbols where identity matters.)
2. **Apply the patch**: `git apply
   docs/plans/parallel/measurements/plan4/sharded_tt_attempt.patch`.
3. **Recreate `src/search/tt/shard.rs`** from
   `measurements/plan4/shard_rs_attempt.rs` **minus its record header**:
   drop the four leading `//` lines ("Preserved record of plan4's …")
   and the blank line before the first `//!`; everything from `//!
   Per-shard storage …` down is the exact file (module doc with the
   shard/index invariant, `MAX_SHARDS = 256`, `Shard`, `layout()`).
   Do not "improve" it — plan4b re-lands, it does not redesign; the
   seqlock alternative is measured-and-rejected (`report4.md`).
4. **Identity proof**: `cargo fmt` (expect no diff beyond the patch),
   `cargo clippy`, `cargo doc` clean; `make test` green — including the
   concurrency-invariant tests that revive with the patch (shard/index
   invariant vs reference bucket enumeration, 4-thread store/probe
   stress over `Arc`, layout partition test, `Send`/`Sync` assertion)
   and the byte-identity golden suites. `git diff` scope check against
   the recorded 14-file list; `shard.rs` present.
5. **Drift protocol (full re-run)** — the gate's identity surface:
   - quick suite `benchmark --suite quick --json --first-outcome
     --runs 1`: `child_evals`/`nodes`/`outcome`/`pv_len` bit-identical
     per case (vs `measurements/plan4/state/baseline_quick_pre.json`;
     on any mismatch, first re-capture a fresh pre baseline with the
     saved binary to separate environment drift from real drift);
   - m22 first-outcome stdout byte-identical:
     `--fen '4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22'
     --timeout 30 --first-outcome --outcome-only`;
   - shuffle-win first-outcome stdout byte-identical:
     `--fen '4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21'
     --timeout 100 --first-outcome --outcome-only`;
   - snapshot dump bytes identical: shallow case
     `--fen '4k3/8/8/8/8/8/8/4KRR1 w - - 0 1' --timeout 5 --first-outcome
     --tt-size 16 --tt-dump-path …`, sha256 `eaa5f2b9…`, 195 B;
   - `test_trajectory_golden` byte-identical (m22 default-mode golden).
6. **Pin the tax (condition 2)** — interleaved A/B wall medians in one
   session, saved pre binary vs landed build:
   - m22 first-outcome (30 s cap): 10 interleaved pairs, median delta;
   - shuffle-win first-outcome (100 s cap): 3 interleaved pairs (plan4
     used 1 — the budget-level decision deserves more than one pair).
   Budget: **≤ +7 % median, per case.** Consistency check: plan4
   measured +6.0 %/+4.4 % — a materially different re-land number
   (|Δ| > ~2 %) triggers a profiling look (`perf record -e cpu-clock`
   per the AGENTS.md recipe) before the verdict is recorded, to rule out
   an environment shift rather than a code difference.
7. Record results under `measurements/plan4b/` (layout per AGENTS.md:
   commit `README.md` provenance table, `env.json`, `state/*.json`;
   logs are `.gitignore`d).

## Non-goals

- No redesign of the locking mechanism (seqlock rejected on measurement,
  `report4.md`); no shard-count retuning (256 is sequential-cost-neutral;
  real-contention tuning is plan5b's to revisit only if the campaign
  implicates it); no `--threads`, no worker threads, no SPDFPN machinery.
- No snapshot-format, movegen, dfpn-logic, repetition-cache, ProofEvent,
  or CLI change. Concurrent `clear()`/`new_generation()` while workers
  run remains out of scope (plan5 owns the policy).
- No wall optimization: the tax is accepted up to the budget, not
  engineered away here.

## Kill point / gate breach

- **Byte-identity failure at step 5**: stop-and-bisect (the patch was
  round-trip verified at the kill, so a failure points at
  environment/toolchain drift — rebuild both binaries from the identical
  working copy before suspecting the patch).
- **Either hard-class median > +7 %**: do not silently redesign or
  accept. The owner's acceptance was conditioned on the *measured* ~5 %
  tax; a materially worse re-land is a gate breach — record the numbers,
  revert to the clean sequential tree, update the initiative status, and
  put the decision back to the owner (`report4b.md`).

## Deliverables

- Code: the re-landed sharded TT (steps 2–4) — identical to the plan4
  attempt by construction.
- `measurements/plan4b/` — provenance README, `env.json`, drift + tax
  results (`state/*.json`).
- `initiative.md` — backlog #4 row: stage 1b landed, pinned tax numbers
  vs the ≤ 7 % budget.
- `docs/plans/README.md` `parallel` row — updated **only** on a gate
  breach / kill; a clean pass needs no row edit.
- `report4b.md` in this directory (final task): drift results, pinned
  tax medians vs budget, plan4-consistency check, problems, plan5
  kickoff inputs.
