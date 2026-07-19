# GHI Test Theory

This document describes the kind of position — and the equivalent synthetic
test — needed to give confidence that the solver's Graph-History-Interaction
handling is correct.

## What we are testing

The solver uses path-dependent *twin* entries in the transposition table and a
Kawano-style `simulate` to decide whether a result proven along one search
path can be reused along another.  A GHI bug occurs when the *same* board
has a different game-theoretic value depending on the history that led to it
and the solver reuses the wrong value.

In this codebase the history that matters is the set of boards already visited
on the current DFS stack (`Search::path`, keyed by `Position::repetition_key()`).
If a move leads to a board already in that set, the search treats it as a draw.

The `tt_key` also includes `rule50`, so two transposing paths must end with the
same `rule50` to hit the same table entry.

## Minimum graph shape (level 1)

A GHI test needs a directed search graph shaped like a *theta*:

```text
          root
         /    \
        P      Q
        |      |
        v      v
   ... -> A <- ...
            |
            m
            v
            B
```

Requirements:

1. `A` is reached by at least two different move sequences `P` and `Q`.
2. `A` has a legal move `m` to board `B`.
3. In `P`, `B` has **not** appeared as an ancestor, so `m` is legal and can be
   part of a winning line.
4. In `Q`, `B` **is** already an ancestor of `A`, so playing `m` repeats a
   board and is only a draw.
5. The two paths `P` and `Q` produce the same `tt_key` for `A`.  Because
   `rule50` is part of the key, the paths must contain the same number of
   pawn moves and captures.

If the solver stores `A` as a Win while following `P` and then reuses that
twin while following `Q`, it must **reject** it.  This is the first thing a GHI
test should assert.

## Robust graph shape (level 2 / nested twins)

The residual risk in the current implementation is cross-path twins whose
proof trees are more than one ply deep.  The top-level twin may simulate
successfully through its first move, but a deeper stored twin may rely on a
repetition right that exists in the twin's original path and not in the
current path.

Graph shape:

```text
P: root ... -> A -m-> B -n-> C
Q: root ... -> A -m-> B -n-> C
```

Requirements:

1. `A`, `B`, and `C` are all transpositions reachable by both `P` and `Q`.
2. `Q`'s prefix already contains `C` (or another board in `C`'s subtree).
3. A Win twin for `A` stores best move `m` to `B`.
4. `B` stores a Win/Loss twin whose best move is `n` to `C`.
5. Under `Q`, the move `n` repeats an ancestor, so `B` is not a win for the
   attacker and therefore `A` is not a win either.

The solver must reject the `A` twin because the simulation reaches `B`,
follows `B`'s twin to `C`, and discovers that `C` is already in the current
prefix.

## Atomic-chess features that can create this

Because `rule50` is part of `tt_key`, the transposing detour must use only
non-capturing, non-pawn reversible moves.  Look for:

- **Two independently movable pieces of the same type** (two rooks, two queens,
  two knights).  Move orders like `Ra1-a2 ... Rh1-h2` and `Rh1-h2 ... Ra1-a2`
  reach the same final board with the same `rule50` but different intermediate
  boards and different `path_code`s.

- **A reversible king+piece shuffle** (e.g. `Rf1-g1, Kc4-b4, Rg1-f1, Kb4-c4`).
  This provides a 4-ply cycle and ancestor boards that a winning move might
  accidentally repeat.

- **A tempo resource** (a passed pawn, a distant checking piece, or a piece
  that can be sacrificed by explosion).  Without this the position is just a
  draw; with it, the same board can be a win in one history and a draw in
  another.

- **A winning move that returns to an intermediate square from one of the
  routes.**  For example, after the transposition one rook is on `a2` and the
  winning idea is `Ra2-a1`; in the route `Ra1-a2 ...` the board with the rook on
  `a1` is an ancestor, so `Ra2-a1` is a repetition there but not in the other
  route.

## Candidate position classes

These are starting points for a real-position search, not guaranteed FENs:

1. **Two rooks + king vs lone king, black king in a small safe area.**  The
   rooks can be shuffled in either order.  The winning plan may require a rook
   move that was already played as an intermediate in one of the shuffles.

2. **Rook + distant passed pawn vs king.**  The rook must give checks while the
   pawn advances; the black king can shuffle in a 2×2 safe zone.  Different
   orders of rook checks and king shuffles can reach the same board with
   different ancestors.

3. **Queen + knight vs king.**  The knight can make 3- or 6-move cycles.  The
   same queen position can be reached by different knight detours, and a
   follow-up knight move may be a repetition in one detour but not the other.

## Synthetic test recipe

Finding a real atomic-chess position that flips a win into a draw across two
histories is hard.  A unit test can build the dangerous graph directly in the
transposition table:

1. Choose a real atomic board `A` that has a short forced win.
2. Run `dfpn` from `A` to populate the table with Win twins for `A` and its
   principal-variation children.
3. Manually seed the search prefix with path `Q` so that it contains the child
   `B` (and, for the nested case, the grandchild `C`) as ancestors.
4. Call `try_use_tt` or `simulate` from `A` with `Q`'s `path_code` and assert
   that the stored Win twin is rejected.
5. For the nested case, also store a Win twin for `B` with best move `n` to `C`,
   and a Win twin for `C` whose own proof relies on a board `D` that is in `Q`
   but not in `P`.  Then assert the top-level `A` twin is rejected.

This does not require a full game tree; it only needs real `Move` values and
real `path_code`s so the path-code arithmetic is exercised.

## Acceptance criteria

A passing GHI test must show that the solver does **not** reuse a twin from
another path when any of the following hold:

- The twin's best move repeats a board in the current prefix.
- The best move leads to a child twin whose best move repeats a board in the
  current prefix.
- The stored proof tree, when replayed under the current prefix, ever reaches a
  board already visited on the current stack.

If the solver instead returns the stored win, the test has found a GHI bug.

## References

- `docs/plans/dfpn/research_ghi.md` — the paper summary and current status.
- `src/search/dfpn.rs` — `try_use_tt` and `simulate` implementations.
- `src/search/tt.rs` — twin-entry storage.
- `src/zobrist.rs` — path-code hashing.
