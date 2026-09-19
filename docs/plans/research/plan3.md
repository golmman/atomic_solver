# Plan 3: Subgame material-mass diagnostic (mid-search recognizer feasibility)

Initiative: `research` backlog #3 (problem inventory). Supersedes plan2's
"pursue #11/#12/#13" recommendation for this plan slot — see *Relationship
to plan2's recommendation* below.

## Goal

Measure how much first-outcome search work (child evals, frame entries) is
spent inside small-material subtrees of material-rich roots, and how much of
that work is threshold-cut churn. The report answers backlog #3: **does the
search of a 20–28-man root descend into ≤5-men subgames where a
preflight-style recognizer (region-closure fixpoint) could fire mid-search,
or does the work mass stay material-rich?**

This is a pure-measurement plan: throwaway instrumentation, reverted before
the report. No recognizer is built here.

## Context

- plan13's preflight (region-closure AND/OR fixpoint, `src/search/preflight/`)
  is the only demonstrated multiplicative node-count win in the solver's
  history: the KQvK ladder root went from >756M unproven nodes to
  5,866,690 child evals / 0.76 s (0.29× of the ≤20M bar), with certificates
  and a by-product draw proof. But the detector is root-only and narrow:
  ≤3 men, pawnless, no castling.
- plan2 showed all 12 unique outlier positions have 18–28 men at the root
  (58.3% of outlier occurrences are the 20-man `BNPPPPPPPRR vs PPPPPPR`
  family), so the detector never fires on the hard class. That says nothing
  about the *subtrees*: atomic blasts only remove pieces, so men count is
  **monotone non-increasing down any search path** — deep conversion lines
  plausibly funnel into small-material regions.
- Counter-evidence: the stress case's refined PV is a tempo conversion
  (pawn pushes + rook triangulation, long capture-free stretches), so the
  *principal line* does not simplify quickly. Whether the *searched tree*
  simplifies is unmeasured.
- plan1's profiler recorded `child_preflight = 0` on m22/stress — this only
  confirms preflight does not fire mid-search (it is root-only by design),
  not that it *could*.

## Relationship to plan2's recommendation

plan2 recommended POC #11 (dynamic ε), #12 (TT eviction), or #13 (frontier
prior). Two of the three are pre-weakened by measurements plan2 did not
engage with:

- **#13**: `lean` plan9 (2026-09-15) closed the ordering surface — OR
  winning child at rank 0–1 in ~97% of OR-Win frames, AND refuter at rank 0
  in 100% of refuted frames. A prior touches only the residual ~3%.
- **#12**: the `dfpn` position study measured the stress case's decisive
  point invariant across TT ∈ {32 MB, 2 GB}; the 0.35% TT-solved hit rate
  is a no-revisit property of the class, not an eviction artifact.
- **#11** survives as fallback: global ε ∈ {0, 0.125, 0.5} was measured
  inert on the stress case (2026-09-11, pre-plan9 — caveat recorded), but
  plan11 showed threshold-arithmetic changes are high-leverage (−37.7% and
  +110% arms). Depth-scheduled ε stays a cheap fallback plan if this plan
  returns NO-GO.

This plan takes the higher-information-value measurement first: either
outcome redirects the roadmap (opens a recognizer lever worth ~plan13-class
gains, or closes the whole recognizer direction for the hard class with
data), whereas #11's outcome only tunes one knob.

## Hypotheses

- **H1 (simplification)**: a material share of first-outcome eval mass sits
  in ≤5-men subtrees; a mid-search, budget-bounded preflight invocation
  (detector widened, no TT interaction, soundness contract per the preflight
  module header) could decide those regions by closure instead of DF-PN
  churn.
