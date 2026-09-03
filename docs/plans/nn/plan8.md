# Plan: NN PoC closure (MVP) + ordering-quality headroom exploration

Context: Gate 4b (`report7.md`) failed the wall-time half of the gate
(+6.0%) but validated the ordering-quality hypothesis: at fixed effort
(m22_white) the v2 residual network needs 0.72x the evals of the tuned
static baseline (v1 needed 1.76x), and both `move_order_fractions`
metrics beat the baseline (82.4%/70.4% vs 69.5%/31.4% rank-1). The
remaining deficit is purely the dense-inference throughput penalty
(~1.6–2.2x per eval), which is bounded and buyable. Decision (this
plan): close the PoC as an MVP — the static scorer stays the shipped
ordering, `--nn-weights` stays flag-gated — and spend the next milestone
measuring the *other* lever: how much ordering-quality headroom exists
at all.

The motivating asymmetry: throughput is capped (~2–3x recoverable), but
the node-reduction ceiling is unknown. The exploration must answer one
question with a pinned decision bar:

> Is the fair-comparison eval-ratio floor of *ideal* move ordering
> meaningfully below ~0.5x? If not, the NN path can never pass the gate
> (0.5x evals x ~1.2–1.3x best-case post-optimization throughput penalty
> is still a wall-time loss) and the PoC closes permanently.

The 0.5x bar derives from report7's numbers: the throughput penalty
floors around 1.2–1.3x under batching/quantization (2.1x today, 2–3x
total recovery assumed as the cap). Pin it now so the milestone cannot
drift into "a bit better than today is enough".

## Pinned decisions

1. **Decision bar:** the NN path is only worth reopening if the
   oracle-floor measurement (Step 3) yields an eval ratio meaningfully
   below **0.5x** on cases where both oracle-ordered and baseline
   searches produce a decisive outcome. "Meaningfully" = the oracle
   ratio minus the practical NN gap (today 0.72x oracle-unaware vs
   oracle-floor) leaves room; the report must state the number and the
   judgment, not a shrug.
2. **Oracle ordering definition:** at OR nodes the *proven decisive*
   child is ranked first; all other children (disproven siblings,
   censored negatives) are ranked next by their recorded `work`
   ascending (cheapest disprovals first). At AND nodes children are
   ranked by recorded `work` ascending. Positions absent from the
   oracle tree fall back to the `StaticAtomicScorer` order (the oracle
   is a per-node override, not a full ordering).
3. **Oracle lookup is by exact Zobrist hash.** The recorded hashes
   include the halfmove clock, so transpositions reached via different
   paths may miss. This is accepted: the example must report lookup
   *coverage* (share of nodes resolved from the tree vs static
   fallback) alongside every measurement, and low coverage invalidates
   the affected case.
4. **Dependency direction is preserved:** search gets a generic
   `Arc<dyn MoveScorer>` injection hook (today only
   `set_nn_scorer(NnMoveScorer)` exists); the oracle scorer itself lives
   in the example layer, which may depend on both `search` and
   `proof_tree`. No core search or proof-tree changes beyond the hook.
5. **Held-out discipline:** the oracle trees for the measured cases are
   generated fresh in Step 2 with the *baseline* config; nothing enters
   any training corpus. m20–m22 remain excluded from the trainer sweep
   data by construction.
6. **No weight-format or architecture change** in this milestone. The
   trainer sweep varies λ, epochs, dropout, and L2 only. A smaller
   hidden layer would change the §10 dims and the Rust parser's
   validation; it is deferred until the oracle floor says quality
   headroom exists at all.
7. **MVP closure is documentation-only:** no code removal. The
   `--nn-weights` path stays behind the CLI flag (off by default),
   `weights.v2.bin` stays loadable, and the durable artifacts (Gate-4
   harness, recorded-work labels, residual recipe) carry forward.

## Step 1 — MVP closure (docs only)

- `docs/plans/nn/concept.md`: status line → PoC closed as MVP after
  Gate 4b; outcome summary (quality validated, throughput capped,
  flag-gated).
- `AGENTS.md` nn paragraph: add one sentence marking the NN path as a
  closed PoC kept behind `--nn-weights` (residual-v2 recipe, quality
  validated, throughput-capped); point at `report7.md` and this plan.
- Verify nothing else references the PoC as active work.

## Step 2 — Oracle trees for the measured cases

Generate baseline proof-tree dumps for every `move-order` case that
converges at 60 s (from report7's timeout-60 runs: m22_white,
m23_white/black, m24–m29 both colors — 13 cases; m20*, m21*,
m22_black do not converge and are excluded):

