# Plan 9 — Portfolio ship shape: `examples/portfolio` driver (backlog item #11)

**Status: READY (owner decision 2026-10-12 — resolves report8
§Ship-shape to option 2).** Executes the `parallel` backlog item **#11**.
Final task: `report9.md`. This is an implementation plan, not a
measurement campaign: no gate measurement is pre-registered here — the
plan8 campaign record stands as the lever's evidence, and the driver is
validated by fidelity smoke tests that re-derive two of its deterministic
anchors exactly.

## Decision record

- **Option 3 (`--race K` product flag) — rejected for now.** The salt
  portfolio is a PoC-scale lever, not a production feature: the racer set
  {0, 2, 3, 5} is a pre-registered constant tuned on the six gate cases
  (per-case basin structure is unknown for unseen positions — rem12 needed
  a stage-2 pre-computation; dec13 is basin-poor at 2); cost scales
  linearly in both scarce resources (K × CPU, RAM = K × TT); there is no
  work sharing. A product surface would freeze exactly this shape into the
  CLI just before backlog #6 (cooperative portfolio) and #7
  (any-worker-can-finish SPDFPN) — the mechanisms that own actual scaling —
  and would then need redesign. Option 3 stays behind the door: revisit
  only if #6/#7 produce a genuinely scalable mechanism.
- **Option 1 (documented operator pattern) — insufficient alone.** The
  measured win would remain behind manual steps (launch bookkeeping,
  kill/grace logic, evals parsing) that a single operator re-executes by
  hand each time.
