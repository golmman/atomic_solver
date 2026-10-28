# Performance Plan 4: Repetition-path bookkeeping — incremental context hash + Bloom filter

Initiative: `performance`, backlog **item 3** (incremental repetition-context
hash) **combined with item 4** (path Bloom negative filter). Evidence:
`docs/performance/performance_report1.md` §7.1/§7.2; the plan-session probe
recorded in [`measurements/plan4/plan_session/`](measurements/plan4/plan_session/).
Successor of [`plan3.md`](plan3.md) / [`report3.md`](report3.md) (TT pre-probe
before make/unmake). Drafted 2026-10-28; the plan session wrote no product code
(all probe builds live in `/tmp`).

*Ownership note.* Backlog items 3/4 are marked "may move to `lean`". This plan
executes them here because the report's evidence, the corpus drivers, and the
identity protocol of this initiative are already in this directory, and the
change is a bit-identical restructuring of path bookkeeping rather than a
timing-only micro-optimization. The plan is self-contained either way; if the
owner prefers `lean`, this file and `measurements/plan4/` move with it.

## Why this plan exists

`Search::path_stack` is the current search path (repetition keys, root to
parent). It is read in three hot places:

1. `core.rs`'s repetition-cache probe folds the whole path into an
   order-independent context hash **once per expanded node**
   (`RepetitionCache::context_hash(&self.path_stack)`).
2. `core.rs`'s local-repetition check and `best_move_repeats_path` scan the
   path for a key **once per expanded node / TT solved entry**.
3. `children.rs`'s pre-probe (plan3) calls `path_contains(child_rep_key)` for
   **every child evaluation** — 3.9M–66.6M scans per corpus cell.

Report §7.1 measured the incremental context hash at ~2–3% on `m22`/`m20` and
~1% on `dec10`; §7.2 measured a 256-slot path Bloom filter at ~1–3% and
recorded that the two together are the promising pair but were not clean enough
to claim gate grade. Report3 recommended item 3 as the next plan and kept item
4 behind it because a Bloom filter needs the same "mutate `path_stack` only
through push/pop" discipline.

### Plan-session probe (read-only; three `/tmp` copies, nothing landed)

Three throwaway builds off `git archive HEAD` (`574f64d`):
`/tmp/plan4_proto` (incremental context hash only), `/tmp/plan4_bloom` (Bloom
filter only), `/tmp/plan4_combo` (both). The driver is
[`measurements/plan4/plan_session/ab_path_bookkeeping.py`](measurements/plan4/plan_session/ab_path_bookkeeping.py);
the parsed results are the three JSON files next to it.

Host: aarch64 container, 4 online CPUs, release profile. Absolute walls are
**not** comparable with `report3.md`'s host state; the contract is the ratio.
Every cell was `child_evals`- and stdout-identical across all arms.

| cell | reps | item 3 alone wall / CPU | item 4 alone wall / CPU | **combined wall / CPU** |
|---|---:|---:|---:|---:|
| `dec10` (3.90M evals) | 31 | 0.988 / 0.988 | 0.979 / 0.979 | **0.954 / 0.954** |
| `m22_b` (10.0M budget) | 31 | 1.006 / 1.006 | 0.997 / 0.997 | **0.992 / 0.992** |
| `stress_b` (10.0M budget) | 31 | 1.001 / 1.000 | 0.999 / 0.999 | **0.988 / 0.988** |
| `m20_b` (30.0M budget) | 15 | — / 1.002 | — / 1.001 | **— / 0.989** |
| `dec13` (5.03M evals) | 15 | — | — | **0.994** (wall only) |
| `rem12` (66.6M evals) | 11 | — | — | **0.994** (wall only) |

The new evidence is decisive for the plan's scope: **item 3 alone is not a
broad win on the current tree** (it helps `dec10` ~1%, but regresses `m22_b`
~0.6% and is neutral on `stress_b`/`m20_b`), while **the combination is
gate-grade on every deep cell** (`dec10` 4.6%, `m20_b` 1.1%, `stress_b` 1.2%,
`m22_b` 0.8%, `dec13`/`rem12` 0.6%). The combined effect is superadditive in
this probe; the plan therefore executes items 3 and 4 as one bit-identical
bookkeeping change, with one shared push/pop invariant and one gate. The report
must record the per-arm probe numbers and any attribution it can support, but
the verdict is on the combined change.

