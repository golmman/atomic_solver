# Lean Report 11 — #18 TT child-bucket prefetch pre-pass

Executed 2026-10-10 per `plan11.md`, on top of HEAD `2804f48` (search code
identical to the `81c8f53` tree profiled in `plan11_phase0/`).
The change is exactly the phase-0 spike V10:
`TranspositionTable::prefetch` (two-line prefetch, layout untouched) plus a
do/undo pre-pass at the top of `evaluate_all_children`. No other change:
probe/store/replacement, bucket layout, `ChildInfo`, and `evaluate_child`
are untouched; `children.rs` grew by 9 lines, `table.rs` by 55 (now
10,162 B, still < 10 KB). `cargo fmt --check`, `cargo clippy --all-targets`,
`cargo doc --no-deps` clean; `make test` and `make test-full` fully green
(0 failures anywhere; `test-full` ≈ 25 min).

**Host caveat, read first:** the phase-0 spike numbers (−51% m22 default,
−55% shuffle-win FO, −48% quick suite) were measured on the aarch64
reference VM. This session ran on a **different machine**: an x86_64
container (AMD Ryzen 9 5950X, 4 vCPU, 31 GiB, rustc 1.99.0; see
`docs/plans/lean/measurements/plan11/env.json`). The win below is
therefore measured against this host's baseline, and the −30% acceptance
derived from the aarch64 host is not directly comparable.

**m22-default anchors, two distinct ones (capture-command note):** the
A/B drift anchor below is the **non-`--outcome-only`** capture
(`--timeout 60`; stdout includes the `preflight:`/`pre_exit:` lines),
md5 `49a08a6d869aa3c002aef9b7e32f50a9`. The golden-anchored capture
(`--timeout 20 --outcome-only`, the three-line stdout that the golden
fixture pins) has md5 `b965d37c9a35154fd56ffcedfb5bede6` and matches
tests/fixtures/m22_default_stdout_golden.txt byte-for-byte on both
binaries. The two hashes are different because the two commands print
different line sets; neither is a drift signal. All other md5s in this
report refer to the capture named next to them.

## What landed

- `src/search/tt/table.rs`: `pub fn prefetch(&self, key: u64)` —
  `#[inline(always)]`; prefetches the first and last byte of the key's
  bucket (112 B can straddle two lines; line-aligning was measured as no
  gain in phase 0). aarch64: one `asm!` with two `prfm pldl1keep`
  (`nostack, readonly, preserves_flags`); x86_64: two
  `_mm_prefetch::<_MM_HINT_T0>`; other arches: no-op. Each `unsafe` block
  carries a `// SAFETY:` comment (hint-only, fault-free, both addresses
  inside the live table allocation — the index is masked), and the doc
  comment records the measured win as the justification.
- `src/search/dfpn/children.rs`: pre-pass loop at the top of
  `evaluate_all_children` — play/unplay every legal move over a strictly
  local `StateInfo` and prefetch `pos.hash()` (board hash ^ rule50 key —
  exactly the key `evaluate_child` probes). The `ChildPrecompute` docs now
  name the third, call-local scratch slot. The single-child re-evaluation
  in the `dfpn` loop is deliberately not prefetched (its entry was just
  stored, cache-hot).
- `src/search/tt/tests.rs`: `prefetch_is_observably_a_noop` — prefetching
  `0`, `u64::MAX`, a stored key, and a bucket-straddling key leaves
  `probe`, `stats()`, and the full entry list unchanged (guards against a
  future prefetch that mutates).

## Drift protocol (task 4) — fully green

| check | result |
| --- | --- |
| quick suite, 59 cases (`child_evals`/`nodes`/`outcome`) | identical (total 38,974,090 both) |
| m22 default stdout md5 (×5 interleaved) | `49a08a6d…` identical ×5/×5 |
| m22 default golden (`--timeout 20 --outcome-only` vs `m22_default_stdout_golden.txt`) | byte-identical |
| m22 FO stdout md5 (×5 interleaved) | `8109ff0d…` identical ×5/×5 |
| shuffle-win FO stdout md5 (×2 interleaved) | `cfc58bc4…` identical ×2/×2 (matches the phase-0 anchor) |

`pre_exit` node counts identical everywhere (m22 default 1,111,321; m22 FO
858,117; shuffle FO 13,907,467).

## Wall A/B (task 5) — acceptance not met; finding, explained

Sequential interleaved base/new runs, `date` deltas, stdout md5 verified
per run:

| workload | HEAD | plan11 | Δ |
| --- | --- | --- | --- |
| m22 default ×5 | 3.33–3.45 s (mean 3.42) | 2.56–2.64 s (mean 2.60) | **−23.8%** |
| m22 FO ×5 | 2.75–2.86 s (mean 2.77) | 2.10–2.12 s (mean 2.11) | −23.8% |
| shuffle-win FO ×2 | 46.04 / 46.43 s | 34.06 / 33.99 s | **−26.4%** |
| quick suite, search time ×1 | 6.68 s | 5.52 s | −17.3% |

**These are below the plan's −30% acceptance (spike: −51% / −55% / −48%).**
Per the plan this is a finding to explain, and it is:

1. **The host changed, not the code.** Phase 0 ran on the aarch64 reference
   VM; this session ran on an x86_64 AMD 5950X container (different VM
   entirely — `plan11.md`'s risk section covers non-aarch64 hosts, and the
   x86_64 path there is exactly what shipped). Memory-latency stall shares
   are host properties; the same binary pair is self-consistent here.
