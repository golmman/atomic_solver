# Cleanup Report 4 — post-plan13 housekeeping

Executed `plan4.md` on top of commit `2bca731` (plan4 committed as `67a6af1`).
No game-theoretic behavior change: the quick-suite benchmark before/after the
Task-5 lint fixes is byte-identical apart from timing fields.

## Task 1 — docs/bookkeeping sync

- `docs/plans/README.md`: `dfpn` row now records #6/plan13 as done
  (`report13.md`) with the open levers (#3 refinement after cap-cut, the
  threshold-cut-frame observation); the `egtb` row moved from **Active** to
  **Dormant** with the refocus note (decision 2 below).
- `docs/plans/egtb/initiative.md`: `## Status` rewritten (plan1 done, NO-GO,
  refocused-to-dormant, next plan number **plan2** if reopened); backlog #1's
  "pivot-vs-close decision pending" resolved; a decision-record entry dated
  2026-09-16 added. The open decisions (4-man coverage, DTZ granularity, #5
  anchoring) stay as recorded reopen triggers.
- `docs/plans/cleanup/initiative.md`: pedantic-lint triage and the
  `selection.rs` split removed from "Open follow-ups" (both addressed here);
  plan4 added to the done list.

**Decision 2** (egtb pivot-vs-close) was applied as written in the plan:
refocus-to-dormant — the generator stays as the `egtb_gen3` correctness
oracle, the 4-man faster-solves goal stays closed.

## Task 2 — AGENTS.md condensation (413 → 200 lines)

`## Conventions` is byte-identical (verified by section extraction before/after);
`## Testing tiers` and `## Conversational Guidelines` are also untouched.
Resulting section sizes: Goal 4, Architecture 42, Dependency direction 8,
Examples 19, Output priorities 8, Testing tiers 20, Profiling 19, Conventions 59,
Conversational Guidelines 7, File size justifications 9, Tuning workflow 3.

### Relocation table (fact → module doc where it now lives)

| AGENTS-only fact | Home |
| --- | --- |
| Refine-cap factor semantics (`max(1,000,000, factor × first-outcome evals)`, `--refine-cap`, `0` disables) | `src/main.rs` CLI help + `Search::set_refine_cap_factor` doc (`src/search/dfpn/mod.rs`) |
| `child_eval_budget` / `ExitReason::BudgetExhausted` contract (Draw result, unsolved-only TT stores, never `Timeout`) | `Search::set_child_eval_budget` doc (`src/search/dfpn/mod.rs`) |
| Preflight R2 gap (no `ProofEvent`s ⇒ offline pipeline cannot reproduce pre-phase proofs) | `src/search/preflight/mod.rs` soundness contract |
| Pre-phase closure as the bounded exception to "RAM = TT only" | `src/search/preflight/mod.rs` + `src/main.rs` help |
| `PvStatus` variant meanings (incl. `preflight-proof` = exact DTM) | `PvStatus` enum doc (`src/search/dfpn/mod.rs`) + `src/main.rs` output docs |
| Hot-path terminal classification / pooled movegen slots | `src/search/dfpn/children.rs` header |
| Repetition-draw cache invariants (keyed per run, never serialized) | `src/search/dfpn/repetition_cache.rs` header |
| `search_depth_with_prefix` skips the pre-phase | `Search::search_depth_with_prefix` doc (`src/search/dfpn/mod.rs`) |
| Twin-selection order per `(hash, outcome)` | `src/proof_tree/worker.rs` (`find_or_create_node` docs) |
| `pt_validate: FAILED` cap-20 + `ProofStats::validation_errors` + reconstruct-side exit-1 contract | `src/proof_tree/worker.rs` (finalize) + `examples/reconstruct_pt.rs` |
| TT snapshot solved-all-generations / unsolved-current-generation | `src/tt_snapshot/mod.rs` format docs |
| **Solver never emits `Clear`** (was nowhere else) | **added** to `src/proof_event.rs` module doc this session |
| Pre-phase closure memory figures | corrected in `src/search/preflight/mod.rs` + `src/main.rs` (see Task 4.2) |

Pre-exit/preflight/`pv_status` output-line formats remain in `src/main.rs`.

## Task 3 — `selection.rs` module sizing

