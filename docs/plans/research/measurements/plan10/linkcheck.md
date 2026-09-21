# Plan 10 link check — 2026-09-21

Verification per `plan10.md` ("every relative link in `structural_floor.md`
resolves"): all `](` targets extracted with `grep -oE` and existence-checked
with a shell loop from this directory. Result:

- `structural_floor.md` — 19 unique relative link targets, **19/19 OK**
  (0 missing; no external URLs used).
- `measurements/plan10/claims.md` — 19 unique relative link targets,
  **19/19 OK** after correcting the initial path depth (links written from
  this directory needed one extra `../` level; fixed in-session and
  re-verified).

Targets covered: `report1–9.md`, `initiative.md`,
`research_alternative_algorithms.md`, `research_child_termination.md`,
`research_deep_dfpn.md`, `measurements/plan9/code-sites.md`,
`../lean/{report9,initiative}.md`, `../conversion/initiative.md`,
`docs/bibliography.md`, `AGENTS.md`.
