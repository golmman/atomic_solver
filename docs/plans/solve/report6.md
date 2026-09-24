# Plan 6 — Implementation report: bounded campaign iteration (scheduler/budget policy, same harness, same gates)

Executed 2026-09-24. Deliverables: the registered V3 abandon trigger (the
one campaign-side code change), the §3 arm run with the §4 gates applied,
the A5 attribution amendment (`campaign_architecture.md` §11),
[`measurements/plan6/`](README.md) (drivers, per-rep JSONs, artifacts,
audits), and this report. Product CLI, sequential-path contracts, and the
benchmark/drift gates untouched. Fast gate (`make test`, incl. the new
abandon-sweep unit test) green; clippy/fmt clean.

**Verdict: Not-GO.** SOUND passed end to end; the ECON gate fired **NO-GO**
for the winner (V1, the one-step ladder) — it completes 5/5 reps but at
0.63× sequential wall, failing even the MARGINAL band — and C4′ failed both
scaling criteria. Per the pre-registered semantics, **plan7 is not
draftable**; the initiative's continuation follows the status rule (plan8
sizing run remains the outstanding lever; `solve` falls dormant only when
the iteration *and* the sizing run have both fired negative).

## 1. What was built (D1 — the only code change)

- `examples/campaign/state.rs`: `abandon_in_flight()` — the V3 sweep over
  in-flight jobs at a result-merge boundary (trigger: the job's leaf
  resolved elsewhere, or its root child resolved/refuted), plus the
  cancelled-result drain path (work counted, never merged) and the
  `jobs_abandoned` summary field; `--abandon` gates the whole path so
  non-V3 arms are dispatch-identical to plan5.
- `examples/campaign_worker.rs`: cancel-marker check at claim time (dropped
  un-run, TT retained per the retention contract); a marker arriving after
  the claim is handled by the master's drop path.
- `measurements/plan6/run_arms.py`: plan5's driver with the registered arm
  table (V0/V1/V2/V3), a `rep_from` re-run option, and worker-aware state
  tags (see §5).

## 2. Gate verdicts (pre-registered in plan6 §4)

### SOUND — **PASS**

Zero master `verify_failed` tripwires across every registered rep (all 40
shuffle/m22 arm reps + the S baselines). Six proven rep-0 artifacts
(shuffle V0/V1/V2/V3, m22 V1/V2) pass `inspect_pt --validate` and root
outcome equality with S; dual-check: **144 facts re-derived by the
sequential solver — 0 contradictions, 0 inconclusive**. In-master
`validate: ok` on every proven rep. The 153 job-level export rejections
(146 fill-depth-cap, 7 reconstruct anomalies of the A3 class) were all
caught by the A1 downgrade discipline and never crossed the boundary.

### Winner selection and ECON — **NO-GO**

Pre-registered rule: most proven reps on shuffle; ties by lower median
wall. Final recorded data (2 workers, 5 reps, cap 600 s; S = 45.95 s /
249.5M child evals; proven-rep medians):

| arm | proven | wall (s) | speedup | inflation | jobs_abandoned |
|---|---|---|---|---|---|
| S | 5/5 | 45.95 | 1.00× | 1.0× | — |
| V0 (incumbent) | 2/5 | 51.15 | 0.90× | 1.91× | — |
| **V1 (winner)** | **5/5** | **73.09** | **0.63×** | **2.64×** | — |
| V2 | 3/5 | 48.49 | 0.95× | 1.57× | — |
| V3 | 2/5 | 150.86 | 0.30× | 7.34× | 0 |

- **ECON (V1): GO requires 5/5 ✓, wall ≥ 1.2× ✗ (0.63×), inflation ≤ 3.0 ✓
  → NO-GO.** MARGINAL also fails (wall < 1.0×). The new 5/5-completeness
  clause did exactly its job: V1's extra proven reps are bought with more
  work per rep (median 658M evals; its one slow rep 2.76B), not with wall.
- The winner designation is the re-run data (§5): the first V2 sample led
  4/5 before its record was destroyed by the driver tag collision; the
  re-run (registered record) gave V2 3/5 and V1 5/5. Both samples agree on
  the substance: no variant is simultaneously complete and fast.

### C4′ scaling — **FAIL** (transcript-recovered, no gate hinged on it)

Winner V1 at 4 workers on shuffle: 4/5 proven, proven median 261.6 s
(worse than its own 2-worker 73.1 s), median proven-rep work 6.5B →
inflation ~26× — both criteria (wall ≤ winner@2W, inflation ≤ 3.5×) fail.
The V2@4W sample agrees (4/5, proven median 57.8 s vs its 48.5 s at 2
workers, ~3.95×). plan5's C4 (1.29× / 1.98×) stands as the upper tail of a
high-variance process, not a reproducible operating point.

