# Lean Report 14 — #21 arm A: same-MB 4-slot TT bucket (shipped)

Executed 2026-10-11 per `plan14.md`, on top of HEAD `a7c3745`. Arm A is
implemented and **shipped** (behavior-changing by design; the drift
protocol is N/A; validation ran per the pinned gate v1.0 with the two
pre-negotiated renegotiations). `cargo fmt --check` and
`cargo clippy --all-targets` clean; `make test` green before, during
(after each task), and after the temporary-hook revert.

## Method

1. **Implementation (tasks 1–2).** `src/search/tt/table.rs`: bucket width
   2 → 4 (`Vec<[TtEntry; 4]>`, 224-byte buckets). `with_mb` entry-count
   formula unchanged, buckets = entries/4 → 128 MB = 2²² entries / 2²⁰
   buckets, same 224 MiB allocation ("same-MB" holds exactly). `probe`:
   linear scan, full-key verification unchanged. `insert_new`: empty/stale
   slots first (lowest index), else argmin of the unchanged
   `(live, solved, work, generation)` score over all four slots, ties
   resolved to the highest index (generalizes the shipped 2-slot tie rule).
   Entry format, salt formula (mask width only), snapshot format unchanged.
2. **Golden/anchor regeneration (task 3).** Two byte-exact artifacts of
   the 2-slot layout had to be re-baselined:
   - `tests/fixtures/m22_default_stdout_golden.txt` regenerated from the
     arm-A build (18,779,890 child evals, deterministic across 3 reruns;
     the slow `#[ignore]`d golden test updated and passing). The new PV is
     `pv_status: cap-cut` and its **PPV verification failed** (defender
     reply `e8c8` not refutable within the verify_ppv horizon) — documented
     in the test header as informational-PV behavior; the test asserts
     trajectory identity, not PPV validity. For contrast, the *old*
     golden PV still verifies under arm A (`is_ppv: true`), so the root
     proof is intact.
   - `tests/portfolio_smoke.rs` dec10 anchor: under arm A **all four
     salts settle at exactly 3,901,847 evals** with an identical 36-ply
     PV, so the race is salt-insensitive and the winner salt is a watcher
     scheduling artifact. The anchor now pins outcome/evals/PV, not the
     salt (plan8's salt-2/2,918,636 pin belonged to the old layout).
   - Prefetch variant measurement (below) pinned the 4-hint form; the
     2-hint loser deleted.
3. **Gate v1.0 rollout (task 4).** `measurements/plan14/driver.py`: 22
   corpus cases × salts {0..5} + a second salt-0 run per case (`s0r1`),
   156 runs, 3 workers. Baseline = the plan12 rollout (legitimate reuse:
   shipped HEAD is byte-identical to the plan12 code base). `parse.py`
   → `state/{runs,summary}.json`; `compare.py` (paired by salt) →
   `state/comparison.json`.
   - **Soundness (renegotiated):** invariant 1 = *within-build
     determinism*: all 22 salt-0 reruns reproduce stdout byte-for-byte and
     evals exactly. Invariant 2: zero decisive-outcome conflicts across
     132 grid runs. Invariant 3 (fixture expectations): all uncensored
     runs match.
   - **Lever verdict: adopt** per the pinned rule — stress = clean
     censored-win (candidate censored only at salt 2 where the baseline is
     also censored; solves s3/s4 where the baseline is censored; hard
     class, basins 5/3 — not low-diversity); no regression or
     censored-loss on any case.
   - **Quick-suite secondary read** (vs `plan12/quick_base.json`): 59/59
     outcomes identical, 52/59 eval counts identical, total wall 5.606 →
     4.165 s (−25.7%), total evals 38.97 M → 42.11 M (+8.1%).
4. **Mechanism measurement (tasks 5–6).** The plan13 hook (reconstructed
   from `report13.md` "Method": env-gated `ATOMIC_TTPROBE`, relaxed-atomic
   counters, 2²⁰-slot evicted-key shadow) re-applied on top of arm A with
   the temporary `examples/tt_retention.rs` driver; cells m19_fo / m20_fo
   / m22_fo (control) × salts {0,3} at 128 MB / 60 s, extended by salts
   {1,4} on the stress cells per the gray-zone protocol. Hook and driver
   reverted afterwards: `git diff` over `src/search/tt/table.rs` is
   byte-identical to the pre-hook arm-A diff, hook files deleted, gate
   green again. The shipped tree carries only arm A (+ the portfolio
   driver fix, see problems).

## Prefetch variant micro-measurement (task 3, pinned)

| metric | 2-hint (shipped form) | 4-hint (pinned) |
| --- | --- | --- |
| m22 default wall ×3 (s) | 2.771 / 2.736 / 2.708 | **1.995 / 2.007 / 1.983** |
| m20_fo s0, evals in 60 s | 418,837,835 | **595,450,282** |

The 224-byte bucket can straddle four cache lines; covering all four
(offsets 0/64/128/192) beats first+last by −27% wall (m22 default) and
+42% work per 60 s on the saturated stress cell. Deleted the loser.

## Gate rollout: per-case numbers

| case | verdict | median ratio | ratios (<1 fraction) | cens cand/base | basins cand/base |
| --- | --- | --- | --- | --- | --- |
| stress * | **censored-win** | 1.237 (n=3) | 1.154/1.501/1.237 (0.00) | 1/3 | 5/3 |
| m20_white * | under-budgeted | — | — (no ratios) | 5/2 | 1/4 |
| m22_white * | unclear | 1.015 | 0.17 | 0/0 | 6/6 |
| m23_white * | unclear | 1.010 | 0.33 | 0/0 | 6/5 |
| dec01 | unclear | 1.054 | 0.33 | 0/0 | 1/6 |
| dec10 | unclear | 1.000 | 0.50 | 0/0 | 1/4 |
| dec13 | unclear | 0.9999 | 0.83 | 0/0 | 1/2 |
| dec02–dec09, dec11, dec12, dec14–dec18 | unclear | 1.000 | ≤0.50 | 0/0 | 1/1 or 1/3 |

\* = hard class. Full data: `state/comparison.json`.

Reading the stress censored-win honestly: the win is the *censoring
pattern*, not lower work — on the three salts where both arms solve, the
candidate does 15–50% **more** evals (median 1.237, 0/3 < 1). Arm A redrew
the stress basins; at the 2.5 B cap the candidate's draw distribution lands
in solvable basins on salts 3/4 where the baseline draw censors. That is a
real solve-rate improvement at the pinned budget (and the pre-registered
rule counts it), but it is basin-luck-shaped, not an efficiency win.

m20_white: candidate censored 5/6 at the 2 B cap (baseline 2/6) — the case
moved to the unresolved list per the pinned censoring rule and contributes
no verdict, but the raw signal (candidate solves 1/6 vs baseline 4/6)
points the same direction as the mechanism analysis below: arm A does not
help the most capacity-starved regime.

## Mechanism fork (task 6, pre-registered)

Per-cell `reprobe_after_eviction / probe_hits` vs plan13's
`state/results.json`:

| cell | arm A | plan13 | persistence | victim-live (arm A) |
| --- | --- | --- | --- | --- |
| m19_fo s0 | 14.18% | 17.40% | 81.5% | 80.4% |
| m19_fo s3 | 14.95% | 15.52% | 96.3% | 79.2% |
| m20_fo s0 | 17.36% | 18.75% | 92.6% | 80.7% |
| m20_fo s3 | 13.84% | 20.06% | 69.0% | 75.5% |
| m19_fo s1/s4 (gray-zone extension) | 15.50% / 14.67% | — | — | 80.6/79.7% |
| m20_fo s1/s4 (gray-zone extension) | 12.65% / 12.19% | — | — | 81.0/77.6% |
| m22_fo s0/s3 (control) | 0.06% / 0.05% | 0.44% / 0.58% | — | 0.16% / 0.28% |

- **Collision-tolerance-was-binding band (≤ 50% on all four): NO.** Not
  one cell is within a factor of two of the plan13 value.
- **Capacity-binding band (≥ 80% on all four): not cleanly.** Three of
  four are ≥ 80%; m20_s3 sits at 69.0%.
- **Gray-zone protocol executed** (salts 1/4 added): the arm-A stress
  fractions sit in a tight 12.2–17.4% band, i.e. the reduction vs plan13's
  15.5–20.1% is roughly 10–30% relative and salt-noisy, not a regime
  change.

**Fork outcome: gray zone, resolving to capacity/saturation as the binding
constraint.** The 2→4 comparison window absorbed only a minority of the
churn; the stress class remains saturated (~100% occupancy, victim-live
~75–80% of stores — slightly *worse* than plan13's ~70%, because four
contenders now share each bucket) and the re-probe mass is still 10–18 M
per stress run. A secondary observation supports the policy reading: the
re-probed keys' work advantage shrank sharply under arm A (re-probe work
mean 31.7–58.5 vs live-population mean 20.4–24.1, vs plan13's 72–87 vs
22–39) — the wider window does protect heavy entries better, but the
remaining loss is driven by there being nowhere to put entries, not by the
policy choosing badly. Arm B (layered TT / capacity family) stays open
with these numbers as its sizing (sketch below).

