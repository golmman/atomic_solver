# Move-Ordering Improvement Ideas

## Context

The DF-PN+ solver expands the most-proving child repeatedly, so the order in
which it examines children directly affects how quickly it finds a decisive
outcome or proves a draw.  The current ordering has two layers:

- `src/search/ordering.rs` implements `StaticAtomicScorer`, a fast, hand-tuned
  static scorer that runs once per node.
- `src/search/dfpn/history.rs` adds dynamic history and killer bonuses on top
  of the static score.

Initial benchmarking shows that the static scorer works well for sharp
positions such as `m26`, where a capture (`d6c5`) is correctly ranked first.
However, for `m19` the top-ranked moves are bishop probes (`d6f8`, `d6e7`)
that receive a large `SCORE_ATOMIC_CHECK`/`SCORE_THREAT` bonus even though they
do not directly attack the enemy commoner.  This suggests the static heuristics
are not yet aligned with atomic-chess tactics.

## Ideas

### 1. Fix the "near commoner" heuristics

**What:** `SCORE_ATOMIC_CHECK` currently rewards a moved piece for attacking a
square *adjacent to* the lone enemy commoner, not the commoner itself.  In
atomic chess the forcing threat is an attack *on* the commoner's square (a
future capture-explosion).  `SCORE_BLAST` also extends the blast zone by one
extra ring for non-captures, which overstates how quickly a piece can threaten
the enemy king.

**Change:**
- Rename or replace `SCORE_ATOMIC_CHECK` with a strict "kamikaze" bonus: give
  points when the moved piece lands on a square adjacent to an enemy commoner
  (`king_attacks(to) & commoners(them) != EMPTY`), because if the opponent
  captures it they may blow up their own king.
- Keep `SCORE_THREAT` only for the real direct commoner attack
  (`attack_bb & commoners(them)`).
- Remove the extra one-square ring from `SCORE_BLAST` for non-captures.

**Expected impact:** High for positions such as `m19`, where the current
heuristic places non-threatening bishop probes at the top of the list.

**Cost:** Low.  Confined to `StaticAtomicScorer` in `src/search/ordering.rs`.

### 2. Atomic Static Exchange Evaluation (aSEE) for captures

**What:** The current capture score is MVV-LVA:
`SCORE_CAPTURE + 10 * victim_value - attacker_value`.  In atomic chess a capture
is an explosion centered on the destination square: the capturing piece, the
captured piece, and all non-pawn pieces in the surrounding 3×3 zone are
removed.  MVV-LVA does not capture that.

**Change:** For every capture, compute the net material destroyed by the
blast, including the moving piece and any own pieces that would also be
removed, and rank captures by that net gain.  Preserve the existing special
case that promotes a capture to `SCORE_WINNING_CAPTURE` when it removes the
opponent's last commoner.

**Expected impact:** High.  Stops the scorer from preferring queen-for-pawn
captures and correctly values multi-piece explosions.

**Cost:** Low.  Requires inspecting the current board around the destination
square but no move generation.

### 3. Node-type-aware ordering

**What:** `sort_moves` uses the same static weights whether the node is an OR
node (attacker trying to prove a win) or an AND node (defender trying to draw
or delay).  The best move for the defender is often the opposite of the
attacker's most attractive move.

**Change:** Pass `is_or_node` into `sort_moves` and `evaluate_all_children`.
- OR nodes: prefer forcing captures, direct commoner threats, and pawn pushes
  that create promotion threats.
- AND nodes: prefer moves that keep the position solid, create counter-threats,
  and head toward a repetition or a tablebase draw.

**Expected impact:** Medium to high.  Could reduce the number of defender
replies the search has to examine before finding the drawing resource.

**Cost:** Medium.  Requires two scoring profiles and plumbing `is_or_node`
through `sort_moves`.

### 4. Staged / lazy child evaluation

**What:** `evaluate_all_children` evaluates every legal move at each `dfpn`
node, even though only one or two children are usually expanded.

**Change:** Keep the move list sorted by static score, but evaluate children
lazily.  Maintain a min-heap of unevaluated moves and pop/evaluate until the
best evaluated child is provably better than the next unevaluated static
candidate.  Stop early as soon as a decisive child is found.

