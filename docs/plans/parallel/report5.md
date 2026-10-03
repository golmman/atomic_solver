# Report 5 — SPDFPN prototype, stage 2 (plan5/plan5a + plan5b): mechanism, campaign, NO-GO verdict, revert executed

Executes `parallel` backlog **#4, stage 2** per `plan5.md` (stage 2a) and
`plan5b.md` (stage 2b). This is the stage-2 report the backlog row intends:
the mechanism (plan5, reported in `report5a.md`), the pre-registered
measurement campaign (plan5b), the verdict against the GO bands, and the
owner's condition-1 decision executed.

**Verdict: NO-GO — the stage-2 mechanism is sound but not fast enough, and
condition 1 (revert-if-missed) has been executed.** The pre-registered GO
band (S4 ≥ 2.0× on both hard-class cases, I4 ≤ 2×, V = 0) fails on m22
twice over: median wall speedup at 4 threads **1.79×** (< 2.0×) and work
inflation **2.06×** (> 2×). Shuffle-win alone would have been a GO (2.68×,
1.41×, V = 0), but the bands require both cases. Outcome agreement was
perfect — **29/29 decisive runs `win`, zero panics, zero lock failures** —
so the soundness kill points never fired; the miss is purely mechanism
quality. Per condition 1, stage 1b + 2a were **reverted**: the working tree
is byte-identical to pre-plan4b (`git diff e18e53e` over src/examples/tests
= 0 lines), `make test` green, quick suite 59/59 `child_evals` identical,
snapshot sha `eaa5f2b9…`/195 B identical to the plan4 record. The ~5 %
sharded-TT tax is refunded with the revert.

## What stage 2 delivered (before the revert)

- **plan5a (mechanism)** — see `report5a.md` for the full record: opt-in
  `--threads N` (N > 1) over the sharded TT, with the declared architecture
  deviation (coordinator runs the unmodified sequential chunk loop; N−1
  SPDFPN helper workers pre-warm the shared TT: W-capped jobs, path-derived
  thresholds, virtual win/loss steering, zero-progress retirement,
  serialized ProofEvents, per-worker repetition caches). N = 1 drift zero;
  smoke sound at 1.42×/4 threads.
- **plan5b (this session)**:
  - **W sweep, then locked** (m22, t4, 3 reps per value): W ∈ {250, 1000,
    4000, 10000, 20000, 50000} — the pre-registered {250, 1000, 4000} plus
    the bracket of the landed 20000 per `report5a`'s next-steps. Best median
    wall **W = 10000** (1.769 s vs 5.578 s at 20000). Locked in
    `MAX_WORK_PER_JOB` with a doc comment citing the sweep; that constant
    (plus its comment) was the *only* plan5b code change. Notable: the
    plan5a coarse probe had picked 20000, which the proper sweep showed to
    be ~3× worse than 10000 at the median — sweeping before locking earned
    its keep.
  - **Campaign**: 5 interleaved rounds of N = 1, 2, 3, 4 per case (m22
    30 s cap; shuffle-win 100 s cap), N = 1 as the in-session denominator;
    ≥ 5 reps per (case, N); 40 runs total.
  - **N = 1 drift re-check** post-campaign (quick suite, stdout hashes,
    snapshot sha): all identical to the plan4/plan4b/plan5a records — the
    campaign did not perturb the sequential surface (condition 2 held).
  - **Revert execution** after the verdict (below).

## Campaign results (medians over 5 interleaved reps; W = 10000)

Sequential references measured in-session
(`logs/moveorder_seqref.json`, `--suite move-order --timeout 100`):
m22 = 3.296 s / 14 156 269 child_evals (identical to the standalone
pre-campaign benchmark run — deterministic); shuffle-win = 58.207 s /
249 480 478 child_evals.

| case | N | median wall | speedup | inflation | cap-hits | agreement |
| --- | --- | --- | --- | --- | --- | --- |
| m22 | 2 | 2.600 s | 1.27× | 1.59× | 0 | 5/5 `win` |
| m22 | 3 | 2.549 s | 1.29× | 2.25× | 0 | 5/5 `win` |
| m22 | **4** | 1.842 s | **1.79×** | **2.06×** | 0 | 5/5 `win` |
| shuffle-win | 2 | 30.672 s | 1.90× | 1.08× | 1 | 4/4 `win` |
| shuffle-win | 3 | 30.757 s | 1.89× | 1.57× | 1 | 4/4 `win` |
| shuffle-win | **4** | 21.734 s | **2.68×** | 1.41× | 0 | 5/5 `win` |

