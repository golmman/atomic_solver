# report11 — Item #17: salt-seeded statistical gate (pilot)

Executes [`plan11.md`](plan11.md). All three phases completed; every pre-registered
soundness gate passed; gate verdict **GO → plan12**.

## Deliverables

1. **Salt knob as a product surface** (D1/D2):
   - `TranspositionTable::set_salt` / `salt()` (`src/search/tt/table.rs`):
     `salt = 0` keeps the shipped `key & mask` path bit-identical (the mixing
     formula is explicitly gated off at 0, per D1); `salt = s > 0` maps the
     bucket index to `((key ^ s) · 0x9E3779B97F4A7C15 >> 17) & mask`. Set once
     before any probe/store; full-key verification unchanged.
   - `Search::set_salt` (`src/search/dfpn/mod.rs`) forwarding to the table.
   - CLI: `--salt <u64>` (default 0) in `src/cli.rs` + `src/main.rs` help/docs.
   - Tests: 4 salt unit tests in `src/search/tt/tests.rs` (s=0 identity,
     determinism + ≥90% remap rate, bucket-count invariance over salts
     {1,2,3,4,u64::MAX} × bucket counts {1,2,4,1024}, full-key verification
     under salted collisions); 2 CLI integration tests in
     `tests/test_cli.rs` (m22_white `--salt 3` still solves win; tiny
     `--budget` deterministically exhausts → draw + `budget exhausted`).
2. **`--budget <EVALS>` CLI option** — scope addition, see "Findings" below.
   It exposes the existing `Search::set_child_eval_budget` so the D4 censoring
   protocol is enforceable on the product CLI (the plan's D4 assumed per-run
   child-eval caps; before this plan only the throwaway probe binary had one).
3. **stderr `evals: <n>` line** (`src/main.rs`, both modes) — the machine-readable
   work metric for gate drivers; kept off stdout so the outcome-only stdout
   contract stays byte-exact (the m22 golden is untouched).
4. **Pilot distribution** (D3–D4, D6): 6 cases × 5 salts + 1 duplicate,
   [`measurements/plan11/`](measurements/plan11/) (`driver.py`, `parse.py`,
   `state/runs.json`, `state/summary.json`, `state/bench_identity_{pre,post}.json`,
   `env.json`). All 31 runs completed in ~13 min wall (3-way parallel), well
   inside the ≤ 2 h envelope.
5. **`gate_methodology.md`** — the pre-registration contract, paired per-salt
   comparison rule, censoring rules, and soundness invariants for #18–#20;
   thresholds drafted from the pilot, to be pinned in plan12.

## Gate results (D5)

| gate | result |
| --- | --- |
| (a) benchmark quick `--json --first-outcome` trajectory identity | **PASS** — per-case outcome/nodes/child_evals/pv byte-identical pre vs. post (wall-clock timing fields excluded: they are not trajectories; pre-change repeat confirmed the stripped JSON is deterministic) |
| (b) stress salt-0 = 249,480,478 | **PASS** — exact; additionally m22 14,156,269 / dec13 3,822,602 / dec10 4,262,128 reproduce the reexamination probe baselines exactly, proving the product `--salt` and the throwaway `PROBE_SALT` are the same mechanism |
| (c) zero outcome flips | **PASS** — all decisive outcomes are `win` on every case/salt; budget-censored runs report the documented no-result draw and are right-censored, not flips |
| PIVOT check | **not triggered** — duplicate stress salt-1 run is bit-identical (758,176,477 evals, same stdout): within-salt variance is exactly zero, the salt channel is a clean deterministic noise realization |

`make test` passes (250 lib + all fast integration tests); `cargo fmt` /
`cargo clippy --release --all-targets` clean.

## Pilot results (first-outcome child evals, 128 MB TT)

