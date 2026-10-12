# Lean Plan 13 — #21 TT retention probe: measure eviction churn before any retention machinery

Initiative: `lean`. Opens backlog item **#21** (TT eviction quality).
Trigger: the owner's layered-TT idea (`docs/notes.md` lines 907–911:
per-entry hit counter, reset on eviction, promote to a second TT at a
threshold) — untried, with no plan record. This plan does **not** build
that machinery; it first measures whether the mass it would recover
exists. Self-contained.

## Context the executor must know

1. **Eviction surface is already gate-grade territory.** Research plan19
   (`docs/plans/research/report19.md`) re-scored a replacement-priority
   variant (eviction V2, `remaining_depth` tiebreak) under pinned gate
   v1.0 and **rejected** it. The incumbent `(live, solved, work,
   generation)` score survived its only gate-grade challenge. Note also:
   the `generation` term is inert within a run (`new_generation()` is
   never called by the search), so among two live unsolved victims with
   equal `work` the shipped policy falls through to slot order.
2. **The proposed mechanism's signal already has a proxy in the score**:
   `work` is stored as `max(work)` and participates in eviction — it is
   a visit-pressure counter. A hit counter + promotion tier is a second
   way to measure the same quantity, plus a place to put the entry.
3. **The hot path is memory-bound.** Plan11/plan12 showed the serial
   child TT-bucket DRAM miss was ≈45% of wall before prefetch; probe
   cost, not eval count, dominates wall at large TT sizes
   (`research_tt_capacity.md`). A layered TT turns every miss into two
   lookups and makes `probe` mutate cache lines (hit-counter writes) —
   cost added exactly where the profile says we cannot afford it. Any
   retention mechanism must be justified by measured recovered mass, not
   plausibility.
4. **Capacity, not policy, is the measured dominant lever** on deep
   searches (64→128 MB = 2.6× less work on m22). The cheap untested
   variant in the same family is the **same-MB 4-slot bucket** (more
   collision tolerance, same total entries, wider probe scan — flagged
   "untested" in `research_tt_capacity.md`).
5. **Precedents for the measurement shape**: the capacity probe
   (temporary instrumentation, removed after measuring) and the env-hook
   pattern of research plan17/20/21 (`ATOMIC_*` env gates, hook reverted
   to byte-identity afterwards).

## Goal and hypothesis

H: On deep (capacity-starved) searches, a material fraction of probes
hit keys that were previously evicted from the live table, and/or a
material fraction of stores overwrite live (current-generation) entries.
If the re-probe loss is negligible, **no** retention mechanism — layered
TT, 4-slot bucket, work re-weighting — can pay, and #21 closes no-go.

## Phase 0 — instrumented retention probe (this session)

### Hook design (temporary, env-gated, reverted after measurement)

Env gate `ATOMIC_TTPROBE=1`; when unset, behavior and code path are
unchanged (bool read once at construction). All changes confined to
`src/search/tt/table.rs` plus a temporary driver example.

- Counters (u64): `probe_hits`, `probe_misses`, `store_total`,
  `store_overwrote_live` (victim `valid && generation == current`),
  victim split (`victim_live_solved`, `victim_live_unsolved`),
  `reprobe_after_eviction`, plus `evicted_live_work_sum` /
  `evicted_live_count` and `live_work_sum` / `live_work_count`
  (sampled on store) to compare victim work against the mean.
