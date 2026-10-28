# Performance Report 4: Repetition-path bookkeeping

Initiative: `performance`, backlog **items 3 + 4** combined (incremental
repetition-context hash + path Bloom negative filter). Plan:
[`plan4.md`](plan4.md); probe record:
[`measurements/plan4/plan_session/`](measurements/plan4/plan_session/).
Execution date: 2026-10-28.

**Verdict: ADOPT (GO).** All six pre-registered bars pass. The
transposition-dense `dec10` cell is **0.9630** wall / **0.9614** CPU (bar
≤ 0.97), a 41-rep confirmation gives **0.9569 / 0.9563**, and every other deep
cell is between **0.9870 and 0.9964** wall. The two latency-floor cells are
1.0316 and 1.0146 (loose guard ≤ 1.05). Every corpus cell is `child_evals`-,
stdout-, outcome- and PV-identical to the pristine baseline; the `m22` golden
passes without regoldening; the `dec10` portfolio anchor is exact at 3,901,847
evals; the quick suite is 59/59 eval-identical (42,109,768 total child evals,
0 wrong, 0 timeouts).

## What shipped

- `Search::path_context_hash` — the order-independent repetition-cache context
  hash maintained incrementally by `path_push` / `path_pop`
  (`wrapping_add`/`wrapping_sub` of the same `repetition_cache::mix_key` the
  reference fold uses). The `core.rs` cache probe reads the field instead of
  folding `path_stack` once per expanded node.
- `Search::path_bloom: [u16; 256]` — a counted path-presence filter indexed by
  the low 8 bits of the repetition key. `path_contains` returns `false`
  immediately on a zero slot and falls back to the linear scan otherwise, so a
  collision costs a scan and never changes a result. `best_move_repeats_path`
  now reads through `path_contains` too.
- `path_stack` is a two-field invariant: every mutation goes through
  `path_push`/`path_pop`; `reset_search_state` is the only reset and re-seeds
  the hash and the Bloom counts from `prefix_path` with the reference fold.
- Unit tests: `incremental_context_hash_matches_fold`,
  `context_hash_replays_prefix_path`, `path_bloom_has_no_false_negatives`
  (including the test-only negative counter), and
  `path_bloom_insert_remove_returns_to_zero`, plus the existing `children.rs`,
  `history.rs` and `tests.rs` fixtures moved from direct `path_stack` mutation
  to `path_push` / `reset_search_state`.
- Docs: `mod.rs` field/method/invariant docs, the `core.rs` cache-site comment,
  the `repetition_cache.rs` module and `context_hash` docs, one `AGENTS.md`
  clause.
- Measurement record: `measurements/plan4/` (driver, state, census).

No CLI knob, public API, TT semantics/sizing/eviction/salt, ordering or
`ChildInfo` field, `ProofEvent`, preflight, or repetition-cache payload changed.

## Phase 0 — corpus record

Baseline: pristine `git archive HEAD` (`154d659`, which differs from the
plan-session base only by the plan document) built into `/tmp/plan4_base` with
the same profile as the after build (`opt-level 3`, `lto = true`,
`codegen-units = 1`, `panic = abort`). After: the final working tree. Driver:
[`measurements/plan4/driver.py`](measurements/plan4/driver.py), 5 runs/case,
`--tt-size 256 --first-outcome --outcome-only` plus the per-case args.

| cell | baseline wall | after wall | `child_evals` | RSS base→after | minflt base→after | outcome / PV / status |
|---|---:|---:|---:|---:|---:|---|
| `mate1` | 0.0011 s | 0.0011 s | 2 | 15→15 MB | 153→156 | win 1 proven-shortest |
| `easy_kq` | 0.0016 s | 0.0017 s | 378 | 15→15 MB | 536→538 | win 3 first-outcome |
| `dec13` | 0.8803 s | 0.8964 s | 5,030,497 | 219→219 MB | 113,026→113,028 | win 31 first-outcome |
| `dec10` | 0.6608 s | 0.6337 s | 3,901,847 | 178→177 MB | 102,337→102,336 | win 36 first-outcome |
| `m20_b` | 4.0451 s | 4.0074 s | 30,000,025 | 227→226 MB | 114,773→114,772 | draw 0 none |
| `m22_b` | 1.4980 s | 1.4866 s | 10,000,032 | 226→226 MB | 114,838→114,841 | draw 0 none |
| `stress_b` | 1.4253 s | 1.4237 s | 10,000,015 | 226→226 MB | 114,742→114,743 | draw 0 none |
| `rem12` | 10.2772 s | 10.0878 s | 66,604,453 | 227→226 MB | 114,873→114,877 | win 45 first-outcome |

