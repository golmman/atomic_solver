# Report 4 — Inert TT-concurrency refactor (A-stage prerequisite)

Executes `parallel` backlog **#4, stage 1** (`plan4.md`). **Verdict: the
pre-registered drift gate failed on the wall surface — the A-stage dies at
the cheap kill point, as designed.** Backlog #4 is closed as NO-GO; the
sequential solver is byte-identical to the pre-plan4 state (code reverted,
gate re-run green).

## What was built (and reverted)

The full pre-registered refactor was implemented and worked:

- `TranspositionTable` (`src/search/tt/table.rs`) sharded into contiguous
  power-of-two chunks (shards = min(buckets, 256)), each behind a
  `std::sync::RwLock`; shard selection derived from the bucket index
  (shard/index invariant in `src/search/tt/shard.rs`).
- Interior mutability: `probe`/`store`/`clear`/`new_generation` on `&self`;
  `Send + Sync` static assertion; the generation counter became an
  `AtomicU32` with the unchanged generation policy.
- Store semantics preserved verbatim inside one write-locked per-shard
  critical section (solved overwrites, unsolved never downgrades, `work`
  monotone, k=2 replacement score).
- `probe` → `Option<TtEntry>` by value; `entries()` →
  `for_each_entry` (native bucket order, shard-by-shard read locks);
  `stats()`/`best_child_counts()` per-shard; `Search::tt_mut()` removed
  (`reconstruct` seeding + `campaign_worker` moved to `tt()`).
- New fast-tier concurrency tests in `tt/tests.rs`: shard/index invariant
  vs a reference bucket enumeration, a 4-thread store/probe stress over
  `Arc` (no torn entries, solved never downgraded, work monotone), and the
  layout partition test; all pre-existing `tt/tests.rs` tests kept their
  semantics (call shapes mechanically adapted).
- `cargo fmt`/`clippy`/`doc` clean; `make test` green **with the refactor
  in place** (including the byte-identity golden suites).

## Gate results (the kill evidence)

Deterministic drift — **zero** everywhere (details in
`measurements/plan4/README.md`):

- quick suite `child_evals`/`nodes`/outcomes: bit-identical per case
  (55/55);
- m22 + shuffle-win first-outcome stdout: byte-identical;
- snapshot dump bytes: identical (sha256 recorded);
- `make test` green; m22 default-mode golden unchanged.

