# Plan: Gate 4b — residual training iteration (option B)

Status: Rust side implemented; trainer side pending in the external repo
(see `report7.md` when written).

Gate 4 (`plan6.md` + `report6.md`) failed its success bar: child_evals
dropped 44% at fixed wall time, but wall time regressed +8.3%, and at fixed
*effort* (m22_white, 60 s runs) the network needed 1.76x more evals — the
ordering itself is worse than the tuned `StaticAtomicScorer`, not just
slower. Option B is one bounded iteration on the training side before
closing the PoC. It changes **what the network is trained to produce**
(a residual correction to the static ordering instead of an absolute
ranking) and **how sibling pairs are weighted** (by recorded work), and
regenerates the corpus with deeper solves. The Gate-4 measurement harness
and success bar are reused verbatim.

## Pinned decisions

1. **Composition (inference):** the final ordering score becomes
   `static + nn + history + killer` — the network *adds* to the
   `StaticAtomicScorer` term instead of replacing it (this reverses the v1
   replacement semantics of `concept.md` §6 and `nn.md` §5). Rationale:
   static scores span up to `score_winning_capture = 100_000_000` while
   `NnMoveScorer` margins map to ~10^3–10^4 i32 units (`NN_SCORE_SCALE =
   4096`), so an additive correction is scale-coherent: it cannot override
   a winning capture but can reorder quiet moves.
2. **Residual target (training):** the pairwise logits include the static
   prior, so the network learns the *correction* to the static ordering,
   not an absolute ranking. For a sibling pair `(i, j)` at a node with
   static scores `static_k` and network margins `s_k`:
   `z_ij = (s_i − s_j) + λ · (ŝ_i − ŝ_j)` with
   `ŝ_k = static_k / max(1, max_l |static_l|)` (per-node max
   normalization; the static top pick becomes a strong prior with bounded
   influence, and a single outlier move dominates the prior exactly as it
   dominates the ordering). `λ = 1.0` for this iteration (open parameter,
   `nn.md` §9).
3. **Work weighting:** each pair's loss contribution is weighted by
   `1 + log2(1 + w)` where `w` is the work of the *cheaper* child of the
   pair for AND rank-by-work pairs, and the work of the proven decisive
   child for OR one-vs-rest pairs (censored negatives keep weight on the
   decisive side). `log2` keeps heavy nodes dominant without letting the
   few largest subtrees swamp the corpus.
4. **Corpus:** regenerate `data/corpus/train.ndjson` with the same
   `--suite quick` (m20–m22 stay held out by construction) but with a
   deeper, budgeted solve (`--timeout 420 --budget-seconds 19200
   --pt-size 1024`, see Step 3). Schema and
   `atomic-corpus/2` version are unchanged (`static_scores` per row already
   carry the per-node OR/AND static scores needed for the residual target).
   **Row accounting caveat (learned while executing):** a corpus row is
   marked `partial` when its case hit the solve cap or needed a synthesized
   root, and the trainer drops partial rows. The timeout-20 corpus kept
   9,130 clean rows; the first timeout-60 regeneration grew *raw* rows
   26,475 → 75,070 but clean rows only 9,130 → 9,275 — deeper caps only pay
   off for cases whose search *converges* (a proven root that is still
   refining neither grows its tree nor clears its timeout flag). The
   budgeted deep generation exists to convert cap-burner cases into
   converged ones; its worth is measured in clean rows, not raw rows.
5. **Weight file:** the §10 v1 byte format is unchanged (same tensors);
   new artifacts are named `weights.v2.bin` (v2 *training recipe*) plus a
   summary JSON. The seed-0 fixture and all format conformance tests stay
   byte-identical. The v1 weights remain loadable but semantically stale
   (they were trained for replacement composition).
6. **Success bar:** unchanged from Gate 4 — >=10-15% reduction in
   `child_evals` **and** wall time on
   `benchmark --suite move-order --first-outcome --json` with identical
   `--epsilon/--tt-size`, `wrong == 0`, with the long-timeout
   deconfounding runs (`--timeout 60` on m20–m22) reported alongside.

## Step 1 — Rust: residual composition at inference

- `src/search/dfpn/history.rs` `sort_moves`: always compute the
  nearest-commoner map and the static term; when the NN scorer is set, add
  its score to the static term (history + killer stay additive as before).
  Removes the `nn_scores.is_none()` branch around `nearest_commoner_map`.
