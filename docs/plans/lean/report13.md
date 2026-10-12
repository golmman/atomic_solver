# Lean Report 13 — #21 TT retention probe: eviction churn measured before any retention machinery

Executed 2026-10-10 per `plan13.md`, on top of HEAD `39279c3` ("add lean
plan13"). Measurement-only session: a temporary, env-gated probe hook in the
TT plus a temporary driver example; both reverted to byte-identity after
measuring. `cargo fmt --check`, `cargo clippy --all-targets` clean;
`make test` green at HEAD before the hook (0 failures) and green again after
the revert. Pre-registered verdict: **K1 (kill) fails; T1 fires → plan14**.

## Method

- Hook (`src/search/tt/probe_hook.rs`, temporary): enabled only when
  `ATOMIC_TTPROBE=1` is set at TT construction (bool read once); when unset,
  the only hot-path cost is one predictable `Option` branch. Counters
  (relaxed atomics, exact, unsampled): `probe_hits`/`probe_misses`
  (current-generation probe outcomes), `store_total`,
  `store_overwrote_live` (victim `valid && generation == current`),
  victim split (`victim_live_solved`/`victim_live_unsolved`),
  `reprobe_after_eviction`, work sums (`live_work_sum`/`_count` sampled on
  new insertions, `evicted_live_work_sum`/`_count` at eviction,
  `reprobe_work_sum` over re-probed keys). Shadow of evicted keys:
  direct-mapped 2²⁰-slot key+work pair, encoding `key.wrapping_add(1)`
  (0 = empty; only `key == u64::MAX` collides with the sentinel — one lost
  shadow record, i.e. undercounting), slot index a salt-aware fixed mix of
  the key, so the shadow tracks each salt's bucket topology.
- Driver (`examples/tt_retention.rs`, temporary): FEN + `--tt-size` (128 MB
  default) + `--first-outcome` + `--salt` + `--timeout`; runs
  `Search::solve`, dumps run line + counter JSON to stderr. Production CLI
  untouched.
- One deterministic run per cell, fresh process per cell. Matrix: hard class
  = m22_white (FO and default), m20_white FO, dec10 FO, dec13 FO, m19_white
  FO (the stress-class fixture of `tests/stress.rs`), each at salt 0 and
  salt 3; shallow controls = dec01/dec03/m25_white/m29_white (FO, 5 s).
  128 MB throughout (the post-plan4 default).

## Artifact pointers

- Committed: `measurements/plan13/README.md` (per-cell table + gate
  verdicts), `measurements/plan13/env.json` (provenance incl. FENs and
  commands), `measurements/plan13/state/results.json` (parsed per-cell
  JSON, every counter).
- Uncommitted (gitignored per the layout rules): `measurements/plan13/raw/`
  — 16 per-cell transcripts (`*.out` stdout, `*.err` stderr with the run and
  `ttprobe:` lines). The drivers (hook + example) are intentionally *not*
  preserved as files; the hook's exact shape is described in plan13 task 2
  and reconstructed trivially from this report if ever needed.

## Results and gate verdicts (pre-registered)

| cell | outcome | child_evals | reprobe/hits | victim-live/stores |
| --- | --- | --- | --- | --- |
| m22_fo_s0 / s3 | Win 1.5 / 1.9 s | 14.2 / 17.8 M | 0.44% / 0.58% | 1.55% / 2.31% |
| m22_default_s0 / s3 | Win 2.1 / 2.7 s | 17.7 / 25.8 M | 0.95% / 0.88% | 1.53% / 1.22% |
| m20_fo_s0 / s3 | Draw (60 s timeout) | 546.5 / 507.0 M | **18.75% / 20.06%** | **71.34% / 70.58%** |
| dec10_fo_s0 / s3 | Win 0.44 / 0.46 s | 4.3 / 4.8 M | 0.03% / 0.02% | 0.00% / 0.00% |
| dec13_fo_s0 / s3 | Win 0.41 / 0.55 s | 3.8 / 5.0 M | 0.03% / 0.02% | 0.09% / 0.15% |
| m19_fo_s0 / s3 | Draw (60 s timeout) | 540.0 / 583.9 M | **17.40% / 15.52%** | **70.24% / 71.82%** |
| controls (dec01, dec03, m25, m29; salt 0) | Win | 47 K–5.7 M | ≤ 0.06% | ≤ 0.21% |

- **K1 (kill → close #21 no-go) FAILS.** It required re-probe < 1% and
  victim-live < 5% on *every* hard-class cell; m19 and m20 exceed both
  margins by an order of magnitude (denominators healthy: 93–96 M probe
  hits per stress cell).
- **T1 (fire → plan14) FIRES on both disjuncts:** re-probe ≥ 2% of hits on
  two hard-class cases (m19 *and* m20, each on both salts), and victim-live
  ≥ 20% of stores on the same four cells. Gray-zone protocol not needed —
  the split is wide (max solvable-cell re-probe 0.95% vs min stress-cell
  15.5%), no ambiguous cells.

## The victim-work analysis (does the incumbent score already protect what gets re-probed?)

No. Two independent readings:

1. **Re-probed keys carry above-mean work.** Per stress cell, mean work of
   re-probed keys at eviction time is 72–87 vs the stored-population mean
   22–39 (m19_fo_s0: re-probe mean 86.1 vs live mean 24.4). The keys the
   search comes back for were *already* the heavy subtrees — the `work`
   term scored them above average and they were still evicted. The cause is
   structural: bucket assignment is key-random, so `insert_new`'s work
   comparison only sees the two keys sharing a bucket; a high-work entry
   whose bucket-mates are even higher dies regardless of its global rank.
2. **Victim-live ≈ 70% of stores is a capacity artifact, not a policy
   failure.** At the measured 99.8% occupancy (4,187,827 live of 4,194,304
   slots on m19_fo_s0) *every* store that doesn't update an existing entry
   must evict, and every evicted entry is current-generation — `generation`
   is inert in production (`new_generation()` is never called, as plan13's
   context already noted). Victim s/u split: ~90–110 K solved vs ~19–22 M
   unsolved — the solved entries the score protects do survive; the churn
   is in unsolved live entries. So the policy-relevant signal is the
   re-probe loss (16–18 M re-probes per stress cell), not the victim-live
   rate; plan14's mechanism analysis should treat victim-live ≥ 20% as
   fired-but-artifactual and lean on the re-probe number.

Saturation explains the bimodal split: every *solvable* cell finishes with
the table ≤ ~17% occupied (m22_fo_s0: 703,469 live of 4.19 M slots) and
recovers almost nothing from any retention mechanism; the stress class
(540–584 M evals in 60 s, ~20× m22's work) refills the table ~20× over and
thrashes. That is exactly the deep-search regime output priority 1 targets.

## Known biases (all documented in plan13 / the hook header)

- Shadow collisions *hide* re-probes → losses undercounted → a kill reading
  is conservative; a fire reading is trustworthy. T1 fired anyway.
- A key evicted and re-stored before its next probe still counts as a
  re-probe (churn *exposure* — the lever's actual opportunity), so the
  metric overstates *loss* strictly speaking; the stress-class magnitudes
  are far too large for this to flip any verdict.
- The `u64::MAX` sentinel collision is a ~2⁻⁶⁴ undercount event.
- Counter mutation on the probe path is relaxed-atomic; exactness holds,
  and when the env gate is unset the fields are never touched.

## Revert verification (task 5)

`git diff` over `src/` and `examples/` is **empty** (byte-identical;
verified after `cargo fmt --check`), the temporary files
(`src/search/tt/probe_hook.rs`, `examples/tt_retention.rs`) are deleted,
and `make test` is green again (0 failures; clippy 0 warnings).

## Host caveat

All numbers are from the x86_64 container (AMD 5950X), not the aarch64
reference host (standing caveat since plan11). The measured quantities are
*fractions of deterministic counter totals* per (FEN, salt, TT size) —
child_evals, probe/store counts and their ratios are host-independent to
first order; only the wall timestamps in the run lines are host-dependent.
The regime split (solvable = low churn, saturated = high churn) is a
structural property of the TT capacity, not of the host. Cross-host
percentage-delta comparisons remain subject to the initiative's caveat.

## Additional tools used

`python3` (counter parsing → `state/results.json`, doc edits), the temporary
`tt_retention` example (the only build target beyond the default gate), and
the repo's standard gate (`make test`, clippy, fmt). No perf profiling this
session (the probe measures TT behavior, not CPU attribution).

## Problems encountered

- **Hook placement deviation (scope finding):** plan13 task 2 said "implement
  the hook in `src/search/tt/table.rs` (~60–80 lines)". `table.rs` is
  already 12 KB with a size justification; the hook went into a temporary
  sibling module `src/search/tt/probe_hook.rs` instead (~5.5 KB), wired by
  five small table.rs touch points. Semantics identical to the plan's
  design; the deviation also made the task-5 revert cleaner (one file
  deletion + small reversals). Recorded here, not silently expanded.
- **Two test-authoring misreads, caught by the tests themselves:** (1) the
  single-bucket eviction tie evicts slot **1** (`usize::from(score0 >=
  score1)`), not slot 0 — first test draft asserted the wrong victim;
  (2) the re-probe counter fires on both hit and miss probes of a
  shadowed key (the miss is the live loss, the hit is churn exposure) — the
  first hook draft only counted the hit branch, which the forced-collision
  test exposed immediately. Both fixed before any measurement run.
- **Revert mishap, caught before commit:** the scripted table.rs reversal
  dropped `self.insert_new(idx, new);` and its closing brace (an
  edit-script bug, not a logic issue) and left one hook initializer behind;
  `cargo fmt --check` failed loudly, both fixed, byte-identity then
  verified by empty `git diff`.
- m20_white FO does not solve within 60 s at 128 MB (it was known to be
  slower than m22; the capacity research only measured m22). The timeout
  cells are not failures — they are precisely the capacity-starved regime
  the probe exists to observe, and their counters are complete.

## Unresolved / missing tests

- The hook was unit-tested only while it existed (forced-collision shadow
  accounting, counter exactness, JSON shape); those tests were removed with
  the hook per the plan. The measurement itself is the artifact.
- `state/results.json` preserves every raw counter, so any future
  re-analysis (different ratios, per-cell occupancy) needs no re-run.

## Next steps

1. **plan14 (T1 follow-up, arm A first):** implement the same-MB 4-slot
   bucket — same total entries at 128 MB, four candidate victims per
   bucket, wider probe scan. Bit-identity is expected to break (eviction
   decisions change); validation per gate v1.0 on the move-order suite
   (plan19 template). The mechanism analysis must address the plan13
   finding: at saturation the binding constraint is *capacity*, not
   per-bucket collision tolerance — the 4-slot bucket improves victim
   selection locality but adds no entries, so if the rollout shows the
   stress-class re-probe loss persisting, that is the signal that arm B
   (small hot-entry tier, the owner's layered TT) or a capacity-family
   answer is the real lever.
2. If arm A is rejected by the gate, write the plan14 arm-B decision from
   its mechanism analysis (the plan13 numbers give it the re-probe mass and
   work distribution to target).
3. `docs/notes.md` stays untouched (the layered-TT idea is answered by
   measurement via this report, per plan13's non-goals).

## Task-list compliance

All seven plan tasks executed in order: preconditions checked (report12
exists; `make test` green at HEAD), hook implemented + unit-tested, driver
added + smoke-run on m22 FO (child_evals 14,156,269 — exactly the capacity
research's 128 MB anchor, an unplanned cross-check), matrix run (16 cells),
gates evaluated, hook and driver reverted to byte-identity with the gate
green again, backlog #21 + index updated, this report written.
