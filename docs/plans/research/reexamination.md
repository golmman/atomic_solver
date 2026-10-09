# Re-examination of the structural-floor record (2026-10-09)

Analysis note, not a plan. It re-reads [`structural_floor.md`](structural_floor.md)
and the closures it consolidates (plus the later `conversion` ε legs) against
the code, and runs read-only probes to test their premises. It commits to no
execution; any follow-up is a new `planN.md` with its own pre-registration.
Evidence: [`measurements/reexam/`](measurements/reexam/README.md) (probes in an
isolated copy at `bb19e7b` with `probe.patch` applied; zero product-code
changes; instrumented baseline reproduces stress 249,480,478 and m22
14,156,269 child evals exactly; zero wrong outcomes in 76 runs + 177 suite
case-runs).

Case names follow the floor's conventions: *stress* = m21_white
(`4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`), m22 = m22_white,
dec13, dec10. All numbers are first-outcome `child_evals` on a 128 MB TT
unless stated. "Censored" = the run hit its child-eval budget (stress 2.5 B,
others 1 B) without a decisive outcome.

## Summary

The individual measurements in the record stand. Two of the floor's
load-bearing premises do not:

1. **The gates measured noise.** A semantics-neutral perturbation (remapping
   TT bucket indices) moves stress from 249.5 M to 758 M and to censored in
   three of four draws; the shipped baselines are the best or near-best of
   five draws on stress, m22 and dec13 (§1). Every closure of the form "wins
   on X, regresses control Y" was judged against one favourable realization.
2. **The work is node pricing, and leaf initialization was never tested.**
   80–88% of child evals on stress/m22/dec13 sit in frames that cut on their
   first sweep without ever recursing, and > 90% of those frames are fresh
   (no TT entry) (§2). Every unexpanded child is initialized to `(1, 1)`.
   The textbook mobility initialization moves stress by **0.24×** and dec13
   by **0.16×** — but sends m22 from 14.2 M to censored at 1 B under every
   salt (§3). The surface is real and load-bearing; the naive form is not
   shippable.

Several candidate levers are measured out by the same probes (§4). The
floor's best-first exclusion (§1/§4 there), the ordering refuter-rank result
(§5 there) and the no-simplification characterization (§8 there) are not
affected.

## 1. The baselines are favourable draws from a heavy-tailed distribution

`PROBE_SALT=s` replaces the TT bucket index `key & mask` with
`((key ^ s) · 0x9E3779B97F4A7C15 >> 17) & mask`. Full-key verification is
unchanged, so search semantics are identical; only which entries share a
bucket — and therefore the eviction pattern — moves.

| case | salt 0 (shipped) | salt 1 | salt 2 | salt 3 | salt 4 |
| --- | --- | --- | --- | --- | --- |
| stress | **249.5 M** | 758.2 M | censored | censored | censored |
| m22 | **14.16 M** | 23.65 M | 15.82 M | 17.83 M | 18.07 M |
| dec13 | **3.82 M** | 5.03 M | 5.04 M | 5.03 M | 5.03 M |
| dec10 | 4.26 M | 3.90 M | 2.92 M | 4.80 M | 3.90 M |

- On stress the spread is ≥ 10× and the distribution is heavy-tailed; the
  recorded baseline is the only draw near it.
- m22: the baseline is the minimum of five (median 17.8 M).
- dec13 is a different shape: the four salted draws agree within 0.2%, and
  a single-chunk run lands at 5.00 M too (R7). The 3.82 M baseline looks
  like one specific favourable collision pattern, not a sample from a wide
  spread.
- dec10 is the one control where the baseline sits mid-distribution.

**Reading (hypothesis, not proven).** Roughly sixty plans tuned constants and
accepted or rejected levers against the salt-0 trajectory, so the shipped
configuration is plausibly fitted to that one noise realization. Under that
reading any trajectory-changing lever will tend to look like a regression on
the controls (regression to the mean), independently of its merit. The
check is direct: re-score a closed lever as a distribution over salts.

**Consequence for the record.** Gate decisions of the form "wins on stress,
regresses a control" were single-draw comparisons in a regime where one
neutral draw moves the gate object by ≥ 3×. Affected closures include:

- ε = 0.375 / 0.5 (`report4.md` Phase 0; `conversion` #7);
- clock-budget solved-entry reuse, −37.7% on stress but m22 collapse
  (`dfpn` #2, plans 10/11);
- the `lean` plan10 history/killer arms (−11 to −20% quick totals, killed by
  per-case flips);
- TT eviction arm V2 (`report7.md`).

These are not shown to be wins — they are shown to be *unjudged*. The
record's own "trajectory chaos" findings (`report4.md`, `report8.md`, `lean`
`research_tt_capacity.md`) already described the regime; the gates were
never adapted to it.

## 2. Where the work is: first-sweep cuts of fresh nodes

Counters C1–C8 on the unmodified trajectory (`state/derived.json`,
`first_sweep_share`). A *first-sweep cut* is a frame whose threshold test
fires on the first loop iteration, right after `evaluate_all_children`,
before any child recursion; such frames never recurse, so the share is
additive.

| | stress | m22 | dec13 | dec10 |
| --- | --- | --- | --- | --- |
| first-sweep-cut share of all child evals | **83.4%** | **80.7%** | **87.5%** | 64.6% |
| … at OR / AND frames | 27.1% / 56.4% | 22.7% / 58.0% | 29.0% / 58.5% | 15.7% / 48.9% |
| frames among them with no TT entry ("fresh") | 91.1% | 92.3% | 98.7% | 16.4% |

Mechanism (code: `children.rs` `_ => (1, 1)` fallback; `core.rs` threshold
test): the parent enters a child believing it costs 1. The sweep reveals a
summed bound near the child's move count, the frame cuts immediately, and the
next tied sibling receives the same treatment. Most of the search is the
cost of discovering each node's branching factor by expanding it once.

This re-reads three earlier findings:

- `conversion/report6.md`'s exit-gap histogram (OR `dn` overshoot 16–63 in
  80.4% of cuts) is the branching factor, not a sweep-ordering artifact.
- "ε inert" (`epsilon_ceil(second) − best = 1` in 93.5% of AND / 81.0% of OR
  cuts) is what ε does on all-ones values: `ceil(x·(1+ε)) = x + 1` for small
  `x`. The ε closure legs (four in `structural_floor.md` §2, the fifth in
  `conversion` plan10) were all measured in the regime where ε cannot act.
- The partial-sum sweep short-circuit (`conversion` plan7) made stored bounds
  *weaker*; this surface asks for bounds that are *stronger* up front.

