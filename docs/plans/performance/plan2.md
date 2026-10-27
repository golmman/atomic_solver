# Performance Plan 2: zero-init (lazy) TT allocation

Initiative: `performance`, backlog **item 5** (report §10, "Lazy/zero-init TT
allocation"; measured in §4 and §6.1 of
[`docs/performance/performance_report1.md`](../../performance/performance_report1.md)).
Successor of [`plan1.md`](plan1.md) / [`report1.md`](report1.md), which removed
the *search* from the easy-3-man latency path and left the TT allocation as the
new floor.

## Why this plan exists

`TranspositionTable::with_mb` builds `vec![[TtEntry::default(); 4]; buckets]`
with a **non-zero** default (`valid: false` but `pn = dn = 1`,
`best_child = u8::MAX`), so the whole 224 MiB of the default `--tt-size 256`
label is written at construction. Every process pays that, even when the
search touches a few dozen entries.

`report1.md` closed with exactly this follow-up: *"The easy-win floor is now
the 256 MB TT allocation (~9 ms) plus process start, not the search."* On the
current container that floor is far larger than the report's aarch64 reading:

| case (`--tt-size 256`, release) | wall (median) | peak RSS | source |
|---|---:|---:|---|
| mate-in-1 `3Qk3/8/8/8/8/8/8/4K3 w` (2 evals) | 103–106 ms | 230 MB | plan-session probe |
| easy KQ `4k3/8/8/8/8/8/8/4KQ2 w` (418 evals) | 106 ms | 230 MB | plan-session probe |
| dec10 (`--timeout 10`, completes 4,901,858 evals) | 760–767 ms | 227 MB | plan-session probe |
| m22_white (`--timeout 10`, completes 18,779,890 evals) | 2,650 ms | 227 MB | plan-session probe |

`--tt-size` sweep on the mate-in-1 case (5 runs each): 1 MB → 3.8 ms / 7.5 MB,
16 MB → 9.1 ms / 20.7 MB, 64 MB → 27.7 ms / 63.4 MB, **256 MB → 96.6 ms /
235 MB**, 512 MB → 187 ms / 465 MB. The allocation *is* the latency.

### Plan-session probe (read-only, `git archive HEAD` copy in /tmp, 7–9 interleaved reps)

`alloc_zeroed` replaces the `vec![default; n]` construction (no other change;
a throwaway build, nothing landed):

| case | baseline | lazy | ratio | RSS |
|---|---:|---:|---:|---|
| mate-in-1 | 103.2 ms | **2.1 ms** | 0.02× | 230 → 8 MB |
| easy KQ | 106.1 ms | **2.8 ms** | 0.03× | 230 → 8 MB |
| dec10 (complete) | 760.5 ms | 800.8 ms | 1.053× | 227 → 193 MB |
| m22 (complete) | 2,650 ms | 2,681 ms | 1.012× | 227 MB |
| m22 (`--budget 10M`, censored) | 1,437.5 ms | 1,468.0 ms | 1.021× | 227 MB |

Controls from the same probe (all child-eval-identical):

- **eager zero-fill** (`alloc_zeroed` + `write_bytes(0)`) is *identical* to the
  baseline (dec10 766.4 vs 767.3 ms): the non-zero `Default` values cost
  nothing; only the touching does.
- **`MADV_HUGEPAGE`** (on lazily or eagerly zeroed regions) changes nothing on
  this container (THP is `madvise`-only and no collapse is observed), so
  backlog item 8 is not a co-lever here.
- Minor faults on dec10: 57,663 (baseline) → 106,402 (lazy). The deep-class
  cost is demand faulting inside the search, not extra work.

So the change is a **latency/memory win with a small, measured throughput
cost of 1–5% on deep runs** — precisely the split verdict the initiative's
conventions separate (`AGENTS`-level priority: correctness, then performance,
then memory). This plan adopts the lazy allocation as the CLI default and
pre-registers an upper bound on the deep-run cost; it does not silently trade
away deep throughput.

## Question and pre-registered decision rule