Note on hook overhead: with the hook enabled, stress cells do 7–23% fewer
evals per 60 s than the same arm-A build without it (e.g. m20_fo_s0
595.5 M → 462.8 M; the shadow lookup is one extra random cache touch per
probe). plan13 carried the same overhead on the 2-slot layout, so the
persistence comparison is apples-to-apples; the *absolute* fractions are
unaffected (they are ratios of counter totals).

## Host caveat

All numbers from the x86_64 container (AMD 5950X, 4 vCPU), not the
aarch64 reference host (standing caveat since plan11). The gate ratios,
counter fractions and eval counts are per-run deterministic quantities;
only wall timestamps are host-dependent. The prefetch measurement is the
one genuinely host-sensitive result (cache-line topology): the 4-hint win
was measured on x86_64 and the aarch64 prefetch path was updated symmetric
(`prfm` ×4) without re-measurement there.

## Raw-vs-parsed artifact pointers

- Committed: `measurements/plan14/env.json` (full provenance incl. FENs
  and commands), `measurements/plan14/README.md` (tables above),
  `driver.py` / `parse.py` / `compare.py`, `quick_arma.json`,
  `state/runs.json` (156 rollout runs), `state/summary.json`,
  `state/comparison.json` (verdicts + lever), `state/probe_results.json`
  (10 mechanism cells, every counter).
