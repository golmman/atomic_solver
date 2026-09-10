# Implementation Plan: Offline Proof Reconstruction Tool + Go/No-Go Experiment

Initiative: `docs/plans/proof/initiative.md`, backlog item #2. This plan is
self-contained; it builds on plan4's TT snapshot (`src/tt_snapshot/`) and runs
the decisive experiment that gates plan6 (flipping the worker-off default).
The only change to existing search code is a read-only-capable `tt_mut()`
accessor; no search behavior changes.

## Goal

1. A reconstruction library (`src/reconstruct/`) plus an example CLI
   (`examples/reconstruct_pt.rs`) that rebuilds a proof tree from the
   **root FEN + TT snapshot** artifact: top-down walk with static terminal
   classification, snapshot lookups, hole classification, and hole filling
   via prefix-aware local solves seeded with the snapshot. Output is the
   existing `binary.rs` proof-tree dump, produced by synthesizing
   `ProofEvent`s into the existing worker.
2. The **go/no-go experiment**: on positions small enough that the event-driven
   worker still succeeds, build the tree both ways (live events vs. snapshot
   reconstruction), require isomorphism, and measure hole rate by cause and
   fill cost in `child_evals`. Explicit kill criteria below decide whether
   plan6 proceeds.

Success criteria:

- `make test` (fast tier) green, including the dual-build oracle test.
- Drift check bit-identical: `benchmark --suite quick --json --first-outcome`
  produces identical `child_evals` per case (expected: no search-path change).
- Reconstruction is deterministic (budget-bounded fills, no wall clock).
- The experiment is executed and its numbers + go/no-go verdict are recorded
  in `report5.md`.

## Context (current state, verified 2026-09-10)

**Snapshot reader** (`src/tt_snapshot/reader.rs`):
`read_tt_snapshot(&mut R) -> io::Result<(SnapshotHeader, Vec<SolvedRecord>,
Vec<UnsolvedRecord>)>`; `SolvedRecord { key: u64, outcome: Outcome,
depth: u32, best_move: Move }`. Records are in native bucket order — treat
the snapshot as a **set keyed by `key`** (duplicate keys across generations
are possible; first-wins on map build, counted in stats).

**Search surface** (`src/search/dfpn/mod.rs`):

- `search_depth_with_prefix(&mut pos, max_depth, prefix_keys) ->
  (Outcome, u32, u64)` (outcome, win depth, nodes). `prefix_keys` are
  `Position::repetition_key()`s (board-only, no halfmove clock) of the
  positions *before* `pos`; they are restored into `prefix_path` so every
  bounded chunk sees the walk's repetition history (first-player-loss GHI
  context). It calls `begin_run()`, which resets `child_evals` — so
  per-attempt budgets work by calling `set_child_eval_budget(b)` before each
  attempt and reading `child_evaluations()` after.
- **Seeding gap**: `Search` owns its TT and exposes only `tt() -> &TT`.
  `TranspositionTable::store(...)` *is* public, and `TtEntry` fields are
  `pub(crate)` — readable anywhere inside the crate. So the seeding primitive
  reduces to one accessor: `Search::tt_mut() -> &mut TranspositionTable`.
- Solved-entry semantics that seeding must replicate exactly
  (`core.rs:313–317`): the live search stores solved results with
  `remaining_depth = u32::MAX`, and `resolved_from_entry` (core.rs:361–370)
  accepts a solved entry at any bound with
  `remaining_depth >= max_depth && depth <= max_depth`. Seeding via
  `store(key, best_move, u8::MAX, 0, Some(outcome), pn, dn, depth,
  u32::MAX)` (with `pn, dn = outcome.to_pn_dn()`) reproduces live entries
  byte-for-byte and stamps the current generation, so `probe` sees them.
- TT-hit re-emission: `core.rs:121` emits a `NodeProven` event when a node is
  resolved from the TT, so the live proof tree contains *every* node of the
  proven subtree, including subtree interiors resolved via TT hits.

