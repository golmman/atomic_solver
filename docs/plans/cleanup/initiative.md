# Initiative: `cleanup` — housekeeping: DRY, YAGNI, lint, module sizing, history surgery

## Status

**Active as-needed** (Boy-Scout maintenance, not a standing backlog).
Plans 1–6 are done (`report1.md`–`report6.md`); a new plan is opened when
a pass leaves named follow-ups or a convention lands that needs
enforcement.

**plan7 done 2026-10-27** (`report7.md`). It closed the housekeeping
follow-up carried in the `conversion/report12.md` closure block: (A) the
missing deterministic restart-schedule tests (non-zero `cutoff` firing
mid-run, `Growth::Linear` end-to-end), (B) recording the `m21_white` /
`m22_white_wins` re-baseline decisions plus the missing `AGENTS.md` size
justifications, and (C) splitting `src/search/dfpn/mod.rs` (40.8 KB /
986 lines, not justified) into `mod.rs` + `config.rs` + `run.rs` by
move-only code motion. The `dfpn/mod.rs` size watch in the index row is
now resolved.

**plan8 done 2026-10-28** (`report8.md`). CLI-entry verbosity: `parse_args`
rebuilt on a `value` / `number<T>` helper pair, the test module's 16
repeated prologues collapsed into one `options()` helper, `print_help`'s
49 `println!`s replaced by one `const HELP` (byte-identical `--help`, now
pinned by `tests/fixtures/cli_help_golden.txt`), the module header's
duplicated option list reduced to a pointer, `PreExitHook` deleted
(inline `pre_exit:`), and the `STARTPOS_FEN` duplicate re-justified and
pinned by a drift guard. 848 → 704 lines in the CLI layer. The plan's
size figures (cli ≤ 370, total ≤ 660) were drafting mis-estimates — the
shortfall is blank-line separators between tests, not code
(`report8.md` §5); bars D2/D3/D4 all pass, D1's size sub-bar is the one
miss.

**plan9 done 2026-10-28** (`report9.md`). `dfpn/children.rs` readability
pass (hot path, identity bars): `impl ChildInfo` with `solved` /
`unsolved` / `unevaluated` constructors replaced all 11 inline literals,
the two duplicated unsolved-bounds ladders collapsed into one
`ChildInfo::unsolved_bounds` helper (both call sites keep
`entry.as_ref()`), and the 666-line test module moved verbatim to
`src/search/dfpn/children/tests.rs` (21 test names unchanged). Census:
`ChildInfo {` 15 → 5, `repetition_seen: false` 10 → 2,
`.pn_dn_for(..).0/.1` 3 → 0, ladder bodies 2 → 1; production file
1,334 lines / 57.8 KB → 624 / 29.2 KB. Quick suite both TT sizes,
dec31/m22/m23 FO, and the m22 default golden are exact (no
re-baselining); interleaved wall medians 0.960–1.009×. All gates green
including `make test-full`.

**plan10 done 2026-10-29** (`report10.md`). Offline proof-tree layer:
the proven-depth rule now lives once in a private `fn proven_depth`
(Win ⇒ min+1, Loss ⇒ max+1) used by both canonical-twin selection and the
finalize recompute, dropping the per-node `Vec<u32>`; the dead
`ProofTreeWorkerHandle::dump_to_bin` / `DumpToBin` protocol (no caller —
consumers use `handle.tree()` + `to_bin`) is deleted; the vestigial
`impl Default for Position` is deleted (documented
`clippy::new_without_default` allow instead); and the `ProofEvent::Clear`
decision was taken as **P1**: the variant, the worker's `clear()`, its
handler arm and its one unit test are gone, so `ProofEvent` has one
variant and the enum survives as the `search`↔`proof_tree` decoupling
contract (P2 collapse stays recorded as not-taken). `worker.rs`
663 → 611 lines, `-clear`/`-dump` protocol plumbing net −129 diff lines;
the proof-tree binary dump is byte-identical (`ab94c69f…`), the three
search anchors are child-eval identical, gates green.

**plan11 done 2026-10-29** (`report11.md`). Tooling DRY + one retirement:
`ScorerParams::validate`'s check 1 generates its 27 field names from a
local `macro_rules!` instead of retyping them (the `pieces.` message
spelling preserved); the benchmark fixture parser is one file
(`tests/common/fixture_parse.rs`) included by path from both
`tests/common/mod.rs` and `examples/common.rs` (normalized duplication
45 → 2 lines); `M19_FEN` is single-sourced in `examples/common.rs`; the
proofdb win-in-1 tree fixture lives once in `examples/proofdb/fixture.rs`
and all four test harnesses use it (FEN copies 6 → 2); and the owner-gated
retirement was taken — `reconstruct_pt --experiment` plus `LiveBuild`,
`run_experiment`, `CaseRow`, `Summary`, `aggregate` are deleted
(662 → 336 lines), `--snapshot`/`--oracle` and `HoleRow` kept. Phase B
**pivoted on measurement**: `config.toml` was not a stale copy of the
defaults but the only copy of the external optimizer's tuned set, so the
stale part was its header claim; the header now says so and the file is
pinned by validity, not by value (owner decision, `report11.md` §3).
Anchors child-eval exact, dump sha256 identical, gates green.