- **Shadow of evicted keys**: direct-mapped `Vec<u64>` of 2²⁰ slots
  (0 = empty; store `key.wrapping_add(1)`, so only `key == u64::MAX`
  collides with the sentinel — note it). Slot index: a fixed mix of the
  key (e.g. the salt formula's rotation; deterministic, salt-aware).
  On evicting a live entry → write its key into the shadow. On a probe
  hit whose shadow slot contains that key → `reprobe_after_eviction += 1`.
- Known biases (document in the report): shadow collisions cause false
  negatives (losses are *undercounted* — bias against the retention
  lever, so a kill reading is conservative and a fire reading is
  trustworthy); a key evicted and re-stored before re-probe still counts
  (measures churn *exposure*, which is the lever's actual opportunity).
- Output: one JSON line to stderr at driver exit when the env is set
  (counters + `stats()` occupancy). The production CLI prints nothing
  new (the hook lives in the TT, driven only via the temporary example).

### Driver

Temporary `examples/tt_retention.rs` (deleted in task 5): FEN +
`--tt-size`-equivalent args, runs `Search::solve` (first-outcome
semantics), dumps the counter JSON. Patterns from the capacity probe.

### Run matrix

Cases from `tests/fixtures/move_order_positions.txt`. Hard class:
`m22_white` (FO **and** default mode), `m20_white` FO, `dec10`,
`dec13`, one stress-class case (the fixture marked slow/hard — pick the
`stress` tag). Shallow control: 4 quick-suite-representative cases.
128 MB default size. Salt 0 plus one salted draw (`--salt 3`, matching
the CLI's existing salt test convention) per hard case — plan19 showed
collision density is the substrate of this surface; salt 0 alone cannot
clear even a Phase-0 existence question. One deterministic run per cell;
wall/child_evals recorded per run (trajectory-sensitive regime — single
draws are directional evidence only, hence the generous gate margins).

### Decision gates (pre-registered)

- **K1 (kill → close #21 no-go):** `reprobe_after_eviction / probe_hits`
  < 1% on **every** hard-class case (both salts) **and**
  `store_overwrote_live / store_total` < 5% everywhere. Close #21 with
  the numbers; the notes.md layered-TT idea is answered by measurement
  (no plan response needed; do not edit `docs/notes.md`).
- **T1 (fire → follow-up plan14):** re-probe loss ≥ 2% of probes on ≥ 2
  hard-class cases, or victim-live ≥ 20% of stores on ≥ 2 hard cases.
  plan14 then executes, in order: **arm A** same-MB 4-slot bucket
  (cheapest retention upgrade; bit-identity expected to break — drift
  protocol N/A, validated per gate v1.0, plan19 template); **arm B**
  (small hot-entry tier — the layered TT) only if arm A's mechanism
  analysis shows collision tolerance is *not* the binding constraint.
- **Gray zone** (neither gate): re-run the ambiguous cases under two
  more salts and decide on the 4-draw spread (D7-style reading, quiet
  band [0.76, 1.32] applies to *ratios of ratios*, not to these raw
  fractions — the fractions themselves are per-run deterministic
  measurements; spread across salts is the noise read here).

## Non-goals

- No retention mechanism is implemented in this plan (including the
  layered TT). Phase 1 is a separate plan, only on T1.
- No change to `probe`/`store` semantics, entry format, snapshot
  pipeline, or the optimizer contract.
- No edit to `docs/notes.md`.
- The hook is never shipped: task 5 restores `src/` byte-identically.

## Tasks

1. Preconditions: `report12.md` exists; `make test` green at HEAD.
2. Implement the hook in `src/search/tt/table.rs` (env-gated; ~60–80
   lines; unit-test the shadow accounting with `with_capacity`-driven
   forced collisions — extend `src/search/tt/tests.rs`).
3. Add `examples/tt_retention.rs`; smoke-run on `m22_white` FO.
4. Run the matrix; raw transcripts to `measurements/plan13/` (uncommitted
   per the layout rules), parsed per-cell JSON + `README.md` provenance
   table committed.
5. Evaluate K1/T1/gray-zone; revert hook and driver (`git diff` over
   `src/` and `examples/` empty; `make test` green again).
6. Update the `lean` backlog (add row #21 with the measured verdict and
   plan reference) and the `lean` row in `docs/plans/README.md` (item
   opened-and-closed or fired-to-plan14).
7. Write `report13.md` — final task: method, raw-vs-parsed artifact
   pointers, gate verdicts with numbers, the victim-work analysis (does
   the incumbent score already protect what gets re-probed?), known
   biases, host caveat, additional tools used, problems, next steps
   (plan14 sketch if T1 fired).
