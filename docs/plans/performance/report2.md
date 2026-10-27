# Performance Report 2: zero-init (lazy) TT allocation

Plan: [`plan2.md`](plan2.md) — initiative `performance`, backlog **item 5**
(report §10, "Lazy/zero-init TT allocation"; measured in §4/§6.1 of
[`docs/performance/performance_report1.md`](../../performance/performance_report1.md)).
Measurements: [`measurements/plan2/`](measurements/plan2/) (`driver.py`,
`ab_wall.py`, `compare.py`, `state/*.json`, `env.json`).
Date: 2026-10-27. Baseline: pristine `HEAD` `fe491b4` (built from
`git archive HEAD` into `/tmp/plan2_base`).

## Verdict

**ADOPT — GO, with one documented bar-3 exception.** The table's bucket array
now comes from `std::alloc::alloc_zeroed` (`src/search/tt/alloc.rs`), so the
`--tt-size` label is a *reservation* bound and pages become resident on first
touch. Every measured cell is **child-eval, outcome, PV-length and
`pv_status` identical** to the baseline; short-class latency and memory drop
by two orders of magnitude (mate-in-1 104.6 → **1.2 ms**, 229 → **16 MB**), the
deep-cell wall cost is **+2.7…+3.6%** (ceiling 1.06 met with margin, `max`
1.036, so the pre-registered fallback D-f was **not** triggered), and bar 3's
relative form is missed on exactly one cell (`ladder_root`, −8.5% instead of
−10%) while its absolute mechanism — a fixed ~85–92 ms saving on every
closure-decided class — holds on all three. Details in §Bars and §Findings.

## What landed

- `src/search/tt/alloc.rs` (new): `zeroed_buckets` via `alloc_zeroed` +
  `Vec::from_raw_parts`, the test-only `eager_buckets` reference construction,
  and the module doc that states the safety invariant (an all-zero byte
  pattern is a valid `TtEntry` whose `valid == false`; non-live slots' other
  fields — notably `outcome`, which reads as `Some(Loss)` because of the
  `Option<Outcome>` niche — are unspecified and are never observed).
- `src/search/tt/table.rs`: `with_mb` / `with_capacity` call
  `zeroed_buckets`; header paragraph on the lazy allocation and the
  liveness contract. `clear()` still writes `TtEntry::default` (it only
  matters on generation wrap). No other change: sizing, bucket layout,
  indexing, salt, eviction and `store`/`probe` semantics are untouched.
- Tests (all in `alloc.rs`, next to the invariant they pin; `tt/tests.rs`
  keeps the pre-existing table tests untouched):
  `zeroed_bytes_form_an_unoccupied_entry`,
  `zeroed_buckets_zero_size_is_empty`,
  `zeroed_buckets_have_the_requested_length_and_are_not_live`,
  `zeroed_buckets_match_eager_defaults_on_non_live_fields`,
  `fresh_table_is_empty`, `zeroed_allocator_matches_eager_defaults`.
  All fast tier, no `#[ignore]`.
- Docs: `src/main.rs` header + `--help` (label is a reservation bound, pages
  on first use), `AGENTS.md` (the `src/search/tt/` bullet gains the lazy
  allocation + non-live-slot contract; the `src/main.rs` "RAM = TT only"
  bullet gains the reservation-bound clause), `docs/proofdb_pipeline.md`
  (`--tt-mb`: the label is a reservation bound).

## Measurements

Host: x86_64 container (4 CPUs, 15 GiB), `rustc 1.99.0`; release profile.
Full environment and per-file provenance in
[`measurements/plan2/env.json`](measurements/plan2/env.json) /
[`README.md`](measurements/plan2/README.md). Raw transcripts are not
committed; `compare.py` regenerates every table below from `state/*.json`.

### Interleaved A/B, 15 reps/case (authoritative wall)

