# Performance Plan 3: TT pre-probe before make/unmake

Initiative: `performance`, backlog **item 2** (report §10, "TT pre-probe before
make/unmake"; measured in §7.3 of
[`docs/performance/performance_report1.md`](../../performance/performance_report1.md)).
Successor of [`plan1.md`](plan1.md) / [`report1.md`](report1.md) (cheap preflight
probe) and [`plan2.md`](plan2.md) / [`report2.md`](report2.md) (lazy TT
allocation). Drafted 2026-10-28; plan session wrote no product code (both
probe builds live in `/tmp`).

*Ownership note.* Backlog items 2/3/4 are marked "may move to `lean`". This plan
executes item 2 here because the report's evidence, the corpus drivers and the
identity protocol of this initiative are already in this directory, and the
change is a bit-identical restructuring of the child-evaluation hot path rather
than a timing-only micro-optimization. The plan is self-contained either way; if
the owner prefers `lean` to execute it, this file and
`measurements/plan3/` move with it.

## Why this plan exists

Report §7.3 instrumented what `evaluate_child` finds *after* it has already
played the child move: on the three deep workload cells, **13–38 % of child
evaluations already have a usable TT entry**, so the move is played (and undone)
only to compute the child's TT key and repetition key. The report estimated a
3–8 % throughput win and ranked the change as the highest-throughput idea in the
backlog ("worth a dedicated plan"), because it touches `ChildInfo` production
and the prefetch pre-pass.

Plan 2 closed the short-latency front (mate-in-1 104.6 → 1.2 ms; the remaining
floor is process start). What is left in the backlog is throughput:
items 2 (this plan), 3 (incremental repetition-context hash), 4 (path Bloom
filter) and 6 (repetition-cache hasher). Item 2 has the largest measured
substrate and is the one with a real semantic surface to pin down, so it goes
first.

### Plan-session probe (read-only; two `/tmp` copies, nothing landed)

Two throwaway builds off `git archive HEAD` (`b1414b7`):
`/tmp/plan3_probe` (census counters only) and `/tmp/plan3_proto` (a working
prototype of the design in §*Normative design*). The prototype is small: one new
`Position` accessor, a keys pre-pass in `evaluate_all_children`, and a
pre-probe ladder at the head of `evaluate_child`.

Host for every number below: aarch64 container, 6 CPUs, release profile (this
is **not** the host `report2.md` measured on — absolute walls are not
comparable across reports); provenance in
[`measurements/plan3/plan_session/`](measurements/plan3/plan_session/) with the
A/B driver, its parsed JSON and `env.json`.

**Census — what a pre-probe can decide without playing the move** (probe build,
`--tt-size 256 --first-outcome --outcome-only`). "skip" = the child has a live
current-generation TT entry, is *not* a path repetition, and rule50 has not
expired:

| cell | child evals | solved-entry skip | live-unsolved skip | path-rep (must fall back) | skip rate |
|---|---:|---:|---:|---:|---:|
| `dec10` | 3,901,847 | 50,903 | 1,435,215 | 43,726 | **38.1 %** |
| `m22` | 15,023,911 | 115,236 | 2,308,917 | 14,100 | **16.1 %** |
| `m22_b` | 10,000,032 | 62,103 | 1,294,098 | 4,463 | **13.6 %** |
| `stress_b` | 10,000,015 | 28,901 | 1,402,274 | 3,236 | **14.3 %** |
| `dec13` | 5,030,497 | 75,298 | 349,008 | 1,124 | **8.4 %** |
| `rem12` | 66,604,453 | 684,424 | 4,598,868 | 16,746 | **7.9 %** |

The overlapping cells reproduce report §7.3's counts exactly (`dec10`:
1,435,215 unsolved / 51,6xx solved / 43,726 repetitions; `m22_b`: 1,294,098 /
62,337 / 4,463), i.e. the report's instrumentation and this census are the same
measurement, re-taken on the current binary. The census additionally shows the
lever's real shape: **the skip rate is a property of transposition density, not
of tree size** — `dec10` (a short, very transpositional 36-ply win) is at 38 %,
while the long cells (`rem12` at 66.6 M evals) sit near 8 %.