## Question and pre-registered decision rule

**Q:** Can the repetition path be maintained incrementally — an O(1)
order-independent ancestor-context hash plus a 256-slot Bloom negative filter
for `path_contains` — with **bit-identical trajectories**, for ≥ 4% median wall
on `dec10`, ≥ 0.5% on `m20_b`/`m22_b`/`stress_b`, and ≥ 0.2% on
`dec13`/`rem12`?

**ADOPT (GO)** only if all hold:

1. **Identity.** Every corpus cell is bit-identical to the Phase-0 baseline in
   `child_evals`, outcome, PV length, `pv_status` and captured stdout;
   `test_trajectory_golden` (`m22` default mode) passes **without
   regoldening**; the portfolio `dec10` anchor is exact at 3,901,847 evals; the
   quick suite is **59/59 eval-identical** (42,109,768 total child evals,
   0 wrong, 0 timeouts); the new unit tests pass.
2. **Mechanism.** A test-only Bloom-negative counter is `> 0` on a fixture with
   an absent key whose slot is zero; a unit test proves
   `path_context_hash == RepetitionCache::context_hash(&path_stack)` after every
   push/pop of a deterministic sequence and after `prefix_path` seeding; a
   unit test proves every key on the path is still found by `path_contains`
   (no false negative).
3. **Throughput (interleaved A/B, counterbalanced, ≥ 21 reps/case except
   `rem12` ≥ 11).** Median wall ratio ≤ **0.97** on `dec10`; ≤ **0.995** on
   each of `m20_b`, `m22_b`, `stress_b`; ≤ **0.998** on `dec13`, `rem12`. The
   process CPU-time ratio must not contradict the wall verdict (it must be
   ≤ the same bar + 0.003 on each cell); both are recorded.
4. **No regression.** No deep/budget cell has a median wall ratio > **1.005**;
   the two short cells (`mate1`, `easy_kq`) are ~0.5–2 ms and noise-dominated,
   so their bar is a loose ≤ **1.05** guard that only catches a structural
   mistake (e.g. an allocation per node).
5. **No semantic surface change.** No CLI knob, no new public API, no change to
   TT semantics/sizing/eviction/salt, no ordering or `ChildInfo` field change,
   no `ProofEvent` change, no preflight change. Only private `Search` state,
   private methods, and the `best_move_repeats_path` read path change.
6. **Gates.** `make test` and `make test-full` green; `cargo fmt --check`,
   `cargo clippy --all-targets`, `cargo doc` clean; new units fast-tier only.

**NO-GO / falsification handling.**

- **Any** `child_evals` difference on a fixed-budget fixture means the
  bookkeeping is not behaviourally equivalent — STOP and root-cause before
  touching anything else. Do not re-baseline a golden, an anchor or a cap.
  The likely causes, in order: (a) a Bloom false negative from a `path_stack`
  mutation that bypassed `path_push`/`path_pop` (including the `prefix_path`
  seed); (b) a counter-underflow in `path_pop` (pop without a matching push,
  or a slot decremented after the key was already removed); (c) the context
  hash field not reset/re-seeded in `reset_search_state`.
- **Bar 3 or 4 fails** → NO-GO; revert **both** changes and record the measured
  arms. There is no partial mode and no fallback knob: the two changes share
  the same push/pop invariant and the plan's evidence is for the combination.
  If the combined change fails but the probe's single-arm picture suggests a
  class-scoped win, the report records it as a finding for the owner; do not
  silently ship one arm.
- **Bar 2 fails** (the Bloom counter never fires or the hash equality test
  fails) → the measured gain came from something else; STOP and re-derive the
  mechanism before proceeding.

## Scope

**In scope**

- `Search::path_context_hash: u64` and `Search::path_bloom: [u16; 256]` (private
  fields), their initialization and reset/prefix seeding.
