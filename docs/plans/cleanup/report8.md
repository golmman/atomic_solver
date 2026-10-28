# Report 8: CLI layer verbosity — option-parsing helpers, single-sourced help, no pre-exit hook indirection

Plan: [`plan8.md`](plan8.md). Initiative: `cleanup`.
Executed 2026-10-28 in one session on top of commit `54a39e2`.
Measurements: [`measurements/plan8/`](measurements/plan8/README.md).

**Verdict: shipped, behaviour-free.** Bar D2 (help/output byte identity),
D3 (`pre_exit:`) and D4 (anchors, lint, gate) pass. Bar D1's *size*
sub-bar is missed (`src/cli.rs` 396 lines vs ≤ 370; total 704 vs ≤ 660)
— see §5: the figure is a drafting arithmetic error, and reaching it
would mean deleting the blank-line separators between tests, not
removing code.

## 1. What changed

| file | before | after | delta |
| --- | --- | --- | --- |
| `src/cli.rs` | 488 lines / 18.1 KB | 396 lines / 14.0 KB | −92 |
| `src/main.rs` | 360 lines / 16.6 KB | 308 lines / 12.2 KB | −52 |
| **CLI layer total** | **848** | **704** | **−144 (−17 %)** |
| `src/lib.rs` | 12 lines | 18 lines | +6 (finding F1) |
| `tests/test_cli.rs` | 16 tests | 17 tests | +1 (`cli_help_matches_golden`) |
| `tests/fixtures/cli_help_golden.txt` | — | 2,601 B | new |
| `src/cli.rs` unit tests | 24 | 25 | +1 (`startpos_matches_position_starpos`) |

Removed constructs:

- **12 hand-rolled option arms** (`parse_args` 131 → 76 lines): the
  repeated "next argv / missing-value error / parse / validate / `i += 2`"
  shape is now two helpers (`value`, `number<T>`); range checks stay
  inline because their messages differ per option. Arms that only set a
  flag keep their own `i += 1`.
- **16 identical test prologues** (the `match parsed { Options(o) => …,
  Help => panic!() }` block) → one `options()` helper; the 12-field
  `CliOptions` destructure disappeared from `cli.rs` (it remains in
  `main.rs`, where the fields are consumed).
- **49 `println!`s** in `print_help` → one `const HELP` raw string plus
  one `HELP.replace("{program}", program)`.
- **The duplicated option list in the `main.rs` module header** (~50
  lines) → an 8-line pointer to `print_help()` / `CliOptions::default()`.
- **`type PreExitHook = Box<dyn FnOnce(ExitReason, Outcome, u64, &[Move]) + Send>`**
  and its one closure → an inline `if !outcome_only { println!("pre_exit: …") }`
  at the same sequence point (the stdin-reader thread stays, now directly
  under the same guard). `pv` is no longer passed anywhere.
- **The `cut_short` / `exit_reason` double dispatch** → one nested
  `match` (behaviour identical, `ExitReason::MemoryLimit` still prints
  `timeout`).