| case | class | base wall (s) | after wall (s) | ratio | speedup | evals (both arms) | RSS base → after (MB) | minflt base → after |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| `mate1` | short | 0.1046 | **0.0012** | 0.011 | **87.0×** | 2 | 229 → **16** | 58,387 → 152 |
| `easy_kq` | short (probe-solved) | 0.1037 | **0.0017** | 0.016 | **61.0×** | 378 | 230 → **16** | 58,395 → 535 |
| `draw_kr` | preflight draw | 0.8308 | 0.7459 | 0.898 | 1.11× | 5,219,752 | 257 → 33 | 66,270 → 8,988 |
| `draw_cyc` | preflight draw | 0.8296 | 0.7412 | 0.893 | 1.12× | 5,219,771 | 257 → 33 | 66,206 → 8,871 |
| `ladder_root` | preflight hard win | 1.0180 | 0.9290 | 0.913 | 1.10× | 5,868,692 | 261 → 37 | 67,320 → 9,965 |
| `dec10` | deep (completes) | 0.6315 | 0.6542 | **1.036** | — | 3,901,847 | 227 → 178 | 57,551 → 102,334 |
| `m22` | deep (completes) | 2.1364 | 2.1938 | **1.027** | — | 15,023,911 | 227 → 227 | 57,561 → 114,864 |
| `m22_b` | deep (`--budget 10M`) | 1.4572 | 1.5060 | **1.034** | — | 10,000,032 | 227 → 227 | 57,544 → 114,838 |
| `stress_b` | deep (`--budget 10M`) | 1.3969 | 1.4474 | **1.036** | — | 10,000,015 | 227 → 226 | 57,488 → 114,743 |

`evals identical` is `yes` for all nine rows (`ab_wall.py` asserts it per
run), and the captured stdout (outcome, PV length, `pv_status`) is byte
identical within each arm.

### Interleaved A/B follow-up, 25 reps/case (the closure-decided cells)

Bar 3's 10% threshold sits near the noise floor of the 15-rep medians, so the
three closure-decided cells were re-measured with 25 interleaved reps:

| case | base wall (s) | after wall (s) | ratio | wall saved | relative |
|---|---:|---:|---:|---:|---:|
| `draw_kr` | 0.8320 | 0.7401 | 0.889 | 92 ms | **−11.1%** |
| `draw_cyc` | 0.8306 | 0.7468 | 0.899 | 84 ms | **−10.1%** |
| `ladder_root` | 1.0133 | 0.9270 | 0.915 | 86 ms | **−8.5%** |

All three arms are eval-identical (5,219,752 / 5,219,771 / 5,868,692).

### Sequential corpus (5 runs/case) — wall, RSS, minor faults

| case | base wall (s) | after wall (s) | base RSS (MB) | after RSS (MB) | base minflt | after minflt | evals |
|---|---:|---:|---:|---:|---:|---:|---:|
| `mate1` | 0.1010 | 0.0008 | 230 | 16 | 58,388 | 151 | 2 |
| `easy_kq` | 0.1006 | 0.0014 | 230 | 16 | 58,396 | 535 | 378 |
| `draw_kr` | 0.8280 | 0.7406 | 257 | 33 | 66,271 | 8,987 | 5,219,752 |
| `draw_cyc` | 0.8224 | 0.7303 | 257 | 33 | 66,206 | 8,871 | 5,219,771 |
| `ladder_root` | 0.9963 | 0.9162 | 260 | 37 | 67,321 | 9,965 | 5,868,692 |
| `dec10` | 0.6322 | 0.6585 | 226 | 178 | 57,551 | 102,334 | 3,901,847 |
| `m22` | 2.1137 | 2.2002 | 227 | 227 | 57,564 | 114,864 | 15,023,911 |
| `m22_b` | 1.4354 | 1.4963 | 227 | 227 | 57,544 | 114,840 | 10,000,032 |
| `stress_b` | 1.3659 | 1.4417 | 226 | 226 | 57,486 | 114,743 | 10,000,015 |

The sequential wall numbers agree with the interleaved ratios; the
interleaved files remain the authoritative comparison.

### `--tt-size` sweep on mate-in-1 (5 runs each)

| `--tt-size` | base wall (s) | after wall (s) | base RSS (MB) | after RSS (MB) | base minflt | after minflt |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 0.0032 | 0.0009 | 16 | 15 | 1,267 | 151 |
| 16 | 0.0095 | 0.0009 | 20 | 15 | 4,627 | 152 |
| 64 | 0.0277 | 0.0010 | 61 | 15 | 15,378 | 151 |
| 256 | 0.1012 | 0.0010 | 229 | 15 | 58,388 | 151 |
| 512 | 0.1938 | 0.0009 | 453 | 15 | 115,732 | 151 |

The baseline arm's short-solve latency is now *entirely* the allocation
(3.2 ms → 193.8 ms across the sweep, RSS 16 → 453 MB); the lazy arm is flat
at ~1 ms / 15 MB — the label no longer costs anything when it is not used.
The remaining ~1 ms floor is process start plus the 2-eval search, i.e. the
plan's premise for the short class is now retired (see §Findings).