The single-arm `dec13` row is environment drift (the authoritative ratio is
0.9964 below), and the two sub-2 ms cells are process-start dominated; their
authoritative comparison is the same-filesystem A/B. `child_evals`, outcomes,
PV lengths and `pv_status` are identical in every row. RSS is flat within
1 MB and minor faults are equal within noise (the absolute fault counts are
much larger than `report3.md`'s record on a different host/architecture; the
base/after parity is what matters).

## Authoritative A/B

[`measurements/plan4/plan_session/ab_path_bookkeeping.py`](measurements/plan4/plan_session/ab_path_bookkeeping.py)
with both binaries on `/tmp` (same filesystem), one base+after pair per
repetition, arm order counterbalanced by repetition, `child_evals` and stdout
equality a hard abort. Parsed result:
[`measurements/plan4/state/ab_final.json`](measurements/plan4/state/ab_final.json).

| cell | reps | base wall | after wall | **wall ratio** | base CPU | after CPU | CPU ratio | paired wall q1/q2/q3 | `child_evals` |
|---|---:|---:|---:|---:|---:|---:|---:|---|---:|
| `dec10` | 31 | 0.6579 | 0.6336 | **0.9630** | 0.6548 | 0.6296 | 0.9614 | 0.9520 / 0.9642 / 0.9764 | 3,901,847 |
| `m20_b` | 21 | 4.0506 | 3.9980 | **0.9870** | 4.0253 | 3.9768 | 0.9880 | 0.9794 / 0.9899 / 0.9954 | 30,000,025 |
| `m22_b` | 21 | 1.5020 | 1.4903 | **0.9922** | 1.4928 | 1.4813 | 0.9923 | 0.9851 / 0.9912 / 0.9977 | 10,000,032 |
| `stress_b` | 21 | 1.4296 | 1.4196 | **0.9930** | 1.4220 | 1.4095 | 0.9912 | 0.9818 / 0.9912 / 1.0027 | 10,000,015 |
| `dec13` | 21 | 0.8895 | 0.8863 | **0.9964** | 0.8836 | 0.8815 | 0.9977 | 0.9884 / 0.9962 / 1.0013 | 5,030,497 |
| `rem12` | 11 | 10.2445 | 10.1213 | **0.9880** | 10.1861 | 10.0642 | 0.9880 | 0.9816 / 0.9858 / 0.9944 | 66,604,453 |
| `easy_kq` | 15 | 0.0016 | 0.0016 | 1.0146 | 0.0015 | 0.0015 | 1.0153 | 0.9674 / 1.0326 / 1.0980 | 378 |
| `mate1` | 15 | 0.0009 | 0.0010 | 1.0316 | 0.0009 | 0.0009 | 1.0366 | 0.9713 / 1.0289 / 1.1007 | 2 |

Confirmation of the borderline cell:
[`state/ab_dec10_repeat.json`](measurements/plan4/state/ab_dec10_repeat.json),
41 reps, `dec10` **0.9569 wall / 0.9563 CPU** (paired q1/q2/q3 0.9436 / 0.9568
/ 0.9685), consistent with the plan-session probe's 0.954. The 31-rep run and
the 41-rep run together are the `dec10` evidence; both are under the bar.

## Mechanism evidence

**Incremental hash equality.** `incremental_context_hash_matches_fold` pushes
and pops a deterministic 64-key sequence and asserts
`path_context_hash == RepetitionCache::context_hash(&path_stack)` after *every*
operation; `context_hash_replays_prefix_path` installs a prefix path, calls
`reset_search_state`, and asserts the same equality plus that every prefix key
is found by `path_contains`. `assert_path_bookkeeping_consistent` additionally
checks the Bloom counts against the exact path multiset (stronger than the
plan's "path keys nonzero / zero-slot keys absent" wording, and it implies
both).

**Bloom soundness.** `path_bloom_has_no_false_negatives` pushes two keys that
collide in slot 0, shows the colliding absent key is still reported absent, and
shows a popped key is absent while its slot stays nonzero (the fallback scan).
The same test finds an absent key in a zero slot and asserts the test-only
`path_bloom_negatives` counter fires. `path_bloom_insert_remove_returns_to_zero`
restores all slots and the hash to zero after a full insert/remove cycle.

