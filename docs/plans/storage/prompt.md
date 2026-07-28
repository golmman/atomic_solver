## Core problem

Transposition table entries are volatile so we face problems in these areas:

- Providing proof for an outcome
- Extracting PPVs and SPPVs
- Search monitoring and debugging

## Idea

**Basics:**

- during search build dedicated tree with proven nodes
- on key press 'q'
  - if outcome was found already, traverse proof tree to find a ppv
  - save the proof tree as a postgres db
    - via ltree
    - path = uci-moves?
    - all AND-nodes, only "shortest" OR-nodes

The ltree-postgres db can then also be analyzed by external tools.

**Stretch goals:**

- proof tree operations run in dedicated thread
- mpsc queue for all operations

## Testability

PPVs should be easily extractable from the proof tree and can be verified via `verify_ppv`.

More plausibility checks and tests can be done by analyzing the proof tree (tbd).

## Task

Help me formulate a concept.

I am aiming for an mvp-solution:

- build the core feature first
- test it extensively
- extend it later

Propose a rough roadmap: which sub-features need to be build in what order?

Ask questions until ambiguity is at an acceptable level. Questions should come with options and their tradeoffs and should be numerated like 1a, 1b, 1c, 2a, 2b, ... .
Push back where necessary.
Write the results to `docs/plans/storage/concept.md`.
