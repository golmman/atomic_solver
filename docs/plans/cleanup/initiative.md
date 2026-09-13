# Initiative: `cleanup` — housekeeping: DRY, YAGNI, lint, module sizing, history surgery

## Status

**Active as-needed** (Boy-Scout maintenance, not a standing backlog).
Plans 1–3 are done (`report1.md`–`report3.md`); a new plan is opened when
a pass leaves named follow-ups or a convention lands that needs
enforcement. Last activity 2026-09-04.

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

## Open follow-ups (carried from report2/report3)

- Triage the remaining `clippy::pedantic` warnings (documentation lints
  are the largest category).
- `src/search/dfpn/selection.rs` is near the 20 KB soft limit; split its
  tests out if it grows.
- The `M19_FEN` / `STARTPOS_FEN` cross-file duplicates could be shared
  if they drift again.

## Cross-references

- `testability/` — plan3's leftover m22 flakiness was resolved by this
  initiative's report3 follow-up.
- Convention changes (tiered tests, file-size rules) land in AGENTS.md,
  not here.

Per repo convention, every plan ends with the task of writing its
`report<N>.md` in this directory.