## Goal

Tighten the codebase without changing any game-theoretic result: dead
code, duplication, visibility, clippy warnings, file-size limits, and
one-off structural repairs.

## Done

- **plan1** — dead code, duplicated constants, visibility tightening
  (`report1.md`).
- **plan2** — DRY/YAGNI/consistency + module sizing pass; mechanical
  pedantic clippy fixes (`report2.md`).
- **plan3** — history surgery reverting the failed NN era to pre-NN
  while keeping the general improvements (test tiers, deterministic
  eval budget, new test positions, atomic-movegen 2.1.0) (`report3.md`);
  plus follow-up: m22 regression tests made machine-independent
  (deterministic eval-budget tripwires instead of wall-clock caps).
- **plan4** — post-plan13 housekeeping: docs/bookkeeping sync (dfpn/egtb
  rows, egtb refocused to dormant), AGENTS.md condensed 413 → 200 lines
  (Conventions byte-identical, facts relocated to module docs),
  `selection.rs` tests split out, `--no-preflight` CLI e2e test, closure
  memory measured (docs corrected to ~35 MB ladder / ~85 MB worst case),
  pedantic clippy 308 → 167 with the remainder triaged as leave-as-is
  (`report4.md`).
- **plan5** — intermediate code-review round: dead test helpers + dead
  setters removed, `dfpn` public re-exports de-published, vestigial
  `_max_pv_len` params dropped, plan/era-named test files renamed to
  behavior names (`report5.md`).
- **plan6** — removal of the rejected cross-racer solved-facts carrier
  (`src/facts/`, the `search::dfpn` hooks, the `--facts-out` / `--facts-in`
  CLI surface, its tests, and its documentation; behaviour-free) —
  `report6.md`.
- **plan7** — housekeeping pass: `dfpn/mod.rs` split move-only into
  `mod.rs` (14.2 KB) + `config.rs` (10.5 KB) + `run.rs` (17.1 KB),
  restart-cutoff and linear-schedule end-to-end tests added, slow-test
  decisions recorded, `AGENTS.md` size justifications updated
  (`report7.md`). Verdict D1/D2/D3 pass; eval-identical and wall-neutral
  under an interleaved HEAD-vs-after control.
- **plan8** — CLI-entry verbosity, behaviour-free: `parse_args` helper
  pair + one test helper, one `const HELP` (byte-identical `--help`,
  golden-pinned), module-header option list → pointer, `PreExitHook`
  removed, `STARTPOS_FEN` drift guard added; 848 → 704 lines
  (`report8.md`).
- **plan9** — `dfpn/children.rs` constructors + one unsolved-bounds
  ladder + move-only test split into `children/tests.rs`; identity and
  wall bars green (`report9.md`).
- **plan10** — one `proven_depth` rule (no `Vec`), dead `dump_to_bin`
  protocol removed, `Position::default` removed, `ProofEvent::Clear`
  deleted (P1); dump byte-identical, anchors exact (`report10.md`).

## Open follow-ups (carried from report2/report3)

- The `M19_FEN` / `STARTPOS_FEN` cross-file duplicates could be shared
  if they drift again. Both halves are **done**: `STARTPOS_FEN` by plan8
  (`startpos_matches_position_starpos`, `report8.md`), `M19_FEN` by plan11
  (single-sourced in `examples/common.rs`; `tests/stress.rs` keeps one
  literal with a pointer, `report11.md` §4).

(Addressed by plan4: pedantic-lint triage, `selection.rs` test split —
see `report4.md`.)

## In flight

Nothing. The 8–11 set drafted by the 2026-10-28 owner-requested code-size /
readability review (one plan per session, independent, any order) is fully
executed: plans 8, 9, 10 and 11 are done.

The review's non-action findings (kept as decisions, not tasks) are
recorded in `plan11.md` §Decision F (keep `RestartSchedule`
configurability + `chunk_growth`) and `plan10.md` §D2 (keep the
`ProofEvent` enum even if it stays single-variant).

## Cross-references

- `testability/` — plan3's leftover m22 flakiness was resolved by this
  initiative's report3 follow-up.
- Convention changes (tiered tests, file-size rules) land in AGENTS.md,
  not here.

Per repo convention, every plan ends with the task of writing its
`report<N>.md` in this directory.
