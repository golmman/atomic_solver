# Initiative: `movegen` — cross-repo plans for `atomic_movegen`

## Status

**Special case: not solver work.** The plan here is written *by* this
project *for* the external
[atomic_movegen](https://github.com/golmman/atomic_movegen) repository,
plus the update reports for the releases this solver consumed. Keep the
directory; do not treat it as a solver initiative backlog.

## Arc (summary)

- **plan_has_legal_move.md** — spec for `has_legal_move` /
  `has_legal_move_with_state` (early-exit existence query), shipped in
  movegen **2.2.0** (`report_update_2.2.0.md`): ~5–10× faster than
  `generate_legal` where moves exist; equivalence proven by seeded
  playouts; order golden unchanged after the `for_each_pseudo_legal`
  refactor.
- **report_update_2.0.0.md** — consumer-side notes for movegen 2.0.0.

## Consumer-side impact

- `lean` plan3 integrated the query (`Position::has_legal_move`) into
  the child-eval loop: the ~95% of evaluated children that are never
  searched no longer pay full movegen (−46% wall on m22 first-outcome).
- Future upstream asks (e.g. incremental child hashes for TT-bound
  sort, `Hash`/`Ord` `Move`) should follow the same pattern: a
  standalone plan here, honoring the movegen repo's own conventions.
