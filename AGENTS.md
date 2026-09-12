# AGENTS.md

## Goal

A pure solver for atomic chess in Rust.

## Architecture

- `src/lib.rs` re-exports `notation`, `position`, `proof_event`, `proof_tree`,
  `search`, `reconstruct`, `tt_snapshot`, and `zobrist`.
- `src/position.rs` wraps `atomic_movegen::board::Board` and tracks the
  `Outcome` (Win/Loss/Draw from the side-to-move perspective), undo state,
  and Zobrist hashing.
- `src/proof_event.rs` defines the neutral `ProofEvent` protocol (`Clear` and
  `NodeProven`) that decouples the solver from the proof-tree implementation.
  `NodeProven` carries a `Vec<Move>` path, the position Zobrist hash, the
  proven `Outcome`, and a depth.
- `src/search/dfpn/` implements the sequential DF-PN+ solver with iterative
  bounded refinement, history/killer heuristics, and a 5-second default
  timeout. Each PV-refinement round is work-capped at
  `max(1,000,000, factor * first-outcome child evals)` (factor `0.25` by
  default, configurable via `Search::set_refine_cap_factor` / CLI
  `--refine-cap`; `0` disables capping) so a futile round is abandoned
  deterministically instead of running to the global deadline; per-phase
  counters are exposed via `first_outcome_evaluations()`,
  `refinement_rounds()`, and `refinement_evaluations()`. `dfpn` emits
  `ProofEvent` nodes for every node it proves or
  disproves; the returned PV is an informational best-effort line from the
  transposition table and is not guaranteed to be a valid proof. The solver
  never clears proof events; proof-tree finalization is the responsibility of the
  proof-tree layer. Searches can additionally be bounded by cumulative child
  evaluations via `Search::set_child_eval_budget` (deterministic alternative
  to the wall-clock timeout, used by the test tiers): a budget-exhausted
   search returns `Draw`, stores only unsolved TT entries, and reports
   `ExitReason::BudgetExhausted` — never `ExitReason::Timeout`, which stays
   exclusively about wall time. The hot path never generates move lists for
   terminal checks: `evaluate_child` decides child terminality with the
   upstream early-exit existence query (`Position::has_legal_move` over
   `atomic_movegen::movegen::has_legal_move_with_state`, fed by a
   caller-populated `StateInfo`) plus board-static classification
   (checkers bit, `occupied == 2`); no legal-move list is produced for the
   ~95% of evaluated children that are never searched. Only searched
   children generate their legal moves, once, into a frame-local pooled
   slot: the `dfpn` entry takes its slot from `Search::precompute_pool` at
   its own depth (one slot per active frame; the pool no longer scales with
   branching) and shares the node `StateInfo` with `sort_moves` instead of
   rebuilding it. Per-frame child vectors and the `sort_moves` score buffer
   are pooled on `Search`. Repetition-dependent draw proofs (the ones the
   first-player-loss shortcut keeps out of the TT) are additionally cached
   per search run in `src/search/dfpn/repetition_cache.rs`, keyed by
   (position hash, order-independent ancestor repetition-key context hash)
   and storing only `Draw` payloads; the cache is cleared once per run in
   `Search::begin_run` (never per chunk or refinement round) and is never
   serialized into TT snapshots or proof artifacts, so only work, never
   outcomes, changes.
- `src/search/tt/` holds the transposition table with path-independent base
  entries. Repetition-dependent results are not cached, following the
  first-player-loss GHI shortcut.
- `src/search/ordering.rs` provides the `MoveScorer` trait and the
  `StaticAtomicScorer`.
