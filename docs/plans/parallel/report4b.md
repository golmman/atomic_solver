# Report 4b — Sharded-TT re-land, drift protocol re-run, tax pinned (gate PASS)

Executes `parallel` backlog **#4, stage 1b** per `plan4b.md`. One session,
single goal: re-land the preserved plan4 refactor mechanically, prove the
tree identical, re-run the full drift protocol, and pin the sequential
wall tax against the owner's condition-2 budget (≤ +7 % median hard-class
wall).

**Verdict: PASS.** The refactor is re-landed in the working tree; drift is
zero everywhere; the tax is m22 **+5.36 %** / shuffle-win **+4.61 %**
median interleaved wall — inside the ≤ +7 % budget and consistent with
plan4's +6.0 %/+4.4 %. The revert-if-missed clause now transfers to
plan5b (condition 1): a stage-2 NO-GO reverts this refactor and the tax.

## What was done

1. **Baseline capture first.** `cargo build --release` on the clean
   pre-land tree; binary stashed at `/tmp/atomic_solver_pre4b`
   (sha256 `86635244…`), `examples/benchmark` at `/tmp/benchmark_pre4b`
   (sha256 `955d2243…`). Fresh pre-land quick-suite capture matched the
   committed plan4 baseline `baseline_quick_pre.json` **59/59** — no
   environment drift before the land.
2. **Patch applied verbatim**: `git apply …/plan4/sharded_tt_attempt.patch`
   (14 tracked files) + `shard.rs` recreated from
   `plan4/shard_rs_attempt.rs` minus its four-line record header
   (`tail -n +6`), exactly as the plan specifies. Nothing redesigned.
3. **Identity proof**: `cargo fmt` (no diff beyond the patch), `cargo
   clippy --release --all-targets` clean, `cargo doc` clean, `make test`
   green, working-tree scope exactly the recorded 14 files + untracked
   `src/search/tt/shard.rs`; sharded binary `strings`-checked for the
   lock symbols (`"tt shard lock poisoned"`).
4. **Drift protocol (full re-run, all green)**:
   - quick suite `benchmark --suite quick --json --first-outcome --runs 1`:
     `name`/`status`/`outcome`/`nodes`/`child_evals`/`pv_len`/`timeout`/
     `wrong` identical 59/59 vs the plan4 baseline;
   - m22 first-outcome stdout byte-identical (sha256 `b7c74f17…`);
   - shuffle-win first-outcome stdout byte-identical (sha256 `64129ef0…`);
   - snapshot dump identical: sha256 `eaa5f2b9…`, 195 B — **same hash as
     plan4's record**;
   - `test_trajectory_golden` ok (inside `make test`).
   - Note: stderr progress lines (`[bounded_search] chunk done … nps=…`)
     embed wall timings and were excluded from the comparison (stdout
     only); the first comparison attempt that merged stderr showed only
     timing-line diffs, nothing else.
5. **Tax pinned (condition 2)** — interleaved A/B medians in one session:
   - m22 first-outcome, 30 s cap, **10 interleaved pairs**: pre median
     3.009 s vs landed 3.170 s → **+5.36 %** (per-pair +3.34 %..+6.90 %);
   - shuffle-win first-outcome, 100 s cap, **3 interleaved pairs**: pre
     53.363 s vs landed 55.822 s → **+4.61 %** (per-pair +4.49 %..+5.06 %).
   - Budget ≤ +7 % per case: **both PASS**.
   - Plan4-consistency check: plan4 measured +6.0 %/+4.4 %; deltas
     −0.64 pp / +0.21 pp — well inside the ~2 pp profiling-trigger band,
     so no `perf` look was needed. The re-land reproduces the tax, as
     expected for a byte-identical re-land.

## Tools used

Standard toolchain only: `git apply`, `cargo fmt/clippy/doc/test`,
`sha256sum`/`cmp`/`strings`, `bc`+`date` for wall timing, small Python
scripts for JSON comparison and median math. No new example binaries; the
`pin_tax.sh` driver is a throwaway `/tmp` script (its output is committed
as `state/tax_interleaved.json`).

## Problems encountered

