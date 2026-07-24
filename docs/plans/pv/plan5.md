# Plan 5: Verify that a move list is a PPV for a position

## Summary

Given a FEN and a sequence of UCI moves, verify that the sequence is a valid
**Proof Principal Variation (PPV)**: every move is legal, the final position is
terminal with the claimed outcome (a win for the attacker, i.e. the side to
move in the root position), and every defender (AND-node) reply along the line
is *closed* by an attacker refutation that is no longer than the line itself.

This plan **challenges** the proposed pseudocode (see "Challenges" below),
adjusts it, and then implements the verifier as an `examples/` binary plus a
small library helper. The verifier reuses the existing bounded solver
`Search::search_depth` as the `proven_loss_within` subroutine, seeded with
the move list itself as a hint so refutations of off-line defender moves are
found quickly.

## Goal

- A new example `examples/verify_ppv.rs` that takes a FEN, a claimed
  `Outcome` (default `Win`), and a whitespace-separated list of UCI moves, and
  prints either `ppv valid` (with the shortest refutation depth found at each
  defender ply) or `ppv invalid` followed by a reason and the offending ply.
- A small reusable library function `atomic_solver::ppv::verify_ppv` that the
  example wraps, so the logic is also unit-tested inside the crate.
- Unit tests for the library helper and an integration test under `tests/`
  that verifies a known-good PPV and a deliberately broken one.

## Non-goals

- No changes to the DF-PN search, transposition table, ordering, or PV
  extraction code.
- No SPPV (shortest-PPV) verification: this verifies *a* PPV, not the
  *shortest* PPV. The depth bounds used are soundness bounds, not optimality
  bounds. SPPV verification is left to a future plan.
- No new transposition-table or move-generation functionality.

## Challenges to the proposed pseudocode

The user's pseudocode is sound in spirit but has several issues that must be
fixed before implementation. Each is addressed in the design.

