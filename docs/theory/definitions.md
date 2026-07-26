# Definitions

## Principal Variation (PV)

A sequence of moves the search currently considers best. It reflects the search's present evaluation, not a guarantee of correctness, and may change as the search deepens. Under perfect/exhaustive search, it coincides with the true minimax line.

## Proof PV (PPV)

For a decisive outcome (`Win` or `Loss` from the root side's perspective):

- **Attacker**: the side forcing the outcome - to move if the root outcome is `Win`, the opponent if `Loss`.
- **Defender**: the other side.

A **PPV** is a sequence of moves from the start to a terminal position such that:

1. **Attacker moves** (OR nodes) preserve the forced win. Any winning move is allowed - not necessarily the shortest.
2. **Defender moves** (AND nodes) are **strong replies**: for every legal reply at the same node (including the chosen one), the attacker can force the root outcome within the number of plies remaining in the sequence. Equivalently, the remaining PPV is at least as long as the attacker's shortest forced win from any sibling reply.
3. The terminal position realizes the root outcome.

**Notes**

- Since attacker moves need not be optimal, a position generally has many PPVs, of different lengths.
- A PPV is a **bounded certificate** of a forced win: it proves the attacker can force the outcome within `n` plies, where `n` is the length of the sequence.
- Applies only to decisive outcomes; draws don't fit the attacker/defender framing.
- A PPV is a **linearization of a proof tree**: a full proof tree keeps every defending branch at each AND node; a PPV keeps one strong reply per AND node, collapsing it to one path.
- **Why PPVs are useful:** Verifying a PPV is a bounded *feasibility* problem — it only asks whether the attacker can force the outcome within the remaining plies. It does not require finding the shortest winning move at every attacker node, which is the harder *optimization* problem solved by an SPPV. This makes PPVs sound certificates that are significantly cheaper to find and verify than SPPVs, while still guaranteeing that the defender never has a reply that wins or holds out longer than the certificate claims.

### Verification and refutation

- A PPV is **verified** when, at every defender node in the line, every legal defender reply leads to a position where the attacker can force the root outcome within the remaining PPV length. In practice this means a bounded search from each child returns `Win` for the attacker within the bound.
- A PPV is **refuted** as soon as there exists a defender reply where the bounded search does not return `Win` for the attacker — it returns `Draw`, `Loss`, or times out. A timeout is “not verified” rather than a logical refutation, but for the verifier it is still a failure.
- Attacker nodes in the supplied line do not need to be checked explicitly. If the following defender node is fully closed, the attacker’s move is automatically a winning move.

## Shortest Proof PV (SPPV)

A PPV in which attacker moves are additionally required to be **shortest** winning moves.

This is the distance-to-outcome-optimal minimax line: attacker minimizes plies to the outcome, defender maximizes them - the same convention as distance-to-mate (DTM) tablebases.

SPPV is more expensive than PPV: it requires comparing lengths across all winning attacker moves, whereas PPV only requires confirming _some_ winning move.

Like PPV, an SPPV is the optimal line of a _completed, verified_ proof - not a search's current best guess, as with PV.

## Summary

|          | Attacker              | Defender                                  | Sound? | Cost           |
| -------- | --------------------- | ----------------------------------------- | ------ | -------------- |
| **PV**   | current guess         | current guess                             | No     | Cheap, ongoing |
| **PPV**  | any winning move      | strong reply (no alternative wins faster) | Yes    | Moderate       |
| **SPPV** | shortest winning move | longest resistance                        | Yes    | Expensive      |

PPV is the practical middle ground: sound, and it checks that the defender never allows a faster win, but it drops the requirement that the attacker play length-optimally - the expensive part under a proof-number search.