Wall (the gate's cost surface) — **failed**, reproducibly:

| case (first-outcome) | pre median | post median | delta | threshold |
| --- | --- | --- | --- | --- |
| m22, 30 s cap, 10 interleaved pairs | 2.995 s | 3.175 s | **+6.0 %** | ≤ +2 % |
| shuffle-win, 100 s cap, 1 pair | 50.46 s | 52.67 s | **+4.4 %** | ≤ +2 % |

Per the plan's risk protocol, profiling was done before concluding
(`perf record -e cpu-clock`, pre vs post, plus `perf stat`):

- The `RwLock` RMW pair itself is only **0.7 %** of runtime — the naive
  "lock is a few ns" model is not where the money is.
- The cost is the lock discipline applied at **child-eval granularity**:
  `probe` is called per child per DF-PN round (~15–20 probes/node on the
  hard class, ~15 M on m22). Pre-refactor it inlined into
  `evaluate_child` as plain loads with full SROA; post-refactor it forces
  48 B `TtEntry` copies (the pre-registered by-value API),
  non-forwardable bucket loads across the opaque atomic critical
  section, and clobber-induced re-loads in the caller.
- Mitigation variants, each wall-neutral (±1 %):
  1. `#[inline]` on `probe` (LLVM then still chose not to inline);
  2. body shrinkage — unchecked shard/bucket indexing + `unwrap`: probe
     *did* inline into `evaluate_child`, **no speedup** (the cost is the
     discipline, not the call);
  3. `-Ctarget-cpu=native` (M1 has LSE; single-instruction atomics):
     pre/post both unchanged.
- A lock-free-reader redesign (per-shard seqlock, atomic entry fields)
  was scoped and **rejected**: sound Rust requires atomics on every entry
  field plus read-side acquire fences (still projects ≈ +2–3 %), and
  per-shard version bumps would cause reader retry storms under plan5's
  real multi-writer contention — i.e. worse for the very workload the
  refactor exists to enable.

Root cause of the gate failure: the plan's cost model ("uncontended
futex-based RwLock read is a few ns vs ~3 µs/node — well under noise")
under-counted probe density by ~20× — the tax is per child-eval, not per
node. This is a measured property of the mechanism on this solver's hot
path, not a defect in the implementation; the invariant tests found no
concurrency defect and the deterministic surface was never perturbed.

## Kill actions taken

- Working tree reverted to the pre-plan4 state (clean `git diff`;
  `patch -R` round-trip verified); `make test` re-run green; snapshot
  byte-identity re-verified post-revert.
- The complete attempt is preserved for the record / a possible
  conscious reopen: `measurements/plan4/sharded_tt_attempt.patch` +
  `measurements/plan4/shard_rs_attempt.rs` (the untracked `shard.rs` is
  outside `git diff`, hence the sibling file).
- `initiative.md`: backlog #4 row closed NO-GO, Status + History updated.
- `docs/plans/README.md`: `parallel` row updated (close event).

## Tools / examples used

- `benchmark --suite quick --json`, the solver CLI (m22, shuffle-win,
  shallow snapshot case), `perf record/report/stat` (per AGENTS.md
  recipe), a scratch-tree A/B harness (pre binary rebuilt from the
  identical working copy with the patch reverse-applied, `strings`-verified),
  interleaved wall medians to defeat session drift.

## Problems encountered

- The wall threshold fired on the first honest measurement; the plan's
  pre-registered expectation was wrong in *magnitude* (probe density),
  which is exactly what a cheap kill point is for.
- `for_each_entry` closures cannot use `?` (plain `FnMut`): the snapshot
  writer needed a first-error-capture pattern — mechanical, no contract
  impact.
- `RUSTFLAGS` builds silently overwrite `target/release` binaries; A/B
  binaries were hash-verified (`strings`) before every comparison.

## Unresolved parts / missing tests

- None in the reverted tree: the codebase is exactly pre-plan4 plus
  docs/measurements; no test is missing because no code remains.
- The concurrency-invariant test suite (stress, shard/index invariant,
  `Send`/`Sync` assertion) exists only inside the preserved patch; if the
  A-stage is ever reopened, it revives with the patch.

## Next steps (reopen triggers — owner's call)

1. **Re-weigh the tax consciously**: a ~5 % sequential wall tax on the
   hard class is the measured price of the shared-TT foundation; if the
   owner judges that acceptable against plan5's GO band (≥ 2.0×/4
   threads, work inflation ≤ 2×), re-apply
   `measurements/plan4/sharded_tt_attempt.patch`, re-register the wall
   threshold (the ≤ 2 % gate as written is unsatisfiable for this
   mechanism), and reopen backlog #4 with plan5 unchanged otherwise.
2. **Stay closed**: the sequential solver is the product; a guaranteed
   ~5 % tax on every hard solve for an unproven 2–8× prototype does not
   clear the project's priority order (correctness, then performance)
   without an explicit owner decision. The option-C no-go, the lean plan7
   deterministic no-go, and process-portfolio architectures unaffected by
   this gate (`parallel` option B) remain the unblocked alternatives.
3. In either case, plan5 should not be written against the current gate
   text; the probe-density finding (child-eval-granularity TT traffic)
   also affects any future TT-side optimization work (e.g. entry
   compression would amortize here too).

## Addendum — owner decision, later the same day (2026-10-03)

The owner chose reopen option 1 after reviewing the speedup estimate
(literature-anchored, 2–10 threads: SPDFPN 0.74 efficiency/16 threads,
Solrex ≈ 5.2×/4; expected ≈ 2.9–3.2× at 4 threads, conservative band
~1.8× at the GO-band edge; container limit = 4 real threads). The kill
verdict above stands unchanged as the measured record; what changes is
the decision that follows from it:

- The ~5 % TT-concurrency tax is **accepted as the entry fee** for the
  A-stage — the only remaining multiplicative lever (all others are
  measured no-gos).
- **Condition 1 (revert-if-missed):** the acceptance is conditional. If
  plan5 misses its pre-registered GO bands (≥ 2.0×/4 threads, work
  inflation ≤ 2×, zero soundness violations), the refactor is reverted
  again and the tax goes with it.
- **Condition 2 (explicit gate):** the plan4 gate (≤ +2 %) is replaced
  by a hard budget of **≤ +7 % median hard-class wall** (m22 / shuffle-
  win first-outcome, vs the unsharded baseline), byte-identity surface
  unchanged (quick-suite `child_evals`, stdout, snapshot bytes).
- **Condition 3 (measurement honesty):** wall measurements at 2–4
  threads are real (4-CPU container); N ≥ 8 are simulated only.

Execution: backlog #4 reopened (see `initiative.md`, Status + History +
backlog row); next session writes **plan4b** (re-land the preserved
refactor, re-run the drift protocol, pin the tax against the ≤ 7 %
budget) and **plan5** (SPDFPN prototype with GO bands and the
revert-if-missed clause).
