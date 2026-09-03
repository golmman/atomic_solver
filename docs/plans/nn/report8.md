# Report: NN PoC closure (MVP) + ordering-quality headroom exploration

Implements `docs/plans/nn/plan8.md`. All measurements in this report were
produced on the reference container; raw outputs live in
`docs/plans/nn/measurements/plan8/`.

## Step 1 — MVP closure (docs only)

Done. `concept.md` status marked the PoC closed as MVP after Gate 4b
(quality validated, throughput capped, `--nn-weights` flag-gated);
`AGENTS.md`'s nn paragraph carries the closure sentence pointing at
`report7.md` and `plan8.md`.

## Step 2 — Oracle trees

14 cases generated (the plan says 13 — a miscount: the fixture has 19
cases, minus the 5 excluded `m20*`/`m21*`/`m22_black` = 14). All 14
converged well under the 60 s budget, so none were dropped:

```
m22_white, m23_white, m23_black, m24_white, m24_black, m25_white,
m25_black, m26_white, m26_black, m27_white, m27_black, m28_white,
m28_black, m29_white
```

Each dump verified: root outcome decisive, node count > 1,
`ProofTree::from_bin` round-trip (re-verified on every `oracle_floor` run).

One deviation from the plan's command: `--outcome-only` disables the
pre-exit hook that writes the dump, so the generation omits it and
redirects stdin from `/dev/null` instead (the solver's stdin reader would
otherwise consume the case list).

## Step 3 — Oracle-floor measurement

### Implementation

- `Search::set_ordering_scorer(Option<Arc<dyn MoveScorer>>)` added next to
  `set_nn_scorer`; when set, the override *replaces* the static base term
  (`override + nn + history + killer`). `sort_moves` is bit-identical when
  no override is set; the full unit suite passes unchanged.
- One deviation from the plan text: `MoveScorer::score` gained an
  `is_or_node: bool` parameter. Without it the oracle's static fallback
  could not reproduce the baseline's AND-node profile (scaled-down
  speculative bonuses), and the measured confound was large — a pure
  fallback scorer (0% coverage) already scored 0.584x evals on
  m22_white purely from the wrong node-type profile. The fallback must be
  baseline-identical for the floor to be interpretable.
- The oracle scorer (`examples/oracle_floor/oracle.rs`) looks positions up
  by exact Zobrist hash (decision 3). The v2 dump format stores no
  per-node hashes (`binary.rs` is driver-free), so the tree is replayed
  from `root_fen` and hashes recomputed per path — the same approach as
  `corpus_gen`. Coverage (resolved-from-tree / distinct positions ordered,
  plus a rule50-ignoring board-hash diagnostic) is reported per case.

### `decompose` (static analysis, no search)

Structural findings that shape the interpretation:

- The finalized proof tree keeps only the winning claim:
  `reconcile_children` keeps exactly one (shallowest) `Loss` child per
  `Win` parent, so refuted OR siblings are **not recorded**. Decision 2's
  "disproven siblings ranked by recorded work" category is therefore
  empty in dumps; every non-decisive legal move at an OR node is a
  censored negative (ranked below the decisive child, static order
  within).
- Their cost is however included in the OR node's own cumulative `work`.
  The decomposition therefore measures the **decisive-child share of OR
  node work** — the fraction perfect OR ordering cannot avoid:

| case       | OR decisive share | recoverable (refutation + own) |
|------------|------------------:|-------------------------------:|
| m22_white  |             95.6% |                           4.4% |
| m23_black  |             61.0% |                          39.0% |
| m23_white  |             81.2% |                          18.8% |
| m24_black  |             70.9% |                          29.1% |
| m24_white  |             23.5% |                          76.5% |
| m25_black  |             53.6% |                          46.4% |
| m25_white  |             63.6% |                          36.4% |
| m26_black  |             51.9% |                          48.1% |
| m26_white  |             62.1% |                          37.9% |
| m27_black  |              5.5% |                          94.5% |
| m27_white  |             17.3% |                          82.7% |
| m28_black  |            100.0% |                           0.0% |
| m28_white  |              4.4% |                          95.6% |
| m29_white  |            100.0% |                           0.0% |
| **aggregate** | **90.6%**      | **9.4%**                       |

  Per-node OR decisive shares are strongly bimodal (median 68–100% per
  case): most OR nodes resolve immediately on a rank-1 winning capture; a
  minority absorb nearly all refutation cost.
- AND-node child work is heavily concentrated (aggregate child-share
  median 52.9%, per-node max-share median 100%): disproving work sits in
  one or two replies per AND node, so AND ordering is a real lever.
- Caveat: `finalize()` copies canonical subtrees onto transpositions, and
  copied children carry the original subtree's `work`, which can exceed
  the parent node's own cumulative work (e.g. 1,258 inflated nodes on
  m22_white). Per-node shares are clamped to 1 and the aggregate uses
  `min(decisive, node work)`; raw values are in `decompose.txt`.

### `solve` (measured floor)

