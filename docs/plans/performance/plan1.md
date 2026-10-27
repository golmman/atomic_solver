# Performance Plan 1: cheap DF-PN probe gate before the 3-man preflight

Initiative: `performance`, backlog **item 1** (report §10, "Preflight cost
gate / cheap-search probe"). This is the highest product-impact item in
[`docs/performance/performance_report1.md`](../../performance/performance_report1.md).

## Why this plan exists

The detector-gated pre-phase (`src/search/preflight/`) is a large win on
forced 3-man draws (KR/KN/KB vs K, cyclic-rook, adjacency-immunity): it
returns a sound Draw in ~0.28–0.60 s instead of the ordinary search timing
out at 20M+ child evals. But it pays a **fixed closure cost** on every
detected root, including positions the ordinary DF-PN search solves almost
immediately:

| position | preflight | `--no-preflight` | source |
|---|---:|---:|---|
| KQ vs K, Qf1 | 0.766 s / 5,827,506 evals | 0.011 s / 229 evals | report §9 |
| KQ vs K, Qd1 | 0.744 s / 5,827,510 | 0.012 s / 235 | report §9 |
| KQ vs K, Qa1 | 0.758 s / 5,827,518 | 0.011 s / 337 | report §9 |
| KQ vs K, Qd8 | 0.785 s / 5,827,466 | 0.011 s / 278 | report §9 |

The report's top recommendation is a **cheap bounded DF-PN probe first**: if
it proves the root, skip the closure. This plan implements that as a
screening gate, not a new claim source.

## Read-only calibration already performed (plan session, 2026-10-27)

Using the current release binary with the probe's intended configuration
(`--no-preflight --first-outcome --outcome-only --tt-size 4 --budget 2000`)
as a stand-in for the scratch probe:

| class | FEN | probe-like result |
|---|---|---|
| easy win | `4k3/8/8/8/8/8/8/4KQ2 w - - 0 1` | win, 189 evals |
| easy win | `4k3/8/8/8/8/8/8/3QK3 w - - 0 1` | win, 193 evals |
| easy win | `4k3/8/8/8/8/8/8/Q3K3 w - - 0 1` | win, 293 evals |
| easy win | `3Qk3/8/8/8/8/8/8/4K3 w - - 0 1` | win, 1 eval |
| hard win | `8/5K2/8/3k4/8/8/8/4Q3 w - - 6 4` (ladder step 3) | budget exhausted (~2,003) |
| hard win | `8/2K5/k7/8/8/8/8/4Q3 w - - 0 1` (ladder root) | budget exhausted (~2,002) |
| draw | `4k3/8/8/8/8/8/8/4KR2 w - - 0 1` (KR vs K) | budget exhausted |
| draw | `4k3/8/8/8/8/8/8/4KN2 w - - 0 1` (KN vs K) | budget exhausted |
| draw | `4k3/8/8/8/8/8/8/4KB2 w - - 0 1` (KB vs K) | budget exhausted |
| draw | `8/8/8/8/2k5/8/8/4KR2 w - - 0 1` (cyclic rook) | budget exhausted |
| draw | `8/8/8/8/8/1K6/1Q6/k7 b - - 0 1` (adjacency immunity) | budget exhausted |

A 2,000-eval cap separates the measured easy wins (max 293) from the hard
ladder root/step 3 (47,830 evals with 256 MB TT, >2,000 with the 4 MB probe
table) and every draw. The execution session must re-run this table and
extend it with at least the existing preflight fixtures before landing.

## Question and pre-registered decision rule

**Q:** Can a bounded DF-PN probe (scratch `Search`, scratch TT, fixed
child-eval cap) identify the easy-win class reliably enough to skip the
region closure, while leaving every draw, hard win, terminal root, and
non-gated position on its current path?

**GO** if all of the following hold on the Phase 0 corpus and the
post-implementation corpus:

1. Every easy-win corpus case is classified `probe-solved` and the ordinary
   search still returns the same decisive outcome.
2. Every draw corpus case still gets a `decided` preflight claim (Draw) with
   `region > 0`; no draw case becomes an ordinary-search timeout.
3. Every hard-win corpus case (ladder root, ladder step 3) still gets its
   replay-verified preflight claim with unchanged outcome, `rank`, and PV
   length.
4. Terminal roots still take the region path (`region == 1`), not the probe.
5. Easy-win wall is ≤ 50 ms median and at least 10× faster than the
   preflight baseline; draw-class wall is within +5%.
6. Extra child evals on the draw/hard classes are ≤ 0.1% of their closure
   cost (the probe cap ≤ 2,019 evals including the sweep overshoot).
7. The production TT is untouched by the probe (a deferred probe leaves
   `tt_stats` unchanged before the ordinary search runs).
8. `make test` and `make test-full` are green; the quick suite is
   eval-identical (there should be no 3-man case in it, but verify).

**NO-GO / defer** if no cap separates easy from hard/draw cases, or if any
draw/hard case changes outcome. The report's alternatives (material/distance
detector, closure cost reduction) are then the next plan; do not widen the
probe into a claim source in this plan.

