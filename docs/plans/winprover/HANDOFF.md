# Handoff — from-scratch provari-parity atomic win prover

**Status:** successor handoff for the closed [`winprover`](initiative.md)
initiative. This is **not** a plan and **not** a report. It is the
self-contained starting point for the session that bootstraps the successor
project and writes its first plan.

| Item | Value |
| --- | --- |
| Date | 2026-11-02 (UTC), **revised 2026-11-06** (see [§0](#0-revision-2026-11-06)) |
| From | the `winprover` close-out ([`report5.md`](report5.md)) |
| Pinned oracle | `aiorla/provari` commit `48686bc6c786d6ea70568f0b2ad4002cf0520206` (`v0.1.0`) |
| Legacy repository | <https://github.com/golmman/atomic_solver> (`git@github.com:golmman/atomic_solver.git`); this file is `docs/plans/winprover/HANDOFF.md` on `main` |
| Legacy product | `atomic_solver` at `5f85133ea9d751368a682e53e354bd3fc0016d00` (product code as of `c7c9bca`, `atomic-movegen` 2.4.0); **unchanged** by this file |
| Rules crate | `atomic-movegen` 2.4.0 (crates.io, MIT) |
| Successor | a **new repository** (working name `prover`; the owner picks the final name), not a `winprover` plan and not a module of this repo |

The recommendation is a **from-scratch attacker-only win prover in a new
repository**. It takes provari's search core as ideas and this repo's
evidence/measurement discipline as specs, fixtures, and (own-licence) code to
copy with provenance. The current general solver stays the product until the
new prover wins the bench. Nothing in this handoff changes product behaviour.

---

## 0. Revision 2026-11-06

The 2026-11-02 version recommended the same repository (§6.1 "decided"). This
revision reverses that and brings the facts up to date:

| # | Change | Why |
| --- | --- | --- |
| R1 | **Code location: new repository** (§6.1, D1) | The owner's coupling/deprecation/drift concerns hold up against a dependency audit (§6.1). The 2.4.0 upgrade moved most of the "rules layer" the prover needed from this repo into the upstream crate. |
| R2 | **Evidence: fresh minimal + external** (§4.1–4.2, D9) | The new engine writes provari JSONL itself and replay-validates with its own small attacker-only validator. `provari check` is the veto. This repo's validator is only an optional offline cross-check through a file, never a code dependency. |
| R3 | **Bench: freeze + fresh** (§2, D10) | This repo's [`bench.md`](bench.md) is frozen as the `winprover` record. The successor opens its own `bench.md` with the same protocol, cells and corpus, and a re-measured post-2.4.0 baseline. |
| R4 | `atomic-movegen` 2.4.0 adopted here on 2026-11-04 (§3.2, D2, D4) | It changes rules behaviour (touching-commoner check immunity, mate-before-clock, terminal-board movegen, self-explosion) and adds the APIs a `Mover` layer builds on. Every number in §1 predates it. |
| R5 | Proof output folded into M0 (§7) | Correctness first: no milestone claims a proved win before the exporter + checker veto exists. |
| R6 | Item 8 (evidence horizon) recorded (§9, horizon) | [`initiative.md`](initiative.md) carries items 7 and 8 to the successor; the first version dropped item 8. Item 7 (workspace split) is moot in a new repo. |
| R7 | Legacy gate state recorded (§10) | `make test-full` is currently red on two pre-existing dec02 wall-clock slow tests ([`../movegen/report_update_2.4.0.md`](../movegen/report_update_2.4.0.md) §Gates). |

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

All cells at the canonical 10 s / 192 MB, same session, provari hand = 1.000,
`solver@35f78bf` (`atomic-movegen` 2.3.1). The opt stack is
`--move-class-pn --quiet-grouping --eval-pn or`. It is **not** promoted
because it turns three quick-suite `Loss` cases (`dec06`, `dec42`, `dec44`)
into 3 s draws.

**These numbers predate the 2.4.0 rules fixes.** The fixes moved search
trajectories: the `dec10` portfolio anchor went from 3,901,847 to 2,717,601
evals. Treat the table as the record of the closed initiative, not as the
successor's baseline (§2).

The remaining gap is not another lever. The plan5 probes found:

- **Candidate/verification pivot**: 0/41 and 0/38 root moves proved at 1 M
  evals each, so there is no affordable candidate source.
- **Rule50-free key**: 0–0.7 % and no coverage change. This is the measured
  upper bound, because the prototype was deliberately unsound.
- **Front-end heuristic parity**: the opt starting numbers do not fix the
  front-end. It proves `core` only at ~30.9 M evals / ~15 s and cannot prove
  `easy-win` at 50 M. The general solver needs ~13.8 M evals / ~3.8 s on
  `core`; the eval count is from the probe configuration in report5 §4.3, the
  wall from the bench.

The opt-in front-end (`src/search/winprover/`) is the right **shape** but the
wrong **engine**. Its child evaluation is a full `Position::outcome()` movegen
per child, and it inherits general-game machinery a win-only prover does not
need. That is the architectural gap.

**What we learned, in one sentence:** the portable ideas were real and bought
~8× on the shared cells (51.47 → 6.30), but the remaining ~6× is per-node
cost and search efficiency in the engine core, not a missing flag.

---

## 2. Target and protocol (do not silently change)

**Primary column:** `provari prove --guide hand`. The `--guide model` column
is a recorded stretch column only; no learned model is a product dependency.

**Bench cells** (unchanged from [`bench.md`](bench.md)):

| Cell | Id | FEN / source |
| --- | --- | --- |
| `core` | — (fingerprint `8d0ff8bca7f0`) | `r1b1kbnr/ppp3pp/n4q2/3pppN1/1P1PP3/8/P1P2PPP/RNBQKB1R w KQkq - 0 1` |
| `easy-win` | `1eb80b92f92c` | `rnbq1b1r/pp1p1kpp/2p1pp1n/8/3PP1P1/2N2N2/PPP2P1P/R1BQKB1R w KQ - 0 1` |
| `deep-win` | `db6a741c723f` | `rnb1kbnr/pppp2pp/5q2/4ppN1/3P4/8/PPP1PPPP/RNBQKB1R w KQkq - 0 1` |

**Corpus:** `docs/plans/provari/measurements/plan1/corpus.json`, sha256
`9cd0169a30bb7d5b162b6a1ec119fe194b776c8a24a46cdf1582c07062847d0e`. Copy it
into the new repo verbatim; the sha256 is the identity.

**Budget:** 10 s and 192 MB label for both sides, and three repeats per cell
and side for any reported wall ratio. Provari runs
`prove --seconds 10 --memory 192 --guide hand`. The new engine runs at the
same wall/memory budget with its own flags (fixed in its M0 plan).

**Success condition:** geometric-mean wall `prover / provari hand < 1.0` on
the shared-win set, coverage at least provari's (3/3 on these cells), every
claimed win accepted by the new repo's own replay validator **and** the
pinned provari checker, and the new repo's gates green. "No outcome
regressions" is checked on the copied fixture corpus (§4.10), not on the
legacy solver's gates.

**Reading rules:** same session, same host; wall is the product metric,
child evals / expansions the mechanism metric; censored is a result; opt-in
is not shipped. See [`bench.md`](bench.md) §Reading rules. Copy them
verbatim into the successor's `bench.md`.

**Bench ledger (decided, D10):**

- This repo's [`bench.md`](bench.md) is **frozen** as the `winprover` record:
  no new rows, and a closing "superseded by `<new repo>/bench.md`" note.
- The successor's `bench.md` reuses the protocol, cells, corpus, oracle pin,
  and reading rules. Its first row is a **re-measured baseline**: provari hand
  (1.000), plus the legacy `atomic_solver` `default` and `opt` stacks at the
  legacy pin above as same-session control rows. This re-prices the gap after
  the 2.4.0 rules change.
- Every successor plan that touches the prover's search appends one row for
  the new engine; the legacy control rows are re-measured only when a
  host/drift question needs them.

---

## 3. What to take from provari (the search core)

Read the pinned source in a scratch clone and never commit it. GPL-3.0: take
ideas only, no code, binary, dependency, or model. The mechanisms, in the
order they matter:

### 3.1 Attacker-only value semantics

Provari proves exactly two things: **attacker wins** or **refuted**. There is
no draw value to prove, no loss proof, no general outcome, and no
PV/refinement machinery inside the search. A refuted node is
`disproof = 0`, carrying the earliest repeated position on the line and
whether the 50-move limit is involved (`Known::Refuted { cycle, from }`).
This is a smaller problem than a general Win/Loss/Draw search, and it relaxes
caching: a proof's length shrinks along the line, so a proof cannot repeat a
position and can be reused across paths.

**Take:** the value type and the two-outcome search. Do not add a draw proof.

### 3.2 Per-position precomputation + effects (the measured bottleneck)

For each expanded position, provari builds one `Mover` (`src/moves.rs`):
pins, check squares by role, uncovering pieces, attack maps, explosion zones
beside each king, cheap-blast targets. Every child question is then a bit
test on that context: `legal`, `explodes_them`, `exposes`, `gives_check`,
`threats`, `plain`, `blasted_cheaply`, `connectable`, `might_end_material`.

The child hash comes incrementally from `child_base() ^ effect()`. `Effect`
carries the child hash, occupancy, capture flag, and 50-move counter. The TT
probe/prefetch happens **before** the child is materialized. `pos.play(m)` is
only called for the minority that needs it: check replies, tablebase,
`cannot_win`, and mate verification.

**Take:** this layer first. It is the difference between
O(children × child movegen) and O(1 position precompute + children × bit
tests). The legacy front-end's full `Position::outcome()` per child is the
measured cost.

**2.4.0 building blocks** (verified in the crate source, `src/board.rs` /
`src/movegen.rs`). The layer can start on these before any new board is
considered:

| Need | `atomic-movegen` 2.4.0 |
| --- | --- |
| non-mutating child key | `Board::hash_after(m)`, `hash_and_rule50_after(m)` (key + clock in one classification), `hash_after_with(m, EnPassantMode)` |
| single-move legality without a move list | `Board::is_legal`, `is_legal_with_state` |
| terminal precedence (one rules definition) | `Board::outcome_with(state, moves_exist)`, `outcome_with_state`, `is_material_draw` |
| cheap existence query | `movegen::has_legal_move[_with_state]` |
| blast geometry | `Board::blast_zone(sq)`, `Board::blast_casualties(m)` |
| attack queries | `Board::attackers_to_color`, `attacks::attacks_from` |
| ep normalization for keys | `Board::legal_ep_square`, `EnPassantMode::{Always, Legal}` |
| wire/UCI | `Move::to_bits`/`from_bits` (stable 16-bit), `Board::move_from_uci` |
| validated input | `Board::validate`, `from_fen_with`, `FenPolicy` |
| make/unmake when needed | `Board::do_move(m, &mut StateInfo)` |
| optional perf | `unsafe-optimizations` feature (off unless a controlled measurement justifies it) |

Nothing in 2.4.0 is a provari-style `Effect` (child occupancy + capture flag +
clock as one value) or a per-node `Mover`; those are the prover's own layer.
If a missing primitive blocks the hot path, follow the existing upstream
pattern ([`../movegen/feedback2.md`](../movegen/feedback2.md) →
[`../movegen/report_update_2.4.0.md`](../movegen/report_update_2.4.0.md)): ask
upstream before forking a board.

### 3.3 Static tactics as child scoring, not node-entry short-circuits

Provari settles these **per child during scoring**, setting both the value
and the starting proof numbers: immediate explosion, exposed king, a check
with no safe evasion, a double threat no defence parries, cannot-win
material, mate in one, and tablebase. This feeds DF-PN's ordering instead of
bypassing it. Our `--static-tactics` node-entry closure measured
neutral/worse for exactly this reason.

**Take:** the per-child form. Do not repeat the node-entry closure.

### 3.4 Search guidance from the first expansion

- **Move-class starting numbers:** checks start at the number of safe evasions
  (capped at 8), double threats at 1, threats at 4, weak threats at 8, quiet
  moves at 9, hanging quiet moves at 15.
- **EF-PN scaling:** a hand evaluation produces a step that scales every fresh
  child's proof/disproof numbers (`scale`).
- **Quiet grouping:** all plain attacker moves become one delayed child
  (`proof = QUIET`, `disproof = count`), opened only when selected.
- **1+ε:** `widen(second) = second + 1 + second/8` (ε = 1/8).

The provari ablations are multiplicative: flattening the starting numbers is
**2.3×** slower, disabling quiet grouping **1.4×**, removing the guide
**1.8×**, roughly **5.8×** combined. Our ports captured most of that. The new
engine should start with all of it, not rediscover it.

### 3.5 Sibling simulation

At an AND node, when a defence is refuted, provari replays its proof on the
open siblings (`simulate`, `simulate_defences`, `replay_threat`). Our
first-form replay works (12–27 % of attempts transfer a proof, frames cut
~20–24 %) but did not close a root at a product cap. In the new engine it is
a first-class part of the AND-node loop, not an add-on.

### 3.6 Memory and table engineering

- 16-bit moves, `MaybeUninit` move lists, per-depth child buffers.
- TT: 4-slot clusters sharing a cache line, prefetch, a Bloom-style filter
  that is dropped when it stops paying, least-work replacement.
- Proofs: a separate open-addressing table with 12-byte entries (key + move
  15 bits + length 10 + limit 7). It keeps the shortest proof plus longer ones
  at higher counters, and drops mates in one when full.
- Board-only key + proof length/limit makes cross-clock proof reuse sound.

### 3.7 Proof output and independent checking

Provari writes a compact `.pvr` (zstd) or JSON-lines proof: a strategy map
keyed by board hash, with the winning move per attacker position.
`provari check` re-walks it using shakmaty only, sharing no code with the
prover.

**Take:** the checker as the independent oracle, and the JSONL strategy map as
the new engine's **first** output format (it is what the checker reads). The
internal proof store stays separate from the output (§4.1).

---

## 4. What to take from `atomic_solver` (ideas, specs, fixtures — not a dependency)

This is the part provari does not have, and the reason a from-scratch engine
is not a from-zero project. In a new repository, "take" means one of three
things. Mark each item in the successor's `initiative.md`:

- **idea/spec**: re-implement from the description and the cited module docs;
- **copy**: copy own MIT-licensed code or data with a provenance line
  (`from atomic_solver@<sha>:<path>`), then own it;
- **external**: use the legacy binary as a separate process, never as a crate
  dependency.

**Dependency audit** (2026-11-06, why "reuse the module" is not free).
`src/proof_tree/` is written against this repo's `Position` and its
three-valued side-to-move `Outcome` (`validate.rs`, `export.rs`, `node.rs`,
`worker.rs`, `binary.rs`). Its tests construct `search::dfpn::Search`.
`src/reconstruct/` reads the `dfpn` TT snapshot. The 16-bit codec and UCI
parsing in `src/notation.rs` are now thin wrappers over 2.4.0 upstream APIs.
The handoff already rejects our clock-including Zobrist key (§5). So the
evidence layer cannot be imported cleanly; its **design** is the asset.

### 4.1 Events + budgeted offline proof output (idea/spec)

Our search never builds the proof tree. It emits `ProofEvent::NodeProven`
(`src/proof_event.rs`); a background worker (`src/proof_tree/worker.rs`)
builds the tree under a memory budget; `finalize()` copies canonical subtrees
onto transpositions; `validate.rs` replay-validates; `binary.rs` serializes.

**Why it matters against provari:** provari's `Proofs` table is both the
pruning store and the output proof, and the search **stops when it fills**
(`proofs_full` → "the proofs filled their memory: give more --memory"). A new
engine can keep a small in-memory proof store for the facts the search needs
and build the output later, under its own budget. That removes a hard memory
cap on deep wins.

**Take (spec):** the event-protocol shape (search depends only on a neutral
event type, never on the output builder), the bounded worker, and the
memory-budget contract. Re-implement them for an attacker-only strategy map
(winning move per attacker position, all replies per defender position).
Not the general Win/Loss/Draw tree.

### 4.2 Replay validator and independent checker (decided, D9)

- **Own validator (re-implemented, small):** replay every path of the emitted
  strategy from the root against `atomic-movegen`. At every attacker node the
  stored move must be legal; at every defender node every legal reply must be
  covered; every leaf must be an attacker win by `Board::outcome_with`; and
  repetition/rule50 must never be counted as a win. Spec references:
  `src/proof_tree/validate.rs` (module doc) and
  `src/proof_tree/export.rs` (the JSONL shape provari accepts; 168/168
  committed root-`Win` shards accepted by the pinned checker).
- **Independent checker (external, veto):** `provari check` from the scratch
  build. A checker disagreement is a veto, not a warning. Classify
  non-accepts as in `winprover`'s working agreement 6.
- **Legacy cross-check (optional, external):** this repo's validator is
  reachable only through a file. It would need a JSONL → `proof_tree.bin`
  importer here, which is a horizon item for this repo (§9, horizon), not a
  successor dependency.

### 4.3 Durable proof corpus (later, horizon)

`examples/proofdb*`: harvest (`frontier-walk`), incremental merge, whole-DB
replay verification, implied-flip analysis. Provari has per-proof `check` but
no corpus. This is not part of the search core. If the successor wants it, it
gets its own plan after M5 and its own schema; this repo's
`docs/spec/global_proof_store.md` is the reference.

### 4.4 Lazy zero-init TT allocation (copy)

`src/search/tt/alloc.rs` uses `alloc_zeroed`: the size label is a reservation
bound and pages become resident on first touch. Measured: mate-in-1
104.6 ms → 1.2 ms and 229 MB → 16 MB; easy KQ 103.7 ms → 1.7 ms. Provari
allocates and zeroes eagerly (`large()` → `resize(count, default)`), so its
`--memory` is actual RSS and startup cost. Copy the allocator; it is small
and safe. Its invariant ("`valid` is the only liveness signal") goes with it.

### 4.5 Restart schedule + generation-aware replacement (idea, A/B)

`RestartSchedule::SHIPPED` (500 k child evals, geometric ×2) is load-bearing
for the tail: `dec31` 278×, and `m23_white` is unprovable in 20 s without a
boundary. It costs ~21 % on stress first-outcome at TT 256 and sign-flips at
TT 64, so it is an A/B, not a default. If adopted, pair it with our
generation-aware TT victim score `(live, solved, work, generation)`;
provari's least-work policy would evict the current generation's entries.

### 4.6 Salt-seeded portfolio (idea, optional driver)

`examples/portfolio` measured 0.768× wall / 0.685× evals on `dec10`, and
solved `m20_white` (791 M evals) where solo returned nothing. Provari already
has `--seed` and `perturb`, so a K-racer take-first-decisive driver is cheap
to build. It is a wall-clock play (~3.9× CPU family-wide), not an efficiency
play; keep it outside the search core.

### 4.7 Benchmark and optimizer harness (copy + adapt)

- **Bench driver:** copy
  [`measurements/bench/run_bench.py`](measurements/bench/run_bench.py). It
  already drives both sides as external binaries by path; only the candidate
  output parser (`evals:` / outcome lines) needs adapting to the new CLI. The
  legacy control rows keep the legacy parser.
- **Optimizer contract:** `examples/benchmark --json` +
  `docs/spec/optimizer_interface.md` is a deterministic evaluator contract.
  Provari's constants are hand-set: `THREAT=4`, `QUIET=9`, `HANGING=15`,
  `WEAK_THREAT=8`, `EVASIONS=8`, `SIMULATION=16`, `EF_A/EF_B`,
  `MAGNITUDE=0.25`, and the hand-eval weights. Expose the new engine's
  constants the same way; the move-class numbers alone are a 2.3× ablation.

### 4.8 Test tiers and drift protocol (idea/spec)

Adopt the discipline, not the files: a fast gate (`make test`), a slow tier
(`make test-full`), a drift protocol for behaviour-changing levers, and a
repetition/rule50 soundness gate (this repo:
`tests/test_repetition.rs -- --include-ignored`). Correctness is the first
quality attribute. Keep the new repo's `AGENTS.md` small; this repo's is
~30 KB and is truncated in agent context.

### 4.9 3-man atomic tablebase generator (optional, external)

`examples/egtb_gen3` is a prototype 3-man atomic WDL generator with solver
cross-validation and an independent depth-limited proof oracle. It is only
useful if atomic Syzygy tables are unavailable; provari's tablebase path is
already external and optional. Do not make it a dependency.

### 4.10 Fixtures to copy (copy, pinned)

Copy test data, not test code, each with a provenance line naming
`atomic_solver@5f85133` and the source path:

- `docs/plans/provari/measurements/plan1/corpus.json` (sha256 above);
- the three bench FENs and their provari reference values;
- **known decisive wins** from `tests/fixtures/decisive_positions.txt`,
  `decisive_remaining.txt` and the `examples/benchmark` quick suite, filtered
  to wins for the side to move. These are the outcome oracle: a refutation
  there is a regression to investigate, not a result;
- terminal/rules fixtures (`tests/test_terminal_outcomes.rs`,
  `tests/test_position.rs`) as FEN + expected-classification tables, all
  re-checked against 2.4.0 semantics;
- repetition/rule50 soundness fixtures (`tests/test_repetition.rs`,
  `tests/test_ghi.rs`), restated for attacker-only semantics (repetition and
  rule50 are refutations, never wins).

---

## 5. What **not** to take

- The general Win/Loss/Draw search, repetition cache, GHI discipline, and
  refinement rounds. Provari deliberately does not need them.
- Our TT key including the halfmove clock. Provari's board-only key + proof
  length/limit is better.
- Our proof-tree binary format, the proofdb schema, and the three-valued
  `Position`/`Outcome` wrapper. The new engine speaks attacker-only strategy
  maps; JSONL/`.pvr` is its interchange format.
- Our move ordering / `StaticAtomicScorer`. Provari's atomic-specific hand
  eval is better.
- **Any crate dependency on `atomic_solver`** (path, git, or published). Use
  copy-with-provenance or the legacy binary as an external process (§4).
- Any provari code, binary, dependency, or model. Ideas only, because of the
  GPL-3.0 boundary; the oracle runs from a scratch directory and is never
  committed.

---

## 6. Proposed architecture

The new engine lives in its own repository. `atomic_solver` stays the product
until the new prover wins the bench (§2) and the owner decides to switch.

```
prover/                       (new repo; working name)
  Cargo.toml    deps: atomic-movegen = "2.4" (+ shakmaty as dev-dependency
                for differential tests only); release profile lto/cgu=1
  AGENTS.md     small: goal, architecture, gates, GPL boundary, provenance rule
  Makefile      test (fast gate) / test-full (slow tier) / bench
  src/
    mover.rs    per-node precomputation over atomic_movegen::Board;
                all child questions as bit tests; Effect = child key/clock/
                capture/occupancy without make/unmake
    eval.rs     hand evaluation (material + atomic attack terms)
    table.rs    4-slot clusters, prefetch, filter, lazy zeroed alloc,
                generation-aware replacement
    proofs.rs   in-search proof store: 12-byte entries, shortest proof +
                length/limit (pruning facts only, not the output)
    search.rs   attacker-only DF-PN+: 1+ε, move classes, EF-PN, quiet
                grouping, static tactics, sibling simulation, optional restarts
    events.rs   neutral proof-event type; search depends only on this
    strategy/   bounded worker -> strategy map -> validate -> JSONL / .pvr
    main.rs     --fen, --seconds/--timeout, --memory, --guide hand|none,
                --proof-out, --seed, --bench
  bench/        corpus.json (pinned), run_bench.py (adapted), results
  docs/plans/   the successor initiative, plans, reports, bench.md
  tests/        differential (vs shakmaty), fixtures (copied), soundness
```

Data flow:

```
root FEN -> prover search -> TT + in-search proof store
                          -> proof events -> bounded strategy worker
                          -> own replay validator -> JSONL (.pvr later)
                          -> provari check (external veto)
                          -> [optional] atomic_solver cross-check via file
```

**Board decision (D2):** start with the `Mover`/`Effect` layer over
`atomic_movegen::Board` and the 2.4.0 primitives (§3.2), and measure. Write a
new provari-style bitboard board only if the layer cannot avoid make/unmake
in the hot path. Any new board is differential-tested against
`atomic_movegen` and shakmaty on random games, like provari's
`matches_the_played_positions`.

### 6.1 Repository boundary (decided 2026-11-06: new repository)

**Decision: start a new repository. `atomic_solver` is frozen as the legacy
product, the `winprover` measurement record, and an optional external
cross-check. The successor does not depend on it as a crate.**

This supersedes the 2026-11-02 decision ("same repository,
`src/search/prover/`, promote to a crate on triggers"). That record is kept
below so the reversal is explicit.

**Why a new repository**

- **Coupling is real, not hypothetical.** The dependency audit (§4) shows the
  evidence layer is built on the general solver's `Position` and three-valued
  `Outcome`, with tests against `dfpn::Search` and a reconstruct path through
  the `dfpn` TT snapshot. Reusing it in-repo would either generalize legacy
  modules for a new consumer (drift into both) or wrap them (coupling). Both
  make the eventual deprecation harder, which is the owner's concern.
- **The 2.4.0 upgrade removed the main reuse argument.** The 2026-11-02 case
  listed "notation, zobrist, the rules layer" as shared needs. Since
  2026-11-04, terminal classification, the 16-bit codec, UCI parsing,
  single-move legality, child key+clock, and blast/attack helpers are
  upstream `atomic-movegen` APIs that any repo can use. The rejected clock key
  was never a reason to share `zobrist`.
- **The comparison needs no shared build.** The bench driver already runs
  both engines as external binaries by path; the corpus is one sha256-pinned
  file; the veto is an external binary. Cross-repo measurement costs nothing
  extra.
- **Context and convention load.** This repo carries 26 initiatives, ~625
  Markdown files, a ~30 KB `AGENTS.md`, and conventions (drift protocol over
  general outcomes, GHI discipline, refinement, the "RAM = TT only" CLI
  contract) that do not apply to an attacker-only engine. In-repo, every
  session pays that load and risks applying legacy rules to the new engine.
- **Gates stay independent.** The legacy slow tier currently has two red
  dec02 tests (§10). A new engine should not inherit an unrelated red gate or
  slow tiers it does not touch.

**What the 2026-11-02 case got right, and how it is kept**

| Same-repo argument | Kept by |
| --- | --- |
| The evidence layer is the differentiator | Re-implemented as an attacker-only spec (§4.1–4.2); provari checker veto from M0 |
| One corpus and one oracle protocol | Corpus copied by sha256; protocol and reading rules copied verbatim; oracle pin unchanged |
| The process (plans/reports, gates, drift, AGENTS.md) | Adopted as discipline in the new repo (§4.8), with a small `AGENTS.md` |
| GPL discipline is about boundaries | Unchanged: ideas only, scratch clone, never committed, in either repo |
| No duplicated maintenance | Copies are of frozen legacy code/data with provenance lines; the legacy repo does not evolve them further for the successor |

**Superseded record (2026-11-02):** same repository, `src/search/prover/`
behind its own binary/flag, a boundary rule (prover imports only
`proof_event`, `proof_tree`, `notation`, `zobrist`, rules/position, and
bench/validation tooling), and promotion to a workspace crate on triggers.
Item 7 (workspace split, stale `workspace/plan1.md`) is therefore moot for
the successor; in this repo it stays unranked/stale.

---

## 7. Milestones and gates

One mechanism per plan, per the working agreement. Each milestone has a stop
rule, and a negative result is recorded, not hidden.

| Milestone | Scope | Gate |
| --- | --- | --- |
| **M0** | Repo bootstrap (Cargo, small `AGENTS.md`, Makefile gates, provenance rule, scratch-provari build script); copied corpus/fixtures; `Mover`-less baseline: legal movegen over `atomic_movegen`, attacker-only DF-PN+ with no heuristics, CLI; **minimal proof output**: strategy JSONL + own replay validator + `provari check` wired into the gate; bench driver adapted; successor `bench.md` with the re-measured baseline row; per-node cost baseline | gates green; solves a mate-in-1/2 and a small win, each validator- **and** checker-accepted; per-node baseline recorded; no performance claim |
| **M1** | Per-node cost: `Mover`/`Effect`, no make/unmake for most children, incremental keys, TT clusters + prefetch + filter + lazy alloc, in-search proof store with length/limit | wall per 1 M child evals at or below the M0 baseline **and** the legacy front-end's (measured cross-repo, same session); a small reference win proved within a stated cap and checker-accepted; soundness fixtures green |
| **M2** | Search guidance: move classes, EF-PN, quiet grouping | `core` and `easy-win` proved within a stated cap; measured vs M1 |
| **M3** | Static tactics as child scoring + sibling simulation | `core`/`easy-win` proved at a product-affordable cap; measured |
| **M4** | Scaled proof output: event-fed bounded strategy worker (no proof-memory stop), `.pvr` export | every claimed win replay-validated and checker-accepted at deep-win scale; the worker budget is honoured |
| **M5** | Head-to-head: 3 cells, 3 repeats, same-session provari hand reference + legacy control rows | geomean < 1.0, coverage 3/3, no fixture-oracle regressions; otherwise record and decide |

**Per-node metric (decide in the M0 plan):** define the new engine's
child-eval counter to match legacy `child_evals` semantics (one per child
classified during an expansion), and also report expansions. This keeps both
the cross-repo front-end comparison (M1) and provari's `nodes` diagnostic
meaningful.

**Stop rule:** if M1 does not show a per-node improvement over the legacy
front-end, the per-node diagnosis is wrong; stop and re-diagnose before
building M2–M3. If M3 cannot prove `core`/`easy-win` at a product-affordable
cap, the from-scratch path is not viable at this scope; record and close.

---

## 8. Next steps

### 8.1 In this repository (close-out housekeeping, no product change)

Steps 1–4 are **done** (2026-11-06).

1. ~~Commit this revised handoff.~~ Done (`606604a`).
2. ~~Freeze [`bench.md`](bench.md).~~ Done: frozen note at the top plus a
   History entry; no row changed.
3. ~~Update [`initiative.md`](initiative.md) §Next direction.~~ Done: revision
   paragraph (new repository, item 7 moot for the successor, item 8 carried)
   plus a History entry.
4. ~~Update [`../README.md`](../README.md).~~ Done: the `winprover` and
   `workspace` rows (Closed), plus a matching note in
   [`../workspace/initiative.md`](../workspace/initiative.md).
5. Optional, separate: triage the two red dec02 slow tests
   ([`../movegen/report_update_2.4.0.md`](../movegen/report_update_2.4.0.md)
   §Next steps 1) so the legacy reference gate is green again.

### 8.2 First session in the new repository (plan session)

The owner creates the empty repository. The first session there is a **plan
session**, not an execution session:

1. Clone <https://github.com/golmman/atomic_solver> as a **read-only
   reference** into a sibling/scratch directory, never as a submodule or
   dependency. Read this handoff from `main` (legacy code pin `5f85133`),
   [`report5.md`](report5.md), [`bench.md`](bench.md),
   [`../movegen/report_update_2.4.0.md`](../movegen/report_update_2.4.0.md),
   and the pinned provari source (`src/moves.rs`, `src/board.rs`,
   `src/dfpn.rs`, `src/table.rs`, `src/proofs.rs`, `src/eval.rs`,
   `src/check.rs`, `src/format.rs`).
2. Decide the remaining open questions (§9).
3. Write `docs/plans/<initiative>/initiative.md` with the goal, non-goals
   (GPL boundary; no `atomic_solver` crate dependency; no learned model), the
   working agreement (adapted from `winprover`'s), and the M0–M5 backlog plus
   the horizon list.
4. Write `plan1.md` for **M0** (bootstrap + correctness + proof output +
   bench + per-node baseline).
5. Write a small `AGENTS.md` and the provenance rule for copied files.

Do not implement in the plan session. The execution session starts from
`plan1.md`.

---

## 9. Open decisions for the plan session

D1, D9 and D10 are settled (2026-11-06); the rest are open.

| # | Decision | Options | Recommendation |
| --- | --- | --- | --- |
| D1 | Code location | **decided (§6.1):** new repository; `atomic_solver` frozen as legacy/record/external cross-check | — |
| D2 | Board | `atomic_movegen` 2.4.0 + `Mover` layer vs new bitboard board | Start with the layer on the 2.4.0 primitives (§3.2); ask upstream for a missing primitive before forking a board |
| D3 | Proof format | strategy JSONL vs `.pvr` vs a native format | JSONL first (checker-readable, M0); `.pvr` in M4; no `proof_tree.bin` |
| D4 | Rule50/repetition key | provari board-only key + length/limit vs a clock key; ep mode `Always` vs `Legal` | Board-only key + length/limit from day one, validated by the copied soundness fixtures. Measure `EnPassantMode::Legal` as an M1 A/B: the new repo has no legacy snapshot compatibility to keep |
| D5 | Restarts | adopt vs A/B vs never | A/B after M2; pair with generation-aware replacement |
| D6 | Portfolio | in-core vs separate driver vs defer | Separate driver after M5; never in the search core |
| D7 | Tablebase | external optional vs none | External optional; not a dependency |
| D8 | Model/NNUE | hand guide only vs stretch column | Hand guide only; no learned model in the product |
| D9 | Evidence | **decided (§4.2):** own minimal attacker-only validator + `provari check` veto; legacy validator only via a file | — |
| D10 | Bench ledger | **decided (§2):** freeze `winprover/bench.md`; successor `bench.md` with a re-measured post-2.4.0 baseline and legacy control rows | — |
| D11 | Per-node counter | legacy-compatible child evals vs expansions only | Both: legacy-compatible child evals for M1, expansions for the provari diagnostic |

**Horizon (carried `winprover` item 8; unranked, after M5):** compact `.pvr`
(M4 covers export); a portable attacker-only export for proofdb transport
(needs the legacy JSONL → tree importer in this repo, see §4.2); the external
`high200` corpus as an out-of-distribution guard; a native second rules
implementation; cannot-win material (part of M3's static tactics). Root-`Loss`
coverage is out of scope for an attacker-only engine unless it grows a
defender-side mode.

---

## 10. Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| New board/rules bugs | Differential tests against `atomic_movegen` and shakmaty; own replay validator; independent checker veto from M0 |
| Per-node cost is not the real bottleneck | M1 gate; stop and re-diagnose if no improvement |
| Scope creep | One mechanism per plan; milestones with stop rules |
| GPL contamination | No provari code/binary/dependency/model in either repo; scratch clone only; ideas only |
| Measurement drift | Same-session ratios; copied reading rules; censored is a result; re-measured post-2.4.0 baseline |
| Soundness regression | Copied repetition/rule50/terminal fixtures restated for attacker-only semantics; checker veto |
| Proof memory cap | Event-fed bounded worker (M4); the in-search proof store is never the output |
| Copied fixtures/code drift from their source | Provenance line with source commit; the legacy repo is frozen for these paths; re-copy deliberately, never sync implicitly |
| Losing the legacy gates' protection | The fixture oracle (§4.10) replaces them for outcomes; the legacy binary stays available as an external cross-check |
| Legacy reference gate is red | `make test-full` here has two pre-existing dec02 wall-clock failures (2.4.0 report §Gates; report5 recorded the tier green at `35f78bf`). They do not block the successor; triage separately (§8.1 step 5) before citing the legacy gate as green |
| Cross-repo comparison friction | Bench driver runs binaries by path; record both commit pins (`prover@<sha>`, `solver@<sha>`) per row |
| The new engine never beats provari | `atomic_solver` stays the product; the new repo is archived with its measured record |

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

**`atomic-movegen` 2.4.0:** `src/board.rs` (`hash_after*`,
`hash_and_rule50_after`, `is_legal*`, `outcome_with*`, `is_material_draw`,
`blast_zone`, `blast_casualties`, `attackers_to_color`, `legal_ep_square`,
`move_from_uci`, `validate`, `do_move`), `src/movegen.rs`
(`generate_legal*`, `has_legal_move*`), crate `CHANGELOG.md` §2.4.0.

**`atomic_solver` (legacy, <https://github.com/golmman/atomic_solver> at
`5f85133`; paths relative to the repo root) — specs to read:**
`src/proof_event.rs`, `src/proof_tree/` (`worker.rs`, `validate.rs`,
`export.rs` module docs), `src/search/winprover/` (the prototype and its
measured limits), `src/search/dfpn/` (restart schedule, budget semantics),
`src/search/tt/` (generations). **To copy with provenance:**
`src/search/tt/alloc.rs`, `docs/plans/provari/measurements/plan1/corpus.json`,
`docs/plans/winprover/measurements/bench/run_bench.py`, the fixtures in §4.10.
**Optional external:** the `atomic_solver` binary (legacy controls,
front-end per-node comparison), `examples/provari_export`,
`examples/portfolio`, `examples/egtb_gen3`.

**Reports to read:** [`report3.md`](report3.md) (front-end skeleton),
[`report4.md`](report4.md) (sibling simulation), [`report5.md`](report5.md)
(close-out and probes), [`bench.md`](bench.md) (ledger and rules),
[`../movegen/report_update_2.4.0.md`](../movegen/report_update_2.4.0.md)
(rules changes and new APIs),
[`../provari/report1.md`](../provari/report1.md) … `report5.md` (levers),
[`../performance/report2.md`](../performance/report2.md) (lazy TT),
[`../parallel/report8.md`](../parallel/report8.md) (portfolio),
[`../conversion/report12.md`](../conversion/report12.md) (restart schedule).

---

## 12. Kickoff prompt (copy-pasteable, for the new repository)

> Bootstrap the successor to `atomic_solver`'s closed `winprover` initiative
> in this new repository (working name `prover`). Clone the legacy repo
> <https://github.com/golmman/atomic_solver> into a sibling directory as a
> read-only reference (never a submodule or dependency; legacy code pin
> `5f85133ea9d751368a682e53e354bd3fc0016d00`, handoff from `main`). Read its
> `docs/plans/winprover/HANDOFF.md` (revision 2026-11-06), `report5.md` and
> `bench.md` next to it, `docs/plans/movegen/report_update_2.4.0.md`, and the
> pinned provari source at commit `48686bc6c786d6ea70568f0b2ad4002cf0520206`
> (scratch clone; never commit it; ideas only). This is a plan session. Repo
> location (D1), evidence strategy (D9) and bench ledger (D10) are settled in
> the handoff; decide the remaining open questions in §9. Write the
> initiative's `initiative.md`, a small `AGENTS.md` with the GPL boundary and
> the copy-with-provenance rule, and `plan1.md` for milestone M0 (bootstrap,
> correctness, minimal JSONL proof output with own validator +
> `provari check` veto, adapted bench driver with a re-measured baseline row,
> per-node cost baseline). No crate dependency on `atomic_solver`. Do not
> implement yet.