- **H2 (material-rich)**: ≥90% of eval mass stays in ≥9-men frames; the
  recognizer direction is dead for the outlier family and generic levers
  (#11/#12/#13) are the honest remainder.

## Method

### Step 1 — Temporary instrumentation (plan1 pattern)

Plain `u64` counter arrays on `Search`, incremented unconditionally, dumped
to stderr once at the end of `solve_with_progress`; reverted before the
report (plan1's design decisions 1–5 apply verbatim; no feature gates, no
CLI flags).

**Men-count buckets** — at `dfpn` entry in `src/search/dfpn/core.rs` (after
`tt_key` is computed), classify the frame's own position:

```rust
let men = pos.board().occupied().count();            // used at children.rs:298
let bucket = match men { 1..=3 => 0, 4..=5 => 1, 6..=8 => 2,
                         9..=12 => 3, 13..=16 => 4, _ => 5 };
```

Counters:

| Counter | Meaning |
|---|---|
| `frame_counts[6]` | `dfpn` entries per bucket (Σ must equal plan1's `frame_total`) |
| `frame_evals[6]` | descendant child evals per bucket (frame eval delta at exit, plan1's accounting) |
| `frame_cut_evals[6]` | threshold-cut slice of the above (the churn mass) |
| `frame_harvestable_counts` | frames matching the widened detector predicate (below) |
| `frame_harvestable_evals` | descendant child evals of those frames |

**Harvestable predicate** — the plan13 detector shape widened to ≤5 men:
`men <= 5 && pos.board().pieces_pt(PieceType::Pawn).is_empty()`
(preflight/mod.rs:156 pattern) `&& no castling rights` (same rights source
the plan13 detector uses). This is the number a mid-search recognizer spike
would act on.

Because men count is monotone non-increasing down a path, per-bucket
`frame_evals` sums define unambiguous cumulative shares ("evals spent at
material density ≤ k").

Exit-site map: reuse plan1's documented map (plan1 steps 2–3; report1 raw
tables are the cross-check). At each exit site, the frame's bucket local is
in scope — add the delta to the bucket's counters plus the category slice.

### Step 2 — Run matrix

```bash
cargo build --release

# m22_white (baseline: 14,156,269 child evals first-outcome)
target/release/atomic_solver \
  --fen "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22" \
  --timeout 30 --first-outcome --outcome-only

# stress case = m21_white (baseline: 249,480,478 child evals first-outcome)
target/release/atomic_solver \
  --fen "4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21" \
  --timeout 300 --first-outcome --outcome-only
```

Optional third case if session time allows: one 28-man decisive-suite
outlier (dec13 or dec14, FEN via the `tests/fixtures/` name→FEN lookup from
plan2 step 3) to check the pattern on rich middlegames.

Sanity invariants before interpreting: per case, Σ buckets = `frame_total`
and Σ `frame_evals` = plan1's frame-eval total (10.74% / 8.65% split
sanity); first-outcome `child_evals` match the baselines above
bit-for-bit.

### Step 3 — Drift check and revert

- `benchmark --suite quick --json --first-outcome` must be bit-identical
  per case versus a clean baseline (instrumentation must not perturb the
  trajectory — plan1's precedent).
- Revert all counters and the dump call; `git diff --exit-code src/` must
  pass before the report is written.

## Decision gates

| Gate | Criterion (stress case, eval-weighted) | Consequence |
|---|---|---|
| **GO** | ≥10% of `frame_evals` in buckets 0–1 (≤5 men), or `frame_harvestable_evals` ≥10% of total | Next plan: mid-search preflight invocation spike (widened detector at `dfpn` entry, eval-budget-bounded, no TT interaction, `ProofEvent`/soundness contract per the preflight module header). Hand-off target: `dfpn` reopen (trigger (a) — a measured search-semantics diagnostic for the class) or a new initiative. |
| **PARTIAL** | Dominant small-material mass sits in 6–8 men | No closure lever exists at that width (region size grows combinatorially — record a sizing estimate vs. plan13's 420k-position 3-men region); literature target #7 (mating-net recognizers) rises to the top of the backlog. |
| **NO-GO** | ≥90% of `frame_evals` in buckets 3–5 (≥9 men) and `frame_harvestable_evals` negligible | Close backlog #3 with data; recognizer direction dead for the outlier family. Next plan falls back to #11 (depth-scheduled ε POC), with the ε-invariance caveat documented in the plan. |

Report both `m22_white` and the stress case against the gates; the stress
case is the gate object (m22 is the regression control).

## Deliverables

- `docs/plans/research/measurements/plan3/` — raw stderr dumps per case,
  derived share tables (per-bucket and cumulative-≤k, counts and evals).
- `docs/plans/research/report3.md` — tables, hypothesis verdict, gate
  decision, hand-off record.

## Verification

- `cargo fmt --check`, `cargo clippy --release --all-targets` (clean revert).
- `make test` (fast gate).
- Quick-suite drift: bit-identical per case (see Step 3).
- `git diff --exit-code src/` after revert.

## Non-goals

- No recognizer or preflight widening is implemented; this plan measures
  only.
- No ε / TT / ordering / parallel work (fallback plans, each their own
  plan).
- No changes to the benchmark suites, fixtures, or CLI.
- No per-depth or per-rule50 histograms (a follow-up measurement only if a
  gate demands it).

## Final task

Write `docs/plans/research/report3.md` (raw tables under
`measurements/plan3/`, the gate decision, and the hand-off or closure
record), and update the backlog rows for #1 (done, plan1), #2 (done,
plan2), #3 (this plan's outcome), and #10 (done, plan1) in
`initiative.md`, plus a History line.
