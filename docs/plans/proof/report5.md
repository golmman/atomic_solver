# Implementation Report: Offline Proof Reconstruction Tool + Go/No-Go Experiment (plan5)

Implements `docs/plans/proof/plan5.md` (backlog item #2). The only change to
existing search code is the read-only-capable `Search::tt_mut()` accessor; no
search behavior changed (drift check bit-identical).

## Summary of changes

- **`Search::tt_mut()`** (`src/search/dfpn/mod.rs`) — the single existing-code
  change per the plan: mutable TT access for snapshot seeding, documented as
  tooling-only (the search must not depend on seeded entries).
- **New `src/reconstruct/` module** (re-exported from `lib.rs`):
  - `mod.rs` (13 KB) — format docs (resolution order, soundness argument),
    `HoleClass` / `ReconstructStats` / `ReconstructConfig` / `ReconstructOutput`,
    `build_solved_map` (first-wins, duplicates counted), `seed_search`
    (replicates the live solved-entry store byte-for-byte:
    `store(key, best_move, u8::MAX, 0, Some(outcome), pn, dn, depth, u32::MAX)`
    with `pn/dn = outcome.to_pn_dn()`), `clock_scan` (board key XOR
    `rule50_key(c)`, `c in 0..=100`), `classify_node` (the 5-step resolution
    order), `tree_signature`, and the `reconstruct` entry point.
  - `walker.rs` (15 KB) — the walk: static terminal classification, exact
    hit, clock-scan adoption (Win/Loss only), repetition-context anomaly,
    absent → fill; iterative-doubling fills via `search_depth_with_prefix`
    with the walk's repetition-key prefix, harvest of solved fill-TT entries
    into the walk map after each fill; one `NodeProven` synthesized per
    resolved node (root included, empty path) into the regular
    `ProofTreeWorkerHandle`; finalize + tree extraction only after a complete
    walk; the worker thread is joined cleanly on every path.
  - `tests.rs` (11 KB) — 10 unit tests (seeding probe-visibility + mate-in-1
    resolution, clock-scan adoption/rejection, fill of a dropped root record,
    Win-without-best-move anomaly, non-decisive root, signature equality and
    three difference classes, duplicate first-wins, holes-line format,
    end-to-end structure sanity).
- **`examples/reconstruct_pt.rs`** (21 KB) — CLI: `--snapshot` (required),
  `--fen` (must match the header FEN), `--out` (default `reconstruct_pt.bin`),
  `--tt-size` (default: header value), `--pt-size`, `--fill-base`,
  `--fill-depth-cap`, `--fill-attempt-budget`, `--fill-total-budget`,
  `--oracle <dump>`, `--json`, plus `--timeout` (experiment live solves;
  plan deviation, see below) and `--experiment` (dual-build go/no-go run over
  the decisive suite via `examples/common.rs`, per-case JSON rows, aggregated
  hole breakdown, median/max fill ratio F/C, and the verdict against the kill
  criteria).
- **`tests/reconstruct.rs`** — fast integration tests: dual-build oracle on
  `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1` (root outcome Win, `hit > 0`, signatures
  equal) and the same FEN with the root record dropped (fill path, `filled == 1`,
  isomorphic); one `#[ignore]`d slow-tier test on dec02 (30-ply decisive
  fixture) reconstructed end-to-end within the live worker's capacity.
- **Docs.** `AGENTS.md`: `reconstruct` added to the `lib.rs` re-export list;
  `reconstruct_pt` added to the examples list.

## Deviations from the plan

- **`tree_signature` is keyed by `Vec<u16>` (move-bit codes), not
  `Vec<Move>`.** `atomic_movegen::types::Move` implements neither `Hash` nor
  `Ord` and the orphan rule forbids implementing `Hash` for it, so a
  `HashMap<Vec<Move>, …>` cannot be built. The encoding is the dump format's
  own `move_to_bits`, so the key type is a faithful path representation.
- **`--timeout` added to the example.** The experiment's live builds need a
  solve timeout; the plan's option list did not include one (its suite mode
  implies the default 5 s, which is the default here too).
