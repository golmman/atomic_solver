# Report 6 — SPDFPN stage-2 follow-up: length-threshold experiment, re-scoped gate, NO-GO confirmed, initiative closed

Executes `parallel` backlog **#4, stage-2 follow-up** per `plan6.md` (the
owner-directed re-measurement after plan5b's NO-GO). This is the plan's
final task.

**Verdict: NO-GO — the plan5b shuffle-win promise did not survive honest
rep counts; the revert is re-executed and the initiative closes on the
strengthened record.** The plan6 campaign (8 interleaved reps ×
N ∈ {2, 3, 4} on m22 + rem12 + shuffle-win, W re-locked to 20 000 by the
pre-registered long-class confirmation) measured **S4 = 1.01× (m22) /
1.20× (rem12) / 1.65× (shuffle-win)** and **I4 = 3.54× / 3.03× / 2.22×**,
with a cap-hit tail up to 3/8 — failing every GO conjunct of the
re-registered gate. H1 (the hypothesis the owner directed this plan to
test) fails on both of its conjuncts: shuffle-win's plan5b numbers
(S4 = 2.68×, I4 = 1.41×) were a **5-rep artifact of a bimodal run
distribution**. Soundness was never in question: 64/64 decisive-run
agreement (every decisive run `win`), zero panics, zero non-zero exit
codes across all 96 campaign runs. Condition-2 revert executed: 0-line
code diff vs pre-plan4b, all three byte-identity hashes identical to the
plan4/plan4b/plan5a/plan5b records.

## What this plan did

1. **Stage 0 — re-land (mechanical).** The stage-2 mechanism (stage 1b
   sharded TT + stage 2a `--threads N` helpers) was restored verbatim from
   commit `b714ef6` (`git checkout b714ef6 --` over the exact 22-file
   revert inventory of `report5.md`, in reverse) plus the plan5b W delta
   (initially `MAX_WORK_PER_JOB = 10_000`). N = 1 verification before any
   parallel run: quick suite 59/59 `child_evals` identical vs
   `baseline_quick_pre5.json`, m22/shuffle-win stdout hashes
   (`b7c74f17…` / `64129ef0…`) and snapshot sha (`eaa5f2b9…`/195 B)
   identical, `make test`/fmt/clippy green.
2. **Stage 1 — case selection (locked before any parallel run).** The
   sequential scan filled the plan's [10, 30] s window from
   `tests/fixtures/decisive_remaining.txt` (the repo's existing pool of
   measured Win cases; the plan's named pools are indeed bimodal —
   m21_black/m22_black cap-hit at 60 s, m23_black 0.68 s, rem04/10/07/08
   3.0–6.7 s). Four in-window candidates measured; **rem12** selected
   (17.09 s, white-to-move Win, same class as both endpoints, deterministic
   reference 69,605,385 child_evals over two identical driver runs,
   log-scale position ≈ 0.57 between m22 and shuffle-win). Scan + locked
   case table in `measurements/plan6/README.md`.
3. **Stage 2 — W confirmation on the long class (then locked).**
   shuffle-win t4, W ∈ {4000, 10000, 20000}, 3 reps each, interleaved
   N = 1 references: medians 29.44 s / 29.69 s / **23.33 s** → the plan5b
   m22 winner 10 000 does **not** hold on the long class; **W locked to
   20 000** per the pre-registered fallback. Honest caveat, carried
   through the campaign: **W is case-dependent** (the plan5b m22 sweep
   punished 20 000 ~3×; the long class rewards larger jobs), so plan6
   campaign numbers are not directly comparable to plan5b's — the gate is
   self-contained (its own in-session N = 1 denominators).
4. **Stage 3 — campaign** (plan5b protocol tightened): ≥ 8 interleaved
   rounds per case, N = 1 in-session denominator, cap-hit rate as a
   first-class gate item.

## H1–H3 results (the pre-registered predictions)