**Negative census.** Instrumented `/tmp` copy of the final tree (counters only,
no search decision changed; not product code):
[`state/census.json`](measurements/plan4/state/census.json).

| cell | `path_contains` calls | zero-slot negatives | negative rate |
|---|---:|---:|---:|
| `mate1` | 2 | 2 | 100.0% |
| `easy_kq` | 222 | 220 | 99.1% |
| `dec13` | 5,323,159 | 5,151,716 | 96.8% |
| `dec10` | 4,505,816 | 4,050,002 | 89.9% |
| `m20_b` | 31,605,550 | 31,027,400 | 98.2% |
| `m22_b` | 10,627,632 | 10,361,362 | 97.5% |
| `stress_b` | 10,553,057 | 10,335,657 | 97.9% |
| `rem12` | 69,818,582 | 66,602,110 | 95.4% |

The filter is used on 89.9–99.1% of lookups, so the mechanism is live on every
cell — but the negative rate does **not** predict the gain: `dec10` has the
*lowest* negative rate and the largest win, while `dec13` is 96.8% negative for
0.36%. The per-call scan cost and path length matter more than the rate; this
is a finding, not a contradiction.

**Per-arm probe attribution.** The plan-session record
([`plan_session/`](measurements/plan4/plan_session/README.md)) measured three
arms on the same driver:

| cell | item 3 alone (wall/CPU) | item 4 alone (wall/CPU) | combined (wall/CPU) |
|---|---:|---:|---:|
| `dec10` | 0.988 / 0.988 | 0.979 / 0.979 | **0.954 / 0.954** |
| `m22_b` | 1.006 / 1.006 | 0.997 / 0.997 | **0.992 / 0.992** |
| `stress_b` | 1.001 / 1.000 | 0.999 / 0.999 | **0.988 / 0.988** |
| `m20_b` | — / 1.002 | — / 1.001 | **— / 0.989** |

What the probe can attribute: item 3 alone is not a broad win on this tree
(it regresses `m22_b` ~0.6%); item 4 alone is a small win on `dec10`/`m22_b`.
It **cannot** attribute the superadditivity of the combination — there is no
per-site instrumentation that separates the fold from the scans inside the
inlined `dfpn` frame. The gate is on the combination, exactly as planned.

## Identity evidence

- `make test-full` green: 666 passed, 0 failed, 0 ignored; includes
  `test_trajectory_golden` (`m22` default mode) **without regoldening**.
- `cargo test --release --test portfolio_smoke`: the `dec10` race anchor is
  exact (3,901,847 evals, identical outcome/PV).
- `benchmark --suite quick --first-outcome --timeout 3 --json`:
  [`state/quick_after.json`](measurements/plan4/state/quick_after.json), 59/59
  eval-identical to `measurements/plan2/state/quick_after.json`, 42,109,768
  total child evals, 0 wrong, 0 timeouts.
- Every corpus cell: `child_evals`, stdout, outcome, PV length and
  `pv_status` identical in the single-arm record and asserted equal per
  repetition by the A/B driver (a mismatch aborts the driver).

## Equivalence argument as implemented

- **D-a/D-b — fields, constants, reset.** `PATH_BLOOM_SLOTS = 256`,
  `path_context_hash: u64`, `path_bloom: [u16; 256]`, and the test-only
  `Cell<u64>` counter are private `Search` state initialized in `new`.
  `reset_search_state` clears both new pieces and re-seeds them from
  `prefix_path` with `RepetitionCache::context_hash` (once per work chunk, not
  per node).
- **D-c — path methods and hot site.** `path_contains` is the zero-slot
  shortcut plus `path_stack.contains`; `path_push`/`path_pop` update the counts
  and the hash with exact wrapping inverses. `core.rs` reads
  `self.path_context_hash` and keeps the pre-`path_push` local for the store
  site; `best_move_repeats_path` reads through `path_contains`.
- **D-d — mutation invariant.** Stated on `path_stack` and the methods; all
  existing direct mutations in the `children.rs`, `history.rs`, `tests.rs`
  fixtures now use `path_push` / `reset_search_state`; the test helper checks
  the hash and the exact Bloom multiset; `debug_assert!`s guard counter bounds.
