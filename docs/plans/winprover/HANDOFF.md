# Handoff — from-scratch provari-parity atomic win prover

**Status:** successor handoff for the closed [`winprover`](initiative.md)
initiative. This is **not** a plan and **not** a report. It is the
self-contained starting point for the session that opens the successor
initiative and writes its `plan1.md`.

| Item | Value |
| --- | --- |
| Date | 2026-11-02 (UTC) |
| From | the `winprover` close-out ([`report5.md`](report5.md)) |
| Pinned oracle | `aiorla/provari` commit `48686bc6c786d6ea70568f0b2ad4002cf0520206` (`v0.1.0`) |
| Current product | `atomic_solver` at the commit of this handoff; **unchanged** by this file |
| Successor | a new initiative (working name `prover`), not a `winprover` plan |

The recommendation is a **from-scratch attacker-only win prover** that takes
provari's search core and this repo's evidence/tooling layer. The current
general solver stays the product until the new prover wins the bench; nothing
in this handoff changes product behaviour.

---

## 1. Why from scratch — the measured case

The `winprover` initiative ported every portable provari lever into the
existing general solver. The porting strategy was right and it worked, then
it plateaued:

| Stack | `core` | `easy-win` | `deep-win` | Geomean | Coverage |
| --- | ---: | ---: | ---: | ---: | ---: |
| product `default` | censored (≥ 20.04) 10.011 s / 72.2 M | **44.06** (5.772 s / 43.3 M) | censored (≥ 1.59) 10.012 s / 74.3 M | **44.06** | 1/3 |
| best opt stack | **7.615** (3.800 s / 17.2 M) | **5.214** (0.683 s / 3.2 M) | censored (≥ 1.59) 10.011 s / 47.6 M | **6.301** | 2/3 |
| provari `--guide hand` | 1.000 (0.499 s / 342 k nodes) | 1.000 (0.131 s / 72.5 k nodes) | 1.000 (6.297 s / 4.57 M nodes) | 1.000 | 3/3 |

Canonical 10 s / 192 MB, same session, provari hand = 1.000. The opt stack is
`--move-class-pn --quiet-grouping --eval-pn or`; it is **not** promoted because
it turns three quick-suite `Loss` cases (`dec06`, `dec42`, `dec44`) into 3 s
draws.

The remaining gap is not another lever. The plan5 probes:

- **candidate/verification pivot**: 0/41 and 0/38 root moves proved at 1 M
  evals each — no affordable candidate source;
- **rule50-free key**: 0–0.7 % and no coverage change (the measured upper
  bound, since the prototype was deliberately unsound);
- **front-end heuristic parity**: the opt starting numbers do not fix the
  front-end — it proves `core` only at ~30.9 M evals / ~15 s (the general
  solver: 13.8 M / 3.8 s) and cannot prove `easy-win` at 50 M.

The opt-in front-end (`src/search/winprover/`) is the right **shape** but the
wrong **engine**: its child evaluation is a full `Position::outcome()` movegen
per child, and it inherits general-game machinery a win-only prover does not
need. That is the architectural gap.

**What we learned, in one sentence:** the portable ideas were real and bought
~8× on the shared cells (51.47 → 6.30), but the remaining ~6× is per-node
cost and search efficiency in the engine core, not a missing flag.

---

## 2. Target and protocol (do not silently change)

**Primary column:** `provari prove --guide hand`. The `--guide model` column
is a recorded stretch column only; no learned model is a product dependency.

**Bench cells** (`docs/plans/winprover/bench.md`):

| Cell | Id | FEN / source |
| --- | --- | --- |
| `core` | — | `r1b1kbnr/ppp3pp/n4q2/3pppN1/1P1PP3/8/P1P2PPP/RNBQKB1R w KQkq - 0 1` |
| `easy-win` | `1eb80b92f92c` | frozen `sample20` corpus |
| `deep-win` | `db6a741c723f` | frozen `sample20` corpus |

**Corpus:** `docs/plans/provari/measurements/plan1/corpus.json`, sha256
`9cd0169a30bb7d5b162b6a1ec119fe194b776c8a24a46cdf1582c07062847d0e`.

**Budget:** 10 s and 192 MB label for both sides; three repeats per cell and
side for any reported wall ratio. Provari:
`prove --seconds 10 --memory 192 --guide hand`. Ours: `--timeout 10
--first-outcome --tt-size 192` plus the measured stack.

