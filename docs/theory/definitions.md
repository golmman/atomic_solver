# Definitions

## Proof PV

For a decisive outcome (`Win` or `Loss` from the root side's perspective), define:

- **Attacker**: the side forcing the outcome. This is the side to move when the root outcome is `Win`, and the opponent when the root outcome is `Loss`.
- **Defender**: the other side.

A **proof PV** is a sequence of moves from the start position to a terminal position such that:

1. At each **attacker move** (OR node from the attacker's perspective), the move preserves the forced win for the attacker. The move need not be the shortest or otherwise optimal winning move; any winning move is allowed.
2. At each **defender move** (AND node from the attacker's perspective), the reply is the **longest defense** — the legal move that maximizes the length of the remaining PV.
3. The final position realizes the root outcome (`Win` or `Loss`).

### Notes

- If attacker moves are not required to be optimal, a position may have many proof PVs of different lengths. A solver must therefore apply a deterministic tie-breaker when selecting attacker moves (for example, the first winning move found, or the shortest winning move).
- Requiring attacker moves to be the shortest winning move yields the unique **principal variation**.
- This definition applies only to decisive outcomes. Draws are not covered because the attacker/defender framing does not map cleanly to drawn positions.