**Expected impact:** High in positions with large branching factors, because it
reduces `child_evals`.

**Cost:** High.  Touches the `dfpn` work-accounting loop, `ChildInfo` handling,
and interaction with the transposition table.

### 5. TT-bound-aware initial sort

**What:** `sort_moves` already pulls the TT best move to the front, but ignores
proof/disproof bounds for the other children.  When all unknown children start
with `(pn, dn) = (1, 1)`, the first child in the sorted list becomes the first
most-proving node by default.

**Change:** Reuse the TT probe performed in `evaluate_child` (or pre-probe once
in `sort_moves`) to give a small bonus to children with a low `pn` at OR nodes
and a low `dn` at AND nodes.  This biases the initial `best_child_index`
toward the true most-proving node.

**Expected impact:** Medium.  Especially helpful when the TT already contains
bound information from a previous work-chunked search.

**Cost:** Low, but care must be taken to avoid duplicate probes.

### 6. Stronger dynamic heuristics

**What:** The current history table is `history[side][from][to]` and killer
slots are indexed only by depth, not by side.

**Change:**
- Add the side to the killer key.
- Increase the number of killer slots or make them depth-adaptive.
- Add a counter-move table (`counter_move[their_move]`).
- Only update history on non-captures that cause a cutoff, not on every solved
  node.

**Expected impact:** Low to medium.  Helps in positions where the same forcing
reply appears in many branches.

**Cost:** Low.

### 7. Repetition / draw-avoidance ordering

**What:** The solver treats path repetition as a draw and has a 50-move draw
rule.  Quiet shuffling can let the defender reach one of those draws.

**Change:**
- For OR nodes seeking a win, penalize moves that lead to a previously seen
  position or keep the 50-move clock high; prefer captures and pawn pushes.
- For AND nodes, reward quiet drawing moves and repetitions.

**Expected impact:** Medium.  Could help finish long wins before the 50-move
clock becomes a factor.

**Cost:** Low.  Requires access to the repetition path, which is already
available in `Search`.

### 8. Endgame pattern / tablebase probes

**What:** At low material the exact outcome of an endgame can be known or
precomputed offline.

**Change:** Ship a small 2- and 3-man atomic endgame table and probe it at
search leaves (or, for ordering, at root children).  Order moves that convert
to a known won tablebase entry first.

**Expected impact:** Very high once the endgame is reached.

**Cost:** High.  Requires generating or importing tablebases and a new probe
path.

## Recommended First Experiment

Start with ideas **1 and 2** together:
- They are confined to `StaticAtomicScorer` and do not touch the DF-PN loop.
- They are easy to validate with the existing `move_order_debug` and
  `static_move_scores` examples.
- They target the most obvious mismatch between the current heuristic and
  atomic-chess tactics.

After that, evaluate idea **3** (node-type-aware ordering), and then **5**
(TT-bound-aware sort).  Idea **4** (lazy evaluation) is the largest change and
should be attempted only after profiling shows `evaluate_all_children` is the
dominant cost.

## Validation Plan

For every change, run:

```bash
cargo test
cargo run --release --example move_order_debug -- "<fen>"
cargo run --release --example benchmark -- --runs 5 --timeout 10 m19
cargo run --release --example benchmark -- --runs 5 --timeout 10 m26
cargo run --release --example benchmark -- --runs 3 --timeout 10 startpos
```

Primary success metrics, in priority order:
1. Correctness: `cargo test` and the regression corpus pass unchanged.
2. Time to first decisive outcome on the benchmark suite.
3. Total `child_evals` and `nodes`.
4. Informational PV length (secondary).

## Risks

- Aggressive capture ordering can mis-rank quiet winning moves if the scoring is
  too tactically focused.
- Removing the `SCORE_ATOMIC_CHECK` bonus entirely may hurt positions where the
  kamikaze motif is the only winning idea; keep it as a correctly-defined
  kamikaze bonus.
- Node-type-aware ordering can introduce subtle bugs if `is_or_node` is
  threaded incorrectly into `sort_moves`.
- Lazy evaluation changes the work-accounting contract and could interact
  badly with the work-chunk timeout logic.