- `Search::path_push` / `Search::path_pop` updates, `Search::path_contains`
  Bloom-negative shortcut, and `best_move_repeats_path` routed through
  `path_contains`.
- `core.rs`'s repetition-cache hot site reading `self.path_context_hash`
  instead of folding `path_stack`; removal of the now-unused
  `RepetitionCache` import there.
- Test updates: every direct `path_stack.push`/`clear` in the dfpn tests goes
  through `path_push`/`reset_search_state`.
- Unit tests for hash equivalence, Bloom soundness and the negative counter.
- Docs: `mod.rs` field/method docs, `core.rs` cache-site comment,
  `repetition_cache.rs` module doc, the `AGENTS.md` `src/search/dfpn/` bullet.
- Measurement record under `docs/plans/performance/measurements/plan4/`.

**Out of scope (non-goals)**

- Backlog item 6 (faster repetition-cache hasher, <1%) — after this plan.
- Removing the terminal existence query for children with no TT entry (report3:
  8.5% + 7.3% on `dec10`); it needs a different invariant.
- Any change to `Search::sort_moves`, the `MoveScorer`, heuristic constants,
  the restart schedule, epsilon, refinement caps, the repetition cache payload,
  the TT representation, the preflight/cheap-probe paths, or
  `atomic-movegen`.
- A second Bloom hash slot, a larger filter, or adaptive sizing. The 256-slot
  single-slot design is the measured one; nothing in this plan tunes it.

## Normative design

### D-a. Fields and constants (`src/search/dfpn/mod.rs`)

```rust
/// Path-presence Bloom filter slots (512 B at `u16` per slot). A zero slot is
/// a sound negative for `path_contains`; a nonzero slot falls back to the
/// linear scan, so a false positive costs a scan, never a wrong result.
const PATH_BLOOM_SLOTS: usize = 256;

pub struct Search {
    // ...
    path_stack: Vec<u64>,
    /// Order-independent context hash of `path_stack`
    /// (`RepetitionCache::context_hash`), maintained incrementally by
    /// `path_push`/`path_pop` so the repetition-cache key costs O(1) per node.
    path_context_hash: u64,
    /// Path-presence filter indexed by the low 8 bits of the repetition key.
    /// Counts, not bits, so duplicate keys (which the no-duplicate path
    /// invariant does not currently produce) stay correct.
    path_bloom: [u16; PATH_BLOOM_SLOTS],
    /// Test-only census counter: `path_contains` calls decided by a zero Bloom
    /// slot without the linear scan. Reset per run in `begin_run`.
    #[cfg(test)]
    path_bloom_negatives: std::cell::Cell<u64>,
    // ...
}
```

`Search::new` initializes `path_context_hash: 0`, `path_bloom: [0; PATH_BLOOM_SLOTS]`,
and (test builds) `path_bloom_negatives: Cell::new(0)`.

### D-b. Reset, prefix seeding, and the slot function

`reset_search_state` is the **only** path reset. It must clear both new pieces
and re-seed them from `prefix_path` with the same fold the hot site used to do:

```rust
fn reset_search_state(&mut self) {
    self.path_stack.clear();
    self.path_bloom.fill(0);
    self.path_context_hash = 0;
    self.move_stack.clear();
    self.max_depth_reached = 0;
    if let Some(keys) = &self.prefix_path {
        self.path_stack = keys.clone();
        for &key in &self.path_stack {
            self.path_bloom[Self::path_bloom_slot(key)] += 1;
        }
        self.path_context_hash = RepetitionCache::context_hash(&self.path_stack);
    }
}

fn path_bloom_slot(key: u64) -> usize {
    (key as usize) & (PATH_BLOOM_SLOTS - 1)
}
```

The prefix fold is once per work chunk, not per node; the steady-state cost is
the O(1) push/pop update.

### D-c. The path methods and the hot-site read