Why it was missed: initialization was blocked as "no heuristic component"
(`report5.md` family 1; §9 rows #6/#13). Mobility initialization needs no
evaluator — only move counts — and leaves soundness untouched (it steers;
solved values are computed exactly as before). The ordering oracle floor
(`lean` plan9) measures refuter *rank*, not bound *magnitude*, so it never
covered this surface.

## 3. Mobility initialization (naive form): large, polarized effect

`PROBE_INIT=mob`: an unsolved child with no reusable TT bound starts at
`(1, n)` when the attacker moves there and `(n, 1)` when the defender moves
there (`n` = legal moves), instead of `(1, 1)`.

| | baseline | mob | ratio |
| --- | --- | --- | --- |
| stress (salt 0 only) | 249.5 M | **58.7 M** | **0.24×** |
| dec13 (identical at salts 0–4) | 3.82 M | **0.61 M** | **0.16×** |
| dec10 (salts 0–4 within 12 evals) | 4.26 M | 7.82 M | 1.83× |
| m22 (all salts) | 14.16 M | censored at 1 B | > 70× |
| quick suite total (10 s/case) | 38.97 M, 59/0 | 89.97 M, 58/1 (dec01) | 2.31× |
| move-order suite total (30 s/case) | 1,036.9 M, 14/5 | 882.0 M, 14/5 | 0.85× (solves m21_white, loses m22_white) |

- The effect is factors, not percent, in both directions.
- Mob trajectories are far less salt-sensitive than the baseline's on the
  two cases measured at all salts — consistent with less dependence on
  "lucky" TT contents. Stress under mob is a single draw.
- The m22 collapse is systematic, not noise (all five salts). Undiagnosed.
  Hypotheses, unverified: `pn = min nᵢ` at OR nodes steers toward
  low-mobility "forcing-looking" lines; the AND-side `dn` init (1) still
  jumps on expansion, so the 1 → b problem moved to the other number.
- Not shippable as is. Shaped variants (capped or log mobility, one side
  only, blends with `(1, 1)`) are the next measurement, behind the gate of §5.

## 4. Measured out by the same probes

| lever | result | verdict |
| --- | --- | --- |
| Bigger TT (stress 256 / 512 / 1024 MB; m22 256 MB) | 280.7 M (+12.5%) / 288.9 M (+15.8%) / 319.6 M (+28.1%); m22 17.35 M (+22.5%) | not a work lever at 128 MB+; entry compaction (more entries per MB) loses its motivation |
| Depth-capped first outcome, stress D = 128 / 160 / 200 / 256 | 246.0 M (−1.4%) / 253.3 M / 245.0 M (−1.8%) / 252.3 M; D = 100 censored; m22 unchanged | within noise; no lever |
| Weak proof numbers (WPNS), naive form | stress / m22 censored; dec13 2.43×, dec10 1.37×; quick 420.5 M (10.8×), 4 timeouts | no-go in this form |
| Single unbounded work chunk (`PROBE_CHUNK0` = 4 G) | stress 1,131 M (4.53×); m22 +32.6%; dec13 +30.8%; dec10 +9.0% | chunk restarts are load-bearing; a 128 M first chunk gives stress −5.8% (one draw) |
| Repetition / bound hygiene (C4–C8) | stress: suppressed-draw stores 1,314 (118 clobber informative bounds); evals after a stale child result 1.7%; evals after the first `explored` mark ≤ 6.1% (nested overlap); m22 smaller | small on the gate object; **dec10 is the exception: 38.1% of evals follow a stale child result** |

## 5. Record corrections

- **`structural_floor.md` §3** says any path-derived bound "poisons" the TT,
  and uses that to exclude Deep df-pn. The incumbent already stores
  path-derived unsolved bounds: an AND frame with a repetition-draw child
  stores pn = ∞ as an *unsolved* entry (62,886 such stores on stress), and
  `evaluate_child` reuses unsolved entries carrying an ∞ bound 422,430 times.
  Unsolved bounds are heuristic, so the argument against path-relative
  valuation is weaker than stated. (Deep df-pn's own evidence, §6 there, is
  unaffected.)
- **`structural_floor.md` §2** (and `conversion` #8): the ε legs were
  measured where ε is arithmetically inert (§2 above). They close ε *on
  all-ones bounds*, not ε in general.
- **`structural_floor.md` §5**: correct about refuter rank; silent on bound
  magnitude, which is where the work is.
- **`structural_floor.md` §9 rows #6 / #13** ("no heuristic component"): a
  knowledge-free initialization exists and was never measured.
- **Gate methodology** (`initiative.md` working agreement #4, the
  `conversion`/`lean` per-case gates): single-draw per-case comparisons are
  below the noise floor on the hard class (§1).

## 6. Next steps, ranked

1. **Salt-seeded statistical gate.** Per-case distributions over ≥ 5 salts
   on a hard corpus of ≥ 20 cases, paired comparison; censored runs as
   right-censored data. Prerequisite for everything below.
2. **Initialization family.** Shaped mobility (capped / log / one-sided /
   blended), with a diagnosis of the m22 collapse first. Then cheap atomic
   features (blast and king-zone threats), and initialization borrowed from
   the same board stored at another rule50 clock (rule50 is in the key, so
   one board is stored once per clock value).
3. **Re-score the "wins on X" closures** listed in §1 under the gate.
4. **Restarts as a single-core lever.** If the per-salt distribution is
   heavy-tailed (stress suggests it), salted restarts that keep the TT can
   cut *expected* work; the chunk result (§4) already points this way.
5. **In-context child results** for the dec10 class: keep a child's
   just-proven repetition-dependent Draw in the parent frame instead of
   re-reading the TT's `(1, 1)` entry (38% of dec10's evals).
6. **Fringe.** Solve ignoring the 50-move rule and use that table only to
   seed initialization (the real search still proves everything); Kaneko
   root threads (see `parallel/reexamination.md` idea D).

## Caveats

- Five salts per case, one run each; stress under mob is a single draw.
- The salt remaps buckets only; other neutral noise channels (tie-breaking
  jitter, history aging) were not measured.
- Counter classes (C2, C4) use descendant evals and overlap across nesting;
  only the first-sweep share is additive.
- R6 suites are wall-clock-capped; their totals mix solved and timed-out
  cases.
