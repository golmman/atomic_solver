# Re-examination of the parallelism no-go record (2026-10-09)

Analysis note, not a plan: it re-reads the `parallel` verdicts (plan2,
lean plan7, plan4/4b, plan5/5a/5b, plan6, plan7) against the code and the
neighbouring initiatives' measurement data, and records what the record
missed. It commits to no execution; any follow-up is a new `planN.md` with
its own pre-registration. Evidence: `measurements/reexam/` (read-only
probes at `2081e8a`, zero product-code changes).

## Summary

The individual measurements in the record stand. The conclusion that the
solver is "not parallelizable" over-reaches in four places:

1. **Plan7 never tested a real portfolio** (three of four racers were the
   same computation; the fourth used the ε value already measured worst on
   both motivating cases). A re-run with a properly diversified ε portfolio
   **passes plan7's own H1/H2 gates** (§1).
2. **SPDFPN was measured in a TT-capacity-starved regime** that was never
   varied, and in an architecture that takes all the variance of a chaotic
   search but none of the min-selection (§2).
3. **The ~5 % TT-concurrency tax on the sequential path was avoidable** (§3).
4. **The gates measured median speedup on already-solving cases**, not the
   AGENTS.md priority-1 surface (decisive outcomes on deep positions) (§4).

Expected value stays modest on the 4-CPU reference host — roughly
1.1–1.6× on solved cases, with the larger upside in the timeout tail — not
the 2–8× the initiative was opened for.

## 1. Plan7 did not test a diversified portfolio

**Code fact.** Under `--first-outcome` the refinement loop never runs
(`while !self.first_outcome_only …`, `src/search/dfpn/mod.rs` ~l. 748),
and `refinement_round_cap()` — the only reader of `--refine-cap` — is
called only inside that loop (~l. 761). Plan7's racers P0/P1/P2 differed
only in `--refine-cap` and ran `--first-outcome`, so they executed the
identical trajectory; their medians (52.22 / 52.19 / 52.54 s on
shuffle-win, 2.963 / 2.973 / 2.971 s on m22) confirm it. report7's remark
that "P1/P2 PVs differ from P0's" is inconsistent with the code path.

**Data fact.** The fourth racer used ε = 0.25. The research plan4 global-ε
sweep (`research/measurements/plan4/plan4_derived.csv`, first-outcome
child_evals; "stress" = shuffle-win, identical default 249,480,478) had
already measured 0.25 as the worst choice on both motivating cases:
+39.2 % stress, +49.4 % m22. Plan7's effective portfolio was therefore
{0.125, 0.25}, which the existing data predicts to be worthless on exactly
those cases (`vbs_epsilon.py`):

| portfolio (ε) | stress | m22 | dec13 | dec10 |
| --- | --- | --- | --- | --- |
| {0.125, 0.25} — plan7 effective | 1.00 | 1.00 | 0.93 | 0.73 |
| {0.125, 0.5} | 0.63 | 1.00 | 1.00 | 0.77 |
| {0.125, 0.375, 0.5} | 0.63 | 0.87 | 1.00 | 0.77 |
| {0.125, 0.375, 0.5, 1.0} | 0.63 | 0.87 | 1.00 | 0.56 |

(ratio of per-case min child_evals to the default; wall tracks child_evals
across ε — plan7 P3: wall 1.393× vs evals 1.392×.)

**The diagnosis was inverted.** report7 concluded that determinism per
(FEN, config) leaves nothing to race. Determinism is per config; *across*
configs the record documents trajectory chaos five times over — ε
(`research` report4: non-monotone, per-case), TT size
(`lean/research_tt_capacity.md`: 14–19 M evals above 128 MB, no trend),
history/killer constants (`lean` report10: per-case swings up to 5×,
"trajectory luck"), clock-budget solved-entry reuse (`dfpn` report10:
stress −37.7 % while m22 collapses 3.1 s → 120 s), and the SPDFPN
bimodality (plan6). Chaos across sound configurations is the precondition
for portfolio gains, not evidence against them.

**Re-measured** (`measurements/reexam/state/races.json`; one 4-way
concurrent race per case, ε ∈ {0.125, 0.375, 0.5, 1.0}, first-outcome,
TT 128 MB):

| case | default ε=0.125 (in race) | W_first | winner | ratio | plan6 SPDFPN t4 median, as ratio |
| --- | --- | --- | --- | --- | --- |
| shuffle-win | 50.42 s | 31.91 s | ε=0.5 | **0.63×** | 0.61× (cap-hit tail 2/8) |
| rem12 | 15.10 s | 11.33 s | ε=0.375 | **0.75×** | 0.83× |
| m22 | 2.85 s | 2.59 s | ε=0.375 | **0.91×** | 0.99× |

Against plan7's pre-registered gate: H1 (shuffle-win ≤ 0.75×) passes,
H2 (m22, rem12 ≤ 1.05×) passes, H3 holds (all decisive racers `win`; the
single non-decisive racer is shuffle-win ε=1.0, a cap-hit `draw`).
Shorter PVs come for free (shuffle-win 199/261 plies vs 477).

Caveats, stated rather than discounted:

- One race per case. Justified by determinism (plan7: ~1 % spread across
  8 rounds), but not the plan7 rep protocol.
- Selection bias: ε = 0.5 was chosen knowing its shuffle-win result.
  rem12 was not part of the ε sweep and is the only out-of-sample case.
- Cost is 4× CPU; ε = 1.0 contributed nothing here and should be replaced
  by a different diversity source.