**Success condition:** geometric-mean wall `ours / provari hand < 1.0` on the
shared-win set, coverage at least provari's (3/3 on these cells), every claimed
win replay-validated and accepted by the independent checker, no outcome
regressions, existing gates green.

**Reading rules:** same session, same host; wall is the product metric,
`child_evals` the mechanism metric; censored is a result; opt-in is not
shipped. See [`bench.md`](bench.md) §Reading rules.

---

## 3. What to take from provari (the search core)

Read the pinned source in a scratch clone (never commit it). The mechanisms,
in the order they matter:

### 3.1 Attacker-only value semantics

Provari proves exactly two things: **attacker wins** or **refuted**. There is
no draw value to prove, no loss proof, no general outcome, no PV/refinement
machinery inside the search. A refuted node is `disproof = 0` carrying the
earliest repeated position on the line and whether the 50-move limit is
involved (`Known::Refuted { cycle, from }`). This is a smaller problem than a
general Win/Loss/Draw search, and it relaxes caching: a proof's length shrinks
along the line, so a proof cannot repeat a position and can be reused across
paths.

**Take:** the value type and the two-outcome search. Do not add a draw proof.

### 3.2 Per-position precomputation + effects (the measured bottleneck)

For each expanded position provari builds one `Mover` (`src/moves.rs`): pins,
check squares by role, uncovering pieces, attack maps, explosion zones beside
each king, cheap-blast targets. Every child question is then a bit test on
that context: `legal`, `explodes_them`, `exposes`, `gives_check`, `threats`,
`plain`, `blasted_cheaply`, `connectable`, `might_end_material`.

The child hash comes incrementally from `child_base() ^ effect()`; `Effect`
carries the child hash, occupancy, capture flag, and 50-move counter. The TT
probe/prefetch happens **before** the child is materialized. `pos.play(m)` is
only called for the minority that needs it (check replies, tablebase,
`cannot_win`, mate verification).

**Take:** this layer first. It is the difference between O(children × child
movegen) and O(1 position precompute + children × bit tests). The current
front-end's full `Position::outcome()` per child is the measured cost.

### 3.3 Static tactics as child scoring, not node-entry short-circuits

Provari settles immediate explosion, exposed king, a check with no safe
evasion, a double threat no defence parries, cannot-win material, mate in one,
and tablebase — **per child during scoring**, setting both the value and the
starting proof numbers. This feeds DF-PN's ordering instead of bypassing it.
Our `--static-tactics` node-entry closure measured neutral/worse for exactly
this reason.

**Take:** the per-child form. Do not repeat the node-entry closure.

### 3.4 Search guidance from the first expansion

- **Move-class starting numbers:** checks start at the number of safe evasions
  (capped at 8), double threats at 1, threats at 4, weak threats at 8, quiet at
  9, hanging quiet at 15.
- **EF-PN scaling:** a hand evaluation produces a step that scales every fresh
  child's proof/disproof numbers (`scale`).
- **Quiet grouping:** all plain attacker moves become one delayed child
  (`proof = QUIET`, `disproof = count`), opened only when selected.
- **1+ε:** `widen(second) = second + 1 + second/8` (ε = 1/8).

The provari ablations are multiplicative: flatten starting numbers **2.3×**
slower, disable quiet grouping **1.4×**, remove the guide **1.8×** — roughly
**5.8×** combined. Our ports captured most of that; the new engine should
start with all of it, not rediscover it.

### 3.5 Sibling simulation

At an AND node, when a defence is refuted, replay its proof on open siblings
(`simulate`, `simulate_defences`, `replay_threat`). Our first-form replay
works (12–27 % of attempts transfer a proof, frames cut ~20–24 %) but did not
close a root at a product cap. In the new engine it is a first-class part of
the AND-node loop, not an add-on.

### 3.6 Memory and table engineering

- 16-bit moves, `MaybeUninit` move lists, per-depth child buffers.
- TT: 4-slot clusters sharing a cache line, prefetch, a Bloom-style filter
  that is dropped when it stops paying, least-work replacement.
- Proofs: separate open-addressing table, 12-byte entries (key + move 15 bits
  + length 10 + limit 7), keeps the shortest proof and longer ones at higher
  counters, drops mates in one when full.