**Proof-path/clock argument (why exact-key hits should dominate).** A
proof-tree node is a unique move path from the root; the walk follows exactly
those paths, and the halfmove clock is a function of the path (resets on pawn
moves/captures). The live search stored each record under
`pos.hash()` at its own path — the same path the walk arrives on. Therefore
exact-key lookups along proof paths should hit whenever the entry physically
survived. The anticipated hole classes are:

- **absent** — entry evicted (`insert_new` can evict solved entries) or never
  solved-stored; indistinguishable from the snapshot alone, reported as one
  class;
- **clock-miss** — expected ≈ 0 on proof paths (the argument above); the
  experiment verifies this. If it measures non-zero, the argument is wrong
  somewhere and must be understood before trusting any hole stats;
- **repetition (GHI)** — a path-repetition Draw is never cached as solved
  (`core.rs:291–307`), so it cannot be reconstructed from the snapshot. By
  the soundness argument below it should not appear as a child inside a
  valid proof; the class exists to verify that.

**Worker** (`src/proof_tree/worker.rs`): `ProofTreeWorkerHandle::spawn(
root_fen, pt_size_mb, memory_limited: Arc<AtomicBool>)`, `event_sender()`,
`finalize()`, `dump_to_bin()`, `stats()`. `finalize()` hard-fails if the root
was never realized or an unexpanded internal node remains — so the walker
must emit an event for **every** node it claims, root included (path empty,
`mv = Move::NONE`, exactly like the live search's root emission).

**Position** (`src/position.rs`): `outcome_from_state(&state, &moves)` gives
the static terminal classification in the exact priority order the solver
uses (own commoners empty → Loss; opponent's empty → Win; no legal move →
check-dependent Loss/Draw; rule50 ≥ 100 → Draw; `occupied == 2` → Draw);
`hash()` (full Zobrist incl. clock), `repetition_key()` (board-only),
`legal_moves_vec()`, `do_move`/`undo_move`. `zobrist::rule50_key(u16)` and
`zobrist::hash(board, rule50)` are public — the clock-scan below relies on
`hash = board_hash XOR rule50_key(rule50)`.

**ProofTree** (`src/proof_tree/node.rs`): `nodes: Vec<ProofNode>` and
`children(id)` are public; an isomorphism oracle can traverse a loaded tree
without touching `proof_tree`. Node creation order differs between the two
builds, so the oracle compares structure, not bytes.

## Design

### 1. Seeding API (the only existing-code change)

`src/search/dfpn/mod.rs`: add

```rust
/// Mutable TT access for tooling that pre-populates the table from a
/// snapshot (see `tt_snapshot` and `reconstruct`). The search must not
/// depend on seeded entries being present.
pub fn tt_mut(&mut self) -> &mut TranspositionTable
```

Seeding itself happens in `src/reconstruct/` via the existing public
`store(...)`; no `TranspositionTable` change. Unsolved records are **not**
seeded (they are generation-local bounds; report4 already noted the builder
should recompute ordering) — revisit only if the experiment shows fill
ordering is the bottleneck.

### 2. Reconstruction walk

Input: root FEN (from the snapshot header; `--fen` override must match it,
else error), the snapshot records, budgets, `--pt-size`.

State: one `Position` (do_move/undo_move), the move path, and a stack of
`repetition_key()`s from root to the current node (inclusive) — the walk's
prefix. Maps built once from the snapshot: `HashMap<u64, SolvedRecord>` for
exact lookups and a plain `HashSet<u64>` of solved keys for the clock-scan.

For each needed node `N` (root first), resolve its outcome:

1. **Static terminal**: populate one `StateInfo`, `outcome_from_state` →
   terminal outcome at depth 0. No TT dependency. (Mirrors the solver's
   terminal check, which stores `Move::NONE` records but classifies
   statically first.)
2. **Exact hit**: `map.get(&pos.hash())` → done.
3. **Clock-scan**: on exact miss, test `board ^ rule50_key(c)` for
   `c in 0..=100` against the solved-key set. If a match is found:
   - outcome **Win/Loss**: adopt it (solved results are position-truth,
     clock-independent) and classify as `clock_hit`;
   - outcome **Draw**: **do not adopt** — a solved Draw may be a rule50 draw,
     which is clock-dependent. Classify as `clock_miss_draw`, treat as a hole
     (the fill search re-derives it under the walk's true clock, where static
     rule50 classification is exact).
4. **Repetition-context**: `N.repetition_key()` is already on the walk prefix
   → the live search would score Draw (uncached GHI shortcut). Count as
   `repetition` and treat as an anomaly (see soundness note): by the argument
   below this cannot be a child of a node in a valid proof.
5. **Absent** → hole → fill (§3).

Soundness note (why the walk is well-formed): a Win node is proven via one
best move, so its proof subtree contains exactly one child; a Loss node
requires *all* legal replies to lose, so its subtree contains every child.
Every non-terminal child of a Win node must be Loss; every child of a Loss
node must be Win. A snapshot Draw (or repetition-context) child therefore
contradicts the parent's proof — counted as an anomaly, the walk aborts
(correctness first) with all stats collected so far. A Win node whose record
has `best_move == Move::NONE` while non-terminal is likewise an anomaly.

Walk rules per resolved node:

- **Win** (non-terminal): descend exactly `record.best_move` (validate it is
  legal; anomaly if not).
- **Loss**: enumerate `legal_moves_vec()`, resolve every child.
- **Filled Win**: descend via the best move harvested from the fill search's
  TT (§3–4).

Root: resolved like any node; must be decisive (Win or Loss) or
reconstruction fails ("snapshot does not prove a decisive root").

### 3. Hole filling

One persistent fill `Search::new(tt_mb)` created at start, seeded once from
the snapshot (§1). For each hole child `C` of parent `P`:

- prefix = the walk's repetition-key stack (root..=P);
- bound: iterative doubling starting at the parent's proven depth
  `d(P)` (guaranteed sufficient: a child's proven depth is `< d(P)` by the
  bottom-up Win=min+1 / Loss=max+1 semantics), or `--fill-base` (default 8)
  when no parent depth exists (root); ×2 up to `--fill-depth-cap`
  (default 32);
- per attempt: `set_child_eval_budget(--fill-attempt-budget, default
  10_000_000)` then `search_depth_with_prefix`; a `Draw` at the cap ⇒
  **unfillable** (recorded; the walk aborts there — go/no-go data);
- expected outcome known from the parent type; a decisive result that is not
  the expected complement (e.g. Loss where Win is required) is an anomaly;
- **no proof-event sender on the fill search**: fill-search events would
  carry fill-relative paths. Instead the *walker* emits one `NodeProven` for
  the filled node itself and then continues walking into it — the harvested
  records below make the filled subtree's interior resolve by exact hits or
  nested fills;
- **harvest after each fill**: scan `search.tt().entries()` for solved
  entries and insert them into the walk map (first-wins). This is in-crate,
  so `pub(crate)` `TtEntry` fields are readable. Harvesting converts the
  fill search's whole proven interior into walk hits and shrinks future
  holes.

A global fill budget `--fill-total-budget` (child_evals summed over all
fills, default 0 = unbounded) guards pathological inputs; exhaustion fails
the reconstruction with `BudgetExhausted`.

Fills are budget-bounded only — no wall clock — so reconstruction is
deterministic.

### 4. Event synthesis into the worker

The walker spawns `ProofTreeWorkerHandle::spawn(root_fen, pt_size,
memory_limited)` and sends `ProofEvent::NodeProven` for every resolved node
(including the root: empty path, `Move::NONE`) carrying
`(path, pos.hash(), outcome, depth)` — depth from the record for hits, 0 for
terminals, the fill's returned win depth for filled Wins (advisory: the
worker's `finalize()` recomputes non-terminal depths bottom-up; leaf depths
are authoritative). Reusing the worker keeps dummy-parent traversal,
canonical finalization, memory accounting, and the binary dump exactly as in
the live pipeline — the initiative's "builder consumes `ProofEvent`" note.

