# Plan 8: CLI layer verbosity — option-parsing helpers, single-sourced help, no pre-exit hook indirection

Initiative: `cleanup` (housekeeping: DRY, YAGNI, lint, module sizing).
Opened by: an owner-requested code-size/readability review session
(2026-10-28) whose findings are the input to plans 8–11. This plan is
**cosmetic only**: it must be evaluable as "no observable behaviour
changed", with one deliberate exception noted in A-5 (an error-message
wording normalization asserted by no test).

Plan numbering: `cleanup` plans 1–7 are done (`report1.md`–`report7.md`);
this is the next free number.

## Prerequisite reading (self-contained)

- `AGENTS.md` §"Conventions" (file-size rule: under ~10 KB, documented
  justification above) and §"Architecture" → the `src/main.rs` bullet,
  which currently claims "The full option, output (`pre_exit:` /
  `preflight:` / `pv_status:` lines), and offline-proof contracts are in
  the module header." Phase C amends that sentence (see C-4).
- `src/cli.rs` — 488 lines / 18.1 KB. Lines 94–224 (131 lines) are
  `parse_args`: one `match` arm per option, each arm repeating the same
  "get the next argv, error if missing, parse, validate, `i += 2`" shape
  12 times. Lines 239–488 are the test module (24 `#[test]`s): 16 of them
  repeat the same
  `match parsed { ParseResult::Options(o) => …, ParseResult::Help => panic!(…) }`
  prologue, plus one 12-field destructuring in `defaults_match_documentation`.
- `src/main.rs` — 360 lines / 16.6 KB. Module doc lines 8–57 restate the
  option set a *third* time (after the `CliOptions` field docs and
  `print_help`); `print_help` (lines 108–158) is 49 `println!` calls;
  `type PreExitHook = Box<dyn FnOnce(ExitReason, Outcome, u64, &[Move]) + Send>`
  (line 187) is constructed at exactly one site, called at exactly one
  site, and its `&[Move]` parameter is ignored (`_pv`).
- `tests/test_cli.rs` — asserts only *substrings* of `--help`
  (`--fen`, `--timeout`, `--epsilon`, `--tt-dump-path`) and of the
  outcome/PV lines; nothing pins the full help text.
- `tests/test_deep_outcomes.rs:160-168` — the only assertion on a
  `pre_exit:` line (prefix match).
- `tests/fixtures/m22_default_stdout_golden.txt` + `tests/test_trajectory_golden.rs`
  — the default-mode stdout golden; it runs with `--outcome-only`, so it
  does **not** cover `pre_exit:`.

## Why this plan exists

The CLI entry layer is ~850 lines and carries no search logic, yet it is
the most mechanically repetitive code in the repository:

1. **The parser repeats one 5-line shape 12 times.** Adding an option
   means adding another copy (the mistake the shape invites is silently
   forgetting `i += 2`, which the missing-value tests would catch only by
   accident).
2. **The test module repeats one 4-line prologue 16 times** (out of 24
   tests). Each test is 3× longer than its assertion; the resulting noise
   hides the actual contract (one line per option).
3. **The option surface is stated three times** (`CliOptions` field docs +
   `print_help` + the `main.rs` module header) with no mechanical link,
   so the three texts have already drifted in detail (e.g. the header
   documents `--no-preflight`'s ~85 MB bounded exception, `print_help`
   does not; the header labels `--tt-size`'s default "256", the help says
   "(default: 256)").
4. **`PreExitHook` is an unnecessary abstraction**: a `Box<dyn FnOnce>`
   with one construction site and one call site, whose last parameter is
   unused. It exists to be generic over nothing.

Net target: **~190 lines removed** (CLI layer 848 → ≤ 660 lines) with
byte-identical stdout contracts.

## Scope

**In scope**

1. Phase A — `parse_args` rebuilt on three small helper functions
   (`value`, `number`, and an inline range check), one option per 1–5
   lines, with the error-message surface unchanged except A-5.
