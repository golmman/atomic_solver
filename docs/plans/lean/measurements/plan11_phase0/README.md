# Plan 11 phase-0 measurements (plan session 2026-10-09, read-only probes)

Fresh profile + spike measurements that justify `plan11.md` (TT child
prefetch pre-pass) and `plan12.md` / `movegen/plan_hash_after.md`
(upstream `hash_after`). No `src/` change was made in the repo: every
spike was built in a throwaway copy under `/tmp/spike/<variant>` and
discarded.

## Environment

| key | value |
| --- | --- |
| host | container (reference host): Apple-silicon aarch64 Linux VM, 6 vCPU, 15 GiB, 4 KiB pages, THP `madvise` |
| rustc | 1.98.1 (48a229cea 2026-09-01) |
| git rev | `81c8f537353c7cc04fdc606ad872abb0e0d1c03a` (clean tree) |
| atomic-movegen | 2.2.0 (crates.io) |
| TT | default 128 MB (allocates 2 Mi buckets × 112 B = 224 MiB, see finding F4) |

## Files

| file | content |
| --- | --- |
| `shuffle_fo_base_leaves.txt` | `perf report --no-children` leaf table, HEAD binary, shuffle-win first-outcome, `--timeout 20` |
| `shuffle_fo_base_evaluate_child_hot_insns.txt` | `perf annotate` of `evaluate_child` (instructions > 0.4% of the symbol): the samples pile up on the loads of the TT bucket's two `valid` bytes (`+0x30`, `+0x68`) and the following instructions — the probe miss |
| `m22_default_base_leaves.txt` | leaf table, HEAD binary, m22_white default mode |
| `shuffle_fo_spike_leaves.txt` | leaf table, spike binary (V10 below), shuffle-win first-outcome, `--timeout 15` |
| `spike_prefetch_prepass.patch` | the V10 spike diff (reference sketch for plan11; aarch64-only prefetch) |
| `quick_base.json` / `quick_spike.json` | `benchmark --suite quick --first-outcome --timeout 3 --runs 1 --json`, HEAD vs V10: `child_evals`, `nodes`, `outcome` identical in 59/59 cases |

## Commands

```bash
M22="4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22"
SH="4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21"
$BIN --fen "$M22" --timeout 60 --outcome-only                 # m22 default mode
$BIN --fen "$SH"  --timeout 300 --first-outcome               # shuffle-win FO
perf record -e cpu-clock -F 2000 -o X.data -- $BIN --fen "$SH" --timeout 20 --first-outcome --outcome-only
```

All wall numbers are sequential (never concurrent) runs, `date` deltas
around the process; identity = md5 of stdout.

## Spike variants and results

| variant | change vs HEAD | m22 default wall (3 reps) | shuffle FO wall | stdout |
| --- | --- | --- | --- | --- |
| V0 base | none | 3.07 / 3.06 / 3.10 s | 45.44 s | reference |
| V1 thp | explicit `madvise(MADV_HUGEPAGE)` on the TT | 3.16 / 3.27 / 3.13 s | — | identical |
| V2 align | bucket `#[repr(align(128))]` (bucket count unchanged) | 3.11 / 3.13 / 3.16 s | — | identical |
| V3 thp_align | V1 + V2 | 3.06 / 3.07 / 3.05 s | — | identical |
| V4 thp_align_pf | V3 + `prfm` of the child bucket right after the child's `do_move` in `evaluate_child` | 2.93 / 2.95 / 3.09 s (also 3.11 / 2.89 / 2.90) | — | identical |
| V5 pf_all | V4 + **pre-pass in `evaluate_all_children`**: do/undo every move, prefetch its bucket | 1.45 / 1.45 / 1.48 s (also 1.50 / 1.52 / 1.47) | **20.66 s** | identical |
| V6 pf_next | V4 + prefetch only child i+1 while evaluating child i | 1.89 / 1.89 / 1.90 s | — | identical |
| V7 noldp | HEAD built with `-C llvm-args=-aarch64-enable-ldst-opt=false` | 2.97 / 3.01 / 2.95 s | — | identical |
| V8 native | HEAD built with `-C target-cpu=native` | 3.08 / 3.05 / 3.11 s | — | identical |
| V9 pf_lines | HEAD + pre-pass + in-`evaluate_child` prefetch, **no** layout change; prefetch both lines a 112 B bucket can straddle | 1.50 / 1.48 / 1.49 s | — | identical |
| **V10 pf_lines_nochild** | **HEAD + pre-pass only, two-line prefetch, no layout change** (the `.patch`) | 1.49 / 1.53 / 1.52 s (also 1.53 / 1.50 / 1.51) | **20.49 s** | identical |
| V11 pf_align_nochild | V5 minus the in-`evaluate_child` prefetch | 1.47 / 1.48 / 1.49 s | — | identical |
| V12 pf_double | V10 with the pre-pass loop run **twice** (cost probe) | 1.69 / 1.69 / 1.70 s | — | identical |

