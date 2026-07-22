# Plan: Streaming Proof-PV Output Workflow

## Summary

This plan implements the streaming solver workflow defined in `docs/theory/definitions.md`.
The CLI no longer prints a single PV together with the outcome. Instead it reports
results as soon as they are known:

1. the decisive outcome (`win`, `loss`, or `draw`);
2. a first **Proof PV (PPV)** where the defender plays the longest resistance;
3. progressively shorter PPVs, ending with the **Shortest Proof PV (SPPV)**.

If the timeout is reached at any stage the solver prints a simple timeout notice
and exits; all results already discovered have already been printed, so nothing is
"partial" or lost.

## Goal

Make the default CLI run

```text
cargo run --release -- --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26"
```

produce output similar to

```text
outcome: win
pv: b1b8 g8f7 b8f8 f7g7 d6e5 g7h7 f8h8
pv: b1b8 g8f7 b8f8 f7g7 d6e5 g7h7 f8h8
timeout
```

or, if refinement completes,

```text
outcome: win
pv: b1b8 g8f7 b8f8 f7g7 d6e5 g7h7 f8h8
```

(Only one PV line is printed if the first PPV already is the SPPV.)

## Non-goals

- No change to `atomic-movegen` rules or move generation.
- No change to the transposition-table structure.
- No change to the core DF-PN+ algorithm itself; plan1/report1 already made the
  search depth-aware. This plan focuses on the control flow, API, and CLI output.
- No parallel search.

## Background

`docs/plans/pv/analysis.md` showed that the solver can return a line that is
neither a PPV nor an SPPV because it stops at the first proven winning child and
the printed PV may contain non-optimal defender replies. `docs/plans/pv/report1.md`
fixed the underlying algorithm so the search tracks `best_win_depth` and
`best_loss_depth` and uses iterative-deepening refinement. This plan changes the
*interface* around that algorithm so the user receives reliable, streaming output
instead of a single possibly-suboptimal line.

`docs/theory/definitions.md` defines the terms used here:

- **Proof PV (PPV)**: defender replies maximize length; attacker moves are any
  winning moves.
- **Shortest Proof PV (SPPV)**: a PPV in which every attacker move is also a
  shortest winning move.

## Design

The solver runs in stages. Each stage prints its result as soon as it is known.

```text
search for outcome
    -> print "outcome: <win|loss|draw>"
    -> if draw: exit
    -> if timeout: print "timeout" and exit

find / verify a PPV
    -> print "pv: <ppv>"
    -> if timeout: print "timeout" and exit

refine toward the SPPV
    -> for each strictly shorter PPV found: print "pv: <shorter>"
    -> when no shorter PPV can be found: exit
    -> if timeout: print "timeout" and exit
```

A single iterative-deepening pass can combine the last two stages. The first
successful depth-bound search returns a PPV; every subsequent successful probe at
a smaller depth bound returns a shorter PPV. The last printed line is therefore the
SPPV (or the best PPV found before timeout).

## API changes

### `src/search/dfpn/mod.rs`

Introduce three public (or `pub(crate)`) stage methods on `Search`:

1. `pub fn solve_outcome(&mut self, pos: &mut Position) -> Outcome`

   Runs the DF-PN+ search until a decisive outcome is proven or the timeout is
   reached. If the timeout is reached, returns `Outcome::Draw` (the current
   convention for an unresolved search) and the caller treats this as a timeout.

2. `pub fn find_ppv(&mut self, pos: &mut Position, outcome: Outcome) -> Option<Vec<Move>>`

   Returns the first valid PPV for the given outcome. With the depth-aware changes
   from plan1 the transposition table already tracks `best_win_depth` and
   `best_loss_depth`, so the initial extract may already be a PPV. If it is not,
   this method performs a bounded search that forces every AND-node defender reply
   to be evaluated so the longest defense can be selected.

3. `pub fn refine_sppv<F>(
       &mut self,
       pos: &mut Position,
       outcome: Outcome,
       mut on_shorter: F,
   ) where F: FnMut(&[Move])

   Searches for shorter PPVs, invoking `on_shorter` every time a strictly shorter
   line is proven. The callback allows `main.rs` to print the line immediately.

   Internally this is an iterative-deepening downward pass from the current PPV
   length, reusing the transposition table and move-ordering history and clearing
   only path-dependent state between probes.

`Search::solve` may be kept for backward compatibility (returning the final SPPV
or the best line found before timeout), but the CLI should use the staged API.

### `src/main.rs`

Rewrite `main` to orchestrate the stages:

```rust
let mut search = Search::new(64);
search.set_timeout(5);
search.set_epsilon(epsilon);

