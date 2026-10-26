# Performance Analysis Report 1

Date: 2026-10-26  
Commit: `b77255e` (`add and implement proofdb plan24`)  
Binary: release (`opt-level=3`, `lto=true`, `codegen-units=1`, `panic=abort`)

This report benchmarks and profiles the product solver (`atomic_solver`) on the
reference container, correlates the measurements with the hot-path source, and
records several measured optimization experiments. The experimental source
patches were built in `/tmp` copies and were **not landed**; the measurements
are evidence for future plans, not product changes.

---

## 1. Executive summary

| # | Finding | Evidence | Action |
|---|---------|----------|--------|
| 1 | Throughput is roughly **10M child evaluations/s** and **0.5–0.7M nodes/s** on the hard-4-CPU container. The quick suite (59 cases) solves in **4.21–4.33 s** of search time / **42.1M child evals** (excluding warm-up and TT allocation). | `benchmark --suite quick` | Baseline |
| 2 | The hot path is dominated by child evaluation, board make/unmake, movegen/terminal checks, static scoring, and the TT prefetch hash pass. No single micro-hotspot exceeds ~21%. | `perf` flat profile | Optimize the top 3–5 together |
| 3 | The **4-line TT prefetch pre-pass is essential**, not optional: removing it makes m22-10M **~1.9× slower** and makes m20-30M miss its deterministic budget (timing out at 5 s). A one-line prefetch is also much worse. | A/B builds | **Keep the 224-byte / 4-line prefetch** |
| 4 | The default **256 MB TT label allocates 224 MiB eagerly** (`vec![default; buckets]`). Median process-side cost on a trivial solve is **9.4 ms** for 256 MB vs **3.2 ms** for 1 MB, and RSS is **~232 MB even for a 30-eval solve**. The benchmark timer excludes `Search::new`, so benchmark numbers understate CLI latency for short solves. | TT-size/tiny-FEN matrix | Consider lazy/zero-init allocation; document benchmark exclusion |
| 5 | The **3-man preflight** has a large fixed closure cost (~2.3–5.8M evals, 0.28–0.79 s). On easy KQ/KR-vs-K wins, `--no-preflight` solves in **~10 ms with 229–337 evals**, i.e. preflight is **~60–75× slower** there. On positions where DF-PN otherwise times out, preflight is a large win. | 3-man A/B | Highest product-impact investigation: cheap search probe first, or a tighter cost gate |
| 6 | TT-size effects are strongly **trajectory-bound and non-monotonic** at short budgets. On the quick suite, 32 MB is fastest (3.08 s) while the default 256 MB takes 4.33 s; on m22 at 5 s, 16/128/256 MB solve but 1/32/64/512 MB time out. Do not tune capacity with single draws. | TT-size matrix, quick/move-order suite | Use deterministic child evals + multiple salts; keep 256 for hard capacity |
| 7 | Two low-risk repetition-path optimizations measured together at **~2–7%**: maintain the repetition-cache context hash incrementally instead of re-folding the path at every node (2–3%), and add a path Bloom negative filter (1–3%). | `/tmp` A/B builds | Incremental context hash is the better first candidate |
| 8 | **13–38% of child evaluations already find a TT entry**, but the code always plays the child move before probing. A pre-probe using the hashes already computed by the prefetch pass could skip make/unmake for that fraction. Estimated **3–8%** overall, workload-dependent. | Instrumented `/tmp` build | Worth a dedicated plan |
| 9 | `target-cpu=native` gave **no measurable gain** (<1%, within noise). A reuse experiment for the per-child terminal-check `StateInfo` gave **no gain**. | `/tmp` builds | Do not pursue either as a performance lever |

The largest *product-visible* issue is not raw search throughput: it is the
3-man preflight spending a fixed ~0.3–0.8 s closure on positions that DF-PN
can solve in milliseconds. The largest *throughput* opportunities are the TT
pre-probe before make/unmake and the repetition-path bookkeeping.

---

## 2. Environment and method

### 2.1 Environment