- Uncommitted (gitignored per the layout rules): `measurements/plan14/raw/`
  — 156 rollout transcripts + 10 probe transcripts (`.out`/`.err`; the
  wall data was parsed into `state/runs.json` before the `.meta.json`
  files were pruned to keep `git add .` clean). The hook and driver are
  intentionally not preserved as files; the hook's exact shape is
  `report13.md` "Method", reconstructed identically.

## Out-of-plan finding: portfolio driver race-end bug (fixed, stays shipped)

`tests/portfolio_smoke.rs` was flaky (~25% when its two tests run in
parallel): a budget-censored racer was misreported as `outcome=error`.
Root cause: the driver's event loop broke on `all_settled()` — a child can
exit while its pipes still hold unflushed lines, and the stderr reader
(`evals:`) wins the race against the stdout reader (`outcome: draw`,
`budget exhausted`), dropping the trailing lines. The fix makes the race
settle on channel disconnect (readers drained) instead of child exit, and
drops the now-unused `all_settled`; 20/20 reruns green after the fix, and
the flake had also surfaced once in a bare `make test` at HEAD before any
plan14 change. This is a driver-only fix in `examples/portfolio.rs`; it is
*not* part of the hook revert and remains in the tree. Also fixed en
passant: the test asserted `salt={DEC10_WINNER_SALT}` via a format-string
template that no longer matched after the anchor re-baseline.

## Problems encountered

- The transient gate failure described above (pre-existing, root-caused,
  fixed — recorded here because it interfered with the "gate green at
  HEAD" precondition).
- The anchor/golden regeneration was broader than the plan's explicit
  list: the plan pre-negotiated only the m22 golden, but the portfolio
  dec10 anchor is the same class of byte-exact artifact and failed
  deterministically. Both re-baselines are documented in their test
  headers; the anchor's salt pin was weakened to outcome/evals/PV because
  arm A made the race salt-insensitive (a strictly stronger fidelity
  property).
- The new m22 golden's cap-cut PV failed the `verify_ppv` check the plan4
  golden had passed. Diagnosed as informational-PV behavior (cap-cut =
  deterministic refine-cap policy, wall-time-invariant), not a soundness
  defect: the old proof line still verifies, and the search outcome is
  unchanged. Pinned with a header note rather than chasing a proven PV at
  a different refine cap (which would change the golden's command
  contract).
- Two hook-authoring slips, both caught by the forced-collision unit test
  before any measurement run: (1) the re-probe shadow check was initially
  placed only in the probe-*hit* branch — report13's bias note says it
  fires on hit *and* miss (the miss is the live loss); (2) the test
  snapshotted the counters once before the probes, reading stale values.
- `git add --dry-run` flagged the rollout `.meta.json` files as
  committable; pruned after parsing their wall data into
  `state/runs.json`.
- m20_white at the 2 B pre-registered cap: 5/6 candidate runs censored
  (> ⌈6/2⌉ = 3) — formally under-budgeted/unresolved. A post-hoc cap
  raise was NOT done (pre-registration discipline); the case is reported
  as unresolved with the raw signal stated.

## Next steps (arm-B sketch, from the measured re-probe mass)

Arm A's numbers size the remaining opportunity on the stress class
(m19/m20 at 128 MB / 60 s): re-probe mass 10.2–18.4 M per run (12.2–17.4%
of 70–133 M probe hits), re-probed work mean 32–59 vs population mean
20–24, occupancy ~100%, victim-live 75–81% of 12.9–21.4 M stores. Options
in increasing scope:

1. **Capacity first (cheapest, confound-free):** the occupancy is the
   shared driver of every stress-class signal (re-probe loss, victim-live,
   wall-per-eval). Backlog #20's 1.75× power-of-two rounding (128 MB buys
   224 MiB but only 2²² entries) and #16's default are the immediate
   levers: rounding *down* to a true 128 MiB entry count is memory-honest
   but behavior-changing; a capacity *bump* is the opposite direction of
   #20's memory-honesty goal — an owner decision, not a plan default.
