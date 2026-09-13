# Initiative: `ultimattt` — porting techniques from the `ultimattt` solver

## Status

**Closed.** Plans 1–5 are done (`report1.md`–`report5.md`); last
activity 2026-07-26. The adopted techniques are now core solver
behavior; further algorithmic work continues in `dfpn/`.

## Arc (summary)

- **plan1** — early exit on a proven winning child (compatible with
  shortest-PV refinement).
- **plan2** — sibling-selection/threshold work (see report2).
- **plan3** — best-child stability + work-based TT replacement
  (`best_child` and `work` fields per entry).
- **plan4** — hybrid depth/work bootstrap (fixed the `max_depth`
  horizon cliff).
- **plan5** — pure work-bounded iterative deepening: doubling `max_work`
  chunks over `u32::MAX` depth, TT/history/killers reused between
  chunks, path-dependent state reset. This is the ancestor of today's
  work-chunk search and the `child_evals` work accounting.

## Successor

- `dfpn/` owns all further search-algorithm levers (thresholds,
  repetition semantics, refinement termination).