- Board-only key + proof length/limit makes cross-clock proof reuse sound.

### 3.7 Proof output and independent checking

Provari writes a compact `.pvr` (zstd) or JSON-lines proof: a strategy map
keyed by board hash, with the winning move per attacker position. `provari
check` re-walks it using shakmaty only, sharing no code with the prover.

**Take:** the checker as the independent oracle. Do not make `.pvr` the
internal proof format (see §4.1).

---

## 4. What to take from `atomic_solver` (the evidence and tooling layer)

This is the part provari does not have, and the reason a from-scratch engine
here is not a from-zero project.

### 4.1 ProofEvent + offline proof tree

Our search never builds the proof tree. It emits `ProofEvent::NodeProven`
(`src/proof_event.rs`); a background worker (`src/proof_tree/worker.rs`)
builds the tree under a memory budget; `finalize()` copies canonical subtrees
onto transpositions; `validate.rs` replay-validates; `binary.rs` serializes.
Offline reconstruction from a TT snapshot lives in `src/reconstruct/`.

**Why provari wants it:** provari's `Proofs` table is both the pruning store
and the output proof, and the search **stops when it fills**
(`proofs_full` → "the proofs filled their memory: give more --memory"). A new
engine can keep a small in-memory proof store for the facts the search needs
and build the output tree later, under its own budget. That removes a hard
memory cap on deep wins.

**Take:** the event protocol, the worker, the finalize/validate/serialize
pipeline, and the memory-budget contract.

### 4.2 Replay validator and independent checker

`src/proof_tree/validate.rs` replays every path from the root against the
rules layer. `src/proof_tree/export.rs` + `examples/provari_export.rs` write
provari's JSON-lines format; all 168 committed root-`Win` shards were accepted
by the pinned checker (168/168, no non-accepts).

**Take:** both. Every claimed win in the new engine must pass our validator
**and** the pinned checker. A checker disagreement is a veto, not a warning.

### 4.3 Durable proof corpus (optional, later)

`examples/proofdb*`: harvest (`frontier-walk`), incremental merge, whole-DB
replay verify, implied-flip analysis. Provari has per-proof `check` but no
corpus. This is a later milestone, not part of the search core.

### 4.4 Lazy zero-init TT allocation

`src/search/tt/alloc.rs` uses `alloc_zeroed`: the size label is a reservation
bound, pages become resident on first touch. Measured: mate-in-1 104.6 ms →
1.2 ms, 229 MB → 16 MB; easy KQ 103.7 ms → 1.7 ms. Provari allocates and
zeroes eagerly (`large()` → `resize(count, default)`), so `--memory` is actual
RSS and startup cost. Take the lazy allocator; it is small and safe.

### 4.5 Restart schedule + generation-aware replacement

`RestartSchedule::SHIPPED` (500 k child evals, geometric ×2) is load-bearing
for the tail: `dec31` 278×, `m23_white` unprovable in 20 s without a boundary.
It costs ~21 % on stress first-outcome at TT 256 and sign-flips at TT 64, so
it is an A/B, not a default. If adopted, pair it with our generation-aware TT
victim score `(live, solved, work, generation)`; provari's least-work policy
would evict the current generation's entries.

### 4.6 Salt-seeded portfolio (optional driver)

`examples/portfolio` measured 0.768× wall / 0.685× evals on `dec10`, and
solved `m20_white` (791 M evals) where solo returned nothing. Provari already
has `--seed` and `perturb`, so a K-racer take-first-decisive driver is cheap
to build. It is a wall-clock play (~3.9× CPU family-wide), not an efficiency
play; keep it outside the search core.

### 4.7 Benchmark and optimizer harness

`examples/benchmark --json` + `docs/spec/optimizer_interface.md` is a
deterministic evaluator contract. Provari's constants (`THREAT=4`, `QUIET=9`,
`HANGING=15`, `WEAK_THREAT=8`, `EVASIONS=8`, `SIMULATION=16`, `EF_A/EF_B`,
`MAGNITUDE=0.25`, hand-eval weights) are hand-set. Expose the new engine's
constants to an external optimizer; the move-class numbers alone are a 2.3×
ablation.

### 4.8 Test tiers and drift protocol

