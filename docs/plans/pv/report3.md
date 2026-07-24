# PPV/SPPV Regression for `4r1k1/...` - Report

## Summary

Investigated and fixed the regression described in `docs/plans/pv/plan3.md`.

The reported FEN

```text
4r1k1/3p4/2pB2p1/6Pp/p4p1P/2N1PP2/P1PP4/1R2R2K w - - 0 24
```

no longer returns the non-Proof PV starting with `b1b8 g8h7 e3f4 h7h8 ...`.  The
solver now prints a valid PPV beginning with the real winning move `e3f4`, then
either continues toward the SPPV or prints `timeout` when the time budget is
exhausted.

## Root cause

The staged solver (`Search::solve_outcome` / `find_ppv` /
`refine_sppv`) was reusing bounds from the transposition table across
iterative-deepening probes.  Work-bounded probes could store an **unsolved**
node with terminal-looking bounds (`pn == 0, dn == INF` or vice versa).  A later
probe reused those bounds as if the node were decided, causing `solve_outcome`
to claim a win from the wrong first move (`b1b8`) and print a PV containing the
weak defender reply `h7h8`.

Three additional control-flow bugs made the symptom worse:

1. `solve_outcome` was overwriting `bootstrap_fail_depth` on a successful win,
   collapsing the refinement interval and preventing `refine_sppv` from
   searching.
2. `refine_sppv` was binary-searching the depth interval, which quickly picks
   very shallow depths (the expensive failed probes) and can waste the entire
   time budget on a single `Draw` proof.
3. `children.rs` would reuse any unsolved TT summary whose `remaining_depth` was
   compatible, even if its `pn`/`dn` pair was degenerate.

## Key changes

- **Clamp unsolved stored bounds** (`src/search/dfpn/core.rs`):
  Before writing an unsolved entry to the TT, `pn` and `dn` are now clamped to
  at least `1`.  This prevents an unproven node from masquerading as a solved
  terminal (`(0, INF)` / `(INF, 0)`) for later searches.

- **Guard unsolved summary reuse** (`src/search/dfpn/children.rs`):
  `evaluate_child` now only reuses an unsolved TT summary as initial bounds when
  `summary.pn > 0 && summary.dn > 0`, in addition to the existing depth checks.

- **Preserve `fail_depth`** (`src/search/dfpn/mod.rs`):
  `solve_outcome` no longer overwrites `bootstrap_fail_depth` when a probe
  returns a decisive result.  The largest known `Draw` depth stays intact so
  `refine_sppv` has a valid search interval.

- **Sequential downward refinement** (`src/search/dfpn/mod.rs`):
  `refine_sppv` now searches downward from the known winning depth
  (`probe = hi - 1`, then `hi = probe` on success).  This reuses the TT and
  move-ordering state from the previous, deeper probe and stops as soon as a
  depth fails, leaving `hi` as the shortest proven depth.  It avoids the
  expensive shallow failed probes of the previous binary search.

## Files changed

| File | What changed |
|------|--------------|
| `src/search/dfpn/core.rs` | Clamp stored unsolved `pn`/`dn` to at least `1`. |
| `src/search/dfpn/children.rs` | Reuse unsolved TT summaries only when `pn > 0 && dn > 0`. |
| `src/search/dfpn/mod.rs` | Keep `fail_depth` on a win; replace binary `refine_sppv` with sequential downward deepening. |
| `docs/plans/pv/report3.md` | This report. |

## Test results

```bash
cargo fmt
cargo clippy --all-targets
cargo test
cargo test --release
cargo doc
```

All passed with no new warnings.  In particular `tests/test_plan6.rs` still
passes `m27_streaming_output`, `m27_shortest_pv`, `m27_ppv_only`, and
`timeout_message`.

## Manual verification

Reported FEN with full refinement:

```bash
cargo run --release -- --fen "4r1k1/3p4/2pB2p1/6Pp/p4p1P/2N1PP2/P1PP4/1R2R2K w - - 0 24" --timeout 60
```

Output:

```text
outcome: win
pv: e3f4 e8e1 b1b4 c6c5 b4b8 g8f7 a2a3 c5c4 b8g8 f7e6 g8g7 e6f5 g7g6
timeout
```

The 13-plies line is a valid PPV.  The `timeout` is printed because the
remaining time is spent trying to prove that no shorter forced win exists.

PPV-only mode:

```bash
cargo run --release -- --no-refine-shortest --fen "4r1k1/3p4/2pB2p1/6Pp/p4p1P/2N1PP2/P1PP4/1R2R2K w - - 0 24" --timeout 60
```

Output:

```text
outcome: win
pv: e3f4 e8e1 b1b4 c6c5 b4b8 g8f7 a2a3 c5c4 b8g8 f7e6 g8g7 e6f5 g7g6
```

## Known limitations / future work

- Proving that the PPV is the SPPV is still expensive: `refine_sppv` may
  consume the whole remaining time budget on a single failed depth probe.  A
  cheaper lower-bound proof (e.g., a quick disproof search at `current_best_len
  - 1`) would let the solver finish more often.
- `solve_outcome` still uses iteratively doubling work chunks.  The chunk sizes
  are currently sufficient, but a position that falls between chunk limits can
  return `Draw` and trigger the fallback unbounded search.
- The previously ignored `m19`–`m29` positions in `tests/test_plan6.rs` remain
  `#[ignore]`; they were not in scope for this regression fix.