| Item | Value |
|---|---|
| Host | `Linux f9433fe73e5c 6.17.7-300.fc43.aarch64` |
| CPU | aarch64, Apple vendor, 6 online CPUs, 1 thread/core |
| cgroup CPU quota | 4 CPUs (`cpu.max = 400000/100000`) |
| Memory limit | 8 GiB |
| Rust | `rustc 1.99.1` / `cargo 1.99.1` |
| Profile | release, fat LTO, 1 codegen unit, panic=abort |
| PMU | **unavailable** (guest has no virtual PMU); `cpu-clock` software sampling only |
| THP | `madvise`; no explicit hugepage use by the TT |

### 2.2 Measurement setup

- Primary throughput metric: **child evaluations** (`evals: <n>` on stderr)
  because it is deterministic for fixed `(FEN, config, salt, TT size)`.
- Wall time is reported as a secondary metric. Repeated runs of a fixed
  configuration are stable to roughly **1–3%**; comparisons below use 2–3
  repetitions where noise matters.
- Two timing scopes are distinguished:
  - **Benchmark timer** (`examples/benchmark`): starts after `Search::new`,
    so it excludes TT allocation/initialization, and it runs one warm-up per
    case that is not included in the mean.
  - **CLI wall/RSS**: end-to-end process, including TT allocation.
- No CPU pinning was used. The container is shared with its own background
  load; this is the main source of timing drift.
- Profiling: `perf record -e cpu-clock -g`; flat leaf attribution with
  `perf report --no-children --no-call-graph`. Kernel symbols are unavailable,
  user-space attribution is intact.

### 2.3 Workloads

| Workload | FEN / suite | Notes |
|---|---|---|
| Quick suite | `benchmark --suite quick` | 59 decisive/move-order cases, all solved |
| m22_white | `4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22` | medium/hard, solves ~15M evals |
| m20_white | `4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R5RK w - - 4 20` | hardest fixture class; not solved at 5 s |
| dec10 | `3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41` | solves ~3.9M evals, TT-entry-heavy |
| 3-man KQ/KR/KN/KB vs K | see §7 | exercises the preflight |

---

## 3. Baseline results

### 3.1 Quick suite (59 cases, first-outcome, 3 s timeout, default 256 MB)

| Metric | Value |
|---|---:|
| Cases solved | **59 / 59** |
| Wrong outcomes | 0 |
| Total child evals | **42,109,768** |
| Total nodes | **2,840,104** |
| Search time, sum of per-case means | **4.21–4.33 s** |
| Effective child-eval rate | **~9.7–10.0M evals/s** |
| Effective node rate | **~0.66–0.67M nodes/s** |
| Process wall incl. warm-up | **~9.4 s** |

Slowest cases:

| Case | Time (s) | Nodes | Child evals |
|---|---:|---:|---:|
| m23_white | 1.200 | 899,510 | 11,621,376 |
| dec01 | 0.609 | 299,530 | 6,021,944 |
| dec13 | 0.549 | 217,325 | 5,030,497 |
| dec10 | 0.363 | 552,328 | 3,901,847 |
| m23_black | 0.286 | 166,338 | 2,878,005 |
| dec14 | 0.255 | 106,728 | 2,443,935 |

The benchmark wall (~9.4 s) is more than twice the reported search sum
(~4.3 s) because `bench_case` performs one warm-up run per case. The warm-up
is intentional, but it makes benchmark wall a poor proxy for batch cost.

### 3.2 Hard case behavior

| Case | Mode | Time | Nodes | Child evals | Result |
|---|---|---:|---:|---:|---|
| m22_white | `--first-outcome` | ~1.57 s | 417,974 | 15,023,911 | win, 75 plies |
| m22_white | default refinement | ~1.94 s | 1,113,301 | 18,779,890 | win, cap-cut |
| m20_white | `--first-outcome`, 5 s | 5.01 s | 1,591,388 | ~49.9M | timeout |
| m20_white | 30M child-eval budget | ~2.94–3.04 s | 1,519,747 | 30,000,025 | budget exhausted |

m20 is the representative deep case: at 5 s it has spent ~30–50M child evals
and is still far from the shipped portfolio-class cost. The wall-time gap is
an algorithmic/parallelism issue, not a low-level throughput issue.

### 3.3 Refinement cost

