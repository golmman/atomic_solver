# Implementation Plan: Transposition-Table Snapshot Writer

Initiative: `docs/plans/proof/initiative.md`, backlog item #1. This plan is
self-contained; it changes no search behavior and touches no proof-tree code.

## Goal

Add an opt-in CLI flag `--tt-dump-path <FILE>` that writes a compact, bounded
binary snapshot of the transposition table after the search finishes. The
snapshot is the transfer artifact for the decoupled proof pipeline: a separate
reconstruction tool (plan5, not in scope here) will rebuild the proof tree
from the root FEN plus this snapshot on a memory-optimized machine.

Success criteria:

- `cargo test` (fast tier) green, including new round-trip and CLI tests.
- Drift check stays bit-identical: `benchmark --suite quick --json
  --first-outcome` produces identical `child_evals` per case.
- Dump size stays bounded by the TT size (≤ ~0.75× TT RAM worst case).
- Two runs of the same binary on the same position and settings produce
  byte-identical dumps.

## Context (current state)

`TranspositionTable` (`src/search/tt/table.rs`) stores two-slot buckets:

```rust
pub struct TranspositionTable {
    table: Vec<[TtEntry; 2]>,   // private
    mask: usize,                // private
    current_generation: u32,    // private
}
```

`TtEntry` (`src/search/tt/entry.rs`) fields are `pub(crate)`:

```rust
key: u64, valid: bool, generation: u32,
best_move: Move, best_child: u8,
work: u64,
outcome: Option<Outcome>,   // Some = solved
pn: u64, dn: u64,
depth: u32, remaining_depth: u32,
```

Relevant semantics:

- `probe` only sees entries with `generation == current_generation`. A solved
  entry from an older generation is invisible to the search but its stored
  result is still position-truth, because solved results are path-independent
  base entries (repetition-dependent results are never stored — GHI
  shortcut). Unsolved `pn`/`dn` bounds are only meaningful for the generation
  they were computed in.
- `store` never overwrites a solved entry with unsolved bounds; solved
  overwrites solved.
- `insert_new` evicts within a bucket by `(live, solved, work, generation)`
  score — solved entries can still be physically evicted. Those losses are
  exactly the "hole" classes the plan5 reconstruction must handle; this plan
  dumps what physically survives, nothing more.
- Zobrist keys **include the halfmove clock** (`src/zobrist.rs`), so a
  snapshot lookup by the future builder must use the exact key along its own
  path; clock mismatches via transpositions are an anticipated hole class.

The 16-bit move encoding used by the proof-tree dump
(`move_to_bits` / `bits_to_move` in `src/proof_tree/binary.rs`) matches
`atomic_movegen::Move`'s documented bit layout. The TT snapshot needs the
same encoding, but `src/search/` must not depend on `proof_tree`
(dependency direction: `search` knows only `proof_event`).

## Format specification

Little-endian throughout, fields packed with no padding. Mirrors the
`binary.rs` conventions (8-byte magic, version byte, newline-terminated FEN
header line).

```
offset  size  field
0       8     magic  = "ATOMTTSN"
8       1     version = 1
9       1     flags = 0 (reserved; must be 0 in v1)
10      n     root FEN, UTF-8, '\n'-terminated (the FEN the search solved)
+0      4     tt_size_mb: u32
+4      4     generation: u32 (table generation at dump time)
+8      8     solved_count: u64
+16     8     unsolved_count: u64

solved record × solved_count          (15 bytes each)
  key: u64
  outcome: u8              (same encoding as the proof-tree dump's root
                            outcome byte: Win/Loss/Draw)
  depth: u32
  best_move: u16           (16-bit move code; 0xFFFF = Move::NONE)

unsolved record × unsolved_count      (42 bytes each)
  key: u64
  pn: u64
  dn: u64
  depth: u32
  remaining_depth: u32
  best_move: u16           (0xFFFF = Move::NONE)
  work: u64                (cumulative child_evals under the subtree)
```

- **Solved section**: every valid entry with `outcome.is_some()`, from *all*
  generations. Rationale: solved results are path-independent and permanent,
  so stale-generation solved entries are free coverage against eviction.