## Identity evidence

| check | result |
|---|---|
| `test_trajectory_golden` (`m22`, 18,779,890 evals, slow tier) | **passes without regoldening** (`make test-full`) |
| corpus `child_evals` (all 9 cells × 15 interleaved reps) | identical in both arms |
| dec10 portfolio anchor | 3,901,847 evals, identical outcome/PV (`portfolio_smoke` / corpus) |
| quick suite (`benchmark --suite quick --first-outcome --timeout 3 --json`) | **59/59 eval-identical** vs `measurements/plan1/state/quick_after.json`, 42,109,768 total child evals in both arms, 0 wrong, 0 timeouts |
| `zeroed_allocator_matches_eager_defaults` | same store/probe/eviction/outcome-overwrite/generation-bump/`clear` sequence on a lazy and an eager table → identical `entries()`, `stats()`, `best_child_counts()` |
| `fresh_table_is_empty` | fresh table: `stats() == (4096, 0, 0, 0, 1)`, no `entries()`, no `best_child_counts()`, no probe hits over a spread of keys (exercises the `valid` gate against the zeroed `outcome == Some(..)` hazard) |
| `cargo +nightly miri test --lib tt::` | **35/35 pass** (miri 0.1.0, nightly 2026-10-26; installed in-container with `rustup component add --toolchain nightly miri`; 66 s) — validates the `alloc_zeroed`/`from_raw_parts` unsafe and the zeroed-`TtEntry` invariant |
| `make test` | green (fast tier) |
| `make test-full` | green, including the 60 s regression/stress suites |
| `cargo fmt --check`, `cargo clippy --all-targets`, `cargo doc` | clean, no new warnings |

`make test`, `make test-full` and the miri run were repeated on the final
tree after the new tests were relocated into `alloc.rs` (test-only move; no
product code changed between the two full-tier runs).

## The deep-cell cost

The cost is exactly the demand-faulting the plan predicted: **minor faults
roughly double** on deep cells (57.5k → 114.9k on dec10/m22/m22_b/stress_b),
and the wall cost is **+2.7…+3.6%**. Two structural confirmations:

- `dec10` (3.9M evals) does not saturate the reservation, so its *peak RSS
  also falls* (227 → 178 MB): the lazy arm only commits the pages it touches,
  while the baseline commits all 224 MiB regardless. Only cells that fill the
  table (`m22`, `m22_b`, `stress_b`) match the baseline's peak — their RSS
  differences are ±0.3 MB, i.e. noise around a fully-resident reservation
  (the sign flips run to run; `compare.py` allows 1 MB).
- The eager-fill work in the baseline is not wasted in the deep case, it is
  merely moved: 57.5k construction faults become ~57.4k extra search faults.
  The residual +2.7…+3.6% is the same page-fault work done on the critical
  path (inside the child-eval loop) instead of in one pre-touched burst.

`max` deep ratio over the four pre-registered deep cells = **1.036**
(`stress_b`), against the **1.06** ceiling; the dec10/m22/m22_b cells are
1.036 / 1.027 / 1.034. The plan-session probe predicted +1–5%; the shipped
result is at the low end of that band.

## Fallback D-f (`--tt-prefault`)

**Not triggered.** Bar 5 held (1.036 ≤ 1.06), so no `--tt-prefault` knob was
added and no CLI surface changed. If an offline/campaign consumer ever wants
today's residency pattern back, D-f remains the designed (and measured-costed)
escape hatch; see §Findings.

## Bars

| # | bar | verdict | measured |
|---|---|---|---|
| 1 | identity (unit test, golden without regoldening, dec10 anchor, quick 59/59, deep cells) | **PASS** | all cells eval-identical; quick 42,109,768 evals in both arms, 0 wrong |
| 2 | short latency ≤ 5 ms and ≥ 10× faster | **PASS** | mate-in-1 1.20 ms (87.0×), easy KQ 1.70 ms (61.0×) |
| 3 | every closure-decided case ≥ 10% faster | **PARTIAL** | `draw_kr` −11.1%, `draw_cyc` −10.1% **PASS**; `ladder_root` **−8.5% MISS** (86 ms saved of a 1.01 s solve) |
| 4 | short/draw-class RSS ≤ 40 MB; deep cells ≤ baseline | **PASS** | 16 / 16 / 33 / 33 / 37 MB; deep: −49.7 MB (dec10), ±0.3 MB noise elsewhere |
| 5 | deep ceiling `max` ratio ≤ 1.06 | **PASS** | 1.036 |
| 6 | gates green, fmt/clippy clean, new units fast-tier | **PASS** | `make test`, `make test-full`, `cargo fmt --check`, `cargo clippy --all-targets`, `cargo doc` |