Iterative PV refinement is not free even when it cannot shorten the line:

| Case | First outcome | Refined (default) | Delta |
|---|---:|---:|---:|
| dec10 | 0.384 s / 3.90M evals | 0.485 s / 4.90M evals | +26% evals |
| dec13 | 0.566 s / 5.03M evals | 0.704 s / 6.29M evals | +25% evals |
| m22 | 1.573 s / 15.02M evals | 1.942 s / 18.78M evals | +25% evals |

All three refinement runs ended `cap-cut` (the per-round cap of
`max(1M, 0.25 × first_outcome_evals)` was hit). For latency-sensitive use,
`--first-outcome` is a predictable ~20–25% saving on already-solved cases.

---

## 4. Hot-path profile

`perf` leaf attribution, sorted by self time. Percentages are of all sampled
CPU time; column values for the same binary do not sum to exactly 100% because
of omitted sub-1% entries and sampling noise.

| Symbol | m22 (10M budget) | m20 (30M budget) | Interpretation |
|---|---:|---:|---|
| `Search::evaluate_child` | **16.56%** | **21.28%** | per-child TT probe, terminal classification, path checks |
| `Board::do_move` | **16.29%** | **16.48%** | every evaluated child makes/unmakes a move |
| `Search::dfpn` | **15.88%** | **15.63%** | recursion, selection, thresholds, path operations |
| `Board::hash_after` | **7.58%** | **6.97%** | prefetch pre-pass hashing |
| `StaticAtomicScorer::score_with_context` | **7.25%** | **8.02%** | static move ordering |
| `Board::undo_move` | 5.62% | 3.16% | symmetry with `do_move` |
| `Board::populate_state` | 4.96% | 4.06% | checkers/pinned/counts for legal check |
| `Board::compute_checkers` | 4.65% | 4.46% | terminal/legality support |
| `has_legal_move_with_state` | 3.90% | 4.36% | early-exit legal-move existence query |
| `Board::legal` | 3.37% | 2.17% | per-pseudo-move legality |
| `castling_rights_after` | 2.70% | 3.12% | hash-after/castling bookkeeping |
| `generate_pseudo_legal` | 2.52% | 2.67% | move generation |
| sort path (`sort_by_key` quicksort/insertion/drift) | ~3.5% | ~3.0% | move ordering sort |
| `generate_legal_with_state` | 1.34% | 1.27% | full generation at frame entry |
| `select_from_children` | 0.66% | 0.45% | DF-PN selection |
| `TranspositionTable::store` | <0.4% | 0.40% | surprisingly small self time |
| `DefaultHasher::write` | 0.32% | 0.32% | repetition-cache `HashMap` |
| `__memcpy_generic` | 0.39% | 0.44% | copy paths/events |

### 4.1 Interpretation

- **Make/unmake is the single biggest controllable cost.** The call graph
  shows `Board::do_move` is called almost entirely from
  `evaluate_child` (10M child evals vs only 565k recursive frames on m22).
  Every child is played and undone even when the TT already contains a
  solved entry or reusable bounds. This is the basis for the TT pre-probe
  opportunity in §7.
- **Movegen + terminal checks** (`populate_state`, `compute_checkers`,
  `has_legal_move`, `legal`, `generate_pseudo_legal`, `generate_legal`) are
  together ~20–23% of CPU. These are in the external `atomic-movegen` crate;
  meaningful wins there require an upstream change.
- **Prefetch hashing** is ~7–8% but buys a larger win (§5).
- **Move ordering** is ~10–11% (scoring + sorting). Earlier tuning work
  already explored the parameter space; structural changes (e.g. unstable
  sort) would need the full drift gate because tie order is search-visible.
- TT probe/store itself is cheap relative to board work.

---

## 5. TT prefetch is load-bearing

The `evaluate_all_children` pre-pass computes `Position::hash_after(move)` for
every child and issues four `prfm`/`_mm_prefetch` hints at byte offsets
0/64/128/192 of the bucket (`src/search/tt/table.rs`,
`src/search/dfpn/children.rs`). Measured variants:

| Workload | Original (4-line) | 1-line prefetch | No prefetch |
|---|---:|---:|---:|
| m22, 10M evals | **~1.02–1.05 s** | ~1.68–1.71 s (1.6×) | ~1.97–1.99 s (1.9×) |
| m20, 30M evals | **~2.95–3.03 s** | 5 s timeout, ~30M evals | 5 s timeout, ~24.7–25.3M evals |
| dec10, 3.9M evals | **~0.38 s** | not measured | ~0.495–0.50 s (1.3×) |
| m22, 16 MB TT, 10M evals | **~0.86–0.95 s** | not measured | ~1.03–1.09 s (1.2×) |

The 4-line coverage is clearly better than one-line, and both are far better
than none. The documentation claim that full 224-byte coverage was the
measured winner still holds on this hardware.

**Recommendation:** do not remove or shrink the prefetch as a micro-optimization
without a new gate. The ~7% hash cost is paid to avoid a much larger memory
latency cost.

---

## 6. Memory, allocation, and startup

### 6.1 TT allocation is eager and fully resident

`TranspositionTable::with_mb(256)` builds `vec![[TtEntry::default(); 4]; 2^20]`
= **224 MiB** of non-zero defaults. This is fully touched at construction.

Median end-to-end time for a trivial position (30 child evals), 10 runs each:

| `--tt-size` | Median wall | RSS |
|---:|---:|---:|
| 1 MB | 3.16 ms | ~12 MB |
| 16 MB | 3.33 ms | ~17 MB |
| 64 MB | 4.42 ms | ~60 MB |
| 128 MB | 6.22 ms | ~117 MB |
| **256 MB (default)** | **9.36 ms** | **~232 MB** |
| 512 MB | 17.02 ms | ~461 MB |

The default therefore adds roughly **6 ms and ~220 MB** over a 1 MB table
even when the search uses only a handful of entries. For a single deep solve
this is negligible; for short solves, many-process portfolios, or memory-
constrained packing it is significant.

**Opportunity:** if `TtEntry` had an all-zero invalid representation, the table
could be allocated with lazy zero pages (`alloc_zeroed` / `mmap` semantics)
instead of eagerly writing every default. Capacity would be unchanged and
unused pages would never become resident. This is a representation change,
not a sizing change, and must preserve probe/store semantics.

### 6.2 Repetition cache

- Default capacity `1 << 18` entries with a `HashMap<(u64,u64), u32>`; the
  working set in the stress case is far below the cap.
- Uses `std`'s default SipHash; `DefaultHasher::write` is ~0.3% of samples.
  A faster hasher could recover perhaps ~0.5–1%, but it is not a priority.
- `RepetitionCache::context_hash` previously re-folded the entire path stack
  at every expanded node. See §7 for the measured incremental alternative.

### 6.3 Preflight memory

The 3-man preflight closure is the documented bounded exception to
"RAM = TT only". Measured max RSS on KQ/KR/KN/KB-vs-K positions:

| Position | Default (preflight) RSS | `--no-preflight` RSS |
|---|---:|---:|
| KQ vs K | ~269 MB | ~232 MB |
| KR vs K | ~265–266 MB | ~232 MB |
| KN vs K | ~269 MB | ~232 MB |
| KB vs K | ~253 MB | ~232 MB |

The closure adds roughly **20–37 MB** on top of the default TT. This matches
the module documentation (~35 MB for the measured KQvK region) and is below
the 85 MB worst-case bound.

### 6.4 THP / TLB

THP is set to `madvise` and the TT is a normal glibc allocation, so it uses
4 KiB pages. An attempt to use `glibc.malloc.hugetlb=1` produced no measurable
improvement on m22-10M. The 1 MB vs 256 MB fixed-budget difference below is
therefore likely a mix of cache footprint and TLB pressure, but the container
has no PMU counters to separate these.

---

## 7. Measured optimization experiments

All experiments below are **temporary `/tmp` builds**. The production tree was
not modified.

### 7.1 Incremental repetition-context hash (recommended)

`src/search/dfpn/core.rs` calls
`RepetitionCache::context_hash(&self.path_stack)` once per expanded node. The
hash is an order-independent wrapping sum of mixed path keys, so it can be
maintained incrementally in `path_push` / `path_pop` instead of re-folding an
O(depth) path.

