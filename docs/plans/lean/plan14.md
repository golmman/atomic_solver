# Lean Plan 14 — #21 arm A: same-MB 4-slot TT bucket (T1 follow-up)

Initiative: `lean`. Executes backlog item **#21**'s pre-registered follow-up:
`report13.md` measured the TT retention surface (plan13, temporary hook,
reverted) and T1 fired on both disjuncts — stress-class re-probe loss
15.5–20.1% of probe hits and victim-live ~70% of stores on m19/m20 (both
salts), while every solvable cell sits at ≤ 0.95% / ≤ 2.3%. plan13
pre-registered the response order: **arm A** (same-MB 4-slot bucket) first,
arm B (layered TT) only if arm A's mechanism analysis shows collision
tolerance is *not* the binding constraint. This plan implements arm A. It is
behavior-changing by design; the bit-identity drift protocol is N/A and
validation runs per the pinned gate v1.0 (plan19 template). Self-contained.

## Context the executor must know

1. **plan13 baseline (the numbers to beat).** At 128 MB, first-outcome,
   60 s timeout, x86_64 host: m19_fo / m20_fo re-probe/hits = 17.40% /
   15.52% (salt 0) and 15.52% / 20.06% (salt 3); victim-live/stores ~70%
   on all four stress cells (artifactual at 99.8% occupancy — *every*
   store that doesn't update must evict and every victim is live, so the
   policy-relevant signal is the re-probe loss); re-probed keys carry
   above-mean work (72–87 vs live-population mean 22–39). Every counter
   is preserved in `measurements/plan13/state/results.json`.