- **Option 2 chosen.** Captures the entire plan8 win at S effort with zero
  product risk, gives the report8 findings a durable home (in particular
  the **stop-when-proven kill policy** that reclaims the grace-burned
  core-seconds — report8's stress cost finding: 543 of 606 core-s), and is
  the natural harness skeleton for #6/#7.
- **Rust example, not a Python driver.** Consistency (AGENTS.md: example
  binaries under `examples/`; every other example is buildable Rust),
  tier-testability (`make test` can exercise it), compile-checked against
  the current CLI flags. The Python campaign harnesses
  (`measurements/plan8/`) are measurement code, not shipped tooling; their
  pattern is not imported across that lifecycle boundary.

## Scope

**In scope:**

- `examples/portfolio.rs` (new) — the only code change.
- Unit tests inside the example; one fast-tier integration test
  `tests/portfolio_smoke.rs`.
- AGENTS.md examples-list entry.
- One operator-validation race run (m20_white) + one cheap dec10 race,
  parsed results under `measurements/plan9/`.

**Out of scope:**

- Any `src/` change: the plan's product diff must be **zero lines**
  (asserted at stage 0, re-asserted in the report).
- `--race` / any product CLI surface; spec changes (the optimizer
  contract and proof-store spec are untouched; the driver is part of no
  spec'd contract).
- The #6/#7 mechanisms themselves — the driver only prepares their
  harness (see Code hygiene).

## Driver contract

**Invocation:** `cargo run --release --example portfolio -- --fen "<FEN>"
[options]`.

**Options:**

| option | default | meaning |
| --- | --- | --- |
| `--fen <FEN>` | required | the raced position (same root for all racers) |
| `--salts <LIST>` | `0,2,3,5` | comma-separated racer salt set; the plan8 pre-registered set as default |
| `--timeout <S>` | `300` | per-race wall cap; a racer exceeding it is killed and recorded `wall-killed` |
| `--budget <EVALS>` | none | pass-through of the solver's child-eval cap |
| `--tt-size <N>` | solver default | pass-through |
| `--solver <PATH>` | auto-discovery | solver binary path override (used by the smoke test) |

**Fixed, non-negotiable racer flags:** every racer is the unmodified
solver invoked with `--first-outcome --outcome-only` (the measured plan8
configuration) plus its `--salt`. This is not a flag: deviating from the
measured configuration silently invalidates the anchors the driver is
validated against.

**Solver discovery:** try `current_exe().parent()/atomic_solver` and, for
the standard cargo layout (`target/release/examples/portfolio`),
`current_exe().parent().parent()/atomic_solver`; explicit error with a
"run `cargo build --release`" hint if neither exists. `--solver` overrides.

**Kill policy (the report8 finding, in code):**

- All racers launch concurrently; their stderr is consumed for
  `outcome:` lines, stdout for `pv:`/`pv_status:`/`budget exhausted`
  lines, per plan8's parsing.
- At the **first decisive** `outcome: win|loss` observed from any racer:
  that racer is the winner and runs to natural completion (its `evals:`
  line and PV lines are the record); **all remaining racers are killed
  immediately** (stop-when-proven — no grace period; report8's smarter
  kill policy).
- A racer that prints the exact stdout line `budget exhausted` is
  recorded `censored=budget` — **never** as a proven draw (plan8
  censoring discipline).
- A racer killed by the race wall cap is recorded `wall-killed`; its
  evals are excluded from the summary decision (plan8 discipline).
- A racer exiting without a decisive outcome and without censoring is
  recorded as its actual (proven-draw) outcome.
- Zero-decisive-outcome conflicts across racers are inherited from the
  solver's soundness; the driver reports, never arbitrates: on multiple
  decisive racers (possible only in wall), the **first** finisher wins
  and later decisive racers are recorded as also-decisive with their
  own anchors.

**Output grammar (stdout, deterministic ordering):**

- one line per racer, sorted by salt:
  `racer: salt=<s> wall=<ms> evals=<n|-> outcome=<win|loss|draw|killed|error> censored=<budget|no>`
- then, on a decisive answer: `portfolio: outcome=<win|loss> salt=<s>
  evals=<n> wall=<ms> racers=<k>` followed by the winner's `pv:` and
  `pv_status:` lines echoed verbatim; exit code 0.
- on no decisive answer: `portfolio: outcome=none racers=<k>`; exit code
  1. (A proven draw across all racers is a legitimate `none` with
  per-racer `draw censored=no` lines.)

**RAM contract, documented in the module header:** the driver holds no
search state; RAM = K × TT of the spawned racers (4 × 128 MB = 512 MB at
defaults), same restatement plan8 documented in `env.json`.

**Soundness note (module header):** each racer *is* the unmodified
solver — per-process soundness is inherited; the only nondeterminism is
*which* valid proof wins (the accepted plan8 premise). The driver itself
is timing-dependent by design (take-first-decisive) and must never be
used where reproducible single-run semantics are required (benchmark /
optimizer runs are unaffected: they do not use the driver).

**Code hygiene:** keep the race logic in separable functions —
racer-set/launch, stderr/stdout consumption, first-decisive selection,
accounting — so #6/#7 can extend the same skeleton (shared-facts table,
any-worker-can-finish) without a rewrite. Target ≤ 10 KB per the file
convention; unit tests may live in a `#[cfg(test)]` module in the same
file.

## Testing

**Unit tests (in the example):**

- output-line parsing: `outcome:`, `evals:`, `budget exhausted`, `pv:`
  lines → racer records; malformed/missing lines → `error` record, not a
  panic.
- salt-list parsing (defaults, malformed list rejected).
- kill-policy state machine driven by simulated racer events:
  first-decisive kills the others; budget-censored never reported as
  proven draw; wall-killed evals excluded.

**Integration smoke test (`tests/portfolio_smoke.rs`, fast tier):**

- *dec10 fidelity anchor*: 4-racer race (default salts, default TT) on
  the plan8 campaign FEN
  `3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41` (plan12
  `CASES` entry, verbatim). Asserts: exit 0, summary outcome `win`,
  winner salt `2`, winner evals exactly **2,918,636** (the plan8
  campaign anchor). Racers are deterministic and concurrency-independent
  (plan8 H4b, 63/63), so the anchor must reproduce exactly — this is the
  driver's fidelity gate, not a tolerance check. Wall ~1–2 s; peak RAM
  512 MB (documented in the test header).
- *censoring path*: same FEN, `--budget 1000`: all racers hit the cap;
  asserts exit 1, summary outcome `none`, every racer recorded
  `outcome=draw censored=budget` (no proven-draw misreport), wall ≪ 1 s.
- The test locates the driver binary via its own `current_exe()` path
  (`target/<profile>/deps/…` → parent → `examples/portfolio`) and the
  solver via `env!("CARGO_BIN_EXE_atomic_solver")` passed as `--solver`;
  fail loudly (no skip) if either is missing — `cargo test` builds
  examples, so absence is a gate defect.

Gate: `cargo fmt`, `cargo clippy`, `make test` green (the two smoke
tests add ≲ 5 s).

## Stages

1. **Stage 0 — precondition.** Clean tree, release build, `git diff
   --exit-code src/` recorded (must remain empty through the plan).
2. **Stage 1 — implement.** `examples/portfolio.rs` + unit tests, per
   the driver contract.
3. **Stage 2 — gate.** `tests/portfolio_smoke.rs`; `cargo fmt` /
   `clippy` / `make test`.
4. **Stage 3 — operator validation (cheap, non-gated).** One 4-racer
   m20_white race (`4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R5RK w - - 4
   20`, default salts, `--budget 2000000000 --timeout 500`): expected —
   decisive win from the min-evals racer (salt 5, plan8 anchor
   791,401,553 evals; wall ~96 s + contention). Parsed result +
   provenance (`README.md`, `env.json`) under
   `measurements/plan9/state/`; raw transcripts never committed. A
   mismatch vs the anchor is a driver defect → fix before the report
   (the anchor itself is fixed plan8 data, not re-measured).
5. **Stage 4 — docs.** AGENTS.md examples entry (one line, per the
   existing list); initiative item #11 status → closed; the
   `docs/plans/README.md` `parallel` row ship-shape note updated.
6. **Final task — `report9.md`**: what landed, smoke/validation results
   vs the plan8 anchors, deviations, follow-ups (incl. the #6/#7
   harness-reuse note and the explicitly open option-3 door).

## Exit criteria

- `make test` green including the two smoke tests; `src/` diff zero;
  stage-3 validation matches the plan8 anchors; report + backlog/index
  updates done.

## Deviation policy

No mechanism change is possible without voiding the plan8 anchors the
driver is validated against: any change to the fixed racer flags, the
kill policy, or the output grammar discovered mid-work goes into
`report9.md` as a finding, and the follow-up is a new plan. Scope
additions (e.g. extra pass-through options) are allowed if they are
purely additive and do not touch the fixed configuration.