**Prototype A/B** (interleaved ABAB in one process pair, `--tt-size 256
--first-outcome --outcome-only`; reps per case in the table; `child_evals`,
stdout, outcome and PV length **identical in every arm**):

| cell | reps | baseline median | prototype median | ratio | speedup |
|---|---:|---:|---:|---:|---:|
| `mate1` (2 evals) | 15 | 0.5 ms | 0.5 ms | 0.963 | — (noise) |
| `easy_kq` (probe-solved, 378 evals) | 15 | 1.0 ms | 1.0 ms | 1.015 | — (noise) |
| `dec13` | 11 | 0.528 s | 0.516 s | 0.976 | 1.024× |
| `dec10` | 11 | 0.352 s | 0.336 s | **0.954** | **1.048×** |
| `m22` | 11 | 1.472 s | 1.416 s | **0.962** | 1.039× |
| `m22_b` | 11 | 0.964 s | 0.941 s | **0.976** | 1.025× |
| `stress_b` | 11 | 0.917 s | 0.893 s | **0.974** | 1.027× |
| `rem12` | 7 | 6.965 s | 6.812 s | **0.978** | 1.022× |

Identity evidence already collected on the prototype: `cargo test --release`
613 passed / 0 failed, `test_trajectory_golden` **passes without regoldening**,
quick suite **59/59 eval-identical** (42,109,768 total child evals, 0 wrong,
0 timeouts) vs `measurements/plan2/state/quick_after.json`.

**Mechanism attribution** (`perf record -e cpu-clock -g`, leaf attribution
`--no-children`, baseline binary, `dec10` ×15). The skipped work is exactly the
pair the report priced:

| symbol | total | inside `evaluate_child` |
|---|---:|---:|
| `Board::do_move` | 10.2 % | 8.8 % |
| `Board::undo_move` | 4.3 % | 3.4 % |
| `Board::hash_after` (existing prefetch pre-pass) | 5.5 % | — |
| `evaluate_child` | 12.8 % | — |

Make/unmake inside `evaluate_child` is **12.2 % of CPU**; × the 38.1 % skip rate
predicts 4.6 % on `dec10`, and 4.8 % is measured. The `hash_after` 5.5 % is the
*existing* prefetch pass (lean plans 11/12); the design keeps it and reuses its
keys, so the pre-pass cost is unchanged.

**One design correction found by the probe (load-bearing).** The prototype's
first version probed the TT in the pre-pass *and* re-probed it on the fallback
(make/unmake) path. That version measured `dec10` 0.952 but `m22_b` 0.996 and
`stress_b` 1.002 — the extra probe on the 62–86 % of children that fall back ate
the whole saving. Reusing the pre-probed entry on the fallback path (§D-f) is
what moves the deep cells from ~0 % to 2.2–2.5 %. Do not drop it.

**Absolute-number caveat for the execution session:** absolute walls on this
container have drifted since `report2.md` (`dec10` 0.65 → 0.35 s, `m22` 2.19 →
1.47 s). Phase 0 re-measures the baseline in-session; **the contract is the
ratio**, never a remembered absolute.

## Question and pre-registered decision rule

**Q:** Can the 8–38 % of child evaluations that already have a usable TT entry
be decided from keys computed *without* playing the move (no
`do_move`/`undo_move`, no terminal existence query), with **bit-identical
trajectories**, for ≥ 4 % median-wall on the transposition-dense class and ≥ 1 %
on every other measured deep cell?

**ADOPT (GO)** only if all hold:

1. **Identity.** Every corpus cell is bit-identical to the Phase-0 baseline in
   `child_evals`, outcome, PV length, `pv_status` and captured stdout;
   `test_trajectory_golden` (`m22` default mode) passes **without
   regoldening**; the portfolio `dec10` anchor is exact at 3,901,847 evals; the
   quick suite is **59/59 eval-identical** (42,109,768 total child evals,
   0 wrong, 0 timeouts); the new unit tests pass, including the
   proof-event-stream identity test (§D-e) and `Position::keys_after` equality
   over every legal move of the §D-a fixture set.