**Q:** Can the transposition table be allocated lazily (all-zero pages, first
touch on use) with bit-identical TT *contents* and search trajectories, cutting
short-solve latency/RSS by two orders of magnitude at a deep-run wall cost that
stays within 6%?

**ADOPT (GO)** only if all hold:

1. **Identity.** The new TT content-equivalence unit test passes; 
   `test_trajectory_golden` (`m22` default mode, 18,779,890 evals) passes
   **without regoldening**; the portfolio dec10 anchor (3,901,847 evals) is
   exact; the quick suite is **59/59 eval-identical** (42,109,768 total child
   evals, 0 wrong); every measured deep cell has bit-identical `child_evals`,
   outcome and `pv_status`.
2. **Short latency.** Mate-in-1 and easy-KQ median wall ≤ **5 ms** each and
   ≥ **10×** faster than baseline.
3. **All-class latency.** Every preflight-decided case in the corpus
   (`draw_kr`, `draw_cyclic_rook`, hard ladder root) is ≥ **10%** faster in
   median wall (the fixed ~100 ms saving shows up on every class).
4. **Memory.** Mate-in-1 / easy-KQ / draw-class peak RSS ≤ **40 MB**; every
   deep cell peak RSS ≤ baseline.
5. **Deep throughput ceiling.** `max` over the deep cells (dec10, m22 complete,
   m22 at `--budget 10_000_000`, stress cell at `--budget 10_000_000`) of the
   interleaved median wall ratio is ≤ **1.06**.
6. **Gates.** `make test` and `make test-full` green, `cargo fmt --check` and
   `cargo clippy --all-targets` clean; new units in the fast tier only.

**NO-GO / fallback.**

- If **identity** (bar 1) breaks anywhere: STOP, root-cause. A drift means the
  lazy path changed semantics (most likely a read of a non-live slot without
  the `valid` gate). Do not re-baseline anything; the change is then reverted.
- If the **deep ceiling** (bar 5) is exceeded: implement the pre-registered
  fallback **D-f** (opt-in `--tt-prefault`) and re-measure. Adopt only if
  `--tt-prefault` restores every deep cell to ≤ 1.02× **and** bars 2–4 still
  hold in default (lazy) mode. If the fallback cannot reach 1.02×, NO-GO:
  revert, and record the measured trade-off as an operator decision for the
  owner (e.g. "lazy for the 3-man/portfolio path only").
- If bars 2–4 (latency/memory) fail while 5 passes, the premise is wrong:
  NO-GO and report.

## Scope

**In scope**

- A single zero-init allocation helper used by both `with_mb` and (test-only)
  `with_capacity`.
- The documented invariant that an all-zero byte pattern is a readable
  `TtEntry` with `valid == false`, plus the contract that non-live slots'
  other fields are unspecified.
- Unit tests for the allocator, content equivalence against an eager table,
  and the empty-table invariants.
- Documentation: `tt/table.rs` header, new `tt/alloc.rs` module docs,
  `src/main.rs` header/help (`--tt-size`, family RAM wording), `AGENTS.md`
  (two bullets), `docs/proofdb_pipeline.md` wording (~line 322).
- Measurement record under `docs/plans/performance/measurements/plan2/`.

**Out of scope (non-goals)**

- No change to sizing (`with_mb` round-down), bucket layout, indexing, salt,
  eviction, or `store`/`probe` semantics.
- No change to the default label (256 MB) or to any `--tt-size` behavior.
- No CLI knob **unless** the fallback D-f is triggered by bar 5.
- No `TtEntry` layout/representation refactor (`Option<Outcome>` stays as is;
  the static `Option<Outcome>` niche is what makes zeroed memory readable
  without touching the field — see D-b).
- No THP / huge-page work (backlog item 8 stays dormant; the probe measured no
  effect), no repetition-cache or movegen work (items 2/3/4/6 stay with
  `lean`).
- No change to the preflight/cheap-probe contracts (the probe's scratch TT
  simply inherits the cheaper construction).

## Normative design

### D-a. New module `src/search/tt/alloc.rs`

