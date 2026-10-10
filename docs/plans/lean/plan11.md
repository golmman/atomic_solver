# Lean Plan 11 — #18 TT child-bucket prefetch pre-pass (bit-identical)

Implements the single new backlog lever **#18 (TT child-bucket
prefetch)**, opened by the 2026-10-09 fresh profile (plan session; data
and spike provenance in `measurements/plan11_phase0/README.md`). **One
lever, no bundle.** The follow-up lever (#19, replace the pre-pass
do/undo by an upstream `Board::hash_after`) is `plan12.md` and depends
on a movegen release; it is deliberately not part of this plan.

Hard constraint: **bit-identical search**. A prefetch is an
architectural no-op (a cache hint) and the pre-pass only plays and
unplays moves on the existing scratch contract, so every TT probe, store,
ordering decision, and child-eval count must be unchanged. The full
drift protocol applies.

## Rationale (measured, 2026-10-09)

- HEAD profile, shuffle-win first-outcome: `evaluate_child` is 52.7% of
  samples, ~85% of them on the two TT-bucket loads of
  `self.tt.probe(child_key)` (`children.rs`, the non-terminal branch).
  ≈45% of wall is one **serial DRAM miss per evaluated child**: the
  probe's result is needed before the next child can start, so the misses
  never overlap. m22 default mode: ≈19%.
- The miss is latency-bound, not TLB- or layout-bound: glibc already
  backs the 224 MiB table with transparent huge pages, and 128-byte
  bucket alignment / explicit `madvise` are within noise (spikes V1–V3).
- Spike V10 (`measurements/plan11_phase0/spike_prefetch_prepass.patch`):
  before the evaluation loop of `evaluate_all_children`, play/unplay
  every move once and prefetch its child's bucket. All misses of a frame
  are then in flight concurrently.

| case | HEAD | spike V10 | Δ |
| --- | --- | --- | --- |
| m22_white default mode (3 reps) | 3.06–3.10 s | 1.49–1.53 s | −51% |
| shuffle-win first-outcome | 45.44 s | 20.49 s | −55% |
| quick suite, search time | 6.31 s | 3.29 s | −48% |

Stdout byte-identical on both live cases; quick suite `child_evals` /
`nodes` / `outcome` identical in 59/59 cases.

Measured non-levers (do **not** add them): 128-byte bucket alignment,
explicit `MADV_HUGEPAGE`, an extra prefetch inside `evaluate_child`
(V9/V11 vs V10 within noise), one-ahead prefetch (V6, only −38%),
`-C target-cpu=native`.

## Design (settled — implement exactly this)

### 1. `TranspositionTable::prefetch` (`src/search/tt/table.rs`)

```rust
/// Hint the CPU to start loading the bucket `key` maps to.
///
/// Pure performance hint: no observable effect on table contents or on
/// any probe/store result. Issues one prefetch for the first and one
/// for the last byte of the bucket, because a 112-byte bucket can
/// straddle two cache lines (the table is not line-aligned; aligning it
/// was measured as no gain, plan11 phase 0).
#[inline(always)]
pub fn prefetch(&self, key: u64) { ... }
```

- Address: `let bucket = &self.table[self.index(key)];` — the index is
  always in bounds (`mask`), so no `unsafe` is needed to form the
  pointer; `first = bucket as *const _ as *const u8`,
  `last = first.wrapping_add(size_of::<[TtEntry; 2]>() - 1)`.
- `#[cfg(target_arch = "aarch64")]`: one `core::arch::asm!` with two
  `prfm pldl1keep, [{}]`, `options(nostack, readonly, preserves_flags)`.
- `#[cfg(target_arch = "x86_64")]`: two
  `core::arch::x86_64::_mm_prefetch::<{ core::arch::x86_64::_MM_HINT_T0 }>(p as *const i8)`
  (wrap in `unsafe` only if the toolchain still requires it).
- Any other arch: no-op (`let _ = (first, last);`).
- `unsafe` justification (AGENTS.md "documented and guarded"): put a
  `// SAFETY:` comment on each block — a prefetch has no architectural
  side effect, never faults (even on an invalid address), and both
  addresses lie inside the live `table` allocation. Mention the measured
  win (−48–55% wall) in the doc comment so the unsafe has its recorded
  reason.
- Pub visibility: `pub` like `probe`/`store` (the table type is public).

### 2. Pre-pass in `evaluate_all_children` (`src/search/dfpn/children.rs`)

At the top of `evaluate_all_children`, after `children.clear()`, before
the existing loop:

