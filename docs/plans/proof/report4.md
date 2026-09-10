# Implementation Report: Transposition-Table Snapshot Writer (plan4)

Implements `docs/plans/proof/plan4.md` (backlog item #1). No search behavior
changed; no proof-tree code changed semantically (only the move-bit encoding
was relocated).

## Summary of changes

- **New artifact module `src/tt_snapshot/`** (re-exported from `lib.rs` as
  `tt_snapshot`), split into three files to respect the size guidelines:
  - `mod.rs` (7.9 KB) — format specification doc comment, record/header/summary
    types, constants, and the writer `write_tt_snapshot`.
  - `reader.rs` (4.5 KB) — strict `read_tt_snapshot` (rejects bad magic,
    unknown version, nonzero flags, invalid outcome/move encodings, truncated
    streams, count mismatch, trailing bytes).
  - `tests.rs` (7.3 KB) — round-trip, generation policy, `Move::NONE` sentinel,
    rejection, determinism, bucket-order, and empty-table tests.
- **Move-bit encoding lifted into `notation`.** `move_to_bits` / `bits_to_move`
  moved verbatim from `src/proof_tree/binary.rs` to `src/notation.rs`;
  `binary.rs` re-exports them (`pub use crate::notation::{bits_to_move,
  move_to_bits};`) so `worker.rs` and the `binary.rs` tests needed no changes.
- **TT iteration surface.** `TranspositionTable::entries()` (all valid entries,
  native bucket order, all generations) and `current_generation()` added in
  `src/search/tt/table.rs`; read-only `Search::tt()` accessor added in
  `src/search/dfpn/mod.rs`, documented for snapshot/debug use.
- **Outcome-byte sharing.** `binary.rs`'s private `outcome_to_u8` /
  `outcome_from_u8` became `pub(crate)` and are reused by the snapshot, so the
  snapshot outcome byte is provably identical to the proof-tree dump's root
  outcome byte (Draw=0, Win=1, Loss=2).
- **CLI.** `--tt-dump-path <FILE>` (opt-in, no default) in `src/cli.rs`;
  `src/main.rs` writes the snapshot after the search returns and *before* the
  `cut_short` handling, so an `ExitReason::MemoryLimit` run still produces the
  artifact before `exit(1)`. Success prints
  `tt_snapshot: <path> solved=<n> unsolved=<n> bytes=<n>`; I/O failures are
  logged to stderr and never change the exit status. Written regardless of
  `--outcome-only`. Help text and the `main.rs` module doc updated.
- **Docs.** `AGENTS.md`: `tt_snapshot` added to the `lib.rs` re-export list;
  `--tt-dump-path` added to the CLI parameter list. Initiative backlog #1
  marked done (this pass, per the plan's final task).

## Deviations from the plan

- **Writer makes three passes over `tt.entries()` (count, solved, unsolved)
  instead of "one pass".** The header must precede the records with exact
  counts, and the sections must be contiguous without sorting; buffering
  records to enable a single pass would risk exceeding the ≤ 0.75× TT RAM
  bound. Each pass is a linear scan over in-memory buckets (~1M entries for
  the 128 MB default); total dump cost at exit is well under a second.
- **`SnapshotSummary` also carries `bytes`** (as the plan's struct definition
  already specified) and is returned by the writer rather than recomputed by
  callers.
- **`git mv` was used** to relocate `src/tt_snapshot.rs` into
  `src/tt_snapshot/mod.rs` during the size-driven split, which stages the
  rename in the git index. Per repo convention no commit was made; the
  working tree is the source of truth.
- The snapshot reader validates UTF-8 for the root FEN (`String::from_utf8`)
  rather than byte-to-char casting as `binary.rs` does; the spec requires
  UTF-8 and the strict reader should enforce it.

## Files changed

| File | Change |
|---|---|
| `src/tt_snapshot/mod.rs` | new — format doc, types, writer |
| `src/tt_snapshot/reader.rs` | new — strict reader |
| `src/tt_snapshot/tests.rs` | new — 8 unit tests |
| `src/lib.rs` | add `pub mod tt_snapshot;` |
| `src/notation.rs` | add `move_to_bits` / `bits_to_move` |
| `src/proof_tree/binary.rs` | functions moved out; re-exports added; outcome helpers `pub(crate)` |
| `src/search/tt/table.rs` | add `entries()`, `current_generation()` |
| `src/search/tt/tests.rs` | add 2 iteration tests |
| `src/search/dfpn/mod.rs` | add `Search::tt()` accessor |
| `src/cli.rs` | `--tt-dump-path` option + tests |
| `src/main.rs` | snapshot write before `cut_short`, help text, docs |
| `tests/test_cli.rs` | 3 integration tests + help assertion |
| `AGENTS.md` | re-export list + CLI parameter list |
| `docs/plans/proof/initiative.md` | backlog #1 done, History updated |

## Verification results

1. `cargo fmt --check`, `cargo clippy --all-targets`, `cargo doc` — all clean
   (zero warnings).
2. `make test` (fast tier, release) — all 26 test suites green, 0 failures,
   including the 8 new `tt_snapshot` unit tests, 2 new TT iteration tests,
   2 new CLI parse tests, and 3 new CLI integration tests.
3. **Drift check** — `benchmark --suite quick --json --first-outcome` before
   vs. after: all deterministic fields (outcomes, `child_evals`, lengths)
   byte-identical per case; only wall-clock `time_*` fields differ, as
   expected for timing noise.
4. Manual CLI runs (release build):
   - `--fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1" --tt-dump-path ...` →
     `tt_snapshot: ... solved=3 unsolved=18 bytes=867`, `pv_status:
     proven-shortest`; file parses via `read_tt_snapshot` (also covered by the
     `cli_tt_dump_path_writes_parsable_snapshot` integration test against the
     real binary).
   - `--fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26"` (m26) →
     `solved=81 unsolved=40345 bytes=1695791` — 1.7 MB against the 128 MB TT
     bound (~1.3%); solved=81 is consistent with a single-generation run where
     all 81 solved entries are current-generation live entries.
   - **Determinism**: two runs of the m26 position produce byte-identical
     dumps (`cmp` clean).
   - **MemoryLimit ordering**: `--pt-size 1` on the m22 position →
     `tt_snapshot: ... solved=4329 unsolved=133523 bytes=5672990` printed
     before `error: proof-tree memory limit (1 MB) reached` and the process
     still exits 1 — the rescue artifact survives the fatal path (5.7 MB,
     well under the TT bound).

## Problems encountered

- The planned single-pass writer conflicts with writing exact counts before
  the records (see deviation above); resolved with the three-pass approach.
- The first split attempt of the 20 KB `tt_snapshot.rs` briefly dropped the
  test submodule during mechanical extraction; caught immediately by the
  compile/test loop and reconstructed verbatim.

## Trade-offs / unresolved parts

- **Records are not sorted** (native bucket order, per spec): byte-identical
  dumps for identical searches, but two *different* builds/allocations could
  theoretically place entries in different buckets; consumers must treat the
  snapshot as a set keyed by `key`, not an ordered structure.
- **Unsolved records include `best_child`?** No — `best_child` was dropped
  from the unsolved record to keep it 42 bytes; plan5's hole-filling searches
  recompute ordering anyway. If plan5 finds it needs `best_child` for seeding,
  that is a v2 format change (bump version, don't grow v1).
- **Snapshot is not crash-safe** (written once at exit); item #5 (periodic
  checkpointing) remains the stretch item for multi-day run resilience.
- The reader buffers all records in memory (`try_reserve_exact` with count
  checks); plan5's builder on a memory-optimized machine may prefer streaming
  reads — trivial to add later since the format has no back-references.

## Missing tests

- No test exercises a *stale-generation solved entry that was later
  physically evicted* (by design it cannot appear in the dump — that is the
  hole class plan5 must handle), nor the generation-counter wrap at `u32::MAX`
  (clears the table).
- No test pins the `Search::tt()` accessor against concurrent mutation (the
  solver is sequential; a threaded accessor would need different semantics).
- MemoryLimit ordering is verified manually (above) but not by an automated
  test, because forcing `ExitReason::MemoryLimit` deterministically needs the
  slow-tier m22 fixture.

## Next steps

- **plan5** — offline reconstruction tool + go/no-go experiment: top-down walk
  from the root FEN over this snapshot (static terminal classification,
  movegen expansion of Loss nodes, hole filling via
  `search_depth_with_prefix` seeded with the snapshot). Define the hole-rate
  kill criteria before building.
- **plan6** — flip the default: worker off during search (gated on plan5).