| Workload | Original | Incremental | Change |
|---|---:|---:|---:|
| m22, 10M evals | 1.043–1.072 s | 1.010–1.021 s | **~2–3% faster** |
| m20, 30M evals | 2.986–3.057 s | 2.929–2.957 s | **~2% faster** |
| dec10, 3.9M evals | 0.373–0.378 s | 0.369–0.378 s | ~1% (within noise) |

Nodes, child evals, and outcomes were bit-identical in all runs. A
`cargo test --release --lib` run on the patched copy passed 274/274 tests,
including the repetition-cache reuse and prefix-path tests.

**Assessment:** low risk, bit-identical, small but real. Best first
"free" throughput win.

### 7.2 Path Bloom negative filter (optional)

Add a 256-slot path-presence filter (u16 per-slot counts, 512 B). A zero
count is a sound negative; a non-zero count falls back to the existing linear
`Vec::contains` scan. This avoids most O(depth) scans at the cost of one array lookup and a
counter update on push/pop.

| Workload | Original | Bloom | Change |
|---|---:|---:|---:|
| m22, 10M evals | 1.012–1.025 s | 0.993–1.036 s | ~1–2% (noisy) |
| m20, 30M evals | 2.907–2.962 s | 2.888–3.036 s | ~0–2% (noisy) |
| dec10, 3.9M evals | 0.375–0.383 s | 0.363–0.367 s | **~3% faster** |

Combined with 7.1, dec10 improved from ~0.38–0.43 s to ~0.36–0.37 s in the
best runs, while m22 was ~1–2% faster. The combined result is not clean enough
to claim gate-grade; it is a plausible second-order win.

**Caveat:** tests and any future code must mutate `path_stack` only through
`path_push`/`path_pop`, otherwise the filter can produce a false negative.
This is easy to enforce but must be documented.

### 7.3 TT pre-probe before make/unmake (highest-throughput idea)

Instrumentation on m22/m20/dec10 counted what `evaluate_child` finds after it
has already played the move:

| Workload | Total child evals | TT solved entry | TT unsolved entry | No TT entry | Path repeat | Extinction | rule50 |
|---|---:|---:|---:|---:|---:|---:|---:|
| m22-10M | 10,000,032 | 62,337 (0.6%) | 1,294,098 (12.9%) | 8,620,602 (86.2%) | 4,463 | 18,532 | 0 |
| m20-30M | 30,000,025 | 85,778 (0.3%) | 4,277,773 (14.3%) | 25,602,114 (85.3%) | 6,397 | 27,963 | 0 |
| dec10 | 3,901,847 | 51,615 (1.3%) | 1,435,215 (36.8%) | 2,366,626 (60.7%) | 43,726 | 4,665 | 0 |

Every child evaluation currently does `do_move` → `hash` → `repetition_key`
→ terminal/TT checks → `undo_move`. For the **13–38%** of children that
already have a usable TT entry, the move is played only to compute the TT key
and repetition key. The prefetch pre-pass already computes all child keys via
`hash_after`; caching those keys alongside the move list would allow:

1. compute `(child_key, child_repetition_key)` once in the prefetch pass;
2. check path repetition and probe the TT before `do_move`;
3. skip `do_move`/`undo_move` entirely when the entry is usable;
4. only fall through to make/unmake + terminal classification when no entry
   exists.

The exact save depends on how many entries pass the depth guard; the
instrumented counts are an upper bound. A rough estimate from the profile
(`do_move` + `undo_move` ≈ 20–22% of CPU, with ~95% of calls coming from
`evaluate_child`) is **3–8% overall**, with dec10-class workloads at the high
end. This needs a dedicated change because it touches `ChildInfo` production
and the prefetch pre-pass, but it is the cleanest way to attack the largest
hot symbol.

### 7.4 Reusing the terminal-check `StateInfo` (rejected)

`evaluate_child` allocates a fresh `StateInfo` for the legal-move existence
query. Replacing it with a reusable scratch slot (in `/tmp`) produced no
measurable change on m22/m20/dec10. The compiler/allocator already handles
this well, or the `populate_state` work dominates the zeroing. Not worth a
product change.

