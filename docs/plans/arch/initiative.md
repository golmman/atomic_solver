# Initiative: `arch` — decouple `search` from `proof_tree` via `ProofEvent`

## Status

**Closed.** Single plan (plan1, 2026-08-07), done (`report1.md`). The
`proof_event` protocol (`Clear`, `NodeProven`) is the standing
search↔proof-tree contract; AGENTS.md documents the dependency
direction.

## Outcome

- `src/proof_event.rs` owns the neutral event schema; `search` emits
  `ProofEvent`s and knows nothing about `proof_tree`.
- `ProofTreeWorkerHandle` (two-channel worker loop: events + queries)
  replaces the old mixed `ProofMessage` API.
- UCI path-string construction moved to `notation::moves_to_uci_path`,
  used by the worker only.

## Remaining threads

- `ProofSink` trait is still an AGENTS.md stretch goal — would let
  `Search` hide the `Sender` and make unit testing trivial.
- Worker index is `HashMap<String, usize>` over UCI paths; a compact
  key needs a `Hash`/`Ord` `Move` (upstream) or a packed move code.

## Cross-references

- `proof/` consumed this protocol and later pivoted it to the offline
  reconstruction builder; `search` itself no longer spawns the worker
  (proof plan7).
