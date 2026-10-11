# Research Report 20 — Item #20 (salted TT-keeping restarts): **Phase-0 PIVOT, item closed**

Executed 2026-10-11 per [`plan20.md`](plan20.md). **The pre-registered kill
gate K1 (retention) fired on every stress-class probe before any rollout
run: at the occupancies where the lever is supposed to act (~80–95 % full
at 128 MB), a reseed retains only 76.9–83.8 % of live TT entries — below
the 85 % live-retention floor — so the "TT-keeping" premise is materially
violated and #20 closes at Phase 0 (PIVOT), with the retention table as
the no-go record.** No Phase-1 grid was executed; no verdicts were read;
the hook is reverted and the tree is clean.

The mechanism itself is sound and live — every fidelity anchor passed
(c0 liveness, c1 env-unset quick byte-identity, c2 determinism, c3 prefix
identity, c4 side-band neutrality, d outcome invariants, e cyclic-rook
invariant) — and at low occupancy the reseed is essentially lossless
(≥ 99.5 % live retention up to ~18 % occupancy). The failure is
structural physics, not implementation: re-placing N entries into 2-slot
buckets under a fresh index function drops every arrival that hits a full
bucket (the incumbent eviction policy discards the victim, exactly as a
normal store does), and at occupancy α the per-bucket arrival count is
~Poisson(2α), so E[min(arrivals, 2)]/live ≈ 74.7 % at α = 0.947 under a
uniform index — the measured 79.4 % is slightly *better* than that model
(imperfectly uniform index + the work/solved victim priority), i.e. the
loss sits at the physics of capacity-2 rehashing under the pinned
mechanism, not at the edge of implementable behavior. A retention-gated
reseed (fire only below some occupancy) would be a different lever and is
out of scope; it is recorded below as the one surviving shape.

## Deliverables

1. **The hook** (temporary, reverted): `TranspositionTable::reseed(new_salt)`
   — set the salt, drain every valid entry in native bucket order, clear,
   re-place each through the normal insertion path (`insert_new`) under the
   new index; overflow resolved by the incumbent eviction policy;
   generations preserved; never invents values, never writes to a key's
   wrong bucket — plus the `Search::reseed_at_boundary` call site in the
   `bounded_search` chunk loop (fires exactly where the loop continues,
   i.e. after a full-work chunk; natural-exhaustion, decisive, and
   resource-cut exits and the final partial chunk never reseed), the
   `ATOMIC_RESEED = rs1 | rsg` env gate (read once at `Search::new`;
   unset ⇒ bit-identical), and the `ATOMIC_RESEED_STATS=1` side-band
   (`reseed: k=<i> salt=<s> live A→B solved C→D unsolved E→F`). Salt
   sequence `s_{k+1} = s_k + 1` (wrapping). History/killers and the
   repetition cache untouched. Memory note recorded in the hook docs: the
   spike stages the drained entries in a transient Vec (~100 MB at 128 MB /
   ~95 % occupancy) — a productized reseed would need bounded extra space.
2. **D3(a) pristine identity, full set** (unhooked build at HEAD `546bbcc`,
   salt 0, plan12 budgets): stress 249,480,478; m22_white 14,156,269
   (stdout md5 `ea72f7ea…`); m23_white 9,673,403; dec13 3,822,602; dec10
   4,262,128; m20_white budget-censored at 2 B (evals 2,000,000,027) —
   all exact.
3. **Fidelity-anchor chain on the hook build** (liveness first), all green
   ([`measurements/plan20/env.json`](measurements/plan20/env.json)).
4. **Phase-0 probe set** (retention + effect probes at the canonical caps).
5. **Kill-gate outcome**: K1 FIRED → PIVOT; K2/K3 pass (recorded for
   completeness).
6. **Revert verified**: hook deleted, `git diff --exit-code` clean; release
   build + examples re-verified; (a)/(c1)-class anchors re-run exact
   post-revert (stress 249,480,478; m22 14,156,269 with stdout md5
   `ea72f7ea…`; quick total 38,974,090);
   `cargo test --release --test test_repetition -- --include-ignored`
   green; `cargo clippy --release --all-targets` and `cargo fmt --check`
   clean; 253 lib tests green on the hook build before the probes.
7. This report; the `reexamination.md` §6 step-4 bullet annotated; the
   `initiative.md` #20 row closed with history entry; the
   `docs/plans/README.md` research row refreshed.

## D3 soundness and fidelity record

