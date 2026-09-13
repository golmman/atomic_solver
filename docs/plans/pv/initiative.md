# Initiative: `pv` — shortest-PV / PPV correctness

## Status

**Closed 2026-09-13 — absorbed into `proof`.** Plans 1–7 are done
(`report1.md`–`report7.md`; plan6 was realized on branch `pv_plan6`).
The initiative's original goal — make `Search` return a validated
shortest Proof PV — was **deliberately abandoned** in plan7. Its one
remaining open item (a validated PPV from the proof artifact) moved to
`proof` backlog #8; this file is the historical record.

## Goal (original)

Produce a correct Proof Principal Variation (PPV, defender plays the
longest resistance) and eventually the Shortest Proof PV from the solver,
streamed from the CLI.

## Outcome (plan7 pivot, 2026-08-07)

- `Search` is responsible only for the decisive `Outcome`; the returned
  PV is an **informational best-effort line** from the TT `best_move`
  chain and is not validated as a proof (now documented in AGENTS.md and
  `pv_status` output).
- `extract_ppv` / `extract_pv_checked` / in-search validation /
  `ppv_valid` printing were removed.
- Independent **verification** survives as `examples/verify_ppv`
  (plan5): replays a supplied UCI line, checks attacker/defender
  minimax correctness including longest-defense replies and repetition
  context via `search_depth_with_prefix`.

Why: outcome-mode DF-PN stops at the first winning child and never
revisits siblings, so the live TT cannot yield a correct PPV; a proven
subtree (proof tree) is the only sound source.

## Open items → moved

- **Proof-tree PPV extractor** → `proof` backlog #8. The tree-layer
  primitives already exist (`ProofTree::extract_ppv` implements the
  plan6 minimax selection rule; `validate_ppv` checks the path to a
  terminal), so the remaining work there is surfacing a validated PPV
  from the reconstructed dump, not porting the algorithm. The
  proof-tree tests that validate the solver's PV stay `#[ignore]`d
  until that lands.
- `Search::search_depth_with_prefix` uses `bounded_search`'s PV length
  as the proven depth — carried into `proof` #8 (fine for
  `verify_ppv`, needs attention if the extractor relies on it,
  report7).
- Optional cleanup: the ignored `_max_pv_len` parameter of
  `assert_solves_to` may be removed → `proof` #8 / `cleanup`.

## Cross-references

- `proof/` — owns the proof tree, the replay validator, and
  reconstruction; the natural home for the PPV extractor.
- `dfpn/` — owns PV-refinement semantics (`PvStatus`, `pv_status`,
  refinement caps) since plan8; length refinement is no longer `pv/`
  territory.
- `storage/` — plan6 (iterative bounded search with proof-tree emission)
  is the direct ancestor of plan7 here.

Per repo convention, every plan ends with the task of writing its
`report<N>.md` in this directory.