## Scope

**In scope**

- A `cheap_probe` gate before the pre-phase in the `Search::solve` /
  `solve_with_progress` path only.
- A scratch `Search` + scratch TT for the probe (no production-TT
  interaction).
- Probe child-eval accounting and a new `PreflightReport` deferral reason,
  `probe-solved`.
- Documentation, tests, and the performance/CLI output contract updates.
- A small 3-man corpus measurement record under
  `docs/plans/performance/measurements/plan1/`.

**Out of scope (non-goals)**

- No change to the region closure, detector predicate, `REGION_BUDGET`,
  preflight soundness contract, or `--no-preflight` semantics.
- No change to DF-PN thresholds, move ordering, TT sizing/eviction, restart
  schedule, or refinement.
- No probe in `search_depth`, `search_depth_with_prefix`, reconstruct, or
  proofdb paths. Those paths keep today's preflight behavior exactly.
- No new CLI flag; the gate is automatic and disabled together with the
  pre-phase by `--no-preflight`.
- No claim/return of the probe's own outcome or PV. The probe is a *gate*;
  the ordinary search produces the product result.

## Normative design

### D-a. Constants

New module `src/search/dfpn/cheap_probe.rs` (keeps `run.rs` from growing
past its split threshold):

```rust
/// Child-eval cap for the pre-preflight screening probe. Calibrated to
/// separate the measured easy 3-man wins (max 293 evals) from the hard
/// ladder root/step 3 (>2,000 at this TT size). The probe is a gate, not a
/// claim source; a cap that is too high only costs a little extra work.
pub(super) const CHEAP_PROBE_EVAL_CAP: u64 = 2_000;
/// Scratch TT for the probe. 4 MB is enough for the cap and is dropped
/// immediately; it never aliases the production table.
pub(super) const CHEAP_PROBE_TT_MB: usize = 4;
/// Wall safety net; the child-eval cap is the deterministic bound.
pub(super) const CHEAP_PROBE_TIMEOUT_SECS: u64 = 1;
```

The execution session may lower the cap to 1,000 if the extended corpus
shows a hard/draw case inside 2,000; it may raise it to 5,000 only if an
easy-class case needs more, and only when every hard/draw case remains well
above the new cap. Record the chosen value and the separating margin in the
report.

### D-b. Gate conditions

The gate runs only when **all** hold:

- `self.preflight_enabled`;
- `preflight::detector_applies(pos.board())`;
- `pos.outcome().is_none()` (terminal roots stay on the region path; their
  closure is one position and needs no probe);
- the run has not timed out, hit the stop flag, or exhausted the
  child-eval budget;
- the remaining child-eval budget is non-zero.

`solve` and `solve_with_progress` call the gate after `begin_run` and before
`preflight_phase`. `search_depth` does not call it, so the certificate-bound
tests and reconstruct behavior are unchanged.

