# Initiative: `move_order` — ordering heuristics for the m20–m29 class

## Status

**Closed — superseded.** Plans 1–5 are done (`report1.md`–`report5.md`);
last activity 2026-08-20. OR-side move-ordering quality was later
**closed by measurement** in `lean` (oracle floor: best possible
per-node ordering gain 0.509× evals work-weighted; 90.6% of OR-node
work already on the decisive child), which bounds all further hand-tuned
or learned rankers out of scope.

## Arc (summary)

- **plan1** — move-order benchmark suite for the m20–m29 winning line.
- **plan2** — near-commoner heuristics + atomic SEE.
- **plan3** — plan-aware pawn-storm and rook-centralization ordering.
- **plan4** — node-type-aware (OR/AND) ordering; TT-bound initial sort
  implemented then removed as not viable without incremental child
  hashes.
- **plan5** — `ScorerParams` externalized to TOML (`--config`), the
  prerequisite for external tuning.

## Where the open threads went

- Hill-climbing/tuning harness → external optimizer per
  `docs/spec/optimizer_interface.md` (implemented by `tune` plan1).
- History/killer parameter externalization → `lean` backlog #10.
- TT-bound-aware sort (needs incremental child hashes / cached
  summaries) → not re-proposed; bounded by lean's oracle-floor result.
- AND-side ordering signals → `lean` backlog #5 (open).
