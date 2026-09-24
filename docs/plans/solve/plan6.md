# Plan 6: Bounded campaign iteration — scheduler/budget policy (same harness, same gates)

Initiative: `solve`. Executes report5 §4's recommendation: **one bounded
iteration** on the recorded diagnosis of the C2 ECON failure before any
plan7 work. plan5's prototype passed SOUND end to end, demonstrated
retention (the load-bearing mechanism, >20× separation vs C2-nr) and
GO-band economics at 4 workers (C4: 1.29× wall / 1.98× inflation, 5/5
proven) — but the 2-worker feedback design point (C2) fired NO-GO
(1/5 proven, 0.76× wall, ~25× inflation), and the recorded diagnosis is
a **scheduler/budget-policy pathology, not an architecture dead-end**.

**Numbering note.** This plan takes the plan6 number. The roadmap's
off-sandbox resource-sizing run (roadmap plan6, blocked on the open
hardware-access question) renumbers to **plan8**; plan7 (campaign
surface + one-quiet-system pilot) is unchanged. The renumbering is
applied to `initiative.md` / `docs/plans/README.md` at this plan's
execution close. The sizing run stays independent of this plan; the two
combine only at plan7's threshold, per the initiative's status rule.

**Scope is bounded by design** (report5 §4): scheduler/budget policy
only, same harness, same gates — no new mechanism families, no product
changes, no re-litigation of retention (settled by plan5's C2-nr
ablation; not re-run). Campaign code only (`examples/campaign*`,
`measurements/plan6/`); the product CLI, the sequential path's
contracts, and the benchmark/drift gates are untouched. Per repo
convention the final task is `report6.md`.

## 1. Background (self-contained)

plan5 (`report5.md`) ran the two-job campaign prototype
(`examples/campaign_{master,worker}`) on two finishable positions
(sequential baselines exist) with five arms:

| arm (shuffle) | proven | wall (median) | inflation | note |
|---|---|---|---|---|
| S | 5/5 | 52.85 s | 1.0× | 249.5M child evals |
| C2 (2 workers, slices `1M→8M` doubling) | 1/5 | 69.2 s | ~25× | 4 reps censored at 600 s, 6.2–8.4B evals each |
| C4 (4 workers) | 5/5 | 40.81 s | 1.98× | GO-band economics |
| C2-nr (no retention) | 0/5 | — | 227× | retention pays decisively |
| C2-nf (complete jobs, no slicing) | 3/5 | 93 s | — | beats C2 |

**The recorded diagnosis** (report5 §4): in the censored C2 reps, 894 of
967 completed jobs were budget-exhausted draws — with only two workers,
both cycled hopeless-leaf slices on the v0 doubling ladder
(`slice × 2^slices`, capped at `max-slice = 8M`) instead of concentrating
on the proof-bearing line, while C4's extra in-flight leaves and nf's
complete-job scheduling got coverage. The proof state, merge contract,
and worker export discipline (architecture doc §11 A1/A2, 0 tripwire
fires post-fix) are all sound; what failed is *which jobs get which
budgets*.

Note on the v0 knob space: with the current dispatch loop, `--nf` is
exactly "flat budget = max-slice, no doubling" — C2 and nf differ *only*
in the budget schedule. The candidate fixes from report5 are therefore:

- **(i) job sizing without the runaway ladder** (the "worker-count-aware
  sizing" direction: bounded or no doubling, so a re-queue is a
  decision, not an exponential commitment), and
- **(ii) the nf shape as default with feedback used only for abandon
  decisions** (complete jobs; the master re-selects between jobs; an
  in-flight job may be abandoned only on a narrow, well-defined trigger).

## 2. Deliverable D1 — registered variants (the only code change)

All variants run the plan5 harness unchanged except where stated.
Positions, flags, reps, caps, metrics, and the audit are identical to
plan5 (`measurements/plan5/README.md`); arms are `--slice 1000000
--max-slice 8000000 --tt-mb 128 --pt-mb 512` unless the variant says
otherwise.

| variant | change | hypothesis under test |
|---|---|---|
| **V0 = nf** (incumbent) | none (re-run same-build) | baseline every variant must beat: (proven count, then median wall) |
| **V1 = one-step ladder** | `--slice 4M --max-slice 8M` | a single budget upgrade step keeps boundary feedback without the exponential commitment; 4M full-sized first slices avoid the 1M-cycling |
| **V3 = nf + abandon trigger** | small dispatch-loop change (below) | complete jobs + narrow abandon recovers coverage *and* responsiveness |

The two-step ladder (`--slice 2M --max-slice 8M`) is registered as
**V2, optional** (run only if the wall budget of §7 is intact when
reached).

**V3 contract (the one registered code change, campaign-side only).**
While a job is in flight, the master may abandon it iff, at a result-merge
boundary, (a) the job's leaf was resolved by the other worker's merged
result, or (b) the job's root child became resolved/refuted. The
abandoned worker keeps its TT (retention semantics unchanged, §5 of the
architecture doc) and picks up the next dispatch; no partial result
crosses the boundary (split contract §2 unchanged — advisory pn/dn from
the abandoned job is dropped, not merged). Master→worker cancellation
follows the §8 message schemas; deterministic budgets everywhere (no
wall clock in any scheduling decision).

Selection of the **winning variant** (pre-registered, in order): most
proven reps on shuffle; ties broken by lower median wall; ties again by
lower median inflation. The winner must beat V0 to count as an iteration
win; if nothing beats V0, V0 is the winner and the iteration verdict is
"ECON of nf at same build".

