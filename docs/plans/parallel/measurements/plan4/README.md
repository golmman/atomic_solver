# plan4 measurements (2026-10-03) — inert TT-concurrency refactor: drift gate FAILED

Stage-1 sharded-TT refactor (`sharded_tt_attempt.patch`) vs the sequential
baseline. All runs on the release build, reference container (Apple
Silicon, 4 CPUs), default TT (128 MB), default ε (0.125), default
refine-cap (0.25). The refactor is **reverted** in the working tree
(clean `git diff`); this directory is the measured record.

**Verdict: the pre-registered drift gate failed on both wall points
(m22 +6.0 %, shuffle-win +4.4 % vs the ≤ +2 % threshold); deterministic
drift was zero everywhere. Backlog #4 stage 1 closed as NO-GO — see
`report4.md`.**

## Command table

| file | command |
| --- | --- |
| `state/baseline_quick_pre.json` | `benchmark --suite quick --json --first-outcome --runs 1` (pre-refactor baseline) |
| `state/post_quick.json` | same, post-refactor build |
| `logs/m22_pre_{1..5}.out` | `atomic_solver --fen '4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22' --timeout 30 --first-outcome --outcome-only` (pre; run 1 has stderr merged) |
| `logs/m22_post_{1..5}.out` | same, post-refactor |
| `logs/shuffle_post.out` | shuffle-win `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`, `--timeout 100 --first-outcome --outcome-only` (post; pre stdout byte-identical, not kept) |
| `logs/snap_pre.sha`, `logs/snap_pre.tt` | shallow case `--fen '4k3/8/8/8/8/8/8/4KRR1 w - - 0 1' --timeout 5 --first-outcome --tt-size 16 --tt-dump-path …`; sha256 `eaa5f2b97bded5e1050cd2e1eb3489186985015d525035ea52cc4918cef81fde`, 195 B |
| `sharded_tt_attempt.patch` | the complete, final sharded-TT diff (`git diff` at the point of the kill; revert-verified round-trip). Recovery: `git apply` + add `src/search/tt/shard.rs` from the patch-era tree — note the patch context does not include `shard.rs` itself (untracked); it is 100 lines implementing `Shard` + `layout()` exactly as specified in `plan4.md` step 2 |

Baseline comparison for the interleaved wall A/B used a scratch tree
(`/tmp`) with the identical working copy and the patch reverse-applied —
same compiler, same default flags, verified `strings`-clean of the
sharded build.

## Results

### Deterministic drift (the gate's identity surface): ZERO

- quick suite `child_evals`/`nodes`/`outcome`/`pv_len`: **bit-identical
  per case** across all 55 cases (`baseline_quick_pre.json` vs
  `post_quick.json`).
- m22 first-outcome stdout: **byte-identical** (3 golden lines; the
  `[bounded_search]` chunk lines are stderr).
- shuffle-win first-outcome stdout: **byte-identical**.
- snapshot dump bytes: **identical** (sha256 above, 195 B).
- `make test` green with the refactor in place (240 lib + all
  integration suites).

### Wall (the gate's cost surface): FAILED

Median first-outcome wall, interleaved pre/post pairs in one session
(10 pairs on m22, single pair on shuffle-win):

| case | pre median | post median | delta | threshold |
| --- | --- | --- | --- | --- |
| m22 first-outcome (30 s cap) | 2.995 s | 3.175 s | **+6.0 %** | ≤ +2 % |
| shuffle-win first-outcome (100 s cap) | 50.46 s | 52.67 s | **+4.4 %** | ≤ +2 % |

Quick-suite wall (single run per case, noisy): mixed ±2–10 % with a
positive skew on short cases — consistent with a small per-probe tax,
not with noise centered at zero.

### Profiling attribution (why the gate failed)

`perf record -e cpu-clock` on m22, pre vs post:

- The per-shard `RwLock` RMW pair itself (`__aarch64_cas4_acq` +
  `__aarch64_ldadd4_rel`) is only **0.7 %** of runtime.
- The cost is the lock discipline applied at *child-eval* granularity:
  `probe` runs once per child per DF-PN round (~15–20 probes/node on
  this class; ~15 M total on m22). Its cost went from ~invisible
  (inlined, SROA-able plain loads) to a ~16 % standalone symbol:
  forced 48 B `TtEntry` copies (the plan's pre-registered
  `Option<TtEntry>` return), non-forwardable bucket loads across the
  opaque atomic critical section, and clobber-induced re-loads in
  `evaluate_child`.
- Attempted mitigations, all wall-neutral (±1 %): `#[inline]` on
  `probe`; shrinking the body (unchecked shard/bucket indexing,
  `unwrap` — probe *did* inline, no speedup); `-Ctarget-cpu=native`
  (LSE single-instruction atomics; pre/post both unchanged). A
  lock-free-reader redesign (per-shard seqlock with atomic entry
  fields) was scoped and rejected: read-side fences still project
  ≈ +2–3 %, and version-bump retry storms under plan5's real
  multi-writer contention would make the *target* workload worse.

The plan's cost model ("uncontended futex-based RwLock read is a few
ns vs ~3 µs/node — well under noise") under-counted probe density by
~20×: the tax is per child-eval, not per node.

### Environment

Apple Silicon (Firestorm/Icestorm-M1, LSE atomics present), 4 CPUs,
release profile, default flags unless stated.