2. **Layered TT (arm B proper, the owner's idea):** per-entry hit counter
   reset on eviction + promote to a small hot tier. Sizing: the tier only
   needs to hold the ~10–18 M re-probed keys' *working set* per run — at
   any moment the re-probe exposure is bounded by the shadow's footprint
   (2²⁰ slots captured ~10–18 M distinct evictions per run), so a 2²⁰-slot
   tier (56 MiB at 56 B entries) would cover the observed mass; a hit
   counter costs 4–8 B per entry (entry grows past 64 B — measure before
   committing to the layout) and makes `probe` mutate cache lines.
   Pre-register the same kill gate shape as plan13 (re-probe loss < 1% /
   victim-live < 5% on the stress cells before building anything beyond a
   prototype).
3. **Do nothing:** the adopt verdict's benefit is basin-luck-shaped; the
   efficiency counters (stress ratios > 1, m20 unresolved) say arm A is
   not a clear win on the regime it targeted. If #21 is closed now, the
   record stands: mechanism measured, window widening insufficient,
   capacity is the lever — reopen only with a capacity-family plan.

Recommended: option 1 as a decision item for the owner (it is #20/#16
territory, explicitly a plan14 non-goal), option 2 only if the owner wants
the retention lever pursued at gate grade.

## Additional tools used

`python3` (driver/parse/compare scripts, counter parsing), the temporary
`tt_retention` example and `probe_hook` module (both reverted), the
standard gate (`make test`, `cargo fmt --check`, `cargo clippy
--all-targets`), `verify_ppv` (golden PV check), `benchmark --suite quick
--json` (secondary read), `git add --dry-run` (layout hygiene). No perf
profiling this session.

## Unresolved parts / missing tests

- m20_white unresolved at the pre-registered 2 B cap (5/6 censored); a
  raised-cap reading of arm A on m20 does not exist.
- The mechanism fork's added salts (1/4) have no plan13 counterpart (the
  plan13 hook measured salts 0/3 only); they serve as the arm-A noise
  read, not a paired comparison.
- The aarch64 prefetch path (4-hint `prfm`) is unmeasured (no reference
  host in this container).
- The stress censored-win rests on redrawn basins; a larger-salt
  replication on the stress case alone (e.g. +6 salts) would harden it,
  at the cost of 2.5 B-eval runs.

SESSION COMPLETE
- Deliverables: arm A shipped in `src/search/tt/table.rs` (+ tests,
  prefetch re-tune, m22 golden + dec10 anchor re-baselined, tt_snapshot doc
  wording); `examples/portfolio` race-end fix; gate v1.0 rollout 156 runs
  (S1–S3 green, lever: adopt) + quick-suite read; mechanism measurement 10
  cells (hook reverted byte-identical, gate green); backlog #21 +
  `docs/plans/README.md` lean row updated; `measurements/plan14/` layout
  per AGENTS.md (drivers + state committed, raw gitignored);
  `report14.md` written. Gate: `make test` green, fmt/clippy clean.
Follow-up options:
  1. **Owner decision on #21's remaining lever** — capacity family
     (#20 rounding / #16 default) vs layered-TT arm B at gate grade vs
     closing #21 with the measured record; kickoff prompt: "Read
     `docs/plans/lean/report14.md` next steps and decide #21's
     disposition (capacity family / arm B / close); then plan15 per the
     decision."
  2. **Replicate the stress censored-win** before any adoption-further
     work: extend the stress case by +6 salts at the 2.5 B cap to test
     whether arm A's solve-rate win is basin-luck; kickoff prompt:
     "Extend `docs/plans/lean/measurements/plan14/` with a stress-only
     salt extension per report14's unresolved parts; report whether the
     censored-win replicates."
