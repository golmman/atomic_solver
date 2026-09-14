# Initiative: `egtb` — 4-man atomic endgame tablebase anchoring for faster solves

## Status

Active, opened 2026-09-14 (decision in session: goal = **faster solves**,
depth = **4 men**). Plans are single-lever, sized to one session, agile like
`dfpn`/`lean`/`conversion`. Next plan number: **plan1**.

Absorbs `lean` backlog #9 (2–3-man tablebases), which was scoped too
narrowly (leaf probes only, no generation story, 2-man layer is degenerate).

## Motivation

The solver's top output priority is decisive outcomes for deep positions
(~60 plies). Endgame tablebases accelerate exactly the subclass of searches
whose critical lines liquidate to few men: a probed child replaces the
recursive solve entirely, collapsing the subtree behind it. `lean` #9 rated
the effect "huge where covered, negligible elsewhere" — the *where covered*
part is unmeasured, so the initiative opens with a go/no-go spike (plan1)
that measures the men-count distribution at child-eval sites on the
decisive/stress suites before any integration work.

Why generate in-repo: there is no standard atomic EGTB format (nothing
Syzygy-equivalent exists for atomic chess), so there is nothing to import;
generation via retrograde analysis over `atomic-movegen` semantics is the
only path.

### Why 4 men (and why 3 men is not skippable)

- **2 men is degenerate.** KvK is a draw by rule (kings cannot be adjacent);
  K+1 commoner vs bare K wins for the strong side. No shipped 2-man table —
  the outcomes are base-case constants.
- **3 men is the first real layer and a hard retrograde dependency.** A
  capture in a 4-man position explodes every non-king man adjacent to the
  captured square (both colors), so 4-man positions resolve into 3- and
  2-man outcomes. Retrograde construction of 4-man tables requires the 3-man
  layer to exist first.
- 3-man also serves as the ruleset-semantics validation vehicle: it is small
  enough to cross-validate wholesale against the solver itself.

## Ruleset grounding (normative for the generator)

- **Stalemate is a Draw** (`src/position.rs`,
  `no_legal_moves_is_stalemate_draw`); no-legal-moves-in-check is Loss.
- **rule50 is terminal**: the 100-halfmove-clock draw, with the halfmove
  clock included in the Zobrist key (`src/zobrist.rs`).
- **Explosions**: a capture detonates the blast zone (all non-king men on
  the 8 neighbors of the captured square, both colors); men count can drop
  by more than one per capture.
- `atomic-movegen 2.2` (crates.io) is the sole semantics authority. The
  generator links it solver-side; no cross-repo ask initially (a fused
  upstream API becomes a `movegen` candidate only if the generator's
  per-position legality workload profiles as a bottleneck).

## Goal

Reduce child evals and wall time on decisive positions by probing a
generated ≤4-man tablebase at search leaves. Secondary: a ground-truth
oracle for ruleset semantics (generator-vs-solver cross-validation, proof
validator checks).

Non-goals: DTM-optimal move choice beyond what solving requires (WDL plus
minimal DTZ only); proof-tree relocation (the `proof` initiative owns that);
multi-TB disk-backed sets beyond 4 men.

## Soundness invariants

1. **rule50 discipline.** A probed Win is a proof only if the 50-move rule
   cannot intervene. Target scheme is Syzygy-style DTZ handling: at search
   time a table win with `dtz > 100 − halfmove_clock` is *Unknown* — never
   Draw, never Win. To be pinned in plan3.
2. **Repetition.** Tablebase WDL values ignore threefold repetition,
   consistent with the path-independent TT and the first-player-loss GHI
   shortcut; repetition handling stays in the solver's path logic.
3. **Semantics agreement.** Generator outcomes must match
   `atomic-movegen`/`Position` semantics. Sampled solver-vs-generator
   cross-validation is a merge gate for every generator plan.

## Backlog