`make test` (fast gate), `make test-full` (slow tier), the drift protocol for
behaviour-changing levers, and the repetition soundness gate
(`tests/test_repetition.rs -- --include-ignored`). The new engine must adopt
the same discipline; correctness is the first quality attribute.

### 4.9 3-man atomic tablebase generator (optional)

`examples/egtb_gen3` is a prototype 3-man atomic WDL generator with solver
cross-validation and an independent depth-limited proof oracle. Only useful if
atomic Syzygy tables are unavailable; provari's tablebase path is already
external and optional. Do not make it a dependency.

---

## 5. What **not** to take

- The general Win/Loss/Draw search, repetition cache, GHI discipline, and
  refinement rounds. Provari deliberately does not need them.
- Our TT key including the halfmove clock. Provari's board-only key + proof
  length/limit is better.
- Our proof-tree binary format / proofdb schema as the internal format.
  Provari's compact `.pvr` is better for interchange; ours is better for
  validation and the DB. Use both, with a clear boundary.
- Our move ordering / `StaticAtomicScorer`. Provari's atomic-specific hand
  eval is better.
- Any provari code, binary, dependency, or model. Ideas only; GPL-3.0
  boundary; the oracle runs from a scratch directory and is never committed.

---

## 6. Proposed architecture

Keep the new engine behind its own binary/flag until it wins the bench. The
current solver remains the product.

```
src/search/prover/            (or a workspace crate if it grows)
  board.rs      bitboards + mailbox, 16-bit moves, Effect, child_base,
                play, null, hash; validated against atomic_movegen + shakmaty
  mover.rs      Mover precomputation; all child questions as bit tests
  eval.rs       hand evaluation (material + atomic attack terms)
  table.rs      4-slot clusters, prefetch, filter, generation-aware replacement
  proofs.rs     12-byte entries, shortest proof + length/limit
  search.rs     attacker-only DF-PN+: 1+ε, move classes, EF-PN, quiet
                grouping, static tactics, sibling simulation, optional restarts
  events.rs     ProofEvent emission (reuse src/proof_event.rs)
  cli.rs        --fen, --timeout, --memory, --guide hand|none, --proofs,
                --first-outcome, --seed, --most, --bench
```

Data flow:

```
root FEN -> prover search -> TT + proof facts -> ProofEvents
                                              -> proof_tree worker
                                              -> finalize/validate
                                              -> .pvr / JSONL export
                                              -> optional proofdb fold
```

**Board decision (open, see §9):** start with a `Mover`/`Effect` layer over
the existing `atomic_movegen` board and measure. Write a new provari-style
bitboard board only if the layer cannot avoid make/unmake in the hot path.
Correctness first: any new board is differential-tested against
`atomic_movegen` and shakmaty on random games, like provari's
`matches_the_played_positions`.

### 6.1 Repository and crate boundary (decided)

**Decision: same repository. Start as `src/search/prover/` behind its own
binary/flag; promote to a workspace crate only when a trigger fires. Do not
start a new repository.**

The rewrite is an engine rewrite, not a project rewrite. The evidence layer
and the measurement discipline are the assets that make the comparison
trustworthy, and they live here.

**Why not a new repository**

- The new prover needs `proof_event`, `proof_tree`, `validate`, `export`,
  `reconstruct`, `notation`, `zobrist`, the rules layer, the bench harness,
  the frozen corpus, the pinned provari oracle, and the proofdb pipeline. A
  separate repo either duplicates them (drift, double maintenance) or depends
  on `atomic_solver` across repos (versioning, two release profiles, awkward
  fallback).
- The current solver stays the product until the new one wins. Same repo lets
  the new engine sit behind a flag/binary and be compared in one build,
  against the same corpus and gates.
- The process is here: `docs/plans`, the session protocol, `bench.md`, the
  test tiers, the drift protocol, AGENTS.md. A new repo needs its own
  conventions and (since there is no CI) its own enforcement — pure overhead
  for a single-author project.
- GPL discipline is about boundaries, not repo topology. A separate repo does
  not reduce the risk of importing provari code; a clear module boundary plus
  the "ideas only, no code/binary/model" rule does.

**Why not a big-bang workspace split now**

- `docs/plans/workspace/plan1.md` is stale: it moves modules removed by
  `proofdb` plans 18/20 and misses current directories. Executing it now
  would be a packaging project before the engine exists.
