# AGENTS.md

## Goal

A pure solver for atomic chess in Rust.

## Architecture

- `src/lib.rs` re-exports `notation`, `position`, `proof_event`, `proof_tree`, `search`, `reconstruct`, `tt_snapshot`, and `zobrist`.
- `src/position.rs` wraps `atomic_movegen::board::Board` and tracks the `Outcome` (Win/Loss/Draw from the side-to-move perspective), undo state, and Zobrist hashing.
- `src/proof_event.rs` defines the neutral `ProofEvent` protocol (`Clear`,
  `NodeProven`) that decouples the solver from the proof-tree implementation.
- `src/search/dfpn/` — sequential DF-PN+ solver: iterative bounded refinement, history/killer
  heuristics, 5 s default timeout; emits `ProofEvent`s for every proven/disproven node (the
  returned PV is informational, from the TT; proof-tree finalization is the proof-tree layer's
  job). Refinement rounds are deterministically work-capped (`set_refine_cap_factor` /
  `--refine-cap`, default 0.25, `0` disables); searches can instead be bounded by cumulative
  child evals (`set_child_eval_budget`: budget-exhausted → `Draw` +
  `ExitReason::BudgetExhausted`, never `Timeout`). Hot-path terminal classification, pooled
  movegen slots (`children.rs`), and the per-run repetition-draw cache (`repetition_cache.rs`)
  are documented in their own module docs.
- `src/search/preflight/` — detector-gated bounded pre-phase (plan13, architecture R): on
  ≤3-men, pawnless, no-castling roots it decides the value via a region-closure AND/OR fixpoint
  plus a mandatory replay verifier; a claim returns the exact-DTM principal-line PV, anything
  else defers and the search runs bit-identically. Its soundness contract — no TT interaction,
  no wall clock, no `ProofEvent`s (the documented R2 gap), budget accounting, and the
  `REGION_BUDGET`-bounded closure memory (the one bounded exception to the search CLI's
  "RAM = TT only") — is normative in the module header.
- `src/search/tt/` — transposition table with path-independent base entries;
  repetition-dependent results are not cached (first-player-loss GHI shortcut).
- `src/search/ordering.rs` — the `MoveScorer` trait and `StaticAtomicScorer`.
- `src/proof_tree/` — `Move`/hash-based proof tree plus a background worker consuming
  `ProofEvent`s under a memory budget; `finalize()` copies canonical subtrees onto unexpanded
  transpositions, validates the tree (replay-based `validate.rs`), and serializes it to a
  compact binary dump (`binary.rs`, importable into PostgreSQL). Construction is an offline
  step: search → TT snapshot (`--tt-dump-path`) → `src/reconstruct` /
  `examples/reconstruct_pt` (worker + finalize + validator) → dump; the search CLI never
  builds trees. `Search::set_memory_limited` / `ExitReason::MemoryLimit` are a
  reconstruct-side contract only (`src/reconstruct/walker.rs`).
- `src/zobrist.rs` — deterministic Zobrist keys, including the halfmove clock;
  `src/notation.rs` — UCI move helpers, including `moves_to_uci_path`.
- `src/main.rs` — the CLI (`--fen`, `--tt-size`, `--epsilon`, `--timeout`, `--first-outcome`,
  `--refine-cap`, `--outcome-only`, `--tt-dump-path`, `--no-preflight`, `-h`; unknown options
  exit with an error). Resource-bounded: RAM = TT only; it never builds proof trees and prints
  no proof-tree lines. The full option, output (`pre_exit:` / `preflight:` / `pv_status:`
  lines), and offline-proof contracts are in the module header.
- `examples/` — example binaries (below); `tests/` — integration/regression tests.

## Dependency direction

- `search` depends only on `proof_event`; it does not know about `proof_tree`.
- `proof_tree` depends on `proof_event` and consumes `ProofEvent` messages.
- `proof_tree` knows nothing about `search`.
- A future `ProofSink` trait (stretch goal) can hide the `Sender` from `search`
  and make unit testing with a `Vec`-collecting sink trivial.

## Examples

`examples/common.rs` holds shared helpers and is not runnable. Runnable:

- `benchmark` — reproducible benchmark harness (`--suite default|move-order|decisive|quick|thorough|all`, `--runs`, `--timeout`, `--epsilon`, `--tt-size`, `--first-outcome`, `--config`, `--json`, `--output-file`; `--json` feeds the external optimizer).
- `chunk_growth` — work-chunk growth settings vs. node counts.
- `find_winning_child` — solves every first-move child; reports the winning root move.
- `egtb_gen3` — 3-man atomic WDL tablebase generator prototype: value iteration over `atomic-movegen` semantics, raw WDL dump, solver cross-validation, and an independent depth-limited proof oracle; exit 1 on any mismatch (`--material q|r|b|n|p|all`, `--out`, `--samples`, `--prove-samples`).
- `inspect_pt` — dump `proof_tree.bin` to JSON; `--validate` runs the replay validator and exits non-zero on defects.
- `list_legal` — all legal UCI moves and the terminal outcome for a FEN.
- `pt_keys` — dump each proof-tree node's replayed Zobrist key + outcome + depth
  (`solve` initiative measurement helper for the M1 metric; the binary tree dump
  format stores no hashes, so keys are recomputed by replay).
- `move_order_debug` — static/history/killer/total ordering scores (`--name <case>`).
- `play_and_solve` — play a given move, then solve the resulting position.
- `reconstruct_pt` — rebuild a proof tree offline from a FEN + TT snapshot (`--snapshot`); reports `validate: ok|FAILED n`, exits non-zero on defects; `--oracle` and `--experiment` run the dual-build oracle.
- `replay` — replay a UCI line from a FEN, then solve the resulting position.
- `solve_depth_limited` — fixed-`max_depth` search without the iterative-deepening bootstrap.
- `static_move_scores` — sorted `StaticAtomicScorer` values (`--name <case>`).
- `twin_stats` — TT statistics for GHI-sensitive positions.
- `verify_ppv` — verify a supplied UCI move list as a PPV for a FEN.

