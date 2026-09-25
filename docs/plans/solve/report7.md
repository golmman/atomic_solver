# Report 7 — sequential d4d5-p2 sizing ladder (item 8, stage 1/3)

Executes `plan7.md` (item 8, stage 1 of 3) in one session, 2026-09-24.
Harness: `measurements/plan7/ladder.py` (adapted from plan3's
`probe.py`; snapshot reader + binary paths reused from
`../plan2/ssfp.py` verbatim). **No product changes.** Both fresh arms
ran strictly sequential on the otherwise idle sandbox, stdin from
DEVNULL, raw captures under `logs/`, per-arm state JSONs under
`state/`, environment in `env.json` (identical cgroup to plan3:
4-CPU quota, 8 GiB memory.max).

## 1. Gate verdict: **LINEAR**

All three pre-registered arms censored (prior ≈ 0 for COMPLETED held);
the §3 gate fires **LINEAR**:

| arm | budget | nodes at censoring | wall (s) | rate (nodes/s) | max-RSS |
| --- | --- | --- | --- | --- | --- |
| L1 (`state/arm_ladder1.json`) | 3600 s | 709,693,440 | 3602.6 | 196,997 | 234,668 kB |
| M2 = plan3 A1 (`../plan3/state/arm_a1.json`) | 7200 s | 1,428,643,840 | 7202.5 | 198,353 | 234,476 kB |
| L4 (`state/arm_ladder4.json`) | 14400 s | 2,870,968,320 | 14402.5 | 199,338 | 234,668 kB |

- **Comparability check passes**: L1 −0.68%, L4 +0.50% vs A1's
  198,353 nodes/s — both within the ±10% band → A1 admitted as the
  ladder midpoint; the three-point fit is used.
- **η = 1.0081** (log N vs log wall least squares over L1/M2/L4; the
  fresh two-point fit gives the identical 1.0081; max log-residual
  0.0006 — the fit is essentially exact, no anomaly to interpret).
- **Floors monotone**: N_L1 < N_M2 < N_L4 (709.7M < 1,428.6M <
  2,870.9M). Expected values from plan3 (≈ 0.71 G / ≈ 2.9 G) both hit.
- **Rate is slightly increasing, not decaying**: +1.2% from L1 to L4
  (≈ +0.6% per budget doubling) — no churn/overhead decay signal at
  4 h. The only recorded decline remains A2's TT-size effect
  (plan3), not a T-dependence.

Interpretation (the §3 LINEAR branch, verbatim): long sequential
sessions buy proportional work at a stable rate; **R_seq is certified
over 1–4 h at the reference scale**. This is the branch stages 2/3
are priced against. plan9's accumulation question stays live exactly
as registered: LINEAR is about work-per-wall, not about whether any
affordable W closes the (unbounded-from-above) gap to N_finish.

## 2. The measured number (the "bottomless → number" conversion)

- **R_seq ≈ 198,229 nodes/s** (mean of the three arms; spread ±0.6%,
  no trend worth modeling) on the d4d5-p2 root at the reference
  config, certified over 1–4 h.
- **N_floor = 2.87 G nodes** (L4 censoring floor). Carried caveat:
  censoring bounds N_finish from below only; no registered mechanism
  estimates it from above.