### C1. The depth bound at an AND node is `1 + proven_depth[i+1]`, but a
PPV *maximizes* defender resistance, so `proven_depth[i+1]` is the *largest*
defender loss depth among the on-line child, not a bound on *all* defender
moves. Using it as the bound for off-line moves is correct only as a
**soundness** check ("the attacker refutes this off-line move in at most that
many plies"), which proves the off-line move does not exceed the on-line line
length. That is exactly the PPV correctness condition for the defender side,
so the bound direction is right. The comment in the pseudocode ("or max over
siblings if you want exact SPPV-style depth") is misleading: for **PPV**
verification we intentionally use the *on-line* child's depth as the bound
because the PPV is defined relative to its own line length, not a global
minimum. This is clarified in the design and asserted in tests.

### C2. Attacker (OR) nodes do **not** need any solver call.
The pseudocode's OR-node branch (`proven_depth[i] = 1 + proven_depth[i+1]`) is
correct and is purely bookkeeping. The only verification an OR node needs is
that `moves[i]` is itself a winning move, which follows transitively from the
final terminal outcome **and** the AND-node closure of the next ply. So the
loop is: OR nodes do nothing but propagate depth; AND nodes do the closure
search. No `proven_loss_within` call is made at OR nodes. This keeps the
verifier cheap (it only searches at defender plies).

### C3. The final-position assertion must account for perspective.
`Position::outcome()` returns the outcome from the **side to move** of the
final position. After `n` plies the side to move is the original side if `n`
is even and the opponent if `n` is odd. The verifier must compare
`final_outcome` against `claim.flip_if(n is odd)`, exactly as the existing
`Search::validate_pv` in `src/search/dfpn/pv.rs:84-88` already does. The
pseudocode's `outcome matches claim` line glosses over this and would reject
odd-length PPVs.

### C4. The terminal condition is not just "terminal" — it must be a **loss**
for the side to move (the attacker mates/refutes the defender). A draw
terminal (stalemate, rule-50, two-piece, repetition) is *not* a win and the
 PPV must be rejected. The asserted terminal outcome must be `Outcome::Loss`
from the final side-to-move perspective (equivalently `Outcome::Win` for the
root side after perspective adjustment). Draw terminals are excluded.

### C5. `proven_loss_within` must be a *win* search from the attacker's
perspective at the child, with **perspective handled correctly**.
After the defender plays an off-line move `m` at ply `i` (an AND node), the
child position's side to move is the **attacker**. We need to prove the
attacker wins from that child within `bound` plies. That means
`search_depth(child, bound)` must return `Outcome::Win` (win for the side to
move = attacker). The pseudocode calls it `proven_loss_within(child, ...)`,
which is misleading because from the *child's* side-to-move (attacker)
perspective it is a **win**, not a loss. The "loss" framing is only correct
from the *original root defender's* perspective. We name the helper
`proven_win_within(pos, bound)` to match the child's perspective and avoid
confusion.

### C6. The `hint=refutation[i+1]` is the *on-line* move at the next ply, not
a refutation of the off-line move.
For an off-line defender move `m != moves[i]`, the natural hint is the
*on-line attacker reply* `moves[i+1]`? No — `moves[i+1]` is the attacker
reply to the *on-line* defender move `moves[i]`, which is irrelevant after a
different defender move. There is no good move hint available for off-line
defender replies. Instead we seed the bounded search by playing the off-line
move `m` so the search resumes from the resulting child position; the search's
own move ordering (history/killers/TT reuse) handles ordering. The "hint"
idea is dropped; the depth bound `bound` itself is the real saving grace that
keeps each off-line probe cheap.

### C7. `proven_loss_within` returning falsy is a **failure**, but a timeout
is *not* a definitive failure.
A bounded search that times out returns `Outcome::Draw` (see `core.rs:30-32`
and `core.rs:68-87`'s frontier behavior), indistinguishable from a genuine
draw. The verifier must report this honestly: a timeout means "unverified",
not "invalid". The example distinguishes `unverified` (search exhausted its
budget without proving the win) from `invalid` (a child was proven *not* to
lose within the bound, i.e. the search returned a non-`Win` outcome *before*
the deadline and within the work budget). Only `invalid` is a definitive
refutation of the PPV claim. The plan introduces an explicit
`VerifyResult { Valid, Invalid, Unverified }` enum.

### C8. The bounded search depth bound must be tight but not zero.
`bound = 1 + proven_depth[i+1]` is the number of plies the attacker has to
refute the off-line move. If `bound == 0` the position after the defender's
move is already terminal — handled before any search. `search_depth` with
`max_depth = bound` is the right call (it counts plies from the child). When
`bound` is large (deep PPV) the off-line search could be expensive; the
verifier caps each off-line probe with a per-probe timeout (default 2 s, the
example default timeout) and reports `Unverified` on timeout.

### C9. Transposition-table and history state must be fresh per off-line probe.
The on-line replay and the off-line closure searches must not contaminate
each other through the shared `Search` instance's path stack / path code, nor
through stale TT entries that encode path-dependent results. The simplest
correct approach is to construct a **fresh `Search` per off-line probe**.
This is cheap relative to the search itself and removes a whole class of
correctness bugs. (See the existing pattern in
`examples/find_winning_child.rs`, which builds a new `Search` per child.)

## Design

### Result type

```rust
pub enum VerifyOutcome {
    /// The line is a valid PPV. `refutation_depths[d]` is the shortest
    /// attacker refutation depth found at the `d`-th defender (AND) ply, for
    /// the off-line move that was hardest to refute (i.e. the maximum over
    /// off-line moves of the minimum refutation depth). Empty for OR-only
    /// lines.
    Valid { refutation_depths: Vec<u32> },
    /// A defender move at 0-indexed ply `ply` was proven NOT to lose within
    /// the bound, so the line is not a PPV.
    Invalid { ply: usize, reason: String },
    /// A search timed out or hit its work budget before the line could be
    /// verified either way.
    Unverified { ply: usize, reason: String },
}
```

### UCI move parsing

The crate has **no** UCI move parser today (only `move_to_uci`). Add a small
function to `src/notation.rs`:

```rust
pub fn move_from_uci(pos: &Position, uci: &str) -> Option<Move>
```

It mirrors the logic in `examples/common.rs::parse_move` but takes a single
4- or 5-character UCI string (e.g. `"e2e4"`, `"a7a8q"`). It enumerates legal
moves and matches on `from_sq()`, `to_sq()`, and (for promotions) the
`promotion_type()`. This keeps notation concerns in `notation.rs` per
`AGENTS.md` and removes the need for the example to duplicate parsing.

### Replay

```rust
fn replay(fen: &str, moves: &[Move]) -> Result<Vec<Position>, String>
```

Starting from `Position::from_fen(fen)`, for each move: enumerate legal
moves, assert `mv` is among them (else `Err`), `do_move(mv)`, and push a
`clone()` of the **post-move** position onto the list. Returns
`positions[0..=n]` where `positions[i]` is the position after `i` plies
(`positions[0]` is the root). Actually, to mirror the pseudocode exactly, we
keep `positions[i] = position before move i` and `positions[n]` = terminal.

We keep a single `Position`, `do_move` in place, and snapshot with `clone()`
for later use (since `Position::clone` resets the undo stack, each snapshot
is a clean starting point for the off-line search at that ply).

### `verify_ppv` main loop

```
let claim = Outcome::Win (default; configurable)
let positions = replay(fen, moves)        # legality-checked
let n = moves.len()
let final_pos = &positions[n]
let final_outcome = final_pos.outcome()
    .ok_or(Invalid, "line does not end in a terminal position")?
let final_expected = if n is even { claim } else { claim.flip() }
if final_outcome != final_expected { return Invalid("terminal outcome mismatch") }
# C4: a winning PPV must end in the *defender* losing, never a draw terminal.
if final_outcome == Outcome::Draw { return Invalid("line ends in a draw, not a win") }

let mut proven_depth = vec![0u32; n+1]
proven_depth[n] = 0
let mut refutation_depths = Vec::new()

for i in (0..n).rev() {
    let pos_i = &positions[i]
    let line_move = moves[i]
    let is_or_node = side_to_move(pos_i) == root_side   # attacker to move
    if is_or_node {
        # C2: OR node — bookkeeping only, no search.
        proven_depth[i] = 1 + proven_depth[i+1]
        continue
    }
    # AND node (defender to move): close every off-line defender move.
    let bound = 1 + proven_depth[i+1]   # plies the attacker has to refute
    let mut worst_off_line_depth = 0u32  # max over off-line moves of refutation depth
    for each legal move m at pos_i:
        if m == line_move { continue }
        let child = pos_i.clone(); child.do_move(m)
        # child side to move is the attacker.
        if child.outcome().is_some_and(|o| o == Outcome::Loss):
            # defender move walks straight into a loss (e.g. self-mate):
            # refuted in 0 attacker plies.
            refuted_depth = 0
        else:
            match proven_win_within(&child, bound):
                Win(d) => refuted_depth = d
                NotWinAndNotTerminal => return Invalid(i, "defender move defends longer than the line")
                Unverified => return Unverified(i, "search budget exhausted")
        worst_off_line_depth = max(worst_off_line_depth, refuted_depth)
    proven_depth[i] = bound
    refutation_depths.push(worst_off_line_depth)

return Valid { refutation_depths (reversed to plies order) }
```

Notes:
- The OR-node `proven_depth[i] = 1 + proven_depth[i+1]` is the **on-line** line
  length remaining; it correctly bounds the *next* AND node.
- The `bound` at an AND node is the number of attacker plies remaining on the
  on-line line. An off-line defender move that needs *more* attacker plies to
  refute than `bound` disproves the PPV (the defender can resist longer than
  the claimed line).
- `proven_win_within(child, bound)` calls a **fresh**
  `Search::search_depth(&mut child_clone, bound)` (see C9) and returns:
  - `Win(d)` if `outcome == Win` and the TT-stored depth is `d` (falling back
    to `bound` if the depth is unavailable);
  - `NotWinAndNotTerminal` if `outcome != Win` **and** the search did not time
    out (definitive);
  - `Unverified` if the search timed out.
  Distinguishing "did not time out" requires checking `search.time_exceeded()`
  (already public).
- The "hardest off-line move" depth recorded in `refutation_depths` is a
  sanity metric, not required for PPV correctness; it is reported to the user.

### `proven_win_within`

```rust
enum ProbeResult { Win { depth: u32 }, NotWin, Unverified }

fn proven_win_within(child: &Position, bound: u32, per_probe_secs: u64) -> ProbeResult {
    let mut search = Search::new(64);
    search.set_timeout(per_probe_secs);
    let mut pos = child.clone();
    let (outcome, _pv, _nodes) = search.search_depth(&mut pos, bound);
    let timed_out = search.time_exceeded();
    match outcome {
        Outcome::Win => {
            let depth = search tt depth for child.hash() ... or bound;
            ProbeResult::Win { depth }
        }
        Outcome::Loss | Outcome::Draw if timed_out => ProbeResult::Unverified,
        Outcome::Loss | Outcome::Draw => ProbeResult::NotWin,
    }
}
```

The stored winning depth is read from the TT probe of the child's hash
(`entry.depth`), falling back to `bound` when the entry is unavailable or its
`outcome` is `None`. `search_depth` already stores solved entries with their
remaining depth (`core.rs` stores `outcome_to_store_depth`).

### CLI (`examples/verify_ppv.rs`)

```
Usage:
  cargo run --example verify_ppv -- <fen> <move1> [move2 ...]
  cargo run --example verify_ppv -- --claim win|loss --timeout 2 <fen> <moves...>
```

- Parses FEN, claim (`win` default; `loss` accepted and flips perspective —
  the root side is then the *defender* and OR/AND nodes swap, see "loss"
  support below), per-probe timeout, and the move list (UCI strings).
- Calls `atomic_solver::ppv::verify_ppv(&fen, &moves, claim, per_probe_secs)`.
- Prints `ppv valid` plus the per-defender-ply hardest-refutation depths, or
  `ppv invalid` / `ppv unverified` with the offending ply and reason.
- Exits with code 0 on `Valid`, 1 on `Invalid`, 2 on `Unverified`.

### Loss-claim support

For `--claim loss` the root side is the *losing* side. The meaning of "OR"
and "AND" swaps: at the root the side to move is the defender (AND node), and
the line alternates. Rather than special-casing, `verify_ppv` computes
`is_or_node = (side_to_move == attacker_side)` where `attacker_side` is the
side that the claim is `Win` for (i.e. the *opponent* of the root side for a
`loss` claim). The final-terminal perspective check (`final_expected`) uses
`claim` from the *root* side perspective and flips on odd `n` as before. The
`bound` and closure logic are identical. This generalization is small and is
included from the start.

## File changes

### `src/notation.rs`

- Add `move_from_uci(pos: &Position, uci: &str) -> Option<Move>`: parses a
  UCI move string by enumerating legal moves and matching
  `from_sq`/`to_sq`/`promotion_type`.
- Add unit tests: standard move, promotion (`a7a8q`, all four piece types),
  castling (`e1g1`), en-passant, illegal-move rejection, malformed-string
  rejection.

### `src/ppv.rs` (new module, re-exported from `src/lib.rs`)

- `pub enum VerifyOutcome { ... }` (as in Design).
- `pub fn verify_ppv(fen: &str, moves: &[Move], claim: Outcome, per_probe_secs: u64) -> VerifyOutcome`.
- Private helpers `replay`, `proven_win_within`, `ProbeResult`.
- Unit tests in `#[cfg(test)] mod tests`:
  - Valid PPV: a known short forced mate (e.g. the two-rook mate
    `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1` with the solver's own PV) verifies as
    `Valid`.
  - Invalid: a deliberately too-long line (append an extra non-winning move)
    verifies as `Invalid`.
  - Draw-terminal line: a line ending in stalemate verifies as `Invalid`.
  - Empty moves on a non-terminal root verifies as `Invalid`.
  - `loss` claim on the symmetric position verifies as `Valid`.

### `src/lib.rs`

- Add `pub mod ppv;`.

### `examples/verify_ppv.rs` (new)

- CLI wrapper as described. Uses `mod common;` for `M19_FEN` if a default is
  wanted, otherwise requires explicit FEN + moves.
- Mirrors the argument parsing style of `src/main.rs` (manual `while i < len`
  loop, `--flag value`, unknown options exit with error).

### `tests/test_verify_ppv.rs` (new integration test)

- `known_two_rook_mate_is_ppv`: solves `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1` with
  `Search::solve`, takes the returned PV, and asserts
  `verify_ppv(...)` == `VerifyOutcome::Valid`.
- `broken_ppv_rejected`: the same PV with an extra deliberately weaker
  defender move substituted is `Invalid`.
- `loss_claim_verified`: the mirrored position with `--claim loss` and a
  known losing line is `Valid`.

## Testing and verification

```bash
cargo fmt
cargo clippy --all-targets
cargo test
cargo test --release
cargo doc
```

Manual checks:

```bash
# Valid PPV from the two-rook mate (solver's own line):
FEN="4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
PV=$(cargo run --release -- "$FEN" --no-refine-shortest | sed -n 's/^pv: //p')
cargo run --release --example verify_ppv -- "$FEN" $PV
# Expected: ppv valid

# Invalid (append a non-winning extra move):
cargo run --release --example verify_ppv -- "$FEN" e1e2
# Expected: ppv invalid ... (e1e2 is legal but not terminal-winning)
```

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| A fresh `Search` per off-line probe is slow on wide defender nodes. | Cap each probe with a per-probe timeout (default 2 s); report `Unverified` rather than hanging. For very deep PPVs the verifier is inherently expensive; correctness is not sacrificed. |
| `search_depth` returns `Draw` both on genuine draw **and** on timeout, conflating `Invalid` and `Unverified`. | Check `search.time_exceeded()` after each probe to separate the two (see C7). |
| The winning depth is not always stored/readable from the TT after `search_depth`. | Fall back to `bound` when the TT entry's `outcome` is `None` or its depth is missing; the bound is a correct upper bound for the closure test. |
| UCI parsing of promotions/castling/en-passant differs from `atomic_movegen`. | `move_from_uci` enumerates **legal** moves and matches by `from_sq`/`to_sq`/`promotion_type`, so the generated `Move` (including its `move_type` for castling/en-passant) is always exactly the legal move. |
| `loss`-claim perspective bugs. | Unit-test both `win` and `loss` claims on mirrored positions; assert `is_or_node` is computed from `attacker_side = root_side.flip()` for `loss`. |
| TT path-dependent state leaks between the on-line replay and off-line probes. | Each off-line probe uses a fresh `Search` (C9). The on-line replay uses `Position::clone()` snapshots and never searches. |

## Success criteria

- `cargo test`, `cargo test --release`, `cargo clippy --all-targets`, and
  `cargo doc` all pass with no new warnings.
- `examples/verify_ppv.rs` correctly reports `ppv valid` for the two-rook
  mate PPV and `ppv invalid` for a deliberately broken line.
- The `tests/test_verify_ppv.rs` integration tests pass.
- `src/ppv.rs` is under ~10 KB and `examples/verify_ppv.rs` is under ~10 KB
  per `AGENTS.md`.
- The final task of this plan is creating `docs/plans/pv/report5.md`.
