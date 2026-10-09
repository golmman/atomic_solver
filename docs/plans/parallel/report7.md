# Report 7 — Process-level portfolio racing (sequential racers, take-first decisive)

Executes `docs/plans/parallel/plan7.md`. Verdict up front: **NO-GO per the
pre-registered gate — H1 (the motivating-case gate) failed; H2/H3 passed.**
Portfolio racing is measured out; the idea joins the initiative's no-go
record. Zero product-code changes were made at any point.

## Stage-0 drift gate — PASS

m22 stdout sha256 `b7c74f17…`, shuffle-win stdout sha256 `64129ef0…` — both
identical to the plan4/4b/5/5a/5b/6 record (`state/drift7_results.json`).
The release build was already up to date; no code was touched, so the
quick-suite byte-identity surface was not re-run (the plan pre-registered
only the two stdout hashes; noted in the drift record).

## Stage-1 driver

`portfolio7.py` (campaign6.py shape adapted): per round, one baseline
sequential default-config run alone on the machine, then one race — P0–P3
(`--refine-cap`/`--epsilon` portfolio frozen in plan7.md) launched
concurrently, wall per racer, `W_first` = first decisive finish. One JSON
line per run → `state/portfolio7_raw.jsonl`; solver stdout/stderr under
`logs/` (gitignored). `analyze_portfolio7.py` evaluates H1–H3 mechanically.

## Stage-2 pilot — clean

2 rounds, shuffle-win only: 4 concurrent processes launched and fit memory
(4 × 128 MB TT), all racers terminated ≤ cap (max 73 s vs 100 s cap), all
outcomes `win`, W_first extraction worked, no oversubscription artifacts.
One harness fix, recorded: the driver's round loop was case-major
(all rounds of a case, then the next case); fixed pre-campaign to the
plan-specified rounds-outer / cases-round-robin order. Racer configs, cases,
caps, rep count and the H1–H3 bands were not adjusted.

## Stage-3 campaign + Stage-4 analysis

8 rounds × 3 cases (pilot excluded). All numbers in-session, self-contained
baselines (this session's sequential walls are ~8% faster than plan6's on
shuffle-win — different day, same machine; the plan's design makes every
comparison internal).

| case | baseline median (max) | W_first median (max) | ratio | racer medians P0/P1/P2/P3 (s) | CPU core-s/race |
| --- | --- | --- | --- | --- | --- |
| m22 | 2.966 (3.021) | 2.943 (2.973) | 0.992× | 2.963 / 2.973 / 2.971 / 4.472 | 13.4 |
| rem12 | 15.684 (15.777) | 15.165 (15.468) | 0.967× | 15.535 / 15.489 / 15.662 / 15.165 | 62.0 |
| shuffle-win | 52.693 (53.046) | 51.944 (52.211) | **0.986×** | 52.218 / 52.189 / 52.543 / 72.758 | 229.9 |

Full distributions (`state/portfolio7_results.json`): W_first spans ~1% per
case — m22 2.902–2.973 s, rem12 14.952–15.468 s, shuffle-win 51.600–52.211 s.
Baselines equally tight.

## Hypothesis verdicts (mechanical, pre-registered)

- **H1 FAIL**: shuffle-win W_first median 0.986× baseline median (gate
  ≤ 0.75×). The max-conjunct (race max ≤ baseline max) passed trivially
  because both distributions are tight — there is no tail on either side.
- **H2 PASS**: m22 0.992×, rem12 0.967× (≤ 1.05×) — no regression, but the
  headroom H2 allowed was not converted into anything on the gated case.
- **H3 PASS**: 120/120 decisive runs all `win` (24 baselines + 96 racers),
  zero cap-hits among racers (counted separately, as in plan6), zero panics,
  zero non-zero return codes.

Per plan7.md's pre-registered failure mode: "if H1 fails, portfolio racing
is measured NO-GO on its motivating case → the idea joins the no-go record."

## The diagnosis (why H1 failed — the load-bearing finding)

Hypothesis H's premise was that plan6's shuffle-win t4 bimodality (5/8 runs
20–39 s, 3/8 at 90–100 s with 5.7–6.3× eval inflation) reflects a
path-dependent *sequential* trajectory that independent configurations
sample differently. The campaign falsifies the premise at the root:

1. **The solver is deterministic per (FEN, config).** A sequential run's
   trajectory is fixed; each racer's wall is a constant ± machine noise.
   The 8 races are 8 replays of the same race — hence the ~1% W_first
   spread and the near-total absence of portfolio benefit.
2. **The plan6 bimodality does not exist sequentially.** Across 40 racer
   runs + 16 baseline runs (4 configurations × 14 rounds on shuffle-win),
   *every* run landed within ~1% of the case median (~52 s). No run came
   near the 20–39 s "good tail" *or* the 90–100 s "bad tail". The bimodality
   was an artifact of the SPDFPN shared-TT mechanism (thread interleaving
   over one table steering work distribution), not a property of the
   position's search space that a portfolio could harvest.
3. **Config diversity changed trajectories but not wall time.** P1/P2
   produce different PVs than P0 (spot-checked), yet all three finish
   shuffle-win at ~52 s. `--epsilon 0.25` (P3) is uniformly ~1.4× slower on
   every case, consistent with `conversion` #7's won't-fix record — ε shifts
   are per-node cost moves here, not trajectory lottery tickets.

**Cost accounting**: a 4-racer race spends ~4.4× the CPU core-seconds of one
baseline run for −1.4% median wall (and by the determinism argument, it
could never do better than `min(t_P0..t_P3)` on *every* future run of the
same configs — the mechanism has zero adaptive power). Accepted at ≤ 4× CPU
only under hypothesis H; with H falsified, the trade is strictly negative.

## Deviations

- Stage-2 harness fix (loop order), recorded above; no frozen parameter moved.
- Stage-0: quick-suite byte-identity not re-run (plan pre-registered only the
  two stdout hashes; zero product-code changes make the rest of the surface
  moot).

## Problems encountered

None beyond the loop-order fix. Memory fit (4 × 128 MB), process launch,
polling-based per-racer wall timing, and cap semantics all behaved as
expected on the first try (pilot verified before the campaign).

## Unresolved parts / missing tests

None applicable — no product code changed, no tests added or removed. The
measurement answers the question it was registered to answer.

## Next steps

- Initiative row updated: `parallel` re-opened for plan7 → re-closed NO-GO
  (2026-10-05); `docs/plans/README.md` row updated accordingly.
- The documented-pattern follow-up ("ship a tiny `examples/portfolio`
  driver?") is **answered: do not ship** — the mechanism demonstrably cannot
  beat its own P0 racer, so an example would encode a measured dead end.
- Any future re-open of `parallel` now needs a lever that changes
  *trajectories* adaptively (e.g. a nondeterministic search mutation), not
  more measurement of deterministic portfolios: plan2 (0.27×), lean plan7
  (1.47×), SPDFPN plan5b/plan6 (≤ 1.65×), and plan7 (0.99×) exhaust the
  process/thread portfolio space end to end.

## Tools used

In-repo driver + analyzer only (`portfolio7.py`, `analyze_portfolio7.py`);
standard `sha256sum` for the drift gate. No external tooling.

## Addendum — re-examination (2026-10-09)

The measured numbers above stand; two interpretations do not
(`reexamination.md` §1, `measurements/reexam/`):

- **P0, P1 and P2 were the identical computation.** All racers ran
  `--first-outcome`, under which the refinement loop never executes
  (`src/search/dfpn/mod.rs`, `while !self.first_outcome_only …`), and
  `--refine-cap` is read only inside that loop. Their identical medians
  confirm it; the remark that "P1/P2 produce different PVs than P0" is
  inconsistent with the code path. The portfolio was effectively
  {ε 0.125, ε 0.25}, and the research plan4 ε sweep had already measured
  ε 0.25 as the worst value on both shuffle-win (+39.2 %) and m22 (+49.4 %).
- **"Deterministic ⇒ no diversity" is inverted.** Determinism holds per
  config; across sound configs the solver's trajectory is chaotic (ε, TT
  size, history constants, solved-entry reuse). A re-run with
  ε ∈ {0.125, 0.375, 0.5, 1.0} measured W_first = 0.63× (shuffle-win),
  0.75× (rem12) and 0.91× (m22) of the in-race default — passing this
  plan's H1 and H2 (single race per case; ε = 0.5 chosen with knowledge of
  its shuffle-win result, rem12 out of sample).

The NO-GO verdict remains the correct mechanical outcome *for the
portfolio this plan registered*; it is not evidence against diversified
portfolios.