2. **Mechanism.** A test-only skip counter is `> 0` on a fixture with a forced
   live TT entry (the fast path actually executes), and the report records the
   census skip rate for every corpus cell (the Phase-0 census reproduces
   `dec10` ≥ 30 %).
3. **Throughput (interleaved A/B, ≥ 11 reps/case, ABAB).** Median wall ratio
   ≤ **0.96** on `dec10`; ≤ **0.99** on each of `m22`, `m22_b`, `stress_b`,
   `dec13`, `rem12`.
4. **No regression.** No deep/budget cell has a median wall ratio > **1.01**;
   the two short cells (`mate1`, `easy_kq`) are ~0.5–1 ms and noise-dominated,
   so their bar is a loose ≤ **1.05** guard that only catches a structural
   mistake (e.g. an allocation per node).
5. **No semantic surface change.** No CLI knob, no new public API, no change to
   TT semantics/sizing/eviction/salt, no ordering or `ChildInfo` field change,
   no `ProofEvent` change, no preflight change.
6. **Gates.** `make test` and `make test-full` green; `cargo fmt --check`,
   `cargo clippy --all-targets`, `cargo doc` clean; new units fast-tier only.

**NO-GO / falsification handling.**

- **Any** `child_evals` difference on a fixed-budget fixture means the fast path
  is not behaviourally equivalent — STOP and root-cause before touching
  anything else. Do not re-baseline a golden, an anchor or a cap. The likely
  causes, in order: (a) the pre-probe took a branch the shipped ladder would
  not have taken (re-check the §D-c ordering, especially the repetition check
  and `rule50_expired`); (b) the non-terminal invariant of §D-d does not hold
  at some store site; (c) the `best_move` guard was evaluated on the wrong
  position.
- **Bar 3 or 4 fails** → NO-GO; revert the whole change and record the measured
  ratios. There is no partial mode and no fallback knob: the change is an
  all-or-nothing restructuring of one hot function, and a knob that toggles it
  would have to reproduce a bit-identical path anyway.
- **Bar 2 fails** (the counter never fires) → the prototype's gain came from
  something else; STOP and re-derive the mechanism with `perf` before
  proceeding.

## Scope

**In scope**

- `Position::keys_after` / `Position::repetition_key_after` (`pub(crate)`,
  non-mutating; the TT key, repetition key and rule50 of the position after a
  move, without playing it).
- The keys pre-pass in `evaluate_all_children` (replacing the prefetch loop's
  throwaway `hash_after` results with reused ones) and the `Search::keys_scratch`
  buffer.
- The pre-probe ladder at the head of `evaluate_child` (split into
  `evaluate_child` + `evaluate_child_keyed`), including the entry reuse on the
  fallback path.
- Unit tests: `keys_after` equality, the pre-probe equivalence fixtures, the
  proof-event identity test, the mechanism counter test.
