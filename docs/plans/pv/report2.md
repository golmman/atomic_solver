# Streaming Proof-PV Output Workflow - Report

## Summary

Implemented `docs/plans/pv/plan2.md` in `atomic_solver`. The CLI now streams
solver results as they are discovered:

1. the decisive outcome (`outcome: win`/`loss`/`draw`);
2. a first Proof PV (PPV) where the defender plays the longest resistance;
3. progressively shorter PPVs, ending with the Shortest Proof PV (SPPV) when
   time allows.

If the timeout is reached at any stage, the program prints `timeout` on its own
line and exits.  Results already printed are preserved, so nothing is lost.

## Key changes

- **Staged solver API** (`src/search/dfpn/mod.rs`):
  - Added `Search::solve_outcome` to find a decisive outcome and record the
    depth bounds found during the search.
  - Added `Search::find_ppv` to verify and extract a PPV for the outcome.
  - Added `Search::refine_sppv` to iteratively deepen downward from the PPV
    length, calling a callback for each strictly shorter PPV.
  - Kept `Search::solve` as a backward-compatible wrapper.  When
    `refine_shortest` is false it still performs a single unbounded search
    (fast on shallow positions); when true it uses the staged API.
  - Made `Search::time_exceeded` public so the CLI can decide whether to print
    a timeout notice.

- **PPV extraction helper** (`src/search/dfpn/pv.rs`):
  - Added `extract_ppv` as a terminal-validating wrapper around the existing
    PV extraction.

- **CLI streaming output** (`src/main.rs`):
  - Rewrote `main` to call `solve_outcome`, print the outcome, then `find_ppv`,
    print the PPV, then optionally `refine_sppv` and print each shorter line.
  - Added `timeout` notices after each stage.
  - Updated help text for `--no-refine-shortest` to mean "find and print the
    outcome and the PPV, but do not refine toward the SPPV".

- **Regression tests** (`tests/test_plan6.rs`):
  - `m27_streaming_output`: exercises the staged API on the reported FEN and
    verifies a 7-plies PPV starting with `b1b8 g8f7`; refinement finds no
    shorter PPV.
  - `m27_ppv_only`: runs the CLI with `--no-refine-shortest` and checks that
    exactly `outcome: win` plus one `pv:` line is printed.
  - `timeout_message`: checks that `Search::set_timeout(0)` makes
    `solve_outcome` return `Outcome::Draw` with `time_exceeded` true and no
    spurious PPV.

## File changes

| File | What changed |
|------|--------------|
| `src/search/dfpn/mod.rs` | Added `solve_outcome`, `find_ppv`, `refine_sppv`; rewrote `solve` as a wrapper; added `bootstrap_success_depth`/`bootstrap_fail_depth` fields; made `time_exceeded` public. |
| `src/search/dfpn/pv.rs` | Added `extract_ppv` helper. |
| `src/main.rs` | Staged CLI output with streaming `pv:` lines and `timeout` notices; updated `--no-refine-shortest` help. |
| `tests/test_plan6.rs` | Added `m27_streaming_output`, `m27_ppv_only`, and `timeout_message` regression tests. |
| `docs/plans/pv/report2.md` | This report. |

## Test results

```bash
cargo fmt
cargo clippy --all-targets
cargo test
cargo doc
```

All passed with no new warnings.

Selected output from the reported FEN:

```bash
cargo run --release -- --no-refine-shortest --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26"
```

produces:

```text
outcome: win
pv: b1b8 g8f7 b8f8 f7g7 d6e5 g7h7 f8h8
```

Default refinement may append `timeout` within the 5-second limit when it is
still proving that no shorter PPV exists; the PPV/SPPV line is still printed
first.

## Known limitations / future work

- `solve_outcome` bootstraps with an iteratively doubling depth bound.  For a
  few shallow-but-wide positions this is slower than a single unbounded search,
  so `Search::solve` keeps the unbounded path when `refine_shortest` is false.
- `refine_sppv` currently proves minimality by probing every smaller depth in
  sequence.  A cheaper lower-bound proof could avoid the final `timeout` on
  positions whose PPV is already the SPPV.