- **D-e — untouched semantics.** Proof-event emission, TT, ordering,
  `ChildInfo`, repetition-cache payload/capacity, outcomes and PV paths are
  unchanged. The only semantic-adjacent edit is `best_move_repeats_path` using
  `path_contains`, which is equal to `path_stack.contains` by the zero-slot
  soundness contract.

Deviations from the plan text (none semantic):

1. `#[inline]` was added to `path_bloom_slot`, `path_contains`, `path_push`
   and `path_pop` (the plan did not specify attributes; measured code includes
   this).
2. `assert_path_bookkeeping_consistent` checks the exact Bloom multiset
   instead of only the two one-directional properties (strictly stronger).
3. The census used a throwaway `/tmp` copy with the `#[cfg(test)]` counter
   always compiled, a `path_lookups` counter, and a `path_census:` stderr line
   in `bounded_search`; this is measurement-only and not in the product.
4. No `children.rs` documentation cross-reference was added (the plan marked
   it optional); only its test fixtures changed.

## Verdict against the six bars

| bar | requirement | result |
|---|---|---|
| 1. Identity | bit-identical everything; golden un-regoldened; anchor 3,901,847; quick 59/59 = 42,109,768; new units pass | **PASS** |
| 2. Mechanism | negative counter > 0 on a zero-slot fixture; hash == fold after every push/pop and after prefix seeding; no Bloom false negative | **PASS** |
| 3. Throughput | `dec10` ≤ 0.97; `m20_b`/`m22_b`/`stress_b` ≤ 0.995; `dec13`/`rem12` ≤ 0.998; CPU ≤ bar + 0.003 | **PASS** (max: `dec10` 0.9630, `m22_b` 0.9922, `stress_b` 0.9930, `m20_b` 0.9870, `dec13` 0.9964, `rem12` 0.9880; CPU 0.9614/0.9923/0.9912/0.9880/0.9977/0.9880) |
| 4. No regression | no deep cell > 1.005; short cells ≤ 1.05 | **PASS** (max deep 0.9964; short 1.0316, 1.0146) |
| 5. No semantic surface | private state/methods and the `best_move_repeats_path` read path only | **PASS** |
| 6. Gates | `make test`, `make test-full`, fmt, clippy `--all-targets`, doc | **PASS** |

## Profiling (optional check)

`perf record -e cpu-clock -g` over 20 `dec10` runs per arm (leaf attribution,
`--no-children`): `dfpn` 37.35% → 35.88%, `evaluate_child_keyed` 16.39% →
15.39%; the old `RepetitionCache::context_hash`/`path_stack.contains` work has
no separate leaf symbol in either arm because it is inlined into `dfpn`, so
the counterbalanced CPU-time ratio (0.9614) is the load-bearing evidence, not
the profile. Raw perf data is not committed (transcript policy).

## Findings and follow-ups

- **The combined effect is class-shaped.** The transposition-dense `dec10` cell
  gets ~3.7–4.3%; censored deep cells 0.7–1.3%; the long completing `rem12`
  1.2%; `dec13` 0.36%. No cell regresses.
- **Bloom negative rate is not a gain predictor** (see the census table):
  `dec10` is the least negative cell and the biggest win. Future path-work
  plans should measure scan cost/path length, not just the negative rate.
- **Item 6 (faster repetition-cache hasher)** is still visible in the profile
  (`RandomState::hash_one::<&(u64, u64)>` 0.65–0.75% leaf), but the hot path no
  longer folds the path, so it remains a <1% micro-optimization; low priority.
- **Terminal existence query** (`has_legal_move_with_state`, report3's 8.5% +
  7.3% of `dec10` CPU) was not touched and remains the largest single
  `evaluate_child` cost after plan3.
- **Item 7 (unstable sort)** and **item 8 (huge pages)** are unchanged: not
  recommended / deferred on prior evidence.

## Measurement inventory

Committed under [`measurements/plan4/`](measurements/plan4/):

- `plan_session/ab_path_bookkeeping.py`, `combined.json`,
  `incremental_only.json`, `bloom_only.json` — the plan-session probe
  (unchanged, created by the plan session).
- `driver.py` — single-arm corpus driver (plan3's driver adapted to the plan4
  corpus: `m20_b` added, `m22` removed).
- `state/baseline.json`, `state/after.json`, `state/ab_final.json`,
  `state/ab_dec10_repeat.json`, `state/quick_after.json`, `state/census.json`,
  `state/env.json`.

Not committed: raw solver transcripts, perf data, the `/tmp` baseline and
census build trees.