| check | result |
|---|---|
| (a) pristine pre-hook salt-0 identity | exact on the unhooked build at HEAD `546bbcc` (full 6-anchor set re-run by this session; see Deliverable 2) |
| (b) baseline-arm per-salt identity | **not applicable** — no Phase-1 rollout ran (K1 fired first); the baseline arm was never started |
| (c0) liveness first | `ATOMIC_RESEED=rs1` salt-0 stress: **9 reseed events**, evals 278,362,531 ≠ baseline 249,480,478, outcome `win` preserved — on the same binary that produced every probe number |
| (c1) env-unset quick byte-identity | hook build at default settings: total 38,974,090, **0 per-case diffs across all 59 cases** vs the fresh unhooked step-0 run (which itself matched plan7's `quick_clean.json` 59/59 before the hook build) |
| (c2) determinism | stress s0 rs1 rerun ×2 (side-band on) and ×2 (off): stdout and `evals:` bit-identical; stderr bit-identical with wall-derived fields (`elapsed=`, `nps=`) normalized |
| (c3) prefix identity | rs1: chunk 1 bit-identical to the pristine baseline (work_done 500,002, nodes 24,732, max_depth 15), divergence begins exactly at the first reseed. rsg: **chunks 1–7 bit-identical** (cumulative after the 6th chunk = 31,500,073 — the pre-registered "31,500,xxx class"; the first reseed fires after the 32 M chunk), divergence begins exactly at the first post-reseed chunk (64,000,015 vs pristine 64,000,012) |
| (c4) side-band neutrality | stress s0 rs1 and s0 rsg, each with and without `ATOMIC_RESEED_STATS`: identical stdout, `evals:`, and time-normalized stderr; side-band lines bit-identical across reruns. Retention numbers were read only after this passed |
| (c5) adopted-claim sub-study | recorded inapplicability (the reseed asserts nothing; every moved/dropped entry carries values the search computed exactly) |
| (d) outcome invariants | every uncensored probe run won (fixture `expected`); the one censored probe (stress s2 rs1, budget 2.5 B) reported the no-result draw and was never read as proven; no outcome flip observed anywhere |
| (e) cyclic-rook invariant | `8/8/8/8/2k5/8/8/4KR2 w - - 0 1` with `--no-preflight` never claims `Win` under rs1 or rsg (both censor at the 600 s wall with `draw` — a base-arm property, not arm-specific) |
| (f) revert | hook deleted, `git diff --exit-code` clean, anchors re-run exact, `test_repetition --include-ignored` green |

## Phase-0 retention table (the K1 no-go record)

Retention = after/before per reseed side-band line; occupancy = live
entries / 4,194,304 slots (128 MB table). Full tables incl. solved/unsolved
splits in [`measurements/plan20/env.json`](measurements/plan20/env.json).

**stress s0 `rs1`** (9 reseeds on the reseeded trajectory):

| k | occupancy before | live retention | solved retention |
|---|---|---|---|
| 1–4 | 0.5–8 % | 99.6–100 % | 100 % |
| 5 | 16.6 % | 98.7 % | 99.98 % |
| 6 | 31.6 % | 95.7 % | 99.86 % |
| 7 | 55.2 % | 90.7 % | 99.56 % |
| **8** | **80.8 %** | **79.8 % ← K1** | 98.72 % |
| **9** | **94.7 %** | **79.4 % ← K1** | 98.03 % |

**stress s1 `rs1`** (9 reseeds): k=8 at 79.9 % occupancy → **83.8 % ← K1**;
k=9 at 94.8 % → **76.9 % ← K1**; earlier k ≥ 88.3 %.

**stress s0 `rsg`** (3 reseeds): k=1 at 55.8 % → 87.8 %; **k=2 at 80.4 %
→ 81.4 % ← K1**; **k=3 at 94.5 % → 79.7 % ← K1**.

**m22 s0 `rs1`** (5 reseeds, low-occupancy control): ≥ 98.5 % live
retention everywhere — no K1 fire; the mechanism is harmless where the
table never fills.

**K1 verdict: FIRED on every stress-class retention run** (three of three;
multiple reseeds each). The loss cliff sits between ~55 % and ~80 %
occupancy. Solved-entry retention stays ≥ 97.6 % everywhere — the
incumbent `(live, solved, work, generation)` policy protects solved
entries well; the dropped working set is unsolved bounds, which is
precisely the search's frontier. Per the pre-registered reading, a
mechanism that drops 15 %+ of its working set per boundary is a
partial-clear restart, not a TT-keeping restart.

## Phase-0 effect probes (single-draw context, not verdicts)

Side-band off, canonical caps; baseline evals from plan12:

| run | arm evals | baseline | ratio | outcome |
|---|---|---|---|---|
| stress s0 rs1 | 278,362,531 | 249,480,478 | 1.116 | win |
| stress s1 rs1 | 377,098,055 | 758,176,477 | **0.497** | win |
| stress s2 rs1 | censored 2.5 B | censored 2.5 B | — | censored draw |
| stress s0 rsg | 297,633,350 | 249,480,478 | 1.193 | win |
| stress s1 rsg | 296,123,157 | 758,176,477 | **0.391** | win |
| stress s2 rsg | 334,292,550 | censored 2.5 B | solves | win |
| m22 s0 rs1 | 19,839,932 | 14,156,269 | 1.402 | win |

K2 (liveness/effect) passes: every probe diverges from its baseline with
outcomes preserved. K3 passes: all uncensored probes won; all parses
clean. The readings are mixed in both directions (the redrawn collision
luck is real: stress s1 improves ~2–2.5×, s2-rsg solves a baseline-censored
salt; s0 and m22 regress) — but with the retention cost structurally
bounded the way K1 found it, the lever's premise fails exactly where it
was supposed to pay, and no verdict machinery was warranted.

## Reseed-count profile

Baseline-trajectory counts (from pristine chunk lines) confirm the D7
pre-registration exactly: stress s0 = 8 boundaries (`rs1` 8, `rsg` 2 —
fires at work_done 32,000,000 and 64,000,012), m22 s0 = 4 boundaries
(`rs1` 4, `rsg` 0 — the built-in null control). Reseeded-trajectory counts
(probe runs): stress s0 rs1 9, s1 rs1 9, s0 rsg 3; m22 s0 rs1 5 — a
reseeded trajectory's own boundary count shifts with its diverged work, as
expected.

## Verdict tables and paired-ratio dispersion

**None produced.** The D4/D5 machinery (`verdicts.py`, pre-registered and
committed) was never exercised: K1 closes the item at Phase 0, before any
rollout cell exists. Consequently the comparison rule's **sixth** real-arm
calibration data point was *not* produced, and the D7 dispersion
prediction (exact-null mass ≥ 40 %, frac-outside [0.9, 1.1] ≈ 20–50 %) is
**untestable** — recorded as such. What D7 *did* predict correctly was the
mechanism-cost direction: "the retention loss … works against both arms at
high occupancy" — that cost turned out to be a Phase-0-fatal 15–23 % loss
per boundary at ≥ 80 % occupancy, which is why the plan's K1 gate existed.

## The one surviving shape (recorded, not opened)

The retention table localizes the failure precisely: reseeds are
essentially lossless below ~50 % occupancy (≥ 90.7 % live retention up to
55 %) and catastrophic above ~80 %. A retention-gated variant — reseed
only while the table is below some occupancy threshold, i.e. early-run
redraws only — would keep the premise intact but forfeits the lever's
point: the collision-luck tails that motivate #20 (stress 249 M → 758 M →
censored, plan12/plan19) are late-run, high-occupancy phenomena, and m22's
4-boundary profile shows the early chunks are exactly where redraws change
least. Early-only reseeds reduce to re-rolling the first few chunks' tiny
collision basins — expected-value-neutral by the same heavy-tail argument
that motivates the lever. This is recorded as mechanism understanding for
any future reopening (it would need a new pre-registered plan), not as an
open item.