### 7.5 `target-cpu=native` (rejected)

Built with `RUSTFLAGS="-C target-cpu=native"` on the aarch64 host:

| Workload | Generic | Native |
|---|---:|---:|
| m22, 10M evals | 1.043–1.061 s | 1.053–1.059 s |
| m20, 3 s timeout | 29.6–30.3M evals | 29.4–30.7M evals |

No gain beyond noise. The dependency is already compiled with the default
aarch64 target and the solver's hot loops are memory/movegen bound rather than
autovectorization bound. Keep the portable build.

### 7.6 Move ordering insertion sort / stable sort

Sorting is ~3% of CPU. Switching from stable `sort_by_key` to an unstable sort
could recover ~1%, but tie order is search-visible: it changes move order and
therefore the entire DF-PN trajectory. Given the project's drift gate and the
history of ordering experiments, this is not worth attempting without a full
multi-salt gate.

---

## 8. TT sizing study and the non-monotonic basin problem

### 8.1 Fixed child-eval budget (m22, 10M evals)

| TT size | Wall | Nodes | Child evals | Note |
|---:|---:|---:|---:|---|
| 1 MB | **0.761 s** | 536,518 | 10,000,001 | fastest per eval, capacity-starved |
| 8 MB | 0.872 s | 579,632 | 10,000,021 | |
| 16 MB | 0.860 s | 581,476 | 10,000,010 | |
| 64 MB | 0.977 s | 565,492 | 10,000,001 | |
| 256 MB | 1.044 s | 565,257 | 10,000,032 | default |

At a fixed eval budget the large table is **~25–35% slower per eval**,
consistent with cache/TLB footprint. This is not a reason to shrink the default:
the smaller table has worse capacity on the completion metric.

### 8.2 Completion at a 5 s timeout (m22)

| TT size | Result | Wall | Child evals |
|---:|---|---:|---:|
| 1 MB | timeout | 5.01 s | 67,477,266 |
| 8 MB | win | 3.21 s | 34,399,785 |
| 16 MB | win | 1.83 s | 20,756,014 |
| 32 MB | timeout | 5.01 s | 58,881,474 |
| 64 MB | timeout | 5.01 s | 56,068,596 |
| 128 MB | win | 1.98 s | 19,111,111 |
| **256 MB** | **win** | **1.57 s** | **15,023,911** |
| 512 MB | timeout | 5.02 s | 54,600,341 |

The 32/64 MB timeouts while 16 MB and 128 MB solve are a **trajectory
bifurcation**, not a smooth capacity curve: changing the bucket index changes
which entries collide and evict, which changes move ordering and thresholds,
which changes the searched tree. This matches the project's existing
gate methodology (multi-salt, paired ratios, no single-draw verdicts).

### 8.3 Quick suite (59 cases, 3 s timeout, first-outcome)

| TT size | Search time | Nodes | Child evals | Solved | Timeouts |
|---:|---:|---:|---:|---:|---|
| 16 MB | 5.93 s | 5,356,882 | 74,311,798 | 58 | dec01 |
| **32 MB** | **3.08 s** | 2,149,472 | 36,672,658 | **59** | 0 |
| 64 MB | 3.74 s | 2,458,889 | 39,698,356 | 59 | 0 |
| 128 MB | 3.97 s | 2,496,363 | 40,042,745 | 59 | 0 |
| 256 MB | 4.33 s | 2,840,104 | 42,109,768 | 59 | 0 |

The quick suite is **~40% faster at 32 MB** than at the default 256 MB, even
though the benchmark timer excludes TT allocation. The quick suite simply
does not need the capacity; its gains/losses are trajectory effects. This is
not evidence that 256 MB is wrong for the product: the move-order suite at a
3 s timeout still shows the hard cases behaving non-monotonically (16 MB
solves 15/19 with 4 timeouts, 256 MB solves 14/19 with 5 timeouts; 32/64 MB
fail both m22 sides), and the known hard-class capacity work is specifically
about deep searches.

**Recommendation:** keep the 256 MB default. If a lower-latency default is
ever considered, it must be justified on the hard class (m20/stress) and with
multiple salts, not on the quick suite alone.

