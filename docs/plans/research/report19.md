# Research Report 19 — Item #19 (fourth and final arm): TT-eviction V2 re-scored; **#19 complete**

Executed 2026-10-11 per [`plan19.md`](plan19.md). **The V2 arm REJECTS per
the pinned gate v1.0 applied verbatim — no `lean` hand-off; the #19
TT-eviction closure is upgraded from "unjudged" to gate grade, and with it
item #19 (re-score the unjudged closures) is complete: all four arms
(ε, clock-budget, history/killer, eviction) are now executed at
distribution grade.**

The headline is a *symmetric* correction of the plan7 record: the archived
salt-0 readings replicate bit-exactly (stress 1.382×, m22 1.237×), but both
dissolve under the salt channel — stress flips to **censored-win** (V2
solves salts 2/3/4 within 2.5 B evals where the baseline is censored, and
cuts the baseline's 758 M salt-1 tail to 0.369×), while m22 collapses to
**censored-loss** (salt-2 at 5.30×, salt-5 budget-censored at 2 B where the
baseline solves). The rejection is carried by m22's tail; plan7's no-go
stands, now at distribution grade — with the honest caveat that the strong
form of its "measured local optimum" claim (no alternative improves
anywhere) does not survive: the stress tail is real improvement evidence
for a depth-tiebreak eviction variant, recorded for future `lean` eviction
work under the two-slot layout constraint.

## Deliverables

1. **The hook** (temporary, reverted): one `v2_replacement: bool` field on
   `TranspositionTable`, read once at construction from `ATOMIC_TT_REPL=v2`
   (plan7's original env name; unset ⇒ shipped score, bit-identical),
   consumed only in `insert_new`'s victim score comparison — the
   lexicographic `(live, solved, work, remaining_depth, generation)` with
   the anchor-arbitrated direction (below). No layout change: two entries
   per bucket, no new entry fields, RAM = TT only untouched.
2. **The rollout**: 22 frozen plan12 cases × 6 salts × 2 arms = **264 runs**
   (`--tt-size 128 --first-outcome --outcome-only --salt S --budget C
   --timeout 600`), baseline arm first and identity-checked, ≤ 3 concurrent
   processes, ~50 min total wall on this x86_64 host (~4× faster than the
   aarch64 plan12/plan18 record host; evals are host-independent, which the
   132-cell identity audit confirms). Resumable driver; transcripts in
   `/tmp/plan19` (not committed).
3. **Fidelity anchors** ([`measurements/plan19/env.json`](measurements/plan19/env.json)):
   all green after the anchor-arbitrated direction arbitration; one
   pre-registered erratum candidate against the plan7 record (below).
4. **Verdict tables + dispersion**
   ([`measurements/plan19/state/verdicts.json`](measurements/plan19/state/verdicts.json)).
5. **Revert verified**: hook deleted, `git diff --exit-code` clean; release
   build + examples re-verified; salt-0 anchors re-run exact post-revert
   (stress 249,480,478; m22 14,156,269 with stdout md5 `ea72f7ea…`; quick
   total 38,974,090); `cargo test --release --test test_repetition --
   --include-ignored` green; `cargo clippy --release --all-targets` and the
   `tt` unit tests (22) green on the hook build before the rollout.
6. This report; the `reexamination.md` §1 eviction-V2 bullet annotated; the
   `initiative.md` #19 row closed with history entry; the
   `docs/plans/README.md` research row refreshed.

## D3 soundness and fidelity record

| check | result |
|---|---|
| (a) pristine pre-hook salt-0 identity | exact on the unhooked build at HEAD `c044d4e`: stress 249,480,478; m22 14,156,269; m23 9,673,403; dec13 3,822,602; dec10 4,262,128; m20 censored at 2 B (2,000,000,027); m22 stdout md5 `ea72f7ea…` |
| (b) baseline-arm per-salt identity | **all 132 cells bit-exact** vs plan12 (evals AND censoring patterns; parse.py hard gate, exit 0) |
| (c0) liveness first | `ATOMIC_TT_REPL=v2` salt-0 stress reproduces the archived anchor **bit-exactly: 344,866,966 evals, first win 97 plies, `pv_status: first-outcome`** — on the same binary that produced every number below |
| (c1) run anchors | m22 17,507,076 ✓; dec13 3,822,602 and dec10 4,262,128 bit-identical to their baselines ✓ (arm inert at salt-0 occupancy there, as recorded) |
| (c1) quick sub-anchor | **deviates on 2 of 59 cases** — see erratum below; outcomes/status/pv_len preserved 59/59 |
| (c2) env-unset quick byte-identity | 38,974,090; 59/59 per-case `child_evals`/status/outcome identical — the hook is inert when unset, no shipped-path defect |
| (d) outcome invariants | zero decisive-outcome conflicts within any arm across salts; every uncensored run of both arms matches its fixture expected outcome (parse.py hard gate, exit 0) |
| (e) cyclic-rook invariant | `8/8/8/8/2k5/8/8/4KR2 w - - 0 1` with `--no-preflight` never claims `Win` under either arm (both censor at the 600 s wall on this repetition-heavy draw proof — a base-arm property, not arm-specific) |
| (f) revert | hook deleted, `git diff --exit-code` clean, anchors re-run exact, `test_repetition --include-ignored` green |

### Anchor-arbitrated tiebreak direction (pre-registered clause, exercised)

The one-line plan7 record ("deep high-work subtrees protected over shallow
churn") fits both polarities. The primary direction (larger
`remaining_depth` kept, implemented as a negated component in the
evict-priority score) **missed**: stress salt-0 gave 306,484,827 / win 261.
The pre-registered opposite direction (smaller `remaining_depth` kept)
reproduces the anchor **bit-exactly** (344,866,966 / win 97) and was
recorded. **Erratum candidate #1 against the plan7 record**: the recorded
wording does not match the archived numbers; the archived stress scalar
corresponds to the *opposite* polarity of the one it suggests.

### Erratum candidate #2: plan7's quick-suite "never fires / evals bit-identical" claim

With liveness proven by the four CLI run anchors, the archived
[`quick_v2.json`](measurements/plan7/quick_v2.json) could not be reproduced:
the live arm fires on quick-suite **dec01 (−19,189)** and **m23_white
(+4,040)** at salt 0 (outcomes/status/pv_len preserved; the other 57 cases
bit-identical). Diagnosis: the archived `quick_v1.json` *and*
`quick_v2.json` are both bit-identical to `quick_clean.json` on all 59
cases — exactly what an arm-inactive (stale, pre-hook) *examples* build
produces — and plan7 (2026-09-20) predates the report10 stale-binary
finding. The trap was reproduced live in this session (a `benchmark` run on
a non-relinked examples build gave different firing deltas than the CLI
until `cargo build --release --examples` was re-run), which pins the
mechanism. Per D3(c3) this is recorded as an erratum candidate against the
plan7 record, not absorbed: its "the table never fills there [on quick]"
premise is falsified — full buckets with `(live, unsolved, equal-work)`
tie pairs exist on quick positions at salt 0 — while V1's quick
bit-identity remains plausible (a different, genuinely rarer firing
condition).

## Verdicts (pinned v1.0 verbatim)

Paired ratios are v2/base `child_evals` per salt over uncensored pairs;
full tables with censoring patterns in
[`state/verdicts.json`](measurements/plan19/state/verdicts.json).
**Lever verdict: REJECT** (m22_white censored-loss; D4: any regression or
censored-loss rejects, no exceptions).

| case | basins | verdict | ratios (paired by salt, salt order) |
|---|---|---|---|
| stress | 3 | **censored-win** | s0 **1.382** (= archived), s1 **0.369**, s5 1.055; v2 solves s2/s3/s4 at ≤ 2.5 B where base is censored |
| m22_white | 6 | **censored-loss** | s0 **1.237** (= archived), s1 0.770, s2 **5.296**, s3 0.983, s4 1.194; s5 v2 budget-censored at 2 B where base solves |
| m20_white | 4 | unresolved_under_budget | v2 censored **6/6** at 2 B vs base 2/6 (pinned under-budget rule hides the asymmetry; recorded as corroborating negative evidence outside the verdict) |
| m23_white | 5 | unclear | [0.971, 0.987, 0.992, 1.0, 1.0, 1.0] |
| dec01 | 6 | unclear | [0.997, 1.0 ×5] |
| dec13 | 2 | unclear (near-null) | s4 +28 evals (5,030,853 vs 5,030,825); all other salts exact |
| all other dec cases | 1–4 | unclear (exact null) | ratio 1.0 on every salt; the arm never fires |

The D5 defer-extension rule is not triggered (reject, not defer). The
stress censored-win cannot support an adopt against the m22 veto
(learn: hard-class wins do not outweigh a hard-class censored-loss under
the pinned rule — by design, WRONG-side evidence dominates).

## Plan7 closure re-grade (D6)

- **Adoption question: closed REJECT at gate grade.** V2's m22 tail
  (5.30×, budget-censored) vetoes; no `lean` hand-off; the two-slot layout
  binding constraint stands untouched.
- **The "incumbent priority is a measured local optimum" claim**: upgraded
  from two salt-0 draws to distribution grade **in its weak form only**.
  The distribution refutes the strong form — a one-line depth-tiebreak
  variant *improves* the stress tail (solves 3 of 3 baseline-censored
  salts; 0.369× on the baseline's worst uncensored salt) — while
  confirming the incumbent on every low-occupancy case (83.6 % exact-null).
  Both plan7 salt-0 readings were noise-floor misreads in *opposite
  directions*: stress's "+38.2 % regression" is a salt-0-specific
  inflation inside a tail-*winning* distribution; m22's in-band "+23.7 %"
  was the visible edge of a censored-loss tail. This is the gate working
  as designed on both sides, and the exact pattern D7 pre-registered as
  "the gate correcting a noise-floor misread" (it anticipated m22→unclear;
  the actual outcome is stronger: m22→censored-loss).

## Paired-ratio dispersion — fifth calibration data point (D7)

Over 122 uncensored paired salts: min 0.369, p10 1.0, median 1.0, p90 1.0,
max 5.296; **frac outside [0.9, 1.1] = 4.9 %**; **exact-null mass
(ratio = 1.0) = 83.6 %**.

**D7 prediction fate: falsified on the spread, confirmed on the null mass.**
Pre-registered: frac-outside ≈ 20–50 % (plan17-like sparse-effect shape)
and exact-null ≥ 40 %. Observed: 4.9 % frac-outside — *far* sparser than
any previous arm (plan17: 48/35 %, plan18: 42/43 %) — with the null mass
prediction comfortably met. The mechanism explains it: the arm fires only
on full buckets whose two slots are live, unsolved, and tie on
`(live, solved, work)` — rarer than plan19's occupancy-only reasoning
assumed (plan7 Phase-0 "full %" measures bucket fill, not tie-pair
frequency). Tails concentrated in the hard class ✓ (only stress/m22 carry
tails; the dec-class firings are micro-edits of +28/−29 evals). No
methodology revision proposed, per the plan.

## Problems encountered

1. **Anchor-arbitrated direction clause exercised** (primary polarity
   missed the stress anchor; opposite polarity reproduced it bit-exactly).
   The clause worked exactly as designed and is the report's erratum #1.
2. **The stale-examples trap bit this session's own anchor procedure**:
   after re-patching the hook, `cargo build --release` alone left the
   `benchmark` example binary at the old semantics, producing
   benchmark-vs-CLI firing deltas that consumed real diagnosis time before
   the probe-example bisection pinned it. Plan18's operational finding is
   confirmed and extended: after *any* lib change, both `cargo build
   --release` **and** `cargo build --release --examples` are required
   before *any* CLI or example-based anchor run.
3. **Plan7's archived quick anchors are unreliable** (erratum #2 above) —
   the second real-arm case (after report11's arma claim in plan17) of an
   archived fidelity claim failing under a liveness-verified rebuild.
4. Minor: the D3(e) cyclic-rook spot-check censors at 600 s under *both*
   arms (base-arm property), so it verifies the never-`Win` invariant but
   does not discriminate the arms.

## Missing tests

- No product change remains, so none are required by this plan. The
  pre-registered `lean` landing contract (if a future eviction proposal is
  ever made) would need `src/search/tt/tests.rs` fixtures updated to the
  new victim policy — recorded in plan19 D6, unused here.
- The tt unit tests cover the shipped `insert_new` score; the hook was
  validated by archived-anchor reproduction only (appropriate for a
  measurement spike).

## Next steps

Item #19 is **complete** (all four §1 re-examination arms at gate grade).
Open reopened-backlog items: **#20 salted TT-keeping restarts** (unblocked;
#17's distribution data exists — and this plan adds more heavy-tail
evidence: stress base 249 M→758 M across salts, m22 5.3× tail),
**#21 in-context child results (dec10 class)**, #22 fringe. The stress
tail-win observation is parked with `lean`'s eviction record (two-slot
layout constraint) — not an open research item unless `lean` reopens #12
with a layout hypothesis.

SESSION COMPLETE
- report19.md written; state/ + env.json + README committed under
  measurements/plan19/; hook reverted (`git diff --exit-code` clean,
  anchors re-exact, test_repetition green); reexamination.md §1 annotated;
  initiative.md #19 row closed + history entry; docs/plans/README.md
  research row refreshed; #19 item complete (all four arms executed)
Follow-up options:
  1. `Execute research item #20 (salted TT-keeping restarts): draft the
     one-goal plan for restarts that keep the TT but reseed the bucket
     index, using the plan12/plan17/plan19 heavy-tail distributions
     (stress 249 M–758 M+, m22 5.3× tail) as the sizing basis.` — the
     largest open expected-work lever now that #19 is closed.
  2. `Execute research item #21 (in-context child results, dec10 class):
     draft the plan for keeping a child's just-proven repetition-dependent
     Draw in the parent frame.` — independent, smaller; dec10 is the one
     control where the baseline sits mid-distribution.