- History/killer arms alone are weak diversity on the quick suite: the VBS
  estimate over lean plan10's 24 configs gives total child_evals ratios
  0.95 / 0.91 / 0.89 / 0.87 for K = 2 / 3 / 4 / 8 (random arm subsets,
  default always included) and 0.83 for all 24 (`vbs_history_arms.py`).

## 2. SPDFPN: capacity confound and architecture shape

- **TT size was never varied** in plan5/5b/6 (all at the 128 MB default).
  `with_mb` rounds 56 B entries to 4 M slots (2 M two-way buckets);
  shuffle-win alone visits 13.9 M nodes sequentially, and
  `lean/research_tt_capacity.md` measured 2.6× more work for m22 at 64 MB
  vs 128 MB. Four threads storing 2–6× more entries into the same table is
  a plausible driver of plan6's bad-trajectory tail (5.7–6.3× eval
  inflation). **Hypothesis**, cheap to test: re-land `b714ef6` + the W
  delta and run shuffle-win t4 at `--tt-size` 512 / 1024, with the
  sequential denominator re-measured at the same size (sequential work
  itself moves chaotically with TT size).
- **Coordinator-only finish is the worst shape for a chaotic search.**
  The landed architecture (report5a deviation 1) let only the coordinator
  prove the root, so helper perturbation of the TT produced the full
  variance with no min-selection. Plan7 had min-selection but (per §1)
  no variance. The combination — every worker can finish, take first — was
  never built.
- **Why the pure SPDFPN pool stalled (hypothesis).** Repetition-dependent
  results are never cached (first-player-loss GHI shortcut; suppressed
  entries), so TT-guided `TRYRUNJOB` descents are blind in
  repetition-heavy regions — consistent with report5a's "two nodes
  alternately re-dispatched with unchanged (pn, dn) forever". Hex, the
  paper's domain, is position-monotone and never hits this. Root-started
  workers that carry their own in-call path state (Kaneko AAAI-10, mined in
  `dfpn/research_parallel.md`, never implemented) would sidestep it.

## 3. The sequential tax was a design choice, not a property

Plan4/4b made the single `TranspositionTable` type concurrent, so the
sequential path paid ~5 % (+5.36 % m22 / +4.61 % shuffle-win). Two shapes
avoid that entirely:

- make `Search` generic over a TT backend so the sequential build
  monomorphizes to today's unsynchronized table (byte-identical and
  tax-free by construction); or
- do not share the TT at all (private per-worker TTs plus a small shared
  solved-facts table, §5 idea B).

Either removes owner condition 2 (the ≤ +7 % tax budget) from any future
pre-registration.

## 4. The gates measured the wrong surface for priority 1

Every campaign gated on median wall speedup for cases that already solve
in 3–58 s, and plan5b required ≥ 2.0× on m22, a 3 s solve. AGENTS.md
priority 1 is decisive outcomes on deep positions; the matching metric is
*timeouts converted at a fixed wall budget* on the thorough / move-order
tier (m20/m21/m22_black etc.). That tail is where portfolio diversity pays
most (on stress ε = 1.0 times out, on dec10 it is the best value). Plan6's
length trend (S4 1.01× → 1.20× → 1.65×, ρ = 1.0) also points away from
short cases.

## 5. Ideas, ranked

| | Idea | Code | Rationale / risk |
| --- | --- | --- | --- |
| A | **Diversified config portfolio**: racers drawn from levers closed for "wins on X, regresses Y" — ε 0.375/0.5, clock-budget solved-entry reuse (needs an opt-in flag), history arms, TT size | none (except the reuse flag) | A regression on some cases is irrelevant in a portfolio; the default racer covers them. Soundness inherited per process. |
| B | **Cooperative portfolio** (ManySAT analog): private TTs + append-only shared solved-facts table; one pure default racer never consumes it, so the worst case is the sequential wall | M | Solved entries are path-independent and already reused across paths within a run; keep the `best_move_repeats_path` guard; same root and run only (not the plan10 cross-solve store). Risk: extra solved facts can destabilize DF-PN (`dfpn` report10/11), hence the isolated P0. |
| C | **SPDFPN TT-size diagnostic** (§2), then an any-worker-can-finish variant | re-land | Cheap test of whether the plan6 no-go is a configuration artifact. |
| D | **Kaneko root threads** with virtual pn congestion | L | Avoids the TT-only descent blindness of §2. |
| E | **Memory-level parallelism** (deterministic, bit-identical): prefetch child TT buckets or re-lay out buckets | S | perf (m22): `evaluate_child` 28.9 % self, ~60 % of it on TT bucket loads (~17 % of runtime); slot 1 (+0x68) is always a second cache line (112 B buckets). Upper bound ~1.2×. A `lean` lever, not parallelism. |
| F | **Fringe**: per-leaf config portfolios for `solve`'s 100 %-cap-pinned frontier (diversity instead of budget escalation); concurrent refinement rounds at several bounds (PV, priority 2); config-rotating restarts on one core | varies | Heavy-tailed censoring is the textbook case for diversity over budget. |

**Stays closed:** deterministic sibling parallelism (lean plan7's 1.47×
ceiling analysis holds), process-per-child option C (plan2, structural
cross-child TT subsidy), and Čížek option D (no safe shared payload).

## Record corrections

- report7: P0/P1/P2 were identical computations under `--first-outcome`;
  the "P1/P2 PVs differ" remark and the "deterministic ⇒ no diversity"
  diagnosis do not hold (addendum appended to `report7.md`).
- plan7's NO-GO stands as the measured result *of that portfolio*; it is
  not evidence against diversified portfolios.
