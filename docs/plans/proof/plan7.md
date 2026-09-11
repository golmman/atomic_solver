# Implementation Plan: Flip the Default — Worker-Off Search CLI

Initiative: `docs/plans/proof/initiative.md`, backlog item #3 (**plan7**,
renumbered 2026-09-11; report5 originally called the flip "plan6"). This plan
is self-contained. It is un-gated: report6 landed the finalize
canonicalization fix and the replay validator, and
`reconstruct_pt --experiment` now reports **46/46 oracle-isomorphic** on the
decisive suite — the precondition report5 set for making the reconstructed
tree the *only* tree.

Two design decisions were recorded up front (2026-09-11, with the
maintainer):

1. **Full removal, no opt-in flag.** The live-tree path is deleted from the
   search CLI entirely (`--pt-size`, `--dump-path`, worker spawn, the fatal
   `ExitReason::MemoryLimit` branch). An opt-in `--live-proof-tree` flag was
   rejected: it would keep the fatal memory-limit path alive (the exact
   failure this initiative exists to remove) for a debugging convenience that
   the snapshot round-trip already covers (`--tt-dump-path` +
   `reconstruct_pt`, oracle-proven isomorphic 46/46).
2. **`--tt-dump-path` stays opt-in.** No auto-written snapshot. The solver's
   stdout/file contract stays minimal; multi-day runs pass the flag
   explicitly. Backlog #5 (periodic checkpoint) remains the separate crash-
   resilience lever.

## Goal

The search CLI becomes **resource-bounded**: RAM = TT only, no worker thread,
no proof-tree budget, no `MemoryLimit` abort path. Proof construction moves
entirely to the plan5 tool: `--tt-dump-path` + `reconstruct_pt --snapshot`.
The standard decisive run prints outcome/PV/pv_status and, on request, a TT
snapshot from which a validated proof-tree dump is rebuilt offline.

Success criteria:

- `make test` green, including the updated CLI tests and the new end-to-end
  workflow test.
- Drift check bit-identical: `benchmark --suite quick --json --first-outcome`
  per case (the benchmark never touches the worker; this must hold trivially —
  if it does not, something else changed and the plan is wrong).
- No search-path change of any kind: no `Search` API, emission, or ordering
  code touched.
- Solver stdout no longer contains `proof_tree:`, `proof_tree_dump:`, or
  `pt_validate:` lines; `pre_exit:` remains (see Design §2).

## Context (current state, verified 2026-09-11)

**What the CLI worker path owns today** (`src/main.rs`):

- `memory_limited` flag (line 182) and worker spawn (183–189,
  `ProofTreeWorkerHandle::spawn(fen, pt_size, …)`), skipped under
  `--outcome-only`.
- `search.set_memory_limited(...)` (243–247) and
  `search.set_proof_event_sender(...)` (248).
- The pre-exit hook (191–236): spawns the stdin reader (`q` quits), then on
  exit prints `pre_exit:`, `proof_tree:` stats, `proof_tree_dump:`,
  `pt_validate: ok|FAILED` (exit 1 on defects).
- The cut-short branch (307–317): `ExitReason::MemoryLimit` prints the
  proof-tree memory-limit error and `exit(1)`s.
- Worker teardown (`drop(pt_handle)`, `pt_join.join()`, 323–327).

**CLI parsing** (`src/cli.rs`): `pt_size` (field 32, default 256, parsing
162–169) and `dump_path` fields, plus unit tests referencing them (238, 302,
387).

**CLI tests** (`tests/test_cli.rs`): help-text assertion for `--dump-path`
(30–35), `cli_dump_path_writes_proof_tree_dump` (63), 
`cli_first_outcome_dumps_proof_tree` (93). The `--tt-dump-path` tests
(245–321) are unaffected and stay.

**What must NOT be removed** (library surface, still load-bearing):

- `Search::set_memory_limited` (`src/search/dfpn/mod.rs:397`),
  `ExitReason::MemoryLimit` (`mod.rs:46`, checked at :788 / :462–466), and
  `set_proof_event_sender` / `emit_proof_node` (`mod.rs:401/442`). The
  reconstruct walker (`src/reconstruct/walker.rs`) uses the flag for the
  *builder's* budget and aborts local prefix-solves on it (walker.rs:147) —
  `ExitReason::MemoryLimit` is unreachable from the search CLI after this
  plan but very much alive in reconstruction. Add a code comment saying so.
- The `ProofEvent` protocol shape — a non-goal of the initiative.
- `--pt-size` in `src/reconstruct` (`ReconstructConfig::pt_size_mb`,
  `reconstruct/mod.rs:159`) and `examples/reconstruct_pt.rs` — that is the
  **builder's** budget, not the search's; deep-proof capacity of the builder
  is backlog item #7, untouched here.