```rust
//! Zero-initialized bucket allocation for the transposition table.
//!
//! ... (module doc: why lazy, why unsafe, the invariant, sizing/overflow)

use super::entry::TtEntry;

/// Allocate `buckets` zero-initialized four-slot buckets.
///
/// Backed by `std::alloc::alloc_zeroed`, which for the default label
/// (224 MiB) is served by `mmap`d zero pages: the allocation costs
/// microseconds and pages become resident on first touch. Capacity and
/// layout are exactly those of `vec![[TtEntry::default(); 4]; buckets]`.
pub(crate) fn zeroed_buckets(buckets: usize) -> Vec<[TtEntry; 4]> {
    if buckets == 0 {
        return Vec::new();
    }
    let layout = std::alloc::Layout::array::<[TtEntry; 4]>(buckets)
        .expect("tt allocation layout overflows usize");
    // SAFETY: `alloc_zeroed` returns a pointer to `layout.size()` bytes of
    // zeroed memory (or null, reported via `handle_alloc_error`). All-zero
    // bytes form a valid `TtEntry` whose `valid` flag is `false`
    // (`zeroed_bytes_form_an_unoccupied_entry`), so the memory may be read
    // as `[TtEntry; 4]`; `Vec::from_raw_parts` gets the exact layout/capacity
    // that was requested. The vector is the sole owner.
    unsafe {
        let ptr = std::alloc::alloc_zeroed(layout).cast::<[TtEntry; 4]>();
        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        Vec::from_raw_parts(ptr, buckets, buckets)
    }
}
```

`table.rs` then calls `zeroed_buckets(buckets)` in `with_mb` and
`with_capacity`; `clear()` keeps writing `TtEntry::default()` (semantics
unchanged; it only matters on generation wrap).

`#[cfg(test)]`-only helper in the same module, used by the equivalence test:

```rust
#[cfg(test)]
pub(crate) fn eager_buckets(buckets: usize) -> Vec<[TtEntry; 4]> {
    vec![[TtEntry::default(); 4]; buckets]
}
```

### D-b. The safety invariant (documented in `alloc.rs` and `table.rs`)

- `std::mem::zeroed::<TtEntry>()` is a **valid** value (checked in the plan
  session on the current toolchain): `size = 56`, `align = 8`, and it reads as
  `key: 0, valid: false, generation: 0, best_move: Move(0), best_child: 0,
  work: 0, outcome: Some(Loss), pn: 0, dn: 0, depth: 0, remaining_depth: 0`.
  `bool` 0 = `false`, `Move(u16)` any bytes (`Move::NONE == Move(0)`), and
  `Option<Outcome>` carries the enum's own discriminants (0 = `Loss`). The
  unit test `zeroed_bytes_form_an_unoccupied_entry` pins `valid == false`.
- **A zeroed slot may read `outcome == Some(..)`.** The `Option<Outcome>`
  niche places `None` at a non-zero byte value, so the zeroed `outcome` field
  is *some* valid variant. `valid == false` is the only liveness signal; every
  read of an entry goes through `probe`, `entries`, `stats` or
  `best_child_counts`, all of which filter on `valid` (and, where they scan,
  on the current generation). The table header states this explicitly:
  *"Non-live slots have unspecified field values; readers must gate on
  `valid`."* This is the invariant that makes lazy allocation a
  representation-only change.
- `insert_new`'s victim score is only computed when all four slots are live,
  so zeroed slots never reach it; `probe` requires `valid && key == key` and
  `generation == current_generation` (zeroed generation is 0, current is ≥ 1).

### D-c. Why the trajectories are identical

For every operation the table's observable state is a function of the live
slots only, and the live-slot transitions (`store`, generation bump,
`clear`) are untouched. Empty slots are 100% equivalent (`!valid`), so the
first store, first probe, eviction order and generation bookkeeping are
identical bucket-for-bucket. The plan's falsifiable core claim is therefore
**bit-identical child evals** on every fixed-budget fixture; the equivalence
unit test checks it structurally (same store/probe sequence on a lazy and an
eager table → identical `entries()`, `stats()`, `best_child_counts()`).

### D-d. RSS / "RAM = TT only" contract update