- **Unsolved section**: valid entries with `outcome == None` from the
  *current generation only*. Rationale: bounds are generation-local. These
  records are best-effort context for plan5's hole-filling searches (seed TT)
  and for debugging; the reconstruction must not rely on them.
- **Order**: native bucket order (`table[0][0], table[0][1], table[1][0],
  ...`), filtered per section. This makes dumps byte-identical for identical
  searches and cheap to write (one pass, no sort).
- The `0xFFFF` sentinel is unambiguous: `move_to_bits` never produces it
  (maximum is `0x7FFF`), and `Move::NONE` must survive the round trip.
- Readers must reject wrong magic, unknown version, nonzero flags, and
  truncated files. Counts are authoritative; no trailing bytes are allowed
  beyond the last record.

## Scope

1. **Lift the move-bit encoding into `notation`.**
   - Move `move_to_bits` / `bits_to_move` from `src/proof_tree/binary.rs`
     into `src/notation.rs` (they use only public `atomic_movegen` API and
     `notation` is the domain-standard home for move encodings).
   - `binary.rs` re-uses `crate::notation::{move_to_bits, bits_to_move}`.
     `binary.rs` must keep re-exporting them, because in-crate callers import
     via `super::binary::move_to_bits` (currently `worker.rs`, 4 call sites —
     verified 2026-09-10; re-run `rg "move_to_bits|bits_to_move" -g '*.rs'`
     before moving to catch new callers). With the re-export in place,
     `worker.rs` needs no changes.
   - Copy the existing doc comments (bit layout) along; the binary-format
     tests in `binary.rs` stay and continue to cover the functions.

2. **Expose table iteration from `search::tt`.**
   - `src/search/tt/table.rs`:
     - `pub(crate) fn entries(&self) -> impl Iterator<Item = &TtEntry>` over
       all `valid` entries, native bucket order, all generations.
     - `pub(crate) fn current_generation(&self) -> u32`.
   - `src/search/dfpn/mod.rs`: `pub fn tt(&self) -> &TranspositionTable`
     read-only accessor, documented as for snapshot/debug use. No other
     change to `Search`.

3. **New module `src/tt_snapshot.rs`** (re-exported from `src/lib.rs`).
   - `pub struct SnapshotSummary { pub solved: u64, pub unsolved: u64,
     pub bytes: u64 }`
   - `pub fn write_tt_snapshot<W: Write>(tt: &TranspositionTable,
     root_fen: &str, tt_size_mb: u32, w: &mut W) -> io::Result<SnapshotSummary>`
     — one pass over `tt.entries()`, partitioning into the two sections
     (solved: any generation; unsolved: `generation == tt.current_generation()`).
   - `pub fn read_tt_snapshot<R: Read>(r: &mut R) ->
     io::Result<(SnapshotHeader, Vec<SolvedRecord>, Vec<UnsolvedRecord>)>`
     with minimal plain-data record structs. The reader exists for tests now
     and for the plan5 builder later; keep it strict (magic/version/flags/
     counts/truncation).
   - The module doc comment documents the format (copy the spec above),
     the solved-across-generations and unsolved-current-only rationale, and
     the halfmove-clock caveat for future lookups.
   - File must stay under the 10 KB guideline (split reader/writer only if
     it does not).

4. **CLI wiring.**
   - `src/cli.rs`: new option `--tt-dump-path <FILE>`, opt-in, no default.
     Unknown-option error behavior unchanged.
   - `src/main.rs`:
     - Write the snapshot **after the search returns and before the
       `cut_short` handling**, so an `ExitReason::MemoryLimit` run still
       produces the snapshot — the memory-limit path is precisely where the
       proof tree failed and the snapshot is the rescue artifact. This is
       the only ordering requirement; it must precede the `exit(1)`.
     - On success print `tt_snapshot: <path> solved=<n> unsolved=<n>
       bytes=<n>`; on I/O failure print an error to stderr and continue
       (exit status unchanged) — a failed debug artifact must not turn a
       good search result into a failure.
     - The dump is written regardless of `--outcome-only` (explicit opt-in
       overrides the no-artifacts convenience default; it spawns no extra
       threads and changes no search behavior).
   - Update the module doc comment and `print_help`.