## 3. Deliverable D2 — measurement protocol

**Positions** (unchanged from plan5): primary `shuffle-win`
(`4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`); sanity
scale `m22_white` (`4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - -
0 22`); exploratory `d4d5-p2` (no gate).

**Arm order** (fixed; strictly sequential; nothing else runs
concurrently):

1. S baselines, both positions, 5 reps (same-build denominators; also
   the plan5 ~10% wall-drift cross-check).
2. V0, V1, V3 on shuffle, 2 workers, 5 reps each, arm cap 600 s.
3. Winner on m22, 2 workers, 5 reps (sanity table; observational).
4. Winner at 4 workers on shuffle (C4′), 5 reps, arm cap 600 s.
5. Optional if §7 budget intact: V2 on shuffle (5 reps); d4d5-p2
   equal-budget pair (S 1800 s vs winner 1800 s, 1 rep, observation
   only — continuity with plan5's exploratory row).

**Metrics** (unchanged): wall speedup vs S (median of 5), work
inflation (aggregate worker child evals ÷ S child evals), proven count
per arm, verify-failure count, worker-log job summaries (per-rep JSONs
committed; transcripts not).

**Soundness audit** (unchanged from plan5): `inspect_pt --validate` on
every proven rep's composed artifact; root outcome equality with S;
dual-check of ≥ 20 sampled decisive facts per GO-supporting artifact
(re-derived by the sequential solver; zero contradictions required).

## 4. Pre-registered gates (fixed before any run)

- **SOUND (hard, unchanged from plan5):** zero false decisive facts end
  to end — every proven rep's artifact validates, 0 master
  `verify_failed` tripwire fires (A2), root outcome equality, dual-check
  clean. **Any violation = NO-GO regardless of economics.**
- **ECON (shuffle, winning variant at 2 workers):**
  - **GO** = 5/5 proven within the 600 s cap AND median wall ≥ 1.2× vs
    S AND median work inflation ≤ 3.0× AND beats incumbent V0. (The
    5/5-completeness clause is new vs plan5: C2's 1/5 proven is exactly
    the ambiguity this iteration must remove — medians over censored
    arms are not evidence.)
  - **MARGINAL** = 5/5 proven, wall ≥ 1.0×, inflation ≤ 5×.
  - **NO-GO** otherwise.
- **C4′ scaling (winner at 4 workers, the registered design point per
  report5):** median wall ≤ winner@2-workers median wall and inflation
  ≤ 3.5×. Required for GO.
- **Attribution:** retention is settled (plan5, not re-run). The
  architecture doc §11 must gain an **A5** recording *which feedback
  shape paid* (V0 vs V1 vs V3, against plan5's C2 data) before any
  plan7 work — an unattributed win yields at best MARGINAL in substance,
  per plan5's rule.
- **Verdict semantics:**
  - **GO** (SOUND + ECON GO + C4′) → **plan7 is draftable** (campaign
    product surface + one-quiet-system pilot to the architecture doc
    §2/§8 spec, with A3's global-prefix fill fix in plan7 scope).
  - **Not-GO** → no plan7. The initiative's continuation follows the
    status rule: the renumbered plan8 sizing run proceeds if hardware
    exists; `solve` falls dormant only when the iteration and the sizing
    run have both fired negative.

## 5. Tasks

1. Rebuild; re-run S baselines (both positions, 5 reps).
2. Implement V3's abandon trigger in `examples/campaign*` (the only
   code change; no product surface); extend `run_arms.py` arm tokens;
   fast gate (`make test`) green.
3. Run the §3 arm order; apply the §4 gates.
4. Optional arms per §3.5 if budget intact.
5. Soundness audit on all proven artifacts + dual-check per §3.
6. Amend `campaign_architecture.md` §11 (A5) with the attribution
   result; record any new incidents (they are expected to be
   campaign-side only — A1/A2 disciplines are in place).
7. Write `measurements/plan6/README.md` (env.json, command table,
   committed per-rep JSONs and artifacts) and `report6.md`: verdicts,
   arms × metrics tables, the attribution statement, deviations, and
   the concrete recommendation (plan7 draft / dormant per status rule).
   Update the `solve` row of `docs/plans/README.md` and the
   `initiative.md` roadmap at close (plan8 renumbering included).

## 6. Non-goals

- No product changes; no new lib-surface additions (plan5's A4 accessor
  covers everything this iteration reads).
- No retention re-ablation; no startpos claims; no quiet-root gates
  (d4d5-p2 stays exploratory, no gate); no EGTB anchoring; no durable
  multi-node store (plan7 scope); no A3 fill fix (plan7 scope).
- No third scheduler mechanism beyond the registered variants — if all
  fire NO-GO, the recorded diagnosis is falsified and that is the
  result (a negative here is a valid completion; the closure block
  reports state, it does not enforce optimism).

## 7. Budget

S 2×5 reps ≈ 10 min; V0/V1/V3 shuffle arms 3 × 5 × ≤600 s ≤ 2.5 h worst
case; winner-on-m22 ≤ 10 min; C4′ ≤ 50 min; audits ≈ minutes. Optional
arms: V2 ≤ 50 min, d4d5 pair ≤ 1 h. **Hard compute cap ≈ 4.5 h** (the
plan5 overrun lesson): if the cap is reached before an arm starts, that
arm is skipped and the skip is reported; started arms finish their
registered reps.