Kept deliberately (plan D-2, re-justified): `src/cli.rs`'s
`STARTPOS_FEN` duplicate. Its old justification ("so that the CLI default
stays `std`-only and does not pull in the solver crate") was false — the
real reason is that the module is compiled in two crate roots — and is
now stated correctly in the file header, backed by the new drift guard.

## 2. Error-message normalization (plan A-5, the only observable change)

Numeric-parse messages are now uniformly `error: invalid <option> value:
<e>`. Two changed spelling, both gaining the `--` prefix:

| invocation | before | after |
| --- | --- | --- |
| `--epsilon x` | `error: invalid epsilon value: invalid float literal` | `error: invalid --epsilon value: invalid float literal` |
| `--timeout x` | `error: invalid timeout value: invalid digit found in string` | `error: invalid --timeout value: invalid digit found in string` |

All other messages are byte-identical, including the deliberately
prefix-less range messages (`epsilon must be in [0.0, 1.0]`,
`timeout must be positive`) — full matrix in
`measurements/plan8/state/error_messages_before_after.txt`. No test
asserted either spelling. Exit codes unchanged (1).

## 3. Both new tests

1. `src/cli.rs::tests::startpos_matches_position_starpos` — the
   `AGENTS.md`-named `M19_FEN` / `STARTPOS_FEN` follow-up's drift guard:
   pins the CLI constant against `Position::STARTPOS_FEN`. Required a
   one-line deviation, finding F1.
2. `tests/test_cli.rs::cli_help_matches_golden` — the rendered `--help`
   bytes vs `tests/fixtures/cli_help_golden.txt`. The plan's `Command::arg0`
   sketch needs `std::os::unix::process::CommandExt` (`arg0` is
   Unix-only), so the test normalizes the invoked path to `atomic_solver`
   instead — host-agnostic, and it is exactly the substitution the four
   other `--help` substring assertions in the file do not check.

Existing tests: none deleted, renamed or weakened (B-4). Two were
re-expressed as loops over their original argument lists
(`help_short_and_long`, `missing_value_returns_err`); `defaults_match_
documentation` keeps all 12 field assertions.

## 4. Re-measured anchors (Bar D2/D3/D4)

| anchor | expected | measured |
| --- | --- | --- |
| quick @ 256 | 59 cases, 42,109,768 child evals, 0 wrong, 0 timeouts | 59 / 42,109,768 / 0 / 0 |
| dec31 FO | `evals: 510426`, `outcome: loss length: 22` | identical |
| m22 FO | `evals: 15023911`, `outcome: win length: 75` | identical |
| `--help` stdout | HEAD | **byte-identical** (2,601 B, `diff` empty) |
| dec31 stdout, default mode | HEAD | **byte-identical** (incl. `pre_exit:`) |
| dec31 stdout, `--outcome-only` | HEAD | **byte-identical** |
| dec31 stderr (`evals:`) | HEAD | identical |

The "before" side was rebuilt from `git archive HEAD` (not remembered
numbers). One stderr line, `[bounded_search] chunk done: … elapsed=…
nps=…`, is nondeterministic — two runs of the *same* HEAD binary differ
only there — so stderr comparison is made with those lines filtered;
`evals:` is exact.

Gate: `make test` green (~1 m 13 s test time), `cargo clippy
--all-targets` and `cargo doc --no-deps` warn-free, `cargo fmt` clean.
`tests/test_deep_outcomes.rs::m27_ppv_only` (the only `pre_exit:`
assertion) run explicitly with `--include-ignored`: green.
`tests/test_cli.rs`: 16/16 green. Targeted cli unit tests: 25/25.

## 5. Bars

- **D2** pass (`main.rs` 308 ≤ 310, help bytes identical, fixture green).
- **D3** pass (both stdout captures identical, `pre_exit:` prefix
  assertion green).
- **D4** pass (anchors, lint/doc, gate, `git status` — plus
  `src/lib.rs`, finding F1, and the measurements directory).
- **D1** tests/fmt/no-test-removed pass; **size sub-bar missed**:
  396 > 370. This is a drafting arithmetic error, not lost
  deduplication: the plan assumed the test module would collapse to
  "one or two lines per test", but each test carries `#[test]` + `fn … {`
  + `}` + a blank separator, i.e. 4 lines minimum at 25 tests ≈ 100
  lines, and B-2/B-3 mandate the 12-assert defaults test and the
  9-argument missing-value test. Measured slack: the file contains
  exactly 27 blank lines in the test module; deleting 26 of them hits 370
  with **zero** code change. That is a readability regression in the very
  file the plan exists to de-noise, so it was not done. The same
  mis-estimate is why the plan's "≤ 660 total" target (= 370 + 290) is
  missed by the shipped 704.

## 6. Findings (found, not fixed — reported, not expanded)

**F1 — the plan's B-5 test cannot compile as written (fixed, scope
touched `src/lib.rs`).** Plan D-2 correctly states that neither
`crate::position::…` (fails in the binary root) nor
`atomic_solver::position::…` (fails in the lib-test root) resolves in
both crate roots — and then B-5 uses the `atomic_solver::` form. Minimal
fix chosen: `extern crate self as atomic_solver;` in `src/lib.rs` (one
documented statement), which makes the path resolve in both roots and
leaves the guard where the plan wants it. Rejected alternatives:
relocating the test into `tests/` (cannot name the private CLI constant)
and publishing `cli` as a library module (explicitly out of scope).
`src/lib.rs` is therefore an extra file in `git status` beyond D4's
list.

**F2 — the help text has five `{program}` sites, not four** (usage line
+ 4 examples). The const uses all five.

**F3 — `AGENTS.md`'s `src/main.rs` bullet still enumerates a stale
option set** (`--fen`, `--tt-size`, … `--no-preflight`) that omits
`--salt`, `--budget` and `--config`. The plan's C-4 edit replaced the
sentence pointing at the module header, but the enumeration itself is
untouched drift of the same kind the plan removed from the module
header. Left as-is (out of scope; it is now the only option list besides
`print_help`, so either delete it next pass or accept it as descriptive).

**F4 — `CliOptions::no_preflight`'s moved memory note needed an
antecedent.** The header sentence "On such positions the pre-phase
allocates …" relied on the preceding paragraph. The field doc now names
the fixpoint itself, so the moved sentence reads correctly standalone;
otherwise the wording is the plan's.

**F5 — the plan's D-3 snippet would not survive `cargo fmt`.** Its
`println!("{}", match … { … })` form is re-wrapped by rustfmt; shipped
as a `println!` with the `match` as its own argument. Behaviour
identical.

**F6 — test-helper shape refinement.** `options()` prepends the program
name instead of each call spelling `"atomic_solver"` (keep-going
effect: a test "parses this option list"). Tests that exercise `argv[0]`
handling still go through `parse_args(&args(…))` directly, so B-3's
"same arguments" holds.

**F7 — no wall-clock measurement** was taken: the plan declares the
change cosmetic and every correctness/determinism anchor above is exact;
a timing cell on the CLI layer would only measure process startup noise.

## 7. Next steps

- Plans 9–11 of the review (`dfpn/children.rs` hot-path tidy,
  `proof_tree` proven-depth rule + dead protocol, tooling DRY + the
  owner-gated `reconstruct_pt --experiment` retirement) are drafted and
  unblocked; they are independent of this one.
- The `M19_FEN` / `STARTPOS_FEN` follow-up is now half-closed here
  (`STARTPOS_FEN` pinned); plan11 single-sources `M19_FEN`.
- If a future session wants the 660-line target to mean something,
  restate it as "code lines" (statement count) rather than `wc -l`;
  blank-separator accounting makes `wc -l` a poor proxy in test-heavy
  files.
