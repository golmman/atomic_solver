# report17 — plan17 (item #19, second arm: clock-budget solved-entry reuse re-score)

Date: 2026-10-11. Initiative: `research`. Evidence:
[`measurements/plan17/`](measurements/plan17/) (drivers + parsed state +
`env.json`; raw transcripts and sample dumps uncommitted, regenerable via
`driver.py`). Instrumentation: a temporary env-gated hook
(`ATOMIC_CLOCKREUSE=plan10 | arma` — the dfpn plan10/plan11 cross-clock
solved-entry mechanisms in temporary `src/search/dfpn/plan17_hook.rs`, wired
into `core.rs`/`children.rs`/`mod.rs`/`main.rs`) plus the temporary D3(e)
verifier (`examples/plan17_verify.rs`); **no product change remains** — both
were added, measured, and reverted (`git diff --exit-code` clean; release
build re-verified; D3(a) salt-0 anchors re-run exact after the revert;
`cargo test --release --test test_repetition -- --include-ignored` green).

## Verdict: **both arms REJECT** — the #19 clock-budget closure is upgraded from *unjudged* to gate grade

Pinned gate v1.0 (D4) applied verbatim to 396 fresh paired runs (3 arms ×
22 cases × 6 salts; baseline arm run first and identity-checked before the
candidate arms). No adopt → no `dfpn` hand-off (D6 reject path); no defer →
the D5 salt-extension rule never fired; the D7 calibration question never
fired (see §6).

| arm | lever verdict | reject drivers |
| --- | --- | --- |
| `plan10` (dfpn plan10 full lever: site 1 + site 2) | **reject** | regressions: **dec01** (basins=6; 1.95–2.28× on five salts, **41.7× at salt 1** — 270.5 M vs 6.49 M child evals), **dec06** (basins=1; 2.48×), **dec14** (basins=3; 1.26×) |
| `arma` (dfpn plan11 mechanism A: site 1 + TT store-back) | **reject** | regression: **dec14** (basins=3; 1.40–1.46×, all six paired ratios outside the quiet band top 1.316 — a clean, non-band-edge negative; D7 not triggered) |

The re-examination's regression-to-the-mean hypothesis is now answered for
this closure with a per-case salt distribution: **the single-draw readings
survive in direction but not in decision weight.** plan10's −37.7 % stress
reading replicates (salt-0 ratio 0.623; salt-1 0.274) *and so does its m22
collapse* — at gate grade m22_white is censored at 1 B on **6/6 salts**
(informational; the under-budget rule makes the case contribute nothing,
but the censoring pattern itself is unambiguous). arma's −8.8 % stress
reading (salt-0 ratio 0.912) sits exactly at its plan11 value and inside
the distribution: stress is a **censored-win** for arma (base censored at
2.5 B on salts 2/3/4; candidate solves all six salts, 215–522 M), m23_white
is a win (0.42–0.57×), dec10 is a win (0.41–0.78×) — but the sole dec14
regression vetoes the arm per the pinned rule, and m22_white is *unclear*
(per-salt ratios 0.65–1.28×, median 1.027: plan11's single 1.28× control
reading was the worst of six draws, its best is 0.65×).

## The D3(c) fidelity-anchor deviation (pre-registered anchor, failed as stated — debugged, not a hook defect)