5. **Docs.**
   - `AGENTS.md`: add `tt_snapshot` to the `lib.rs` re-export list; add
     `--tt-dump-path` to the `main.rs` CLI parameter list with one sentence
     on what the snapshot is for. Update the `proof` initiative pointer in
     `docs/plans/proof/initiative.md` backlog (#1 → done) only in the final
     report, not in this plan's implementation commit.

## Explicitly out of scope

- Any reconstruction/reading logic beyond the strict test reader.
- Changing the proof-tree worker, `ProofEvent`, `--pt-size`,
  `memory_limited`, or `ExitReason::MemoryLimit` semantics (plan6).
- Changing eviction policy (item #4 is a separate, drift-validated plan).
- Periodic checkpointing (item #5).
- Any change to `store`, `probe`, or search timing.

## Tests

Unit tests (fast tier, `#[cfg(test)]` modules):

- `src/tt_snapshot.rs`:
  - round-trip: small table populated via `TranspositionTable::with_capacity`
    + `store`; write, read, compare every field of every record.
  - generation policy: store entries, `new_generation()`, store more; assert
    stale **solved** entries are present, stale **unsolved** entries absent,
    current-generation entries of both kinds present.
  - `Move::NONE` sentinel round-trips as `0xFFFF` and back.
  - rejection: bad magic, unknown version, nonzero flags, truncated stream,
    count mismatch.
  - determinism: two tables with identical stores produce identical bytes.
- `src/search/tt/tests.rs`: `entries()` yields all valid entries in bucket
  order across generations; `current_generation()` reflects `new_generation`.

Integration tests (`tests/test_cli.rs`, fast):

- Run the CLI binary on a decisive mate-in-3 FEN (e.g. `4k3/8/8/8/8/8/8/4KRR1
  w - - 0 1`) with `--tt-dump-path` into a temp file; assert the
  `tt_snapshot:` line, parse the header, assert `solved_count >= 1`.
- Same with `--outcome-only` to pin the opt-in-overrides-default behavior.
- Sanity size bound: `bytes <= tt_size_mb * 1_048_576` for the default
  `--tt-size`.

No slow/ignored tests are required by this plan.

## Verification

1. `cargo fmt --check`, `cargo clippy --all-targets`, `cargo doc` — clean.
2. `make test` — fast tier green including the new tests.
3. Drift check: `benchmark --suite quick --json --first-outcome` before and
   after — `child_evals` identical per case (expected: no search-path change
   at all).
4. Manual CLI runs, release build:
   - `--fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1" --tt-dump-path /tmp/opencode/tt.bin`
     — inspect the `tt_snapshot:` line; round-trip the file through
     `read_tt_snapshot` in a quick example or test.
   - `--fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26"
     --tt-dump-path /tmp/opencode/tt26.bin` (m26) — verify the dump size is
     well under the TT bound and `solved_count` is plausible vs.
     `tt_stats()` output.

## Risks / notes

- **I/O cost at exit**: worst case ≈ 0.75× TT RAM written once (e.g. ~100 MB
  at the 128 MB default). Seconds at most; acceptable for an opt-in flag.
  If a future use case needs faster exit, section streaming is trivial
  because no sort is required.
- **Format evolution**: the flags byte plus strict version checking is the
  only forward-compat mechanism; bump the version rather than growing v1.
- **`notation` refactor blast radius**: the two functions are used by
  `binary.rs` itself and by `worker.rs` (via `super::binary`); keeping the
  re-exports in `binary.rs` contains the change to one file.

## Final task

Write `docs/plans/proof/report4.md` in this directory: summary of changes,
files changed, verification results (including the drift check output),
problems encountered, trade-offs / unresolved parts, missing tests, and next
steps (plan5 reconstruction tool). Update the initiative backlog status for
item #1 to done in the same pass.