2. Phase B — the test module's prologue collapsed into one `options()`
   helper; `defaults_match_documentation` asserted field-by-field
   through it; **one test added** (D-1's `STARTPOS_FEN` drift guard).
3. Phase C — `print_help` becomes the single user-facing option text
   (one `const HELP` string + one `replace`), the module header's option
   list is replaced by a pointer, and `AGENTS.md`'s claim is corrected.
   `--help` output must stay **byte-identical**.
4. Phase D — `PreExitHook` deleted; the `pre_exit:` line is printed
   inline at the same point in the sequence, byte-identically. The
   `STARTPOS_FEN` duplicate is kept (rationale in D-2) with its false
   justification corrected.

**Out of scope (non-goals)**

- Any change to `Search`, `Position`, ordering, TT, pre-flight, or the
  budget contract. This plan touches `src/cli.rs`, `src/main.rs`,
  `AGENTS.md`, one test file and its fixture only.
- Changing *which* options exist, their defaults, or their validation
  ranges.
- `CliOptions`' field set and `ParseResult` (a two-variant enum where one
  variant means "help" is mildly redundant, but it is a clean public
  contract in the test-only module; changing it buys ~5 lines and costs a
  reviewer question).
- `examples/` and `tests/` beyond the two files named above.
- Making `src/cli.rs` a non-test library module (that would be the only
  way to single-source `STARTPOS_FEN`; see D-2 for why it is not worth a
  new `pub mod`).

## Normative design

### Phase A — parser helpers

Add above `parse_args`:

```rust
/// Take the value of the option at `args[*i]` and advance past it.
fn value<'a>(args: &'a [String], i: &mut usize, name: &str) -> Result<&'a str, String> {
    let v = args
        .get(*i + 1)
        .ok_or_else(|| format!("error: {name} requires a value"))?;
    *i += 2;
    Ok(v.as_str())
}

/// Parse the value of the option at `args[*i]` as `T`.
fn number<T>(args: &[String], i: &mut usize, name: &str) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value(args, i, name)?
        .parse::<T>()
        .map_err(|e| format!("error: invalid {name} value: {e}"))
}
```

The arms then become (this is the whole normative shape; reproduce it for
every option):

```rust
"--fen" => opts.fen = value(args, &mut i, "--fen")?.to_owned(),
"--tt-size" => {
    let v: usize = number(args, &mut i, "--tt-size")?;
    if v == 0 {
        return Err(format!("error: --tt-size must be positive, got {v}"));
    }
    opts.tt_size = v;
}
"--epsilon" => {
    let v: f64 = number(args, &mut i, "--epsilon")?;
    if !(0.0..=1.0).contains(&v) {
        return Err(format!("error: epsilon must be in [0.0, 1.0], got {v}"));
    }
    opts.epsilon = v;
}
"--first-outcome" => {
    opts.first_outcome = true;
    i += 1;
}
```

Rules:

- A-1. Every existing error string is preserved verbatim **except** the
  numeric-parse messages, which become uniformly
  `error: invalid <option> value: <e>` (`--epsilon` was the only arm
  spelling it without the `--` prefix). No test asserts either spelling
  (`tests/test_cli.rs` asserts only `unknown option`); record the
  normalization in `report8.md`.
- A-2. `--tt-size`, `--timeout`, `--budget` keep the `must be positive`
  wording; `--epsilon` keeps `epsilon must be in [0.0, 1.0]` (no `--`);
  `--refine-cap` keeps `--refine-cap must be non-negative`.
- A-3. The leading "ignore `argv[0]` if it does not start with `-`" rule
  and the `-h`/`--help` early return are unchanged.
- A-4. `ParseResult` and `CliOptions` are unchanged (no field added,
  removed, reordered, or re-defaulted).
- A-5. (the only deliberate observable change) the two numeric-parse
  messages above.

### Phase B — test module

```rust
fn options(list: &[&str]) -> CliOptions {
    match parse_args(&args(list)).expect("parse ok") {
        ParseResult::Options(o) => o,
        ParseResult::Help => panic!("unexpected help"),
    }
}
```

- B-1. Every parser test becomes `let o = options(&["atomic_solver",
  "--tt-size", "128"]); assert_eq!(o.tt_size, 128);` — one or two lines.
- B-2. `defaults_match_documentation` keeps all 12 assertions but reads
  `o.<field>` instead of destructuring `CliOptions`. The 12-field
  destructure disappears from `cli.rs` (it survives only in
  `src/main.rs`, where the fields are actually consumed).
- B-3. `help_short_and_long`, `unknown_option_returns_err`,
  `removed_proof_tree_options_rejected`, `missing_value_returns_err`, and
  the four rejection tests keep their exact assertions and arguments.
- B-4. No test is deleted, weakened, or renamed (the file's 24 `#[test]`s
  stay; one is added by B-5).
