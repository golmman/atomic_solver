# Plan 18: Reduce `dfpn.rs` Size, Add File-Size Convention, and Make the PV Cap Safe

## Start

- Read `AGENTS.md` and note the current project conventions.
- Read `src/search/dfpn.rs` to understand which parts are independent enough
  to move into submodules.
- Read `src/search/tt.rs` and check its size after `dfpn.rs` is split.
- Read `src/main.rs` and `src/search/dfpn.rs` to confirm how the 1000-ply cap
  and `extract_pv` are used.
- Read `docs/plans/ghi/test_theory.md` for context on the GHI tests that are
  out of scope for this plan.

## Goal

1. Split `src/search/dfpn.rs` so that no source file is unnecessarily large.
2. Add a file-size convention to `AGENTS.md` with a documented-justification
   rule for exceptions.
3. Remove the remaining dead `print_pv_update` / `refine_shortest` guard code.
4. Replace the hard 1000-ply `extract_pv` cap with a configurable limit and
   clear warnings when the PV is truncated.
5. Keep the solver correct: do not treat long PVs or long lines as losses.

## Background

- `src/search/dfpn.rs` is ~54 KB / 1,687 lines, far larger than any other
  source file.  It mixes the public `Search` API, the recursive DF-PN engine,
  PV extraction/validation, GHI simulation, and move-ordering helpers.
- `src/search/tt.rs` is ~16 KB and will also exceed a strict size cap once
  `dfpn.rs` is split, unless it is also split or justified.
- `print_pv_update` and the `refine_shortest` guard inside `dfpn` are no
  longer reachable from the public API but still add noise and size.
- `extract_pv` stops after 1000 plies and falls back to an unvalidated PV
  without telling the user.  This is a display limit, not a game-theoretic
  cutoff, and must not be interpreted as "white loses after 1000 plies".

## Implementation tasks

### Part 1: Adopt a file-size convention

1. Add a new item to `AGENTS.md` under Conventions:

   ```text
   - Keep source files under ~10 KB.  Files larger than 10 KB must include a
     short documented justification in the file header or in AGENTS.md.
     Files larger than ~20 KB should normally be split into submodules.
   ```

2. Enforce the convention on `dfpn.rs` by splitting it.  Enforce or justify it
   on `tt.rs`.

### Part 2: Split `src/search/dfpn.rs`

Create `src/search/dfpn/` as a directory module and move the internals into
focused files.  Keep `Search` and its public API in `mod.rs`.

Proposed layout:

- `src/search/dfpn/mod.rs` — `Search` struct, public API (`new`, `solve`,
  `search_depth`, `set_timeout`, `set_epsilon`, etc.), and module re-exports.
- `src/search/dfpn/core.rs` — the `dfpn` recursive routine, `select_children`,
  `evaluate_child`, `is_solved_by_children`, threshold propagation, and helper
  types `ChildInfo`, `ChildSelection`, `Resolved`.
- `src/search/dfpn/pv.rs` — `extract_pv`, `extract_pv_checked`,
  `validate_pv`, and `should_print_update`.
- `src/search/dfpn/simulate.rs` — `try_use_tt` and `simulate`.
- `src/search/dfpn/history.rs` — `update_history`, `update_killers`,
  `maybe_age_history`, and `killer_bonus`.
- `src/search/dfpn/tests.rs` — unit tests for the `dfpn` module (or keep
  tests in the relevant submodule per `AGENTS.md` unit-test convention).

Notes:

- `search/mod.rs` already does `pub mod dfpn;`, so changing `dfpn.rs` to
  `dfpn/mod.rs` keeps the public API unchanged.
- `impl Search` blocks can be split across submodules as long as each file
  imports `Search` from `super`.
- Move constants close to where they are used (e.g. `SIM_MAX_DEPTH` and
  `SIM_MAX_NODES` into `simulate.rs`, history/killer constants into
  `history.rs`).  Keep `INF` and `DEFAULT_*` re-exported from `mod.rs` or
  `core.rs` as appropriate.

### Part 3: Split or justify `src/search/tt.rs`

After the `dfpn.rs` split, re-measure `tt.rs`.  If it is still over 10 KB:

- Option A: split into `src/search/tt/entry.rs` (`TtEntry`, `TwinEntry`,
  `EntryResult`) and `src/search/tt/table.rs` (`TranspositionTable` and its
  `impl`).  Keep `src/search/tt/mod.rs` as a small re-export module.
- Option B: add a short header comment documenting why `tt.rs` is larger than
  10 KB (the transposition table implementation, entry layout, and tests are
  tightly coupled).

Prefer Option A if it keeps each file under 10 KB without circular imports.

### Part 4: Remove dead code

1. Remove `fn print_pv_update` from `dfpn`.
2. Remove the `refine_shortest` guard inside the `dfpn` loop
   (`if self.refine_shortest && self.path_stack.len() == 1 ...`).
3. Remove the `refine_shortest` checks in the threshold-break logic if they are
  also unreachable (confirm after reading the current code).
4. Keep `Search::refine_shortest` as the public switch for the binary-search
  `solve_refined` path, but stop using it inside `dfpn`.
5. Keep `last_pv` because `solve_refined` sets it directly; do not let
   `print_pv_update` write to it.

### Part 5: Configurable PV cap with warnings

1. Replace the hard-coded `1000` in `extract_pv` with a `const
   DEFAULT_MAX_PV_PLIES: usize = 1000`.
2. Add a `max_ply: usize` field to `Search` (or re-use an existing depth field),
   set by `Search::set_max_ply`, defaulting to `DEFAULT_MAX_PV_PLIES`.
3. Use `self.max_ply` in `extract_pv` and `extract_pv_checked`.
4. When `extract_pv` stops because of the cap:
   - Return the partial PV.
   - Emit `eprintln!("warning: PV truncated after {} plies", self.max_ply)`
     from `extract_pv_checked` or from `main.rs`.
5. Do **not** change the outcome.  A truncated PV is still returned with the
   correct `Outcome`.  The cap is a display safety, not a proof cutoff.
6. Consider making `SIM_MAX_DEPTH` in `simulate` respect the same `max_ply` so
   that deep cross-path twins are not rejected simply because the simulation
   depth cap is smaller than the PV cap.  If this is invasive, document it
   as a follow-up in the plan report.

## File changes

- `AGENTS.md` — new file-size convention.
- `src/search/mod.rs` — update if `tt.rs` becomes a directory module.
- `src/search/dfpn.rs` — delete; replaced by `src/search/dfpn/` directory.
- `src/search/dfpn/mod.rs` — new.
- `src/search/dfpn/core.rs` — new.
- `src/search/dfpn/pv.rs` — new.
- `src/search/dfpn/simulate.rs` — new.
- `src/search/dfpn/history.rs` — new.
- `src/search/dfpn/tests.rs` or tests split across the above — new.
- `src/search/tt.rs` — either split into `src/search/tt/` or add a header
  justification.
- `src/main.rs` — possibly adjust warning display.

## Risks

- Splitting a large file can introduce import/visibility bugs.  Run the full
  test suite after each submodule is extracted.
- Removing `refine_shortest` guards must be done carefully; verify no public
  code path still expects intermediate PV updates from `dfpn`.
- Raising or configuring `max_ply` may expose slow `extract_pv` behavior on
  cyclic positions.  Keep `SIM_MAX_NODES` as a backstop in `simulate`.
- Do not weaken the GHI or depth-bounded correctness work from Plan 17 while
  moving code around.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo test --release
$ cargo doc --no-deps
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
$ cargo run --release -- --fen "4k3/PP6/8/8/8/8/8/4K3 w - - 0 1"
```

Additional checks:

- No source file is >10 KB without justification (measure with `wc -c`).
- CLI still prints exactly one `outcome:`/`pv:` block on decisive positions.
- A position whose shortest PV exceeds the cap produces a warning, not a wrong
  `outcome`.

## Final task

Write `docs/plans/review/report18.md` documenting:

- The new `AGENTS.md` file-size convention and which files were split or
  justified.
- The new `dfpn` submodule layout.
- The dead code removed.
- The configurable PV cap and warning behavior.
- Verification results (`cargo test`, `cargo clippy`, `cargo doc`, CLI output).
- Any follow-up items (e.g. `SIM_MAX_DEPTH` tuning).