```rust
// TT prefetch pre-pass (lean plan11): issue every child's bucket load
// before the first probe so the misses overlap instead of serializing.
// Pure performance: the do/undo pair is net-zero on `pos`, and the
// prefetch has no observable effect.
let mut prefetch_state = StateInfo::new();
for i in 0..moves.len() {
    let mv = moves[i];
    pos.do_move_with_scratch(mv, &mut prefetch_state);
    self.tt.prefetch(pos.hash());
    pos.undo_move_with_scratch(mv, &prefetch_state);
}
```

Soundness: `do_move_with_scratch`/`undo_move_with_scratch` strictly
paired over the same slot satisfy the documented scratch contract
(`position.rs`); the slot is a local, distinct from the frame slot, the
eval-scratch pool slot, and the undo stack. Update the "Eval-scratch
slots" paragraph of the `ChildPrecompute` docs with one sentence naming
this third, local pre-pass slot.

Key correctness detail: the prefetched key must be `pos.hash()` (board
hash ^ rule50 key) — exactly the key `evaluate_child` probes. Do not
prefetch for the single-child re-evaluation in the `dfpn` loop (its entry
was stored by the just-returned child frame and is cache-hot).

### 3. Nothing else

No change to probe/store/replacement, bucket layout, bucket count, the
`ChildInfo` path, or `evaluate_child`. `children.rs` grows by ~10 lines
(already justified >20 KB in its header; no new justification needed).
`table.rs` stays < 10 KB.

## Tasks

1. **Baseline capture (before any edit)** — build HEAD release and save
   under `measurements/plan11/`:
   - `benchmark --suite quick --json --first-outcome --timeout 3 --runs 1`
     → `quick_before.json`;
   - m22 first-outcome stdout
     (`--fen "$M22" --timeout 60 --first-outcome`) → md5;
   - m22 default-mode stdout (`--timeout 60`) → md5 (compare also with
     `tests/fixtures/m22_default_stdout_golden.txt` as the existing
     golden test does);
   - shuffle-win first-outcome stdout (`--timeout 300 --first-outcome`)
     → md5, plus wall;
   - keep the HEAD binary (copy it to `/tmp/plan11_base`) for the A/B.
2. **Implement** design §1 and §2.
3. **Unit test** (`src/search/tt/tests.rs`): `prefetch` on arbitrary keys
   (0, `u64::MAX`, a stored key) leaves `probe` results and `stats()`
   unchanged — guards against a future "prefetch" that mutates.
   Existing `children.rs` tests cover the pre-pass path
   (`evaluate_all_children` is exercised by every search test).
4. **Drift protocol** (all must be byte-/bit-identical vs task 1):
   quick-suite `child_evals`/`nodes`/`outcome` per case; m22 FO stdout;
   m22 default stdout; shuffle-win FO stdout.
5. **Wall A/B** (sequential, interleaved base/new, never concurrent):
   m22 default ×5, m22 FO ×5, shuffle-win FO ×2, quick-suite search time
   ×1. Acceptance: shuffle-win FO and m22 default each ≤ −30% wall
   (spike: −55% / −51%); anything less is a finding to explain (e.g. the
   pre-pass placement differs from the spike), not a silent pass.
6. **Profile** the new binary (shuffle-win FO, `--timeout 20`, leaf
   table) → `measurements/plan11/shuffle_fo_post_plan11_leaves.txt`;
   update the lean initiative's profile section with the re-ranked pie.
7. **Gates**: `cargo fmt --check`, `cargo clippy --all-targets`,
   `make test`, and — TT/search hot-path change — `make test-full`
   (AGENTS.md testing tiers). Known pre-existing slow-tier caveats are
   listed in `report8.md`/`initiative.md` history; report any failure
   with a HEAD-vs-new comparison before attributing it.
8. **x86_64 compile check**: `cargo check --target x86_64-unknown-linux-gnu`
   if the target is installed (`rustup target list --installed`);
   otherwise record "x86_64 path not compile-checked" as an unresolved
   item in the report.
9. **Docs**: lean `initiative.md` (backlog #18 → done, profile section,
   history line), AGENTS.md only if a new convention emerged (none
   expected), `measurements/plan11/README.md` provenance table.
10. **Write `report11.md`** (final task): results table, drift evidence,
    post-plan11 pie, unresolved items, and whether plan12 (#19) is still
    justified by the measured pre-pass share (phase-0 estimate: ≈12% of
    post-plan11 wall, V12 spike).

## Risks

- **Non-aarch64 hosts**: the win depends on the prefetch instruction;
  the no-op fallback keeps correctness but loses the win (and would add
  the pre-pass do/undo cost, ≈+12%). Hence the x86_64 path in §1.
- **Shallow/cache-resident workloads** (tiny TT, small trees): the
  pre-pass is pure overhead there. The quick suite (many small cases at
  the default TT) still measured −48%; if the report finds a
  regressing class, record it — do not add heuristics in this plan.
- **Hint semantics drift**: none possible; the drift protocol is the
  enforcement.