- B-5. **One test added** (the `AGENTS.md` named follow-up "`M19_FEN` /
  `STARTPOS_FEN` duplicate"):

```rust
/// The CLI keeps its own `STARTPOS_FEN` because this module is compiled in
/// two crate roots (lib-test and bin); pin the two constants together so
/// they cannot drift.
#[test]
fn startpos_matches_position_starpos() {
    assert_eq!(STARTPOS_FEN, atomic_solver::position::Position::STARTPOS_FEN);
}
```

### Phase C — one help text, one header pointer

- C-1. Replace `print_help`'s 49 `println!` calls with a single
  `const HELP: &str` (a `r#"…"#` literal containing `{program}` in the
  four places that interpolate it) and

```rust
fn print_help(program: &str) {
    println!("{}", HELP.replace("{program}", program));
}
```

  The rendered bytes must be **identical** to HEAD's (bar D-2); the
  easiest way to guarantee that is to copy HEAD's `--help` output, turn
  the four `atomic_solver` occurrences back into `{program}`, and re-diff.
- C-2. Delete the module header's option list (`src/main.rs` lines ~8–57)
  and replace it with:

```rust
//! Usage:
//!   `atomic_solver` [OPTIONS]
//!
//! The user-facing option text is `print_help()` (`--help`); the defaults
//! live in `CliOptions::default()` (`src/cli.rs`), and the per-option
//! contract for flags with non-obvious resource behaviour is stated next to
//! the field that implements it. The output contract below is normative.
```

- C-3. Keep the header's `Output:` block and `Examples:` block verbatim
  (they are the `pre_exit:` / `preflight:` / `pv_status:` / `evals:`
  contract and are referenced by `AGENTS.md`), and keep the one-line
  `--no-preflight` memory note by moving it into `CliOptions::no_preflight`'s
  field doc in `src/cli.rs` — that field doc today says only "Disable the
  detector-gated bounded pre-phase (plan13)". The moved sentence is:
  "On such positions the pre-phase allocates a transient closure of up to
  1,000,000 packed positions (~85 MB worst case) — the documented bounded
  exception to 'RAM = TT only'."
- C-4. Amend `AGENTS.md`'s `src/main.rs` bullet: replace "The full option,
  output (`pre_exit:` / `preflight:` / `pv_status:` lines), and
  offline-proof contracts are in the module header." with "The output
  (`pre_exit:` / `preflight:` / `pv_status:` lines) and offline-proof
  contracts are in the module header; the user-facing option text is
  `print_help()` and the option defaults live in `src/cli.rs`'s
  `CliOptions`." No other `AGENTS.md` edit.
- C-5. New fixture `tests/fixtures/cli_help_golden.txt` containing HEAD's
  byte-exact `--help` output (with `{program}` already substituted for
  the binary name the test invokes), asserted by a new fast test in
  `tests/test_cli.rs`:

```rust
#[test]
fn cli_help_matches_golden() {
    // … run the binary with --help, assert stdout == include_str!(…)
}
```

  This is the *only* new integration test in this plan and it is the
  mechanical guard against the drift this phase removes (an option that
  exists but is undocumented, or a stale `help` line).

### Phase D — delete the hook indirection, keep the const

- D-1. Delete `type PreExitHook`, the `let hook: Option<PreExitHook> = …`
  block's closure (the stdin-reader thread stays — it is functional), and
  the trailing `if let Some(hook) = hook { hook(...) }`. Emit inline at
  the **same point in the sequence**:

```rust
    drop(search);

    if !outcome_only {
        println!("pre_exit: reason={search_exit_reason} outcome={outcome} nodes={search_nodes}");
    }
```

  `search_exit_reason` and `search_nodes` are already captured before the
  TT-snapshot block; keep that capture order so the printed values are
  unchanged. `pv` is no longer passed anywhere (it was `_pv`).
- D-2. **Keep** `src/cli.rs`'s `STARTPOS_FEN` duplicate, for a correct
  reason: `src/cli.rs` is compiled in two crate roots (`#[cfg(test)] mod
  cli;` inside the library, and `mod cli;` inside the binary), and a
  single path cannot resolve in both (`crate::position::Position::…`
  fails in the binary; `atomic_solver::position::Position::…` fails in
  the lib-test build; both were verified in the drafting session). Replace
  the current false justification

  > so that the CLI default stays `std`-only and does not pull in the
  > solver crate

  with the true one ("compiled in two crate roots; the equality with
  `Position::STARTPOS_FEN` is pinned by `startpos_matches_position_starpos`").
- D-3. Collapse the `cut_short` / `exit_reason` double dispatch into one
  `match` (behaviour identical):

```rust
    let exit_reason = search.exit_reason();
    if search.time_exceeded() || exit_reason == ExitReason::BudgetExhausted {
        println!("{}", match exit_reason {
            ExitReason::Quit => "quit",
            ExitReason::BudgetExhausted => "budget exhausted",
            _ => "timeout",
        });
    }
```

  `ExitReason` already derives `PartialEq`. `ExitReason::MemoryLimit`
  still prints `timeout` (unchanged; it is unreachable from the CLI).

## Phase 0 — baseline capture (read-only)

Captured by the drafting session at commit `7c339bb` (re-verify in the
execution session; a mismatch is a stop-and-investigate event):

| anchor | command | expected |
| --- | --- | --- |
| quick @ 256 | `./target/release/examples/benchmark --suite quick --first-outcome --timeout 3 --runs 1 --tt-size 256 --json` | 59 cases, total `child_evals` 42,109,768, 0 wrong, 0 timeouts |
| dec31 FO | `./target/release/atomic_solver --fen 'r1bq1k2/ppN3p1/2p1p2P/6P1/1b1P1p2/8/PP3P2/R2K1B1R b - - 0 15' --timeout 20 --first-outcome` | `evals: 510426`, `outcome: loss length: 22` |
| m22 FO | `./target/release/atomic_solver --fen '4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22' --timeout 60 --first-outcome` | `evals: 15023911`, `outcome: win length: 75` |

Also capture, into `measurements/plan8/state/`:

1. `help_before.txt` — `./target/release/atomic_solver --help` stdout.
2. `stdout_default_before.txt` — dec31 FEN, non-`--outcome-only`,
   stdout **and** stderr (the `pre_exit:` line lives here).
3. `stdout_outcome_only_before.txt` — dec31 FEN with `--outcome-only`.
4. `wc -l src/cli.rs src/main.rs` and `git status --porcelain`.

## Execution phases and bars

**Bar D1 (Phase A+B)** — `cargo test --release cli::` green with the same
24 tests plus the new drift guard; `cargo fmt` clean; no test
removed; `src/cli.rs` ≤ 370 lines.

**Bar D2 (Phase C)** — `diff help_before.txt <after>` is **empty**;
`cli_help_matches_golden` green; the four `--fen`/`--timeout`/`--epsilon`/
`--tt-dump-path` substring assertions in `tests/test_cli.rs` still pass
unchanged; `src/main.rs` ≤ 310 lines.

**Bar D3 (Phase D)** — `stdout_default_before.txt` byte-identical
(stdout *and* stderr, i.e. the `pre_exit:` line and the `evals:` line);
`stdout_outcome_only_before.txt` byte-identical; `tests/test_deep_outcomes.rs`'s
`pre_exit:` prefix assertion green.

**Bar D4 (whole plan)** — the three Phase 0 anchors are child-eval
identical (they must be: no search code is touched — this bar exists to
catch an accidental edit in the shared files); `cargo clippy --all-targets`
and `cargo doc` produce no new warnings; `make test` green;
`git status --porcelain` lists only `src/cli.rs`, `src/main.rs`,
`tests/test_cli.rs`, `tests/fixtures/cli_help_golden.txt`, `AGENTS.md`,
`docs/plans/cleanup/{plan8.md,report8.md,initiative.md}` and
`docs/plans/cleanup/measurements/plan8/`.

**Rollback rule** — Phase A/B and Phase C are independent; if D1 fails,
revert Phase A+B only and record why (D2/D3 then still proceed). If D2
fails on a *semantic* difference (not whitespace), restore `print_help`
from HEAD and keep Phases A/B/D.

**Size target** — `wc -l src/cli.rs src/main.rs` totals ≤ 660 (from 848).

## Tests to add / change

1. `cli::tests::startpos_matches_position_starpos` (B-5, new).
2. `tests/test_cli.rs::cli_help_matches_golden` (C-5, new).
3. `defaults_match_documentation` and every parser test: rewritten to use
   `options(..)` (B-1/B-2), assertions unchanged.
4. No test deleted or weakened.

## Measurement artifacts

`docs/plans/cleanup/measurements/plan8/` (per `AGENTS.md`'s fixed layout):
`README.md` (provenance table + reproduce commands), `env.json` (commit,
host, rustc), `state/` (the four Phase 0 captures), and the parsed
before/after line counts. No solver transcripts, no snapshots.

## Final task

Write `docs/plans/cleanup/report8.md`: the before/after line counts per
file, the removed-construct inventory (`PreExitHook`, 12 parser arms, 16
test prologues, 49 `println!`s, the duplicated option list), the
error-message normalization from A-5, both new tests' provenance, the
anchor table re-measured after the change, and anything found but not
fixed (report it, do not expand scope).