| # | Item                                                                                                                         | Notes                                                                                                                                                                                              | Size | Status            |
| - | ---------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---- | ----------------- |
| 1 | Go/no-go spike: men-count histogram at child-eval sites + 3-man generator prototype + solver cross-validation                | Metric: share of child evals with ≤4 men on the decisive/stress suites. Histogram is near-free (one `popcount` + increment). Cross-validation settles stalemate/rule50/explosion conventions empirically. | M    | **open (plan1)** |
| 2 | 4-man generator + storage format + validator                                                                                 | Coverage decision (full set of ~40 material classes vs. frequent subset) and size verification land here. Estimate: ~30M entries/table, 2-bit WDL ≈ 8 MB raw/table, full set well under 100 MB compressed. | L    | open             |
| 3 | Probe integration in `evaluate_child` + rule50 soundness scheme + resource-contract wording                                  | Tablebase held fully in **RAM** (decision 2026-09-14: RAM load, no mmap). "RAM = TT only" needs rewording to admit the tablebase (e.g. "RAM = TT + read-only tablebase"); at 4 men the full set fits easily (~300 MB raw at 2-bit WDL, less compressed). Drift protocol: trajectories may change **only inside tablebase coverage**. | M–L  | open             |
| 4 | Ordering hints from tablebase wins (`move_order/ideas.md` #8)                                                                | Order moves that enter tablebase-won lines; only after #3.                                                                                                                                          | S–M  | open             |
| 5 | Proof-tree anchoring of probed leaves (`ProofEvent` semantics, reconstruction against a relocatable table)                   | Coordinate with `proof` (#7 deep-proof builder). The dump must mark oracle-proven leaves so reconstruction can expand them deterministically.                                                        | M    | open             |

## Open decisions

### 1. Coverage: full 4-man set vs. frequent subset

The 4-man material classes are one-vs-one (5 × 5 = 25 tables) and
two-vs-zero (15 tables, mirrored by side-to-move) — ~40 tables, ~30M
entries each.

**Option (a) — full set.** Every ≤4-men position has a well-defined probe
result; no "unknown because not built" state; a proof dump anchored against
"the 4-man tablebase" is reproducible by anyone holding the same set
(no coverage manifest needed). Cost: generation effort across classes the
decisive suite may never touch, and the largest footprint (~300 MB raw at
2-bit WDL — compatible with the RAM-load decision).

**Option (b) — subset from the plan1 histogram.** Only classes the suites
actually reach (likely Q/R/P-dominated). Smallest footprint and effort.
Costs: (i) a probe miss on an ungenerated class must be distinguishable
from a genuine unknown, for soundness accounting and for reproducible
reconstruction — the dump needs a coverage manifest, which is real format
complexity in backlog #2; (ii) coverage creep: each new suite position may
demand new classes, i.e. repeated follow-up plans; (iii) — the decisive
argument — **promotions jump between material classes at constant men
count** (K+P vs K+P promotes into K+Q vs K+P, still 4 men). Partial
coverage therefore holes out precisely on promotion moves, which are
exactly the forcing winning tries a solver wants to probe. Only full
coverage makes class transitions continuous.

**Tilt:** (a). The subset option only wins if plan1 shows the suite touches
few classes *and* the promotion-continuity problem is judged acceptable
(it is hard to defend). Decide on plan1's per-class histogram.

### 2. DTZ granularity for the rule50 invariant

Invariant 1: a probed Win/Loss is a proof only if the 50-move rule cannot
intervene (`dtz ≤ 100 − halfmove_clock`). Note the probe itself is
clock-independent (the table is built at clock 0); the halfmove clock
enters only the usability rule, even though the Zobrist key includes it.
Clock-robustness differs by value: a table **Draw** holds at any clock
(the defending side can hold forever, and a higher clock only brings its
draw claim closer), while a table **Win/Loss** decays to Unknown once
`dtz > 100 − clock`.

**Option (a) — exact DTZ (Syzygy-style), capped at 100 + sentinel.** Any
dtz > 100 is unprovable at every clock, so 7 bits + 2 WDL bits suffice;
at 4 men the storage delta over WDL-only is negligible (~8 → ~30 MB per
table). Captures the full set of provable wins, including clock-edge
leaves — relevant because our deep-search leaves (the conversion class:
shuffle lines with pawn pushes) carry high clocks precisely where wins
matter. Cost: DTZ generation is the notoriously fiddly part of EGTB
construction (distance-to-zeroing fixpoint, not plain mate distance);
it should be a separately gated layer, not plan2's first deliverable.

**Option (b) — WDL only, usable at clock 0 (i.e. directly after a
zeroing move).** Trivially sound and much simpler to generate. Cost: every
probe at clock > 0 must downgrade a table Win to Unknown (we cannot know
whether the mate fits the remaining clock), which can gut the "faster
solves" goal in exactly the deep-shuffle positions the solver cares about.

**Tilt:** sequence them — plan2 ships WDL with the clock-0 rule (sound,
simple), and exact DTZ becomes a follow-up layer *if* the plan1 clock
sub-metric (see plan1 Task A) shows a meaningful share of ≤4-men evals at
clock > 0 and the plan3 A/B shows that share is being discarded. This
keeps the soundness invariant identical across both stages; only the
usable-win count grows.

### 3. Proof-tree anchoring semantics for probed leaves (backlog #5)

**Option (a) — version-stamped opaque oracle leaves.** A probed child
becomes a `ProofNode` marked proven-by-tablebase, carrying a table
fingerprint/version. Validator accepts these leaves but counts them
separately from replay-verified nodes. Pros: small trees, no expansion
cost, search-side simple. Cons: those leaves are verified only modulo
generator correctness (mitigated by the cross-validation gate, but the
`proof` initiative's "independent, replayable proof" property weakens for
trees anchored on them); PPV extraction through such a leaf is fine, its
subtree is simply absent.

**Option (b) — no tablebase leaves in trees; expansion at reconstruction
time.** Reconstruction expands tablebase-anchored positions against its
own copy of the table. Pros: the dump stays a pure search product. Cons:
two sources of truth, reconstruction gains a table dependency and its own
probe path, and the "relocatable proof" story now requires shipping the
table anyway — strictly more moving parts for the same trust basis.

**Tilt:** (a), with the fingerprint in the dump format from the start
(backlog #2/#5 coordination with `proof`). Decide when #5 is scheduled,
not before.

## Decision records

- **2026-09-14 — loading strategy: full RAM load, no mmap.** At 4 men the
  whole set fits the search process budget with room to spare, which
  removes I/O failure modes and page-fault jitter from the hot probe path
  and keeps determinism trivial. mmap remains the fallback only if a
  future coverage extension (5+ men) outgrows RAM; revisit then, not
  before.