Identical settings per run (`--timeout 60 --epsilon 0.125 --tt-size 64`,
first-outcome); baseline runs are deterministic and reproduce report7's
counts exactly (m22_white 37,503,264).

**Comparison base:** the baseline is the shipped, hand-tuned
`StaticAtomicScorer` heuristic. The network is *not* part of this
measurement — it enters only through report7's separate fixed-effort
number. On the dominant case the ladder is:

| ordering (m22_white)           | evals vs heuristic |
|--------------------------------|-------------------:|
| tuned heuristic (shipped)      |             1.000x |
| v2 network (report7, measured) |              0.72x |
| oracle ceiling (this report)   |            0.407x  |

The oracle is an *improvement upon* the heuristic, not a replacement:
positions absent from the oracle tree fall back to the heuristic order,
so the measured floor is what per-node tree knowledge adds on top of the
shipped ordering.

| case       | baseline evals | oracle evals | eval ratio | coverage | board-hash cov |
|------------|---------------:|-------------:|-----------:|---------:|---------------:|
| m22_white  |       37.50 M  |     15.28 M  | **0.407x** |    0.4%  |      0.6%      |
| m23_white  |        9.78 M  |      7.14 M  |   0.730x   |    0.2%  |      0.3%      |
| m23_black  |        2.84 M  |      2.36 M  |   0.830x   |    1.8%  |      2.1%      |
| m24_white  |        0.408 M |      0.857 M |   2.104x   |    1.0%  |      1.1%      |
| m24_black  |        0.093 M |      0.131 M |   1.396x   |    6.7%  |      8.1%      |
| m25_white  |        0.023 M |      0.028 M |   1.238x   |    3.5%  |      4.1%      |
| m25_black  |        0.005 M |      0.005 M |   1.055x   |   12.2%  |     12.2%      |
| m26_white  |        1,086   |      1,149   |   1.058x   |   16.9%  |     16.9%      |
| m26_black  |        1,011   |        959   |   0.949x   |   22.2%  |     22.2%      |
| m27_white  |          257   |        495   |   1.926x   |    7.7%  |      7.7%      |
| m27_black  |          126   |         40   |   0.317x   |  100.0%  |    100.0%      |
| m28_white  |           89   |         35   |   0.393x   |  100.0%  |    100.0%      |
| m28_black  |            3   |          3   |   1.000x   |  100.0%  |    100.0%      |
| m29_white  |            1   |          1   |   1.000x   |  100.0%  |    100.0%      |
| **aggregate (work-weighted)** | 50.65 M | 25.81 M | **0.509x** | 0.5% | — |
| **unweighted mean** |         |              |   1.029x   |          |                |

Wall time (informational; the oracle scorer pays its own lookup cost):
aggregate 0.71x, i.e. the eval reduction outweighed the scorer overhead
on the big cases.

Reading of the numbers:

- The work-weighted aggregate floor is **0.509x**, dominated by
  m22_white (0.407x). It is **not meaningfully below 0.5x** — it is not
  below it at all.
- The unweighted mean is **1.029x**: on typical (non-extreme) positions
  the tree-guided oracle does not help at all, and on several medium
  cases it actively hurts (m24_white 2.1x, m27_white 1.9x) — ranking AND
  children by recorded work ascending interacts badly with DF-PN's
  pn/dn dynamics on small trees.
- Lookup coverage is very low exactly where it would matter (0.2–0.4% on
  the three hardest cases; decision 3 anticipated misses from
  halfmove-clock differences, but the measured board-hash coverage shows
  the misses are mostly *structural*: the oracle search proves a
  different, smaller tree than the baseline's, so most of its nodes are
  simply absent from the baseline dump).
- **What the ratio is not:** it cannot be read as "the heuristic has
  OR-node branching factor ≈ 2". The OR-side branching factor is
  directly measurable and much lower: ≈ **1.10** in work terms
  (1/0.906, the decisive-child share from the decomposition) and ≈
  **1.4** in children-tried terms (mean decisive-child rank under the
  static ordering, `move_order_fractions` rank distribution). The
  remaining reduction in the 0.509x comes from AND-side ordering,
  proof-shape changes (the oracle proves a *different, smaller* tree),
  and restart dynamics — none of which are OR-node branching. The clean
  statement: perfect OR ordering alone is bounded by the decomposition
  at ~9.4% of OR work; the learnable signal lives mostly on the AND
  side.

## Step 4 — Practical NN ceiling: skipped

The bounded trainer sweep (λ × epochs) was not run. Its only consumer
was a positive Step-5 verdict ("floor < 0.5x → measure the best variant
and draft plan9"). The verdict below is negative, so the sweep would
inform nothing; running it would spend ~3 h of solver/trainer time for
numbers with no decision attached. The v2 weights and the Gate-4 harness
remain available should the question ever be reopened.

## Step 5 — Verdict against the pinned 0.5x bar

**Pinned decision 1**: reopen only if the oracle-floor eval ratio is
*meaningfully below 0.5x* on decisive cases.

**Measured floor: 0.509x work-weighted aggregate (1.029x unweighted).**

**Verdict: the bar is not met — the PoC closes permanently.**

The number and the judgment, not a shrug:

1. The aggregate floor sits *above* the bar. Even granting the oracle
   reading the most favorable case — m22_white at 0.407x — that is a 19%
   margin below 0.5x on one case, achieved with 0.4% lookup coverage;
   the plan's "meaningfully" demands room for the practical NN gap on
   top, and today's v2 network sits at 0.72x on that same case (1.77x
   above the measured floor), with a 1.6–2.2x throughput penalty (2.1x
   measured, ~1.2–1.3x best-case post-optimization).