```rust
pub(super) fn path_contains(&self, key: u64) -> bool {
    if self.path_bloom[Self::path_bloom_slot(key)] == 0 {
        #[cfg(test)]
        self.path_bloom_negatives.set(self.path_bloom_negatives.get() + 1);
        return false;
    }
    self.path_stack.contains(&key)
}

pub(super) fn path_push(&mut self, key: u64) {
    let slot = Self::path_bloom_slot(key);
    debug_assert!(self.path_bloom[slot] < u16::MAX, "path deeper than u16");
    self.path_bloom[slot] += 1;
    self.path_context_hash = self
        .path_context_hash
        .wrapping_add(repetition_cache::mix_key(key));
    self.path_stack.push(key);
    self.max_depth_reached = self.max_depth_reached.max(self.path_stack.len() as u32);
}

pub(super) fn path_pop(&mut self) {
    if let Some(key) = self.path_stack.pop() {
        let slot = Self::path_bloom_slot(key);
        debug_assert!(self.path_bloom[slot] > 0, "Bloom counter underflow");
        self.path_bloom[slot] -= 1;
        self.path_context_hash = self
            .path_context_hash
            .wrapping_sub(repetition_cache::mix_key(key));
    }
}
```

`repetition_cache::mix_key` is the same `SplitMix64` finalizer the context hash
already uses; `wrapping_add`/`wrapping_sub` are exact inverses, so the field
equals `RepetitionCache::context_hash(&path_stack)` at every point this
invariant holds. `RepetitionCache::context_hash` stays public(super) for the
reset seed and the tests; it is no longer called on the hot site.

`best_move_repeats_path` (`core.rs`) changes its final read from
`self.path_stack.contains(&rep)` to `self.path_contains(rep)`. Its play/undo
semantics are unchanged; only the negative-filter lookup is added.

`core.rs`'s repetition-cache probe site becomes:

```rust
let context_hash = self.path_context_hash;
```

with the comment updated to say the field is maintained by
`path_push`/`path_pop` and equals the fold of the strict-ancestor path. The
store site already reuses the local `context_hash` captured before this frame's
`path_push`; keep that structure — the field at the store site (after the
children have pushed/popped symmetrically) is the same value, but the local is
the clearer contract and avoids relying on descendant symmetry in the hot path.

### D-d. The path-stack mutation invariant

`path_stack` is part of a two-field invariant now: every mutation must go
through `path_push`/`path_pop`, so the Bloom counts and the context hash stay
in sync. The plan's obligations:

- state the invariant in the field doc and in `path_push`/`path_pop` docs;
- update every direct `path_stack.push`/`clear` in the dfpn unit tests to
  `path_push`/`reset_search_state` (the `children.rs`, `history.rs` and
  `tests.rs` fixtures);
- add a `#[cfg(test)]` helper
  `assert_path_bookkeeping_consistent(&self)` that checks
  `path_context_hash == RepetitionCache::context_hash(&path_stack)`, every
  path key maps to a nonzero Bloom slot, and every zero-slot key is absent;
  call it after each push/pop in the new tests (not in the hot path);
- keep the `debug_assert!`s in `path_push`/`path_pop` for counter bounds.

A future code path that mutates `path_stack` directly would make the Bloom
filter produce a **false negative** (a repetition missed) and/or the context
hash stale. That is a `child_evals` drift, caught by bar 1; the invariant text
and tests are the first line of defence.

### D-e. Proof events and search semantics are untouched

`ProofEvent::NodeProven` emission, the TT, ordering, `ChildInfo`, the
repetition cache's payload and capacity, and every outcome/PV path are
unchanged. The only semantic-adjacent change is `best_move_repeats_path` using
`path_contains`, which is equal to `path_stack.contains` by the Bloom
soundness contract (zero ⇒ absent). Draws still emit nothing; decisive events
still carry the same path/hash/outcome/depth. No CLI or public API changes.

### D-f. What is deliberately untouched

No new allocation on the hot path (`path_bloom` is a fixed array; `path_stack`
and `path_push`/`path_pop` already existed). No `unsafe`. No change to the
per-depth pools, the pre-probe ladder, or the prefetch pre-pass. The Bloom slot
function intentionally uses only the low 8 bits of the key: distribution is a
performance property, not a correctness one, because a collision only causes a
fallback scan.

