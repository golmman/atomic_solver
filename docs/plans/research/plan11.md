# plan11 — Item #17: salt-seeded statistical gate (pilot)

Initiative: `research` (re-opened 2026-10-09). Executes reopened backlog
item **#17**. Evidence base: [`reexamination.md`](reexamination.md) §1/§2/§6;
the closed-record context is [`structural_floor.md`](structural_floor.md)
(with its 2026-10-09 addendum). Self-contained: a fresh session can execute
this without reading the full record.

## Motivation

`reexamination.md` §1 showed that a semantics-neutral perturbation of the TT
bucket index (`PROBE_SALT` env-gated probe code, applied in an isolated copy)
moves the stress case from 249.5 M to 758 M child evals and to censored in
three of five draws, while search semantics are unchanged (full-key
verification intact). Consequence: every per-case single-draw gate decision
in the closed record ("wins on X, regresses control Y") is below the noise
floor on the hard class. Before any lever is re-measured (#18, #19, #20),
the gate itself must be rebuilt as a per-case distribution over salts, with
censored runs handled as right-censored data.

The probe code from `measurements/reexam/` is throwaway and env-gated. This
plan promotes the salt mechanism into a product CLI option so the gate is
reproducible by anyone, then runs a pilot distribution to calibrate the
methodology for the full rollout.

## Objective

1. A `--salt <u64>` CLI option on `atomic_solver` (and the corresponding
   `Search` setter) that remaps TT bucket indices; **salt 0 (default) is
   bit-identical to shipped behavior**.
2. A pilot per-case salt distribution: 6 cases × 5 salts, first-outcome
   `child_evals`, censored runs recorded.
3. A methodology note, `gate_methodology.md` in this directory, defining the
   gate rule that #18/#19/#20 plans must pre-register (final thresholds are
   calibrated on this pilot and fixed in plan12).

## Pre-registered scope decisions

- **D1 — salt mechanism.** In `src/search/tt/table.rs`, `index()` is
  currently `(key as usize) & self.mask` (line ~48). With salt `s > 0` the
  bucket index becomes `(((key ^ s).wrapping_mul(0x9E3779B97F4A7C15)) >> 17)
  & mask`; with `s = 0` the plain `key & mask` path is kept (the mixing
  formula is *not* applied at s = 0 — `(key · G) >> 17 & mask ≠ key & mask`,
  so unconditional mixing would not be bit-identical). Full-key verification
  is unchanged, so hit/miss semantics are identical; only which entries share
  a bucket — and therefore the eviction pattern — moves. The salt is carried
  on the table (set once at construction, before any probe/store), not
  reseeded mid-run.
- **D2 — product surface.** `--salt <u64>` on the `main.rs` CLI + `Search`
  setter; default 0. `benchmark` is *not* touched in this plan (its `--json`
  contract is normative in `docs/spec/optimizer_interface.md`); the pilot
  driver invokes the solver binary directly, as the re-examination probes
  did. TT snapshots (`--tt-dump-path`) are out of scope: salted runs are
  fresh-process outcome-only runs and never dump or reuse snapshots.
- **D3 — pilot corpus.** Six cases, all with named fixtures in
  `tests/fixtures/`: **m20_white, m21_white (= stress), m22_white,
  m23_white** (`move_order_positions.txt`), **dec13, dec10**
  (`decisive_positions.txt`). This spans the known shapes: the m20–m23
  outlier family (heavy-tailed per reexamination §1), dec13 (salt-invariant
  shape), dec10 (mid-distribution control).
- **D4 — run protocol.** Per case, salts {0, 1, 2, 3, 4} (salt 0 doubles as
  the trajectory-identity check). First-outcome mode, 128 MB TT, per-run
  child-eval caps as in `reexamination.md`: stress 2.5 B, all others 1 B;
  wall-clock secondary cap 600 s/run. A run that hits either cap without a
  decisive outcome is recorded **censored** (right-censored at the cap).
  Budget envelope: ≤ 2 h wall on the reference host for all 30 runs
  (worst case ≈ 37.5 B child evals; censored runs dominate it).
- **D5 — soundness invariants (hard gates, any violation halts the plan).**
  (a) `benchmark --suite quick --json --first-outcome` bit-identical before
  vs. after the code change at default settings; (b) stress salt 0
  reproduces 249,480,478 child evals exactly; (c) **zero outcome flips**
  across salts on any case: if a case's decisive outcome differs between
  two salts, that is a soundness defect (TT/GHI interaction), not noise —
  halt, diagnose, do not proceed to analysis.
- **D6 — analysis.** Per case: per-salt child_evals (or censored flag),
  min/median/max over uncensored draws, censored count, max/min spread.
  Case classified **salt-sensitive** if (max/min ≥ 1.5× among uncensored)
  or (any censored draw while another salt solves). Pilot outputs feed
  `gate_methodology.md`; they do not by themselves re-open any closure.

## Method

- **Phase 0 — salt knob.** Implement D1/D2: `salt` field + setter on the TT
  table, `index()` remap, `Search::set_salt` (naming per code review),
  CLI flag with the existing unknown-option conventions. Unit tests:
  s = 0 identity; s > 0 deterministic and remaps at least some keys;
  bucket-count invariance. Integration test: m22_white with `--salt 3`
  solves `win` (fast, ~20 M evals). Run the D5 (a)/(b) identity gates.
- **Phase 1 — pilot runs.** Driver script (Python, committed under
  `measurements/plan11/`) runs the D4 grid, parses the solver's child-eval
  output, and writes parsed results (`state/*.json`) — transcripts stay
  uncommitted per the AGENTS.md measurement layout.
- **Phase 2 — methodology + report.** Write `gate_methodology.md` (gate
  rule draft for #18–#20: paired per-case comparison, censoring handling,
  sensitivity classification, minimum corpus; thresholds calibrated on the
  pilot, to be pinned in plan12). Write `report11.md` (final task).

## Gates

- **GO** → plan12: full ≥20-case corpus rollout, then the first #19 re-score
  arms under the pinned gate rule.
- **PIVOT** → if salt-remap alone leaves within-salt rerun variance
  comparable to between-salt variance on any pilot case (checked cheaply:
  one duplicate run on the worst case), the noise channel is insufficient —
  record and extend the channel set (tie-breaking jitter, history aging)
  before any rollout.
- **HALT** → any D5 invariant violated (outcome flip, identity-gate
  failure); becomes a soundness investigation, not a measurement result.

## Out of scope

- Any initialization change (#18), any re-score run (#19), restarts (#20).
- `benchmark`/optimizer-interface changes; proof-tree layer; TT snapshot
  format.
- Other noise channels (recorded as caveats in `gate_methodology.md`).

## Final task

Write `report11.md` in this directory: deliverables, gate result, problems
encountered, missing tests, next steps (plan12 kickoff prompt).