## Output priorities

1. **Decisive outcome** for deep positions (~30 full moves / 60 plies or more).
2. **Informational PV** from `Search::solve` — best-effort from the TT, not validated as a proof.
3. **Proof tree dump** (`proof_tree.bin`) from offline reconstruction (`reconstruct_pt` over a TT snapshot; the search CLI never builds a tree), finalized onto transpositions; PPV extraction and validation are the proof-tree layer's job.

`Search::solve` returns the first decisive line, then uses the remaining budget to shorten the PV (`Search::first_outcome_only` / `--first-outcome` skips this). `Search::pv_status()` qualifies the PV *length* (never its validity as a proof): `ProvenShortest` (last bounded refinement round exhausted naturally at `bound = pv_len - 2`, or the win is 1 move), `FirstOutcome`, `Unproven` (cap- or resource-cut round), `PreflightProof` (replay-verified pre-phase certificate, length = exact DTM), or `None`. The proof tree is never cleared automatically; the root FEN is fixed for the lifetime of the program.

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

`perf` works for per-process profiling of the container's own processes
(host-side launcher flags: `CAP_PERFMON`, `SYS_PTRACE`, `seccomp=unconfined`,
`label=disable`); verified 2026-09-08 on the release build.

- Works: `perf record -e cpu-clock -g -- <cmd>` (sampling with call graphs), `perf stat -e task-clock,context-switches,page-faults`, `perf report`, `perf top`, `perf trace`.
- Not available: hardware PMU events (`cycles`, `instructions`, cache/branch counters — the guest has no virtual PMU) and system-wide/CPU-wide profiling; `cpu-clock` software sampling is the ceiling.
- Kernel symbols do not resolve (`kptr_restrict`); user-space attribution is unaffected.
- The release build omits frame pointers: prefer leaf attribution (`perf report --no-children`), or `perf record --call-graph dwarf` / `RUSTFLAGS=-Cforce-frame-pointers=yes` for full call chains.

```bash
perf record -e cpu-clock -g -o /tmp/opencode/prof.data -- \
  target/release/atomic_solver --fen '<FEN>' --timeout 6 --first-outcome --outcome-only
perf report -i /tmp/opencode/prof.data --stdio --no-children
```

If these commands start failing (`EPERM`/`EACCES` on event open), the host-side launcher flags have regressed — the launcher lives outside this repo, so the fix is launcher-side; nothing in-repo can grant the missing permissions. This section is the only in-repo record of that setup; keep it in sync if the launcher changes.

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
  - measurements (performance etc.) go to `docs/plans/<initiative>/measurements/`
  - an initiative may be described by an `initiative.md`
  - `docs/plans/README.md` is the status index of all initiatives; update
    the affected row only when an initiative opens, pivots, or closes —
    the per-initiative `initiative.md` stays authoritative
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
- Literature references are indexed in `docs/bibliography.md` (status +
  pointer per paper). Analytical mining files (`research_*.md`) live in
  the initiative directories, written by the plan that mines the paper;
  when a plan mines an entry, update its status in the bibliography.
  Paper originals (PDF) and full-text extractions are vendored under
  `docs/theory/<slug>/` (slug = algorithm-name-year; see
  `docs/theory/README.md`) — never vendor PDFs inside `docs/plans/`;
  link to the theory library instead.
- you can ignore `docs/notes.md`: the users ideas and unfiltered notes go there
- Boy Scout principle: you should leave the codebase as clean or cleaner than you found it

## Conversational Guidelines

- You are not just a simple coder but a consultant for the user
- Push back if the users ideas or tasks are not sound or need clarification
- Feel free to ask questions where decisions are needed
- Explain the trade-offs for decision options

## File size justifications

- `src/search/preflight/mod.rs` and `region.rs` — the pre-phase's soundness contract (mod.rs header) plus the region closure, packed-key codec, exact-rank fixpoint, and strategy/PV extraction share one indexing scheme; tests are split out (`preflight/tests.rs`, `verifier.rs` test module).
- `src/proof_tree/worker.rs` — the full worker state machine (threaded handle, event loop, path traversal, dummy-node reconciliation, canonical finalization, memory accounting); splitting would fragment shared fields.
- `src/search/ordering.rs` — the complete `StaticAtomicScorer` heuristics and their co-tuned constants; tests are split out (`ordering/tests.rs`).
- `src/search/dfpn/children.rs` — `ChildPrecompute` pooled frame movegen slots, existence-query terminal classification, TT reuse, and proof-event emission share one `Position` move/undo sequence; slot invariants live next to the owning type.
- `src/search/dfpn/selection.rs` — OR/AND selection and best/second-unsolved search over the `ChildInfo` table; tests are split out (`selection/tests.rs`).
- `src/main.rs` — self-contained CLI: argument parsing, help text, search setup, and pre-exit hook in one place.

## Tuning workflow

`docs/spec/optimizer_interface.md` defines how an external optimizer evaluates candidate `ScorerParams` by invoking `benchmark --json`. The contract is narrow: `atomic_solver` provides the evaluator (`--suite quick` / `--suite thorough`), validates the TOML config, and returns raw metrics; the optimizer owns baselines, parameter space, TOML mapping, projection, and the scalar loss. Prefer `child_evals` (deterministic) as the efficiency metric; `WRONG_PENALTY` must dominate the loss (correctness first).
