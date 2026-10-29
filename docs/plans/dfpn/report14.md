# report14 — GHI-safe cut-context table, Phase 0: NO-GO

Plan: `plan14.md` (backlog item #7, "context-keyed threshold-cut reuse").
Session: execution only; the spike was temporary and has been reverted.

## Verdict

**NO-GO.** The mechanism is real and reproduces the plan-session probe
numbers *exactly* at the candidate shape, but the Phase 0 corpus breaks it:

1. the designated hard control `m20_white` burns **3.02×** the baseline child
   evals at the proposed shape (2,161,196,327 vs 715,390,942);
2. the 59-case quick suite has an **outcome flip**: `dec44`, a 102,615-eval
   `Loss` proof in the baseline, becomes a 5 s `Draw` (censored loss,
   48,644,568 evals burned at the cut);
3. the win is not robust to the product's own size knobs: the same table is
   harmless-to-helpful at 256 MB / 2²² slots, **10.2×** on `m22` at
   128 MB / 2²², and **5.24×** on `m22` at 256 MB / 2²¹.

Per the pre-registered gates (hard-class regression/censored-loss + quick
suite 59/59) the item closes as measured out. No productionization, no
`plan15.md`; backlog #7 is closed and this initiative has no open item again.

## What was built (temporary spike)

A per-run, direct-mapped **cut-context table** exactly as specified in the
plan's normative section:

- 32 B slot `{ key, context, cut_pn, cut_dn, pn, dn }` (validity encoded by
  `cut_pn == 0`, which is an impossible threshold, so the slot stays 32 B);
- index `mix(key ^ context.rotate_left(17)) & (slots - 1)`, full
  `(key, context)` equality required on probe;
- store at the `dfpn` frame exit **only** for the existing threshold-cut
  branch (`threshold_cut`), writing the frame-entry ancestor context hash
  (the plan9 repetition-cache context), the call thresholds `th_pn/th_dn`,
  and the unsolved bounds `store_pn/store_dn`; solved/terminal/suppressed-
  Draw stores never insert;
- probe after the TT solved-result check and the path-repetition return, but
  before the plan9 repetition-cache probe: skip if a live TT entry is solved;
  on `(key, context)` hit with `th_pn <= cut_pn && th_dn <= cut_dn` return
  `Draw`, restoring the stored `(pn, dn)` into the TT **only** when the TT
  has no entry for the key (so a live unsolved entry is kept, never
  overwritten);
- `restart_slice == u64::MAX` (`RestartSchedule::NEVER`) disables both the
  probe and the store, the documented behavior-preserving termination guard;
- temporary env gate `DFPN_CUTCTX=1`, `DFPN_CUTCTX_SLOTS=<log2>`,
  `DFPN_CUTCTX_STATS=1` (counters on drop).

Notable implementation choice: the plan-session probe build enlarged
`TtEntry` to 80 B and used capacity-matched labels (160 MB probe ≙ 128 MB
product; 320 MB probe ≙ 256 MB product). This spike keeps the product 56 B
`TtEntry` and a separate side table, so the product labels are used directly
and **main-table capacity is unchanged** (128 MB label = 2¹⁹ buckets; 256 MB
label = 2²⁰ buckets). At 256 MB this reproduces every probe number to the
digit (below), so the side-table shape is equivalent to the probe's
mechanism for the candidate configuration.

The spike diff and the new module are archived:
`measurements/plan14/spike_src.diff` + `measurements/plan14/cut_context.rs.spike`.
`src/` is byte-identical to `HEAD` after the revert.

## Current-HEAD baselines (salt 0, first-outcome, outcome-only)

| case | FEN | TT 128 MB | TT 256 MB |
| --- | --- | ---: | ---: |
| stress (`m21_white`) | `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21` | 305,404,068 | 287,872,755 |
| m20 | `4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R5RK w - - 4 20` | — | 715,390,942 |
| m22 | `4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22` | 19,111,111 | 15,023,911 |
| m23 | `4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23` | 9,553,618 | 11,621,376 |
| dec10 | `3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41` | 3,901,847 | 3,901,847 |
| dec06 | `3r2k1/8/p7/2P2P2/8/1P1p3P/P2K2r1/6R1 w - - 1 32` | 1,507,482 | 1,507,482 |
| dec13 | `r1bq1k1r/ppN4p/n1p1p3/3p1n1P/1b1P2P1/2P5/PP6/RNBQKB1R w KQ - 1 14` | 5,030,497 | 5,030,497 |
| dec02 | `8/1k1p4/1P2p3/p2P1P2/P7/6p1/6K1/8 b - - 0 27` | 138,124 | 138,124 |
| dec01 | `r5r1/5N1k/2p2p2/pp1p3p/3p4/2P1P3/P7/2bQ1R1K w - - 0 30` | censored (120 s, 1.80 B evals) | censored (120 s, 1.71 B evals) |

These are the only comparisons used in this plan. The archived plan12 corpus
numbers are confirmed stale at this HEAD (dec01 is censored here even at
128 MB; several plan12 rows no longer apply). `dec01` is the one censored
case: its eval count at the 120 s cut is wall-clock dependent and is not a
replay anchor.

Every non-censored cell was re-measured after the revert and reproduced
**exactly** (`state/matrix_post_revert.json` vs the spike build's
feature-off rows).

## Phase 0 matrix

Child evals to first outcome. `on` arms = `DFPN_CUTCTX=1`. Ratios are
against the same-case baseline at the same main-TT label. Candidate product
shape = 2²⁰ main buckets (the 224 MB label is exactly 2²⁰ buckets, so the
256 MB rows are the candidate rows) + 2²² context slots (128 MiB).

| case | base 128 | base 256 | on 128/2²¹ | on 128/2²² | on 256/2²¹ | on 256/2²² |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| stress | 305,404,068 | 287,872,755 | 213,908,981 (0.700×) | 153,354,828 (0.502×) | 225,177,074 (0.782×) | **188,937,258 (0.656×)** |
| m20 | — | 715,390,942 | — | — | — | **2,161,196,327 (3.021×)** |
| m22 | 19,111,111 | 15,023,911 | 16,352,854 (0.856×) | 195,399,921 (**10.224×**) | 78,702,115 (**5.238×**) | 15,647,876 (1.042×) |
| m23 | 9,553,618 | 11,621,376 | 9,021,522 (0.944×) | 9,103,068 (0.953×) | 8,882,321 (0.764×) | 9,626,650 (0.828×) |
| dec10 | 3,901,847 | 3,901,847 | 2,292,257 (0.587×) | 2,292,227 (0.587×) | 2,292,257 (0.587×) | 2,292,227 (0.587×) |
| dec06 | 1,507,482 | 1,507,482 | 1,374,808 (0.912×) | 1,374,808 (0.912×) | 1,374,808 (0.912×) | 1,374,808 (0.912×) |
| dec13 | 5,030,497 | 5,030,497 | 5,072,982 (1.008×) | 5,072,982 (1.008×) | 5,072,982 (1.008×) | 5,072,982 (1.008×) |
| dec02 | 138,124 | 138,124 | 143,794 (1.041×) | 143,794 (1.041×) | 143,794 (1.041×) | 143,794 (1.041×) |
| dec01 | censored | censored | censored (0.720×) | censored (0.704×) | censored (0.746×) | censored (0.727×) |

Concordance with the plan-session probes at the candidate shape:

| case | plan-session probe | this spike |
| --- | ---: | ---: |
| stress 256 | 188,937,258 (0.656×) | 188,937,258 (0.656×) |
| m22 256 | 15,647,876 (1.042×) | 15,647,876 (1.042×) |
| m23 256 | 9,626,650 (0.828×) | 9,626,650 (0.828×) |
| dec10 | 2,292,227 (0.587×) | 2,292,227 (0.587×) |
| dec06 | 1,374,808 (0.912×) | 1,374,808 (0.912×) |
| dec13 | 5,072,982 (1.008×) | 5,072,982 (1.008×) |
| dec02 | 143,794 (1.041×) | 143,794 (1.041×) |

The one place the plan-session matrix and this spike disagree is the
128 MB main label (`m22` 17,086,349 there vs 195,399,921 here; stress 169.7 M
there vs 153.4 M here). Everything at 256 MB matches, including the exact
`m22` and `stress` values. That asymmetry is itself part of the finding: the
mechanism's effect is basin-like, not monotone in table size.

### Cut-context counters (feature on)

`probes` / `hits` / `threshold_rejects` / `overwrites` / `fallback_stores`
(TT absent, cut-context bounds written back) / `saturated_cut` /
`saturated_bounds` (finite non-INF values above `u32::MAX`, see below).

| case | arm | probes | hits | rejects | overwrites | fallback | sat.cut | sat.bounds |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| stress | 256/2²² | 10,776,109 | 315,383 | 1,037,254 | 5,485,547 | 65,500 | 0 | 0 |
| m20 | 256/2²² | 515,087,525 | 407,041,810 | 18,738,746 | 58,036,685 | 858,763 | 0 | 0 |
| m22 | 256/2²² | 953,294 | 17,817 | 92,988 | 73,446 | 1 | 0 | 0 |
| m22 | 128/2²² | 76,337,256 | 64,096,954 | 1,362,659 | 375,434 | 404 | 667 | 858 |
| m22 | 256/2²¹ | 8,776,713 | 2,616,721 | 950,824 | 2,912,488 | 2,202 | 1,667 | 120,279 |
| m23 | 256/2²² | 553,934 | 1,439 | 46,591 | 27,511 | 0 | 0 | 0 |
| dec10 | 256/2²² | 517,617 | 313,113 | 49,202 | 968 | 0 | 0 | 0 |
| dec06 | 256/2²² | 132,331 | 13,991 | 21,972 | 1,031 | 0 | 0 | 0 |
| dec13 | 256/2²² | 219,354 | 154 | 17,322 | 3,246 | 0 | 0 | 0 |
| dec02 | 256/2²² | 19,552 | 2,082 | 3,390 | 27 | 0 | 0 | 0 |
| dec01 | 256/2²² | 84,213,653 | 13,174,721 | 16,144,240 | 46,931,508 | 697,224 | 0 | 0 |

The full 32-row counter set is in `state/matrix_spike.json`. Observations:

- The candidate arm's `m22` / `m23` / `dec06` / `dec13` / `dec02` hit counts
  are tiny and the eval counts are the plan-session values, i.e. the win is
  *not* a high-hit-rate effect on those cases. `stress` and `dec10` are the
  cases with substantial hit counts.
- The catastrophic arms are the high-hit, high-overwrite regimes:
  `m22` 128/2²² (64.1 M hits, 375 k overwrites) and `m20` (407 M hits),
  `dec01` (13–40 M hits, ~47 M overwrites). The skip becomes
  self-sustaining: a frame that would have made progress returns `Draw`
  instead, the parent re-evaluates, the same context is still live, and the
  skip repeats across restart slices (the table is cleared only in
  `begin_run`, never per slice).
- **Saturation defect (measured, not just theoretical).** The plan's
  encoding `min(x, u32::MAX)` with `u32::MAX → INF` turns any *finite*
  threshold/bound above 4.29 G into `INF`. DF-PN's threshold arithmetic
  routinely produces such values (`th.saturating_sub(pn) + child_pn` near a
  genuine `INF`), so this is not a corner: the bad `m22` arm saturated 667
  cut thresholds and 858 bounds. An inflated cut threshold makes
  `th_pn <= cut_pn` too permissive and can only produce a false *draw*
  (a completeness defect, never a false decisive result), but that is exactly
  the observed failure shape. A conservative variant (`INF` exact; finite
  values saturating *down* to `u32::MAX - 1`, so a stored finite threshold is
  never inflated into `INF`) was tested: it does not rescue the arm
  (`m22` 128/2²² → 1.28 B evals, censored at 100 s, with 143 M
  threshold-rejects), so the collapse is the basin, not only the encoding.

## Quick suite (`--suite quick --json --first-outcome`, 5 s)

Feature off vs `DFPN_CUTCTX=1 DFPN_CUTCTX_SLOTS=22` at the default 256 MB
main label.

- **59/59 outcomes? NO.** `dec44` flips `loss → draw` (baseline 102,615 evals
  to a 50-ply loss proof; on-arm 48,644,568 evals to the 5 s cut). Everything
  else keeps its outcome.
- 30/59 cases are child-eval identical; 28 change (below).
- Off aggregate: 42,109,768 evals, 3.98 s, 59 solved, 0 timeouts, 0 wrong.
- On aggregate: 86,428,485 evals, 8.85 s, 58 solved, 1 timeout, 0 wrong.

| case | off | on | off evals | on evals | ratio |
| --- | --- | --- | ---: | ---: | ---: |
| dec01 | win | win | 6,021,944 | 6,200,956 | 1.030 |
| dec02 | loss | loss | 138,124 | 143,794 | 1.041 |
| dec05 | win | win | 314,615 | 307,498 | 0.977 |
| dec06 | loss | loss | 1,507,482 | 1,374,808 | 0.912 |
| dec10 | win | win | 3,901,847 | 2,292,227 | 0.587 |
| dec12 | win | win | 303,191 | 303,058 | 1.000 |
| dec13 | win | win | 5,030,497 | 5,072,982 | 1.008 |
| dec14 | win | win | 2,443,935 | 2,449,910 | 1.002 |
| **dec15** | win | win | 1,077,420 | 2,072,795 | **1.924** |
| dec16 | win | win | 107,110 | 107,072 | 1.000 |
| dec17 | win | win | 539,273 | 538,458 | 0.998 |
| dec19 | win | win | 135,708 | 135,679 | 1.000 |
| dec21 | win | win | 243,842 | 220,517 | 0.904 |
| dec26 | win | win | 151,933 | 151,858 | 1.000 |
| dec28 | loss | loss | 11,665 | 6,995 | 0.600 |
| dec30 | win | win | 12,041 | 12,005 | 0.997 |
| dec31 | loss | loss | 510,426 | 226,610 | 0.444 |
| dec37 | loss | loss | 28,257 | 27,786 | 0.983 |
| dec38 | loss | loss | 1,169,070 | 1,122,961 | 0.961 |
| dec39 | win | win | 61,552 | 61,526 | 1.000 |
| dec40 | loss | loss | 852,716 | 852,366 | 1.000 |
| dec42 | loss | loss | 80,849 | 69,342 | 0.858 |
| dec43 | loss | loss | 1,506,959 | 501,983 | 0.333 |
| **dec44** | **loss** | **draw** | 102,615 | 48,644,568 | **474.0 (timeout)** |
| dec46 | loss | loss | 128,347 | 122,726 | 0.956 |
| m23_white | win | win | 11,621,376 | 9,626,650 | 0.828 |
| m23_black | loss | loss | 2,878,005 | 2,560,250 | 0.890 |
| m24_white | win | win | 406,737 | 404,665 | 0.995 |
| m24_black | loss | loss | 93,449 | 87,657 | 0.938 |

The suite shows the same split as the matrix: strong wins on some deep
cases (`dec43` 0.333×, `dec31` 0.444×, `dec28` 0.600×, `dec10` 0.587×),
near-neutral tails, and a small-solve regression set that includes a
censored baseline loss (`dec44`) and a near-2× (`dec15`).

## Soundness and termination evidence

The soundness contract is the plan's, restated for the record: the table
stores no `Outcome`, every hit returns `Draw` exactly like a threshold cut,
exact 64-bit ancestor-context equality is required (no cross-context reuse),
and the reuse direction is monotone (`th_pn <= cut_pn ∧ th_dn <= cut_dn`).
A hit can therefore only lose a win (completeness), never invent one.

- `DFPN_CUTCTX=1 DFPN_CUTCTX_SLOTS=22 cargo test --release --test
  test_repetition -- --include-ignored`: **3/3 pass** (cyclic rook never
  `Win`; reversible cycle stays draw; solve/reset/solve never claims a win).
  The feature is also structurally unavailable to the pre-phase, which runs
  first for detected roots.
- Unit test `cut_context_never_schedule_is_bit_identical_and_terminates`:
  a `RestartSchedule::NEVER` depth-3 search with the table enabled is
  bit-identical `(outcome, pv length, child_evals)` to feature-off — the
  `restart_slice == u64::MAX` guard makes the mechanism inert there. The test
  terminates on its own.
- Unit test `cut_context_liveness_on_a_quick_threshold_cut_case`: the dec02
  root produces hits > 0 and the outcome is unchanged (`Loss`).
- `make test` (fast gate) with the spike in the tree: green. `cargo fmt`
  clean; `cargo clippy --release --all-targets` clean.
- Identity: with `DFPN_CUTCTX` unset, the full 8-case × 2-label baseline
  reproduced every pre-spike value. After the revert every non-censored
  anchor reproduced exactly again (see `state/matrix_post_revert.json`).
- No `ProofEvent` interaction: a hit returns `Draw` before the frame's
  event/store sites, and `emit_proof_node` never emits for `Draw`; the
  fallback TT write is an ordinary non-terminal unsolved entry (the
  pre-probe invariant is preserved, `remaining_depth = max_depth`,
  `best_move = Move::NONE`). No snapshot/serialization path was touched.

The spike was still reverted, so none of this adds a product test surface.

## Memory / configuration decision

None, because this is a NO-GO. The measured candidate was exactly the plan's
proposal: main table at the 224 MB label (2²⁰ buckets, identical bucket count
to today's 256 MB default) plus 2²² × 32 B = 128 MiB of context slots, a
352 MB label total. The spike implementation confirms the accounting is
exactly that (no hidden per-entry growth; the main `TtEntry` stays 56 B), so
gate 6 ("no unexplained memory beyond the declared split") would have passed.
The 2²¹ rows show 64 MiB is *not* a sufficient lower bound in this
implementation either (`m22` 5.24× at 256 MB), matching the plan-session
finding that the slot count is load-bearing.

## Pre-registered gate evaluation

| # | gate | result |
| --- | --- | --- |
| 1 | stress first-outcome ≥ 10% better (candidate shape) | **PASS**: 0.656× (287,872,755 → 188,937,258) |
| 2 | m22 ≤ 1.05× baseline, not censored | **PASS at candidate shape**: 1.042×; **FAIL matrix-wide**: 10.224× at 128 MB/2²², 5.238× at 256 MB/2²¹ |
| 3 | hard class (stress/m20/m22/m23) no censored-loss and no ratio > 1.10; dec10/dec06 ≤ 1.05×; dec13/dec02 ≤ 1.10× | **FAIL**: `m20` 3.021× at the candidate shape (the other listed cases pass: stress 0.656×, m22 1.042×, m23 0.828×, dec10 0.587×, dec06 0.912×, dec13 1.008×, dec02 1.041×) |
| 4 | quick suite 59/59 outcomes unchanged, zero conflicts, cyclic rook green | **FAIL**: `dec44` loss → 5 s draw; cyclic rook green |
| 5 | liveness (hits > 0) + `RestartSchedule::NEVER` termination test | **PASS** (both unit-tested; counters in the matrix) |
| 6 | no unexplained memory beyond the declared split | **PASS** (measured exactly 128 MiB at 2²²; main table unchanged) |

Because gates 3 and 4 fail, the pre-registered conclusion is **NO-GO**, and
per `plan14.md` the item closes as measured out (no `plan15.md`).

## Findings / interpretation

1. **The plan-session probe under-sampled the hard class and the quick
   suite.** Its 8-case salt-0 table was reproduced exactly at the candidate
   shape, including the advertised 0.656× stress win, but it did not include
   `m20` (which the gate itself names) and it did not run the 59-case quick
   suite. Adding those flips the verdict: `m20` 3.02× and `dec44`
   censored-loss.
2. **The effect is basin-like, not a monotone memory/capacity trade-off.**
   At 2¹⁹ main buckets the same mechanism is 10.2× on `m22` at 2²² slots and
   0.856× at 2²¹; at 2²⁰ buckets it is 1.042× at 2²² and 5.238× at 2²¹.
   Small changes in aliasing/occupancy flip `m22` between good and
   catastrophic trajectories. A production default cannot rely on having
   landed in the good basin.
3. **The skip is self-sustaining across restarts.** The table survives
   `bounded_search` restart slices by design (cleared only in `begin_run`),
   so a context that returns `Draw` keeps returning `Draw` after the restart,
   while the parent's "unchanged bounds ⇒ explored" rule only protects within
   a slice. High-hit arms show this directly (407 M hits / 2.16 B evals on
   `m20`; 64 M hits / 195 M evals on `m22` 128/2²²). The `NEVER` guard does
   not address the shipped schedule, which is where the regression lives.
4. **The saturation direction the plan asked to check is a real,
   over-permissive path.** `u32::MAX → INF` inflates finite thresholds that
   DF-PN produces routinely; the bad `m22` arm saturated hundreds of them.
   Deflating instead removes that path but not the basin collapse, so it is
   not a fix by itself. (It remains a correctness/completeness hazard any
   future variant must not carry.)
5. **This is the plan10/plan11 shape of result again**: the mechanism is
   sound under the stated contract, gives a large win on the target
   (stress/dec10-class shuffle work), and destabilizes designated controls
   through the same threshold dynamics. The three-family surface (child
   `pn`/`dn`, threshold-cut context, GHI-safe transposition context) has now
   been probed at Phase 0 grade; within this architecture the surviving
   intersection does not clear the hard-class/quick-suite bars.

## Problems encountered / deviations

- The m20 control is named by gate 3 but was missing from the plan's task-4
  matrix list; it was measured anyway (`state/matrix_m20.json`, 256 MB /
  2²²) because the gate names it. It is the deciding hard-class failure.
- The plan-session probe's 128 MB arm numbers could not be reproduced
  (`m22` 17.09 M there vs 195.4 M here) while every 256 MB arm matches
  exactly. The likely cause is the probe build's 80 B `TtEntry` / label
  matching at the smaller label; the discrepancy is reported rather than
  chased further, because the candidate shape is the 2²⁰-bucket/2²²-slot
  configuration and that is reproduced bit-for-bit.
- `dec01` is censored at its 120 s timeout, so its eval count is
  wall-clock-dependent; it is used only for direction, never as an anchor.
- The quick suite reports per-case `child_evals` and outcomes, not the
  benchmark's `--salt`; salt 0 is the default and was used throughout.

## Artifacts and commands

Archived under `measurements/plan14/`:

- `plan14_matrix.py` — driver (case list, env-gated arms, JSON output).
- `env.json` — host/toolchain/flags, label ↔ bucket mapping, memory labels.
- `state/matrix_spike.json` — the full matrix incl. counters.
- `state/matrix_m20.json` — the m20 candidate-shape control.
- `state/matrix_post_revert.json` — post-revert baseline identity re-run.
- `state/quick_off.json`, `state/quick_on.json` — quick suite.
- `spike_src.diff`, `cut_context.rs.spike` — the reverted spike source.

Reproduction commands are in `measurements/plan14/README.md`. The spike was
removed with `git show HEAD:<file>` restores; `git diff --exit-code -- src/`
is clean and the stress-128 anchor is 305,404,068 again.

## Next steps

- Close backlog item #7 and return the `dfpn` initiative to dormant (no open
  items; the reopen triggers remain a new measured search-semantics
  diagnostic or a soundness regression).
- Do **not** draft `plan15.md`. Any future threshold-cut-context proposal
  must (a) bound skips per context (or cap total skips) so a live entry
  cannot re-trigger after a restart, (b) treat saturation as deflation for
  cut thresholds, and (c) demonstrate `m20`/`m22`/`dec44` safety before a
  corpus rollout. The plan-session probe is preserved as the evidence that
  the surface exists; this Phase 0 is the evidence that it is not safely
  extractable with the proposed shape.
