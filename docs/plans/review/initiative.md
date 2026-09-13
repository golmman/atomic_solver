# Initiative: `review` — full-solver code review series

## Status

**Closed.** Plans 1–18 are done (`report1.md`–`report18.md`, plus the
source review that acts as report0); last activity 2026-07-19. The
initial systematic review was fully worked off; later code-quality work
is `cleanup/`'s territory.

## Arc (summary)

A plan-per-review-finding series over the early codebase: terminal/outcome
classification fixes (checkmate vs. stalemate, plan1), solver-correctness
repairs, API and visibility cleanups, module splits (including the
`dfpn`/`tt` split in plan18), and the PV 1000-ply cap safety. It
established the plan→report cadence the later initiatives still follow.

## Successor

- Housekeeping and lint work: `cleanup/`.
- Correctness follow-ups on repetition/terminal semantics:
  `dfpn/` (GHI line of work).