---

## 9. Three-man preflight: large fixed cost, workload-dependent value

Detector: `<= 3 men`, no pawns, no castling rights. On every such position the
preflight runs a region-closure AND/OR fixpoint before DF-PN. Measured A/B,
2 s timeout, no `--first-outcome` difference (preflight unaffected):

| Position | Preflight | `--no-preflight` | Verdict |
|---|---:|---:|---|
| KQ vs K (Qf1) | 0.766 s / 5,827,506 evals | **0.011 s / 229 evals** | preflight ~70× slower |
| KQ vs K (Qd1) | 0.744 s / 5,827,510 | **0.012 s / 235** | ~62× slower |
| KQ vs K (Qa1) | 0.758 s / 5,827,518 | **0.011 s / 337** | ~69× slower |
| KQ vs K (Qd8) | 0.785 s / 5,827,466 | **0.011 s / 278** | ~71× slower |
| KR vs K (Rf1) | **0.569 s / 5,217,752 draw** | 2.01 s timeout / 23.1M | preflight wins |
| KR vs K (Ra1) | **0.599 s / 5,217,752 draw** | 2.01 s timeout / 23.2M | preflight wins |
| KN vs K (Nf1) | **0.558 s / 4,109,608 draw** | 2.01 s timeout / 25.9M | preflight wins |
| KB vs K (Bf1) | **0.282 s / 2,299,948 draw** | 2.01 s timeout / 27.7M | preflight wins |

Observations:

- The closure cost is nearly constant for a given material class
  (~5.8M evals for KQ, ~5.2M for KR, ~4.1M for KN, ~2.3M for KB), independent
  of how easy the position actually is.
- For easy **wins**, DF-PN is already solving in 229–337 evals. The preflight
  delays the answer by ~0.75 s and consumes ~5.8M evals. This is the largest
  product-visible performance regression found in this analysis.
- For otherwise **unsolved draws**, the preflight is decisive and is a major
  win. The architecture is correct for that class.
- The preflight also adds ~20–37 MB RSS for the closure.

**Possible directions** (not implemented):

1. Run a cheap bounded DF-PN probe first (e.g. a small child-eval budget or a
   shallow depth bound); if it proves the root, skip the closure. This would
   preserve the draw wins while eliminating the easy-win regression.
2. Add a cheap "easy position" detector (e.g. material class / distance
   heuristic) that defers the preflight to the ordinary search.
3. Reduce the closure cost itself (e.g. incremental / symmetry reduction or a
   smaller per-class region budget). This is a correctness-sensitive change
   and should be planned separately.
4. Expose the preflight decision in the CLI so latency-sensitive users can
   choose `--no-preflight` per workload; the flag already exists, but the
   default behavior is the issue.

---

## 10. Ranked opportunities

### High product impact

1. **Preflight cost gate / cheap-search probe** — removes a ~60–75× latency
   regression on easy 3-man wins while keeping the draw wins. This is a
   performance/product decision, not a micro-optimization.

### High throughput, moderate implementation cost

2. **TT pre-probe before make/unmake** — 14–38% of child evals already find a
   TT entry; cache the prefetch-pass hashes and skip `do_move`/`undo_move`.
   Estimated 3–8% overall. Requires changes to `ChildInfo` production.
3. **Incremental repetition-context hash** — bit-identical, ~2–3%, low risk,
   small diff. Recommended first throughput change.

### Small / optional

4. **Path Bloom negative filter** — ~1–3%, low risk if `path_stack` mutation
   stays behind `path_push`/`path_pop`.
5. **Lazy/zero-init TT allocation** — ~6 ms and ~220 MB startup/RSS on short
   solves; representation change, must preserve TT semantics.
6. **Faster repetition-cache hasher** — likely <1%; low priority.
7. **Unstable move sort** — ~1%, but search-visible tie order; full gate
   required, not recommended on current evidence.
8. **Huge pages / explicit `madvise(MADV_HUGEPAGE)`** — not demonstrated with
   the glibc tunable; would need a raw allocation change and a proper A/B.

### Upstream / structural