```
cargo run --release -- --fen <case FEN> --timeout 60 --first-outcome \
  --outcome-only --tt-size 64 --dump-path data/oracle/trees/<case>.bin
```

(Case FENs come from the benchmark suite list in `examples/common.rs`;
create `data/oracle/trees/`, git-ignored like `data/corpus/`.) Verify
each dump: root outcome decisive, node count > 1, and
`ProofTree::from_bin` round-trips.

## Step 3 — Oracle-floor measurement (new example `oracle_floor`)

One new example binary, two modes:

1. `decompose` (static analysis, no search): load each dump and report
   per case + aggregate:
   - share of OR-node `work` spent on the decisive (rank-1) child vs
     non-decisive children — the upper bound on what perfect OR
     ordering saves;
   - AND-node `work` distribution across children (min/median/max
     share) — how much disproving work is concentrated, i.e. whether
     AND ordering is even a lever;
   - tree size and depth stats for context.
2. `solve` (measured): for each case, run the solver twice with the
   same settings as the report7 deconfounding runs
   (`--timeout 60 --runs 1 --epsilon 0.125 --tt-size 64`,
   first-outcome):
   - baseline (no scorer override) — fresh runs, same host;
   - oracle-ordered, via the new `Arc<dyn MoveScorer>` hook: per node,
     look up the position hash in the case's oracle tree and order
     children per pinned decision 2, falling back to static order
     (decision 2/3), reporting coverage.

Report per case: evals and wall time for both runs, eval ratio,
oracle-node coverage. The **oracle eval ratio** (oracle evals / baseline
evals on cases solved by both) is the floor number the decision bar
consumes. Reuse `examples/common.rs` helpers; keep the binary under the
~10 KB guideline (split the decomposition into a module if needed).

Plumbing detail: add `Search::set_ordering_scorer(Option<Arc<dyn
MoveScorer>>)` next to `set_nn_scorer` (the NN scorer then becomes a
producer of `Arc<dyn MoveScorer>` or the hook composes with it —
whichever keeps the residual `static + nn + history + killer` semantics
intact for `--nn-weights`; `sort_moves` behavior must be bit-identical
for the existing tests).

## Step 4 — Practical NN ceiling (external trainer, cheap sweep)

One bounded sweep in the trainer repo, §6a v2 recipe unchanged,
corpus unchanged: λ ∈ {0.5, 1.0, 2.0} x epochs ∈ {3, 6} (plus the
existing run as anchor; ~9 s/run). Same split discipline as
`report_trainer_gate_4b.md`. Deliver `weights.v3-sweep/<variant>.bin`
plus a one-paragraph summary of val losses. The single best variant by
val loss (not overfit to it — state both train/val) ships as
`weights.v3.bin` and is measured once on the Rust side with
`move_order_fractions --suite move-order --nn-weights` only (no
benchmark — this milestone judges quality ceilings, not the gate).

## Step 5 — Synthesis and verdict

`report8.md` must contain:

1. Oracle floor: per-case and aggregate eval ratios + coverage, static
   decomposition tables (Step 3).
2. Practical ceiling: best sweep variant's `move_order_fractions`
   metrics vs the v2 numbers (82.4%/70.4% rank-1) and vs the oracle
   floor — i.e. how much of the theoretical headroom the practical NN
   already captures.
3. The verdict against the pinned 0.5x bar, with one of two outcomes:
   - **Floor < 0.5x:** quality headroom exists → next milestone would
     be inference throughput (batching first) plus the training levers
     that the floor says are worth it; draft plan9.
   - **Floor >= 0.5x:** the PoC closes permanently; the residual-v2
     recipe and harness remain the durable artifacts, documented in
     `concept.md`.

## Verification

- `make test` after the `Arc<dyn MoveScorer>` hook (existing
  `nn_scorer_is_residual` and `test_nn` tests must pass unchanged —
  bit-identical ordering semantics for the NN path).
- `cargo fmt --check`, `cargo clippy --release` clean.
- `move_order_fractions` re-run once without weights must reproduce the
  baseline aggregate (69.5%/31.4%) — guard against accidental harness
  drift.

## Non-goals

- No inference-throughput engineering (batching, incremental updates,
  quantization) — that is plan9's subject, and only if Step 5 says so.
- No weight-format or architecture change (decision 6).
- No corpus regeneration, no NN-influenced corpus (echo-chamber risk
  and ~3 h solver cost; only justified after a positive Step 5).
- No changes to `search` beyond the scorer hook, none to `proof_tree`,
  none to the spec files under `docs/spec/`.
- No removal of the `--nn-weights` code path (decision 7).
