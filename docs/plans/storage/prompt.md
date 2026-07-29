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

---

I feel we need to increase the Testability of the mvp phases.

Core idea: not only hitting q+return but also the timeout triggers the pre-exit hook.
That way a coding agent can easily verify the trigger functionality by waiting for the timeout.

Proposal where each phase has a testable result:
* Phase 1: --outcome-only flag and timeout/q-trigger -> simple log
* Phase 2: timeout / q -> dump dummy/test postgres ltree
* Phase 3: add worker thread and proof tree creation, on timeout / q -> log simple proof-tree statistics
* Phase 4: timeout / q -> extract PPV from proof-tree and add to the logged statistics
* Phase 5: timeout / q -> export proof tree to db

What do you think?


---

My idea adressing the unbounded memory risk:
* new option --pt-size (pt = proof tree) with a default of 256mb
* when exceeded: execute pre-exit hook and exit
* could be added in phase 3

Answers to the open questions:
1. on `q` the pre-exit hook is executed, then the program quits
2. `mpsc` should be sufficient for now, evaluation for more sophisticated techniques comes after the mvp
3. i'd prefer `src/proof_tree/` since it runs in another thread and this decoupling should be reflected in the directory structure
4. partial trees are fine for the mvp, no `complete` boolean for now

Again, push back if necessary.

With these clarifications, are there any open questions that need answers before we go into implementation plan creation stage?