- **H1 (shuffle-win reproduces at higher rep count: S4 ≥ 2.0×, I4 ≤ 1.5×)
  — FAILS on both conjuncts.** S4 = 1.65×, I4 = 2.22× at 8 reps. The
  mechanism is now measurable in the raw data: the shuffle-win t4
  distribution is **bimodal** — 5 of 8 runs land 20–39 s (healthy:
  1.5–2.8× speedup, 0.3–0.5 G evals ≈ 1.3–2.0× sequential) and 3 of 8
  land 90–100 s (bad-trajectory tail: 1.4–1.6 G evals ≈ 5.7–6.3×
  sequential, cap-hit timeouts). plan5b's 5 reps and plan6's stage-2 sweep
  (3 reps, 18.3–26.4 s) both sampled the good tail; at 8 reps the tail is
  in the median. This is the same coordinator-perturbation pathology
  plan5/5b diagnosed qualitatively ("~1 of 5 runs steered into a bad
  region"), now quantified at the distribution level: the bad-tail
  frequency is ~30–40 %, and it is *median-visible* at honest rep counts.
- **H2 (mid case lands between m22 and shuffle-win; Spearman ρ > 0.8)
  — holds directionally, and the trend is perfectly monotone
  (ρ = 1.0):** S4 = 1.01× → 1.20× → 1.65×. The length gradient H
  predicted is real. But the *level* is the problem: even the longest
  case (58 s sequential) reaches only 1.65× at 4 threads with 2.22× work
  inflation — far below the literature band (SPDFPN ≈ 5.2×/4 threads,
  f_s ≤ 1.40) and far below the re-registered GO threshold.
- **H3 (m22 at 8 reps does not materially regress: S4 ≥ 1.5×) — fails**
  (S4 = 1.01×), with the declared confound: m22 runs at W = 20 000, which
  the plan5b sweep showed is ~3× worse for the short case than 10 000.
  Even discounting the confound, m22 was only a control — no gate rests
  on it.

## Campaign results (medians over 8 interleaved reps; W = 20 000)

Sequential references (in-session, deterministic): m22 3.263 s /
14,156,269 evals (`benchmark --suite move-order`); rem12 16.886 s /
69,605,385 evals (`seq_ref.rs` driver, 2 identical runs); shuffle-win
57.707 s / 249,480,478 evals (`benchmark --suite move-order`, identical to
plan5b).

| case | N | median wall | speedup | inflation | cap-hits | agreement | helper share |
| --- | --- | --- | --- | --- | --- | --- | --- |
| m22 | 2 | 3.234 s | 1.01× | 1.98× | 0/8 | 8/8 `win` | 0.49 |
| m22 | 3 | 2.803 s | 1.16× | 2.55× | 0/8 | 8/8 `win` | 0.65 |
| m22 | **4** | 3.235 s | **1.01×** | **3.54×** | 1/8 | 7/7 `win` | 0.74 |
| rem12 | 2 | 12.701 s | 1.33× | 1.51× | 0/8 | 8/8 `win` | 0.49 |
| rem12 | 3 | 13.120 s | 1.29× | 2.31× | 0/8 | 8/8 `win` | 0.66 |
| rem12 | **4** | 14.070 s | **1.20×** | **3.03×** | 0/8 | 8/8 `win` | 0.74 |
| shuffle-win | 2 | 49.282 s | 1.17× | 1.76× | **3/8** | 5/5 `win` | 0.49 |
| shuffle-win | 3 | 33.755 s | 1.71× | 1.78× | 2/8 | 6/6 `win` | 0.65 |
| shuffle-win | **4** | 34.944 s | **1.65×** | **2.22×** | **2/8** | 6/6 `win` | 0.74 |

- **Outcome agreement: 64/64 decisive runs equal the sequential outcome
  (`win`)**; the 8 cap-hit runs are resource-cut `draw`s at the wall cap
  (rc = 0, documented `ExitReason::Timeout` semantics), recorded
  separately per the declared plan5b convention. Zero panics, zero lock
  anomalies, zero non-zero return codes in all 96 runs. **V = 0.**
- **Cap-hit rates** (gate item C): m22 t4 1/8, shuffle-win t2 3/8, t3 2/8,
  t4 2/8 → worst gated-case rate C = 3/8 = 0.375 > 1/8 → **fails**.
- Helper work share at t4 is 0.74 on all three cases (t2 ≈ 0.49): helpers
  do real work; the bottleneck remains pre-warm *quality* and the
  coordinator perturbation tail, not idling.
- Inflation scales with case length at t4 (3.54× / 3.03× / 2.22×) —
  helper pre-warm never amortizes enough to offset the coordinator
  trajectory damage, even on the 58 s case.
- **N ∈ {8, 16} extrapolations** (log-fit on the four medians,
  `state/campaign6_summary.json`): labeled non-gating, and here
  *unreliable* (shuffle-win medians are non-monotone: S4 < S3), so no
  scaling claim is drawn from them at all.

## Verdict against the re-registered gate (mechanical)

- **NO-GO clause 1**: H1 fails (shuffle-win S4 = 1.65× < 2.0× and
  I4 = 2.22× > 1.5× at 8 reps) → NO-GO. *(This is the clause the owner's
  re-open directive targeted — the "promising" S4 = 2.68×/I4 = 1.41× was
  indeed a 5-rep artifact, in the direction the data now shows.)*
- **NO-GO clause 2**: mid case I4 = 3.03× > 2× → NO-GO independent of
  clause 1 (rem12 also misses S4 ≥ 1.5×).
- **NO-GO clause 3**: C = 3/8 > 1/8 on a gated case → NO-GO.
- GO required all conjuncts simultaneously; MARGINAL required H1 to hold
  with other conjuncts missing — H1 does not hold, so this is not an
  owner-escalation case. **NO-GO, executed without discretion.**

## Decision executed (NO-GO branch)

1. All numbers recorded first (`measurements/plan6/state/`:
   `campaign6_raw.jsonl`, `campaign6_summary.json`, `campaign6.csv`,
   `sweep6_raw.jsonl`, `sweep6.csv`, `moveorder_seqref.json`,
   `quick_reland.json`, `quick_post_revert.json`, `drift6_results.json`;
   raw stderr logs under `logs/`, gitignored).
2. **Revert re-executed** (the `report5.md` mechanical procedure):
   `git checkout 7cefe41 --` over the 19 tracked files of the revert
   inventory + manual deletion of the 4 files that 7cefe41 does not have
   (`src/search/dfpn/parallel/{mod,jobs}.rs`, `src/search/tt/shard.rs`,
   `tests/test_parallel.rs`); the temporary `examples/seq_ref.rs` driver
   removed (its source stays committed under `measurements/plan6/`).
   `git diff 7cefe41 -- src tests examples Cargo.toml Cargo.lock
   AGENTS.md` = **0 lines**. Two `git checkout` calls touched the index
   (staged-then-reset via `git restore --staged`); no commits, no tree
   changes beyond content writes — the index ends the session clean, and
   the untracked `measurements/plan6/` + docs edits are the only deltas.
3. **Verification, all green**: `make test` (407 passed / 0 failed),
   `cargo fmt --check` clean, `cargo clippy --all-targets` clean, quick
   suite 59/59 `child_evals` identical vs `baseline_quick_pre5.json`,
   m22/shuffle-win stdout sha256 `b7c74f17…` / `64129ef0…` and snapshot
   sha256 `eaa5f2b9…`/195 B — all identical to the plan4/plan4b/plan5a/
   plan5b records (`state/drift6_results.json`). The ~5 % TT-concurrency
   tax is refunded again.
4. `initiative.md` backlog #4 row closed NO-GO (final state); status
   paragraph updated; `docs/plans/README.md` `parallel` row updated
   (close event).

## Problems encountered

- **The `benchmark --suite move-order` sequential reference takes
  ~13–15 min wall** at `--timeout 100` (the suite's m20/m21_black cases
  cap-hit; m20_white is ~100 s) — far slower than the plan's session
  budget assumed; run in the background with polling.
- **W is case-dependent and now measured to be so**: the plan5b sweep
  (m22) and the plan6 confirmation (shuffle-win) pick *different*
  winners. The pre-registration anticipated the possibility (lock the
  long-class winner) but the consequence is a real confound: plan6's
  campaign runs at W = 20 000, so the m22 regression (S4 1.01× vs
  plan5b's 1.79× at W = 10 000) partially reflects the W choice. This
  does not soften the verdict — the gated cases (rem12, shuffle-win)
  ran at their own sweep's winner, and both fail their gates — but a
  future re-open should treat W as a per-case or adaptive parameter,
  not a global constant.
- The mid-case selection filled the [10, 30] s window from
  `decisive_remaining.txt`, a pool the plan named only implicitly
  ("docs/plans measurement records"); recorded as a declared scan-pool
  note in `measurements/plan6/README.md`, with all four in-window
  candidates measured sequentially before the lock.

## Deviations from the pre-registered protocol (declared)

1. **`git checkout` + `git restore --staged` used for the revert** (vs
   `report5.md`'s write-back approach): content-identical, faster, and
   the index was restored to HEAD state; no commits or tree-state writes
   beyond file contents. Noted for the no-`git`-writes convention.
2. **Scan pool extension** (above): the named pools could not fill the
   [10, 30] s window; the pool extension was measured-first and
   documented before the lock, consistent with the plan's "candidate
   scan" language.
3. **Campaign per-case rounds sequential in case order** (m22 → rem12 →
   shuffle-win) rather than fully interleaved across cases: same shape as
   plan5b (rounds × N back-to-back per case); cross-case session drift is
   bounded by the per-round N = 1 denominators, which were stable
   (m22 3.238–3.291 s, rem12 16.806–16.994 s, shuffle-win
   57.237–58.097 s).

## Tools / examples used

`cargo`/`make`, `benchmark` (quick + move-order suites, `--json`), the
solver CLI, `sha256sum`, and five Python/Rust drivers committed under
`measurements/plan6/` (`sweep_w6.py`, `campaign6.py`,
`analyze_campaign6.py`, `seq_ref.rs` — the last compiled as a temporary
`examples/seq_ref.rs` copy during the session and removed after; no
solver code changes beyond the re-land + the W lock, all reverted).

## Unresolved parts / missing tests

- The torn-state/race stress test for the helper path under
  `make test-full` load was never written (carried from `report5a`/
  `report5`; moot after the second revert).
- Per-phase work accounting (separating helper pre-warm quality from
  coordinator perturbation) was never built — it remains the first
  diagnostic any future re-open should build.
- The W-case interaction (adaptive W, or per-case W) is unmeasured
  beyond the two sweep points; noted as a follow-up lever only.
- The bad-trajectory tail's cause (which helper-stored entries steer the
  coordinator) is identified only statistically (bimodal distribution,
  5.7–6.3× eval inflation in tail runs), not mechanistically.

## Next steps

- **Initiative closure (executed by this report + the initiative/README
  row updates)**: backlog #4 closed NO-GO on the strengthened record —
  the no-go now survives a second, larger-sample campaign with a mid
  case, a cap-hit gate, and W re-swept per pre-registration. The
  parallelism space as scoped is measured out end to end: plan2 option C
  (0.27×), lean plan7 deterministic (1.47×), SPDFPN stage 2 (plan5b and
  plan6, ≤ 1.65×/4 threads at honest rep counts). No ship shape exists
  that clears any pre-registered gate; an opt-in mode documented for
  long solves would still ship a mode that is *slower than sequential*
  on the median for every measured case — there is nothing to document.
- If the owner ever re-opens option A again, the bar has moved: a
  re-open must bring a *mechanism* lever targeting the bad-tail
  pathology (per-phase work accounting as the diagnostic; TT
  replacement scoring that discounts helper entries; helper caps;
  steering helpers away from the coordinator's chunk), not another
  measurement campaign. Measurement alone has now answered the question
  twice.
- Recommended next session: none required for this initiative (closed);
  the roadmap's other initiatives are unaffected.

SESSION COMPLETE
- plan6.md executed end to end: stage-0 re-land verified (all hashes green),
  case selection locked (rem12), W re-locked to 20 000, campaign run
  (96 runs, 8 reps × 3 cases × N ∈ {1..4}), NO-GO verdict executed
  (revert re-done, byte-identity re-verified), report6.md written,
  initiative.md + docs/plans/README.md closed, measurements/plan6/ complete
  (README, env.json, 4 drivers, state/*), gate: make test green (407/0)
- Follow-up options:
  1. No follow-up session needed for `parallel` — the initiative is closed
     on a twice-measured no-go. If the owner wants to pursue option A
     anyway, the kickoff prompt is: "Re-open parallel option A with a
     mechanism-first plan: build per-phase work accounting (report6
     unresolved item), then prototype helper-entry-discounting TT
     replacement scoring; the ship gate from plan6.md is closed history —
     a new pre-registration is required."
  2. Alternative: leave `parallel` closed and spend the next session on a
     different initiative's open items (e.g. `proofdb` or `lean` backlog) —
     reason: the parallelism space is measured out and the remaining levers
     are mechanism-level, not measurement-level.