Tests moved verbatim to `src/search/dfpn/selection/tests.rs` (precedent:
`ordering.rs` + `ordering/tests.rs`); `selection.rs` now ends with
`#[cfg(test)] mod tests;`. Post-split sizes: `selection.rs` **11,762 bytes**
(>10 KB — a file-size justification line was added to AGENTS.md),
`selection/tests.rs` 7,100 bytes. No logic changes; all 13 moved tests pass.

## Task 4 — preflight test gaps (report13)

1. **CLI e2e test** — `tests/test_cli.rs::cli_no_preflight_disables_prephase_on_gated_position`
   on the KQvK ladder root `8/2K5/k7/8/8/8/8/4Q3 w - - 0 1`:
   the default run prints `preflight: decided …` and `pv_status: preflight-proof`
   (win in 15 plies); the `--no-preflight` run must print
   `preflight: deferred reason=disabled` and neither a decided line nor a
   preflight-proof status.
   **Deviation from the plan's assertion**: the plan expected the
   `--no-preflight` run to print *no* `preflight:` line, but the CLI
   deliberately prints exactly one hook line unless `--outcome-only`
   (documented R5 behavior; the disabled pre-phase reports
   `deferred reason=disabled`). Changing that output would be a CLI behavior
   change, out of scope — the test pins the actual contract instead.
2. **Closure memory measurement** — `/usr/bin/time` is not installed in this
   container; used `os.wait4` (`ru_maxrss`) per child process instead, release
   build, default 128 MB TT, 5 s timeout:

   | run | max RSS |
   | --- | --- |
   | mate-in-1 baseline (`--no-preflight`) | 226.4 MiB |
   | ladder root `--no-preflight` (5 s search) | 227.2 MiB |
   | ladder root, pre-phase claim | 262.5 MiB |

   Closure delta ≈ **35 MiB** for the 420,532-position KQvK ladder region
   (~85 B/position ⇒ **~85 MB worst case at the 1M budget**). The documented
   "~60 MB on the ladder / ~100–120 MB worst case" claims were contradicted
   and corrected to the measured values in `src/search/preflight/mod.rs` and
   `src/main.rs`. No CI assertion added — the budget stays a documented bound.

## Task 5 — pedantic clippy triage

- `cargo clippy --all-targets -- -W clippy::pedantic`: **308 warnings → 167**
  after the mechanical pass (`cargo clippy --fix` over all targets: doc
  backticks, lossless `From` casts, `#[must_use]` additions, `map_or`, format
  strings; 41 files touched, mostly docs/attributes).
- Remaining 167 (per-target duplicate counting; 81 unique in the lib):
  deliberate truncating/precision casts in bitboard math (~75), missing
  `# Errors`/`# Panics` doc sections (20), `let…else` / clone-assign /
  wildcard-match style lints, and ~15 too-many-lines function-size notes.
  **Decision: left unfixed** — silencing them with scoped `#[allow]`s would
  itself be churn with no correctness or performance value; the cast lints are
  intentional in engine bit math and the doc-section lints need real content,
  not boilerplate. `clippy --all-targets` (default lints) stays clean.

## Gates

- `cargo fmt --check` — clean.
- `cargo clippy --all-targets` — 0 warnings.
- `cargo doc` — 0 warnings (closes report13's third item for good).
- `make test` — 348 passed / 0 failed (ignored tests excluded per tier rules).
- Benchmark drift check: `benchmark --suite quick --json --first-outcome`
  before vs. after the Task-5 fixes — all 59 cases identical on every
  non-timing field (outcome, nodes, child_evals, pv_len, wrong).
- AGENTS.md: 200 lines; `git diff` shows no hunks inside `## Conventions`
  (nor inside `## Testing tiers` / `## Conversational Guidelines`).

## Problems encountered

- `/usr/bin/time` missing (Task 4.2) — replaced with a Python `os.wait4`
  max-RSS wrapper; same metric.
- The Task-4 CLI expectation vs. the actual one-`preflight:`-line contract
  (see above) — test pins the documented behavior, no CLI change.
- The first cut of the condensed AGENTS.md landed at 209–243 lines in draft
  form; iterated wrapping (wider lines in the condensable sections, verbatim
  sections untouched) to reach exactly 200 without dropping facts.

## Next steps

- Remaining follow-up in this initiative: the `M19_FEN` / `STARTPOS_FEN`
  cross-file duplicates (only if they drift).
- The next feature session is expected to be `proof` #8 (PPV from the
  finalized tree), per the post-plan13 re-ranking; `dfpn` #3 and `lean`
  #7/#10 stay with their initiatives.
