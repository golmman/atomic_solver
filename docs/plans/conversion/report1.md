# Report: Monotone Draw Cache v2 — Plan 1 (Phase 0 No-Go)

## Summary

Implemented `docs/plans/conversion/plan1.md` through its Phase 0 sizing
spike; the go/no-go came out **no-go**, and per the plan all spike code was
reverted to the byte-identical plan9 state. The v2 reuse rule (board-only
`repetition_key` key, `(clock, ancestors, depth)` payload, reuse iff
probe clock ≥ stored clock and probe ancestors ⊇ stored ancestors) is
mechanically sound and cheap — but on the stress case the *extra* hit
surface beyond plan9's exact cache is almost empty: 267 v2-only hits in a
54-second, 250M-eval run. Net effect on the primary metric was **+0.1%
child evals** (bar: ≥5% improvement); default mode measured **+8.0%**
(traversal-order noise, slightly unfavorable). The monotonicity lemma
itself remains valid and is recorded in the backlog as a candidate
soundness argument for `dfpn` backlog #4.

## What the spike did

- `src/search/dfpn/repetition_cache.rs`: temporarily held both maps — the
  plan9 v1 map `(tt_key, context_hash) -> depth` and the v2 map
  `rep_key -> Vec<Entry { clock, ancestors, depth }>` (insertion-order
  scan; deterministic). Mode latched from `CONV1_SPIKE=1` at construction;
  default off = byte-identical plan9 behavior (v1 authoritative, v2 map
  never written).
- In v2 mode the v1 map was still maintained as a *shadow* so every v2 hit
  could be classified as exact-plan9 or v2-only.
- Probe site unchanged (after TT solved-result check, before `sort_moves`);
  store site unchanged (`suppress_draw` branch), storing additionally
  `pos.board().rule50()` and `path_stack[..len-1]` (sorted, deduplicated;
  `debug_assert_eq!(path_stack.last(), Some(&rep_key))` held throughout).
- Temporary counters: stores, probes, non-empty probes, v2 hits, v2-only
  hits, clock-delta histogram, stored-ancestor-set-size histogram, peak
  entries/bytes — printed from the cache's `Drop` when the spike env var
  was set; plus one `conv1_search: nodes/child_evals/first_outcome_evals`
  stderr line in `main.rs` (same temporary instrumentation methodology as
  report9).

Baseline verification: with the spike off, the stress first-outcome run
reproduced the inherited post-plan9 numbers **bit-for-bit**
(13,907,467 nodes / 249,480,478 child evals, 477-ply first line, 54.0 s)
— so every delta below is attributable to the v2 reuse rule alone.

## Measurements

### Stress case `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21` (128 MB TT, default ε)

| Mode | Metric | plan9 baseline | v2 spike | Δ |
|------|--------|----------------|----------|---|
| first-outcome | child evals (primary) | 249,480,478 | 249,728,076 | **+0.1% (bar: ≤ −5%)** |
| first-outcome | nodes | 13,907,467 | 13,969,002 | +0.4% |
| first-outcome | wall time | 54.0 s | 53.9 s | ~0 |
| first-outcome | first line | 477 plies | **285 plies** | shorter line at equal cost |
| default | child evals | 338,094,183 | 365,211,444 | **+8.0%** |
| default | nodes | 19,943,731 | 20,663,921 | +3.6% |
| default | wall time | 72.3 s | 79.7 s | +10% |
| default | pv_status | cap-cut | cap-cut | — |

The first-outcome first line moved 477 → 285 plies: the 7,884 v2 hits do
redirect DF-PN's traversal (and found a much shorter first line), but the
redirect does not reduce work — the decisive chunk's expansion dominates,
and the hit savings are noise against it.

### Spike counters (stress case)

| Counter | first-outcome | default |
|---------|---------------|---------|
| stores | 1,445 | 1,836 |
| probes (total) | 13,968,998 | 19,989,394 |
| non-empty probes (entry under same board) | 14,119 | 22,157 |
| v2 hits | 7,884 | 12,276 |
| **v2-only hits** (plan9 exact cache would miss) | **267** | **291** |
| peak entries | 1,445 | 1,836 |

### Histograms (stress case, hits; buckets: 0, 1–4, 5–9, 10–19, 20–49, 50–99, 100+)

Clock delta `probe_clock − stored_clock`:

- first-outcome: `[7651, 160, 72, 1, 0, 0, 0]`
- default: (recorded pattern identical in shape; ≥97% in bucket 0)

Stored ancestor-set size at hit:

- first-outcome: `[0, 0, 5, 4293, 3578, 8, 0]` (sets of 10–49 keys dominate)

### m22_white control `4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22`

Bit-identical to baseline with the spike on: 858,117 nodes /
14,156,269 evals / 3.1 s, win length 95 (baseline: 3.15 s). Its 164 v2
hits were all exact-plan9 hits (0 v2-only) — no regression, no
destabilization.

### Quick suite (`benchmark --suite quick --json --first-outcome`)

All 59 outcomes unchanged, `wrong=false` everywhere. 53/59 cases
bit-identical on nodes and child evals. Drift cases (all repetition-heavy,
consistent with the surface the plan changes):

| Case | Δ child evals | Note |
|------|---------------|------|
| dec10 | −650,186 (−15.3%) | first line 41 → 53 plies; replayed with `examples/replay`: legal, ends in defender Loss |
| dec15 | −1 | |
| dec31 | +13 | |
| dec43 | −7 | |
| dec44 | −3 | |
| m23_black | −243 | |