## Tasks

1. **Phase 0 — baseline corpus (read-only).** Build the pristine `HEAD` release
   binary into `/tmp/plan4_base` (`git archive HEAD`), record the single-arm
   corpus (5 runs/case, `--tt-size 256`, per-case args) in
   `measurements/plan4/state/baseline.json`, and record `env.json`. Copy
   plan3's `driver.py` into `measurements/plan4/` and add the `m20_b` cell
   (`--budget 30000000`); keep `sys.dont_write_bytecode`.
2. **`src/search/dfpn/repetition_cache.rs`** — no logic change; update the
   module doc to say the context hash is maintained incrementally on the path
   stack and that `context_hash` remains the reset/test reference. Ensure
   `mix_key` is reachable from `mod.rs` (it is `pub(super)`).
3. **`src/search/dfpn/mod.rs`** — add `PATH_BLOOM_SLOTS`, the two fields (plus
   the test-only `Cell` counter), initialization, `path_bloom_slot`,
   `reset_search_state` seeding, `path_contains`, `path_push`, `path_pop`,
   and the field/method docs (D-a–D-d). Add the test helper
   `assert_path_bookkeeping_consistent`. Reset the test-only
   `path_bloom_negatives` counter in `Search::begin_run` (next to
   `preprobe_skips`).
4. **`src/search/dfpn/core.rs`** — replace the hot-site fold with
   `self.path_context_hash`; update the cache-site comment; route
   `best_move_repeats_path` through `path_contains`; remove the now-unused
   `RepetitionCache` import if nothing else needs it.
5. **Tests** (`mod.rs` tests or `dfpn/tests.rs`; all fast tier, no
   `#[ignore]`):
   - `incremental_context_hash_matches_fold` — deterministic push/pop sequence
     (e.g. a small LCG), comparing the field to the fold after every operation;
   - `context_hash_replays_prefix_path` — set `prefix_path`, call
     `reset_search_state`, assert hash and Bloom contents;
   - `path_bloom_has_no_false_negatives` — every pushed key is found; after
     pop it is absent; a key whose slot is zero is absent and increments the
     test-only negative counter;
   - `path_bloom_insert_remove_returns_to_zero` — push/pop a set of keys,
     assert all slots zero and hash zero.
   Update the pre-existing tests in `children.rs`, `history.rs` and `tests.rs`
   that push/clear `path_stack` directly; the existing
   `evaluate_child_repetition_is_draw`, rule50, killer and best-move-guard
   fixtures are the regression net.
6. **Docs** — `mod.rs` field/method docs, `core.rs` cache-site comment,
   `repetition_cache.rs` module doc, `children.rs` pre-probe note if a
   cross-reference helps, `AGENTS.md`'s `src/search/dfpn/` bullet (the
   push/pop-only invariant and the Bloom/context bookkeeping).
7. **Measure and gate** (Validation below). Use
   `measurements/plan4/plan_session/ab_path_bookkeeping.py` (it already records
   wall + CPU and aborts on identity failures) as the authoritative A/B;
   single-arm runs are not accepted for bars 3/4. Commit the parsed
   `state/*.json` and drivers only (no raw transcripts).
8. **Write `report4.md`**, update `initiative.md` (items #3 and #4 rows, the
   item↔plan mapping) and the `docs/plans/README.md` `performance` row.

## Corpus

Measured with `--tt-size 256 --first-outcome --outcome-only` plus the per-case
argument. Probe ratios are the plan-session calibration, not a prediction to
match.