## 3. Attribution (A5, recorded in campaign_architecture.md §11)

- **V3's abandon trigger is structurally inert on these positions.** Across
  30 reps (V1/V2 on m22; V0–V3 on shuffle) zero jobs were abandoned: proofs
  here come exclusively from an all-replies-won child resolution — no leaf
  ever loses (children refute only via build-time terminal classification)
  and locked leaves cannot be resolved by another worker. V3 ≡ V0
  empirically (2/5 both). A useful abandon trigger would need a different
  deadness signal (e.g. master-side pn bounds) — *not* validated here.
- **Budget ladders price the coverage race, they do not win it.** Every
  censored rep in every arm burns 9–13B child evals (36–52× S) grinding
  non-proof-bearing leaves; the root's ~41 children / 640+ open leaves are
  worked by static priors that do not identify the proof-bearing child.
  V1 (bigger first slices) completes more reps by spending more per rep;
  V2 (smaller first slices) has the best 2-worker economics (0.95× wall,
  1.57× inflation) but stays incomplete. No registered scheduler/budget
  policy fixes the coverage problem itself — hence Not-GO.
- **Retention and the nf (complete-job) shape remain the confirmed wins**
  (plan5); this iteration's negative adds: the slice ladder family cannot
  recover the 2-worker GO band, and 4 workers do not rescue a 2-worker
  losing shape.

## 4. Compute budget (§7)

≈ 3 h 50 m of the 4.5 h hard cap (S 6 m; V0 32 m; V1 23 m + 9 m re-run;
V2 13 m + 22 m re-run; V3 51 m incl. the OOM re-run; m22 arms 6 m; C4′ 43 m;
audits 12 m). The optional d4d5-p2 equal-budget pair was **skipped** (would
have breached the cap at ~1 h for an observation-only, certain-to-censor
row); skip reported per §7.

## 5. Deviations and incidents

1. **V3 rep4 OOM (environment).** The sandbox has an 8 GiB cgroup memory
   limit (`oom_kill 1`); the first V3 rep4 master was killed at ~374 s.
   Re-run clean; no protocol change.
2. **Driver tag collision (my defect).** The first C4′ runs reused the
   2-worker state-file tags, overwriting the first V1/V2 2-worker rep
   JSONs. Fix: worker count in the tag (`<pos>_<arm>w4_rep<k>.json`); both
   2-worker arms re-run in full with the fixed driver. Consequence: the
   committed 2-worker V1/V2 data is the re-run (this is the record the
   gates were applied to); the first V2 sample (4/5) survives only as
   transcript. The C4′ per-rep JSONs were overwritten before the fix and
   are **transcript-recovered** (recorded verbatim during execution); the
   C4′ re-run was skipped under the §7 cap. No gate hinged on C4′ (GO was
   already impossible via ECON).
3. **Arm-order interaction.** §3 places optional V2 last, so the winner
   arms (m22, C4′) first ran under a provisional winner (V1). The final
   winner (V1, per the re-run data) made the m22 row valid; the V2 m22 row
   is attribution data. The provisional C4′-V1 run matches the final
   winner, so no gate was applied to a non-winner.
4. One session-hygiene note: `examples/campaign/state.rs` (~26 KB) predates
   this plan and is not in the AGENTS.md file-size justification list;
   plan7-scope or a cleanup pass should split it (my delta added ~60 lines).

## 6. Tools, problems, unresolved parts, missing tests, next steps

- **Tools**: plan5's `run_arms.py`/`audit.py`/`summarize.py` adapted under
  `measurements/plan6/`; product `inspect_pt --validate` for the audit.
- **Problems encountered**: §5's three; plus the bash-`&` backgrounding
  mistake that wasted one 600 s smoke (invalid, discarded, not a rep).
- **Unresolved**: A3 (job-local fills) unchanged — dual-check compensates;
  the coverage race mechanism (what identifies the proof-bearing child
  cheaply) is now the *measured* open problem, and this iteration's
  registered family is falsified as the fix (A5.4).
- **Missing tests**: unchanged from plan5 (no session-level integration
  tests for the file protocol); the new abandon sweep has a unit test.
- **Next steps**: (1) plan8 (off-sandbox resource-sizing run) proceeds
  independently — it is the initiative's remaining lever, blocked only on
  the hardware-access question; (2) no plan7; (3) if the iteration + sizing
  run are both negative, `solve` goes dormant per the status rule
  (reopeners: mechanism innovation for the coverage race, or a resource
  step-change).

## 7. State hygiene

Per-rep JSONs, `summary.json`, six proof artifacts, audit JSONs and
`env.json` committed under `measurements/plan6/state/`; worker transcripts
and session dirs not committed (convention). The transcript-recovered C4′
rows are flagged in `README.md` §Provenance.