- `src/proof_tree/mod.rs` provides a `Move`- and hash-based in-memory proof
  tree and a background worker that consumes `ProofEvent` messages, maintains
  the tree, enforces a memory budget, and serializes the full proven subtree
  to a compact binary adjacency dump (`src/proof_tree/binary.rs`). The search
  CLI no longer builds trees — proof-tree construction is an offline step and
  the producer chain is: search → TT snapshot (`--tt-dump-path`) →
  `src/reconstruct` / `examples/reconstruct_pt` (worker + finalize +
  validator) → binary dump; tests and examples may still spawn the worker
  directly. Each
  `ProofNode` carries the Zobrist hash of its position; the worker's
  `finalize()` pass copies fully expanded canonical subtrees onto unexpanded
  transpositions, making the tree authoritative without a transposition-table
  reconstruction step. Twin selection per `(hash, outcome)` prefers, in
  order: consistency, shallower proven depth, more children (a completeness
  proxy for Loss twins), then first created. The worker validates the
  rebuilt tree with the replay-based validator in `src/proof_tree/validate.rs`
  (`validate_proof_tree`) during `finalize()`, prints one
  `pt_validate: FAILED ...` stderr line per defect (capped at 20) and records
  the count in `ProofStats::validation_errors` (0 on success; a non-zero
  count must fail the run — the dump is still written as the debugging
  artifact; the exit-1 contract is enforced by the reconstruct-side
  producers, e.g. `reconstruct_pt`, not by the search CLI). The validator
  re-plays every path on a real `Position` and
  checks the structural rules of a proof (Loss covers *all* legal replies,
  Win has exactly one winning child, depths bottom-up consistent, terminals
  statically correct); it therefore adds a `proof_tree → position`
  dependency (position is a base layer below `search`, so the
  `search`/`proof_tree` decoupling is unchanged). The worker exposes
  `ProofTreeWorkerHandle` with `event_sender()`, `stats()`, `tree()`,
  `finalize()`, and `dump_to_bin()` for querying.
  External tools can import the binary dump into PostgreSQL. Note that
  `Search::set_memory_limited` / `ExitReason::MemoryLimit` are not wired by
  the search CLI; they remain a reconstruct-side contract — the
  reconstruction walker sets the flag for the *builder's* budget and aborts
  local prefix-solves on it (`src/reconstruct/walker.rs`).
- `src/zobrist.rs` generates deterministic Zobrist keys for positions,
  including the halfmove clock for transposition-table lookup.
- `src/notation.rs` provides UCI move helpers, including `moves_to_uci_path`
  for converting a `Vec<Move>` path into the tree's string key format.
- `src/main.rs` is the CLI entry point. It accepts `--fen <FEN>` (default
  standard start position), `--tt-size <MB>` (default 128), `--epsilon <VALUE>`
  (default 0.125), `--timeout <SECONDS>` (default 5), `--first-outcome`
  (stop after the first decisive line without iterative shortest-PV refinement),
  `--refine-cap <FACTOR>` (default 0.25; per-refinement-round work-cap factor
  relative to the first-outcome child-eval count, `0` disables capping),
  `--outcome-only` (no stdin reader, no pre-exit summary),
  `--tt-dump-path <FILE>` (opt-in; writes a compact binary TT snapshot after
  the search — solved entries from all generations, unsolved from the current
  generation),
  plus `-h`/`--help`. Unknown options exit with an error. It prints the outcome
  and an informational PV when the result is decisive. The search CLI is
  **resource-bounded** (RAM = TT only): it never builds proof trees — no
  worker thread, no proof-tree memory budget, no `MemoryLimit` abort path —
  and prints no `proof_tree:`/`proof_tree_dump:`/`pt_validate:` lines. Proof
  construction is an offline step: `--tt-dump-path` produces the TT snapshot
  from which `examples/reconstruct_pt` (worker + finalize + replay validator,
  exit 1 on a defective tree) rebuilds the validated binary dump. The
  pre-exit hook reduces to the stdin reader (`q` quits) and one
  `pre_exit: reason=… outcome=… nodes=…` line. For decisive
  outcomes it also prints `pv_status: proven-shortest | first-outcome |
  cap-cut | cut-short`, reporting whether the PV length is proven minimal
  (the last bounded refinement round exhausted naturally, or the win is 1
  move) or why refinement stopped early; it qualifies the PV *length* within
  the solver's search semantics, not the PV's validity as a proof.
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
- `find_winning_child` — Enumerates every legal first move, solves the resulting
  child with a short timeout, and reports the first move that is winning for
  the root side (a child `Loss`).
- `inspect_pt` — Dump a binary `proof_tree.bin` to human-readable JSON;
  `--validate` additionally runs the replay-based proof validator on the
  loaded tree and exits non-zero on defects.
- `list_legal` — List all legal UCI moves and the terminal outcome for a FEN.
- `move_order_debug` — Print static, history, killer, and total move-ordering
  scores for every legal move. Use `--name <case>` to inspect a move-order
  benchmark position.
- `play_and_solve` — Plays a user-specified move and then solves the resulting
  position. Useful for inspecting a particular line.
- `reconstruct_pt` — Rebuilds a proof tree offline from the root FEN plus a TT
  snapshot (`--snapshot`), synthesizing events into the regular proof-tree
  worker; reports `validate: ok|FAILED n` for the reconstructed tree and
  exits non-zero on validation failure; `--oracle` compares against an
  event-built dump, and `--experiment` runs the go/no-go dual-build oracle
  over the decisive suite (per-case live-tree `validate` column included).
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

