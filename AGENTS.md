# AGENTS.md

## Goal

A pure solver for atomic chess in Rust.

## Architecture

- `src/lib.rs` re-exports `notation`, `position`, `proof_event`, `proof_tree`,
  `search`, and `zobrist`.
- `src/position.rs` wraps `atomic_movegen::board::Board` and tracks the
  `Outcome` (Win/Loss/Draw from the side-to-move perspective), undo state,
  and Zobrist hashing.
- `src/proof_event.rs` defines the neutral `ProofEvent` protocol (`Clear` and
  `NodeProven`) that decouples the solver from the proof-tree implementation.
  `NodeProven` carries a `Vec<Move>` path, the position Zobrist hash, the
  proven `Outcome`, and a depth.
- `src/search/dfpn/` implements the sequential DF-PN+ solver with iterative
  bounded refinement, history/killer heuristics, and a 5-second default
  timeout. `dfpn` emits `ProofEvent` nodes for every node it proves or
  disproves; the returned PV is an informational best-effort line from the
  transposition table and is not guaranteed to be a valid proof. The solver
  never clears proof events; proof-tree finalization is the responsibility of the
  proof-tree layer. Searches can additionally be bounded by cumulative child
  evaluations via `Search::set_child_eval_budget` (deterministic alternative
  to the wall-clock timeout, used by the test tiers): a budget-exhausted
  search returns `Draw`, stores only unsolved TT entries, and reports
  `ExitReason::BudgetExhausted` — never `ExitReason::Timeout`, which stays
  exclusively about wall time.
- `src/search/tt/` holds the transposition table with path-independent base
  entries. Repetition-dependent results are not cached, following the
  first-player-loss GHI shortcut.
- `src/search/ordering.rs` provides the `MoveScorer` trait and the
  `StaticAtomicScorer`.
- `src/proof_tree/mod.rs` provides a `Move`- and hash-based in-memory proof
  tree and a background worker that consumes `ProofEvent` messages, maintains
  the tree, enforces a memory budget, and serializes the full proven subtree
  to a compact binary adjacency dump (`src/proof_tree/binary.rs`, version 2).
  Each `ProofNode` carries the Zobrist hash of its position and the recorded
  real `work` (the cumulative `child_evals` spent proving its subtree, emitted
  at prove time by the search); the worker's `finalize()` pass copies fully
  expanded canonical subtrees onto unexpanded transpositions, making the tree
  authoritative without a transposition-table reconstruction step. The worker
  exposes `ProofTreeWorkerHandle` with `event_sender()`, `stats()`, `tree()`,
  `finalize()`, and `dump_to_bin()` for querying. External tools can import
  the binary dump into PostgreSQL.
- `src/zobrist.rs` generates deterministic Zobrist keys for positions,
  including the halfmove clock for transposition-table lookup.
- `src/notation.rs` provides UCI move helpers, including `moves_to_uci_path`
  for converting a `Vec<Move>` path into the tree's string key format.
- `src/main.rs` is the CLI entry point. It accepts `--fen <FEN>` (default
  standard start position), `--tt-size <MB>` (default 64), `--epsilon <VALUE>`
  (default 0.125), `--timeout <SECONDS>` (default 5), `--first-outcome`
  (stop after the first decisive line without iterative shortest-PV refinement),
  `--outcome-only` (disables the pre-exit hook and stdin reader), `--pt-size <MB>`
  (default 256, max in-memory proof-tree size), `--dump-path <FILE>`
  (default `proof_tree.bin`, binary dump of the full proven subtree), plus
  `-h`/`--help`. Unknown options exit with an error. It prints the outcome and
  an informational PV when the result is decisive and, by default, logs
  proof-tree statistics and writes the binary dump before exit.
- `examples/` contains example binaries for exploring solver behavior.
- `tests/` contains integration/regression tests.

## Dependency direction

- `search` depends only on `proof_event`; it does not know about `proof_tree`.
- `proof_tree` depends on `proof_event` and consumes `ProofEvent` messages.
- `proof_tree` knows nothing about `search`.
- A future `ProofSink` trait (stretch goal) can hide the `Sender` from `search`
  and make unit testing with a `Vec`-collecting sink trivial.

## Examples

`examples/common.rs` provides shared helpers for the example binaries; it is
not itself a runnable example.

The runnable examples are:

- `benchmark` — Reproducible benchmark harness over a fixed suite of positions.
  Supports `--suite default|move-order|decisive|quick|thorough|all`, `--runs`,
  `--timeout`, `--epsilon`, `--tt-size`, `--first-outcome`, `--config`, `--json`,
  and `--output-file`. Prints a table by default and, with `--json`, emits a JSON
  document suitable for an external optimizer.
- `chunk_growth` — Explore work-chunk growth settings and their effect on
  node counts.