2. The unweighted mean of 1.029x says the headroom is concentrated in
   the single hardest benchmark, not a general ordering-quality deficit:
   a network that perfectly matched the oracle would still lose wall
   time on every medium case.
3. The decomposition explains why: 90.6% of OR-node work is already spent
   on the decisive child (recoverable ≤ 9.4% locally). The oracle's
   aggregate win comes mostly from AND-node ordering and second-order
   DF-PN steering — signals a ranker can plausibly learn, but the
   ceiling they add up to (0.509x) is inside the gate's failure region
   once the throughput penalty is applied.

One honest caveat, stated rather than smoothed over: the plan's preamble
derives the 0.5x bar from "0.5x evals × ~1.2–1.3x post-optimization
penalty is still a wall-time loss", whose arithmetic does not hold
(0.5 × 1.25 = 0.625x wall would pass). The pinned decision, not the
preamble's derivation, is binding, and the measured floor fails it. Had
the floor landed at e.g. 0.45x the correct reading of the pinned bar
would still have been "reopen"; at 0.509x aggregate / 1.03x unweighted
it is not.

## Durable artifacts (carried forward per decision 7)

- `--nn-weights` path, residual-v2 recipe, `weights.v2.bin` — unchanged,
  flag-gated.
- Recorded per-child `work` labels (corpus `atomic-corpus/2`) and the
  Gate-4 harness (`move_order_fractions`, `work_proxy_ablation`).
- New: `oracle_floor` example + the oracle-tree generation recipe
  (`measurements/plan8/README.md`), reusable if the question is ever
  reopened with a better oracle methodology.
- `Search::set_ordering_scorer` hook (generic `Arc<dyn MoveScorer>`
  override replacing the static base term).

## Problems encountered

- Plan Step-2 command contained `--outcome-only`, which disables the
  pre-exit hook that writes the dump (omitted; see above).
- Plan said 13 converging cases; the fixture arithmetic gives 14.
- The v2 dump stores no hashes; the oracle scorer replays the tree from
  `root_fen` (corpus_gen approach) — decision 3's halfmove-clock caveat
  applies exactly as written, but turned out to be the *smaller* miss
  source (board-hash coverage ≈ full-hash coverage everywhere).
- Refuted OR siblings are structurally absent from dumps
  (`reconcile_children` keeps one `Loss` child per `Win` parent);
  decision 2's disproof-sibling ordering is vacuous on real dumps and
  the decompose metric was reframed to the decisive-child share of node
  work.
- Transposition copies double-count child `work` after `finalize()`
  (child work can exceed parent work); per-node shares clamped, counts
  reported.
- A replay DFS bug (leaf moves never undone) briefly produced illegal
  positions; caught by `do_move` panics and fixed.

## Verification

- `cargo test --release`: all pass (the `nn_scorer_is_residual` and
  `test_nn` suites unchanged — bit-identical NN-path ordering; the
  `MoveScorer` trait change required adding the `is_or_node` argument to
  21 static-scorer unit-test call sites, values unchanged).
- `cargo fmt --check`, `cargo clippy --release --all-targets`: clean.
- Harness-drift guard: `move_order_fractions --suite move-order`
  per-case results bit-identical to
  `measurements/gate4b/fractions_baseline.txt` for every
  non-timeout-bound case; the aggregate differs (68.9% vs 69.5% flat
  rank-1, work-weighted 31.4% both) only through the wall-clock-sensitive
  m20–m22 timeout cases (14806 vs 14863 OR nodes) — environmental, not
  harness, drift.

## Missing tests / unresolved parts

- No automated tests for `oracle_floor` (example-layer, consistent with
  the other examples); its correctness rests on the decompose/solve
  cross-checks, deterministic baseline reproduction, and coverage
  reporting.
- The oracle is baseline-tree-guided, not truly ideal: a genuinely
  optimal ordering oracle would need either search-time recognition of
  any equivalent proof or path-independent (GHI-safe) hashing. The
  measured floor is therefore an upper-bound estimate of the ideal
  ordering's benefit — which makes the negative verdict conservative
  *against* closure on the aggregate, and it still fails the bar.
- Step 4 (trainer sweep) skipped, justified above.

## Next steps

- None on the NN path: the PoC is closed permanently. `concept.md`
  updated accordingly.
- If ordering quality is revisited someday, the two levers this
  milestone identified are AND-node ordering (where disproving work
  concentrates) and inference throughput — but the measured ceiling
  (0.509x) leaves no room for both to pay for themselves together.
