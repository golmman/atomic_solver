# Report 11 — Low-tax solved-facts carrier: key-only emission, chunk-boundary handoff, admission filter (backlog #12)

**Verdict: REJECT per the pre-registered rule — M2 fails (stress 1.141×,
m22 1.110× vs the ≤ 1.06× ceiling) and H2 fails on stress (1.172×/1.158×)
and dec10 (1.152×/1.088×).** The plan's reason to exist, the report10
wall-tax attribution, is *partially confirmed*: M1 passed decisively (solo
ratios 0.995–1.006× vs ≤ 1.03×; plan10's carrier measured 1.19–1.25×), so
the solo plumbing tax is gone. But the in-race gate failed — and the fresh
A0 control data show the M2 ceiling itself was miscalibrated: the pure
facts-free 4-way contention floor of the current binary is 1.24–1.25×
(R0 on the dec13 A0 rounds), above 1.06 before any facts flag enters. The
residual regression channel the plan named as expected — **TT pollution of
winning basins** — is what actually fails H2. Per the verdict rule, the
**cooperative-portfolio thread closes for good** (second strike); the
shipped plan8/plan9 salt portfolio remains the family's only adopted
lever. There is no plan12.

## What was executed

- **Stage 0 (implementation + drift gate).** `src/facts/` reworked: record
  v2 (byte 9 repurposed to `path_len`), `PATH_LEN_CAP = 24` export
  admission filter, synchronous `FactsExporter` (process-owned `BufWriter`,
  flush every 256 records + pre-exit, silent stop on write failure) wired
  through the new one-method `search::dfpn::FactExporter` observer hook,
  and a threadless `FactsImporter` reading sibling files directly at the
  existing `FactImporter` hook calls (persistent offsets, 1 MiB/call read
  cap, torn-tail retry, corrupt-record skip, ≥1 s rescan cadence). The
  plan10 writer/reader threads, pending queue, and the A2 env arm are
  deleted; the proof-event sender stays `None` on the facts path. Emission
  sites: `emit_proof_node` (frame exit, TT re-proof, terminal-at-entry;
  `path_len = move_stack.len()`) and the terminal-children site in
  `children.rs` (`len + 1`). Gates: fmt/clippy/doc clean, `make test`
  green (274 unit tests incl. 12 new/rewritten facts tests: v2 round-trip +
  CRC rejection, filter boundary at 24/25, buffer/flush/finish, capped
  catch-up, torn tail, corrupt-record skip, rescan throttle, idempotence +
  self-proven store path, export-inertness). **Drift gate clean**: facts
  off ⇒ quick suite 59/59 `child_evals` + outcomes + nodes identical,
  m22 default/FO + shuffle-win FO stdout byte-identical (md5), TT-snapshot
  sha256 unchanged (`95c7d80cd67c2d0f…`, fresh base captured at HEAD
  before the diff).
- **Stage 1 (M1 probes + volume).** 5 interleaved same-binary pairs per
  case (dec13 s0, dec10 s0, m22 s0), plain vs `--facts-out`: **M1 PASS** —
  wall ratios 1.0056 / 0.9953 / 0.9981 (ceiling ≤ 1.03; plan10: 1.19–1.25),
  stdout md5 and `evals:` identical in every pair (H4e at CLI level).
  Volume: dec13 147,752 records / 3.5 MB (distinct-key share 0.419), dec10
  61,152 / 1.4 MB (0.204), m22 208,926 / 5.0 MB (0.282) — all records
  within 0–24 plies by construction. (First pass of this stage produced a
  FAIL at ratios 1.07–1.18; diagnosed as an environmental confound — see
  Deviation 1.)
- **Stage 2 (canary, A1 only — there is no escalation arm).** One race
  each on m22_white and stress with fresh A0 baselines: clean on both
  (m22 `W_first` ratio 0.92, stress 0.99 vs the 2× abort line; zero
  outcome conflicts, zero abnormal exits, all racers within wall caps).
- **Stage 3 (campaign).** 5 rounds × {m20_white, stress, m22_white,
  dec10, rem12} + 3 rounds × dec13 (veto-only), one A0 race and one A1
  race per round, order alternating; stop-when-proven kill policy;
  per-race records in `measurements/plan11/state/`.
- **Stage 4 (analysis).** `analyze_portfolio11.py` → `state/races11.json`;
  M1/M2/H1–H4 evaluated mechanically.

## Gate results (medians over rounds; winnerless = flag, not a sample)

| case | role | A0 evals | A1 evals | ratio | A0 wall | A1 wall | ratio | winnerless A1 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| m20_white | hard, 4 basins | 1,152,505,525 | 662,871,081 | **0.575** | 114.8 s | 69.4 s | **0.601** | 1 (r4, all-censored) |
| stress | hard, 3 basins | 260,688,794 | 305,404,068 | 1.172 | 27.0 s | 31.3 s | 1.158 | 0 |
| m22_white | hard, 6 basins | 16,339,874 | 17,157,744 | 1.050 | 1.87 s | 1.97 s | 1.055 | 0 |
| dec10 | multi-basin | 2,549,882 | 2,937,925 | 1.152 | 0.35 s | 0.38 s | 1.088 | 0 |
| rem12 | out of sample | 61,821,025 | 61,065,683 | 0.988 | 7.20 s | 7.15 s | 0.994 | 0 |
| dec13 (veto-only) | 2 basins | 5,030,497 | 3,892,251 | **0.774** | 0.65 s | 0.53 s | **0.817** | 0 |

m20 A1 decisive rounds: 423.7 M / 442.2 M / 884.2 M / 944.8 M evals
(winners salt 2, 2, 3, 2); r4 winnerless — all four racers budget-censored
at the 2 B cap, recorded censored (the plan10 chaos-tail reproduced at the
pre-registered 1-in-5 rate; ≤ 1 winnerless passes the sharpened H2).

## Hypotheses

- **M1 (solo tax ≤ 1.03×) — PASS** (1.0056 / 0.9953 / 0.9981, with stdout
  and evals identical in all 15 pairs). The report10 solo-tax attribution
  is confirmed for the plumbing part and *revised* for the rest: see
  Deviation 1 — most of plan10's 1.19–1.25× solo tax was a filesystem
  confound, not plumbing.
- **M2 (pure-R0 in-race wall ≤ 1.06× solo, median over A1 rounds where R0
  finishes decisive) — FAIL: stress 1.141× (n=4: 31.27–31.76 s vs 27.55 s
  solo), m22 1.110× (n=1: 2.157 s vs 1.943 s).** Mechanically this is a
  reject input. Diagnostically it is *not* the carrier: R0 is pure (no
  imports), its export is measured near-free solo (M1), and the campaign's
  own **facts-free control** shows the same inflation without any facts
  flag — R0 won all three dec13 A0 rounds at 649–655 ms in-race vs 524 ms
  solo = **1.24–1.25×, zero facts**. The ≤ 1.06 ceiling was transplanted
  from plan8's 1.02–1.05 contention measurement, which predates the
  lean/research engine changes (the same drift report10 recorded for the
  solo anchors: stress s0 249.5 M → 305.4 M evals). On the current binary
  the 4-way contention floor alone exceeds the ceiling, so M2 (as
  pre-registered) measures the environment, not the carrier. The verdict
  rule is applied as written; this diagnosis is recorded, not substituted.
- **H1 (value on ≥ 1 hard-class case with ≥ 3 basins) — FIRED on
  m20_white**, stronger than plan10: 0.575× evals / 0.601× wall (plan10:
  0.661× / 0.817×). The winner's evals collapsed below its solo basin
  minimum again (423.7 M / 441.6 M vs ~1.2 B salt-2 solo), driven by
  near-root imports resolving whole OR/AND subtrees (29.0 k–42.0 k
  accepted imports per winning racer from 2.39 M read records). The
  filter deliberately preserved this channel and it delivered. dec13
  (veto-only) improved to 0.774×/0.817×; rem12 was clean (0.988×/0.994×);
  m22 near-neutral (1.050×/1.055×).
- **H2 (no regression vs A0) — FAILED on stress and dec10** (evals ratios
  1.172 / 1.152; walls 1.158 / 1.088). Both failures are the TT-pollution
  channel, not wall tax:
  - **dec10**: A0's winner is always salt 2 at exactly 2,549,882 evals.
    In A1, imports perturbed the cooperative racers off that basin in 4/5
    rounds (winners salt 3 at 2.75–3.55 M, or salt 2 at 2.55 M once —
    that round was *faster* than A0, 351 ms vs 352 ms). The wall ratio is
    downstream of the basin shift.
  - **stress**: in 4/5 A1 rounds all three cooperative racers were
    perturbed off their 238–250 M basins and the race fell back to pure
    R0's 305,404,068-eval anchor (its exact solo value, H4a intact). The
    one round a cooperator survived its basin, the race *beat* A0 (240.4
    M / 24.8 s vs 260.7 M / 26.9 s). Stress's A1 Σ-racer CPU cost is only
    1.158× A0 — the loss is basin luck, not per-racer tax.
- **H3 (destabilization) — CLEAN.** Zero decisive-outcome conflicts in 66
  races (both arms), zero panics/abnormal exits, zero CRC failures across
  all imports; canaries clean on both cases; no m22/stress collapse
  anywhere near the 2× line.
- **H4 (fidelity/soundness) — PASS.** (a) Pure R0 reproduced its fresh
  same-binary solo salt-0 anchor bit-for-bit in every round where it
  finished (stress r2–r5: 305,404,068 exactly; m22 r5: 19,111,111 exactly;
  dec13 A0 rounds: 5,030,497 exactly) and sat at the budget cap where
  censored solo (m20, killed rounds excluded). (b) Zero outcome
  conflicts. (c) Censoring discipline held (the winnerless m20 r4
  recorded all-censored, never proven draws). (d) Codec/filter invariants
  unit-tested in the fast gate. (e) `--facts-out` alone is byte-inert at
  the CLI level (stage-1 pairs: stdout md5 + evals identical).

**Verdict rule applied verbatim: M2 fails and H2 fails on two cases →
REJECT.** The cooperative-portfolio thread closes for good (second
strike). H1 firing does not rescue it: the mechanism's value channel is
real and now twice demonstrated, but the no-regression clause fails in a
way that is intrinsic to the *sharing* (perturbing cooperative racers'
basins), not to any removable plumbing.

## Where the value came from, and where the cost went

Value (m20, dec13): the filter preserved exactly the near-root channel
that produced plan10's H1, and it delivered more — the m20 winner's wall
dropped to 0.38–0.83× A0 across decisive rounds. The winner accepted
29–42 k imports (comparable to plan10's 24–28 k) from a *much smaller*
delivered stream (2.39 M records read on m20 vs plan10's 2.61 M — and
only 46.0 M emitted vs plan10's unfiltered ≈ 5× re-delivered mass).

Cost: the two channels report10 named are now cleanly separated.
1. **Plumbing tax: eliminated solo (M1), unmeasurable in-race.** The
   carrier's solo cost is ≈0.5% wall at the highest emission rate; in-race
   it is buried under the environment's contention floor (≥ 1.10–1.25×,
   facts-free). The plan10 attribution "the tax is plumbing-shaped" is
   *half*-confirmed: the removable plumbing part is gone; what remains is
   contention, which A0 pays too.
2. **TT pollution: the surviving cost channel, and it is intrinsic to the
   mechanism.** Imports are ordinary solved entries; a cooperative racer's
   trajectory is chaotic by design. On dec10 (sub-second, sharp basin at
   2.55 M evals) even a few thousand imported entries flipped 4/5 races to
   a slower basin; on stress they flipped 4/5 races away from the 238–250 M
   basins entirely. A1 Σ-racer CPU cost vs A0: 0.78× (m20), 1.16×
   (stress), 1.06× (m22), 1.10× (dec10), 0.99× (rem12), 0.80× (dec13) —
   the CPU is not the problem; basin luck is.

## Facts-channel accounting (campaign)

| case | emitted (pass rate) | read | accepted | crc_fail |
| --- | --- | --- | --- | --- |
| m20_white | 46.0 M (0.754) | 2.39 M | 268,450 | 0 |
| stress | 11.0 M (0.982) | 248 k* | 28,690 | 0 |
| m22_white | 1.02 M (0.944) | 578 k* | 54,807 | 0 |
| dec10 | 294.9 k (0.996) | 407 k* | 20,002 | 0 |
| rem12 | 5.75 M (0.986) | 1.14 M* | 140,220 | 0 |
| dec13 | 329.9 k (1.000) | 367 k* | 43,653 | 0 |

\* undercounts: only racers that finish (winners) print their
`facts_import:` line; killed racers' imports are invisible to the driver —
same caveat as plan10. Filter pass rates 0.75–1.0: the deep-path mass the
filter removes is exactly the mass plan10 delivered fivefold (m20's 0.754
is the one case where the near-root region itself is huge). Distinct-key
shares at stage 1: 0.204–0.419 (plan10: 0.19–0.21) — less re-delivery
noise per useful record.

## Deviations (recorded; none mid-campaign)

1. **Facts-file location: repo tree → tmpfs (stage-1 discovery, before any
   race ran).** The first stage-1 pass (kept as
   `state/probes11_overlayfs_confound.json`) measured M1 ratios of
   1.07–1.18 with facts files inside the repo tree — and failed the ≤ 1.03
   gate. Diagnosis before any verdict: the confound is the *filesystem*,
   not the carrier — writing the facts file to the overlay-backed repo
   mount blocks on backing-store writeback (identical `task-clock` 534 vs
   537 ms; context-switches 1 → 886; the identical tax reproduces with the
   plan10 binary), while tmpfs shows ratio 1.01. Facts dirs are per-race
   and ephemeral, so the harness now puts them under `/tmp/plan11_facts`
   (`FACTS_TMP` override; documented in `measurements/plan11/env.json`).
   No mechanism, racer-set, cap, filter, or read-cap change. **Corollary
   finding (report10 record revision): plan10's stage-1 solo tax
   (1.19–1.25×) was measured with facts files in the same overlay-backed
   logs/ tree — most of it was this filesystem confound, not the
   plumbing; the plan10 *campaign* tax (multi-second races, MB-scale
   files) contains a smaller share of the same confound plus the
   thread contention that was the diagnosis there.** The campaign here ran
   entirely on tmpfs facts dirs.
2. **H4a evidence rule (analysis script, post-campaign).** The first
   analysis pass flagged H4a as failed because cases where pure R0 never
   finished have an empty evidence list; fixed to a vacuous pass (killed
   racers never print their evals line — evidence exists only where R0
   wins). No verdict input changed: H4 passes under both readings, and
   every R0 eval that exists matches its anchor exactly.
3. No mechanism, racer-set, cap, admission-filter, or read-cap changes
   occurred; no escalation arm exists.

## Problems encountered

- The M2 ceiling (≤ 1.06) was calibrated on plan8's contention
  measurement from an older binary; the current binary's facts-free
  contention floor is 1.10–1.25×. Any future in-race wall gate on this
  host must re-baseline the contention floor in the same session (the
  report10 stale-anchor lesson extends to contention anchors).
- dec10 is a new sensitive point for TT pollution: its 2.55 M-eval
  winning basin is razor-sharp, and even 20 k accepted imports flip it.
  plan10 did not see this (its dec10 A1 rounds all reproduced the basin —
  its imports perturbed *wall* there via threads, not basins).
- The winnerless m20 tail reproduced (1/5 rounds), matching plan10's
  1-in-5; the sharpened H2 clause (≥ 2 fails) did not fire.

## Missing tests / gaps

- Still no cross-process end-to-end integration test (carried over from
  report10; the campaign exercised the 4-process shape via the drivers).
  A cheap `tests/facts_smoke.rs` (two solver processes, one tmpfs dir,
  assert the importer's outcome) would pin the v2 wiring.
- The silent-stop-on-write-failure path is implemented but not exercised
  by a test (needs an injectable I/O fault).

## Backlog / index updates

- `parallel` backlog **#12: closed NO-GO** (this report); row added and
  closed in `parallel/initiative.md`; the Status section records the
  thread closure (second strike).
- `docs/plans/README.md` `parallel` row updated (close event).
- #7 (any-worker-can-finish SPDFPN) is the only open item in the
  initiative; its premise is unchanged.

## Next steps

The cooperative-portfolio family is closed: plan2/5b/6/7/10/11 join as
measured non-adopted parallelism mechanisms, and the shipped plan8/plan9
salt portfolio (`examples/portfolio`) remains the family's only adopted
lever. The m20 H1 result (0.575× evals / 0.601× wall) is preserved in the
record as the demonstrated upper bound of what cross-racer fact sharing
can deliver on multi-basin hard cases; any revival would need a
pre-registered mechanism that does not perturb cooperative racers'
basins (e.g., a read-only facts consumer with a frozen trajectory) — that
is a new initiative, not a plan12.

## SESSION COMPLETE

- Deliverables: plan11 executed end to end — carrier shipped (`src/facts/`
  v2 rework + `search::dfpn::FactExporter` hook + `--facts-out`/`--facts-in`
  wiring; drift gate clean, `make test` green, fmt/clippy/doc clean);
  `measurements/plan11/` complete (drivers, `state/*.json` incl.
  `races11.json` + the confound record, `env.json`, `README.md`);
  `report11.md` written; backlog #12 + `initiative.md` Status/History +
  `docs/plans/README.md` row updated; AGENTS.md facts bullet updated.
  Gate: `make test` green; drift gate clean (quick 59/59, stdout md5s,
  TT-snapshot sha unchanged).
- Follow-up options:
  1. `Close the parallel initiative's cooperative-portfolio thread in the
     status index as final (done in this session) and evaluate backlog #7
     (any-worker-can-finish SPDFPN) — the only open item — against the
     plan8 distribution data before committing to a build` — the
     conservative default given the second-strike closure.
  2. `Open a new initiative for a read-only facts consumer (frozen
     trajectory, no TT pollution of the consumer's own basins) targeting
     the m20_white conversion (0.575×/0.601× measured twice)` — only if
     the owner weights the demonstrated m20 value channel above the
     closure decision; this is a new initiative, not plan12.