- Docs: `src/search/dfpn/children.rs` module/`ChildPrecompute` docs (the
  pre-probe and its invariant), `src/search/dfpn/core.rs` (cross-reference at
  the #12a comment), one `AGENTS.md` clause.
- Measurement record under `docs/plans/performance/measurements/plan3/`.

**Out of scope (non-goals)**

- Backlog items 3 (incremental repetition-context hash), 4 (path Bloom filter),
  6 (repetition-cache hasher) — item 3 is the recommended next plan; see
  §Follow-ups in `report3.md`.
- Any change to `Search::sort_moves`, the `MoveScorer`, the heuristic
  constants, the restart schedule, epsilon, the refinement caps, the repetition
  cache, the TT representation or the preflight/cheap-probe paths.
- Skipping the terminal existence query for children that have **no** TT entry
  (that query is 8.5 % + 7.3 % of CPU on `dec10`; removing it needs a different
  invariant and is not part of this plan).
- `Position::hash_after`'s upstream implementation, and any `atomic-movegen`
  change (backlog item 9 stays upstream).

## Normative design

### D-a. `Position::keys_after` / `repetition_key_after`

```rust
/// Repetition key ([`Position::repetition_key`]) of the position after `m`,
/// without playing it.
#[must_use]
pub(crate) fn repetition_key_after(&self, m: Move) -> u64 {
    self.board.hash_after(m)
}

/// `(tt_key, repetition_key, rule50)` of the position after `m`, without
/// playing it. Equal to `{ do_move(m); (hash(), repetition_key(),
/// board().rule50()) }`.
#[must_use]
pub(crate) fn keys_after(&self, m: Move) -> (u64, u64, u16) {
    let board_only = self.board.hash_after(m);
    let rule50 = self.board.rule50_after(m);
    (board_only ^ zobrist::rule50_key(rule50), board_only, rule50)
}
```

Upstream `Board::hash_after(m)` is the *board-only* hash after `m`, i.e. exactly
`repetition_key`; `Position::hash_after` is that value XOR the rule50 key (fixed
by `Position::refresh_zobrist`). Both upstream calls are non-mutating and
covered by the upstream 2.3.0 contract; `hash_after_equals_do_move_hash_for_every_legal_move`
already re-checks the full-key equality consumer-side and the new test extends
it to the triple (`repetition_key` and raw `rule50`, un-clamped — the ladder
needs the raw value for the `>= 100` branch, while `rule50_key` clamps at 100).

### D-b. Keys pre-pass (replaces the throwaway prefetch keys)

`evaluate_all_children`'s prefetch pre-pass (lean plan 11) currently computes
`pos.hash_after(moves[i])` for every child and throws the value away after the
`prefetch` address computation. The pre-pass becomes:

```rust
if self.keys_scratch.len() < moves.len() {
    self.keys_scratch.resize(moves.len(), (0, 0, 0));
}
for i in 0..moves.len() {
    let keys = pos.keys_after(moves[i]);
    self.tt.prefetch(keys.0);
    self.keys_scratch[i] = keys;
}
```

so the pre-pass costs the same as today (one `Board::hash_after` + one
`rule50_after` per child) and now also supplies the pre-probe keys. The eval
loop passes `self.keys_scratch[i]` into `evaluate_child_keyed`. The public
`Search::evaluate_child` keeps its signature and computes its own keys (it is
used by the best-child re-evaluation site in `core.rs` and by the unit tests);
`keys_scratch` is a plain `Vec` field on `Search` with no invariant beyond the
`resize` above: it only grows, every frame's `0..moves.len()` prefix is written
before it is read, and entries past `moves.len()` are stale-but-never-read.

### D-c. The pre-probe ladder (the semantic core)

`evaluate_child` becomes a thin wrapper over `evaluate_child_keyed(pos, mv,
max_depth, is_or_node, child_key, child_rep_key, rule50_after)`. The ladder runs
**in this order**, which is the shipped order with the make/unmake hoisted out:

```text
1. child_is_or = !is_or_node
2. rule50_expired = rule50_after >= 100
3. if !rule50_expired && path_contains(child_rep_key):
       -> Draw, repetition_seen = true, depth 0       (return, no make)
4. if !rule50_expired, probe the TT once with child_key:
       a. resolved_from_entry(entry, child_max_depth) and
          (entry.best_move == NONE ||
           !path_contains(pos.repetition_key_after(entry.best_move))):
             -> the solved child                                  (return)
       b. entry.outcome.is_none():
             -> unsolved bounds, shipped degeneracy rule
                (pn > 0 && dn > 0 && remaining_depth != MAX &&
                 remaining_depth <= child_max_depth, else (1, 1))  (return)
       c. otherwise (live, solved, depth guard or best-move guard failed):
             fall through
5. shipped make/unmake path, unchanged, EXCEPT that it uses the entry from
   step 4 instead of probing again (D-f)
```

Every branch is the shipped branch with the position-identical input:

- **Step 3** is the shipped repetition branch; its input `child_rep_key` is now
  computed without playing the move, and the branch is *identical* (including
  `repetition_seen = true` and `depth = 0`). It must stay **before** the TT
  probe — the shipped code classifies a path repetition as a Draw regardless of
  what the TT holds.
- **Step 4a** is the shipped `resolved` branch; the one-ply best-move guard
  `best_move_repeats_path` is replaced by the equal non-mutating
  `path_contains(repetition_key_after(best_move))` (the shipped helper plays
  and undoes the move purely to obtain that key — soundness comes from
  D-a's equality, pinned by the new test).
- **Step 4b** is the shipped `#12a` reuse branch, including the degeneracy
  rule; it is exactly why no terminal classification is needed on this path.
- **Step 4c → 5** is the shipped fall-through: a live-but-unusable entry says
  nothing about the child's bounds, so the full make/unmake path runs.

### D-d. The load-bearing invariant (state it, and assert it)

Steps 3–4 skip the terminal classification (extinction ×2, `occupied == 2`,
`has_legal_move`). That is sound only because of the invariant already
documented at the `#12a` site in `children.rs` and at the store sites in
`core.rs`: **a live current-generation entry for a key was stored only after the
position passed the full `outcome_from_state` check** — hence it has legal
moves, `rule50 < 100` and `occupied > 2` for that exact key (the key includes
rule50). Terminality is position-static, so no live entry can belong to a
terminal position, and `rule50_expired` is a function of the key too (which is
why step 4 is gated on `!rule50_expired`: a rule50-expired child's key would
clamp and the store could never have happened).

The plan's obligations for this invariant:

- restate it in one paragraph at the pre-probe site (the invariant is now
  load-bearing for *two* sites, not one);
- add a `debug_assert_eq!(child_key, pos.hash_after(mv))` at the make path so a
  future divergence of the two key computations fails loudly in debug/test
  builds;
- list the store sites in the comment (`core.rs` terminal store, depth-0 leaf
  store, GHI-draw-suppressed store, snapshot import) so a future store-site
  change knows it must revisit the pre-probe;
- behaviour is not changed if a *future* store site ever violated the invariant
  in a way that produced a live entry for a terminal position: the fast path
  would return the stored outcome instead of the terminal one — which is the
  same outcome by the solver's own semantics, so the failure mode is a
  `child_evals` drift (caught by bar 1), not a wrong answer.

### D-e. Proof events are byte-identical

The shipped code emits `ProofEvent::NodeProven { path, pos.hash(), outcome,
depth }` for decisive children after the make. The fast path emits the same
event with `child_key` in place of `pos.hash()` (equal by D-a) and the same
`depth`/`outcome`; Draw children emit nothing in both paths. A unit test
captures the event stream of a small solve with a *forced* supported pre-probe
(live TT entries pre-seeded) and asserts it is equal to the stream of the same
solve with the TT entries removed (make/unmake path). The proof-tree layer's
contracts (`Move`/hash-based tree, replay validation) are untouched.

### D-f. One probe per child on the fallback path

The pre-probe's `Option<TtEntry>` is passed into the fallback instead of being
re-dropped. Nothing between the pre-probe and the fallback's probe can modify
the table (`do_move_with_scratch` touches the position, `path_contains` touches
`path_stack`), so the two probes are guaranteed-identical reads; the plan-session
probe measured that dropping this reuse costs the entire deep-cell gain
(§*Why this plan exists*). The fallback keeps a `debug_assert_eq!(child_key,
child_key_fast)`.

### D-g. What is deliberately untouched

`ChildInfo`'s fields and construction, the `evaluate_all_children` early-break
on a decisive child (the pre-pass still computes keys for the whole move list;
this is the same work the prefetch pass did before), `sort_moves`, the history/
killer tables, `child_evals` accounting (the fast path still counts exactly one
eval per child), and every TT entry field. No new allocation on the hot path
(`keys_scratch` is resized once per depth-band and reused), no `unsafe`.

## Tasks

1. **Phase 0 — baseline and census (read-only).** Build the pristine `HEAD`
   release binary into `/tmp/plan3_base` (`git archive HEAD`) and record the
   single-arm baseline corpus (5 reps/case, `--tt-size 256`, per-case args);
   re-run the census (a temporary `/tmp` build with per-branch counters, as in
   the plan session) for the skip rate of every corpus cell. Save
   `measurements/plan3/state/{baseline,census}.json` + `env.json`. Reuse
   `measurements/plan2/ab_wall.py` and `driver.py` (add the census cell fields;
   keep `sys.dont_write_bytecode`).
2. **`src/position.rs`** — add `repetition_key_after` and `keys_after`
   (`pub(crate)`), and the test
   `keys_after_equals_do_move_triple_for_every_legal_move` (same fixture set and
   style as `hash_after_equals_do_move_hash_for_every_legal_move`; assert the
   full key, `repetition_key`, and raw `rule50` against a played-and-undone
   round trip, and that `pos` is untouched).
3. **`src/search/dfpn/mod.rs`** — add the `keys_scratch: Vec<(u64, u64, u16)>`
   field (doc comment: per-child key scratch for the pre-pass).
4. **`src/search/dfpn/children.rs`** — the keys pre-pass (D-b), the ladder
   (D-c), the invariant paragraph (D-d), the event identity (D-e), the entry
   reuse (D-f). Keep the file's existing size justification; update the module
   doc to name the pre-probe.
5. **Tests** (`children.rs` tests + `dfpn/tests.rs` where shared fixtures live;
   all fast tier, no `#[ignore]`):
   - `preprobe_resolves_live_unsolved_entry_without_make` — seed a live
     unsolved entry for a quiet child's key; assert the returned `ChildInfo`
     equals what the make path returns for the same position (bounds and
     `outcome: None`), and that the test-only skip counter incremented.
   - `preprobe_resolves_solved_entry_and_emits_the_same_proof_event` — the D-e
     stream-identity test (supported path vs forced make/unmake path).
   - `preprobe_falls_back_on_path_repetition` — the repetition branch wins over
     a live solved entry (`repetition_seen == true`, Draw, depth 0).
   - `preprobe_falls_back_when_rule50_expired` — a rule50-expired child with a
     seeded entry still classifies via the shipped rule50 branch.
   - `preprobe_degenerate_bounds_fall_back_to_neutral` — live unsolved entry
     with `pn == 0` (and one with `remaining_depth == u32::MAX`) → (1, 1).
   - `preprobe_best_move_guard_matches_make_path` — a live solved entry whose
     `best_move` repeats the path yields the shipped neutral result in both
     paths.
   - the census counter is `#[cfg(test)]`-only (`Search::preprobe_skips`,
     mirroring `RepetitionCache::hits`), reset per run in `begin_run`.
   The pre-existing `evaluate_child_*` unit tests (which call
   `evaluate_child` with `max_depth = u32::MAX` and push onto `path_stack`
   directly) must pass unchanged — they are the regression net for the ladder.
6. **Docs** — `children.rs` module doc + `ChildPrecompute`/eval-scratch notes
   (the pre-probe and the keys pre-pass), the `core.rs` `#12a` comment gains a
   one-line cross-reference, `AGENTS.md`'s `src/search/dfpn/` bullet gains the
   pre-probe clause (keys computed by the prefetch pre-pass, live-entry
   children decided without make/unmake, the non-terminal invariant).
7. **Measure and gate** (Validation below) with an interleaved A/B driver
   (`measurements/plan3/ab_wall.py`, ≥ 11 reps/case, ABAB) as the authoritative
   wall comparison; single-arm runs are not accepted for bars 3/4. Commit the
   parsed `state/*.json` and drivers only (no raw transcripts).
8. **Write `report3.md`**, update `initiative.md` (item #2 row + the item↔plan
   mapping table) and the `docs/plans/README.md` `performance` row.

## Corpus

Measured with `--tt-size 256 --first-outcome --outcome-only` plus the per-case
argument; skip rates are the plan-session census, prototype ratios the
plan-session A/B (both are the calibration for bars 2–4, not a prediction to
match):

| id | FEN | args | class | skip | proto ratio |
|---|---|---|---|---|---|
| `mate1` | `3Qk3/8/8/8/8/8/8/4K3 w - - 0 1` | `--timeout 2` | 2-eval short floor | n/a | 0.963 (noise) |
| `easy_kq` | `4k3/8/8/8/8/8/8/4KQ2 w - - 0 1` | `--timeout 2` | probe-solved easy win (378) | n/a | 1.015 (noise) |
| `dec13` | `r1bq1k1r/ppN4p/n1p1p3/3p1n1P/1b1P2P1/2P5/PP6/RNBQKB1R w KQ - 1 14` | `--timeout 10` | mid completing win (5.03 M) | 8.4 % | 0.976 |
| `dec10` | `3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41` | `--timeout 10` | transposition-dense completing win (3.90 M) | **38.1 %** | **0.954** |
| `m22` | `4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22` | `--timeout 10` | deep completing win (15.02 M), golden trajectory | 16.1 % | 0.962 |
| `m22_b` | same as `m22` | `--budget 10_000_000` | censored deep cell | 13.6 % | 0.976 |
| `stress_b` | `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21` | `--budget 10_000_000` | censored, capacity-saturated | 14.3 % | 0.974 |
| `rem12` | `rnbqkbnr/8/6pp/pppppp1B/3PPP2/N5PN/PPP4P/R1BQ1RK1 w kq - 0 10` | `--timeout 60` | long completing win (66.6 M) | 7.9 % | 0.978 |

`rem12` needs its own wall cap (its baseline run is ~7 s ⇒ ≥ 11 reps is 2.5 min
per arm); `mate1`/`easy_kq` are the latency-floor sanity cells.

## Validation

1. `make test` (fast tier) — all new units.
2. `make test-full` — required: `test_trajectory_golden` is the primary
   trajectory-identity evidence (bar 1), plus the 60 s regression/stress suites.
3. Portfolio `dec10` anchor: `cargo test --release --test portfolio_smoke` →
   3,901,847 evals, identical outcome/PV.
4. Quick suite: `benchmark --suite quick --first-outcome --timeout 3 --json` →
   59/59 eval-identical vs `measurements/plan2/state/quick_after.json`
   (42,109,768 total child evals, 0 wrong, 0 timeouts).
5. Corpus A/B per bars 3/4, interleaved (`measurements/plan3/ab_wall.py`), with
   per-run `child_evals` equality asserted inside the driver (any mismatch is a
   hard error, not a data row).
6. Census re-run (bar 2) on the final tree, recorded per cell.
7. `cargo fmt --check`, `cargo clippy --all-targets`, `cargo doc` (no new
   warnings). No miri run needed: no `unsafe`, no allocator work.
8. Optional (recommended if cheap): `perf record -e cpu-clock -g` on `dec10`
   before/after to confirm `do_move`/`undo_move` inside `evaluate_child` drops
   by ≈ the skip rate — the mechanism check that bar 2 only proxies.

## Report requirements

`docs/plans/performance/report3.md` must contain:

- the Phase-0 baseline and after corpus tables (median wall, `child_evals`,
  peak RSS, minor faults) and the interleaved A/B ratios per cell;
- the census table (per cell: solved-entry skip, live-unsolved skip,
  path-repetition fallbacks, skip rate) and the mechanism evidence
  (`perf` leaf table before/after if run);
- the identity evidence (golden without regoldening, `dec10` anchor, quick
  suite total, every corpus cell's `child_evals`/stdout equality);
- the equivalence argument as implemented (§D-c order, §D-d invariant,
  §D-e events, §D-f single probe) and any place where the implemented ladder
  differs from this plan;
- the ADOPT/NO-GO verdict against the six bars, with the exact `max` ratios;
- findings/follow-ups: in particular whether the skip rate predicts the gain
  well enough to make the lever class-scoped in future work, whether the
  now-single TT probe per child suggests anything for the repetition cache, and
  the state of backlog items 3/4/6 (item 3 as the recommended next plan).

## Follow-ups (not this plan)

- **Backlog item 3** (incremental repetition-context hash): report §7.1
  measured 2–3 % on `m22`/`m20` and ~1 % on `dec10`, bit-identical, low risk.
  It touches a different surface (`path_push`/`path_pop` bookkeeping in
  `mod.rs`, `children.rs`, `history.rs` and the `path_stack` mutation sites) and
  is the natural plan 4.
- **Backlog item 4** (path Bloom filter): 1–3 %, noisy; only after item 3, and
  only if item 3's path bookkeeping lands (the Bloom filter needs the same
  "mutate `path_stack` only through push/pop" discipline).
- The terminal existence query (`has_legal_move_with_state`, 8.5 % + 7.3 % on
  `dec10`) remains the largest single `evaluate_child` cost after this plan; it
  is out of scope here and would need an invariant of its own.