- `corpus_gen` — Gate-1 corpus generation for the learned move-ordering
  concept. `solve` runs the quick/decisive suites at fixed deterministic
  settings and writes one `proof_tree.bin` dump plus a manifest per case;
  `load` replays the dumps, derives one row per expanded non-leaf node
  (`hash`, `source`, `fen`, `stm`, `outcome`, `depth`, `subtree_size`,
  `legal_moves`, `static_scores`, `children`, `first_decisive_rank`,
  `partial`), deduplicates by Zobrist hash, and emits NDJSON for the external
  trainer. Since design B (`docs/plans/nn/plan4.md`) every `children[]` entry
  carries the recorded real `work` (`child_evals` spent proving that child's
  subtree) from the v2 dump, and the corpus version is `atomic-corpus/2`: the
  AND label is "rank the children by `work`". The move-order cases m23+ are
  part of the `quick` suite and therefore train; only the m20–m22 move-order
  cases are held out for evaluation.
- `find_winning_child` — Enumerates every legal first move, solves the resulting
  child with a short timeout, and reports the first move that is winning for
  the root side (a child `Loss`).
- `inspect_pt` — Dump a binary `proof_tree.bin` to human-readable JSON.
- `list_legal` — List all legal UCI moves and the terminal outcome for a FEN.
- `move_order_debug` — Print static, history, killer, and total move-ordering
  scores for every legal move. Use `--name <case>` to inspect a move-order
  benchmark position.
- `move_order_fractions` — Gate 0 measurement for the learned move-ordering
  concept: solves positions and reports, for every OR (Win) node in the
  finalized proof tree, the rank of the proven decisive child under the static
  ordering, flat and work-weighted by subtree size. Supports `--fen`,
  `--suite move-order|decisive|all`, `--timeout`, `--epsilon`, `--tt-size`,
  and `--pt-size`.
- `play_and_solve` — Plays a user-specified move and then solves the resulting
  position. Useful for inspecting a particular line.
- `replay` — Replay a UCI line from a FEN and solve the resulting position.
- `solve_depth_limited` — Runs `Search::search_depth` with a fixed
  `max_depth` and no iterative-deepening bootstrap.
- `static_move_scores` — Prints the `StaticAtomicScorer` values for all legal
  moves, sorted from highest to lowest. Use `--name <case>` to inspect a
  move-order benchmark position.
- `twin_stats` — Report transposition-table statistics for GHI-sensitive
  positions.
- `verify_ppv` — Verifies that a supplied UCI move list is a Proof Principal
  Variation for a given FEN.
- `work_proxy_ablation` — Ablation (nn plan3 design A, re-measured with the
  design-B recorded-work ground truth) measuring whether the corpus's
  `subtree_size` label proxies the solver's real per-child work: at every AND
  (Loss) node with ≥ 2 children in the finalized proof tree it ranks children
  by `ProofNode.work` (the `child_evals` recorded at prove time) and reports
  the pair flip rate, Kendall τ, top-child agreement, and work-weighted flip
  share, plus a TT cross-check (`Search::tt_work_for`) with per-case coverage
  and `tt_agree`. Supports `--fen`, `--suite quick|decisive|all`, `--timeout`,
  `--epsilon`, `--tt-size`, and `--pt-size`.

## Output priorities

When the solver must trade off result quality against time or implementation
complexity, prefer them in this order:

1. **Decisive outcome** for deep positions (roughly 30 full moves / 60 plies or
   more).
2. **Informational PV** returned by `Search::solve` as a best-effort line from
   the transposition table. It is not validated as a proof.
3. **Proof tree dump** (`proof_tree.bin`) produced by the worker's `finalize()`
   pass. The authoritative in-memory tree carries Zobrist hashes and copies
   fully expanded canonical subtrees onto unexpanded transpositions before the
   dump is written. PPV extraction and validation are handled separately by
   the proof-tree layer.

`Search::solve` returns the first decisive line quickly, then uses the
remaining time budget to iteratively improve the informational PV. Use
`Search::first_outcome_only` (or the CLI `--first-outcome` flag) to skip
refinement when only a decisive outcome is needed. The proof tree is never
cleared automatically and the root FEN is fixed for the lifetime of the
program.

## Testing tiers

The test suite is split into tiers. Test selection is orthogonal to the build
profile: slow tests are marked with plain `#[ignore = "slow: ..."]` attributes,
never with `#[cfg_attr(debug_assertions, ignore)]`.

- `make test` — the default fast gate. `CARGO_PROFILE_RELEASE_LTO=thin
  cargo test --release`; runs all unit tests plus every fast integration test
  (all `#[ignore]`d tests are skipped). Target: < ~60 s of test time on the
  reference host (compile time excluded).
- `make test-full` — everything, including the 60 s wall-clock
  regression/stress suites (`cargo test --release -- --include-ignored`;
  ~25 min). Required for search, move-ordering, TT/GHI, and proof-tree changes,
  and before releases. Pre-commit hooks must never run `make test-full`.
