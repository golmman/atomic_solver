# Definitions

## Principal Variation (PV)

A **Principal Variation** (PV) is a sequence of moves the search currently considers best. It reflects the search's present evaluation, not a guarantee of correctness — depth, heuristics, and completeness all affect it. Under perfect/exhaustive search, it coincides with the true minimax line.

---

## Proof PV (PPV)

For a decisive outcome (`Win` or `Loss` from the root side's perspective):

- **Attacker**: the side forcing the outcome — to move if the root outcome is `Win`, the opponent if `Loss`.
- **Defender**: the other side.

A **Proof PV** (PPV) is a sequence of moves from the start to a terminal position such that:

1. At each **attacker move** (OR node), the move preserves the forced win. Any winning move is allowed — not necessarily the shortest.
2. At each **defender move** (AND node), the reply is chosen to resist strongly — ideally the move maximizing the remaining PPV length, though solvers may use a cheaper proxy (e.g. largest subtree, highest disproof number) instead of the exact longest defense.
3. The final position realizes the root outcome.

**Soundness vs. quality.** A PPV is a valid proof _regardless_ of which defender moves are chosen — any legal reply keeps the defender in a proven-lost position. The "strong resistance" clause is a **quality criterion**, not a soundness requirement: it prevents degenerate proofs where the defender collapses immediately, but even a weak-defense PPV is still a sound certificate.

**Notes**

- Since attacker moves need not be optimal, a position generally has many PPVs of different lengths.
- Applies only to decisive outcomes; draws don't fit the attacker/defender framing.
- A PPV is a **linearization of a proof tree**: a full proof tree (e.g. from proof-number search) keeps every defending branch at each AND node, while a PPV keeps only one, chosen by resistance strength.

---

## Shortest Proof PV (SPPV)

An **SPPV** is a PPV where additionally:

1. Every attacker move is a **shortest** winning move.
2. Every defender move is the **exact** longest defense.

This is the distance-to-outcome-optimal minimax line — the same convention used in distance-to-mate (DTM) tablebases. It's not necessarily unique; a deterministic tie-breaker is needed to pin one down.

SPPV is far more expensive to compute than PPV: it needs exact distance information throughout the tree, whereas PPV only needs to know which moves preserve the proven outcome — information a df-pn search already produces as a side effect of proving it.

An SPPV relates to a PV the same way a PPV does: it's the optimal line in a _completed, verified_ proof, not a search's current best guess.

---

## Summary

|          | Attacker              | Defender                   | Sound? | Cost                              |
| -------- | --------------------- | -------------------------- | ------ | --------------------------------- |
| **PV**   | current guess         | current guess              | No     | Cheap, ongoing                    |
| **PPV**  | any winning move      | approx. longest resistance | Yes    | Cheap (byproduct of df-pn proof)  |
| **SPPV** | shortest winning move | exact longest resistance   | Yes    | Expensive (needs exact distances) |

PPV is the practical middle ground for a df-pn solver: a sound, cheaply-extracted certificate of a proven outcome, without paying for SPPV's exact distance computation.