let outcome = search.solve_outcome(&mut pos);
print_outcome(outcome);
if outcome == Outcome::Draw {
    // Either a true draw or a timeout. The timeout message is already printed
    // by a helper if needed.
    return;
}

if let Some(ppv) = search.find_ppv(&mut pos, outcome) {
    print_pv(&ppv);

    search.refine_sppv(&mut pos, outcome, |shorter| {
        print_pv(shorter);
    });
}

if search.time_exceeded() {
    println!("timeout");
}
```

The timeout is checked after each stage. If it is reached, the program prints
`timeout` and exits. Because outcome and PPV are printed immediately when known,
nothing is lost.

### `--no-refine-shortest`

Redefine this option to mean: "find and print the outcome and the PPV, but do not
refine toward the SPPV." This matches the name and gives the user a fast way to
get a correct proof line without the extra cost of shortest-forced-win refinement.

Optionally rename it to `--ppv-only` and keep `--no-refine-shortest` as a hidden
alias for backward compatibility. The plan will keep the existing name for now.

## CLI output format

For decisive outcomes:

```text
outcome: win
pv: <ppv>
pv: <shorter ppv>
pv: <sppv>
```

If the timeout is hit after the PPV but before the SPPV:

```text
outcome: win
pv: <ppv>
timeout
```

For draws:

```text
outcome: draw
```

If the timeout is hit before any decisive outcome:

```text
timeout
```

(The outcome could be `draw` from the unresolved convention; the timeout message
makes the reason explicit.)

## File changes

### `src/search/dfpn/mod.rs`

- Add `solve_outcome`, `find_ppv`, and `refine_sppv`.
- Keep `solve` as a thin wrapper around these stages for library users.
- Ensure `time_exceeded` is checked and respected when a stage returns.

### `src/main.rs`

- Use the staged API.
- Print outcome immediately.
- Print the PPV when available.
- Print each shorter PPV produced by `refine_sppv`.
- Print `timeout` when appropriate.
- Update help text for `--no-refine-shortest`.

### `src/search/dfpn/pv.rs`

- Add a helper `extract_ppv` (or rename `extract_pv_checked`) that validates that
  the extracted line satisfies the PPV definition (defender replies are longest).
- Keep `validate_pv` as the terminal correctness check.

### `tests/`

Add `tests/test_plan2.rs` or extend `tests/test_plan6.rs` with:

- `m27_streaming_output`: verify that the reported FEN produces `outcome: win`
  followed by a PPV of length 7 starting with `b1b8 g8f7`, and that refinement
  does not find a shorter one.
- `m27_ppv_only`: with `--no-refine-shortest`, verify that a valid PPV is printed
  but no additional shorter PVs are attempted.
- `timeout_message`: a test using `Search::set_timeout` with a very small value
  to verify the `timeout` message is produced (or that the program exits cleanly
  without printing a spurious PV).

## Testing and verification

1. Format, lint, and test:

   ```bash
   cargo fmt
   cargo clippy --all-targets
   cargo test
   cargo doc
   ```

2. Manual CLI checks:

   ```bash
   cargo run --release -- --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26"
   # Expected: outcome: win followed by the 7-plies PV.

   cargo run --release -- --no-refine-shortest --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26"
   # Expected: outcome: win followed by a valid PPV, no further refinement.
   ```

## Risks and mitigations

| Risk | Mitigation |
|------|------------|
| Splitting `solve` into stages exposes timeout state more prominently. | Keep the shared `deadline` inside `Search`; each stage checks `time_exceeded` before returning. |
| `find_ppv` may need additional search if the initial outcome proof is shallow. | Reuse the existing TT and history; the depth-aware loop from plan1 already keeps defender-optimal replies when `refine_shortest` is enabled. |
| Streaming PV output makes unit tests more verbose. | Provide a `Search` API that returns lines, and test that separately from the CLI stdout. |
| `--no-refine-shortest` now means "PPV only", which is a breaking semantic change. | Update help text and consider keeping the old name as an alias or printing a deprecation warning. |

## Success criteria

- Default CLI run on `6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26` prints
  `outcome: win` and a 7-plies PPV, and if time allows the same 7-plies SPPV.
- `--no-refine-shortest` prints `outcome: win` and a valid PPV, then exits without
  searching for a shorter line.
- `cargo test` and `cargo clippy --all-targets` pass with no new warnings.
- Timeout at any stage produces only an additional `timeout` line and exits cleanly.