- `make test-lite` — debug build (`cargo test`) for quick logic checks.

There is no CI (project decision): the make targets plus these conventions are
the enforcement point. Regressions caught only by the slow tier surface when
someone chooses to run it.

## Conventions

- Follow standard Rust 2024 edition idioms.
- Use `cargo clippy`, `cargo fmt`, `cargo test`, and `cargo doc` to ensure
  correctness and code quality.
- Avoid `unsafe` by default; prefer safe Rust. If `unsafe` is needed for a
  measurable performance win, document it clearly and guard it appropriately.
- Name public API types and functions clearly; prefer full words over
  abbreviations. Existing public modules use domain-standard abbreviations
  such as `dfpn`, `tt`, and `zobrist`; prefer full words for new public API
  unless the abbreviation is domain-standard.
- Example binaries go under `examples/`.
- Keep source files under ~10 KB. Files larger than 10 KB must include a short
  documented justification in the file header or in `AGENTS.md`. Files larger
  than ~20 KB should normally be split into submodules.
  - this limit does not hold for `docs/`
- Unit tests go in a `#[cfg(test)] mod tests` at the bottom of each module.
  Integration/regression tests go under `tests/`.
- Slow tests are marked with `#[ignore = "slow: ..."]` and are excluded from
  the default gate (`make test`); run them with
  `cargo test --release -- --include-ignored`. Do not reintroduce
  `#[cfg_attr(debug_assertions, ignore)]` — the build profile must not select
  which tests run.
- The most important quality attributes for this project are (highest priority first):
  - correctness
  - performance
  - efficient memory usage
  - maintainability
  - testability
  - consistency
- Only use reading `git` commands, never writing ones (no `git add`,
  `git rm`, `git commit`, etc.).
- `docs/plans/` contains prompts, implementation plans and reports
  - ignore all `prompt.md` files
  - implementation plans can be found via `find . -type f -name 'plan*.md'`
  - implementation reports can be found via `find . -type f -name 'report*.md'`
  - implementation plans should always be self contained so they can be implemented i a seaparate session
  - the final task of an implementation plan is creating the corresponding implementation report
  - a report should include additional tools/examples used, problems encountered, unresolved parts, missing tests, next steps
  - older plans and reports may not reflect the current state of the application or its goals
- Boy Scout principle: you should leave the codebase as clean or cleaner than you found it

## Conversational Guidelines

- You are not just a simple coder but a consultant for the user
- Push back if the users ideas or tasks are not sound or need clarification
- Feel free to ask questions where decisions are needed
- Explain the trade-offs for decision options

## File size justifications

- `src/proof_tree/worker.rs` is larger than the 20 KB guideline because it
  contains the full proof-tree worker: the threaded handle, event loop,
  `find_or_create_node` path traversal, dummy-node reconciliation, canonical
  finalization, and memory accounting. Splitting it further would fragment the
  state machine and the shared `ProofTreeWorker` fields.
- `src/search/ordering.rs` is larger than the 10 KB guideline because it holds
  the complete `StaticAtomicScorer` move-ordering heuristics (kamikaze, threats,
  atomic SEE, pawn-storm, rook centralization, and back-rank bonuses) and the
  constants that are tuned together. The unit tests are split out into
  `src/search/ordering/tests.rs` to keep the main file under the 20 KB limit.

## Tuning workflow

The optimizer interface contract in `docs/spec/optimizer_interface.md` defines
how an external optimizer can evaluate candidate `ScorerParams` by invoking the
`benchmark` example with `--json`. The contract is intentionally narrow:

- `atomic_solver` provides the evaluator (`--suite quick` and `--suite thorough`),
  validates the TOML config, and returns raw metrics as JSON.
- The optimizer is responsible for generating its own baselines, choosing which
  `ScorerParams` to vary, mapping the optimizer's parameter space onto the TOML
  format, projecting invalid proposals back into the valid region, and computing
  a scalar loss.

Use `child_evals` as the preferred deterministic efficiency metric and ensure
that any `WRONG_PENALTY` dominates the loss, reflecting correctness as the
highest priority.

## External NN trainer (Gate 2)

The move-ordering-network trainer is an external Python/PyTorch toolchain that
runs in its own Docker container and must not depend on the Rust toolchain.
The authoritative documents are `docs/spec/nn.md` (features, architecture,
weight-file layout), `docs/plans/nn/plan_external_trainer.md` (Gate 2
implementation plan), and `docs/plans/nn/trainer_handoff.md` (setup handoff:
disposable container with the trainer's own repo rw-mounted, `uv`-only image,
repo-local venv/cache; the in-repo bootstrap is pinned by the plan). The
training corpus
`data/corpus/train.ndjson` (`atomic-corpus/2`) is git-ignored; regenerate it
with `make nn_corpus`.