## Output priorities

When the solver must trade off result quality against time or implementation
complexity, prefer them in this order:

1. **Decisive outcome** for deep positions (roughly 30 full moves / 60 plies or
   more).
2. **Informational PV** returned by `Search::solve` as a best-effort line from
   the transposition table. It is not validated as a proof.
3. **Proof tree dump** (`proof_tree.bin`) produced by the worker's `finalize()`
   pass during offline reconstruction (`reconstruct_pt` over a TT snapshot —
   the search CLI never builds a tree). The authoritative in-memory tree
   carries Zobrist hashes and copies
   fully expanded canonical subtrees onto unexpanded transpositions before the
   dump is written. PPV extraction and validation are handled separately by
   the proof-tree layer.

`Search::solve` returns the first decisive line quickly, then uses the
remaining time budget to iteratively improve the informational PV. Use
`Search::first_outcome_only` (or the CLI `--first-outcome` flag) to skip
refinement when only a decisive outcome is needed. After a solve,
`Search::pv_status()` reports whether the returned PV is proven shortest
(PvStatus::ProvenShortest: the last bounded refinement round exhausted
naturally at `bound = pv_len - 2`, or the win is 1 move) or why not
(`FirstOutcome` / `Unproven`); this qualifies the PV length within the
solver's search semantics, not the PV's validity as a proof. The proof tree is
never cleared automatically and the root FEN is fixed for the lifetime of the
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

## Profiling in this container

`perf` is usable for per-process profiling of the container's own processes
(made possible by host-side launcher flags: `CAP_PERFMON`, `SYS_PTRACE`,
`seccomp=unconfined`, `label=disable`). Verified 2026-09-08 on the release
build.

- Works: `perf record -e cpu-clock -g -- <cmd>` (statistical sampling with
  call graphs), `perf stat -e task-clock,context-switches,page-faults`,
  `perf report`, `perf top`, `perf trace`.
- Not available: hardware PMU events (`cycles`, `instructions`, cache/branch
  counters) — the guest has no virtual PMU — and system-wide/CPU-wide
  profiling. `cpu-clock` software sampling is the profiling ceiling.
- Kernel symbols do not resolve (`kptr_restrict`); user-space attribution is
  unaffected.
- The release build omits frame pointers. Use leaf attribution
  (`perf report --no-children`) for hot-path work, or
  `perf record --call-graph dwarf` / `RUSTFLAGS=-Cforce-frame-pointers=yes`
  when full call chains are needed.

Typical hot-path session:

```bash
perf record -e cpu-clock -g -o /tmp/opencode/prof.data -- \
  target/release/atomic_solver --fen '<FEN>' --timeout 6 --first-outcome --outcome-only
perf report -i /tmp/opencode/prof.data --stdio --no-children
```

If these commands start failing (`EPERM`/`EACCES` on event open), the
host-side launcher flags have regressed — the launcher lives outside this
repo, so the fix is launcher-side; nothing in-repo can grant the missing
permissions. This section is the only in-repo record of that setup; keep it
in sync if the launcher changes.

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
  - each sub-directory in `docs/plans/` is treated as an initiative with a mutual high-level goal 
  - an initiative may be described by an `initiative.md`
  - initiative work is agile not waterfall
  - you can ignore all `prompt.md` files, which are unfiltered user-thoughts
  - implementation plans can be found via `find . -type f -name 'plan*.md'`
  - implementation reports can be found via `find . -type f -name 'report*.md'`
  - implementation plans should always be self contained so they can be implemented i a seaparate session
  - the final task of an implementation plan is creating the corresponding implementation report
  - a report should include additional tools/examples used, problems encountered, unresolved parts, missing tests, next steps
  - older plans and reports may not reflect the current state of the application or its goals
- Specifications in `docs/spec/` must be standalone documents: no references
  to `docs/plans/`, reports, or repo-internal process vocabulary (gate names,
  fixture/case names). They are normative contracts copied verbatim into
  external repos (e.g. the external optimizer reading
  `docs/spec/optimizer_interface.md`), where such references dangle.
  Rationale and history belong in `docs/plans/`; a spec may reference other
  files under `docs/spec/` only.
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
- `src/search/dfpn/children.rs` is larger than the 20 KB guideline because
  `ChildPrecompute` (the per-depth pooled frame movegen slots), the
  existence-query terminal classification in `evaluate_child`, TT reuse, and
  proof-event emission share one `Position` move/undo sequence; the slot
  reuse and staleness invariants are documented next to the type that owns
  them.

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