The plan pre-registered, as part of D3(c), *"arma's quick-suite
byte-identity anchor: `benchmark --suite quick --json --first-outcome` at
default settings must show all 59 per-case `child_evals` equal to the
unhooked run (dfpn report11's measured drift-free property)."* **It does
not hold under the faithful port:** 37/59 quick cases drift (largest dec01
5.7 M → 15.5 M; smallest dec33 by 1 eval), with **zero outcome flips and
`wrong=false` everywhere** (the soundness surface is clean, matching
report10's full-lever finding).

The debug record, per the plan's "a mismatch halts for hook debugging"
clause:

1. The hook build with the env unset is pristine (plan12 salt-0 values
   exact through the benchmark: dec02 138,124; dec10 4,262,128; dec13
   3,822,602; m23 9,673,403).
2. The port reproduces **every archived numeric anchor bit-exactly**,
   including counters dfpn's reports recorded: arma stress FO 227,597,289
   with **exactly 58,206 site-1 adoptions**, **255,971 index entries**,
   301-ply line; arma m22 18,139,173; plan10 stress FO 155,394,450 with
   **exactly 491,031 site-2 adoptions**, **169,978 entries**, 303-ply line.
3. Byte-identity on a quick case requires **zero** site-1 adoptions in that
   run. The identically-parameterized mechanism that produces exactly
   58,206 stress adoptions demonstrably produces adoptions on quick cases
   (dec02: 74 adoptions, 138,124 → 113,240 evals). There is no
   adoption-rule variant that keeps the stress anchors exact and zeroes the
   quick cases: the two properties are arithmetically incompatible.
4. Conclusion: report11's "quick suite byte-identical — every per-case
   `child_evals` equal" claim for arm A is an erratum candidate, most
   plausibly a stale (unhooked) benchmark binary in that session's
   comparison (plan10's session *did* capture quick-suite drift with the
   same command). The rollout proceeded on the strength of the numeric
   anchors; the deviation is recorded here and in
   [`measurements/plan17/env.json`](measurements/plan17/env.json).

No other D3 invariant was violated; there was no HALT.

## Soundness invariants (all pass)

- **(a) Pristine identity:** pre-hook build at HEAD reproduces plan12
  salt-0 counts exactly (stress 249,480,478 / 477-ply; m22 14,156,269; m23
  9,673,403; dec13 3,822,602; dec10 4,262,128; m20 censored at 2 B) — run
  before the hook was applied; re-run exact after the revert (g).
- **(b) Baseline-arm identity:** the fresh base arm reproduces plan12's
  recorded per-salt values exactly, all 132 cells (evals AND censoring) —
  the null is degenerate, as pre-registered.
- **(c) Fidelity anchors:** all numeric anchors bit-exact (see above);
  the quick-suite byte-identity sub-anchor failed as stated (§ above).
  m22/`plan10` recorded informationally per salt: censored 6/6 at 1 B
  (the archived 714.6 M @ 120 s reading used different caps; at the
  rollout's 1 B cap the trajectory is censored on every salt).
- **(d) Outcome invariants:** zero decisive-outcome conflicts within any
  arm across salts; every uncensored run of every arm matches its fixture
  expected outcome — the substantive shift-lemma gate for the candidate
  arms (Win/Loss proofs re-derived under shifted clocks never flipped a
  decisive value on 264 candidate runs).
- **(e) Adopted-claim re-verification:** 540 `plan10` + 498 `arma` claims
  (deterministic side-band dump: first 5 adoptions per rollout run; 18/22
  cases covered; depth median 1, p90 3, max 9) replayed from the root FEN
  and re-solved at `max_depth = d` with the hook off — **zero
  disagreements** (plan10's 444/444 pattern at gate grade, ×2.3 scale).
  Limitation: the first-5-per-run sampler captures shallow claims; deep
  claims (the regime where `rule50 + d ≤ 100` binds) are under-covered —
  noted under missing tests.
- **(f) Cyclic-rook invariant:** `8/8/8/8/2k5/8/8/4KR2 w - - 0 1` (and at
  clock 90) never claims Win under either arm with `--no-preflight`;
  the draw-only searches populate 0 index entries (matching report11).
- **(g) Revert:** hook + verifier deleted; `git diff --exit-code` clean;
  release build re-verified; (a) anchors re-run exact; repetition tests
  green.

Rollout cost: ~2.2 h wall at ≤ 3 concurrent processes (base 2534 s /
plan10 3866 s / arma 1587 s); plan10's m22 censoring (6 × 1 B) and dec01
salt-1 (270 M) absorbed by the caps.

## Per-arm verdict tables (D4; ratios paired by salt, basin counts from plan12)

Full per-salt censoring patterns and ratios:
[`measurements/plan17/state/verdicts.json`](measurements/plan17/state/verdicts.json).
Condensed (basins = baseline basin count; verdicts per the pinned rule):

<details><summary>plan10</summary>

| case | basins | verdict | ratios / censoring |
| --- | --- | --- | --- |
| m20_white | 4 | unresolved_under_budget | cand cens 4/6; pairs 0.634, 0.796 |
| stress | 3 | win | 0.623, 0.274; base_only cens salts 3/4, cand_only cens salt 5 (cens at 2.5 B where base solves 252 M — mixed-censoring caveat, reported per salt) |
| m22_white | 6 | unresolved_under_budget | cand cens **6/6** at 1 B (informational catastrophe) |
| m23_white | 5 | win | 0.298–0.505, 6/6 pairs |
| dec01 | 6 | regression | 1.947, 1.950, 2.221, 2.277, 2.283, **41.671** (salt 1: 270.5 M vs 6.49 M) |
| dec02 | 1 | win | 0.826 |
| dec03 | 1 | unclear | 1.000 (no adoption on this trajectory) |
| dec04 | 1 | win | 0.595 |
| dec05 | 1 | unclear | 0.998 |
| dec06 | 1 | regression | 2.475–2.477 |
| dec07 | 1 | unclear | 1.000 |
| dec08 | 1 | unclear | 1.000 |
| dec09 | 1 | unclear | 1.000 |
| dec10 | 4 | win | 0.230–0.378 |
| dec11 | 1 | unclear | 1.000 |
| dec12 | 1 | unclear | 0.988 |
| dec13 | 2 | win | 0.731–0.964 |
| dec14 | 3 | regression | 1.233–1.284 |
| dec15 | 1 | win | 0.886 |
| dec16 | 1 | unclear | 1.000 |
| dec17 | 1 | unclear | 1.073 |
| dec18 | 1 | unclear | 1.000 |

</details>

<details><summary>arma</summary>

| case | basins | verdict | ratios / censoring |
| --- | --- | --- | --- |
| m20_white | 4 | unclear | 0.874, 0.994; base_only cens salt 0, cand cens salts 1/2/3 (mixed) |
| stress | 3 | **censored-win** | 0.912, 0.284, 0.912; base cens 2.5 B on salts 2/3/4, candidate solves all six (215–522 M) |
| m22_white | 6 | unclear | 0.650–1.281, median 1.027, 6/6 pairs, never censored |
| m23_white | 5 | win | 0.423–0.565, 6/6 pairs |
| dec01 | 6 | unclear | 0.767–2.713 (direction-mixed across salts) |
| dec02 | 1 | win | 0.820 |
| dec03 | 1 | unclear | 1.000 |
| dec04 | 1 | win | 0.721 |
| dec05 | 1 | unclear | 1.032 |
| dec06 | 1 | unclear | 1.043 |
| dec07 | 1 | unclear | 1.000 |
| dec08 | 1 | unclear | 1.000 |
| dec09 | 1 | unclear | 1.000 |
| dec10 | 4 | win | 0.406–0.779 |
| dec11 | 1 | unclear | 1.000 |
| dec12 | 1 | unclear | 0.989 |
| dec13 | 2 | unclear | 0.983–1.291 |
| dec14 | 3 | **regression** | 1.402–1.461 (outside the quiet band) |
| dec15 | 1 | unclear | 0.990 |
| dec16 | 1 | unclear | 1.000 |
| dec17 | 1 | unclear | 0.981 |
| dec18 | 1 | unclear | 1.000 |

</details>

Notes: m20_white is unresolved under budget for `plan10` (candidate
censored 4/6 at the pre-registered 2 B cap) and unclear for `arma` (mixed
censoring, 2 pairs) — the case contributes nothing decisive for either
arm, consistent with plan16's finding that m20 is heavy-tailed and
under-budgeted at 2 B. The many basins=1 `unclear 1.000` rows are exact
per-salt nulls: on those trajectories the index never binds (byte-identical
searches). Per the diversity clause, `arma`'s reject rests on dec14
(basins=3) — a multi-basin negative outside the noise band, the strongest
class of veto the gate has.

## Paired-ratio dispersion (comparison rule's third real-arm calibration data point)

| arm | n | p10 | median | p75 | p90 | max | frac outside [0.9, 1.1] |
| --- | --- | --- | --- | --- | --- | --- | --- |
| plan10 | 118 | 0.385 | 1.000 | 1.000 | 1.284 | 41.67 | 48.3 % |
| arma | 125 | 0.659 | 1.000 | 1.000 | 1.043 | 2.71 | 35.2 % |

Recorded with a reason, per the standing rule, **and as a falsification of
this plan's own D7 prediction**: the plan expected this family to be
"strongly bimodal … at least as heavy" as plan16's 93 %/90 % frac-outside.
Measured: 48 %/35 % — the cross-clock lever is **sparse-effect**, not
heavy-tailed overall: a large exact-null mass at ratio 1.0 (the index only
binds where cross-clock revisits occur) around a case-concentrated heavy
tail (dec01's salt-1 41.7×; plan10's dec01 cluster 1.95–2.28×). The pinned
median + 75 %-agreement clauses handled both shapes without strain (no
verdict hinged on a band edge), but the two measured lever families now
span very different dispersion shapes — relevant input for any future
methodology revision, none proposed here.

## D5 / D6 / D7 record

- **D5 (defer extension):** no case deferred for either arm → the {6, 7}
  salt extension never fires.
- **D6 (hand-off):** both arms reject → **no `dfpn` hand-off**; the #19
  clock-budget closure is upgraded from *unjudged* to **gate grade** with
  the tables above; the `reexamination.md` §1 bullet (the record's actual
  location of the claim — `structural_floor.md` does not carry a
  clock-budget claim) gains the gate-grade pointer.
- **Out of scope, for the record:** plan11's quarantine arms B1–B3 and
  contingency arm C remain closed on their catastrophic single-draw
  readings; this plan's `plan10` arm did **not** overturn its own
  catastrophic m22 reading (6/6 censoring at 1 B), so no re-score of the
  B/C arms is indicated.
- **D7 (dec10 caveat):** not triggered — dec10 is a *win* for both arms
  (0.23–0.38× / 0.41–0.78×), and `arma`'s sole negative (dec14) sits
  outside the quiet band, so no calibration question arises.

## Problems encountered

1. **The D3(c) quick-suite byte-identity anchor failed as stated** (§
   above) — the session's one pre-registered deviation. Debugged to a
   definitive conclusion (anchor's provenance, not the port); recorded in
   `env.json` and here.
2. The temporary verifier initially expected a bare JSON array while
   `verify.py` wrote `{"claims": [...]}` — caught by the sub-study's own
   zero-verified guard (`ok` requires `verified ≥ 100`), fixed before any
   result was read.
3. The first `arma` port accidentally left site 2 armed for both arms; the
   hook counters caught it immediately (arma stress reproduced plan10's
   155,394,450 instead of 227,597,289) and the anchors were re-run after
   the one-line arm gate — the counter-bearing hook paid for itself.
4. `/usr/bin/time` is absent in the container; the anchor script switched
   to shell timestamps (no measurement impact).

## Missing tests

- None to add: no product code changed (report15/report16 precedent;
  `make test` not required for a no-code-change session; the release build
  plus the 132-cell salt identity — a stronger check — were run twice).
- The D6 hand-off test list (cross-clock unit tests, `cross_clock.rs`)
  is **not** triggered: no adopt, no productization.
- Sub-study limitation worth noting for any future revival of this lever:
  the adopted-claim sampler should stratify by proven depth (the first-5
  rule over-covers shallow claims, max d = 9 in this sample) — the deep
  claims are where the shift lemma's `rule50 + d ≤ 100` binding actually
  lives.

## Next steps

1. **`dfpn` backlog #2 stays closed** — now at gate grade on the frozen
   corpus distribution, not just the spike's single draws. The quarantine
   arms B1–B3 and arm C remain closed on their catastrophic readings (not
   re-indicated: plan10's m22 catastrophe replicated at 6/6 censoring).
2. Remaining #19 closures, each a one-goal plan under the pinned gate:
   the `lean` plan10 **history/killer arms** re-score, and the `lean` plan7
   **TT-eviction arm V2** re-score.
3. The dispersion record (§ above) is standing input for the methodology
   owner: two measured lever families with opposite shapes (plan16
   heavy-tailed sparse-null vs plan17 sparse-effect with heavy tails) — a
   revision discussion, if any, needs to handle both.

SESSION COMPLETE
- plan17 executed end-to-end: D3(a)–(g) pass (one pre-registered sub-anchor
  failed as stated, debugged to a report11 erratum candidate and recorded);
  396/396 runs; fidelity anchors bit-exact incl. adoption counters; 1,038
  adopted claims re-verified with zero disagreements; verdicts plan10
  REJECT / arma REJECT (D4 verbatim; no defer, D5 not fired, D7 not
  triggered, no D6 hand-off — closure upgraded unjudged → gate grade);
  measurements/plan17/ written (driver, parse, verify, verdicts, env.json,
  README.md, state/); hook + verifier reverted clean (`git diff
  --exit-code`), release build re-verified, anchors re-run exact,
  test_repetition green; report17.md written; reexamination.md §1 bullet
  annotated with the gate-grade pointer; docs/plans/README.md research row
  tail refreshed
Follow-up options:
  1. "Execute docs/plans/research/plan18.md for the `lean` plan10
     history/killer arms re-score" — draft + execute the next #19 closure
     under the pinned gate (one-goal sized, same 396-run shape; the
     single-draw kills there were per-case flips, exactly the class the
     salt distribution re-judges).
  2. Alternative: close the `research` session here — the remaining #19
     arms (history/killer, eviction V2) can wait for an owner go; the
     clock-budget closure record is complete at gate grade.