- **max-RSS flat at 234.7 MB** across 1 h and 4 h arms (and matching
  A1's 2 h 234.5 MB) — memory is TT-bounded and time-independent at
  this scale; the 8 GiB cgroup has ≈ 34× headroom for plan9's
  checkpoint sizing. No OOM risk from session length per se.
- **TT fill point ≤ 1 h** (§2 secondary question answered): both fresh
  arms' snapshots sit at exactly 4,194,304 keys = the 128 MB table's
  capacity at censoring (L1: 2,368,731 solved + 1,825,573 unsolved;
  L4: 2,370,041 + 1,824,263 — the solved/unsolved split churns ≈
  0.05% between 1 h and 4 h while total occupancy is pinned).
  Checkpoint cadence for plan9 must be well under 1 h to capture any
  pre-churn state; TT keys remain a non-metric for progress (§1
  capacity fact), so plan9's ρ lives on the campaign master proof
  state as registered.

## 3. Stage-1 rows of the §4 horizon table (locked)

T(W) = W / R_best with R_best = R_seq until plan8 measures R_camp
(m2 ≥ 1 would switch R_best to R_camp); single-session vs accumulated
flagging needs ρ (plan9). Rows marked **pending** are stage-2/3
deliverables and intentionally left blank.

| W (nodes) | T(W) at R_seq (198.2k/s) | campaign @ 4 workers (m2, plan8) | 8-worker envelope (12 h sessions) |
| --- | --- | --- | --- |
| 10^14 | 5.04e8 s ≈ **16.0 years** | pending plan8 | pending plan8/plan9 |
| 10^15 | 5.04e9 s ≈ **160 years** | pending plan8 | pending plan8/plan9 |
| 10^17 | 5.04e11 s ≈ **16,000 years** | pending plan8 | pending plan8/plan9 |

Anchors for the pending rows: a 12 h single session at R_seq reaches
≈ 8.6 G nodes ≈ 3× N_floor (the §4 NEGATIVE branch's "single-digit G
nodes" bound, now with the stage-1 constant); the envelope row's
optimistic 2×-linear scaling over that is exactly what plan8's
memory-checked m2 must test (A5.4 prior: campaign < 1× sequential).
The table confirms the initiative's own calibration: even the W =
10^14 anchor is decade-scale at R_seq — the verdict mechanism (ρ ≥ 0.7
and m2 ≥ 1 for POSITIVE) can only move T down by the campaign/stacking
factors, never into affordable range; the user owns the go/no-go call
with these numbers on record.

## 4. Deviations, problems, tools

- **None material.** Both arms ran to their registered budgets
  uninterrupted; no arm was interrupted, so the §7 contingency (no
  re-run) never fired. Compute: 1 h + 4 h + ≈ 5 min analysis ≈
  5.1 h of the 6 h cap.
- `ladder.py env` records `nproc = 6` (host CPUs) while the cgroup
  quota is 4 CPUs (`cpu.max` = "400000 100000") — identical to
  plan3's recorded environment; runs are effectively sequential on
  the quota.
- The solver's `tt_snapshot:` stdout line and the snapshot file agree
  exactly on solved/unsolved counts in both arms (reader
  cross-check, no parsing drift).
- Tools: Python 3 stdlib only (`os.wait4` for ru_maxrss — no
  `/usr/bin/time` in this container); release binaries as built
  (product path unchanged since plan3, confirmed by git log: only
  `examples/` + docs changed; binary timestamp postdates the last
  `src/` commit).

## 5. Artifacts and snapshot pruning

- `measurements/plan7/ladder.py`, `README.md`, `env.json` — harness.
- `state/arm_ladder1.json`, `state/arm_ladder4.json` — parsed runs
  (counts, rates, RSS, TT occupancy, commands).
- `analysis.json` — gate constants, comparability check, η fits,
  verdict, R_seq certification block.
- `logs/arm_ladder{1,4}.*` — raw stdout/stderr (gitignored).
- Snapshots pruned per convention at report time:
  `snaps/ladder{1,4}.tt` → `.tt.keep` markers; the state JSONs carry
  the counts and the README command table reproduces them
  (wall-clock-bounded censored runs → content-equivalent, not
  byte-identical).

## 6. Next steps

Stage 2 (plan8) runs the campaign arms at 4 workers on this root and
delivers R_camp, m2, the nodes↔child-evals conversion, and any floor
extension, with the memory check mandatory (report6 §5.1 OOM
precedent); stage 3 (plan9) then delivers ρ and fires the item-8
verdict with the §3/§4 bands of plan7.md. Stage-1 inputs above are
locked: R_seq = 198.2k nodes/s, N_floor = 2.87 G, fill ≤ 1 h, RSS
flat at 235 MB.

SESSION COMPLETE
- plan7.md executed: harness `measurements/plan7/` (ladder.py, README, env.json), both arms run sequentially, gate **LINEAR** (η = 1.0081, floors monotone, rates ±0.7%), `analysis.json` + `report7.md` written, snapshots pruned to `.keep`. No index row change (initiative stays active; stage 1 of 3).
Follow-up options:
  1. `Execute docs/plans/solve/plan8.md` — stage 2: campaign arms at 4 workers on the d4d5-p2 root (R_camp, m2, node↔child-eval conversion, memory check); plan8 must be drafted first if not yet present.
  2. Alternative: if plan8 is not draftable yet, run its preamble measurements first (4-worker memory feasibility probe) in a short session, per the §7 contingency pattern.
