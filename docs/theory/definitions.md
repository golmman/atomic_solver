# Definitions

## Principal Variation (PV)

A **Principal Variation** (PV) is a sequence of moves that the search considers best given a certain search effort. It represents the line the search would follow if both sides play the moves the search currently evaluates as strongest.

A PV is not guaranteed to be objectively correct; it depends on the depth, heuristics, and completeness of the search that produced it. In a perfect or exhaustive search, the PV coincides with the true minimax line.

## Proof PV (PPV)

For a decisive outcome (`Win` or `Loss` from the root side's perspective), define:

- **Attacker**: the side forcing the outcome. This is the side to move when the root outcome is `Win`, and the opponent when the root outcome is `Loss`.
- **Defender**: the other side.

A **Proof PV** (PPV) is a sequence of moves from the start position to a terminal position such that:

1. At each **attacker move** (OR node from the attacker's perspective), the move preserves the forced win for the attacker. The move need not be the shortest or otherwise optimal winning move; any winning move is allowed.
2. At each **defender move** (AND node from the attacker's perspective), the reply is the **longest defense** — the legal move that maximizes the length of the remaining PPV.
3. The final position realizes the root outcome (`Win` or `Loss`).

### Notes

- If attacker moves are not required to be optimal, a position may have many PPVs of different lengths.
- This definition applies only to decisive outcomes. Draws are not covered because the attacker/defender framing does not map cleanly to drawn positions.

## Shortest Proof PV (SPPV)

A **Shortest Proof PV** (SPPV) is a PPV in which every attacker move is a **shortest winning move**. In other words, it is a PPV that minimizes its total length: the attacker tries to end the game as quickly as possible, while the defender still replies with the longest defense at every turn.

An SPPV is not necessarily unique. There may be several attacker moves that win in the same minimal number of plies, or several defender moves that delay the loss equally long. A solver must apply a deterministic tie-breaker to produce a unique SPPV.

The SPPV differs from a **principal variation (PV)**: the SPPV is the optimal line in a completed proof tree, whereas a PV is the line a search currently considers best given its exploration so far.
