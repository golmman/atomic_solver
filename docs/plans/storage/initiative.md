# Initiative: `storage` — persistence, pre-exit artifacts, and the proof dump

## Status

**Closed — superseded by `proof`.** Plans 1–6 are done
(`report1.md`–`report6.md`; `plan4-1` amends plan4); last activity
2026-08-07. The live-tree side of this initiative (worker dumping the
proof tree from the search process, PPV extraction from it) was
**removed from the search CLI** by `proof` plan7 after the
resource-boundedness pivot documented in
`docs/plans/proof/initiative.md`. `concept.md` here is the origin
document of the `proof` initiative.

## Arc (summary)

- **plan1** — `pre_exit_hook` machinery + `--outcome-only`.
- **plan2** — serializer for the proof-tree binary dump (adjacency
  format, later `src/proof_tree/binary.rs`).
- **plan3** — `q` + stdin quit path (see report3).
- **plan4 / plan4-1** — real-dump pipeline + PPV extraction from the
  tree.
- **plan5** — full proven-subtree emission into the worker; fixed
  path-stack imbalance and worker depth-selection defects.
- **plan6** — iterative bounded search with proof-tree emission,
  replacing the PPV/SPPV extraction stage.

What survives unchanged: the pre-exit hook (reduced to the stdin reader
+ `pre_exit:` line by proof plan7) and the binary dump format, which is
now produced offline by `reconstruct_pt`.

## Successor

- `proof/` (`docs/plans/proof/initiative.md`) — proof construction,
  validation, reconstruction; owns the dump format contract.
- Deferred here and still open nowhere else: direct PostgreSQL export
  (stays external per `proof`'s non-goals; the dump is the contract).
