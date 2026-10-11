# Research Report 18 — Item #19 (third arm): `lean` plan10 history/killer arms re-scored

Executed 2026-10-11 per [`plan18.md`](plan18.md). **Both candidate arms
REJECT per the pinned gate v1.0 applied verbatim — no `lean` hand-off; the
#19 history/killer closure is upgraded from "unjudged" to gate grade.** The
`reexamination.md` §1 class of "killed by per-case flips" resolves as
follows: the quick-suite wall-cap kills were indeed artifacts (the arms do
not die at the gate's eval budgets the way gate 2 implied), but the tail
regressions that also killed them **replicate exactly** at distribution
grade — dec05 at 5.19× on all six salts under both arms, dec14 at 1.55–1.62×
— so lean plan10's no-go stands, now at distribution grade instead of
single-draw evidence.

## Deliverables

1. **The hook** (temporary, reverted): the lean plan10 constant-override
   pattern — `Search::new` reads `LEAN10_HISTORY_BONUS` / `_MAX` /
   `_AGE_INTERVAL` once into fields defaulting to the `history.rs` consts;
   `update_history_entry`, `age_history`, and the aging comparison take the
   fields; the temporary `LEAN10_STATS=1` side-band diagnostic (sort_calls /
   reorder-top-1-picks). No `SCORE_KILLER` override (dead degree of freedom,
   `k200` was byte-identical). Env unset ⇒ consts ⇒ bit-identical search.
2. **The rollout**: 22 frozen plan12 cases × 6 salts × 3 arms = **396 runs**
   (`--tt-size 128 --first-outcome --outcome-only --salt S --budget C
   --timeout 600`), baseline arm first and identity-checked, ≤ 3 concurrent
   processes, 2.45 h total wall (the plan's envelope was 2.2–2.5 h for the
   full 396; censoring stayed low as predicted).
3. **Fidelity anchors** (all green, recorded in
   [`measurements/plan18/env.json`](measurements/plan18/env.json)).
4. **Verdict tables + dispersion**
   ([`measurements/plan18/state/verdicts.json`](measurements/plan18/state/verdicts.json)).
5. **Revert verified**: `git diff --exit-code` clean; release build
   re-verified; salt-0 anchors re-run exact post-revert (stress 249,480,478;
   m22 14,156,269 with md5 `ea72f7ea…`; quick total 38,974,090);
   `cargo test --release --test test_repetition -- --include-ignored` green.
6. This report; the `reexamination.md` §1 bullet annotated; the
   `initiative.md` #19 row and history updated.

## D3 soundness and fidelity record (all green)

| check | result |
|---|---|
| (a) pristine pre-hook salt-0 identity | exact: stress 249,480,478; m22 14,156,269; m23 9,673,403; dec13 3,822,602; dec10 4,262,128; m20 censored at 2 B; m22 stdout md5 `ea72f7ea…` |
| (b) baseline-arm per-salt identity | all 132 cells bit-exact vs plan12 (parse.py hard gate, exit 0) |
| (c0) liveness first | lean plan10 probe arms reproduced bit-exactly on the hook build: `ag1` 31,493,500 (58 ok + archived m23_white timeout) and `hb1` 38,373,893 (58 ok + archived dec01 timeout); zero per-case diffs on ok cases |
| (c1) env-unset quick byte-identity | 38,974,090; 59/59 per-case `child_evals`, statuses, outcomes identical to the archived step-0 baseline — no hook defect |
| (c2) counter-bearing anchor | **2,477,336 `sort_calls` / 482,956 `reorder_diffs` exact** vs the archived lean plan10 `LEAN10_STATS` diagnostic; child-eval total with stats on unchanged at 38,974,090 (side-band confirmed) |
| (c3) candidate-arm quick fidelity | `hs2ag1` 31,345,410 and `hs05ag1` 34,631,157 bit-exact on all status-ok cases; `m23_white` reproduces `timeout: true` under both arms (per the pre-registered wall-cap limitation) |
| (d) outcome invariants | zero decisive-outcome conflicts within any arm across salts; every uncensored run of every arm matches its fixture expected outcome |
| (e) adopted-claim sub-study | dropped as pre-registered inapplicable (reordering lever asserts no claims); the cost went into (c)'s per-case fidelity, all exact |
| (f) cyclic-rook invariant | `8/8/8/8/2k5/8/8/4KR2 w - - 0 1` with `--no-preflight` proves `Draw` under both arms (never `Win`) |
| (g) revert | hook + diagnostic deleted, `git diff --exit-code` clean, anchors re-run exact, `test_repetition --include-ignored` green |

The report11-erratum lesson (liveness-first ordering, counter-bearing
anchor) worked as designed: (c2) caught nothing here because the port was
faithful on the first try, but the ordering rule meant every identity claim
below rests on a build whose liveness and counter behavior were proven
first — no stale-binary ambiguity anywhere in the record.

## Verdicts (pinned v1.0 verbatim)

Paired ratios are candidate/baseline `child_evals` per salt over
uncensored pairs; full tables with censoring patterns in
[`state/verdicts.json`](measurements/plan18/state/verdicts.json).

### Arm `hs2ag1` (BONUS 200, MAX 20000, AGE_INTERVAL 1) — **REJECT**

| case | basins | verdict | ratios (paired by salt) |
|---|---|---|---|
| m20_white | 4 | win | [0.379, 0.652] |
| stress | 3 | **censored-win** | [1.219, 0.481, 1.374] |
| m22_white | 6 | unclear | [0.77, 0.807, 1.087, 1.307, 1.537, 1.716] |
| m23_white | 5 | **censored-loss** | [0.763, 1.148, 0.884, 1.082, 1.065] |
| dec05 | 1 | **regression** | [5.187 … 5.19] (all six salts) |
| dec14 | 3 | **regression** | [1.552 … 1.615] |
| dec15 | 1 | **regression** | [1.173 ×6] |
| dec17 | 1 | **regression** | [1.133 ×6] |
| dec06 | 1 | win | [0.74 ×6] |
| dec13 | 2 | win | [0.785 … 1.034] |
| dec10 | 4 | unclear | [0.734, 0.904, 0.904, 0.919, 0.985, **4.60**] |
| dec01–dec04, dec07–dec09, dec11, dec12, dec16, dec18 | 1–6 | unclear | ≤ 1.07 single-direction |

Negatives: m23_white censored-loss (salt 0 budget-censored at 1 B — the
quick suite's 3 s kill was a wall artifact, but the underlying draw is a
>100× blowup vs the 9.67 M baseline), dec05/dec14/dec15/dec17 regressions.
Wins (m20_white, stress censored-win, dec06, dec13) are real but
insufficient under the rule: any regression or censored-loss rejects.

### Arm `hs05ag1` (BONUS 50, MAX 5000, AGE_INTERVAL 1) — **REJECT**

| case | basins | verdict | ratios (paired by salt) |
|---|---|---|---|
| m20_white | 4 | unresolved_under_budget | [1.086] (4/6 salts censored under the arm) |
| stress | 3 | **censored-win** | [1.222, 0.423, 1.214] |
| m22_white | 6 | **censored-loss** | [0.737, 1.18, 1.547, 1.087, 0.748] (salt 0 budget-censored at 1 B vs baseline 14.2 M) |
| m23_white | 5 | unclear | [0.763, 0.884, 1.065, 1.065, 1.16, **3.257**] |
| dec05 | 1 | **regression** | [5.187 … 5.19] (all six salts) |
| dec14 | 3 | **regression** | [1.552 … 1.615] |
| dec15 | 1 | **regression** | [1.170 ×6] |
| dec17 | 1 | **regression** | [1.133 ×6] |
| dec06 | 1 | win | [0.842 ×6] |
| dec13 | 2 | win | [0.78 … 1.028] |
| dec10 | 4 | unclear | [0.805, 0.878, 1.023, **1.726, 3.801, 4.678**] |
| rest | 1–6 | unclear | ≤ 1.07 |

Negatives: m22_white censored-loss (hard class; a salt-0 draw the baseline
solves in 14.2 M is censored at 1 B — the corpus exposes an m22-class
collapse lean plan10 could never see, as pre-registered) plus the same
dec05/dec14/dec15/dec17 tail as `hs2ag1`.

### Lever-level

No defer fired on either arm (no `unclear`-only shape met the D5
conditions; the single `unresolved_under_budget` cell contributes nothing
by the under-budget rule) → D5 never fired, `hs2ag5` was never needed.
No adopt → D6 reject path applies. Note the asymmetry plan18 D6
pre-registered: this is not merely a confirmation of lean plan10's no-go —
the closure now rests on 6-salt distributions per case rather than one
draw, i.e. the strongest form of the original closure.

## What the re-score decided

1. **The wall-cap kills were artifacts; the tail kills were not.** The
   per-case flips that gate 2 treated as fatal (m23_white under both arms,
   1.27× on 9.67 M evals) do not censor at the gate's eval budgets — but
   the arms die anyway, on *different* hard-class evidence: m23
   budget-censored at 1 B under `hs2ag1` (salt 0) and m22 budget-censored
   at 1 B under `hs05ag1` (salt 0). The 3 s flip was the shadow of a real
   blowup, just not at the wall cap.
2. **dec05's 5.19× is a constant-level fact, not noise.** Both arms land
   1,631,879–1,632,918 evals against the baseline's salt-invariant
   314,615 on every salt (ratio band 5.187–5.19, width 0.006) — the
   lean plan10 salt-0 reading replicated with essentially zero dispersion.
   dec14 (1.55–1.62×) and dec17/dec15 (~1.13–1.17×) are equally
   deterministic. The aging×scale constant family has a structural
   dec05/dec14 pathology, not a sampling risk.
3. **The aggregate quick wins do not transfer to the corpus.** The −19.6%/−11.1%
   quick totals translate into stress censored-wins + m20/dec06/dec13 wins
   that are outweighed by hard-class censoring losses and the
   deterministic tail. There is no subset of these constants that both
   keeps the wins and avoids the tail — `hs05ag100` (the flip-free −3.3%
   runner-up) remains excluded per D2: its aggregate cannot produce a
   case-level win under the pinned rule.
4. **The killer axis stays dead.** No `SCORE_KILLER` override was built
   (D2 rationale recorded in the plan); nothing in this rollout suggests
   reopening it.

## Paired-ratio dispersion — D7 prediction's fate: **falsified (mass claim), confirmed (tail claim)**

Fourth real-arm calibration data point for the comparison rule
(after plan13's ε arms, plan16's init arms, plan17's clockreuse arms):

| arm | n pairs | min | p10 | median | p90 | max | frac outside [0.9, 1.1] | exact-1.0 share |
|---|---|---|---|---|---|---|---|---|
| hs2ag1 | 124 | 0.379 | 0.785 | 1.000 | 1.552 | 5.190 | **41.9 %** | 29 % |
| hs05ag1 | 123 | 0.423 | 0.842 | 1.015 | 1.582 | 5.190 | **43.1 %** | 29 % |

The pre-registered prediction ("no byte-identical mass expected; frac
outside ≥ 90 %, plan16-like") is **falsified in its mass claim**: 29 % of
pairs sit at exactly 1.0 and only ~42 % fall outside the quiet band —
the same sparse-effect shape plan17 measured (48 %/35 %), not plan16's
93 %/90 % bimodality. The mechanism guess in the plan ("constants perturb
history values from the first proven frame") overestimated the coupling:
on the 14 salt-invariant, basin-1 cases the constants often do not change
the searched trajectory at all (dec03/04/07/08/09/16/18 sit at exactly 1.0
under `hs2ag1`), and when they do, the effect is frequently inside the
band. The **tail claim confirmed**: case-concentrated heavy tails in
exactly the pre-registered dec05/dec14 classes, with dec05 replicating at
the archived 5.19× — far outside the quiet band top 1.316, triggering the
pre-registered "rejects regardless of its aggregate win" clause.

No methodology revision is proposed from within this plan (D7 standing
rule). The rule now has four real-arm calibrations spanning both shapes
(bimodal init levers; sparse-effect constant/clock levers).

## Problems encountered

1. **`cargo build --release --examples` does not relink the `atomic_solver`
   bin** (only lib + example targets). The D3(c2) diag loop silently ran
   against the stale pre-hook solver binary and produced no
   `lean10stats:` line — caught immediately because the archived counter
   anchor could not reproduce; an explicit `cargo build --release` fixed
   it. This is the lean report10 stale-binary trap on the *solver*
   binary (that report hit the *example* binary); the standing procedure
   in `env.json` records that both binaries need explicit rebuilds after
   lib changes.
2. A post-revert m22 md5 mismatch (`3ae389bf…`) turned out to be a
   self-inflicted capture artifact (stderr merged into stdout on the
   re-run); with stderr separated the md5 is `ea72f7ea…` as archived. No
   defect.
3. Rollout wall time (2.45 h) matched the envelope's low end; no
   unexpected censoring explosion (m20/m22-class censoring under the
   candidate trajectories was the flagged unknown — it materialized as
   budget-censoring at exactly the salt-0 draws that decide the
   censored-loss verdicts, which is itself informative).

## Missing tests / gaps

- No test gaps in the product: the tree is byte-identical to HEAD
  (`git diff --exit-code` clean); nothing landed.
- The `LEAN10_STATS` diagnostic was validated only against the archived
  aggregate (one number pair); no unit test existed for the counter
  semantics — acceptable for a temporary diagnostic that was reverted.
- The dispersion rule's calibration set now has four data points but the
  pinned rule text is unchanged; if a fifth family shows a *new* shape,
  the owner may want an explicit two-shape note in
  `gate_methodology.md` (owner decision, not proposed here).

## Next steps

1. **Kickoff prompt (recommended): the last #19 arm** —

   > Execute `docs/plans/research/plan19.md` — item #19, fourth arm: the
   > `lean` plan7 TT-eviction arm V2 re-score under the pinned gate. Follow
   > plan18's structure: fresh 396-run 3-arm rollout over the frozen plan12
   > corpus (base + eviction-V2 mechanism ported from `lean` plan7's spec
   > via a temporary env hook, liveness-first with counter-bearing anchors
   > per the report11-erratum lesson), pinned v1.0 verdicts verbatim,
   > revert + D3(g) after. With the history/killer closure now at gate
   > grade (both arms reject), this is the last unjudged §1 closure; a
   > reject closes #19 entirely.

2. Alternative: close #19's record with a consolidated note once the
   eviction-V2 arm is decided (both remaining paths — reject or adopt —
   are already routable per D6).

3. If a future constant re-tune is ever proposed for `lean`, it must
   pre-register the dec05/dec14 tail as the primary gate object: these
   cases respond to history constants deterministically and sit far
   outside the quiet band at any scale×aging setting measured so far.

SESSION COMPLETE
- Deliverables: 396-run rollout + state/{runs,summary,verdicts}.json committed under `measurements/plan18/`; env.json with all D3(a)/(c) anchors; report18.md written; `reexamination.md` §1 bullet annotated; `initiative.md` #19 row + history entry updated; hook reverted, `git diff --exit-code` clean, post-revert anchors exact, `test_repetition --include-ignored` green.
- Gate result: hs2ag1 REJECT, hs05ag1 REJECT (pinned v1.0 verbatim); no D5 defer; D6 reject path; #19 history/killer closure upgraded unjudged → gate grade; no product change.
- Follow-up options:
  1. Kickoff prompt above — the `lean` plan7 TT-eviction V2 re-score, the last #19 arm (one plan closes item #19 entirely).
  2. Alternative: pause the #19 thread and take up #20 (salted TT-keeping restarts) or #21 (in-context child results) — both unblocked by #17, independent of #19's last arm; reason: the eviction-V2 port needs a careful re-implementation of plan7's V2 mechanism, while #20/#21 are self-contained measurements on the existing product.