- Tests and examples spawn workers directly
  (`tests/test_proof_tree.rs`, `tests/test_proof_validate.rs`,
  `tests/reconstruct.rs`, `examples/reconstruct_pt.rs`) — unaffected.

**Downstream docs**: `docs/spec/proof_tree_dump.md` is producer-neutral
(format-only) and needs no change. `AGENTS.md`'s `main.rs` and proof-tree
sections describe the worker-spawning CLI and must be rewritten.

**Evidence the flip is safe** (report5/report6): hole rate 0.003%
(`absent` = 2/91,470 proof nodes), median fill F/C = 0.0, zero
anomalies/unfillable, 46/46 oracle-isomorphic after plan6 — the reconstructed
tree is a superset of (and now isomorphic to) the live tree.

## Design

### 1. CLI surface removal (`src/cli.rs`, `src/main.rs`)

- `CliOptions` loses `pt_size` and `dump_path`; `--pt-size` / `--dump-path`
  parsing and their help lines are deleted. They become **unknown options**
  and hit the existing unknown-option error path — consistent with the CLI
  contract that unknown options exit with an error.
- `main.rs`: delete the `memory_limited` Arc, the worker spawn, the
  `set_memory_limited` / `set_proof_event_sender` wiring, the proof-tree
  portions of the pre-exit hook, the `ExitReason::MemoryLimit` arm of the
  cut-short branch (now unreachable; keep `Quit`/`BudgetExhausted`/timeout
  handling), and the worker teardown. The `--tt-dump-path` block (284–305)
  is untouched.
- The pre-exit hook reduces to the stdin reader plus the `pre_exit:`
  line — quit/timeout observability is preserved without any proof-tree
  dependency.

**Documented stdout changes** (the drift protocol allows these if listed):

| Removed | Reason |
|---|---|
| `proof_tree: nodes=… win=… loss=… root_depth=…` | no live tree |
| `proof_tree_dump: <file>` | no live dump |
| `pt_validate: ok` / `pt_validate: FAILED n defect(s)` | validation moves to `reconstruct_pt` (same validator, exit 1 there) |

| Kept | |
|---|---|
| `outcome:` / `pv:` / `pv_status:` | unchanged |
| `pre_exit: reason=… outcome=… nodes=…` | kept (reduced hook) |
| `timeout` / `quit` / `budget exhausted` | unchanged |
| `tt_snapshot: <file> solved=… unsolved=… bytes=…` | unchanged (opt-in) |

`--outcome-only` semantics narrow slightly and are re-documented: *no stdin
reader, no pre-exit summary* (its proof-tree exclusion is now vacuous).

### 2. The standard workflow (docs)

Decisive run → reconstruction, replacing the live dump:

```
atomic_solver --fen <FEN> --timeout <S> --tt-dump-path tt.bin
reconstruct_pt --snapshot tt.bin --dump-path proof_tree.bin   # validate: ok, exit 1 on defects
inspect_pt --validate --dump-path proof_tree.bin              # optional post-hoc check
```

`docs/spec/proof_tree_dump.md` is unchanged (the format is the contract; the
producer changed). `AGENTS.md` documents the new producer chain: search →
TT snapshot → `reconstruct_pt` (worker + finalize + validator) → dump.

### 3. Tests

- **`tests/test_cli.rs`**: drop the `--dump-path` help assertion and the two
  live-dump tests; keep all `--tt-dump-path` tests as-is. Add: passing
  `--pt-size 64` or `--dump-path x.bin` exits non-zero with the
  unknown-option error (pins the removal; guards accidental re-adds).
- **New `tests/test_plan7.rs`** (fast tier): the end-to-end default workflow —
  spawn the CLI binary on the two-rook mate fixture with `--tt-dump-path`,
  assert the snapshot line, then run `src/reconstruct` over the snapshot (same
  pattern as `tests/reconstruct.rs`) and assert `validate: ok` /
  `validation_errors == 0` / exit success. This is the replacement gate for
  what the `pt_validate:` pre-exit hook used to guarantee.
- Existing worker/validator tests stay green unchanged (they never go through
  the CLI).

### 4. Docs

- **`AGENTS.md`**: rewrite the `main.rs` section (drop `--pt-size`,
  `--dump-path`, the pre-exit proof-tree summary and `pt_validate:` exit-code
  contract; state the worker-off default, the remaining stdout lines, and the
  offline reconstruction workflow); update the proof-tree bullet to say the
  search CLI no longer builds trees — the reconstruct path is the producer;
  note that `ExitReason::MemoryLimit` remains a reconstruct-side (builder
  budget) contract.
- **`examples/`** docs in AGENTS.md unchanged except where they describe the
  solver's live dump.

## Acceptance criteria

1. `make test` green, including updated `test_cli.rs` and the new
   `tests/test_plan7.rs`.