### D-c. Scratch probe

Build a scratch `Search::new(CHEAP_PROBE_TT_MB)` and configure it to match
the production run as closely as the scratch table allows:

- `set_preflight_enabled(false)` (no recursion);
- `set_first_outcome_only(true)` (stop at the first decisive outcome);
- `set_child_eval_budget(min(CHEAP_PROBE_EVAL_CAP, remaining))`;
- `set_timeout(CHEAP_PROBE_TIMEOUT_SECS)`;
- copy `epsilon_num`/`epsilon_den`, `scorer` (clone), `max_ply`, the restart
  schedule, the TT salt, and the stop flag from `self`.

Clone the root `Position` (`Position::clone` snapshots board+hash and starts
an empty undo stack). Call `probe.solve(&mut probe_pos)`.

`ProbeGate`:

```rust
pub(super) struct ProbeGate {
    /// The gate ran (detector/flag/budget conditions held).
    pub ran: bool,
    /// The probe returned a decisive Win/Loss within the cap.
    pub decided: bool,
    /// Child evals the probe consumed (0 when it did not run).
    pub evals: u64,
}
```

`decided = outcome != Outcome::Draw`. A Draw return is a resource cut or a
proven draw; either way the gate defers to the existing pre-phase. Do not
distinguish it here. The probe's PV is discarded.

### D-d. Accounting, report, and control flow

- Add `probe.evals` to `self.child_evals` immediately after the probe.
  The existing budget contract already allows a sweep overshoot (<256
  evals); the observed probe overshoot was ≤ 19 evals at cap 2,000.
- Extend `PreflightReport.evals` semantics: it is the total pre-phase work
  for the run, i.e. probe evals plus closure/certificate evals. Update the
  field doc and the module header.
- Add `probe-solved` to the documented `PreflightReason` list.
- `preflight_phase` gains a `prior_evals: u64` argument. Every report it
  constructs adds `prior_evals` to the closure/certificate evals. Callers:
  `search_depth` passes `0`; `solve_with_progress` passes `gate.evals`.
- If `gate.decided`, do **not** call `preflight_phase`; set
  `self.preflight_report = Some(PreflightReport::deferred("probe-solved",
  gate.evals, 0))` and let the ordinary search run. This preserves all
  existing `PvStatus` semantics: easy wins report the ordinary search's
  `FirstOutcome`/`ProvenShortest`/`Unproven`, not `PreflightProof`.
- If `gate.decided == false`, call `preflight_phase` as today (including
  when the gate did not run, with `prior_evals = 0`).
- The preflight soundness contract is unchanged: the production TT is never
  touched by the pre-phase, and the scratch probe's TT is dropped inside the
  gate. The probe emits no `ProofEvent`s because the scratch `Search` has
  no sender (the existing R2 gap is not widened).

### D-e. Why this is safe

- The probe's only effect on the production run is (a) consuming child evals
  and (b) suppressing the closure on roots where a bounded DF-PN search
  already found a decisive outcome. It cannot turn a Draw claim into
  something else: Draws always continue to the pre-phase.
- The probe is deterministic for a fixed `(FEN, config, cap)` and does not
  consult the wall clock except through its 1 s safety timeout.
- The ordinary search after a `probe-solved` is the same search that
  `--no-preflight` runs, which the report measured at ~10 ms on the easy
  class.

## Tasks

1. **Phase 0 corpus calibration (read-only first).** Re-run the table above
   with `--tt-size 4 --budget 2000`; add the existing preflight fixtures
   (terminal mate/stalemate/extinction, ladder root/step 3, adjacency draw,
   cyclic rook). Confirm the cap separates the classes; adjust within the
   pre-registered bounds if needed. Save the shell transcript/table under
   `docs/plans/performance/measurements/plan1/` (drivers + parsed table;
   no raw logs per the measurement layout).