The reservation is still bounded by the label (`buckets · 224 B ≤ mb · 2²⁰`),
but it is no longer committed up front: RSS becomes *touched TT pages* ≤ label.
Update:
- `src/main.rs` header (`--tt-size` line) and help text: "the table never
  allocates more than requested" stays, plus one clause that pages are
  populated on first use (so RSS is bounded by, not equal to, the label).
- `src/main.rs` header's "RAM = TT only" sentence and `AGENTS.md`'s
  `src/main.rs` bullet: same clause.
- `AGENTS.md`'s `src/search/tt/` bullet: add the zero-init allocation (and
  the non-live-slots-unspecified contract) in one sentence.
- `docs/proofdb_pipeline.md` ~line 322 (size labels are exact upper bounds):
  one clause that this is a reservation bound.

### D-e. Interaction with the cheap probe (plan1)

`cheap_probe_gate` builds a scratch `Search::new(CHEAP_PROBE_TT_MB)` per
gated root. With lazy allocation that 4 MB table becomes nearly free, so
`probe-solved` / deferred paths get slightly cheaper; no contract changes
(the scratch table still never aliases the production TT and is dropped inside
the gate).

### D-f. Pre-registered fallback (execute only if bar 5 fails)

Add `--tt-prefault` (default off): eager zero-fill of the freshly allocated
region (`std::ptr::write_bytes(ptr, 0, layout.size())`, which is the measured
baseline behavior) so throughput-first consumers can restore today's residency
pattern. Plumbing: `TranspositionTable::with_mb_prefaulted(mb, prefault)`
(or a `prefault: bool` field set at construction only — it must not be
callable after the first `store`), `Search::new_prefault(tt_mb, prefault)`,
`SearchConfig`/`src/main.rs` flag + help line, one unit test (prefaulted table
and lazy table have identical contents after the same store sequence), and the
docs bullet. Not implemented unless triggered.

## Tasks

1. **Phase 0 — re-measure the baseline (read-only).** On the release binary of
   pristine `HEAD`, capture the corpus below at `--tt-size 256`: median wall
   over ≥ 5 interleaved runs, peak RSS (`os.wait4` `ru_maxrss`), `child_evals`,
   and the `--tt-size` sweep {1, 16, 64, 256, 512} on the mate-in-1 case. Save
   as `measurements/plan2/state/baseline.json` + `env.json`. Reuse
   `measurements/plan1/driver.py` (add a `--budget` mode and `ru_minflt`
   capture; set `sys.dont_write_bytecode`).
2. **`src/search/tt/alloc.rs`** — `zeroed_buckets`, `eager_buckets`
   (test-only), module docs with the D-b invariant; register
   `mod alloc;` in `src/search/tt/mod.rs`.
3. **`src/search/tt/table.rs`** — use `zeroed_buckets` in `with_mb` and
   `with_capacity`; header paragraph for the lazy allocation + non-live-slot
   contract. No other change.
4. **Tests** (`src/search/tt/tests.rs`, plus `alloc.rs` unit tests):
   - `zeroed_bytes_form_an_unoccupied_entry`: `mem::zeroed::<TtEntry>()` reads
     as `valid == false` (documents the unsafe justification).
   - `zeroed_allocator_matches_eager_defaults`: same store/probe/generation
     sequence on `zeroed_buckets(n)` and `eager_buckets(n)` → identical
     `entries()` projection, `stats()`, `best_child_counts()`; include
     collisions, evictions, an outcome overwrite, a generation bump and
     `clear()`.
   - `fresh_table_is_empty`: `stats() == (buckets, 0, 0, 0, 1)`,
     `entries().count() == 0`, `probe(key) == None` for a spread of keys
     (guards the zeroed `outcome == Some(..)` hazard by exercising the
     `valid` gate).
   - `zeroed_buckets(0)` returns an empty vector (no zero-size allocation).
   - keep `with_mb_never_allocates_more_than_the_label` and the bucket-count
     pins green (they must not change).
   All new tests are fast-tier; no `#[ignore]`.
5. **Docs** per D-d (main.rs header + help, `AGENTS.md` two bullets,
   `docs/proofdb_pipeline.md` clause).