9. **`atomic-movegen` hot paths** — `do_move`, `undo_move`, `hash_after`,
   `populate_state`, `compute_checkers`, and legal-move generation are
   together ~50% of CPU. The largest remaining win is in the external crate,
   not in `atomic_solver`.
10. **Parallel/portfolio** — the m20 gap is an algorithmic/cost issue, not a
    throughput issue; already handled by the separate portfolio work.

---

## 11. Caveats

- **No PMU.** The container exposes no hardware counters, so instruction
  counts, cache misses, branch misses, and IPC are unavailable. All
  attribution is `cpu-clock` sampling with user-space symbols.
- **No pinning / shared host.** Wall times vary ~1–3%; the small 1–3%
  micro-optimization results should be re-measured with the project's
  multi-salt gate before landing.
- **Trajectory sensitivity.** Changing TT size, salt, or even a path cache can
  change the entire DF-PN tree. Fixed-budget evals are the only fair
  throughput comparator; completion-time comparisons need multiple salts and
  the established gate methodology.
- **Benchmark scope.** `examples/benchmark` excludes `Search::new` and runs an
  uncounted warm-up per case. Its `total_time` is a search-time metric, not a
  process-latency metric. The CLI measurements in §6 include allocation.
- **Preflight measurements** use a small sample of 3-man positions. They
  demonstrate the mechanism (fixed closure cost vs easy search) but are not a
  full 3-man corpus study.
- **No source changes were landed.** All variants in §7 are `/tmp` builds;
  the production tree remains at `b77255e`.

---

## 12. Reproduction recipes

All commands use the release binary. `FEN` placeholders are the FENs listed
in §2.3.

```bash
# Quick-suite baseline (59 cases, first-outcome)
target/release/examples/benchmark \
  --suite quick --timeout 3 --runs 1 --first-outcome --json \
  > /tmp/quick.json 2>/dev/null

# Deterministic hard-case comparison (child-eval budget, not wall)
target/release/atomic_solver --fen "$M22" --first-outcome --outcome-only \
  --budget 10000000

# TT allocation/startup cost
target/release/atomic_solver \
  --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1" \
  --first-outcome --outcome-only --tt-size 256

# 3-man preflight A/B
target/release/atomic_solver --fen "4k3/8/8/8/8/8/8/4KQ2 w - - 0 1" \
  --outcome-only --timeout 2
target/release/atomic_solver --fen "4k3/8/8/8/8/8/8/4KQ2 w - - 0 1" \
  --outcome-only --timeout 2 --no-preflight

# Profile (kernel symbols unavailable; user-space attribution is intact)
perf record -e cpu-clock -g -o /tmp/perf.data -- \
  target/release/atomic_solver --fen "$M22" --first-outcome \
  --outcome-only --budget 10000000
perf report -i /tmp/perf.data --stdio --no-children --no-call-graph \
  --percent-limit 0.3
```

A convenient Python wrapper used for the wall/RSS/evals table:

```python
import subprocess, time, resource
cmd = ["target/release/atomic_solver", "--fen", FEN,
       "--first-outcome", "--outcome-only", "--budget", "10000000"]
t0 = time.perf_counter()
p = subprocess.run(cmd, capture_output=True, text=True)
wall = time.perf_counter() - t0
ru = resource.getrusage(resource.RUSAGE_CHILDREN)
print(f"wall={wall:.4f}s rss={ru.ru_maxrss}KB")
print(p.stdout.strip().splitlines()[-1])
print(p.stderr.strip().splitlines()[-1])
```

---

## Appendix A — Raw quick-suite aggregate (default 256 MB, 3 s)

```
total_nodes      = 2,840,104
total_child_evals= 42,109,768
total_time       = 4.328 s (run 2; 4.211 s in run 1)
solved           = 59/59
timeouts         = 0
wrong            = 0
mean_pv_len      = 17.73
```

## Appendix B — Profile commands and sampling volume

- m22: 4K samples, ~1.21e9 cpu-clock event period units, run ~1.0 s.
- m20: ~7K samples over ~3 s.
- Flat reports used `perf report --stdio --no-children --no-call-graph`.
- Call-graph report used `perf report --stdio --percent-limit 1
  --call-graph callee`.
