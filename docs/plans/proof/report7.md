# Implementation Report: Flip the Default — Worker-Off Search CLI (plan7)

Implements `docs/plans/proof/plan7.md` (backlog item #3). The search CLI no
longer builds proof trees: no worker thread, no proof-tree memory budget, no
fatal `MemoryLimit` abort path. Proof construction moved entirely to the
plan5 tool — `--tt-dump-path` + `reconstruct_pt --snapshot`. Zero search-path
changes (drift check bit-identical, see below); the library search API
(`set_memory_limited`, `set_proof_event_sender`, `ExitReason::MemoryLimit`)
is untouched.

## Summary of changes

- **CLI surface removal** (`src/cli.rs`): `CliOptions` lost `pt_size` and
  `dump_path`; `--pt-size` / `--dump-path` parsing and their help lines are
  deleted. Both flags now hit the existing unknown-option error path
  (exit 1), consistent with the CLI contract.
- **`src/main.rs`**: deleted the `memory_limited` Arc, the
  `ProofTreeWorkerHandle::spawn` call, the `set_memory_limited` /
  `set_proof_event_sender` wiring, the proof-tree portions of the pre-exit
  hook (`proof_tree:` stats, `proof_tree_dump:`, `pt_validate:`, exit-1 on
  defects), the `ExitReason::MemoryLimit` arm of the cut-short branch, and
  the worker teardown. The pre-exit hook reduces to the stdin reader
  (`q` quits) plus one `pre_exit: reason=… outcome=… nodes=…` line. The
  `--tt-dump-path` block is untouched. Module docs and help text rewritten.
- **`src/search/dfpn/mod.rs`** (comment only): `ExitReason::MemoryLimit` and
  `set_memory_limited` now document that they are a reconstruct-side
  contract (the walker sets the flag for the *builder's* budget and aborts
  local prefix-solves on it), not wired by the search CLI — to prevent a
  well-meaning "dead code" deletion.
- **`tests/test_cli.rs`**: dropped the `--dump-path` help assertion and the
  two live-dump tests (`cli_dump_path_writes_proof_tree_dump`,
  `cli_first_outcome_dumps_proof_tree`); added
  `cli_removed_proof_tree_options_exit_nonzero` (pins the removal: non-zero
  exit + unknown-option error + no search performed). All `--tt-dump-path`
  tests kept as-is.
- **`tests/test_plan7.rs`** (new, fast tier): the end-to-end default
  workflow — CLI binary on the two-rook mate fixture with `--tt-dump-path`,
  snapshot line asserted, absence of the three removed stdout lines
  asserted, then in-process reconstruction over the snapshot
  (`tests/reconstruct.rs` pattern) with `validate: ok` via
  `validate_proof_tree` and a binary-dump round-trip. This is the
  replacement gate for what the solver's `pt_validate:` pre-exit hook used
  to guarantee.
- **`AGENTS.md`**: `main.rs` bullet rewritten (worker-off resource-bounded
  CLI, remaining stdout lines, offline reconstruction workflow, reduced
  `--outcome-only` semantics); proof-tree bullet states the new producer
  chain (search → TT snapshot → `src/reconstruct` / `examples/reconstruct_pt`
  → dump; tests/examples may still spawn the worker directly) and notes the
  `ExitReason::MemoryLimit` reconstruct-side contract; Output priorities #3
  clarifies the dump is produced during offline reconstruction.

### Documented stdout changes (per the drift protocol)

| Removed | Reason |
|---|---|
| `proof_tree: nodes=… win=… loss=… root_depth=…` | no live tree |
| `proof_tree_dump: <file>` | no live dump |
| `pt_validate: ok` / `pt_validate: FAILED n defect(s)` | validation moved to `reconstruct_pt` (same validator, exit 1 there) |

| Kept | |
|---|---|
| `outcome:` / `pv:` / `pv_status:` | unchanged |
| `pre_exit: reason=… outcome=… nodes=…` | kept (reduced hook) |
| `timeout` / `quit` / `budget exhausted` | unchanged |
| `tt_snapshot: <file> solved=… unsolved=… bytes=…` | unchanged (opt-in) |

## Files changed

| File | Change |
|---|---|
| `src/cli.rs` | `pt_size`/`dump_path` fields, parsing, help, unit tests removed; new `removed_proof_tree_options_rejected` unit test |
| `src/main.rs` | worker wiring / proof-tree hook / `MemoryLimit` arm / teardown removed; hook reduced; docs + help rewritten |
| `src/search/dfpn/mod.rs` | comments on `ExitReason::MemoryLimit` and `set_memory_limited` (reconstruct-side contract) |
| `tests/test_cli.rs` | live-dump tests removed; unknown-option rejection test added |
| `tests/test_plan7.rs` | new — end-to-end default workflow (solve → snapshot → reconstruct → validate → dump round-trip) |
| `AGENTS.md` | `main.rs` bullet, proof-tree bullet, Output priorities #3 |

Unchanged by design: `src/search/dfpn/*` behavior, `src/proof_tree/*`,
`src/reconstruct/*`, `examples/reconstruct_pt.rs` (including its builder
`--pt-size`), `docs/spec/*`, binary formats.

## Verification results

1. `cargo fmt --check`, `cargo clippy --all-targets` (debug + release),
   `cargo doc` — clean.
2. `make test` (fast tier) — all green, including updated `test_cli.rs`
   (11 tests) and new `tests/test_plan7.rs`.
3. **Drift check** (criterion 2): `benchmark --suite quick --json
   --first-outcome --runs 1`, 59 cases, run before (pre-change binary) and
   after; per-case comparison shows all metric fields identical — only
   `time_min/time_mean/time_max` differ (allowed):

   | aggregate | before | after |
   |---|---:|---:|
   | total_nodes | 2,444,817 | 2,444,817 |
   | total_child_evals | 39,496,274 | 39,496,274 |
   | solved / timeouts / wrong | 59 / 0 / 0 | 59 / 0 / 0 |

4. **Manual end-to-end** (release), two-rook mate and dec02:

   - `atomic_solver --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1" --timeout 5
     --tt-dump-path …` → `outcome: win length: 3`, `pv_status:
     proven-shortest`, `tt_snapshot: … solved=3 unsolved=18 bytes=867`,
     `pre_exit: reason=Complete outcome=win nodes=25`; **none** of
     `proof_tree:` / `proof_tree_dump:` / `pt_validate:` in stdout.
   - `reconstruct_pt --snapshot …` → `outcome: win`, `nodes: 4`,
     `validate: ok`, exit 0; `inspect_pt --validate …` → `pt_validate: ok`,
     `validate_ppv: true` with the same `f1f7 e8d8 g1g8` line.
   - dec02 (`8/1k1p4/1P2p3/p2P1P2/P7/6p1/6K1/8 b - - 0 27`): solve →
     `outcome: loss length: 30`, `pv_status: proven-shortest`; reconstruct →
     `nodes: 2257`, `holes: hit=1843 terminal=414 … absent=0 filled=0`,
     `validate: ok`, exit 0; `inspect_pt --validate` → `pt_validate: ok`,
     `validate_ppv: true` (Loss depth 30).
   - `rg -n 'pt_size|dump_path|MemoryLimit' src/main.rs src/cli.rs` shows
     only `tt_dump_path` (the opt-in snapshot, kept); no `MemoryLimit`, no
     proof-tree wiring.

5. **Experiment confirmation** (criterion 5; `reconstruct_pt --experiment`,
   release, decisive suite, tt=128 MB, pt=256 MB, timeout=5s): **46/46
   oracle-isomorphic**, `validate: ok` on every live and reconstructed tree,
   zero coverage failures. Hole stats unchanged from report5/report6:

   | metric | value |
   |---|---|
   | live_ok / skipped / recon_ok | 46 / 0 / 46 |
   | coverage_failures / validate_failures | 0 / 0 |
   | holes | hit=69,598 terminal=21,869 clock_hit=1 clock_miss_draw=0 repetition=0 absent=2 filled=2 unfillable=0 anomalies=0 |
   | total C / total F | 65,138,783 / 2 |
   | median F/C / max F/C | 0.0000 / 0.0000 |
   | verdict | GO: zero coverage failures, all oracles isomorphic, median fill ratio ≤ 10% |

## Problems encountered

- The plan's manual-verification step wrote `inspect_pt --validate
  --dump-path proof_tree.bin`; `inspect_pt` actually takes the dump path as
  a positional argument (`inspect_pt [--validate] [file]`). Used the
  positional form; no code change needed. Plan docs only.
- The plan's test sketch asserted "`validation_errors == 0`" for the
  reconstructed tree. `ProofStats::validation_errors` lives on the worker
  handle, which `reconstruct()` does not surface in `ReconstructOutput`;
  the test instead asserts `validate_proof_tree(&tree).is_ok()` — the same
  validator, same defect classes, run on the finalized tree that
  `reconstruct()` returns (the worker's finalize validation already ran
  inside `reconstruct()`).

## Deviations

None beyond the two notes above; both are documentation-level.

## Missing tests

- No test pins `--pt-size` rejection *with a value omitted* (a bare
  `--pt-size` also errors via the unknown-option path; covered logically by
  the same mechanism, not separately asserted).
- The `--outcome-only` narrowing (no stdin reader, no summary) is covered by
  the pre-existing `cli_outcome_only_does_not_print_pre_exit_summary`, but
  there is no automated check that the stdin reader still quits the search
  on `q` for the non-`--outcome-only` path (pre-existing gap, not
  introduced here — interactive behavior is hard to test deterministically).
- `ExitReason::MemoryLimit`'s reconstruct-side usage is exercised only
  indirectly through `tests/reconstruct.rs` and the experiment; no unit
  test simulates a builder-budget abort (pre-existing; backlog #7
  territory).

## Next steps

- **Re-rank the backlog** (per the plan): the initiative's core motivation
  — decouple proof construction from the search's resource footprint — is
  now fully landed (#1–#3, #6 done). Remaining levers: **#5 periodic TT
  checkpoint** (crash resilience for multi-day runs, small) vs. **#7
  builder capacity for deep proofs** (large; needs a design spike only when
  deep proofs actually overflow the builder). Recommendation: #5 first —
  it is small, independent, and directly serves the multi-day run scenario
  that motivated the pivot; #7 stays conditional on observed builder
  pressure. **#4 (pin solved entries)** remains deprioritized (absent=2
  across the suite).
- External consumers that read a solver-produced `proof_tree.bin` must
  migrate to `--tt-dump-path` + `reconstruct_pt --snapshot` (one command;
  documented in plan7 §2).