- **Latent flaky race in the revived stress test (deviation, fixed).**
  `search::tt::tests::concurrent_store_and_probe_respect_solver_invariants`
  failed on the first `make test` run (and 4/5 when run in isolation).
  Diagnosis: the race is **in the test's own phase-2 assertion**, not in
  the table. A concurrent higher-work solved store from another thread
  can legitimately evict this thread's `Win` entry (k=2 replacement over
  two live solved slots — the unchanged sequential policy), after which
  the thread's *own* phase-2 unsolved store re-inserts the key as a fresh
  unsolved entry. The assertion treated any probe-visible own-key entry
  as "must still be Win", misreading a legal eviction-refill as a policy
  downgrade. Empirically confirmed: every observed failure showed the
  exact fresh-refill shape (`work=7, remaining_depth=0, depth=0, gen=1`)
  — a genuine in-place downgrade is impossible in the store code (the
  unsolved path never overwrites a solved slot). Fix: test-only; the
  phase-2 check now accepts the refill's exact store-argument shape and
  still rejects anything else (10/10 green after the fix, full gate
  green). **The solver/store code is bit-identical to the plan4 attempt**;
  this is a test-correctness fix under the Boy Scout principle, not a
  redesign (the plan's "do not improve" clause targets `shard.rs`, which
  is untouched beyond the prescribed header strip).
  Plan4's `make test` green was luck on this flake — worth knowing for
  plan5, since this suite will run under multi-thread contention again.
- First m22/stdout comparison merged stderr and showed a diff at char 76;
  resolved as timing lines (see above). No code implication.

## Unresolved parts / missing tests

- None for stage 1b's scope. The concurrency-invariant suite covers
  store/probe/eviction invariants under 4-thread contention, but there is
  no test for concurrent `clear()`/`new_generation()` during worker runs —
  explicitly out of scope (sequential-owner contract in the `table.rs`
  module header); plan5 owns that policy and its documentation.
- The tax numbers are from one session on the 4-CPU reference container;
  plan5b's campaign re-measures on the landed build anyway.

## Next steps

- **plan5 (stage 2a)**: `--threads N` SPDFPN mechanism behind the landed
  sharded TT — W-threshold interruptible jobs + same-child resume,
  virtual-TT steering + `TRYRUNJOB`, thread-local repetition caches,
  serialized `ProofEvent`s; N = 1 byte-identical surface; 2/4-thread
  smoke. Prerequisite reading: `research_spdfpn.md`,
  `../dfpn/research_ghi_journal.md` §5, this report.
- **plan5b (stage 2b)**: W sweep locked, 2–4-thread real campaign with
  pre-registered GO bands (≥ 2.0×/4 threads, inflation ≤ 2×, zero
  soundness violations) and the condition-1 revert-if-missed verdict.
- If plan5b returns NO-GO: revert this refactor (`git apply -R` of the
  patch + delete `shard.rs`), re-run the gate, record the revert — the
  tax acceptance dies with it, exactly as pre-registered.

SESSION COMPLETE
- Deliverables: sharded-TT re-landed (14 files + `shard.rs`, one test-only
  fix); `measurements/plan4b/` (README, env.json, drift + tax state);
  `initiative.md` backlog #4 row + Status/History updated;
  `report4b.md` written (this file). Gate: `make test` green; drift
  protocol zero; tax +5.36 %/+4.61 % ≤ +7 % budget. No
  `docs/plans/README.md` row edit (clean pass, per plan).
Follow-up options:
  1. "Execute plan5 (`parallel` stage 2a): implement the `--threads N`
     SPDFPN mechanism per `docs/plans/parallel/plan5.md` over the landed
     sharded TT; N=1 byte-identity surface plus 2/4-thread smoke; write
     `report5a.md`." — the tax is now pinned, so stage 2's prerequisite is
     satisfied and the prototype is the only remaining work on the
     initiative's critical path.
  2. Alternative: hold stage 2 and run `make test-full` first — only
     worthwhile if the re-land should be burn-in-validated before the
     prototype builds on it (the gate plan4b used is the default fast
     tier; the plan does not require the full tier for a byte-identical
     re-land).