2. **Current layout** (`src/search/tt/table.rs`): `Vec<[TtEntry; 2]>`,
   56 B entries, 112 B buckets. `with_mb`: entry count =
   `(MB·2²⁰/56).next_power_of_two().max(32)`, buckets = entries/2 → at
   128 MB: 2²² entries, 2²¹ buckets, 224 MiB allocated (the 1.75× power-of-
   two rounding is backlog #20, untouched here). Eviction score
   `(live, solved, work, generation)`, victim = argmin, tie evicts slot 1
   (`usize::from(score0 >= score1)`). `new_generation()` is never called
   by the search → `generation` is inert in production.
3. **What arm A changes and what it does not.** Bucket width 2 → 4 at an
   *identical entry count and identical RAM*. Mechanism: `insert_new`'s
   work comparison only sees the keys sharing a bucket (plan13's
   structural finding — a high-work entry whose ≤ 2 bucket-mates score
   higher dies regardless of its global rank); four slots widen that
   comparison window and widen the probe scan. It adds **no capacity** —
   plan13 pre-registered that at saturation the binding constraint may be
   capacity, not collision tolerance; the mechanism fork (task 6) decides
   between these readings with numbers.
4. **Behavior intentionally changes.** The drift protocol (quick-suite
   bit-identity, m22 stdout byte-identity, lean golden) is N/A. Validation
   = pinned gate v1.0 (`docs/plans/research/gate_methodology.md`; its
   `measurements/plan12/driver.py` reference resolves to
   `docs/plans/research/measurements/plan12/driver.py`). Two consequences
   pre-negotiated here:
   - The m22 default-mode trajectory golden
     (`tests/test_trajectory_golden.rs` +
     `tests/fixtures/m22_default_stdout_golden.txt`) is **regenerated**
     from the new build, with the test header updated (it already
     documents a regoldening history).
   - Gate soundness invariant 1 (salt-0 byte-identity to shipped) is
     renegotiated to **within-build determinism**: salt-0 reruns of the
     new build must reproduce stdout and child_evals bit-for-bit. Invariant
     2 (zero decisive-outcome conflicts across salts) stays absolute — a
     conflict is a soundness defect, halt and diagnose.
5. **Baseline reuse is legitimate.** Shipped HEAD is byte-identical to the
   plan12 code base (plan13 was measurement-only and reverted), so the
   plan12 rollout (22 cases × 6 salts, 132 runs,
   `docs/plans/research/measurements/plan12/state/runs.json`) is the valid
   paired-by-salt baseline, same x86_64 host (AMD 5950X container) as every
   measurement since plan11.
6. **Snapshot/proof-tree compatibility.** `write_tt_snapshot` is key-keyed
   and count-driven; only the *order* of streamed records changes with
   bucket topology. `entries()` iteration order changes (4 slots per
   bucket); all consumers (`tt_snapshot`, `reconstruct`, `proofdb` tooling)
   must stay green via `make test`.
7. **Hot-path risk.** The probe scan widens 2 → 4 key comparisons and the
   bucket footprint doubles (112 → 224 B, up to 5 cache lines) — on
   exactly the memory-bound path plan11/plan12 optimized (prefetch
   pre-pass). The prefetch currently issues 2 hints (first + last byte);
   coverage width at 224 B was never measured (plan11 phase-0 measured
   line-*alignment*, not hint count). Task 3 measures it.

## Goal and hypothesis

H: giving `insert_new` a 4-candidate victim set at unchanged capacity
materially reduces the stress-class re-probe loss — the incumbent
`(live, solved, work, generation)` score fails only because its comparison
window is one bucket (2 keys) wide — without regressing the solvable class
beyond the gate. Falsification signal: stress re-probe loss ≈ unchanged →
collision tolerance is not binding; capacity (arm B family) is the real
lever.

## Design decisions (pinned)

- `Vec<[TtEntry; 4]>`. `with_mb`: entry-count formula unchanged, buckets =
  entries/4 (min 1) → 128 MB yields 2²² entries / 2²⁰ buckets / the same
  224 MiB allocation. "Same-MB" holds exactly: `--tt-size` still buys the
  same number of entries; only the topology changes.
- `with_capacity(n)` (test helper) = n buckets of 4 entries; forced
  collision/eviction tests now need 5 distinct keys per bucket.
- `probe`: slot-order linear scan, early exit on `valid && key == key &&
  generation == current` (deterministic order; full-key verification
  unchanged — no cross-key hits, salted or not).
- `insert_new`: empty/stale slots first (lowest index); else victim =
  argmin of the **unchanged** `(live, solved, work, generation)` score,
  ties resolved to the **highest** index (generalizes the shipped 2-slot
  tie behavior: equal scores evict slot 1). No score-tuning in this plan —
  one lever.
- Entry format (56 B), salt formula (rot-free mix `>> 17` then mask —
  mask width changes only), snapshot format: unchanged.
- Prefetch coverage: measured decision between (a) shipped 2-hint
  (first + last byte) and (b) 4-hint (offsets 0/64/128/192). Task 3 pins
  the winner on wall time and deletes the loser variant.

## Phase 0 — implementation + micro-measurement (tasks 1–3)

## Phase 1 — gate v1.0 rollout (task 4)

- Corpus: the frozen plan12 corpus (22 cases: m20–m23 family +
  dec01–dec18; list and expected outcomes in the plan12 driver).
- Salt set {0, 1, 2, 3, 4, 5}; salt 0 always runs (within-build
  determinism anchor).
- Budgets: stress 2.5 B, m20_white 2 B, all others 1 B child evals;
  wall `--timeout 600` strictly a safety cap. Censoring rules per the
  pinned methodology (censored evals never enter ratios; > ⌈6/2⌉ censored
  draws → case under-budgeted, move to the unresolved list).
- Comparison: per-salt paired ratios `candidate_s / baseline_s` against
  `docs/plans/research/measurements/plan12/state/runs.json`; per-case
  verdicts (censored-win/loss, win, regression, unclear) per the pinned
  rule, with basin counts; diversity clause as pinned.
- Lever verdict: **adopt** (≥ 1 censored-win or win on a hard-class case,
  no regression/censored-loss anywhere) / **reject** / **defer** (extend
  salts on affected cases before re-judging).
- Secondary read (reported, not a gate): `benchmark --suite quick --json
  --first-outcome` totals before (shipped) vs after (arm A) — a coarse
  solvable-class regression read outside the corpus.
- Raw transcripts under `docs/plans/lean/measurements/plan14/raw/`
  (gitignored); driver + parser + `state/*.json` + `README.md` provenance
  + `env.json` committed per the AGENTS.md measurement layout.

## Phase 2 — mechanism measurement (tasks 5–6)

### Temporary probe hook (reconstructed, reverted after measuring)

Re-apply the plan13 hook (shape fully specified in `report13.md` "Method":
env-gated `ATOMIC_TTPROBE=1`, relaxed-atomic counters, 2²⁰-slot evicted-key
shadow, temporary `examples/tt_retention.rs` driver), on top of the arm-A
build. Run the plan13-comparable cells: m19_fo / m20_fo × salts 0 / 3 at
128 MB / 60 s, plus m22_fo as the solvable control. Parse per-cell counter
JSON to `state/probe_results.json`. Then revert hook + driver to
byte-identity (`git diff` over `src/` and `examples/` empty) and confirm
the gate still passes. If the gate (task 4) rejected arm A, still run this
phase — the numbers inform the arm-B decision — but the report must state
they describe the rejected arm, not shipped behavior.

### Pre-registered mechanism fork (task 6)

Compare per-cell `reprobe_after_eviction / probe_hits` against plan13's
`state/results.json`:

- **Collision-tolerance-was-binding:** re-probe ratio drops to ≤ 50% of
  its plan13 value on both m19 and m20 (each salt) → arm A is the lever.
  If a material residual remains (≥ 2% of hits), arm B stays open with
  these numbers as its sizing; otherwise recommend closing #21.
- **Capacity-binding:** re-probe ratio persists at ≥ 80% of its plan13
  value on both stress fixtures (both salts) → per plan13's pre-registration,
  collision tolerance is not the constraint; write the arm-B (small
  hot-entry tier) vs capacity-family decision as a next-plan sketch from
  the re-probe mass + work distribution, do **not** implement it here.
- **Gray zone** (between the bands): add two more salts on the stress
  cells before deciding (the fractions are per-run deterministic; salt
  spread is the noise read, D7-style).

## Non-goals

- No arm B implementation (layered TT), no entry-format or snapshot-format
  change, no eviction-score retuning, no capacity default change (#16
  territory) and no #20 allocation fix.
- No change to the search CLI's output contracts beyond the regenerated
  golden; no proof-tree layer or optimizer-contract change.
- No edit to `docs/notes.md` (the layered-TT idea is answered by
  measurement, per plan13's non-goals).
- The probe hook is never shipped: task 5 restores `src/` byte-identically.

## Tasks

1. Preconditions: `report13.md` exists; `make test` green at HEAD; record
   the plan13 artifact pointers (baseline for tasks 4–6).
2. Implement the 4-slot bucket in `src/search/tt/table.rs` per the pinned
   design decisions; update `src/search/tt/tests.rs` (forced eviction with
   5 keys in one bucket, tie-break victim = highest index, stale-slot
   preference, `entries()` bucket order, `with_mb` math: 128 MB → 2²⁰
   buckets, salt tests unchanged in spirit). `cargo fmt --check`,
   `cargo clippy --all-targets` clean.
3. Regenerate the m22 default-mode golden from the new build (updated
   header note); prefetch variant micro-measurement: m22 default wall ×3
   and one stress cell (m20_fo salt 0, 60 s) under 2-hint vs 4-hint; pin
   the winner, delete the loser variant. `make test` green.
4. Gate v1.0 rollout per Phase 1 (driver copied from the plan12 template
   into `measurements/plan14/`); verdicts + soundness checks; quick-suite
   secondary read.
5. Mechanism measurement per Phase 2 (temporary hook + driver, plan13-
   comparable cells, revert to byte-identity, gate green again).
6. Mechanism fork decision per the pre-registered bands; if capacity-
   binding, draft the arm-B next-plan sketch (from the measured re-probe
   mass and work distribution) as part of the report's next steps.
7. Update backlog #21 (arm A verdict + numbers + plan reference) and the
   `lean` row in `docs/plans/README.md`.
8. Write `report14.md` — final task: method, raw-vs-parsed artifact
   pointers, gate verdict with per-case numbers, the mechanism reading
   (fork outcome), golden-regeneration note, host caveat, additional tools
   used, problems encountered, next steps (arm-B sketch if forked).