- The repo already rejected splitting the solver lib into multiple crates
  (workspace item 2) because there is no isolation gain at this size. A new
  prover crate is a different case, but the same single-author overhead
  applies until it has a real consumer/test surface.
- `lto = true`, `codegen-units = 1` make module placement codegen-neutral, so
  there is no performance reason to split early.

**Promotion triggers** (then re-draft the stale workspace plan as its own
packaging plan, not as part of the rewrite):

- the new engine has its own test surface or a consumer that needs only it;
- it becomes the product and the old solver becomes a legacy binary;
- compile times or test tiers justify isolating it;
- you want the compiler to enforce the boundary (a crate boundary is stronger
  than a module boundary).

**When a separate repository would make sense:** only if the rewrite becomes a
separate product — different audience, different license, independent release
cadence, or a library consumed by external projects where the evidence layer
is irrelevant. None of those are true for "the fastest independently-checked
atomic win prover"; the evidence layer is the differentiator, not a side
artifact.

**Boundary rule (enforce from day one):** the prover may import only
`proof_event`, `proof_tree`, `notation`, `zobrist`, the rules/position layer,
and the bench/validation tooling — **not** `dfpn` internals. If the module
boundary is clean, promoting to a crate later is mechanical.

---

## 7. Milestones and gates

One mechanism per plan, per the working agreement. Each milestone has a stop
rule; a negative result is recorded, not hidden.

| Milestone | Scope | Gate |
| --- | --- | --- |
| **M0** | Skeleton: board (or a `Mover` layer over `atomic_movegen`) + legal movegen, attacker-only DF-PN+ with no heuristics, CLI, bench harness, differential tests | correctness green; solves a mate-in-1/2 and a small win; no performance claim |
| **M1** | Per-node cost: `Mover`/`Effect`, no make/unmake for most children, incremental hash, TT clusters + prefetch + filter, proof table with length/limit | per-node cost (wall per 1 M child evals) at or below the front-end's; a small reference win proved within a stated cap; no soundness regressions |
| **M2** | Search guidance: move classes, EF-PN, quiet grouping | `core` and `easy-win` proved within a stated cap; measure vs M1 |
| **M3** | Static tactics as child scoring + sibling simulation | prove `core`/`easy-win` at a product-affordable cap; measure |
| **M4** | Proof output: events, offline tree, finalize, validate, export, independent checker | every claimed win replay-validated and checker-accepted |
| **M5** | Head-to-head: 3 cells, 3 repeats, same-session provari hand reference | geomean < 1.0, coverage 3/3, no regressions; otherwise record and decide |

**Stop rule:** if M1 does not show a per-node improvement over the front-end,
the per-node diagnosis is wrong — stop and re-diagnose before building M2–M3.
If M3 cannot prove `core`/`easy-win` at a product-affordable cap, the
from-scratch path is not viable at this scope; record and close.

---

## 8. First plan session (recommended)

The first session is a **plan session**, not an execution session:

1. Read this handoff, [`report5.md`](report5.md), [`bench.md`](bench.md), and
   the pinned provari source (`src/moves.rs`, `src/board.rs`, `src/dfpn.rs`,
   `src/table.rs`, `src/proofs.rs`, `src/eval.rs`).
2. Confirm the code-location decision (§6.1) and decide the remaining open
   questions (§9).
3. Open the successor initiative (`docs/plans/prover/initiative.md`) with the
   goal, non-goals, working agreement, and the M0–M5 backlog.
4. Write `docs/plans/prover/plan1.md` for **M0** (skeleton + correctness +
   bench harness). M0 must include a per-node cost baseline so M1 can be
   measured against it.
5. Update `docs/plans/README.md` (new active row). The pointer from
   [`initiative.md`](initiative.md) §Next direction to this handoff is already
   in place.

Do not implement in the plan session. The execution session starts from
`plan1.md`.

---

## 9. Open decisions for the plan session

D1 is settled by §6.1; the rest are open.