Bar 3's parenthetical ("the fixed ~100 ms saving shows up on every class") is
satisfied — 92 / 84 / 86 ms on the three closure-decided cells — but its
relative form is a function of how long the *search* takes: on
`ladder_root` the closure spends 5.87M evals (~0.93 s) after the pre-phase, so
a fixed ~86 ms saving can only ever be ~8.5%. The bar was written as a
relative threshold for all three cases without that arithmetic check; the same
fixed saving that is 11–12% of a 0.83 s draw is 8.5% of a 1.01 s hard win. The
change is a strict improvement on that cell (fewer faults, lower RSS, 86 ms
faster, identical search), so the session did **not** treat this as a NO-GO;
it is recorded here as a pre-registration defect, not a product regression.

## Findings and follow-ups

1. **The short-class floor is now process start.** With the allocation gone,
   mate-in-1 runs in 1.2 ms and RSS 16 MB; `--tt-size` is free up to 512 MB.
   Any further work on this class is `fork`/`exec`/dynamic-linker territory
   (a static or `no_std`-ish binary), not search or TT work. Not worth a plan
   on current evidence.
2. **The corpus table in `plan2.md` has two stale numbers.** It lists dec10 as
   4,901,858 evals and m22 as 18,779,890 evals for the CLI configuration; the
   measured CLI values are 3,901,847 (dec10, matching the portfolio anchor in
   the same plan) and 15,023,911 (m22). 18,779,890 is the `test_trajectory_golden`
   figure (library harness, different flags) and the golden still passes at
   that count. The plan's absolute numbers were not used as targets, so no
   bar was affected.
3. **Deep-class guidance for campaign consumers.** A lazy table costs +2.7–3.6%
   wall on 1.4–2.2 s runs and *saves* RSS whenever the reservation is not
   filled (dec10: 49.7 MB less). Multi-billion-eval proofdb jobs fill the table
   and will pay the faulting cost; that is inside the ceiling and no knob was
   added. If an operator later wants the old residency (`proofdb_harvest`
   batches where wall dominates), D-f (`--tt-prefault`) is pre-costed and is
   the only sanctioned way to reintroduce it.
4. **`--tt-size` semantics are now purely a reservation.** Operators reading
   RSS as "the label" will see less; the CLI help, `AGENTS.md` and
   `docs/proofdb_pipeline.md` now say reservation bound explicitly. The one
   place that must keep the old meaning is `tt_snapshot`'s `tt_size_mb`
   header field (it records the label, i.e. the *upper* bound) — unchanged.
5. **Item #8 (huge pages) stays deferred with better evidence.** The
   plan-session probe measured no THP effect on either allocation style; the
   lazy arm now faults pages individually, and `MADV_HUGEPAGE` on a
   demand-faulted region is the configuration most likely to hurt, not help.
6. **Miri is usable in this container** (component installed during this
   session) and is cheap on the `tt` module (66 s). Worth adding to the
   `tt`-touching plans' validation lists; it would have caught a
   non-`valid`-gated read of a zeroed slot (which is also why the identity
   gate exists).

## Unresolved / next steps

- No product issue is open: all six bars are met except bar 3's relative form
  on the hard-ladder root, which is a pre-registration defect (see §Bars) and
  is flagged for the owner rather than silently reinterpreted.
- D-f (`--tt-prefault`) is **not** implemented; it stays a designed, costed
  escape hatch behind the deep-class guidance in §Findings 3.
- The measurement corpus is CLI-driven only; no missing test is known. The
  invariant that would need a new test if `TtEntry` ever grows a field with
  an invalid zero pattern (e.g. a `NonZero*` or a `char`) is covered by
  `zeroed_bytes_form_an_unoccupied_entry` failing to compile/assert — that is
  the intended tripwire.

## Tools used

- `measurements/plan2/driver.py` — single-arm corpus/sweep runner (wall, `ru_maxrss`, `ru_minflt`, evals, parsed stdout).
- `measurements/plan2/ab_wall.py` — interleaved A/B runner (the bar-2…5 authority).
- `measurements/plan2/compare.py` — table renderer + bar evaluator.
- `cargo +nightly miri test --lib tt::` — UB check of the new allocator.