6. **Measure and gate** (Validation below), with an interleaved A/B driver
   (`measurements/plan2/ab_wall.py`, 15 reps/case, ABAB) as the authoritative
   wall comparison — single-arm runs are not accepted for bar 5. Commit the
   parsed `state/*.json` only (no raw transcripts).
7. **Write `report2.md`** and update `initiative.md` (item #5 row + item↔plan
   mapping) and the `docs/plans/README.md` `performance` row.

## Corpus

| id | FEN / mode | class | why |
|---|---|---|---|
| `mate1` | `3Qk3/8/8/8/8/8/8/4K3 w - - 0 1`, `--timeout 2` | 2 evals | the latency floor |
| `easy_kq` | `4k3/8/8/8/8/8/8/4KQ2 w - - 0 1` | probe-solved (418 evals) | plan1's easy class |
| `draw_kr` | `4k3/8/8/8/8/8/8/4KR2 w - - 0 1` | preflight Draw | closure-class saving |
| `draw_cyc` | `8/8/8/8/2k5/8/8/4KR2 w - - 0 1` | preflight Draw | closure-class saving |
| `ladder_root` | `8/2K5/k7/8/8/8/8/4Q3 w - - 0 1` | preflight hard win | claim unchanged, latency −~100 ms |
| `dec10` | `3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41`, `--timeout 10` | completes (4,901,858 evals) | deep, TT-memory-bound |
| `m22` | `4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22`, `--timeout 10` | completes (18,779,890 evals) | golden trajectory |
| `m22_b` | same, `--budget 10_000_000` | censored | deterministic deep cell |
| `stress_b` | `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`, `--budget 10_000_000` | censored | capacity-saturated cell |

## Validation

1. `make test` (fast tier) — all new units, `test_trajectory_golden` is
   `#[ignore]`d (slow) and stays so.
2. `make test-full` — required: the golden, stress and 60 s regression suites
   are the identity evidence.
3. Portfolio dec10 anchor: `cargo test --release --test portfolio_smoke` (or
   the documented reproduction) → 3,901,847 evals, identical outcome/PV.
4. Quick suite: `benchmark --suite quick --first-outcome --timeout 3 --json`
   → 59/59 eval-identical vs `measurements/plan1/state/quick_after.json`
   (42,109,768 total child evals, 0 wrong, 0 timeouts).
5. Corpus A/B per the pre-registered bars, interleaved (`ab_wall.py`):
   wall medians + `ru_maxrss` + `child_evals`; additionally record `ru_minflt`
   to keep the fault mechanism on the record.
6. `cargo fmt --check`, `cargo clippy --all-targets`, `cargo doc` (no new
   warnings).
7. Optional but preferred: `cargo +nightly miri test --lib tt::` on the new
   allocator tests if miri is usable in this container (the invariant is
   exactly what miri would check); if unavailable, say so in the report.

**Falsification handling:** any `child_evals` difference on a fixed-budget
fixture means the lazy path is not representation-only — STOP and root-cause
(the most likely cause is a non-`valid`-gated read of a zeroed slot; the
`probe`/`entries` filters are the places to check). Do not adjust caps,
goldens or the ceiling to make the gate pass.

## Report requirements

`docs/plans/performance/report2.md` must contain:

- the measured baseline and after tables (wall, RSS, evals, `ru_minflt`) for
  the whole corpus plus the `--tt-size` sweep, and the interleaved A/B ratios;
- the identity evidence (golden, dec10 anchor, quick suite total, deep-cell
  evals) and the content-equivalence unit test result;
- the deep-cost story (why +1–5%: demand faulting, ~2× minor faults), and the
  exact `max` ratio against the 1.06 bar;
- whether fallback D-f was triggered, and the result if it was;
- the docs/contracts updated (RSS wording, non-live-slot contract);
- the ADOPT/NO-GO verdict against the six bars;
- findings/follow-ups (e.g. whether a per-class policy or the D-f knob should
  be made permanent for offline/campaign consumers; whether the remaining
  short-class floor is now process start + probe).
