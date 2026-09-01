# Task: Gate-4b training delta — residual logits + work weighting (v2 recipe)

> Meta note for the human, not the agent: this file is the prompt for the
> coding agent working **in the trainer repository**. Point the agent at
> it ("read and follow `docs/plans/nn/handoff_agent_train_plan7.md`").
> Delete this blockquote before pasting if the agent is confused by it.

## Context

You are working in the external Python/PyTorch trainer repository. It
already implements the Gate-2 v1 trainer (`docs/plans/nn/
plan_external_trainer.md`, `report_external_trainer.md`): corpus loading,
features, the tiny RankNet, pairwise loss, §10 weight-file writer, and the
seed-0 sample fixture. That pipeline produced `weights.v1.bin`, which
**failed** its benchmark gate (details: `docs/plans/nn/report6.md` if
present).

Gate 4b (`docs/plans/nn/plan7.md`) is **one bounded training-side
iteration**: change what the network is trained to produce (a residual
correction to the static ordering instead of an absolute ranking) and how
sibling pairs are weighted (by recorded work), then re-train against the
regenerated corpus. The architecture, feature layout, and §10 weight-file
byte format are all **unchanged** — you are modifying the loss/recipe and
the run plumbing only.

Read first, in this order:
1. `docs/spec/nn.md` — normative. Especially §5 (inference composition is
   now `static + s (scaled)`), §6 (label semantics), **§6a (the v2 recipe
   you are implementing)**, §10 (byte layout, still version 1).
2. `docs/plans/nn/plan7.md` — the plan; its "Pinned decisions" 2 and 3 are
   the exact math.
3. `docs/plans/nn/plan_external_trainer.md` + `report_external_trainer.md`
   — the existing trainer you are extending.
4. `data/corpus/train.ndjson` `_meta` line — the regenerated corpus.

## Corpus (regenerated — verify, don't assume)

The committed `data/corpus/train.ndjson` was regenerated on the Rust side
with a deep budgeted solve (`--suite quick --timeout 420
--budget-seconds 19200 --pt-size 1024`). Facts pinned by the `_meta` line
(`atomic-corpus/2`; it is authoritative over any doc):

- 24,109 rows, of which **9,552 clean**; `partial: true` rows (timed-out
  or synthesized-root cases) are **dropped** by default, as before.
- 59 cases but **54 distinct `source` values** (five cases were fully
  absorbed by cross-case transposition dedup). The validation split
  operates on sources; four cases (`dec03`, `dec07`, `dec16`, `dec34`)
  are new convergers vs the previous corpus.
- All existing parsing caveats still apply: `hash` is a decimal u64 —
  parse with exact `int` semantics, never `float`; `static_scores` keys
  are JSON-sorted — align via `legal_moves[i]`, never key order; FENs use
  standard `k`/`K`; the other-side transform is color swap + file mirror
  (`f → 7 − f`); `children[].work >= 1` for every expanded child.

## Deliverables

### 1. Recipe flag and plumbing

Add `--recipe {v1,v2}` (default **v1**, so the previous behavior stays
reproducible byte-for-byte) and `--lambda FLOAT` (default `1.0`, v2 only).
All v1 code paths stay intact.

### 2. Residual logits (v2, spec §6a / plan7 decision 2)

For a sibling pair `(i, j)` at a node with static scores `static_k` and
network margins `s_k`, the pairwise logit becomes:

    z_ij = (s_i − s_j) + λ · (ŝ_i − ŝ_j)

with the per-node max-normalized static prior

    ŝ_k = static_k / max(1, max_l |static_l|)

`static_k` comes from the row's `static_scores` (keyed by UCI; promotion
variants share one policy index — dedup as today). Worked example: static
`{capture: 100_000_000, quietA: 560, quietB: 0}` → `ŝ = {1.0, ~0, 0}` —
the prior strongly protects the capture top pick, while pairs *among the
quiets* get a ~zero prior and are decided by the network margins. This is
the point: the network learns only the correction.

