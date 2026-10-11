# plan17 measurements (2026-10-11) — clock-budget solved-entry reuse re-score (`dfpn` #2)

Evidence for [`../../report17.md`](../../report17.md). Zero product-code
changes remain: the temporary env-gated hook (`ATOMIC_CLOCKREUSE=plan10 |
arma` — cross-clock solved-entry index, store site + probe sites per dfpn
`plan10.md` §Design / `plan11.md` §Mechanism A, in temporary
`src/search/dfpn/plan17_hook.rs` wired into `core.rs`/`children.rs`/`mod.rs`)
plus the temporary D3(e) verifier (`examples/plan17_verify.rs`) were added,
measured, and reverted (`git diff --exit-code` clean; release build
re-verified; D3(a) salt-0 anchors re-run exact after the revert;
`test_repetition --include-ignored` green).

| artifact | role |
| --- | --- |
| `driver.py` | rollout driver: 3 arms × 22 cases × 6 salts = 396 runs; arm via the `ATOMIC_CLOCKREUSE` env var; candidate runs also set `ATOMIC_CLOCKREUSE_SAMPLE` (side-band adopted-claim dump); resumable, ≤ 3 concurrent processes; `base` arm first (D3(a)/(b) gate) |
| `parse.py` | raw transcripts → `state/{runs,summary}.json` (claims collected into `summary.json`); hard-fails on any D3(b)/(c)/(d) invariant violation |
| `verify.py` | D3(e) sub-study runner: claims → `examples/plan17_verify` (replay + `max_depth = d` re-solve, hook off) → `state/verification.json`; exits 1 on any disagreement |
| `verdicts.py` | pinned-v1.0 verdict machinery (D4) + D5/D7 notes + paired-ratio dispersion → `state/verdicts.json` |
| `state/runs.json` | per-run records (arm, evals, outcome, censoring, wall, hook counters) |
| `state/summary.json` | per-case-per-arm records + D3 check results + the collected D3(e) claims (`claims` key) |
| `state/verification.json` | adopted-claim re-verification results (540/540 plan10, 498/498 arma, zero disagreements) |
| `state/verdicts.json` | per-case verdict tables, lever verdicts, dispersion |
| `env.json` | environment, hook, run grid, soundness-invariant results |

Raw transcripts, per-run sample dumps, and anchor files
(`/tmp/plan17/results`, `/tmp/plan17/anchors`) are not committed;
`driver.py` regenerates them deterministically.

## Headline results (lever verdicts, pinned gate v1.0 applied verbatim)

**Both arms: reject.** The #19 clock-budget solved-entry closure is upgraded
from *unjudged* to gate grade.

| arm | verdict drivers |
| --- | --- |
| `plan10` | regressions: **dec01** (basins=6; ratios 1.95–2.28× on five salts, **41.7× at salt 1** — 270.5 M vs 6.49 M), **dec06** (2.48×), **dec14** (1.26×); hard-class wins: stress (win, 0.27×/0.62× on 2 uncensored pairs + 2 censored-wins + 2 both-censored), m23_white (win, 0.30–0.51×), dec10 (win, 0.23–0.38×); m22_white censored **6/6** at 1 B (under-budget rule → contributes nothing; informational), m20_white unresolved under budget |
| `arma` | regression: **dec14** (basins=3; 1.40–1.46×, outside the quiet band — D7 question not triggered); hard-class support: stress **censored-win** (base censored at 2.5 B on salts 2/3/4; candidate never censored; uncensored pairs 0.912/0.284/0.912), m23_white (win, 0.42–0.57×), m22_white unclear (0.65–1.28×, median 1.027 — the archived 1.28× single reading is the worst of six draws, best 0.65×), m20_white unclear; dec10 win (0.41–0.78×) |

No defer was issued on any case (D5 salt extension never fired). The D7
calibration question did not fire (dec10 is a *win* for both arms; arma's
sole negative dec14 sits outside the quiet band).

## Paired-ratio dispersion (third real-arm calibration data point)

| arm | n | p10 | median | p75 | p90 | max | frac outside [0.9, 1.1] |
| --- | --- | --- | --- | --- | --- | --- | --- |
| plan10 | 118 | 0.385 | 1.000 | 1.000 | 1.284 | 41.67 | 48.3 % |
| arma | 125 | 0.659 | 1.000 | 1.000 | 1.043 | 2.71 | 35.2 % |

The plan17 D7 prediction ("expected strongly bimodal, at least as heavy as
plan16's 93 %/90 %") was **wrong**: this lever family is *sparse-effect* — a
large no-effect mass at exactly 1.0 (the index only binds where cross-clock
revisits occur; many cases are byte-identical) around a heavy-tailed
case-concentrated core (dec01's salt-1 41.7×). Effect mass is concentrated
in fewer cases than the shaped-init family, with milder within-case tails
except at catastrophic outliers.

## Soundness invariants (all pass; no HALT)

- **(a) Pristine identity:** pre-hook build at HEAD reproduces plan12
  salt-0 counts exactly (stress 249,480,478 / 477-ply line; m22 14,156,269;
  m23 9,673,403; dec13 3,822,602; dec10 4,262,128; m20 censored at 2 B) —
  run before the hook was applied; re-run exact after the revert (g).
- **(b) Baseline-arm identity:** the fresh base arm reproduces plan12's
  recorded per-salt values exactly, all 132 cells (evals AND censoring).
- **(c) Fidelity anchors:** arma stress 227,597,289 / 58,206 site-1
  adoptions / 255,971 index entries / 301-ply line; arma m22 18,139,173;
  plan10 stress 155,394,450 / 491,031 site-2 adoptions / 169,978 entries /
  303-ply line — **all bit-exact vs dfpn report10/report11**, including the
  adoption counters. **One sub-anchor failed as stated:** report11's arma
  quick-suite byte-identity claim does not replicate (37/59 quick cases
  drift; zero outcome flips, `wrong=false` everywhere) — see report17.md
  for the debug record: the claim is arithmetically incompatible with the
  exact stress anchors above (byte-identity requires zero quick-case
  adoptions; the identically-parameterized probe makes 74 on dec02).
- **(d) Outcome invariants:** zero decisive-outcome conflicts within any
  arm across salts; every uncensored run of every arm matches its fixture
  expected outcome (substantive shift-lemma gate for the candidate arms).
- **(e) Adopted-claim re-verification:** 540 plan10 + 498 arma claims
  (deterministic side-band dump, first 5 adoptions per rollout run) replayed
  from the root FEN and re-solved at `max_depth = d` with the hook off —
  **zero disagreements** (plan10's 444/444 pattern at gate grade, ×2.3 scale).
- **(f) Cyclic-rook invariant:** `8/8/8/8/2k5/8/8/4KR2 w - - 0 1` (and at
  clock 90) never claims Win under either arm with `--no-preflight` (draw-only
  searches populate 0 index entries, matching report11); post-revert
  `cargo test --release --test test_repetition -- --include-ignored` green.
- **(g) Revert:** hook + verifier deleted; `git diff --exit-code` clean;
  release build re-verified; (a) anchors re-run exact.

Rollout cost: ~2.2 h wall at ≤ 3 concurrent processes (base 2534 s /
plan10 3866 s / arma 1587 s); plan10's m22 censoring (6 × 1 B) and dec01
salt-1 (270 M) absorbed by the caps.