| # | Decision | Options | Recommendation |
| --- | --- | --- | --- |
| D1 | Code location | **decided (§6.1):** `src/search/prover/` now, workspace crate when a trigger fires; not a new repository | Implement the module boundary from day one; re-draft the stale `workspace/plan1.md` only when promoting |
| D2 | Board | `atomic_movegen` + `Mover` layer vs new bitboard board | Start with the layer; write a new board only if the hot path cannot avoid make/unmake |
| D3 | Proof format | `.pvr` vs our proof-tree binary vs both | Our format internally (validation/DB), `.pvr`/JSONL as export for the independent checker |
| D4 | Rule50/repetition | Provari board-only key + length/limit vs our clock key | Provari's, validated by our repetition soundness gate |
| D5 | Restarts | Adopt vs A/B vs never | A/B after M2; pair with generation-aware replacement |
| D6 | Portfolio | In-core vs separate driver vs defer | Separate driver after M5; never in the search core |
| D7 | Tablebase | External optional vs none | External optional; not a dependency |
| D8 | Model/NNUE | Hand guide only vs stretch column | Hand guide only; no learned model in the product |

---

## 10. Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| New board/rules bugs | Differential tests against `atomic_movegen` and shakmaty; replay validator; independent checker veto |
| Per-node cost is not the real bottleneck | M1 gate; stop and re-diagnose if no improvement |
| Scope creep | One mechanism per plan; milestones with stop rules |
| GPL contamination | No provari code/binary/dependency/model in the repo; scratch clone only; ideas only |
| Measurement drift | Same-session ratios; `bench.md` reading rules; censored is a result |
| Soundness regression | Repetition/rule50/terminal tests; `make test` and `make test-full`; checker veto |
| Proof memory cap | Offline tree + worker budget; do not make the search's proof store the output tree |
| The new engine never beats provari | The current solver stays the product; the new engine stays opt-in or is retired with the measured record |

---

## 11. Source map

**Provari (pinned `48686bc`, scratch clone, never committed):**
`src/moves.rs` (`Mover`, `Effect`, static tactics), `src/board.rs` (16-bit
moves, `child_base`, `effect`, `play`), `src/dfpn.rs` (search loop, move
classes, EF-PN, quiet grouping, simulation, proofs), `src/table.rs` (clusters,
prefetch, filter), `src/proofs.rs` (12-byte proofs, length/limit),
`src/eval.rs` (hand eval), `src/check.rs` (independent checker),
`src/format.rs` + `src/walk.rs` (`.pvr`/JSONL, strategy map),
`src/tablebase.rs` (Syzygy atomic), `src/search.rs` (exhaustive baseline),
`src/main.rs` (CLI, bench, memory split).

**`atomic_solver` assets:** `src/proof_event.rs`, `src/proof_tree/`
(`worker.rs`, `validate.rs`, `binary.rs`, `export.rs`), `src/reconstruct/`,
`src/search/dfpn/` (general solver, restart schedule, budget semantics),
`src/search/winprover/` (the prototype and its measured limits),
`src/search/preflight/` (region closure + replay verifier),
`src/search/tt/` (`alloc.rs` lazy alloc, generations), `src/notation.rs`,
`src/zobrist.rs`, `src/main.rs`, `src/cli.rs`, `examples/benchmark.rs`,
`examples/portfolio.rs`, `examples/egtb_gen3.rs`, `examples/proofdb*`,
`docs/spec/optimizer_interface.md`.

**Reports to read:** [`report3.md`](report3.md) (front-end skeleton),
[`report4.md`](report4.md) (sibling simulation), [`report5.md`](report5.md)
(close-out and probes), [`bench.md`](bench.md) (ledger and rules),
[`../provari/report1.md`](../provari/report1.md) … `report5.md` (levers),
[`../performance/report2.md`](../performance/report2.md) (lazy TT),
[`../parallel/report8.md`](../parallel/report8.md) (portfolio),
[`../conversion/report12.md`](../conversion/report12.md) (restart schedule).

---

## 12. Kickoff prompt (copy-pasteable)

> Open the successor initiative to `winprover` (working name `prover`). Read
> `docs/plans/winprover/HANDOFF.md`, `docs/plans/winprover/report5.md`,
> `docs/plans/winprover/bench.md`, and the pinned provari source at commit
> `48686bc6c786d6ea70568f0b2ad4002cf0520206` (scratch clone; do not commit
> it). This is a plan session: the code-location decision is settled in
> HANDOFF §6.1; decide the remaining open questions in §9, write
> `docs/plans/prover/initiative.md` and `docs/plans/prover/plan1.md` for
> milestone M0 (skeleton + correctness + bench harness, with a per-node cost
> baseline), and update `docs/plans/README.md`. Do not implement yet.
