# Lean Plan 8 — #8 cheaper static scoring (bit-identical)

Implements the single backlog lever **#8 (cheaper static scoring)**, named
as the next lever by `report7.md` ("Next lean lever falls back to #8,
cheaper static scoring, 5.5% of the post-plan6 pie") and ranked open in
the initiative backlog. **One lever, no bundle**: #10 (history/killer
re-tune) is deliberately *not* included — it changes ordering decisions
(behavior change, move-order-suite validation, oracle-floor regression
risk) and gets its own plan if this one leaves budget.

Hard constraint: every score `StaticAtomicScorer` produces must be
**bit-identical** before vs after. The lever is a compute-cost change
only — same integer results, same ordering decisions, same search
trajectory — so the bit-identical drift protocol applies in full.

Scope constraint: solver-side only. `atomic-movegen` is a crates.io
dependency (2.2, not vendored; plan6 precedent); any optimization that
needs a new upstream API (e.g. a fused attackers/attacks query) spins
out to the `movegen` initiative instead of blocking this plan.

## Rationale (from the post-plan6 profile, 2026-09-12)

`score_with_context` is **5.5% of the profile pie** (`sort_moves` runs
once per searched node at `history.rs:79-122`; per quiet move it does
2–5 sliding-attack scans: `attacks_from` in the threat block, up to two
`rook_attacks` in the rook block, plus `attackers_to` when a threat
fires). Backlog potential: ~3–5% wall. Additional cost inside the same
cluster: `nearest_commoner_map` is O(64 × k) Chebyshev pairs per node
(k = enemy commoner count) with no early structure exploitation.
Realistic target for this plan: **−2–4% wall** on the validation cases,
bit-identical trajectories. If phase 0 measures the addressable total
below ~2%, #8 is demoted without implementation (the #17 precedent).

## Phase 0 — attribution spike (measurement only, reverted)

