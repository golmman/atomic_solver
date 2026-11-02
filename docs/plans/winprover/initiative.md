# Initiative: `winprover` — an independently-checked atomic win prover that beats provari

## Status

**Closed 2026-11-02 (search thread measured out; goal not met — see
[Next direction](#next-direction)).** This initiative consolidates three earlier ones
into a single goal: [`provari`](../provari/initiative.md) (search levers,
closed into this initiative), [`interop`](../interop/initiative.md)
(independent proof validation, closed into this initiative) and the crate
promotion thread of [`workspace`](../workspace/initiative.md) (closed into
this initiative as optional item 7). Their plan/report history and their
measurement artifacts stay where they are; only the backlog moves.

Plan numbering here is fresh (`plan1.md` … `plan5.md` are executed, see
[`report1.md`](report1.md), [`report2.md`](report2.md),
[`report3.md`](report3.md), [`report4.md`](report4.md) and
[`report5.md`](report5.md)) and *independent* of the retired initiatives'
numbering. The mapping is recorded in [Renumbering record](#renumbering-record).

## Goal

**Be the fastest atomic win prover on a fair, reproducible, shared-win corpus —
with every claim checkable by a second rules implementation.**

Three legs, one goal (they are not separate projects):

1. **Search** — port provari's win-proving levers (and then some) onto this
   solver, or replace the win path with a purpose-built attacker-only front-end.
2. **Evidence** — prove our wins with an independent checker, so speed work is
   never built on a self-validating rules layer.
3. **Fairness** — a pinned external oracle, a frozen corpus, and budgets that
   compare like with like.

Success condition (the head-to-head target, item 6):

- on the **shared-win set**, geometric-mean wall `atomic_solver / provari < 1.0`;
- **coverage** at least as high as the same provari configuration;
- every claimed win replay-validated by our tree validator *and* accepted by the
  independent checker;
- all existing drift/soundness gates green; no outcome regressions.

### Two comparison columns (this is a goal clarification, not a detail)

The retired `provari` initiative compared against provari's **bundled NNUE
model** default while listing learned models as a non-goal. That mixes an
out-of-scope artifact into the target and flatters provari by ~5× on the one
case used as the shared win (13,779 vs 72,523 nodes, report 1 vs the plan-3/5
hand-guide ablations).

- **Primary target:** `provari prove --guide hand`. This is the fair,
  hand-evaluation-for-hand-evaluation comparison and the number the initiative
  must win.
- **Stretch column (recorded, never a product dependency):**
  `--guide model`. We may only match it with an optional external model
  artifact that a later plan licenses and measures explicitly; the hand-eval
  path must stand on its own.

Coverage claims must state which column they are measured in. `sample20`
coverage under the hand guide is expected to be ~2/20, not the 4/20 bundled
number in `provari` report 1 — item 1 re-derives it.

## Why consolidated

The three retired initiatives all measured, or were measured against, the same
external artifact at the same pinned commit, under the same GPL boundary, and
each was sized by the same corpus. Splitting them meant the corpus, the pinned
oracle, the budgets and the target FENs lived in three places with no single
ranked backlog; the previous session's ordering question existed only because
of that split.

Consolidation does **not** merge quality attributes: correctness (independent
evidence) stays ahead of performance, and a search plan may not proceed past a
red evidence gate — the checker item is ranked second for exactly that reason.

## Non-goals

- **No provari code, dependency, binary, network, or model in this repo.** No
  `Cargo.toml` entry, no vendored GPL source, no committed oracle binary. The
  oracle runs as a separate process from a scratch directory.
- **No NNUE training or product dependency on a learned model.** The stretch
  column stays an optional, separately-measured external artifact.
- **No tablebase integration** (owned by `egtb`; the baseline stays
  tablebase-free).
- **No weakening of soundness.** Repetition, rule50, terminal, and proof
  contracts stay intact. A faster false win is a failure, and an independent
  checker disagreement is a veto.
- **No general-solver outcome regressions.** Behaviour-changing levers use the
  existing drift protocol, quick suite, repetition soundness gate, and anchors.
- **No parallel/portfolio work** (`parallel`), **no proofdb/proof-tree format
  ownership** (`proofdb`/`proof`), **no unrelated micro-optimization**
  (`performance`/`lean`). Packaging work is optional item 7 and never a
  precondition for a search lever.

## Working agreement

1. **Measure before planning.** Every plan starts from the committed baseline
   and a read-only probe; sizing spikes are temporary and reverted.
2. **One lever per plan.** A plan may add one mechanism (or one deliberately
   coupled pair, stated up front), not a bundle. A census may be folded into a
   plan session (read-only), never into an execution plan.
3. **Correctness gates are vetoes.** `make test` by default; `make test-full`
   for search/TT/proof changes. Behaviour-changing plans pass the drift
   protocol on unaffected cases, the quick suite, and the repetition soundness
   gate (`tests/test_repetition.rs -- --include-ignored`).
4. **Deterministic budget contract.** `child_eval_budget` and
   `ExitReason::BudgetExhausted` semantics stay exactly as documented. New
   front-ends account their evals in the same counters or document why they are
   outside the budget.
5. **Fallback, not replacement.** A provari-style win front-end may return a
   proven win early; when it does not, the existing solver runs unchanged. The
   general solver remains authoritative for draws/losses and proof
   construction.
6. **Oracle isolation and export discipline.** Build provari in a scratch
   directory, pin the commit/tag, record `provari --version`. The export is
   derived, never authoritative: the proof tree/DB remains the validated thing
   and the external checker is a second opinion. Every non-accept is
   classified (`accepted`, `convention-repeat`, `unchecked`, `export-bug`,
   `suspect`); only the last two trigger follow-up work.
7. **Negative results are recorded.** A lever that loses its gate closes with
   the measured evidence; it is not silently retained behind a default.

## Backlog

Plan numbers are sequential and independent of item numbers. Item numbers are
stable semantic identity across re-ranks.

| # | Item | Mechanism | Why / target | Metric | Status |
| --- | --- | --- | --- | --- | --- |
| 1 | Fair baseline: hand-guide column + corpus re-derivation | Re-run the pinned provari at `--guide hand` over the frozen `sample20` (and the core FEN); re-derive shared-win set, coverage and wall ratios; record both columns side by side | Every later verdict is priced against this; the retired baseline mixes an out-of-scope model into the target | wall, nodes/child evals, coverage per column | **done — plan1/report1**: hand wins exactly the 3 shipped bench cells (3/21 = 2/20 `sample20`); model 5/21 (4/20); our `default` 1/21, `opt` 2/21; shared factors `hand×default` 51.739 (1 cell), `hand×opt` 6.848 (2 cells). No bench-set change, no ledger row |
| 2 | Differential proof checker oracle | Export an existing `proof_tree.bin` to provari's JSONL format; run `provari check`; classify every mismatch | Independent rules implementation catches correlated `atomic-movegen`/`Position` bugs; the exporter is reusable, and new proof shapes (item 3) need it in place *before* they are trusted | per-tree verdict class, defect count | **done — plan2/report2**: `write_provari_jsonl` (validate+replay first, explicit DFS, root-`Win` only) + `provari_export` example; all 168 committed root-`Win` shards exported and **168/168 accepted** by the pinned oracle (`48686bc`, 60 s, no tablebases), 0 non-accepts; three corrupted-export controls all rejected (checker not vacuous); output byte-deterministic; 25 trees have benign duplicate attacker positions (no differing move); verdict **CLEAN**, no ledger row |
| 3 | Win-only front-end skeleton | Opt-in attacker-only win prover that proves a win and otherwise falls back; no new heuristics; includes the item-4 sibling-simulation **census** in its plan session | Isolates provari's attacker-only proof semantics from the general outcome/PV machinery; the architectural pivot, and the home for items 4–5 | correctness (fallback identity), evals, wall | **done — plan3/report3**: `src/search/winprover/` is a small win-only DF-PN+ sibling with a 32 MB scratch TT, `--win-front-end` (default off) + `--win-front-end-budget` (1 M evals), `PvStatus::WinFrontEnd`, proof events, and 11 unit + 3 integration tests; fallback is bit-identical (outcome/PV/nodes/evals/TT snapshot). Bench **neutral**: proves 0/3 cells, adds 1,000,0xx evals / ~0.35 s; geomean 6.81 → 8.75, coverage 2/3 unchanged. One ledger row; item-4 census recorded |
| 4 | Sibling proof replay / simulation | Replay a proven defence or threat on sibling positions (provari's `SIMULATION = 16` replay) | The last unported provari search lever; the measured residual is node count (~222k expansions vs provari's 13.8k bundled / 72.5k hand on the shared win at 14.5 child evals per expansion) | evals, nodes, wall, replay soundness | **done — plan4/report4: first form shipped, thread closed**. The first-form replay is implemented in `src/search/winprover/simulation.rs` (guide replay at AND nodes, 16-position replay budget, simulation-local one-ply helper, cap/global-budget charging, scratch-TT sibling store, proof events; `--no-win-front-end-simulation` disables it), with 18 unit + 6 integration tests including a replay-validated proof tree. **Gate failed** (10 M: neither `core` nor `easy-win`), so the second form (`replay_threat`) is deferred and the front-end search thread is closed; the front-end stays opt-in/default-off |
| 5 | Rule50-free key + proof length/limit | Key boards without the clock; store proof length and highest holding clock; replay/verify conservatively | Recover move-order transpositions without breaking rule50/repetition soundness | TT hit rate, evals | **closed measured-out — plan5/report5**: board-only-key prototype (unsound upper bound) left `core`/`easy-win` eval-identical and saved only ~0.7 % on the censored `deep-win` without changing coverage; the proof-length/holding-clock half is closed with it rather than implemented speculatively |
| 6 | Head-to-head convergence | Tune the adopted stack on the target corpus; decide the default flag set; require shared-win wall < provari (primary column) and coverage ≥ provari | The initiative's success condition; no new mechanism | wall ratio, coverage | **done — plan5/report5: goal not met**. Pre-registered quick-suite rule kept the shipped defaults (opt stack regresses `dec06`/`dec42`/`dec44` loss → 3 s draw); product default 44.061 (1 cell, coverage 1/3), opt stack 6.301 (2 cells, coverage 2/3), both censored on `deep-win` vs provari hand 3/3. One ledger row. Search thread closed |
| 7 | **Optional:** crate promotion / workspace split | Re-draft and execute the workspace split (root package + `apps/proofdb` + a `winprover` crate) *if* the front-end grows a test surface or consumer worth isolating | Clean boundaries for the new engine; never a precondition, and not needed for codegen (`lto`, `codegen-units = 1`) | packaging only (byte-identical solver output, gate green) | unranked — carried to the successor; **requires a re-draft**: [`workspace/plan1.md`](../workspace/plan1.md) is stale (see Renumbering record) |
| 8 | Horizon — evidence extensions | Root-`Loss` coverage + compact `.pvr`; portable attacker-only export for proofdb transport; external `high200` corpus; native second rules implementation; cannot-win material | Completes checker coverage and interchange; out-of-distribution corpus guards against overfitting | — | unranked — carried to the successor; items 2–5 of [`interop/initiative.md`](../interop/initiative.md) and the `provari` 4b / cannot-win threads |

**Item 3 ahead of item 4.** `provari` report 5 ranked sibling simulation next,
but that ranking assumed the current `dfpn` frames. Sibling replay's home is
the attacker-only front-end, so implementing it in `dfpn` first risks writing
it twice; the census is read-only and belongs in item 3's plan session, which
prices both before either is implemented. If the front-end is rejected, item 4
moves back to the top of the ranking. Plan3's outcome: the front-end is
**kept** (correct, fallback-identical, opt-in) but proved none of the bench
cells inside 1 M evals, so item 4 remains next **with a gate** — re-measure the
census on the front-end and require it to prove `core`/`easy-win` at a stated
cap, otherwise pivot to a candidate/verification role for the front-end.

### Measured-out levers (do not re-try without new evidence)

Recorded here so the consolidation does not lose the negatives
(details in [`provari/report2.md`](../provari/report2.md) …
[`report5.md`](../provari/report5.md)). Verdicts are as measured under the
retired initiative's own conventions — **not** re-derived against the primary
hand-guide column of item 1 (plans 2–4 used the bundled model on `sample20`;
plan 5's guide isolation used `--guide hand` on the core FEN):

| Lever | Verdict | Evidence |
| --- | --- | --- |
| Move-class starting numbers (OR) | **GO, opt-in** (`--move-class-pn`) | core 8.74× evals / 7.58× wall; shared 3.41×/3.18× |
| Quiet-move grouping | **GO, opt-in** (`--quiet-grouping`) | core 2.60×/1.97×; shared 2.27×/1.85×; best stack 437.2 M → 27.9 M evals |
| Static immediate-explosion closure | **negative/neutral, opt-in** (`--static-tactics`) | 1.24× worse on core, 1.49× better on shared; not additive — node-entry closures discard DF-PN ordering feedback |
| EF-PN hand guide, both node kinds | **REJECT** | core 0.440× but shared 1.483× (per-case floor) |
| EF-PN hand guide, attacking branch only | **GO, opt-in** (`--eval-pn or`) | core 0.618×, shared 0.580×, geomean 0.599×; ~9 % throughput tax |

Shipped opt-in stack: `--move-class-pn --quiet-grouping --eval-pn or`.

## Next direction

All three plan5 probes are measured-out on the bench cells: the
candidate/verification pivot has no viable candidate source at a
product-affordable cap, the rule50-free key buys nothing measurable, and the
front-end with the opt starting-number levers proves `core` only at ~30.9 M
evals / ~15 s (2.2× the general solver's work, 4× its wall) and `easy-win` not
at 50 M. The remaining gap is architectural, not a missing small lever.

**Recommended successor (new initiative, not a winprover plan):** a
from-scratch provari-parity attacker-only prover — full opt stack, optimized
child evaluation (the front-end's full `Position::outcome()` movegen per child
is the measured bottleneck), and provari-style `Mover`/effect precomputation.
It is the only measured path to the initiative's `< 1.0` geometric-mean goal.
Items 7 (optional packaging) and 8 (evidence horizon) are carried to that
successor; anything else here would be a new mechanism without a measured
substrate.

## Corpus and measurement conventions

- **Primary metric:** `child_evals` to the first decisive outcome
  (deterministic) and wall time (the product metric). Nodes/expansions are
  secondary and are **not** compared one-to-one across implementations — an
  expansion count divided by child evals is the useful diagnostic
  (14.5 evals/expansion on the shared win at the shipped stack).
- **Core case:** `r1b1kbnr/ppp3pp/n4q2/3pppN1/1P1PP3/8/P1P2PPP/RNBQKB1R w KQkq - 0 1`.
- **Corpus:** the frozen `sample20` defined by `provari` plan 1 (every 10th
  line of provari's pinned `bench/high200.jsonl`, 1-based indices 1, 11, …,
  191). Pinned by digest:
  `docs/plans/provari/measurements/plan1/corpus.json`,
  sha256 `9cd0169a30bb7d5b162b6a1ec119fe194b776c8a24a46cdf1582c07062847d0e`.
  **Do not silently change the corpus.** New cases are added by a plan with a
  recorded selection rule, as a separate, separately-reported set.
- **Budgets:** 10 s and 192 MB for both sides unless a plan states otherwise.
  Provari: `prove --seconds 10 --memory 192 [--guide hand|model|none]`. Ours:
  `--timeout 10 --first-outcome --tt-size 192` plus the measured flag stack.
- **Repeats:** three for the core case and any reported wall ratio; one per
  sample case, with censoring/timeouts reported, never dropped.
- **Oracles are rebuilt, not committed.** The pinned provari commit is
  `48686bc6c786d6ea70568f0b2ad4002cf0520206` (`v0.1.0`); bundled model
  `network.nnue` sha256 `99dc67ea…a04a6` and `model.json` sha256
  `8822a85f…7f987` only matter for the stretch column. Toolchain/network
  blockers are recorded as `INCONCLUSIVE`, never substituted.
- **Progress ledger:** [`bench.md`](bench.md) is the standing performance
  document. It pins a three-cell bench set (every `core + sample20` cell the
  pinned provari oracle wins with `--guide hand`), defines
  `factor = our wall / provari hand wall` with provari as 1.000, and records
  the canonical geometric mean plus coverage. **Every executed plan whose
  result touches the win path appends one row** (per-cell factors, geomean,
  coverage, host, stack, deterministic `child_evals`); housekeeping plans
  append nothing. Goal: geometric mean `< 1.0` with coverage ≥ provari. Its
  reference measurement is
  [`measurements/bench/`](measurements/bench/) (driver + parsed results).
- **Artifacts:** this initiative's drivers, corpus definitions, parsed results
  (`*.json`, `*.csv`) and provenance READMEs go under
  `measurements/planN/`; raw transcripts, provari sources/binaries, and
  `__pycache__` are not committed. Historical measurements stay under
  `../provari/measurements/` and `../interop/measurements/` — they are not
  moved, and plans here reference them by path + digest.

## Renumbering record

Item ↔ plan mapping and where the retired threads went:

| Old | New | Note |
| --- | --- | --- |
| `provari` item 1 (baseline) | item 1 | extended to both guide columns |
| `provari` item 2 (win-only front-end) | item 3 | re-ranked ahead of the sibling item |
| `provari` item 3a/3b (class numbers, grouping) | measured-out table | both shipped opt-in |
| `provari` item 4a (static closure) | measured-out table | shipped opt-in, not adopted |
| `provari` item 4b (`exposed`/no-safe-evasion/double-threat) | item 8 (horizon) | unranked; report 4's diagnosis stands |
| `provari` item 5 (EF-PN) | measured-out table | `or` shipped opt-in, `both` rejected |
| `provari` item 6 (rule50-free key) | item 5 | unchanged rank relative to the search levers |
| `provari` item 7 (head-to-head) | item 6 | success condition |
| `provari` item 8 (sibling simulation) | item 4 | census folded into item 3's plan session |
| `interop` item 1 (checker oracle) | item 2 | plan to be re-cut from the drafted `interop/plan1.md` |
| `interop` items 2–5 | item 8 (horizon) | unchanged content, re-homed |
| `workspace` item 1 (split + proofdb crate) | item 7 (optional) | **`workspace/plan1.md` is stale** |
| `workspace` item 2 (further carve-outs) | not carried | remains rejected; `workspace/initiative.md` keeps the rationale |

Retired plan numbers: `interop/plan1.md` was drafted, never executed — it is
**not** renumbered into this initiative; item 2's plan is a fresh draft that
may reuse it. `workspace/plan1.md` was drafted 2026-10-09 and is stale: it
moves `examples/proofdb_ledger_union.rs` (retired by `proofdb` plan 20) and a
module tree (`pns/`, `descend`, `and_close`) removed by `proofdb` plan 18, sizes
`examples/proofdb/` at ~6,800 lines (now 3,534), and misses
`examples/campaign/` and `examples/proofdb/sync.sh` (called by
`make proofdb_sync`). Item 7 therefore starts with a re-draft from the current
tree; `workspace/initiative.md`'s D1–D3 decision record stays the reference for
that re-draft.

## History

- **2026-11-02** — item 6 executed and the initiative **closed**
  ([`plan5.md`](plan5.md), [`report5.md`](report5.md), measurements in
  [`measurements/plan5/`](measurements/plan5/README.md)). This was a
  decision/close-out session, not a new mechanism. The plan-session probes
  (candidate/verification pivot, rule50-free key, front-end heuristic parity)
  are recorded as measured-out with their limitations. The pre-registered
  quick-suite rule kept the **shipped defaults**: the opt stack
  (`--move-class-pn --quiet-grouping --eval-pn or`) turns three `Loss` cases
  (`dec06`, `dec42`, `dec44`) into 3 s draws, so it is not promoted. Final
  canonical head-to-head: product default **44.061** (1 shared cell, coverage
  1/3), opt stack **6.301** (2 shared cells, coverage 2/3), both censored on
  `deep-win`; provari hand 3/3. The success condition (geomean `< 1.0`,
  coverage ≥ provari) is **not met**; the search thread is closed, the
  front-end stays opt-in/default-off, and the next direction is a from-scratch
  provari-parity prover initiative. One ledger row.
- **2026-11-02** — item 4 first form executed ([`plan4.md`](plan4.md),
  [`report4.md`](report4.md), measurements in
  [`measurements/plan4/`](measurements/plan4/README.md)): sibling proof replay
  (`simulation.rs`) is in the opt-in front-end, on by default with
  `--win-front-end`, with a 16-position replay budget, a simulation-local
  one-ply helper, simulation counters/accessors, and `--no-win-front-end-simulation`.
  The replay is exact (repetition/rule50 are never wins), charges the
  front-end cap and global budget, stores successful sibling roots in the
  scratch TT, and emits replay-validated proof events (including a first-form
  fix that terminal children now get events; a simulated win builds a
  replay-validated proof tree). Bench: `opt` 6.69 → `opt+win` 8.62 →
  `opt+win+sim` 8.49 (2 shared cells), coverage 2/3 unchanged; improvement over
  `opt+win` is noise-level. **Gate failed**: the front-end with simulation
  proves neither `core` nor `easy-win` at the pre-registered 10 M cap (nor any
  bench cell at 1 M), although replay attempts succeed 12–27 % and cut front-end
  frames ~20–24 % at 10 M. Verdict **PIVOT**: the second form
  (`replay_threat`) and further sibling-simulation levers are not pursued; the
  candidate/verification-layer role from report3 §7 is the recommended next
  plan session. One ledger row.
- **2026-11-02** — item 3 executed ([`plan3.md`](plan3.md),
  [`report3.md`](report3.md), measurements in
  [`measurements/plan3/`](measurements/plan3/README.md)): the opt-in
  attacker-only win front-end exists (`src/search/winprover/`, 32 MB scratch
  TT, `--win-front-end` default off, `--win-front-end-budget` 1 M), returns a
  replay-validated `Win` + `PvStatus::WinFrontEnd` or falls back to a
  bit-identical general search, and emits proof events for every proven node.
  The item-4 census is recorded in the plan/report. Bench: **neutral** — the
  front-end proved 0/3 cells and added 1,000,0xx evals / ~0.35 s per cell;
  geomean `opt` 6.81 → `opt+win` 8.75, coverage 2/3 unchanged. One ledger row;
  no default-flag decision. Item 4 (sibling simulation) is next with a
  front-end re-measurement gate; a pivot to a candidate/verification layer is
  the fallback.
- **2026-11-02** — item 2 executed ([`plan2.md`](plan2.md),
  [`report2.md`](report2.md), measurements in
  [`measurements/plan2/`](measurements/plan2/README.md)): the differential
  checker oracle is in place. New `src/proof_tree/export.rs` validates and
  replays a finalized tree, then writes provari's JSON-lines proof format;
  `examples/provari_export.rs` is the thin driver. All 168 committed
  root-`Win` proof-tree shards (33,020 nodes, largest 4,762) were exported and
  checked by the pinned oracle (`48686bc6…`, `--seconds 60`, no tablebases):
  **168/168 accepted**, no `convention-repeat`/`unchecked`/`export-bug`/
  `suspect`, so the verdict is **CLEAN** and no defect-fix plan is needed.
  Three corrupted-export controls are all rejected by the checker, the export
  is byte-deterministic across two full runs, and 25 trees' duplicate attacker
  positions never disagree on the winning move. The 94 root-`Loss` shards are
  plan scope (horizon item 8). No ledger row (the win path is untouched).
  Item 3 (win-only front-end, with the item-4 census in its plan session) is
  the recommended next plan.
- **2026-11-02** — item 1 executed ([`plan1.md`](plan1.md),
  [`report1.md`](report1.md), measurements in
  [`measurements/plan1/`](measurements/plan1/README.md)): the pinned oracle was
  rebuilt from source (bundled build, model artifacts fetched, nothing
  committed) and `core + sample20` was measured under both guides. Provari
  `--guide hand` wins exactly the three shipped bench cells (3/21 overall,
  2/20 on `sample20`), so the fixed selection rule confirms the set with no
  addition or removal; the stretch `--guide model` column wins 5/21 (4/20).
  Our stacks: `default` 1/21, `opt` 2/21; shared factors `hand×default`
  51.739 (1 cell) and `hand×opt` 6.848 (2 cells), matching `bench-v0` on a new
  host. No ledger row (the plan touches no win path); `bench.md` gained a
  dated reference note. Model column measured (no `INCONCLUSIVE`). Item 2
  (differential checker oracle) is the recommended next plan.
- **2026-10-30** — performance ledger opened: [`bench.md`](bench.md) with the
  `bench-v0` baseline (provari `--guide hand` rebuilt from the pinned commit,
  three-cell set, canonical 10 s/192 MB protocol, driver in
  `measurements/bench/`). Baseline: shipped default stack 51.47× on the one
  canonical shared cell (coverage 1/3); shipped opt-in stack
  (`--move-class-pn --quiet-grouping --eval-pn or`) 6.86× geometric mean on two
  cells (coverage 2/3; the third cell is 9.70× only at a 120 s budget). Distance
  to the goal (geomean < 1.0, coverage 3/3): ~7× plus one coverage-closing
  mechanism. No product code changed.
- **2026-10-30** — initiative opened by consolidating `provari`, `interop` and
  the crate-promotion thread of `workspace` (all three closed into this
  initiative). No product code changed. The comparison target was clarified:
  provari `--guide hand` is the primary column, `--guide model` is a recorded
  stretch column. Items re-ranked by dependency, not by prior plan number:
  baseline first, independent checker second (it gates trust in item 3's new
  proof shapes), then the front-end pivot ahead of sibling simulation. The
  `workspace` plan1 staleness finding is recorded above.
