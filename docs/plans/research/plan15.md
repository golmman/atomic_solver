# plan15 — Item #18, Phase 0: the m22 collapse diagnosis (mechanism only)

Initiative: `research` (re-opened 2026-10-09). Executes the pre-registered
Phase 0 of reopened backlog item **#18** (leaf-initialization family): the
diagnosis of the m22 collapse. Self-contained: a fresh session can execute
this without reading the full record. Prerequisites are done: #17 gate
pinned (`gate_methodology.md` v1.0), #23 closed H0 (plan14). **This plan
issues no lever verdict** — the gate's single-draw comparisons are retired,
so all single-salt readings here are hypothesis-ranking data only; the
gated shaped-variant rollout is the follow-on plan (plan16), which issues
the adopt/reject/defer verdicts.

## Motivation

`reexamination.md` §2–§3 found the load-bearing surface: 80–88% of child
evals on stress/m22/dec13 are first-sweep cuts of fresh frames — the cost of
discovering each node's branching factor — and every unexpanded child is
initialized to `(1, 1)` (`src/search/dfpn/children.rs` `_ => (1, 1)`). The
naive mobility arm (`PROBE_INIT=mob`: unsolved child with no reusable TT
bound starts at `(n, 1)` when its parent is an OR node — i.e. the child has
the defender to move — and `(1, n)` when its parent is an AND node) moves
**stress 0.24×** (249.5 M → 58.7 M) and **dec13 0.16×** (3.82 M → 0.61 M),
but sends **m22 from 14.16 M to censored at 1 B on all five salts**. The
collapse is systematic, not noise; its mechanism is undiagnosed.

Reexamination §3 lists two unverified hypotheses:
- `pn = min nᵢ` at OR nodes steers toward low-mobility "forcing-looking"
  lines; and
- the AND-side `dn` init still jumps on expansion, so the `1 → b` problem
  moved to the other number.

Context from this plan's read-only opening probes (2026-10-11, HEAD
`d7de659`): the m22 salt-0 baseline reproduces plan12 exactly
(14,156,269, ~1.8 s) and its proven win is a **95-ply maneuvering line with
~30 repetition cycles** of a king shuffle (`h8g8`/`g8h8` ×~30) — the win
itself lives in a repetition-cycle region, which motivates a third
hypothesis (H-C below). Root mobilities of the four probe cases do not
separate them (m22 43, stress 41, dec13 41, dec10 16), so root branching
alone does not explain the polarization.

## Objective

1. **Classify the m22 collapse mechanism** among pre-registered hypotheses
   H-A/H-B/H-C/H-D (below), using temporary env-gated instrumentation
   (plan14 pattern: measure, revert, `git diff --exit-code`).
2. **Discriminate with split micro-arms** (OR-side only / AND-side only /
   capped / log): which half of the mob init carries the stress/dec13 win,
   and which carries the m22 collapse.
3. **Nominate or retire shaped-variant candidates** for the plan16 gated
   rollout — or record a family-level Phase-0 no-go if no shape can keep
   the win without the collapse.
4. No product knob, no gate amendment, no lever verdicts.

## Pre-registered hypotheses (fixed before any instrumented run)

- **H-A — OR-side selection steering.** With OR-parent children at
  `(pn, dn) = (n, 1)`, the OR frame's min-pn selection visits low-mobility
  defender-to-move children first; on m22 those lines are the expensive
  deep maneuvering, so budget burns in wrong subtrees. Signature:
  the mob run's first-sweep-cut share *drops* and per-frame recursion depth
  rises at OR frames; the selection trace diverges early and stays apart.
