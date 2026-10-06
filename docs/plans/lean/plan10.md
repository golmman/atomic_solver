# Lean Plan 10 — #10 history/killer constant re-tune (behavior-changing, env-gated sweep)

Initiative: `lean` backlog #10 ("History/killer constant re-tuning — never
re-tuned after the GHI/twin removal; side-aware killers").
Prerequisite reading: `plan9.md`/`report9.md` (the M1 ordering-rank
measurement this plan's expectations are calibrated against),
`plan8.md`/`report8.md` (the bit-identical precedent that shows what a
*behavior-changing* ordering plan must do differently), the Motivation
section of `initiative.md` (nn oracle floor), and
`docs/spec/optimizer_interface.md` (the contract this plan deliberately
does *not* extend).

## Scope decision

One lever: **re-tune the existing history/killer heuristic constants** —
no new signal, no new table, no ranking-term change. The backlog item
bundles a second mechanism, "side-aware killers"; that sub-item is
**retired by analysis in this plan** (see Grounding facts F5), leaving
the constant sweep as the entire lever.

Production outcome on adopt: the winning constants are **hardcoded** in
`src/search/dfpn/history.rs` (one-line const changes). The optimizer
interface (`[scorer]` / `ScorerParams`, `docs/spec/optimizer_interface.md`)
is *not* extended: history/killer constants are search-layer, not
static-scorer parameters, and adding a runtime-tunable surface for a
one-time re-tune is YAGNI. The sweep runs through temporary env-gated
overrides (the `CONV*`/`LEAN9_SPIKE` precedent); the env hooks are
removed in every outcome (adopt → hardcoded consts, no-go → full
revert). No state survives in `src/` except, on adopt, new const values.

This is the last open ordering-adjacent item in the backlog. A measured
no-go is a fully valid completion: it closes #10 and empties the `lean`
backlog (report records the dormancy implication).

## Goal

Replace the backlog's unverified "~0–5% evals" potential with a measured
decision: either adopt a constant set that reduces suite child-evals
under the pre-registered gates below, or close #10 as a measured no-go.

Metric: **total `child_evals`** over the quick suite, first-outcome
(`benchmark --suite quick --json --first-outcome`), deterministic per
(deterministic search). Wall time secondary. Behavior change is the
point of this lever, so the drift protocol applies in its
behavior-changing form (working agreement §3: validated on the
move-order/benchmark suites, not byte-identity).

## Grounding facts (in-code; verified, not re-derived)

- **F1 — the constants** (`src/search/dfpn/history.rs:15-20`):
  `HISTORY_MAX = 10_000`, `HISTORY_BONUS = 100`, `HISTORY_AGE_INTERVAL
  = 10_000` (halving aging, fired from `maybe_age_history` at every
  frame close, `core.rs:377`), `SCORE_KILLER = 50_000`,
  `KILLER_SLOTS = 2`, `MAX_KILLER_DEPTH = 256` (no bonus beyond).
- **F2 — update trigger is proof-gated and sparse**: history and
  killers are updated only when a frame stores a proven non-Draw
  outcome with a best move (`core.rs:371-375`), i.e. once per proven
  frame, not per cut like alpha-beta history. `history` is side-indexed
  (`[[i32; 64]; 64; 2]`); killers are depth-indexed.
- **F3 — scale context**: static scores are thousands-scale
  (`ScorerParams`, e.g. `score_pawn_storm = 5600`), so `SCORE_KILLER`
  (50k) outright dominates static+history and `HISTORY_MAX` (10k) is
  comparable to the largest static terms. The never-tuned constants
  predate the plan8 scorer rework (Chebyshev table, blast aSEE) that
  changed the static scale's composition.
- **F4 — prior expectation cap**: plan9 M1 measured the OR winning
  child at static rank 0–1 in ~97% of OR-Win frames, and refuters at
  final rank 0 in 100% of refuted AND frames (pre-refuter mass
  0.00–0.02%). The oracle-floor record bounds what *any* ordering
  improvement can recover. #10's realistic potential is therefore the
  low end of the backlog's 0–5% band, concentrated in tail cases where
  the decisive child is not already at rank 0–1.
- **F5 — "side-aware killers" is structurally moot** (analysis
  retirement): `sort_moves` and `update_killers` both key on
  `depth = path_stack.len()` — the ply index from the root — and the
  root position is fixed for the lifetime of the search (`AGENTS.md`).
  Side-to-move parity is a function of ply only (no passes, no null
  moves), so all frames at a given depth have the same side to move;
  the depth-indexed killer table is already implicitly side-aware, and
  history is explicitly side-indexed. Doubling the killer table by
  `[side][depth]` indexing would change nothing. The plan's report
  records this argument; no code is needed.

## Mechanics of the spike

Add temporary env-gated overrides read once in `Search::new` (fields on
`Search`, defaulting to the consts; the const defaults keep every
arithmetic path identical when no env var is set):

- `LEAN10_HISTORY_BONUS` (default 100)
- `LEAN10_HISTORY_MAX` (default 10_000)
- `LEAN10_HISTORY_AGE_INTERVAL` (default 10_000)
- `LEAN10_SCORE_KILLER` (default 50_000)

`KILLER_SLOTS` is not tunable (fixed-size array dimension; changing it
is an invasive spike for a second-order effect that F4 says is
near-inert) and `MAX_KILLER_DEPTH` is not tunable (the >256-ply tail is
rare and killer staleness there is unmeasurable at suite scale).

Non-perturbation gate (before any arm is read): with no env vars set,
the m22 first-outcome stdout is byte-identical to the HEAD capture
(md5 recorded at step 0) and the quick-suite JSON equals the step-0
default baseline exactly.

## Phase 0 — baselines (measurement only)

1. Build release; record m22 FO stdout md5 (the non-perturbation
   anchor).
2. Capture the default baseline: quick FO `--json` (`--timeout 3`) and
   thorough FO `--json` (`--timeout 5`), plus m22_white, dec13, dec10
   FO single runs. Parsed per-case `child_evals` summaries go under
   `measurements/plan10/` (parsed results only; no raw transcripts
   committed, per AGENTS.md).
3. Optional one-run diagnostic (counter-only, env-gated
   `LEAN10_STATS=1`, reverted with the spike): fraction of `sort_moves`
   calls where history+killer changes the static-only top-1 pick. If
   the fraction is ~0 on the quick suite, the no-go is expected and the
   screen can be shortened to the two scale axes.

## Phase 1 — one-factor-at-a-time screen (quick suite FO)

Nine runs incl. baseline (each ~10–15 s):

| axis | arms |
|---|---|
| history scale `s` (BONUS and MAX scaled together, count-to-cap constant) | 0.5×, **1×**, 2× |
| killer dominance `SCORE_KILLER` | 10_000, **50_000**, 200_000 |
| aging `HISTORY_AGE_INTERVAL` | 2_500, **10_000**, 50_000 |
| cap ratio `HISTORY_MAX/BONUS` (at default BONUS) | 50, **100**, 200 |

Ranking metric: total quick-suite `child_evals` (sum over `status: ok`
cases). Timeout-status flips are counted separately and count against
the arm.

## Phase 2 — refinement + finalist validation

Take the two most promising axes and run their small cross-product
(≤ 8 combos) on quick. The best combo (and, for contrast, the single
best OFAT arm) are **finalists**; each finalist runs:

- thorough suite FO (`--timeout 5`),
- m22_white, dec13, dec10 FO single runs,
- stress FO (`4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`,
  post-plan9 baseline 249,480,478 child evals — the deep
  repetition-dominated class where ordering changes are most likely to
  backfire via threshold-lattice churn).

## Pre-registered gates

**Adopt** requires all of, for the winning finalist:

1. quick total ≤ **97%** of the default baseline (≥ 3% — above suite
   run-to-run noise is irrelevant since runs are deterministic, but
   below the level where trajectory chaos between neighboring constants
   produces fake wins; conversion plan8's four-point-sample lesson);
2. no quick case regresses > **+10%** child_evals, zero outcome flips,
   zero `wrong`, zero new timeout-status flips on previously-solving
   cases;
3. thorough total ≤ **102%** of default (non-regression within noise);
4. hard cases within **+5%** of default each: m22 FO, dec13, dec10;
5. stress FO ≤ **102%** of the re-measured step-0 default (re-measured
   at HEAD in phase 0 — the inherited 249,480,478 predates plan8's
   scorer change, so it is re-captured, not assumed).

**No-go** if no finalist passes: close #10 measured no-go; the report
records the measured constant sensitivity (even flat results are the
decision evidence) and the backlog becomes empty.

**Kill switch**: if phase 1 shows every arm within ±1% of default
(and the phase-0 optional diagnostic measured a near-zero
reorder-fraction), skip phase 2 entirely and close #10 no-go — the
surface is insensitive and the constants stay.

## Phase 3 — landing (adopt only)

1. Hardcode the winning consts; delete the env overrides and the
   `LEAN10_STATS` hooks; `src/` diff = const lines only.
2. `cargo fmt`, `cargo clippy`, `make test` (fast gate).
3. `make test-full` — required: this is a move-ordering change
   (AGENTS.md testing tiers).
4. Update `initiative.md`: #10 row status, the post-plan10 note in the
   Motivation/history, and the F5 side-aware-killer retirement.
5. Write `report10.md` (final task): arms, gates, decision, measurement
   file pointers, findings (incl. F5 argument), next steps (likely:
   backlog empty → dormancy discussion for the initiative).

On no-go: revert all spike code (tree byte-identical to HEAD, verified
by re-running the m22 stdout md5), write `report10.md`, update the #10
row.

## Budget

Build + ~20 suite/hard-case runs + (on adopt) `make test-full`
(~25 min). Fits one session with margin; the kill switch bounds the
no-go path to ~30 min of measurement.

## Non-goals

- No new ordering signal, no counter-move table (plan9 M4 probe is
  recorded moot), no `ScorerParams`/optimizer-spec change.
- No tuning of `KILLER_SLOTS`/`MAX_KILLER_DEPTH` (see Mechanics).
- No proof-tree, TT, or refinement-path semantics change; the
  deterministic `child_evals` budget contract is untouched (the
  heuristic only reorders move lists).