| id | FEN | args | class | probe combined wall / CPU |
|---|---|---|---|---|
| `mate1` | `3Qk3/8/8/8/8/8/8/4K3 w - - 0 1` | `--timeout 2` | 2-eval short floor | not measured (guard only) |
| `easy_kq` | `4k3/8/8/8/8/8/8/4KQ2 w - - 0 1` | `--timeout 2` | probe-solved easy win (378) | not measured (guard only) |
| `dec13` | `r1bq1k1r/ppN4p/n1p1p3/3p1n1P/1b1P2P1/2P5/PP6/RNBQKB1R w KQ - 1 14` | `--timeout 10` | mid completing win (5.03 M) | 0.994 |
| `dec10` | `3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41` | `--timeout 10` | transposition-dense completing win (3.90 M) | **0.954 / 0.954** |
| `m20_b` | `4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R5RK w - - 4 20` | `--budget 30000000` | deepest censored cell (30.0 M) | — / **0.989** |
| `m22_b` | `4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22` | `--budget 10000000` | censored deep cell (10.0 M) | **0.992 / 0.992** |
| `stress_b` | `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21` | `--budget 10000000` | censored, capacity-saturated (10.0 M) | **0.988 / 0.988** |
| `rem12` | `rnbqkbnr/8/6pp/pppppp1B/3PPP2/N5PN/PPP4P/R1BQ1RK1 w kq - 0 10` | `--timeout 60` | long completing win (66.6 M) | 0.994 |

`rem12` needs its own wall cap (its baseline run is ~10 s on the current host
⇒ ≥ 11 reps is ~3.7 min per arm); `m20_b` is ~3.9 s CPU per run, so use the
driver's 21-rep default. `mate1`/`easy_kq` are the latency-floor sanity cells.

## Validation

1. `make test` (fast tier) — all new units and the updated path fixtures.
2. `make test-full` — required: `test_trajectory_golden` is the primary
   trajectory-identity evidence (bar 1), plus the 60 s regression/stress
   suites.
3. Portfolio `dec10` anchor: `cargo test --release --test portfolio_smoke` →
   3,901,847 evals, identical outcome/PV.
4. Quick suite: `benchmark --suite quick --first-outcome --timeout 3 --json` →
   59/59 eval-identical vs `measurements/plan2/state/quick_after.json`
   (42,109,768 total child evals, 0 wrong, 0 timeouts).
5. Corpus A/B per bars 3/4, counterbalanced, with per-run `child_evals`/stdout
   equality asserted inside the driver (any mismatch is a hard error, not a
   data row).
6. `cargo fmt --check`, `cargo clippy --all-targets`, `cargo doc` (no new
   warnings). No miri run needed: no `unsafe`, no allocator work.
7. Optional (recommended if cheap): `perf record -e cpu-clock -g` on `dec10`
   and `m20_b` before/after to confirm the path-scan/fold work drops; the
   mechanism is already backed by the Bloom counter and the hash equality
   tests.

## Report requirements

`docs/plans/performance/report4.md` must contain:

- the Phase-0 baseline and after corpus tables (median wall, `child_evals`,
  peak RSS, minor faults) and the counterbalanced A/B wall **and CPU-time**
  ratios per cell, with paired quartiles;
- the mechanism evidence: Bloom-negative counter values, the hash-equality
  test, and the per-arm probe numbers (item 3 alone / item 4 alone / combined)
  with an honest note on what the probe can and cannot attribute;
- the identity evidence (golden without regoldening, `dec10` anchor, quick
  suite total, every corpus cell's `child_evals`/stdout equality);
- the equivalence argument as implemented (D-a–D-e), including any place where
  the implementation differs from this plan;
- the ADOPT/NO-GO verdict against the six bars, with the exact `max` ratios;
- findings/follow-ups: whether the combined effect is class-shaped, whether
  the Bloom filter's negative rate predicts the gain, whether item 6 is still
  worth a plan, and the state of the terminal-existence-query follow-up.

## Follow-ups (not this plan)

- **Backlog item 6** (faster repetition-cache hasher): <1%, low priority; only
  after this plan, and only if the hasher shows up in a fresh profile.
- **Terminal existence query** (`has_legal_move_with_state`): report3 measured
  it at 8.5% + 7.3% of `dec10` CPU. It remains the largest single
  `evaluate_child` cost after plan3 and needs its own invariant.
- **Item 7 (unstable move sort)** stays not recommended: search-visible tie
  order requires a full drift gate for ~1%.
- **Item 8 (huge pages)** stays deferred: plan2's probe measured no effect on
  this container, and the lazy TT makes THP more likely to hurt.
