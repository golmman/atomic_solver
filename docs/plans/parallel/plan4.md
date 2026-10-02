# Parallel Plan 4 — Inert TT-concurrency refactor (A-stage prerequisite)

Executes `parallel` backlog **#4, stage 1** (the SPDFPN staging skeleton,
`research_spdfpn.md` §staging item 1). The owner confirmed the A-stage GO on
2026-10-03; plan3's mining (`research_spdfpn.md`) is the reference design.

This is the **inert prerequisite** for the SPDFPN prototype (plan5): make
`TranspositionTable` safe for concurrent readers/writers — sharded behind
interior-mut locks — with **byte-identical sequential behavior**. No threads
are spawned by the solver, no `--threads` option appears, no search semantics
move. The refactor is justified *only* by the A-stage: if the drift gate
cannot be satisfied at reasonable cost, the A-stage dies here — that is the
cheap kill point.

Prerequisite reading: `parallel/initiative.md` (constraints 1–4, backlog #4),
`parallel/research_spdfpn.md` §2 item 4 (shared-TT discipline) and §staging
(kill point), `parallel/design_space.md` §1 row A (the gate item this plan
executes), `../dfpn/research_ghi_journal.md` (why cross-context sharing is
load-bearing — untouched here, cited by plan5), `src/tt_snapshot/mod.rs`
header (snapshot byte-identity contract this plan must preserve).

## Goal

`TranspositionTable` (`src/search/tt/table.rs`) today is a single
`&mut self`-owned `Vec<[TtEntry; 2]>` plus a plain `u32` generation counter.
Plan5 needs N DF-PN workers over *one* table, so the table must become
internally concurrent while the N=1 path stays observably identical:

1. **Shard the bucket array** into contiguous power-of-two chunks, each
   behind a `std::sync::RwLock` (no new dependency).
2. **Interior mutability**: `probe`/`store`/`clear`/`new_generation` take
   `&self`; the struct becomes `Send + Sync` by construction.
3. **One bucket's read-modify-write stays inside one write-locked critical
   section** — `store()`'s existing semantics (solved overwrites, unsolved
   never downgrades a solved entry, `work` monotone, k=2 replacement score)
   are preserved verbatim, which is exactly the SPDFPN §2 shared-TT
   discipline at the per-shard level.
4. The **generation counter** becomes an `AtomicU32` (plain-u32 reads across
   shard locks would be a data race); the generation *policy* is unchanged
   — `new_generation()` bumps the single shared counter exactly as today.

### Design decisions (pre-registered)

