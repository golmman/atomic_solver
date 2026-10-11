# Research Report 21 — Item #21 (in-context child results, dec10 class): **REJECT, item closed**

Executed 2026-10-11 per [`plan21.md`](plan21.md). **Phase 0 passed every
kill gate (the mechanism is live, sound, and captures 100 % of the measured
stale mass), the full pre-registered Phase-1 rollout ran (264 runs, all
soundness/fidelity invariants green, baseline identity 132/132 cells), and
the pinned-v1.0 verdict applied verbatim is REJECT: the sole negative is a
deterministic dec02 regression (1.117× on all six salts), which vetoes
under D4 ("any regression ⇒ reject") — despite a stress censored-win, an
m20_white censored-win, and a strong dec10 win (median 0.54×, all six
salts below 1) on the lever's own target class.** The hook is reverted;
`git diff --exit-code` clean; no product change.

The item's own framing anticipated exactly this asymmetry: dec10 (basin
count 4) is the repetition-dominated class the lever fixes — and it
delivers there (~1.9× median work reduction, H1's directional expectation
confirmed) — but D4 cannot adopt on a non-hard-class win alone, and the
hard-class support it does have (stress/m20 censored-wins) is vetoed by the
dec02 regression. The D7 pre-registered sparse-effect shape is **met** for
the first time since plan16 (63.8 % exact-null ≥ 40 %; 21.3 % frac-outside
within the 10–40 % band) — the mechanism hypothesis ("effects concentrate
exactly where the rep-cache holds entries") was correct, unlike plans
17/18/19 whose spread predictions failed. m22's pre-registered H2 wildcard
also fired: salt-5 explodes to 6.50× (the `(INF, 0)` in-context bounds
distorting threshold carry-over on a redrawn trajectory), while its other
five salts sit at 0.80–1.40×.

## Deliverables

1. **The hook** (temporary, reverted): `Search::inctx_probe(child_key,
   child_context)` — env gate `ATOMIC_INCTX=1` read once at `Search::new`;
   probe of the plan9 repetition cache at `evaluate_child`'s unsolved site,
   ordered exactly like `dfpn` (TT resolved → terminal classification →
   rep-cache probe → unsolved bounds → `(1, 1)`; the hit is not
   depth-guarded, matching `dfpn`'s frame-entry probe). On a hit:
   `ChildInfo { outcome: Some(Draw), (pn, dn) = Draw.pn_dn_for(child_is_or),
   depth = cached_depth, repetition_seen: true, explored: false }` —
   `repetition_seen` keeps the parent's own first-player-loss suppression
   correct (load-bearing, as pre-registered). Child context derived O(1)
   per `dfpn` frame as `context_hash + mix(rep_key)` (both loop-invariant
   locals) and passed to `evaluate_all_children`/`evaluate_child` (one
   extra `u64` parameter; no `Search` field, no per-probe stack recompute).
   Side-band `ATOMIC_INCTX_STATS=1` → `inctx: probes=<n> hits=<m>` on
   stderr (off during all measurement runs; counters increment whenever the
   hook is on). The rep-cache itself untouched. A hit consumes no
   child-eval budget beyond the `evaluate_child` call it answers.
2. **D3(a) pristine identity, full set, re-run by this session on its own
   build** (unhooked, salt 0, plan12 budgets): stress 249,480,478; m22
   14,156,269; m23 9,673,403; dec13 3,822,602; dec10 4,262,128; m20
   budget-censored at 2 B (evals 2,000,000,027, no decisive outcome) — all
   exact.
3. **Fidelity-anchor chain on the hook build** (liveness first), all green
   ([`measurements/plan21/env.json`](measurements/plan21/env.json)).
4. **Phase-0 probes**: liveness, 6 paired effect probes, and the coverage
   quantification with the temporary C8 instrumentation (below).
5. **Phase-1 rollout**: 264 runs at ≤ 3 concurrency (base arm first,
   identity-checked 132/132 vs plan12, then the candidate arm); raw
   transcripts not committed; parsed state committed.
6. **Verdicts** per pinned v1.0 verbatim → REJECT (tables below).
7. **Revert verified**: hook deleted, `git diff --exit-code` clean; release
   build + examples re-verified; (a)/(c1)-class anchors re-run exact
   post-revert (stress 249,480,478; m22 14,156,269; m23 9,673,403; dec13
   3,822,602; dec10 4,262,128; quick total 38,974,090, 0/59 per-case
   diffs); `cargo test --release --test test_repetition -- 
   --include-ignored` green; `cargo clippy --release --all-targets` and
   `cargo fmt --check` clean; 251 lib tests green on the hook build before
   the rollout.
8. This report; the `reexamination.md` §6 step-5 bullet annotated with the
   gate-grade pointer; the `initiative.md` #21 row closed with a history
   entry; the `docs/plans/README.md` research row refreshed.

## D3 soundness and fidelity record

| check | result |
|---|---|
| (a) pristine pre-hook salt-0 identity | exact on the unhooked build at HEAD `ad900b3` (full 6-anchor set re-run by this session; also pre-verified in the plan session at `5f2b210`) |
| (b) baseline-arm per-salt identity | **132/132 cells exact** vs plan12 (evals AND censoring status; the null is degenerate — any mismatch would have been an identity failure, none occurred) |
| (c0) liveness first | `ATOMIC_INCTX=1` salt-0 dec10 with the side-band: **hits = 340**, evals 2,200,657 ≠ baseline 4,262,128, outcome `win` preserved — on the same binary that produced every probe/rollout number |
| (c1) env-unset quick byte-identity | hook build at default settings: total 38,974,090, **0 per-case diffs across all 59 cases** vs the archived plan4 `quick_clean.json` |
| (c2) determinism | dec10 s0 inctx ×2: stdout and `evals:` bit-identical |
| (c3) context fidelity | `ATOMIC_INCTX_ASSERT=1` instrumented probe (dec10 s0): **zero assertion failures across ~2.13 M probes**, evals identical to the non-assert run; plus the throwaway unit test `evaluate_child_inctx_hit_returns_child_frame_depth` (green): a *real* stored rep-draw child — captured by a temporary `cfg(test)` recorder at the plan9 frame-store site — replayed parent-side through `evaluate_child` (hook on, assert on) under the exact ancestor context returns `Draw` + `repetition_seen` + the child frame's proven depth (`(INF, 0)` AND-side), vs a pristine-engine hook-off `(1, 1)` control |
| (c4) side-band neutrality | dec10 s0 inctx with/without `ATOMIC_INCTX_STATS`: identical stdout, `evals:`, and time-normalized stderr outside the stats line |
| (c5) adopted-claim sub-study | recorded inapplicability (the hook asserts nothing new; every hit returns a value the search itself proved under the exact same context) |
| (d) outcome invariants | all 264 rollout runs + all probes: zero decisive-outcome conflicts within any arm across salts; every uncensored run matches its fixture `expected`; a budget-censored draw is never read as proven. No outcome flip anywhere |
| (e) cyclic-rook invariant | `8/8/8/8/2k5/8/8/4KR2 w - - 0 1` with `ATOMIC_INCTX=1`, `--no-preflight`: censored at the 600 s wall with `draw` (evals 3,601,198,043) — never claims `Win` |
| (f) revert | hook deleted, `git diff --exit-code` clean, anchors re-run exact, `test_repetition --include-ignored` green |

## Phase-0 coverage quantification (the 38.1 % mass)

Temporary C8 instrumentation (the reexamination's counters, from
`measurements/reexam/probe.patch`) was applied on top of the hook for this
probe only and removed before the rollout. On dec10 salt 0:

| | hook off | hook on |
|---|---|---|
| `child_evals` | 4,262,128 | 2,200,657 |
| `stale_after_return` | **10,956** | **0** |
| `stale_evals_after` | **1,624,604** (38.1 %) | **0** |
| probes / hits | — | 2,134,858 / 340 |

The hook-off numbers replicate the reexamination's `r1_dec10` record
**exactly**, confirming both the probe-site placement and the C8 counter
placement. With the hook on, the entire measured stale mass disappears:
the cache route captures **100 %** of the C8 class (and its pre-registered
superset — the `evaluate_all_children` first-sweep surface — is included by
construction, since all production reads funnel through `evaluate_child`).
The elimination of the stale re-price loop is what halves dec10's work:
only 340 cache hits occur, but each one short-circuits the re-select /
re-descend / re-evaluate-stale cycle until threshold exhaustion.

## Phase-1 verdict table (D4, pinned v1.0 verbatim)

Candidate arm `inctx` vs fresh `base` (baseline = plan12 bit-exact). All
22 cases solved `win`/`loss` per fixture on every uncensored run in both
arms; censoring pattern per salt below. Basin counts from plan12
calibration.

| case | verdict | basins | censoring (base/cand) | paired ratios (salt order) |
|---|---|---|---|---|
| m20_white | **censored-win** | 4 | 2/0 | 0.546, 1.024, 0.976, 1.079 |
| stress | **censored-win** | 3 | 3/0 | 0.833, 0.376, 0.859 |
| m22_white | unclear | 6 | 0/0 | 0.801, 0.846, 0.906, 1.171, 1.398, 6.501 |
| m23_white | unclear | 5 | 0/0 | 0.985, 1.0, 1.0, 1.0, 1.0, 1.009 |
| dec01 | unclear | 6 | 0/0 | 0.910, 0.944, 0.966, 0.974, 0.999, 1.098 |
| dec02 | **regression** | 1 | 0/0 | 1.117 ×6 (deterministic) |
| dec06 | **win** | 1 | 0/0 | 0.831 ×6 (deterministic) |
| dec10 | **win** | 4 | 0/0 | 0.420, 0.514, 0.516, 0.564, 0.567, 0.707 |
| dec15 | unclear | 1 | 0/0 | 0.986 ×6 |
| dec13, dec14, dec03–05, dec07–09, dec11, dec12, dec16–18 | unclear (all-null: bit-identical) | 1–3 | 0/0 | 1.0 ×6 each |

- **m20_white**: the hook solves both baseline budget-censored salts (2 B)
  uncensored; of the four shared-uncensored salts, one improves strongly
  (0.546), three sit at 0.98–1.08.
- **stress**: the hook solves all three baseline budget-censored salts
  (2.5 B); shared-uncensored salts at 0.376–0.859 (all < 1).
- **m22_white** (the pre-registered H2 wildcard): three salts improve
  (0.801–0.906), two worsen mildly (1.171, 1.398), salt 5 explodes 6.50× —
  the `(INF, 0)` in-context bounds change the threshold carry-over
  arithmetic adversely on that redrawn trajectory (H2's stated mechanism).
- **dec02**: deterministic 1.117× (138,124 → 154,349 child evals on every
  salt; a tiny position where the hook's one trajectory shift costs 11.7 %
  work). This is the D4 veto.
- **dec10** (the target class): every salt improves, median 0.54 — H1's
  directional expectation confirmed with direction agreement.
- **dec06**: deterministic 0.831× win.
- **dec13/dec14**: bit-identical across all salts — the null shape here is
  "fires without trajectory effect or does not fire"; not further probed
  (its reexamined C8 share is 0.7 %, and no verdict depends on it).

**Lever verdict: REJECT** (dec02 regression vetoes; the rule admits no
exception, and no censored-loss occurred anywhere). Hard-class support
existed (m20_white, stress censored-wins, both basins ≥ 3) but cannot
override a negative under the pinned rule. No D5 extension applies (that
path is for defers; this is a recorded reject with its tables).

## Paired-ratio dispersion (the comparison rule's sixth real-arm calibration data point)

127 paired ratios (5 salts-pairs excluded by symmetric censoring):
min 0.376, p10 0.831, p25 0.999, median 1.000, p75 1.000, p90 1.000,
max 6.501; **exact-null mass 63.8 %**; **frac-outside [0.9, 1.1] = 21.3 %**.

**D7 prediction fate: MET** — the pre-registered sparse-effect shape
(exact-null ≥ 40 %, frac-outside ≈ 10–40 %) holds, and the only
directional expectation (dec10, H1 median < 1) landed exactly. First
confirmed dispersion prediction since plan16 (plans 17/18/19 falsified
theirs); plan20 pivoted at Phase 0 and recorded none. The mechanism-level
reason this prediction held: the hook fires only where the rep-cache holds
an entry for the probed `(child_key, context)` — repetition-region work
only — so everywhere else the trajectory is bit-identical.

## D7 calibration question

The sole negative is a dec02 regression whose all six ratios are 1.117 —
inside the pinned quiet band ([0.760, 1.316]). Recorded as a
methodology-calibration question for the owner (the rule's premise —
single-draw comparisons below the noise floor — vs a *deterministic*
six-salt regression at 1.117×), not applied as a rule.

## Problems encountered

1. **edit-tool atomicity**: a multi-edit call failed on one block match and
   silently applied *none* of its edits — the first C8-instrumented build
   reported zeros because the counter sites were absent, not because the
   counters were inert. Detected by cross-checking against the
   reexamination's recorded numbers; re-applied and re-verified. (Lesson:
   after any multi-block instrumentation patch, verify a known-nonzero
   counter before reading results.)
2. **The (c3) unit-test construction**: the pre-registered wording ("a
   constructed repetition position where `evaluate_child` (hook on)
   returns the same `Draw` depth the child's `dfpn` frame returns") proved
   hard to satisfy by hand — root frames of king-shuffle regions never
   complete a bounded Draw proof (the very reason the region needs the
   rep-cache), and stored entries live deep along the search's dynamic
   trajectory, unreachable by a shallow replay. Solved with a temporary
   `cfg(test)` recorder at the plan9 frame-store site
   (`inctx_stores: Vec<(move path, proven depth)>`), replaying a real
   stored child's parent-side `evaluate_child` under the recomputed
   ancestor context. Removed with the hook.
3. **dec13/dec14 all-null cells**: the rollout cannot distinguish "hook
   fires but is inert here" from "hook never fires" without a re-hooked
   stats run (the side-band was off during the rollout by design and the
   hook is now reverted). No verdict depends on it; noted for any future
   revisit of this mechanism family.

## Missing tests

None added to the product (no product change). The (c3) throwaway unit
test and the `cfg(test)` store recorder were part of the temporary hook
and are gone with it. The permanent regression surface for a future
landing remains what D6 pre-registered: `evaluate_child` hit-semantics
tests in `src/search/dfpn/children.rs` + a `repetition_cache.rs` probe-site
contract extension, exercised under `make test` / `make test-full`.

## Next steps

#21 closes (reject → recorded). With it, **#22 fringe is the last open
reopened-backlog item** of the `research` initiative.

SESSION COMPLETE
- Deliverables: plan21 executed (Phase 0 PASS → Phase 1 rollout 264 runs → verdict REJECT), hook reverted (`git diff --exit-code` clean, anchors exact, `test_repetition` green), `measurements/plan21/` committed (driver/parse/verdicts/env.json/state), `report21.md` written, `reexamination.md` §6 step 5 annotated, `initiative.md` #21 row closed + history entry, `docs/plans/README.md` research row refreshed.
- Gate result: `make test` not required (no product change); release build + examples, clippy, fmt, 251 lib tests, `test_repetition --include-ignored` all green.

Follow-up options:
1. **Execute #22 fringe (last open item)** — kickoff prompt: "Execute docs/plans/research/plan22.md" (after a plan session drafts it: the rule50-seeding lever needs its own pre-registered Phase-0 design per the reexamination §6 step 6 wording; Kaneko root threads are owned by `parallel` and out of scope here).
2. **Close the research initiative after #22** — with #21 rejected and #22 fringe either executed or explicitly not taken, the reopened backlog is empty and `initiative.md` can record closure; the dispersion calibration series now has six real-arm data points.
