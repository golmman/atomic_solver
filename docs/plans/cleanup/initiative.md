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

## Open follow-ups (carried from report2/report3)

- The `M19_FEN` / `STARTPOS_FEN` cross-file duplicates could be shared
  if they drift again.

(Addressed by plan4: pedantic-lint triage, `selection.rs` test split —
see `report4.md`.)

## In flight

(none)

## Cross-references

- `testability/` — plan3's leftover m22 flakiness was resolved by this
  initiative's report3 follow-up.
- Convention changes (tiered tests, file-size rules) land in AGENTS.md,
  not here.

Per repo convention, every plan ends with the task of writing its
`report<N>.md` in this directory.