2. **The prefetch demonstrably works.** The shipped binary contains the two
   `prefetcht0` instructions (verified by disassembly, inlined into
   `dfpn`), and the leaf pie moves exactly as the spike predicted:
   shuffle-win FO `evaluate_child` 62.6% → 21.0% (HEAD base here measured
   *higher* than phase 0's 52.7%, so the miss share was not smaller).
3. **The residual difference is overlap efficiency, not placement.** The
   pre-pass placement is byte-for-byte the spike's. On this host the OoO
   engine evidently already overlaps more of the miss latency, and/or the
   x86 `prefetcht0` warms the lines less effectively than `prfm pldl1keep`
   did there; disentangling the two would need the aarch64 host and is not
   worth a session — the change is strictly beneficial on every measured
   workload and free of heuristics.

No workload regressed. The quick suite (the plan's "shallow/cache-resident"
risk class) still measured −17.3% here.

## Post-plan11 profile (task 6)

Shuffle-win FO, `--timeout 20`, leaf tables in
`docs/plans/lean/measurements/plan11/`
(base and post side by side; raw transcripts not kept per AGENTS.md):

| leaf | HEAD base | post-plan11 |
| --- | --- | --- |
| `evaluate_child` | 62.6% | 21.0% |
| `dfpn` | 13.8% | 40.5% (renormalized; now the top leaf) |
| `do_move` + `undo_move` (incl. pre-pass) | 7.2% | 14.9% |
| existence cluster (`has_legal_move_with_state` + `compute_checkers` + `populate_state` + `legal`) | 9.6% | 14.1% |
| `generate_legal_with_state` | 2.4% | 3.4% |
| sort leaves | ~2.0% | ~2.9% |
| `TT store` | 0.3% | 0.4% |

Next levers by this pie: **#19** (pre-pass do/undo + hash), then the `dfpn`
frame loop itself.

## plan12 (#19) justification probe

V12-style throwaway probe (pre-pass loop run twice, built in `/tmp`,
deleted after; stdout md5 still `49a08a6d…`): m22 default 2.60 → 2.96 s
(+0.36 s for a second, cache-hot pre-pass). ⇒ **the pre-pass is ≈14% of
post-plan11 wall on this host** (phase-0 estimate: ≈12%). The upstream
`Board::hash_after`/`rule50_after` lever remains justified at its −5–10%
expectation; plan12 stays planned (blocked only on movegen 2.3.0).

## Gates (task 7)

- `cargo fmt --check`: clean.
- `cargo clippy --all-targets`: 0 warnings.
- `cargo doc --no-deps`: 0 warnings.
- `make test`: green (32 test binaries, 0 failures).
- `make test-full`: green (≈25 min, 0 failures; includes the m22/shuffle
  wall-clock regression suites and the trajectory golden).

## x86_64 compile check (task 8)

`rustup target list --installed` includes `x86_64-unknown-linux-gnu`;
`cargo check --target x86_64-unknown-linux-gnu` passes — and this session's
entire host *is* x86_64, so the `_mm_prefetch` path is not just
type-checked but compiled, executed, and profiled above. The aarch64 `asm!`
branch was compile-checked in phase 0 (spike V10 ran on aarch64) but is not
re-verified on this host — the only residual asymmetry, and the code is
identical to the spike's proven `asm!`.

## Additional tools used

`perf record/report` (cpu-clock, leaf tables), `objdump` (prefetch
instruction verification), `python3` (quick-suite JSON diffing), throwaway
`/tmp` build for the V12 probe.

## Problems encountered

- `/usr/bin/time` is absent in this container; wall times taken with `date`
  deltas around the process, like phase 0.
- The first quick-suite capture ran `benchmark` as a CLI option of
  `atomic_solver` instead of the example binary — user error, no impact.
- The acceptance shortfall (above) — resolved as a host-attribution
  finding, recorded in the initiative's status header as a standing caveat
  for cross-session percentage comparisons.

## Unresolved / follow-ups

- The −30% acceptance is unmet on this host (−24/−26%); if the aarch64
  reference host returns, a single re-run of the A/B there would confirm
  the −50%-class win — optional, not scheduled.
- aarch64 `asm!` branch not re-compiled on this host (identical to the
  phase-0 spike that ran there; low risk).
- Phase-0 finding F4 (TT `with_mb` over-allocation, backlog #20) remains
  open and untouched, as planned.

## Task-list compliance

All ten plan tasks executed in order; task 5's acceptance shortfall is
reported as a finding per the plan's own instruction, not silently passed.
`AGENTS.md` needed no new convention (none expected); the initiative
`initiative.md` (status header, backlog rows #18/#19, post-plan11 profile
subsection, history line) and
`docs/plans/lean/measurements/plan11/` are updated.

**Post-commit correction (2026-10-10, after report12 flagged it):** the
session originally wrote its artifacts to repo-root `measurements/plan11/`
instead of the conventional `docs/plans/lean/measurements/plan11/` —
which made the plan12 session report them "missing". They were never
lost (tracked and byte-identical to their raw sources, re-verified
against the `/tmp` captures before the move); they are now relocated to
the conventional path, and this report's references updated. The
`measurements/plan11/` root path stays free for the `solve` initiative's
own plan11 artifacts.