The 5.5% share is a cluster, not a line item; the sub-costs must be
sized before any code changes (working agreement §1). Temporary
instrumentation, reverted after measuring (lean spike pattern; #12/#14/
#17/plan7 are the precedents):

1. Temporary counters (plain `u64` fields on `Search` or function-local
   statics — quickest to revert), incremented in `history.rs::sort_moves`
   and `ordering.rs`:
   - `sort_moves` invocations and total moves scored (calibrates
     per-move cost estimates);
   - threat block: `attacks_from` calls split by piece class (slider vs
     occupancy-independent pawn/knight/commoner) and `attackers_to`
     calls (the downgrade path);
   - rook block entries (requires `lone_commoner`), split by whether the
     first `rook_attacks` hit or the second semi-open-file scan ran;
   - `capture_net_value` calls and blast-zone popcount total;
   - `nearest_commoner_map` calls, enemy-commoner counts, and a rough
     element-op total (64 × k).
2. Measure both validation cases (per Measurement conventions):
   - `m22_white` first-outcome, `--timeout 30 --outcome-only`
     (dominant case);
   - the shuffle-win FEN
     `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`
     first-outcome, `--timeout 100`.
3. Cross-check with `perf record -e cpu-clock -g --no-children` leaf
   attribution on the same runs (inlined fragments land inside
   `score_with_context` / `sort_moves` leaves; the counters give the
   per-fragment call rates to convert leaves into per-block walls).
4. **Revert the instrumentation completely.** Decision rule: rank the
   candidate sub-levers by projected wall saving; implement only those
   ≥ ~1% wall each; if the addressable sum < ~2% wall, demote #8 to
   "spiked, below bar" with numbers and stop (write the report with the
   spike verdict; phases 1–2 collapse into design notes for a future
   re-rank).

## Phase 1 — `nearest_commoner_map` multi-source BFS

Replace the O(64 × k) Chebyshev-pair scan (`ordering.rs:169-185`) with a
multi-source BFS over king-move adjacency from all enemy commoner
squares simultaneously: O(64) with a small queue, min-Chebyshev distance
is exactly the BFS layer index. Output must be identical for every
input, including the all-`i8::MAX` map when the enemy has no commoners.

1. Implement in `ordering.rs` (same signature, same `pub` visibility —
   `history.rs` and the unit tests import it).
2. Add a fast-tier property test (new file; check `tests/` for naming
   collisions first — the `test_plan<N>.rs` pattern is shared across
   initiatives): random boards (seeded, deterministic) comparing the
   BFS map against the existing brute-force implementation kept as a
   `#[cfg(test)]`-only reference, across 0–8 commoners, plus the
   empty-commoners case.
3. The exact-score pins in `src/search/ordering/tests.rs` must pass
   unchanged — they are the primary bit-identical witness for the
   scorer.

## Phase 2 — `score_with_context` fast paths (conditional on phase 0)

Candidates, each gated on its phase-0 projection; implement strictly
inside the existing integer semantics:

1. **Blast-mask hoist**: blocks 1 and 5 both compute `king_attacks(to)`
   (block 1 as `king_attacks(to) | square_bb(to)`, block 5 as
   `king_attacks(to)`); compute once per move.
2. **Occupancy-independent threat check**: for pawn/knight/commoner
   movers the threat test (block 4) does not depend on `new_occupied`
   — use the static attack tables and skip the occupancy rewrite and
   `attacks_from` dispatch. Sliders keep the current path (any ray
   pre-filter must be proven exact, not heuristic).
3. **Empty-enemy fast path**: when `ctx.them_commoners` is empty, every
   threat/kamikaze/approach/pawn-storm/rook term is provably zero (the
   guards already imply it); only the centrality term can fire — skip
   the remaining block machinery.
4. **Bitboard `capture_net_value`**: replace the per-square `piece_on`
   loop over the blast zone with intersections against the piece
   bitboards, accumulating the identical integer sum (same `params`
   values, same addition order not required — only the final `i32` must
   match, which it does by commutativity of the sums).

Each sub-lever keeps a one-line comment pointing at the invariant it
relies on ("identical scores, verified by the exact-score pins and the
drift gate"). If `ordering.rs` approaches the 20 KB guideline, split the
new helpers into `ordering/context.rs` rather than growing the header
justification.

## Companion tasks (same session, not levers)

1. **initiative.md**: update backlog row #8 (done with numbers, or
   demoted per the phase-0 verdict) and add the plan8 history bullet.
2. **Post-plan8 profile** (working agreement §4): re-run the m22
   first-outcome profile command from AGENTS.md, refresh the profile
   section shares, and re-rank the backlog. If static scoring drops
   below ~2%, the next-wall-lever candidates are effectively exhausted
   and the successor is #10 or the `dfpn` algorithmic items — note that
   in the report, don't plan it here.
3. No AGENTS.md change is expected (no user-visible surface moves).

## Validation (drift protocol is the gate)

1. **Baselines before any change**: `benchmark --suite quick --json
   --first-outcome --runs 1` (per-case `child_evals`), m22 first-outcome
   stdout (`--timeout 30 --outcome-only`), and the committed
   `tests/fixtures/m22_default_stdout_golden.txt` as the default-mode
   reference.
2. **After each phase** (bisect by reverting a phase if anything moves):
   - quick suite `child_evals` **bit-identical per case**;
   - m22 first-outcome stdout **byte-identical**;
   - slow-tier lean3 golden byte-identical
     (`cargo test --release --test test_lean3 -- --include-ignored`);
   - `cargo test --release --test test_move_order` green (fast tier) —
     ordering output unchanged;
   - budget mode untouched: `cargo test --release --test
     test_decisive_remaining -- --include-ignored` passes unchanged.
3. **Wall measurement**: m22 first-outcome `--timeout 30`, 5 runs,
   median, before vs after; one shuffle-win first-outcome run each side.
   Expect −2–4% combined (or the phase-0 demotion instead); any
   regression beyond noise on either case is a stop-and-bisect.
4. `make test` green within the ~60 s gate.
5. `git diff` scope check: `src/search/ordering.rs`
   (+ `ordering/tests.rs`, optional `ordering/context.rs`),
   `src/search/dfpn/history.rs`, `tests/`, `docs/plans/lean/` docs.
   Phase 0 instrumentation must not appear in the final diff.

## Deliverable

`report8.md` in this directory, including: the phase-0 attribution
table and verdict (per-block walls on both validation cases), the
sub-levers implemented vs skipped with their measured deltas, the
BFS-equivalence test evidence, drift evidence (quick-suite per-case +
m22 stdout + golden + move-order suite), wall deltas on both validation
cases, the post-plan8 profile table, problems encountered, missing
tests, and next steps (#10 re-tune and/or the `dfpn` algorithmic items,
per the re-ranked backlog).

Per repo convention, this plan ends with the task of writing its
`report8.md` in this directory.