77 of the 59-case suite's search instances registered ≥1 v2-only hit.

## Why the lever is empty (diagnosis)

1. **Re-entries reproduce their context.** The deterministic DFS re-walks a
   board under the *same* (clock, ancestor set) it was proven under — that
   is exactly what plan9's exact key catches. The clock-delta histogram
   shows 97.1% of v2 hits at delta 0; the monotone rule's two relaxation
   directions (drifted clock, excursion-extended ancestor set) fire on
   2.9% of hits combined.
2. **The rare relaxations are cheap anyway.** The 267 v2-only hits per
   first-outcome run sit on shallow re-descents; even perfect reuse of
   them cannot move a 249M-eval metric by the 5% bar. The plan's Phase 0
   suspicion ("plan9's exact cache already catches nearly everything")
   was correct: 7,617 of 7,884 v2 hits (96.6%) were exact hits.
3. **Default mode even pays a little** (+8.0% evals): the redirected
   traversal changes what the TT holds entering the refinement rounds, and
   the noise landed slightly unfavorable. Not a defect — a reminder that
   context-drift reuse buys traversal-order variance, not work reduction,
   on this position class.

The −29% that plan9 measured came from intercepting the *deep, frequent*
exact-context re-proofs (31,620 re-proofs of 1,467 distinct positions).
v2 targets the residual churn — but the residual turns out to be thin and
shallow. The stress case's remaining ~250M evals live where plan9's
"Next Steps" already pointed: same node re-walked under genuinely
different *deep* proof structure, which neither exact nor monotone
context keys address.

## Go/no-go decision

- (a) stress ≥5% improvement: **failed** (+0.1% evals; v2-only surface
  thin, 267/run). Decisive on its own.
- (b) m22 control within noise: passed (bit-identical).
- (c) quick-suite outcomes unchanged: passed (59/59).

Per the plan: **no-go** — all spike code reverted; negative result
documented here; backlog re-ranked (backlog #2, candidate-guided outcome
search, becomes next; initiative.md updated).

## Revert verification

- Working tree byte-identical to the committed pre-plan1 state
  (`git status` clean after restoring `repetition_cache.rs`, `core.rs`,
  `main.rs` from HEAD).
- `CARGO_PROFILE_RELEASE_LTO=thin cargo test --release`: all 29 test
  binaries green (218 unit + fast integration, 0 failed).
- Stress first-outcome run reproduced `outcome: win length: 477` and the
  54.0 s baseline shape.
- `cargo fmt --check`, `cargo clippy --all-targets`, `cargo doc
  --no-deps`: clean.

## Tools/Examples Used

- `atomic_solver` CLI with `--timeout 120 --first-outcome --outcome-only`
  (stress, m22) and default mode (stress), each with/without
  `CONV1_SPIKE=1`.
- `benchmark --suite quick --json --first-outcome` (before/after drift).
- `examples/replay` to verify dec10's changed first line is legal and
  decisive.
- Temporary instrumentation (dual v1/v2 maps with shadow classification +
  `Drop` counter dump + one `conv1_search` line in `main.rs`) — **all
  reverted** after measuring.

## Problems Encountered

1. The `peak_bytes` counter in the spike multiplied the *last stored
   entry's* byte size across all entries, so its absolute value (0.9–3.2
   MB) overestimates; the honest payload estimate from peak entries and
   the set-size histogram is ~1,836 × (~24 B + ~25 keys × 8 B) ≈
   **0.4–0.5 MB** — two orders of magnitude below the `1 << 18` capacity
   bound, matching report9's finding. Moot after the revert.
2. The plan's Phase 0 checklist asked for a v2-only-hit classification
   without specifying the mechanism; the dual-map shadow approach solves
   it exactly (v2 hit + v1 key miss ⇒ v2-only) at the cost of one extra
   HashMap lookup per hit, which is spike-only.
3. No soundness issues surfaced: every spike-on outcome was one the
   plan9 search could have returned, per the lemma; the m22 control stayed
   at 3.1 s (no plan10-style threshold destabilization — as designed, hits
   return at frame entry and never fold into parent bounds).

## Unresolved Parts

- None for plan1: the plan's go-branch tasks (productionization, unit
  tests, dfpn context-change test, property test, capacity shrink,
  AGENTS.md update) are intentionally not executed — they are the
  no-go branch's non-actions. AGENTS.md still describes the plan9 cache,
  which remains the shipped behavior.
- The monotonicity lemma is now written down (plan1 §"The monotonicity
  lemma", normative) but unimplemented anywhere; it should be reused, not
  re-derived, when `dfpn` backlog #4 (bounded cross-path verification)
  is planned.

## Missing Tests

- None added: the no-go branch reverts everything, so no new behavior
  exists to test. The existing plan9 test surface
  (`repetition_cache_reused_within_a_run`, `test_repetition
  --include-ignored`) covers the unchanged shipped cache.

## Next Steps

- **Backlog #2 (candidate-guided outcome search)** is next per the
  re-ranking: an engine line oracle attacks the first-outcome phase's
  wrong-subtree exploration — the ~250M evals this plan measured as
  unreachable by cache semantics.
- `dfpn` backlog #4 (bounded cross-path verification of solved results)
  should adopt plan1's monotonicity lemma as the soundness skeleton for
  any cross-path reuse it proposes; coordinate, don't duplicate.
- If a future plan revisits context-drift reuse, the histograms above are
  the sizing argument *against* it on this position class; a candidate
  would need a position class where ancestor sets genuinely drift
  (e.g. root-excursion-heavy refinement rounds in default mode).