2. Drift check: `benchmark --suite quick --json --first-outcome --runs 1`
   byte-identical per case before/after (only wall-clock differs).
3. `cargo test --release --test test_cli` — unknown-option rejection for
   `--pt-size` / `--dump-path`, snapshot tests intact.
4. Manual (release): decisive solve with `--tt-dump-path` → `reconstruct_pt
   --snapshot` → `validate: ok` and a parseable dump; solver stdout contains
   none of the three removed lines.
5. `rg -n 'pt_size|dump_path|MemoryLimit' src/main.rs src/cli.rs` shows no
   search-CLI wiring (the `ExitReason` enum variant itself stays in
   `src/search/dfpn/mod.rs`).

## Scope

1. `src/cli.rs` — remove `pt_size`/`dump_path` fields, parsing, help, unit
   tests.
2. `src/main.rs` — remove worker wiring/hook-proof-tree parts/MemoryLimit
   arm; reduce hook; rewrite module docs and help text. (File stays under
   its current size; the header justification's "proof-tree wiring" wording
   is updated.)
3. `src/search/dfpn/mod.rs` — comment only (on `ExitReason::MemoryLimit` /
   `set_memory_limited`: reconstruct-side contract, not wired by the CLI).
4. `tests/test_cli.rs` — update as in §3.
5. New `tests/test_plan7.rs`.
6. `AGENTS.md`.
7. **Initiative update**: backlog #3 → done (plan7); History entry (see
   Final task).

## Explicitly out of scope

- **Library search API**: `set_memory_limited`, `set_proof_event_sender`,
  `emit_proof_node`, `ExitReason::MemoryLimit` — all stay; the `ProofEvent`
  protocol shape is a non-goal.
- **`src/reconstruct` / `examples/reconstruct_pt.rs`** — including its
  `--pt-size` builder budget and the `--experiment` harness.
- **Backlog #7** (builder capacity for deep proofs), **#5** (periodic TT
  checkpoint), **#4** (pinning) — unchanged.
- **Binary formats** (dump v1, TT snapshot v1).
- **`benchmark` and all other examples** — they never touch the worker.
- **`docs/spec/*`** — all specs stay verbatim-valid (the dump spec is
  producer-neutral).

## Verification

1. `cargo fmt --check`, `cargo clippy --all-targets`, `cargo doc` — clean.
2. `make test` (fast tier) — green.
3. Drift check (criterion 2).
4. Manual end-to-end (release): two-rook mate and one decisive-suite case
   (e.g. dec02) — solve with `--tt-dump-path`, `reconstruct_pt --snapshot …
   --oracle` against nothing (no live dump exists anymore — oracle mode is
   exercised only by `--experiment`, which builds its own live tree),
   `inspect_pt --validate` → ok.
5. `reconstruct_pt --experiment` (decisive suite, release, ~6 min): **not
   strictly required** (no proof-tree or search change), but run it once as
   cheap confirmation that 46/46 still holds and record the table in
   `report7.md`.
6. `rg` audit (criterion 5).

## Risks / notes

- **External consumers of solver-produced `proof_tree.bin` break.** Any
  script that read the dump from a solver run must switch to
  `--tt-dump-path` + `reconstruct_pt`. This is the initiative's purpose, not
  a regression; the migration path is one command and is documented in §2
  and in `report7.md`.
- **Validation gate relocation.** Defective proofs are now caught by
  `reconstruct_pt`'s `validate:` (exit 1) instead of the solver's pre-exit
  hook — the same validator, same defect classes, just at build time. No
  coverage is lost; the solver simply no longer ships a tree at all.
- **`ExitReason::MemoryLimit` becomes CLI-unreachable** but is required by
  the reconstruct walker (builder budget). The added comment + AGENTS.md
  note prevent a well-meaning "dead code" deletion later.
- **`--outcome-only` semantics narrow** (stdin reader + summary only).
  Documented; existing `cli_outcome_only_does_not_print_pre_exit_summary`
  test keeps passing.
- **Rollback** is trivial (git revert restores the CLI; the library never
  changed). The revert would re-open the MemoryLimit abort path, which is
  the known-bad behavior this plan removes.
- **Windows of confusion in tests**: `tests/reconstruct.rs` and
  `tests/test_proof_validate.rs` wire the worker manually — that stays the
  supported library-level pattern and needs no change.

## Final task

Write `docs/plans/proof/report7.md` in this directory: summary of changes,
files changed, verification results (drift table, CLI test results, manual
end-to-end, the experiment confirmation table), problems encountered,
deviations, missing tests, next steps (re-rank the backlog: #5 checkpoint vs.
#7 builder spike; the initiative's remaining motivation after the flip).
Update the initiative backlog (#3 → done) and History in the same pass.