- `examples/move_order_fractions.rs` `rank_samples`: rank by
  `static + nn` in the NN branch (matching `sort_moves`).
- Doc comments that state the replacement semantics: `src/nn/mod.rs`,
  `src/nn/scorer.rs`, `AGENTS.md` (nn paragraph), `concept.md` §6.
- Tests: `tests/test_nn.rs` keeps its solve-correctness assertions; add a
  residual-composition test asserting that with the NN scorer enabled the
  first move for a position where the static top pick is wrong-but-static-
  plausible changes per the additive rule, or minimally that scores are
  static + nn (a small unit-level check through `move_scores` + scorer is
  acceptable — the composition itself lives in `sort_moves`, which is not
  directly unit-testable; an integration ordering test is enough).

## Step 2 — Spec updates

- `docs/spec/nn.md` §5: inference composition is now
  `static + s (scaled)` before the mask/sort.
- `docs/spec/nn.md` §6: add the **v2 training recipe** subsection —
  residual logits (decision 2) and work weighting (decision 3); keep the
  v1 recipe documented as history.
- `docs/plans/nn/concept.md`: status line + §6 composition decision.
- `Makefile` `nn_corpus`: regenerate at `--timeout 60`.

## Step 3 — Corpus regeneration (this repo)

Executed as a single budgeted deep generation (see the Step-4 corpus
decision's row-accounting caveat): `--suite quick --timeout 420
--budget-seconds 19200 --tt-size 64 --pt-size 1024` — `corpus_gen solve`
gained `--budget-seconds` (even split of the remaining wall budget over the
remaining cases, capped by `--timeout`; underspent cases leave more for the
rest) and `--exclude <NAME>` for this. Outcome:

- Solve phase: 59/59 cases, 9,251 s (~2.6 h) of the 19,200 s budget; 37/59
  cases converged (clean rows), 22 refine indefinitely.
- A 900 s probe on the three largest burners (`dec10`, `m23_white`,
  `dec01` as control) did not converge any of them — the remaining burners
  are at the practical clean-data ceiling of these fixtures; deeper caps
  were not pursued further (~3.5 h of the 6 h envelope used in total).
- `load`: raw_rows 73,248 → 24,109 rows after hash dedup, of which
  **9,552 clean** (partial 14,557, dropped by the trainer) vs 9,275 clean
  in the timeout-60 corpus and 9,130 at timeout-20. Four additional cases
  converged (dec03, dec07, dec16, dec34, +277 clean rows).
- `Makefile nn_corpus` updated to this recipe; `weights.v1.bin{,.json}`
  preserved across the wipe; m20–m22 verified absent.


## Step 4 — External trainer delta (trainer repo, outside this session)

The trainer implements the §6 v2 recipe against the regenerated corpus:

1. Load `static_scores` per row (already in `atomic-corpus/2`; keyed by
   UCI, promotion variants share one policy index — dedup as today).
2. Residual logits per decision 2; λ configurable, default 1.0.
3. Pair weights per decision 3.
4. Emit `weights.v2.bin` (§10 v1 bytes) + summary JSON
   (`recipe: "residual-v2"`, λ, weighting, corpus meta) + refreshed seed-0
   fixture **only if the fixture recipe changed** (it must not; keep the
   byte-frozen fixture).
5. Same validation-split discipline: split by `source`, m20–m22 absent by
   construction; report per-source val row counts.

## Step 5 — Re-measure (Gate 4 harness, verbatim)

When `data/corpus/weights.v2.bin` lands:

```
cargo run --release --example benchmark -- --suite move-order --first-outcome \
  --runs 3 --json --nn-weights data/corpus/weights.v2.bin
cargo run --release --example benchmark -- --suite move-order --runs 1 \
  --timeout 60 --first-outcome --json --nn-weights data/corpus/weights.v2.bin
cargo run --release --example move_order_fractions -- --suite move-order \
  --nn-weights data/corpus/weights.v2.bin
```

plus the baseline runs from `report6.md` (unchanged — baseline does not
load weights). Judge against the unchanged success bar and write
`report7.md`. One eval-throughput improvement is in scope *before*
measuring if trivially available: none — batching/incremental inference is
explicitly out of scope here (Gate 4b tests the *ordering quality*
hypothesis; throughput work is only worthwhile if quality wins first).

## Non-goals

- No change to the network architecture, feature layout, or weight-file
  format.
- No inference-throughput engineering (batching, incremental stage 2–5).
- No change to the held-out discipline: m20–m22 never enter the corpus.