| case | salt 0 | salts 1–4 | spread (uncensored) | sensitive (D6) |
| --- | --- | --- | --- | --- |
| stress | 249,480,478 | 758,176,477; censored ×3 @ 2.5 B | 3.04× | **yes** |
| m20_white | censored @ 1 B | censored ×2; 861.8 M, 912.1 M | — | **yes** (mixed) |
| m22_white | 14,156,269 | 15.8–23.7 M | 1.67× | **yes** |
| m23_white | 9,440,650 | 9.7–12.1 M | 1.28× | no |
| dec13 | 3,822,602 | 5.03 M ×4 | 1.32× | no |
| dec10 | 4,262,128 | 2.9–4.8 M | 1.65× | **yes** (barely) |

New evidence beyond the reexamination:

- **m20_white** (first time measured) is the hardest shape in the family: the
  shipped salt-0 trajectory does *not* solve it at the 1 B budget while salts
  3/4 solve it at ~860–912 M. Another closed-record premise measured on one
  favourable draw.
- dec10 is borderline (1.65×, driven by salt 2 = 2.92 M vs salt 3 = 4.80 M);
  the salt-1/salt-4 draws agree within 3 evals, hinting at structure in the
  noise worth one look during plan12 calibration.
- Salt 0 is the minimum-of-five on stress, m22 and dec13 (as in the
  reexamination), corroborating the "fitted to one realization" reading.

## Problems encountered

- **`parse.py` flip detection, first draft, was wrong**: it flagged
  `draw` outcomes of budget-censored runs as outcome flips. The budget
  contract (`Search::set_child_eval_budget` docs) makes that `draw` a no-result
  sentinel; D5(c) is about conflicting *decisive* outcomes. Fixed to compare
  decisive outcomes only; no real flips exist. The distinction is now stated
  in `gate_methodology.md` (invariant 3) so plan12 inherits it.
- **D5(a) "bit-identical" needed interpretation**: `benchmark --json` embeds
  wall-clock timing fields that are not reproducible by design. The gate
  compares all trajectory fields (outcome/nodes/child_evals/pv) and excludes
  timings; a pre-change repeat established the stripped JSON is itself
  deterministic. Recorded in `measurements/plan11/env.json`.
- **D4 protocol vs. product surface gap**: the plan's per-run child-eval caps
  could not be enforced without a CLI budget option (D2 listed only `--salt`).
  Added `--budget` rather than approximating the protocol with `--timeout`
  alone, which would have made "censored" wall-dependent and broken the
  determinism the gate relies on. This is the only deviation from D2's
  enumeration; `benchmark`'s `--json` contract is untouched.

## Missing tests / caveats

- No test pins that salted and unsalted *search trajectories* diverge on a
  real position (they trivially do — the pilot shows it — but a cheap
  regression test on a small position asserting salt 1 ≠ salt 0 eval counts
  would pin the remap end-to-end; the unit tests pin `index()` directly).
- The salt channel's independence from other noise sources (tie-breaking,
  history aging) is unmeasured, per the plan's out-of-scope list; recorded as
  a caveat in `gate_methodology.md`.
- `make test-full` not run: the change is behind `salt = 0` (bit-identical by
  gate (a)) and the fast tier covers the new code. Recommended before any
  release that ships `--salt`, per the standing convention.

## Next steps

GO → **plan12**: pin the gate thresholds from the paired-ratio distribution of
this pilot, roll out the ≥ 20-case corpus under the pre-registration contract,
then open the first #19 re-score arms under the pinned rule.

SESSION COMPLETE
- plan11 executed: salt/`--budget`/`evals:` product surface + tests (make test green, D5 a/b/c + PIVOT all pass), pilot 31 runs measured (measurements/plan11/), gate_methodology.md + report11.md written, docs/plans/README.md research row updated
Follow-up options:
  1. "Execute docs/plans/research/plan12.md" — pin the gate thresholds from the pilot's paired-ratio distribution and run the ≥ 20-case baseline rollout (prerequisite for every #18–#20 measurement).
  2. Alternative: first diagnose the dec10 salt-pair structure (salt 1 ≈ salt 4 within 3 evals) as a small read-only probe inside plan12's calibration, to decide whether the salt channel needs a second noise channel after all.