- **Shard selection is derived from the bucket index, not from key bits.**
  `index()` (`key & mask`) is untouched; a key's shard is the shard that
  owns its bucket. Shard `s` owns the contiguous bucket range
  `[s · buckets_per_shard, (s+1) · buckets_per_shard)`. This keeps probe
  (lock bucket's shard → scan bucket) correct without double mapping, and
  preserves the *native bucket order* that `entries()` and the snapshot
  dump contract depend on.
- **Shard count**: `shards = min(bucket_count, MAX_SHARDS = 256)` rounded
  down to a power of two (the `with_mb` minimum of 16 buckets and the
  test-only `with_capacity` both yield ≥ 1 bucket per shard). 256 shards ≈
  256 extra lock words + Vec headers — noise against the "RAM = TT only"
  contract (128 MB default TT → ~4 k buckets per shard).
- **`probe` returns `Option<TtEntry>` by value** (read-lock, copy, unlock).
  `TtEntry` is `Copy` and ~48 B; every current call site already
  `.copied()`s (`dfpn/core.rs`, `dfpn/children.rs`) or reads fields off the
  copy (`dfpn/pv.rs`, `reconstruct/tests.rs`), so this is a mechanical
  adjustment, not an API-semantics change.
- **`entries()` → `for_each_entry(&self, f: impl FnMut(&TtEntry))`**: locks
  shards one at a time *in index order*, invoking `f` in native bucket
  order. Consumers: `tt_snapshot/mod.rs` (dump writer — order is the
  byte-identity contract), `reconstruct/walker.rs` (hole-filling seed),
  `reconstruct/tests.rs`. A `Vec`-collecting helper is *not* added (the
  dump is the hot `entries()` user; a copy of an entire XL table would be
  an unforced memory regression).
- **`stats()` / `best_child_counts()`**: shard-by-shard under read locks
  (both are order-independent aggregates; `best_child_counts` keeps its
  sort).
- **`Search::tt_mut()` is removed.** With `store` on `&self`, its only
  consumer (`reconstruct/mod.rs`, snapshot seeding) uses `tt()`. The
  remaining accessor `tt()` (shared ref) covers snapshot tooling.
- **No behavior change anywhere else**: no movegen, no dfpn logic, no
  repetition cache, no ProofEvent, no snapshot format, no CLI surface.

## Implementation steps (drift-gated, bisectable)

1. **Baselines (before any change, release build)** — stored under
   `measurements/plan4/`:
   - `benchmark --suite quick --json --first-outcome --runs 1`
     (per-case `child_evals`, `nodes`, `outcome`);
   - m22 first-outcome stdout capture:
     `--fen '4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22'
     --timeout 30 --first-outcome --outcome-only`;
   - snapshot dump bytes on a shallow quick case with `--tt-dump-path`;
   - `tests/fixtures/m22_default_stdout_golden.txt` is the default-mode
     reference (unchanged file; exercised via `test_trajectory_golden`).
2. **Sharding + locks in `src/search/tt/table.rs`** (one drift check).
   If the file would cross ~10 KB, split the shard plumbing into
   `src/search/tt/shard.rs` (mod docs state the shard/index invariant
   pairing); tests stay in `tt/tests.rs`.
3. **Call-site mechanical updates** (same drift check as step 2 — they are
   forced by step 2's signatures, so they land together):
   `dfpn/core.rs`, `dfpn/children.rs`, `dfpn/pv.rs` (drop `.copied()`),
   `reconstruct/mod.rs` (`tt()` instead of `tt_mut()`), `tt_snapshot/mod.rs`
   + `reconstruct/walker.rs` + `reconstruct/tests.rs` (`for_each_entry`).
   Remove `Search::tt_mut()` in the same step (unused afterward).
4. **Concurrency invariant tests** (new, fast tier, in `tt/tests.rs`):
   - shard/index invariant: `index()` values and `for_each_entry` order
     match a reference two-slot-bucket enumeration (pins native order);
   - multi-thread stress: 4 threads, disjoint key ranges, concurrent
     `store` + `probe` over a shared `Arc<TranspositionTable>`; assert no
     panic, no torn entries (probe never returns a mix of old/new fields —
     enforced by the write-lock critical section), and the solver
     invariants hold under races (solved never downgraded, `work`
     monotone);
   - `Send`/`Sync` static assertion;
   - all existing `tt/tests.rs` unit tests (eviction, generation,
     `with_capacity` determinism) pass **unchanged** — they are themselves
     part of the inert gate.
5. `cargo fmt`, `cargo clippy`, `cargo doc` clean; `make test` green.

## Non-goals

- No `--threads` flag, no worker threads, no SPDFPN machinery (W-threshold,
  virtual TT, `TRYRUNJOB`, job lock — plan5).
- No repetition-cache or GHI contract change (the journal contract is cited,
  not edited; thread-locality of the repetition cache is plan5 work).
- No snapshot format change; the dump stays byte-identical by the
  `for_each_entry` ordering.
- No sharding *count* tuning for parallel performance (256 is a sequential
  -cost-neutral default; plan5 may revisit under real contention).
- Concurrent `clear()`/`new_generation()` *while workers run* is explicitly
  out of scope: in the sequential solver both are only called between
  search phases by the single owner thread; plan5 owns the policy (and the
  documentation) for calling them under concurrency.

## Risks

- **Hot-path lock cost** (probe is called per node, several times): an
  uncontended futex-based `RwLock` read is a few ns vs ~3 µs/node on m22 —
  expected well under noise. **Stop-and-bisect threshold: median m22
  first-outcome wall +>2% pre/post.** If it fires, profile first (per
  AGENTS.md `perf` recipe) before redesigning; do not trade the concurrency
  correctness for speed silently.
- **`for_each_entry` borrow shapes**: `tt_snapshot` builds records while
  iterating; per-shard locking means `f` must not itself call back into the
  table (document: no re-entrant calls inside `f`). The two consumers are
  simple record builders — no expected friction.
- **`with_capacity` test contract**: eviction/generation tests rely on
  exact bucket counts; the shard split must keep bucket_count semantics
  (`bucket_count()` stays the total across shards).

## Validation (drift protocol is the gate)

1. **After step 2 (sharding) and step 3 (call sites), and at the end**:
   - quick suite `child_evals` **bit-identical per case** (vs baseline);
   - m22 first-outcome stdout **byte-identical**;
   - snapshot dump **byte-identical** on the shallow baseline case;
   - `cargo test --release --test test_trajectory_golden -- --include-ignored`
     byte-identical golden;
   - `cargo test --release --test test_move_order` green (fast tier);
   - `cargo test --release --test test_decisive_remaining -- --include-ignored`
     green unchanged (budget determinism surface);
   - `make test` green within the ~60 s gate.
2. **Wall check**: m22 first-outcome, 5 runs median, pre vs post; expect
   within noise (≤ 2%); one shuffle-win first-outcome run each side as the
   second point. Any regression beyond the threshold = stop-and-bisect.
3. **Concurrency tests green** (new unit tests) + existing `tt/tests.rs`
   unchanged and green.
4. `git diff` scope check: `src/search/tt/` (+ optional `tt/shard.rs`),
   the five call-site files, `src/search/dfpn/mod.rs` (`tt_mut` removal),
   `tests/`, `docs/plans/parallel/`. No CLI, no main.rs, no snapshot-format
   changes expected.

## Deliverables

- Code: the sharded, interior-mut `TranspositionTable` + call-site updates
  (steps 2–3) and the concurrency invariant tests (step 4).
- `measurements/plan4/README.md` — provenance table (commands, baselines,
  post-change results), per the measurement conventions in AGENTS.md;
  raw transcripts are `.gitignore`d, only parsed results and the README
  are committed.
- `parallel/initiative.md` — backlog #4 row: stage 1 done (or the kill
  verdict if the gate fails) + history bullet.
- `docs/plans/README.md` `parallel` row — updated only on a kill (pivot/
  close event); a clean pass needs no row edit.
- `report4.md` in this directory (final task): gate results, wall medians,
  problems, next steps (plan5 kickoff inputs).

## Kill point

If the drift gate fails after a bounded bisect effort (steps revert
individually) or the wall regression cannot be brought under the threshold
without giving up the concurrency discipline, the A-stage **dies here**:
record the measured reason in `report4.md`, close backlog #4 as a no-go,
update the initiative status and the README row. The sequential solver is
unchanged either way — that is the point of doing this refactor inertly.