2. **Add `src/search/dfpn/cheap_probe.rs`** with the constants,
   `ProbeGate`, and `impl Search::cheap_probe_gate`. Register
   `mod cheap_probe;` in `src/search/dfpn/mod.rs`.
3. **Wire the gate** in `src/search/dfpn/run.rs`: `solve_with_progress`
   calls it after `begin_run`, branches on `decided`, and passes
   `gate.evals` to `preflight_phase`; `preflight_phase` gains
   `prior_evals` and adds it to every report; `search_depth` passes `0`.
4. **Report/docs** in `src/search/preflight/mod.rs`: add `probe-solved`,
   state that `evals` includes the screening probe, and add one sentence to
   the module header that the gate's scratch TT is not the production TT
   and the pre-phase's own no-TT contract is untouched.
5. **CLI help/module header** in `src/main.rs`: `--no-preflight` disables
   the cheap-probe gate too; non-`--outcome-only` output may show
   `preflight: deferred reason=probe-solved evals=<n>`; easy wins then
   report the ordinary search's `pv_status`, not `preflight-proof`. Update
   `AGENTS.md`'s preflight bullet with one clause.
6. **Tests**:
   - fast unit test: easy KQ nonterminal root → `report.reason ==
     "probe-solved"`, outcome Win, `report.decided == false`,
     `report.evals > 0`, `child_evaluations() > report.evals` (probe +
     ordinary search).
   - fast unit test: hard/draw 3-man root still claims via preflight
     (`decided`, `region > 0`), with `report.evals` including the probe.
   - fast unit test: `--no-preflight`/`set_preflight_enabled(false)` runs no
     probe (report reason `disabled`).
   - fast unit test: terminal root still reports `region == 1` (probe
     skipped).
   - dfpn unit test: call the gate directly on a hard draw and assert
     `tt_stats()` is unchanged by a deferred probe.
   - keep `preflight_eval_budget_defers_then_budget_contract_holds` green
     (its `child_evaluations() == report.evals` must still hold because the
     probe evals are included in the report).
   - update `tests/test_preflight.rs` expectations only where a nonterminal
     easy-position status genuinely changes; do not weaken the full-closure
     draw/ladder tests.
7. **Measure and gate** (details below), then write `report1.md` and update
   `initiative.md` + the `docs/plans/README.md` `performance` row.

## Validation

1. `make test` (fast tier).
2. `make test-full` (this touches the search/preflight path; the slow
   full-closure tests are the point).
3. 3-man corpus A/B on the release binary: baseline (`HEAD`) vs after, at
   `--tt-size 256 --timeout 2` (and the report's `--first-outcome` where
   applicable). Record outcome, wall median over 5 runs, RSS, evals,
   `preflight:` line, `pv_status`.
   - Easy wins: after ≤ 50 ms median and ≥10× faster than baseline.
   - Draws: outcome Draw, preflight `decided`, wall within +5%.
   - Ladder root/step 3: outcome/rank/PV length unchanged; extra evals ≤
     0.1%.
4. `target/release/examples/benchmark --suite quick --first-outcome
   --timeout 3 --json` vs the report's baseline: per-case child evals
   identical, 59/59, no `wrong`.
5. `cargo fmt --check` and `cargo clippy --all-targets`.

**Falsification handling:** if a draw case loses its claim or an easy win
does not become `probe-solved`, STOP and root-cause before adjusting the
cap. Re-baselining a preflight test is not pre-authorized; only the
easy-position `pv_status` change is expected. Record any cap change with
the separating measurements.

## Report requirements

`docs/plans/performance/report1.md` must include: the calibrated cap and
margin; the full corpus table before/after; the easy-win latency
distribution; the draw/hard extra-eval cost; the budget/accounting evidence;
the TT-isolation test result; any test expectation changed and why; and the
GO/NO-GO verdict against the eight bars above. Update the initiative's
backlog row (#1 → done/closed with verdict) and the `docs/plans/README.md`
row only if the initiative state changes.