- **H-B — AND-side threshold inversion.** With AND-parent children at
  `(1, n)`, the AND frame's summed `dn` grows by n per child, so the
  first-sweep cut (fired in 80.7% of m22's evals under `(1,1)`) stops
  firing at AND frames and they recurse. Signature: AND-side
  first-sweep-cut share collapses while OR-side stays near baseline;
  recursion share rises at AND frames.
- **H-C — repetition-cycle thrash.** The steered trajectory enters
  repetition-cycle regions (m22's own win is ~30 cycles); repetition-dependent
  draws are never TT-cached and refutations must be re-derived, so
  per-distinct-node cost rises. Signature: suppressed-draw stores /
  evals-after-stale-child-result counters rise sharply under mob on m22
  (vs. mob on stress/dec13) while unique-node share does *not* rise
  proportionally.
- **H-D — honest expansion (null-ish).** The init genuinely widens the
  proven-number frontier: the mob run visits far more *distinct* nodes and
  does proportionally more real work per node. Signature: unique-node share
  rises strongly, counters of H-A/H-B/H-C stay near baseline. If H-D
  dominates, no "fix" exists within the family's shape knobs and the
  shaped-variant space collapses to blends/caps at best.

Hypotheses are not exclusive; the decision table (P1–P3) classifies the
dominant mechanism per signature.

## Probes (temporary, env-gated, reverted — no product change remains)

Instrumentation is re-derived from the archived reexam `probe.patch`
(`measurements/reexam/probe.patch`, made at `bb19e7b`; current HEAD is
`d7de659` — port, do not blind-apply). All counters write one JSON line per
run via the existing `examples/probe_solve.rs` pattern.

- **P1 — trajectory anatomy** (per run, both arms, salt 0): first-sweep-cut
  share split OR/AND; fresh-frame share among cuts; children-per-frame
  distribution; evals by ply-from-root buckets; unique-node share (bounded
  2²⁶-entry key set, fixed memory cap, probe-only — documented as a
  temporary exception like the preflight `REGION_BUDGET`); repetition
  counters (suppressed-draw stores, informative-bound clobbers,
  evals-after-stale-child-result, evals-after-first-explored-mark).
- **P2 — selection/divergence trace** (m22 + dec13, both arms): per frame,
  append the selected child's Zobrist key + ply (u64+u8, raw binary) capped
  at the first 2²⁴ selections per run; offline alignment gives the first
  divergence index and the share of trace after divergence. Raw traces stay
  in the uncommitted RAW dir.
- **P3 — split micro-arms** (env-gated, all with the same counters as P1):
  - `mob-or`: OR-parent children `(n, 1)`, AND-parent children `(1, 1)`;
  - `mob-and`: AND-parent children `(1, n)`, OR-parent children `(1, 1)`;
  - `mobcap8`: both sides `min(n, 8)`;
  - `moblog`: both sides `1 + floor(log2 n)`.
  Runs: m22 at salts {0, 1, 2} (the reexam collapse is known systematic
  across five salts; three salts confirm systematicity per sub-arm);
  stress, dec13, dec10 at salt 0 (direction-only).

## Pre-registered decision rules

- **GO-plan16:** ≥ 1 sub-arm keeps a stress-or-dec13 direction win
  (paired-salt-0 ratio ≤ 0.9) **and** on m22 holds ratio ≤ 1.5× baseline
  uncensored on ≥ 2 of its 3 salts. The mechanism admits a shippable
  shape; plan16 = gated rollout of the surviving shapes over the canonical
  corpus (22 cases × 6 salts, pinned v1.0).
- **PIVOT:** evidence splits (e.g. `mob-or` keeps the wins but `mob-and`
  is what destroys the m22 cuts, and no single sub-arm qualifies). plan16
  is scoped to the surviving superset (e.g. blends: capped OR-side +
  conservative AND-side), with its own pre-registration.
- **NOGO:** every sub-arm either collapses m22 (ratio > 1.5× or censored
  on all 3 salts) or loses the stress/dec13 direction (ratio > 1 on both).
  The win and the collapse are the same arithmetic; #18 records the
  Phase-0 verdict and pauses pending an owner decision.
- **HALT:** any soundness invariant below is violated — investigate, do
  not report measurements.

**Honesty clause:** every ratio in P3 is a salt-0 (or 3-salt m22) reading
below the pinned gate's noise floor on the quiet band [0.76, 1.32]; m22
readings are systematic (all-salt reexam evidence) but stress/dec13/dec10
readings are single-draw. They rank hypotheses and nominate candidates;
they are never lever verdicts. plan16 re-measures every nominated shape
under the paired-salt rule.

## Soundness invariants (hard gates)

(a) **Probe-disabled identity:** with no probe env set, m22, dec13, dec10,
dec15 at salt 0 reproduce the plan12 salt-0 counts exactly
(14,156,269 / 3,822,602 / 4,262,128 / 1,077,420) — before and after the
instrumented runs.
(b) **Probe-enabled non-perturbation:** the counter-only baseline arm
(probe on, init off) reproduces the same counts exactly.
(c) **Outcomes:** every uncensored run of every arm reports `win` (all four
cases are `win` fixtures); a budget-censored `draw` is the no-result
sentinel, never a proven draw.
(d) **Determinism:** a rerun at the same (case, salt, arm) reproduces
stdout and child evals bit-for-bit.
(e) **Revert:** `git diff --exit-code` passes after the instrumentation is
removed; the release build is re-verified.

## Method

- **Phase 0 — identity anchors.** Build at HEAD `d7de659`; run (a); apply
  the ported instrumentation; run (b).
- **Phase 1 — P1/P2.** Baseline + mob arms × {m22, stress, dec13, dec10} at
  salt 0; traces on m22/dec13. Budgets: stress 2.5 B, others 1 B
  (`--timeout 600` secondary); ≤ 3 concurrent processes; RAW transcripts
  uncommitted.
- **Phase 2 — P3.** Four sub-arms per the run grid above. Caps: stress
  micro-arms at 300 M (direction-only; censored recorded as such), m22 1 B,
  dec13/dec10 1 B.
- **Phase 3 — analysis + revert + report.** Parse to
  `state/anatomy.json`, `state/divergence.json`, `state/arms.json`; apply
  the decision table; revert (e); write `measurements/plan15/` (driver
  scripts, `env.json`, parsed state, `README.md`), `report15.md` (final
  task), update the `initiative.md` #18 row (diagnosis outcome + plan16
  pointer or pause), and update the `docs/plans/README.md` research row
  only on a pivot/close-worthy outcome.

## Gates

- **GO-plan16 / PIVOT** → plan16 drafted (gated shaped-variant rollout).
- **NOGO** → #18 Phase-0 verdict recorded with data; owner decides close
  vs. redirect (e.g. to #20/#21).
- **HALT** → soundness investigation first.

## Out of scope

- Productization of any initialization variant; the gated rollout (plan16).
- Atomic features (blast/king-zone threats), clock-borrowed init (#18's
  later stages), #19 remaining arms, #20 restarts, #21 in-context child
  results, #22 fringe.
- `gate_methodology.md` changes; benchmark/optimizer interface;
  proof-tree layer; preflight; the `dfpn`/`parallel`/`workspace` items.

## Budget envelope

P1: 2 arms × 4 cases at salt 0 ≈ worst 11 B evals ≈ 25 min serial
(mob stress lands at 58.7 M, so realistically far less); P2: 4 runs, m22
mob may censor at 1 B (~2–3 min). P3 worst case: all four m22 sub-arms
censor on all 3 salts ≈ 12 × ~2.2 min ≈ 26 min, plus stress direction runs
censored at 300 M ≈ 4 × ~40 s. Total ≤ ~1.5 h wall with 3-way concurrency;
Phase 0 identity anchors are minutes.

## Final task

Write `report15.md` in this directory: deliverables, verdict
(GO-plan16/PIVOT/NOGO/HALT), the mechanism classification with P1–P3
tables, nominated (or retired) plan16 candidates, problems encountered,
missing tests, next steps (plan16 kickoff prompt or the alternative).
