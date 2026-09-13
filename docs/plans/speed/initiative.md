# Initiative: `speed` — wall-time micro-optimizations

## Status

**Closed — superseded by `lean`.** Plans 1–12 are done
(`report1.md`–`report12.md`); last activity 2026-07-22. `lean`
(`initiative.md`) is the direct continuation of the same mandate
(profile-driven wall-time/node reduction with a bit-identical drift
gate) and inherits its method: profile → single lever → measure →
re-rank.

## Arc (summary)

Build-config release options (plan1) through a series of hot-path
levers: repetition-path `HashSet` → stack+linear scan (plan6), TT
generation counter instead of physical `clear()` (plan10), `ChildInfo`
caching across `dfpn` loop iterations (plan11, 2–3.5× wall on
loop-heavy positions), simulation path-stack lending (plan12; plans
predate the twin/simulation removal of `dfpn` plan7, so parts of the
context are historical).

## Successor

- All further wall-time engineering lives in `lean`
  (`docs/plans/lean/initiative.md`), including the drift protocol that
  formalized this initiative's ad-hoc practice.