### 3. Work-weighted pair loss (v2, plan7 decision 3)

Each pair's loss contribution is weighted by

    1 + log2(1 + w)

where `w` is:
- the work of the **cheaper** child of the pair for AND rank-by-work
  pairs, and
- the work of the **proven decisive** child for OR one-vs-rest pairs
  (censored negatives keep weight on the decisive side).

Label/pair semantics are otherwise **unchanged from v1**: OR rows are
one-vs-rest (every legal move that is not decisive is a negative, including
never-expanded moves with no `children[]` entry); AND rows rank children
by `work` **ascending** (cheapest first); a censored move is never the
preferred element of a pair and no pair is formed between two censored
moves.

### 4. Outputs

- `data/corpus/weights.v2.bin` — §10 v1 bytes, same header/dims/size as v1
  (967,312 bytes). No format change, no version bump.
- `data/corpus/weights.v2.bin.json` — summary JSON with at least:
  `"recipe": "residual-v2"`, `lambda`, the weighting formula (as a
  string), the corpus `_meta` object verbatim, kept-row counts (total /
  clean / after-dedup as your pipeline sees them), the validation split
  description **with per-source validation row counts** (sources are
  heavily skewed — dec10 ~5k rows vs some with 2 — so which sources landed
  in validation matters), hyperparameters, seed, and the loss history.
- `docs/plans/nn/report_gate4b.md` — short run report: the exact input
  contract consumed (corpus meta, partial handling, pair semantics, λ),
  train/val losses (compare against v1's train 0.25 / val 0.53), and any
  deviations.

### 5. Fixture integrity (hard constraint)

`trainer/fixtures/weights.v1.bin` is **byte-frozen**. The seed-0 sample
fixture is generated by a fixed-seed writer that is independent of corpus
and recipe — it must not change. Verify before finishing:

    sha256sum trainer/fixtures/weights.v1.bin
    # must be cb6dafd458d6ad044204f65f4faf378223527eee4ef09e707c9771d4946db2e0

If it differs, you regenerated it by mistake — restore it. Do not add a
v2 fixture; the Rust loader conformance tests stay pinned to the v1
fixture.

## Tests

Add unit tests for the new math, with hand-computed values:
- `ŝ` normalization: all-zero static scores → `ŝ = 0` everywhere (the
  `max(1, …)` guard); single huge outlier → `ŝ = 1` for it, ~0 for the
  rest; negative scores.
- Residual logit: a small node where the prior dominates (capture vs
  quiet) and one where the margins dominate (quiet vs quiet, λ · Δŝ ≈ 0).
- Pair weights: `w = 1 → 2.0`, `w = 15 → 5.0`, `w = 255 → 9.0`.
- v1 regression: running with `--recipe v1` produces a loss curve matching
  the v1 run (or at minimum, identical pair construction and weights).
- End-to-end smoke on a tiny synthetic corpus for v2, and one real-corpus
  run (see below).

## Verification

```sh
uv run pytest                      # from the repo root (uv CWD rule)
uv run python -m trainer.train --recipe v2 --lambda 1.0 \
    --corpus data/corpus/train.ndjson --out data/corpus/weights.v2.bin
```

The real-corpus run is small (9.5k clean rows, ~242k-weight net, CPU
minutes) — actually run it; do not deliver untested recipe code.

## Constraints

- No change to the network architecture, features, or §10 byte format.
- No fixture regeneration; the byte-frozen fixture must hash-verify.
- Do not modify the corpus; if it looks wrong, stop and report.
- m20–m22 must be absent from training rows by construction — assert no
  `m20`/`m21`/`m22` sources in the training split.
- Keep the repo's uv discipline: run uv from the repo root, CPU-only
  torch index, commit code + docs, leave `.venv/`/`.uv-cache/` ignored.
- Out of scope: measuring ordering quality (the Gate-4 harness lives in
  the Rust repo and is run there), any Rust-side work, batching/incremental
  inference, further recipe iterations.