- **Speedup vs the unsharded pre-plan4b binary** ("tax-adjusted"
  denominator, reported for transparency per the plan): add back the
  plan4b tax (+5.36 % m22 / +4.61 % shuffle-win) → m22 S4 ≈ 1.89×,
  shuffle-win S4 ≈ 2.81×. m22 still misses the ≥ 2.0× band on this
  denominator too, so the verdict is denominator-robust.
- **Work split**: helper share of total evals at t4 is 0.73–0.74 on both
  cases (t2 ≈ 0.48) — helpers do real work and are not starved; the
  bottleneck is *quality* of the pre-warm (next point), not idling.
- **Inflation driver**: in roughly 1 of 5 runs the coordinator's trajectory
  is steered into a bad region by helper-stored entries (m22 t4 worst run:
  59.5 M total ≈ 4.2× sequential; shuffle-win t3 worst: 1311.9 M total with
  coordinator 478 M ≈ 8.5× its healthy 116–165 M median). This is the same
  pathology as the plan5a smoke outlier, now visible in the medians on the
  shorter case.
- **Cap-hits**: 2 of 30 parallel runs hit the wall cap (resource-cut
  `Draw`, rc = 0, documented timeout semantics — not false proofs). They do
  not affect the medians but set the practical tail risk at N = 2/3.
- **Simulated N ∈ {8, 16} (non-gating, log-linear fit S(N) = a + b·ln N on
  the three real points)**: m22 S8 ≈ 2.2×, S16 ≈ 2.7× (efficiency at 16
  ≈ 0.17); shuffle-win S8 ≈ 3.2×, S16 ≈ 4.0× (≈ 0.25). Extrapolations only —
  per condition 3 they gate nothing and are not wall measurements.

## Verdict against the pre-registered bands (not renegotiated)

- **GO** requires S4 ≥ 2.0× on *both* cases, I4 ≤ 2×, V = 0.
  - m22: S4 = 1.79× → miss; I4 = 2.06× → miss.
  - shuffle-win: S4 = 2.68×, I4 = 1.41× → meets GO on its own.
  - V = 0 (no decisive disagreement, no panic/lock anomaly in any log).
- **MARGINAL** requires the S4 miss with I4 ≤ 2× — m22's I4 = 2.06× > 2×
  fails that conjunct (and m22's I4 at N = 3 is 2.25×, so this is not a
  one-run fluke of the median; the paper's f_s ≤ 1.40 at *16* threads
  contextualizes how far the helper pool sits from the literature band).
- **NO-GO** fires on "I4 > 2×" regardless of speedup. → **NO-GO.**

The verdict is robust to reasonable perturbations (the tax-adjusted
denominator still misses; the N = 3 inflation exceeds 2× independently),
so it is reported as measured: the bands were pre-registered precisely so
this call is mechanical, and a 3 % overshoot on one metric does not justify
a renegotiation the protocol forbids.

## Revert-if-missed execution (condition 1)

1. All numbers recorded first (`state/campaign_raw.jsonl`,
   `campaign_summary.json`, `campaign.csv`, `sweep.csv`, raw stderr logs —
   the evidence survives the revert).
2. **Revert executed**: every code file touched by plan4b + plan5 restored
   to its pre-plan4b state (`git show e18e53e:<file>` content written back
   for the 18 modified files; `src/search/dfpn/parallel/{mod,jobs}.rs`,
   `src/search/tt/shard.rs`, `tests/test_parallel.rs` deleted; AGENTS.md
   architecture/file-size lines restored — preflight docs intact, parallel
   docs gone). No `git` state was modified (repo convention).
3. **Verification, all green**:
   - `git diff e18e53e -- src examples tests Cargo.toml Cargo.lock` = 0
     lines (byte-identical to the pre-plan4b code state; `e02fab1` == the
     code underneath, `e18e53e` is docs-only on top);
   - `make test` green (fast gate, zero failures); `cargo fmt --check` and
     `cargo clippy --all-targets` clean;
   - quick suite 59/59 `child_evals` pass vs `baseline_quick_pre5.json`
     (`state/quick_post_revert.json`);
   - snapshot dump sha256 `eaa5f2b9…`/195 B identical to the plan4 record;
   - the release binary rebuilt from the reverted tree.