On success: `finalize()` then `dump_to_bin(--out)`. If the worker's
`memory_limited` flag trips during reconstruction, abort and report (a data
point for backlog item #7). On a failed walk, do **not** finalize; report
stats only.

### 5. Isomorphism oracle

In `src/reconstruct/`:

```rust
pub fn tree_signature(tree: &ProofTree)
    -> HashMap<Vec<Move>, (Option<Outcome>, u32)>
```

DFS from the root over the public `nodes`/`children` accumulating
path → (outcome, depth). Two builds are isomorphic iff their signatures are
equal. `--oracle <dump>` loads the reference via `ProofTree::from_bin` and
reports `oracle: isomorphic | differing (a only-in-events, b only-in-recon)`.

### 6. Example CLI `examples/reconstruct_pt.rs`

Options: `--snapshot <file>` (required), `--fen` (default: header FEN),
`--out <dump>` (default `reconstruct_pt.bin`), `--tt-size <MB>`,
`--pt-size <MB>`, `--fill-base`, `--fill-depth-cap`, `--fill-attempt-budget`,
`--fill-total-budget`, `--oracle <dump>`, `--json` (machine-readable stats),
`--experiment` (suite mode, below). Default output prints outcome, node
count, and the hole-rate breakdown:
`holes: hit=<n> terminal=<n> clock_hit=<n> clock_miss_draw=<n> repetition=<n> absent=<n> filled=<n> unfillable=<n> anomalies=<n>`.

`--experiment` (the go/no-go run): iterate the decisive suite via
`examples/common.rs`. Per case:

1. Live build: `Search::new` + worker events on → solve → record original
   `child_evals` C, `finalize()`, tree T₁ (dump).
2. Snapshot: `write_tt_snapshot(search.tt(), fen, tt_mb, …)`.
3. Reconstruct from FEN + snapshot → T₂, hole stats, fill child_evals F.
4. Oracle: compare signatures of T₁ and T₂.

Output: per-case JSON rows plus a summary (hole-rate breakdown aggregated by
cause, median/max fill ratio F/C, oracle results, verdict against the kill
criteria).

## Kill criteria (explicit, per working agreement #4)

Let F = total fill `child_evals` of a reconstruction, C = the original
search's `child_evals` for the same case. Thresholds are proposed here and
may be re-justified (not silently changed) in `report5.md`.

- **No-go (coverage)**: any suite case where the live worker succeeded but
  reconstruction fails (unfillable hole, anomaly, non-isomorphic oracle,
  memory limit) under the default budgets. Correctness first — one failure
  refutes "a bounded artifact suffices" until the cause is understood.
- **No-go (amplification)**: median F/C > 50% across the suite. If filling
  holes costs half of re-searching, the snapshot is too lossy to earn its
  decoupling; the fallback is backlog item #4 (pin solved entries) as its own
  measured lever.
- **Watch (informative, not gating)**: max single-hole fill cost > 10% of C,
  and the hole-rate breakdown — `absent` dominance motivates item #4;
  `clock_miss` non-zero invalidates the proof-path/clock argument and blocks
  trusting the rest of the stats until explained.
- **GO** (plan6 may proceed): zero coverage failures, oracle isomorphic on
  all cases, median F/C ≤ 10% (the snapshot carries ≥ 10× leverage).

## Scope

1. `src/search/dfpn/mod.rs`: add `tt_mut()` only.
2. **New `src/reconstruct/` module** (re-exported from `lib.rs`):
   - `mod.rs` — public types (`HoleClass`, `ReconstructStats`, `ReconstructConfig`),
     seeding, the clock-scan/adoption helper, `tree_signature`.
   - `walker.rs` — the walk, fill loop, event synthesis.
   - `tests.rs`.
   Keep files under the ~10 KB guideline; split further (e.g. `stats.rs`)
   rather than growing past it.
3. **`examples/reconstruct_pt.rs`** — CLI + experiment mode.
4. **`tests/reconstruct.rs`** — fast integration tests (below).
5. **Docs**: `AGENTS.md` — add `reconstruct` to the `lib.rs` re-export list
   and `reconstruct_pt` to the examples list (two sentences each).

## Explicitly out of scope

- Seeding unsolved records (ordering hints) — measure first.
- Any search-behavior change beyond the `tt_mut` accessor; plan6's default
  flip; item #4 pinning; item #5 checkpointing; item #7 builder capacity.
- Changes to either binary format (`binary.rs` dump, TT snapshot v1).
- Disk-backed finalize or any worker change (the builder consumes the worker
  as-is).
- PostgreSQL importer concerns.

## Tests

Unit tests (`src/reconstruct/tests.rs`, fast tier):

- Seeding: `tt_mut().store(...)` with the §1 field mapping is probe-visible
  in the current generation, and `resolved_from_entry` would accept it at a
  bound ≥ its depth (via a small max_depth search resolving from the seeded
  entry on a mate-in-1).
- Clock-scan: build a position `P`, insert a solved record under the
  synthetic key `board_hash ^ rule50_key(c')` for `c' ≠ clock(P)`; assert a
  Win record is adopted (`clock_hit`) and a Draw record is not.
- Fill: from the mate-in-3 fixture's snapshot, drop the root record from the
  map; assert the walk fills the root via search and counts `filled = 1`.
- Anomalies: craft a map where a non-terminal Win node's record has
  `best_move = Move::NONE`; assert the walk aborts with the anomaly counted.
- `tree_signature`: equal trees → equal signatures; differing outcome, depth,
  and child set each detected.
- Duplicate snapshot keys: first-wins, duplicates counted.

Integration tests (`tests/reconstruct.rs`, fast tier):

- **Dual-build oracle** on `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1`: in-process
  (a) solve with worker events, finalize, keep T₁; (b) `write_tt_snapshot`;
  (c) reconstruct → T₂; assert root outcome Win, `hit > 0`, signatures equal.
- Same FEN with the root record dropped (fill path) still reconstructs
  isomorphically.
- One slow-tier test (`#[ignore = "slow: ..."]`): a deeper decisive fixture
  reconstructed end-to-end within the live worker's capacity.

## Verification

1. `cargo fmt --check`, `cargo clippy --all-targets`, `cargo doc` — clean.
2. `make test` — fast tier green including the new tests.
3. Drift check: `benchmark --suite quick --json --first-outcome` before vs.
   after — `child_evals` identical per case (expected: the only search-path
   surface is an unused accessor).
4. Manual CLI run (release): mate-in-3 with `--tt-dump-path`, then
   `reconstruct_pt --snapshot … --oracle <event dump>` — assert
   `oracle: isomorphic` and inspect the hole breakdown.
5. **Run the experiment** (`--experiment`) on the decisive suite in release;
   record per-case JSON, the aggregated breakdown, F/C distribution, and the
   go/no-go verdict in `report5.md`.

## Risks / notes

- **Fill-bound semantics**: the `d(P)` start rests on the bottom-up depth
  arithmetic; the doubling cap covers off-by-one surprises, and the oracle
  catches any resulting non-isomorphism.
- **Clock-scan cost**: 101 set lookups per miss, only on misses — negligible.
- **Worker finalize strictness**: finalize hard-fails on incomplete trees;
  the walker finalizes only after a complete walk, so a failed reconstruction
  never reaches finalize. The worker thread must still be joined cleanly on
  the abort path.
- **Expectation management**: `clock_miss` and `repetition` are expected ≈ 0
  on proof paths (the argument in Context). Non-zero measurements are a
  finding to explain, not a number to tune away.
- **Snapshot as a set**: duplicate keys across generations are possible;
  first-wins is deterministic given the native bucket order.
- **Example-side suite access**: `--experiment` reuses `examples/common.rs`
  loaders; suite cases that the live worker cannot handle under default
  `--pt-size` are skipped and reported as skipped (they are item #7's
  territory, not oracle cases).

## Final task

Write `docs/plans/proof/report5.md` in this directory: summary of changes,
files changed, verification results (including the drift check and the full
experiment table), the hole-rate breakdown by cause, the F/C distribution,
the go/no-go verdict against the kill criteria above, problems encountered,
deviations, missing tests, and next steps (plan6 flip if GO; item #4 pinning
if the absent class dominates). Update the initiative backlog (#2 → done) and
History in the same pass.