## Problems encountered

1. `/usr/bin/time` does not exist in this container — the first pristine
   probe script silently produced empty transcripts; caught on inspection
   and re-run with bash timing (no anchor was read from the failed batch).
2. The stderr determinism/neutrality comparisons needed wall-derived-field
   normalization (`elapsed=`, `nps=` in chunk lines): the first diff
   showed false mismatches before the fields were identified. All c2/c4
   conclusions are on stdout + `evals:` + time-normalized stderr.
3. No other issues: the hook compiled clean (clippy/fmt), all 253 lib
   tests passed on the hook build, and the anchor chain ran in the
   pre-registered order without a single HALT.

## Missing tests

- No product change remains, so none are required. The three temporary
  `reseed` unit tests added with the hook (probe-reachability under the
  new salt, determinism/idempotence, overflow-resolved-by-eviction) were
  reverted together with it; the pre-registered D6 landing contract for a
  future *adopt* (which did not occur) would have added permanent rehash
  fixtures to `src/search/tt/tests.rs`.
- If a retention-gated or bounded-extra-space reseed is ever proposed, the
  transient-Vec memory note in the hook docs is the starting point for the
  productization design.

## Next steps

Item **#20 is closed** (PIVOT at Phase 0; `reexamination.md` §6 step 4
gains the gate-grade pointer). Open reopened-backlog items: **#21
in-context child results (dec10 class)** and **#22 fringe** — the last
two.

SESSION COMPLETE
- report20.md written; env.json + README.md + pre-registered drivers
  committed under measurements/plan20/ (no state/ — no rollout); hook
  reverted (`git diff --exit-code` clean, anchors re-exact,
  test_repetition green); reexamination.md §6 step 4 annotated;
  initiative.md #20 row closed + history entry; docs/plans/README.md
  research row refreshed; #20 closed at Phase 0 (K1 PIVOT)
Follow-up options:
  1. `Execute research item #21 (in-context child results, dec10 class):
     draft the one-goal plan for keeping a child's just-proven
     repetition-dependent Draw in the parent frame instead of re-reading
     the TT's (1, n) entry — reexamination.md §6 step 5 puts dec10's
     38 % stale-follow share as the sizing basis.` — the larger of the two
     remaining open items, and the only one with a measured mechanism
     target.
  2. `Execute research item #22 (fringe): draft the plan for the
     rule50-ignorant seeding table and/or Kaneko root threads per
     reexamination.md §6 step 6.` — smaller, more exploratory; closes the
     reopened backlog entirely if run last.