Quick suite (`benchmark` aggregates, search time only): V0 6.31 s → V10
3.29 s (−48%); `total_child_evals` 38,974,090 both; 59/59 solved, 0 wrong.

Shuffle-win: `pre_exit: reason=Complete outcome=win nodes=13907467` in
both V0 and V10 (stdout md5 `cfc58bc4…` both).

## Findings

- **F1 — the TT probe miss is the dominant cost.** HEAD shuffle-win FO:
  `evaluate_child` 52.7% self, ~85% of it on the bucket loads → ≈45% of
  wall is stalled on a serial DRAM miss per evaluated child (m22 default:
  ≈19%). The miss is latency-bound, not layout- or TLB-bound: glibc
  already backs the 224 MiB table with THP (`AnonHugePages` 229376 kB in
  V0), and V1/V2/V3 are within noise.
- **F2 — memory-level parallelism fixes it.** Issuing all child-bucket
  prefetches before the evaluation loop (V5/V9/V10/V11) halves wall on
  every workload measured (m22 default −51%, shuffle-win FO −55%, quick
  suite −48%), byte-identical. Prefetching one child ahead (V6) recovers
  only part of it; prefetching inside `evaluate_child` (V4) is too late
  (−3–5%). Bucket alignment and in-`evaluate_child` prefetch add nothing
  on top of the pre-pass (V9/V10/V11 within noise) — V10 is the minimal
  form.
- **F3 — the pre-pass costs ≈ 12% of the new wall.** V12 (second,
  cache-hot pre-pass) adds +0.18 s to V10's ~1.51 s on m22 default: the
  pre-pass do/undo round-trips are the next lever, addressable by an
  upstream `Board::hash_after` (plan12 / `movegen/plan_hash_after.md`).
- **F4 — TT sizing quirk (not planned).** `TranspositionTable::with_mb`
  rounds the *entry count* up to a power of two with 56-byte entries, so
  `--tt-size 128` allocates 224 MiB (1.75×). Fixing it changes capacity,
  i.e. search behavior; recorded in the lean backlog for a decision.
- **F5 — build flags are not a lever.** `target-cpu=native` is null;
  disabling LDP formation is −3% pre-prefetch (store-forwarding noise),
  too fragile to ship as a build flag.
- **F6 — preflight fixed cost (not planned).** A trivial 3-man root
  (`4k3/8/8/8/8/8/8/4R1K1 w - - 0 1`) takes 0.56 s with the pre-phase vs
  0.009 s with `--no-preflight`; irrelevant for deep searches, visible in
  small-position batch tooling.
- **Post-spike pie (V10, shuffle-win FO):** `do_move` 20.8% + `undo_move`
  9.9% (incl. the pre-pass), `evaluate_child` 16.9% (≈ half residual TT
  stall), `dfpn` 16.1%, `score_with_context` 8.5%, existence cluster
  (`has_legal_move_with_state` 5.9% + `compute_checkers` 3.8% +
  `populate_state` 3.3% + `legal` 2.5%) 15.5%, `generate_legal_with_state`
  4.5%, sort leaves ~4%. HEAD's 10% `compute_checkers` share was largely
  the shadow of the TT miss (it fell to 3.8%).