- **The CLI writes the dump via `ProofTree::to_bin` on the returned tree
  instead of the worker's `dump_to_bin`.** The reconstruct library returns the
  finalized tree; both paths serialize the identical finalized tree through
  the same `binary.rs` writer (verified: `cmp` of the CLI dump vs. the live
  event dump on the mate fixture is byte-identical).
- **Filled-Loss advisory depth is the successful fill bound** (not 0): the
  plan fixes only the filled-Win depth ("the fill's returned win depth") and
  leaves filled-Loss open; the successful bound is the better advisory and is
  recomputed bottom-up by `finalize()` anyway.
- **Fill searches get a ~100-year wall-clock timeout constant.** The plan says
  fills are budget-bounded only; without an override the fill `Search` would
  inherit the 5 s default timeout and a slow fill could be wall-clock-cut into
  a false `Draw` (non-deterministic). The child-eval budgets remain the only
  effective bound.
- **Slow-tier fixture is dec02, not "the mate-in-3 fixture" for the fill unit
  test and dec02 for the slow integration test.** The plan's integration-test
  bullet named m19 implicitly via `M19_FEN` in my first attempt; m19 does not
  solve within 60 s (it is not in the decisive suite). dec02 (30-ply Loss,
  solves in ~0.2 s) is a genuine deeper decisive fixture from
  `tests/fixtures/decisive_positions.txt`. The unit-test fill fixture is the
  two-rook mate (the plan's dual-build FEN), reused in-process.
- **Files slightly above 10 KiB** (`mod.rs` 13 KB, `walker.rs` 15 KB,
  `tests.rs` 11 KB, example 21 KB) carry header justifications per the repo
  convention; the walk/fill/event synthesis is one state machine whose
  invariants should not be scattered.

## Files changed

| File | Change |
|---|---|
| `src/search/dfpn/mod.rs` | add `Search::tt_mut()` (only existing-code change) |
| `src/reconstruct/mod.rs` | new — types, seeding, clock-scan, classification, signature |
| `src/reconstruct/walker.rs` | new — walk, fill loop, event synthesis |
| `src/reconstruct/tests.rs` | new — 10 unit tests |
| `src/lib.rs` | add `pub mod reconstruct;` |
| `examples/reconstruct_pt.rs` | new — CLI + experiment mode |
| `tests/reconstruct.rs` | new — 2 fast + 1 slow integration tests |
| `AGENTS.md` | re-export list + examples list |
| `docs/plans/proof/initiative.md` | backlog #2 done, History updated |

## Verification results

1. `cargo fmt --check`, `cargo clippy --all-targets`, `cargo doc` — all clean
   (zero warnings).
2. `make test` (fast tier, release) — green, including the 10 new unit tests
   and 2 new fast integration tests.
3. **Drift check** — `benchmark --suite quick --json --first-outcome --runs 1`
   before vs. after: 59 cases, `status`/`outcome`/`nodes`/`child_evals`/
   `pv_len`/`timeout`/`wrong` byte-identical per case; only wall-clock `time_*`
   fields differ. Confirms zero search-path change.
4. Manual CLI run (release): solve the mate fixture with `--tt-dump-path` +
   `--dump-path`, then `reconstruct_pt --snapshot … --oracle <event dump>` →
   `outcome: win`, `nodes: 4`, `holes: hit=3 terminal=1 … anomalies=0`,
   `oracle: isomorphic`, and the two dumps are byte-identical (`cmp` clean).

## Experiment results (go/no-go, decisive suite, release build)

Config: default budgets (tt 128 MB, pt 256 MB, timeout 5 s, fill-base 8,
fill-depth-cap 32, fill-attempt-budget 10M, fill-total-budget 0).

- 46/46 cases: live worker succeeded, none skipped, zero unfillable holes,
  zero anomalies, zero `clock_miss_draw`, zero `repetition`.
- **Oracle: 43/46 isomorphic.** 3 cases differ — all in the same direction:
  the reconstruction is a **strict superset** of the live event-built tree
  (`a only-in-events: 0` in every case; see root-cause analysis below).
- **Hole rate**: 91,470 proof nodes needed in total;
  `hit = 69,598` (76.1%), `terminal = 21,869` (23.9%), `clock_hit = 1`,
  `absent = 2`, `clock_miss_draw = 0`, `repetition = 0`. Hole rate by cause:
  3/91,470 ≈ **0.003%** (1 clock-scan adoption + 2 fills).
- **Fill cost**: F total = **2 child evals** against C total = 65,138,783
  (the two fills resolved instantly from snapshot-seeded TT entries);
  median F/C = **0.0**, max F/C = 2.1e-7 (dec13). The proof-path/clock
  argument holds: exact-key hits dominate (the single `clock_hit` was adopted
  correctly, and the `repetition` class never fired).

Per-case table (C = original `child_evals`, F = fill `child_evals`, nodes =
reconstructed proof nodes):

| case | status | outcome | C | F | nodes | case | status | outcome | C | F | nodes |
|---|---|---|---:|---:|---:|---|---|---|---:|---:|---:|
| dec01 | ok | win | 7,142,132 | 1 | 6,292 | dec24 | ok | loss | 532,754 | 0 | 279 |
| dec02 | ok | loss | 181,074 | 0 | 2,257 | dec25 | ok | win | 1,030,761 | 0 | 126 |
| dec03 | ok | win | 1,047,074 | 0 | 96 | dec26 | ok | win | 1,674,502 | 0 | 448 |
| dec04 | ok | win | 1,507,415 | 0 | 148 | dec27 | ok | win | 135,399 | 0 | 88 |
| dec05 | ok | win | 1,314,618 | 0 | 612 | dec28 | ok | loss | 1,011,672 | 0 | 35 |
| dec06 | ok | loss | 2,515,879 | 0 | 1,663 | dec29 | ok | win | 130,738 | 0 | 70 |
| dec07 | ok | win | 1,504,254 | 0 | 30 | dec30 | ok | win | 1,012,056 | 0 | 220 |
| dec08 | ok | win | 127 | 0 | 4 | dec31 | ok | loss | 2,507,683 | 0 | 271 |
| dec09 | ok | win | 1,910 | 0 | 6 | dec32 | ok | win | 62,114 | 0 | 66 |
| dec10 | differing | win | 4,801,924 | 0 | 50,544 | dec33 | ok | win | 249,955 | 0 | 86 |
| dec11 | ok | win | 1,253,894 | 0 | 684 | dec34 | ok | win | 1,026,860 | 0 | 100 |
| dec12 | ok | win | 1,333,227 | 0 | 834 | dec35 | ok | win | 51,204 | 0 | 12 |
| dec13 | ok | win | 4,822,604 | 1 | 1,520 | dec36 | ok | loss | 1,020,206 | 0 | 99 |
| dec14 | ok | win | 3,397,492 | 0 | 2,016 | dec37 | ok | loss | 1,048,883 | 0 | 255 |
| dec15 | ok | win | 2,173,010 | 0 | 1,744 | dec38 | ok | loss | 1,211,008 | 0 | 2,721 |
| dec16 | ok | win | 1,323,246 | 0 | 230 | dec39 | ok | win | 1,865,773 | 0 | 242 |
| dec17 | ok | win | 1,539,277 | 0 | 3,304 | dec40 | ok | loss | 2,579,394 | 0 | 3,271 |
| dec18 | ok | win | 232,305 | 0 | 56 | dec41 | ok | loss | 1,027,476 | 0 | 375 |
| dec19 | ok | win | 1,135,708 | 0 | 148 | dec42 | ok | loss | 1,080,920 | 0 | 283 |
| dec20 | ok | win | 1,657,748 | 0 | 476 | dec43 | ok | loss | 2,506,953 | 0 | 781 |
| dec21 | ok | win | 1,243,846 | 0 | 1,594 | dec44 | differing | loss | 103,082 | 0 | 4,305 |
| dec22 | ok | win | 1,004,894 | 0 | 26 | dec45 | ok | win | 1,003,422 | 0 | 12 |
| dec23 | ok | win | 1,003,471 | 0 | 24 | dec46 | differing | loss | 128,839 | 0 | 3,017 |

### Root-cause analysis of the 3 oracle mismatches (dec10, dec44, dec46)

The oracle failures are **live-tree incompleteness, not reconstruction
failure** — the reconstruction rebuilt the *complete* proof where the
event-built tree is missing required children. Evidence and mechanism,
established on dec46 (16 extra recon paths) and confirmed in direction
(a-only = 0) on dec10 and dec44:

1. In the recon tree, Loss node P = `root.c1d2.g2f1n.c5c6.f2f1q` has all 6
   legal replies as Win children; the live tree has only 5 (missing
   `d2e3`, a required defender reply — a Loss node's proof must cover *all*
   replies, so the live tree at P is not a valid proof subtree).
2. Instrumented search trace: the `dfpn` frame at P's own path never solved P
   (child `d2e3` stayed unsolved there, cut by work chunks); P's solved
   `Loss` record was stored by a *transposition* frame at another path Q
   reaching the same position (same hash incl. clock), whose complete
   expansion emitted its children under **Q's** path.
3. At P's own path the search later resolved P from the TT and re-emitted the
   node event (`core.rs` TT-hit re-emission) *without* its children; the
   worker's `reconcile_children` then kept the 5 children that happened to
   have been emitted under P's path by `evaluate_child` calls.
4. `finalize()`'s canonicalization selects one expanded node per
   `(hash, outcome)` and copies it onto transpositions, but its
   "expanded = has children" notion cannot distinguish a *completely*
   expanded twin (Q, 6 children) from an *incompletely* expanded one
   (P, 5 children); the tie-break (both consistent, equal depth, first
   created wins) picked the incomplete P. The dump therefore preserved the
   incomplete subtree.

So the snapshot carries strictly more proof information than the event-built
tree in exactly the situation the event tree was known to be lossy
(TT-hit re-emission is childless; canonicalization is heuristic). The
reconstruction's Loss-completeness comes for free from the walk: a Loss node
expands *all* legal replies by construction.

### Verdict against the kill criteria

- **No-go (coverage)**: technically fires — 3 suite cases where the live
  worker succeeded but the oracle reported non-isomorphism. **Re-justified
  (not silently waived)**: the criterion was written to catch reconstruction
  failures ("a bounded artifact does not suffice"); the measured failures are
  the reverse — the bounded artifact sufficed and the *reference* build was
  incomplete. A strict superset with `a only-in-events = 0`, snapshot-backed
  extra children, and a fully identified live-side mechanism is not evidence
  against the snapshot design; it is a pre-existing proof-tree-layer defect
  that plan5's oracle surfaced (valuable on its own).
- **No-go (amplification)**: median F/C = 0.0 ≤ 50% — passes by a huge margin
  (the snapshot carries ≫ 10× leverage: 2 fill evals vs. 65M original).
- **Watch**: max single-case F/C = 2.1e-7 ≪ 10% of C. The breakdown is
  dominated by `hit` (76%) + `terminal` (24%); `absent` = 2, so item #4
  (pinning solved entries) is not motivated at this scale; `clock_miss_draw`
  = 0 and `repetition` = 0 — the proof-path/clock argument is confirmed.
- **GO criteria** (zero coverage failures, all isomorphic, median ≤ 10%):
  satisfied *modulo the reference-side incompleteness* — with the
  re-justification above, the verdict is **GO for plan6**, conditional on a
  follow-up proof-tree fix (below) landing before plan6's flip, since plan6
  makes the reconstructed tree the *only* tree.

## Problems encountered

- **`Move` is not `Hash`** → signature keys are `Vec<u16>` move-bit codes
  (deviation above).
- **Trees loaded from a dump carry `hash: 0`** on every node; the signature
  DFS's cycle guard (path-hash set) fired at depth 1 for loaded trees,
  silently truncating signatures to 2 paths. Fixed by applying the guard only
  to real (non-zero) hashes; loaded trees are acyclic by construction
  (`parent_id < child id`).
- **Walker clock-hit bug (found by the experiment, fixed)**: for a
  `clock_hit` resolution the walker re-looked-up `best_move` by exact hash
  (which misses by definition) instead of using the scanned record's move →
  false "best_move NONE" anomaly on dec13. The scanned record's best move
  belongs to the same board (castling/ep are inside the board hash), so it is
  directly usable.
- **The m19 fixture does not solve in 60 s** (my initial slow-tier choice);
  replaced with dec02 (see deviations). Note for future plans: `M19_FEN` is a
  move-order fixture, not a decisive one.
- **`--outcome-only` skips the pre-exit hook**, so the manual verification
  must run the solver *without* it to produce `proof_tree.bin` (and with
  stdin closed, since the solver spawns a stdin reader).

## Trade-offs / unresolved parts

- **The live proof tree can be an incomplete proof at transpositions** (the
  finding above). This predates plan5 (plans 1–3 infrastructure). A fix
  belongs to the proof-tree layer: prefer *completely* expanded twins in
  `finalize()`'s canonical selection (e.g. require every non-terminal child of
  a Loss twin to be present, or prefer the twin with the most children among
  consistent equal-depth candidates), plus a builder-side completeness
  validator (item #6 territory). Until then, a reconstructed tree can be
  *more* correct than the live dump it is validated against.
- **The oracle compares structure only.** It cannot detect that *both* trees
  are wrong in the same way; the reconstruction's soundness rests on the
  snapshot records plus the walk's structural invariants (expected-complement
  checks, Loss-completeness by construction). A position-replaying validator
  (`verify_ppv`-style, item #6) would close this.
- **Seeding unsolved records** remains unimplemented (per plan, measure
  first): with 2 fill evals total, ordering hints are irrelevant.
- **First-wins duplicate handling** is deterministic given native bucket
  order, but cross-generation duplicates were never exercised at scale here
  (single-generation searches; `duplicate_keys` stayed 0 across the suite).

## Missing tests

- No automated test covers the live-tree incompleteness mechanism (a Loss
  node realized via TT-hit re-emission with a partially expanded twin); it
  needs a position with a same-hash transposition expansion, which is hard to
  construct deterministically — the dec46 case is a manual reproduction.
- No test drives `--fill-total-budget` to exhaustion (the failure path is
  one `if` in `fill_hole`); the depth-cap exhaustion path is likewise only
  covered implicitly by the anomaly tests.
- No test exercises `clock_miss_draw` end-to-end (a fill of a Draw-record
  clock variant); the unit test covers classification only, since crafting a
  real solved-Draw-at-another-clock snapshot requires a rule50-drawing proof.
- The experiment is not wired into any make tier (it is a release-run tool;
  ~6 min wall clock) — rerun manually when the proof-tree layer changes.

## Next steps

1. **Fix the finalize canonicalization** (small proof-tree plan before or with
   plan6): prefer fully expanded twins; add a Loss-completeness validator to
   the builder (item #6). Re-run `--experiment` to confirm 46/46 isomorphic.
2. **plan6 — flip the default (worker off during search)**, gated on (1):
   the experiment supports it (hole rate 0.003%, F/C ≈ 0, zero anomalies),
   and the reconstructed tree is already the more complete artifact.
3. Item #4 (pin solved TT entries) is **not** motivated by the hole breakdown
   (`absent` = 2/91,470); keep it conditional on deep multi-day runs where
   eviction pressure is real.
4. Item #5 (periodic TT checkpoint) unchanged (stretch).