4. The stage-2 mechanism survives in history (commit `b714ef6` + the
   plan5b W-constant delta on top) and in `measurements/plan4/` (stage 1b
   patch); a future re-land would start from those, not from scratch.

## Deviations from the pre-registered protocol (declared)

1. **Sweep set extended**: the plan pinned W ∈ {250, 1000, 4000} (written
   when W was expected to be 1000); plan5 landed W = 20000 with the sweep
   deferred, and `report5a`'s next-steps extended the sweep to bracket the
   landed value. Executed as {250, 1000, 4000, 10000, 20000, 50000} — a
   superset of the pre-registration, locked before the campaign, no
   post-campaign tuning.
2. **Sequential eval references via benchmark**: `child_evals` is not on
   the solver CLI's stdout, and the `[bounded_search]` chunk-line sum
   undercounts the decisive chunk, so the sequential references were
   measured with `benchmark --suite move-order --timeout 100 --json`
   (in-session, `--first-outcome`, same build). m22's value was
   cross-checked deterministic (two identical runs 14 156 269).
3. **Cap-hits counted separately from agreement**: the plan's V metric
   reads "every run's decisive outcome"; the two 100 s-cap timeout draws
   are resource-cut (documented `ExitReason::Timeout` semantics), reported
   as cap-hits rather than soundness violations. They are also reported
   explicitly so a stricter reading (any non-`win` = disagreement) is
   evaluable from the data: even under that reading the verdict stays
   NO-GO, only the *reason mix* changes.

## Tools / examples used

`cargo`/`make`, `benchmark` (quick + move-order suites, `--json`), the
solver CLI, `sha256sum`/`cmp`, and three Python drivers committed under
`measurements/plan5/` (`sweep_w.py`, `campaign.py`,
`analyze_campaign.py`). No new example binaries; no solver code changes
beyond the swept/locked `MAX_WORK_PER_JOB` constant (reverted with
everything else).

## Problems encountered

- **The t2/t3 pathology is median-visible on m22**: single bad-trajectory
  runs (coordinator 3–8× its healthy eval count) sit inside the 5-rep
  median. Mitigation levers identified in `report5a` (helper eviction
  pressure on coordinator entries, helper participation caps, steering
  helpers away from the coordinator's current chunk) were **not** tried —
  post-sweep mechanism changes mid-campaign would have invalidated the
  pre-registration, and the plan forbids chasing a band.
- The campaign cost ~45 min of the session (shuffle-win rounds are
  ~4 min each at the 100 s cap with a 56 s N = 1 reference per round) —
  sized as planned, but it caps how many (case, N) cells fit in one
  sitting; the two hard-class points and the pre-registered rep count
  fit exactly.
- The drift-hash protocol initially reproduced mismatched hashes because
  the recorded stdout surface includes `--outcome-only` (no `pre_exit:`
  line); re-derived from the plan4 log files and re-verified byte-identical.

## Unresolved parts / missing tests

- No torn-state/race stress test for the helper path under `make test-full`
  load was ever written (carried from `report5a`; moot after the revert).
- The per-phase work accounting that would separate helper pre-warm
  quality from coordinator perturbation was never built (candidate lever,
  below).
- The two cap-hit runs are uninvestigated beyond the counter summary
  (their raw stderr logs are preserved under `logs/`).

## Next steps

- **Initiative closure state (executed by this report + the initiative/
  README row updates)**: backlog #4 closed NO-GO; the condition-1 revert
  refunds the tax; the initiative's remaining space is measured out —
  option C (plan2, 0.27×), deterministic sibling parallelism (lean plan7,
  1.47–1.48×), and now SPDFPN shared-TT helpers (this report, ≤ 1.79×/2.68×
  at 4 threads) all have measured no-gos covering the whole parallelism
  space as scoped. Closure decision and any owner re-open are the owner's
  call, on this evidence.
- If the owner ever re-opens option A: the identified levers are (a) TT
  replacement scoring that discounts helper work (protect coordinator
  entries — the inflation driver), (b) helper participation caps (N−1
  helpers + forwarder oversubscribe 4 CPUs), (c) steering helpers away
  from the coordinator's current chunk, (d) per-phase work accounting as
  the diagnostic. The W = 10000 finding also suggests the plan5a coarse
  probe under-explored; a re-land should re-sweep W on both cases.
- `docs/plans/README.md` `parallel` row updated (close event) as part of
  this session's closure.
