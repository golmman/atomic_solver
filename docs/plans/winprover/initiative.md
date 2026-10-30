# Initiative: `winprover` — an independently-checked atomic win prover that beats provari

## Status

**Active (opened 2026-10-30).** This initiative consolidates three earlier ones
into a single goal: [`provari`](../provari/initiative.md) (search levers,
closed into this initiative), [`interop`](../interop/initiative.md)
(independent proof validation, closed into this initiative) and the crate
promotion thread of [`workspace`](../workspace/initiative.md) (closed into
this initiative as optional item 7). Their plan/report history and their
measurement artifacts stay where they are; only the backlog moves.

Plan numbering here is fresh (`plan1.md` is the next plan) and *independent*
of the retired initiatives' numbering. The mapping is recorded in
[Renumbering record](#renumbering-record).

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
| 1 | Fair baseline: hand-guide column + corpus re-derivation | Re-run the pinned provari at `--guide hand` over the frozen `sample20` (and the core FEN); re-derive shared-win set, coverage and wall ratios; record both columns side by side | Every later verdict is priced against this; the retired baseline mixes an out-of-scope model into the target | wall, nodes/child evals, coverage per column | **next — plan1** |
| 2 | Differential proof checker oracle | Export an existing `proof_tree.bin` to provari's JSONL format; run `provari check`; classify every mismatch | Independent rules implementation catches correlated `atomic-movegen`/`Position` bugs; the exporter is reusable, and new proof shapes (item 3) need it in place *before* they are trusted | per-tree verdict class, defect count | open — re-cut from the drafted [`interop/plan1.md`](../interop/plan1.md) against the current binary format |
| 3 | Win-only front-end skeleton | Opt-in attacker-only win prover that proves a win and otherwise falls back; no new heuristics; includes the item-4 sibling-simulation **census** in its plan session | Isolates provari's attacker-only proof semantics from the general outcome/PV machinery; the architectural pivot, and the home for items 4–5 | correctness (fallback identity), evals, wall | open — **re-ranked ahead of item 4** (see note) |
| 4 | Sibling proof replay / simulation | Replay a proven defence or threat on sibling positions (provari's `SIMULATION = 16` replay) | The last unported provari search lever; the measured residual is node count (~222k expansions vs provari's 13.8k bundled / 72.5k hand on the shared win at 14.5 child evals per expansion) | evals, nodes, wall, replay soundness | open — census in item 3's plan session |
| 5 | Rule50-free key + proof length/limit | Key boards without the clock; store proof length and highest holding clock; replay/verify conservatively | Recover move-order transpositions without breaking rule50/repetition soundness | TT hit rate, evals | open (soundness-sensitive) |
| 6 | Head-to-head convergence | Tune the adopted stack on the target corpus; decide the default flag set; require shared-win wall < provari (primary column) and coverage ≥ provari | The initiative's success condition; no new mechanism | wall ratio, coverage | open (deferred until the search levers settle) |
| 7 | **Optional:** crate promotion / workspace split | Re-draft and execute the workspace split (root package + `apps/proofdb` + a `winprover` crate) *if* the front-end grows a test surface or consumer worth isolating | Clean boundaries for the new engine; never a precondition, and not needed for codegen (`lto`, `codegen-units = 1`) | packaging only (byte-identical solver output, gate green) | open, unranked — **requires a re-draft**: [`workspace/plan1.md`](../workspace/plan1.md) is stale (see Renumbering record) |
| 8 | Horizon — evidence extensions | Root-`Loss` coverage + compact `.pvr`; portable attacker-only export for proofdb transport; external `high200` corpus; native second rules implementation; cannot-win material | Completes checker coverage and interchange; out-of-distribution corpus guards against overfitting | — | unranked; items 2–5 of [`interop/initiative.md`](../interop/initiative.md) and the `provari` 4b / cannot-win threads |

**Item 3 ahead of item 4.** `provari` report 5 ranked sibling simulation next,
but that ranking assumed the current `dfpn` frames. Sibling replay's home is
the attacker-only front-end, so implementing it in `dfpn` first risks writing
it twice; the census is read-only and belongs in item 3's plan session, which
prices both before either is implemented. If the front-end is rejected, item 4
moves back to the top of the ranking.

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

- **2026-10-30** — initiative opened by consolidating `provari`, `interop` and
  the crate-promotion thread of `workspace` (all three closed into this
  initiative). No product code changed. The comparison target was clarified:
  provari `--guide hand` is the primary column, `--guide model` is a recorded
  stretch column. Items re-ranked by dependency, not by prior plan number:
  baseline first, independent checker second (it gates trust in item 3's new
  proof shapes), then the front-end pivot ahead of sibling simulation. The
  `workspace` plan1 staleness finding is recorded above.
