# Report 10 — Cooperative portfolio: cross-racer solved-fact sharing (backlog #6)

**Verdict: REJECT (NO-GO) per the pre-registered rule — H2 fails on 5/6
cases.** The mechanism is sound and stable (H3/H4 clean), and the
pre-registered value trigger fired exactly where predicted (H1 on
m20_white: 0.66× evals / 0.82× wall) — but the facts-plumbing wall tax
(~1.2–1.4× `W_first` at ≈1.0× `evals_first` on every other case) breaches
the pre-registered 1.1× no-regression line broadly enough that the rule
resolves to reject, not defer. Item #6 closes NO-GO in its pre-registered
A1 shape; the value channel is real and its cost channel is precisely
attributed (§ Mechanism diagnosis), so a cheaper facts-plumbing shape is
draftable as a *new* pre-registration if the owner wants to pursue it.

## What was executed

- **Stage 0 (implementation + drift gate).** `src/facts/` (24 B checksummed
  record codec, writer thread, bounded tailed reader, the
  `FactsImporter`), the one-method `search::dfpn::FactImporter` hook
  (drained only between work chunks, after any pre-phase), and the two CLI
  options `--facts-out` / `--facts-in` wired in `main.rs`. The pre-registered
  A2 admission arm (Loss-only) is selected by
  `ATOMIC_SOLVER_FACTS_LOSS_ONLY=1` in the environment, keeping the product
  surface at exactly two options. Gates: `cargo fmt`/`clippy` (all targets)
  clean, `cargo doc` clean, `make test` green (272 unit tests incl. the new
  facts suite: codec round-trip + checksum rejection + CRC vectors, writer
  no-Draw invariant, import idempotence + depth-guard equivalence,
  incremental tail + corrupt-record skip, A2 filter, export-inertness,
  imported-entry resolution, drain-inertness, imported-root-fact
  resolution). **Drift gate clean**: facts off ⇒ quick suite 59/59
  `child_evals` + outcomes identical, m22 + shuffle-win stdout
  byte-identical, TT-snapshot sha256 unchanged
  (`64085743ccf5caa3…`).
- **Stage 1 (export-inertness + volume probe).** Paired same-binary runs
  (m22 s0, dec10 s0, dec10 s2) with and without `--facts-out`:
  stdout **byte-identical** (md5-equal), `evals:` identical
  (19,111,111 / 3,901,847 / 2,549,882) — H4e held at the CLI level, not
  only in unit tests. Facts volume: dec10 ≈ 60–66 k records (1.4–1.6 MB),
  m22 227,727 records (5.5 MB); distinct-key share ≈ 0.19–0.21 (≈5×
  re-delivery at re-visits, the designed no-exporter-dedup behavior).
- **Stage 2 (canary, A1).** One race each on m22_white and stress, machine
  exclusive: clean on both (m22 `W_first` ratio 1.10, stress 1.31 vs the
  pre-registered 2× abort line; zero outcome conflicts, zero panics, zero
  CRC failures, all racers within wall caps). **A2 escalation never
  fired**; the campaign arm stayed A1 (import-all).
- **Stage 3 (campaign).** 5 rounds × {m20_white, stress, m22_white, dec10,
  rem12} + 3 rounds × dec13 (veto-only), one A0 race and one A1 race per
  round, order alternating across rounds; stop-when-proven kill policy;
  per-race records in `measurements/plan10/state/`.
- **Stage 4 (analysis).** `analyze_portfolio10.py` → `state/races10.json`;
  H1–H4 evaluated mechanically.

## Gate results (medians over rounds)

| case | role | A0 evals | A1 evals | ratio evals | A0 wall | A1 wall | ratio wall |
| --- | --- | --- | --- | --- | --- | --- | --- |
| m20_white | hard, 4 basins | 1,152,505,525 | 762,219,180 | **0.661** | 114.7 s | 93.7 s | **0.817** |
| stress | hard, 3 basins | 260,688,794 | 285,054,582 | 1.093 | 27.2 s | 37.2 s | 1.317 |
| m22_white | hard, 6 basins (control) | 16,339,874 | 16,551,679 | 1.013 | 1.85 s | 2.23 s | 1.207 |
| dec10 | multi-basin | 2,549,882 | 2,549,882 | 1.000 | 0.35 s | 0.44 s | 1.255 |
| rem12 | out of sample | 61,821,025 | 61,917,037 | 1.002 | 7.10 s | 9.10 s | 1.283 |
| dec13 (veto-only) | 2 basins | 5,030,497 | 5,030,497 | 1.000 | 0.66 s | 0.82 s | 1.250 |

Per-round distributions are in `state/races10.json`; full A1 round lists:

- m20 A1 evals: 456.4 M, 1202.8 M, **(winnerless round)**, 465.5 M,
  1059.0 M — winner salt 2 in three rounds, salt 5 in one.
- stress A1 evals: 305.4 M (pure R0 won), 285.1 M, 293.5 M, 249.1 M,
  238.0 M — winners spread over salts 0/3/3/3/5 vs A0's constant salt 5.

## Hypotheses

- **H1 (value beyond the portfolio floor) — FIRED on m20_white** (the only
  hard-class candidate it fired on): median `evals_first(A1)` = 0.661× A0
  and `W_first(A1)` = 0.817× A0, both ≤ 0.9×. This is the mechanism's
  entire point working: the winning racer (salt 2) finished at 456 M /
  465 M evals in two rounds — far below its own solo basin minimum
  (~1.38 B per plan12, ~1.2 B under the current binary) — because imported
  solved facts resolved subtrees it would otherwise have re-proven. One
  round converted at 1.06 B (salt 5) and one at 1.20 B (still below A0's
  1.15 B? no — above it: 1.202 B vs 1.153 B, ratio 1.04). Stress did not
  fire (1.09× / 1.32×); m22 did not fire (1.01× / 1.21×).
- **H2 (no regression vs A0) — FAILED on stress, m22_white, dec10, rem12,
  dec13** (wall ratios 1.21–1.32 > 1.1; evals ratios 1.000–1.093, all
  within the line). **The failure channel is wall, not work**: on dec10,
  dec13 and rem12 the A1 winner's evals equal the A0 winner's exactly, yet
  wall is 20–30% higher. Passed only on m20_white (0.82× wall), which is
  also the H1 case.
- **H3 (destabilization) — CLEAN, but moot.** Zero decisive-outcome
  conflicts in 58 races (both arms), zero panics/abnormal exits, zero CRC
  failures across all imports, canary clean on both cases, and the m22
  wall ratio is 1.21 — nothing like the dfpn report10 collapse signature
  (3.1 s → 120 s ≈ 39×). The cross-clock reuse channel stays closed by the
  key (full Zobrist incl. halfmove clock); no Draw ever crossed a process
  boundary. The dfpn report10/11 "injected solved mass destabilizes"
  record does **not** extend to the exact-key cross-worker variant.
- **H4 (fidelity/soundness) — PASS.** (a) Pure R0 export-inertness: R0's
  evals in A1 races reproduce the fresh same-binary solo salt-0 anchors
  bit-for-bit wherever it finishes (stress: 305,404,068 exactly; dec13:
  5,030,497 exactly, in both A1 rounds where it was decisive and in both
  A0 rounds where it was decisive) and its m20 A1 censored count sits at
  the budget cap like the solo censored anchor. (b) Zero decisive-outcome
  conflicts across 58 races. (c) Censoring discipline held throughout
  (budget-censored recorded censored, wall-killed evals excluded; the
  winnerless m20 A1 round is recorded as all-censored, never proven
  draws). (d) Codec invariants unit-tested in the fast gate. (e)
  Export-inertness proven paired at the CLI level (stage 1).

**Verdict rule applied verbatim: *reject* ⇔ H2 fails on any case → H2
failed on five → REJECT.** H1 firing does not rescue it; the A2 escalation
did not run (its canary trigger never fired), so the "NO-GO extends the
destabilization record" clause is *not* the closing reason — the closing
reason is the regression clause.

## Mechanism diagnosis

Where the value came from (m20_white): the winner's work collapsed below
its solo basin minimum — near-root solved subtrees from the other racers
arrived in its TT and resolved whole OR/AND regions it would have
re-proven (24–28 k imports accepted per winner from ~300–400 k delivered
records). This is genuine cross-racer deduplication, live, exactly the
backlog #6 promise. The same channel converted a dec13 round (3.71 M vs
5.03 M) and produced stress's best A1 round (238.0 M vs A0 260.7 M, 0.91×).

Where the cost came from (everywhere else): two additive channels.

1. **Plumbing wall tax (dominant, uniform).** One writer + one reader
   thread per racer, and the emission site on the search thread (channel
   send + `move_stack` clone per proven node, ≈124 k events/s on dec13)
   cost ~15–25% wall on a *solo* sub-second run in a paired micro-probe
   (dec13 s0: plain 515–549 ms, writer-only 647–660 ms, writer+import
   589–599 ms), and ~20–38% under 4-way race contention (pure R0 on the
   stress A1 r1 race: 37.9 s vs its 27.4 s solo anchor for identical
   evals). Under the stop-when-proven policy the winner's wall *is* the
   race answer, so a uniform per-racer tax directly inflates every
   `W_first`. This is a cost of the pre-registered shape (fixed constants:
   fire-and-forget `ProofEvent` sender, one writer + one reader thread per
   racer), not of fact sharing itself — A0 racers pay none of it.
2. **TT pollution / trajectory perturbation (case-dependent).** Imported
   entries (`work = 0`) are preferred eviction victims but still occupy
   slots; cooperative racers' trajectories are chaotic by design (their
   anchors are void). On stress this showed as the A0 winner salt 5
   getting *perturbed off its 260 M solo trajectory* — in A1 r1 no
   cooperative racer beat pure R0's 305 M; in r4/r5 salt 3 landed at
   249/238 M. On m20 it contributed the one catastrophic round: **m20 A1
   r3 was winnerless** — all four racers hit the 2 B budget censored
   (cooperative racers perturbed into bad regions, and R0 is censored
   solo anyway), so the race returned nothing where A0 always answers in
   114 s. 1-in-5 on the primary H1 case is a real tail risk of the
   mechanism, not noise.

CPU cost: Σ racer core-seconds A1/A0 = 1.09× (m20), 1.25× (stress), 1.21×
(m22), 1.31× (dec10), 1.29× (rem12), 1.25× (dec13) — the portfolio's
family-wide 3.9× CPU cost from plan8 grew by ~9–31% under the facts
plumbing.

Facts-channel accounting (campaign totals): delivered 5.45 M records read,
186 k accepted, 0 checksum failures; per case read/accepted: m20
2,605,215/156,507; stress 1,143,637/61,978; m22 391,868/21,107; dec10
403,542/14,636; rem12 768,669/70,099; dec13 143,702/15,784. Skip
(sol_already-solved) rates ~75–93% — the idempotence probe did its job
(imports were cheap; the channel never became the bottleneck).

## Deviations (recorded, none mid-campaign)

1. **Anchor re-baseline (pre-execution).** The plan12/plan8 cross-binary
   anchors are stale for the current binary (e.g. m22 s0 14,156,269 →
   19,111,111; stress s0 249,480,478 → 305,404,068; dec10 s0 4,262,128 →
   3,901,847 — deterministic engine drift from lean/research plans shipped
   after plan12). The H4a fidelity check was re-baselined onto fresh
   same-binary solo salt-0 anchors (`state/solo_s0.json`,
   `solo_anchors10.py`) and in-race paired runs; determinism itself is
   intact (A0 winner evals identical across all 5 rounds on every case).
   Stage-1's "match the plan12/plan8 anchors exactly" was satisfied as
   "match the fresh paired anchors exactly"; the stale-anchor finding is
   recorded here and does not affect any verdict input.
2. **Winnerless-round gate ambiguity (campaign discovery).** The
   pre-registered medians don't define `evals_first` for a race with no
   decisive racer (m20 A1 r3). Analysis reports medians over decisive
   rounds and flags the winnerless round explicitly; under either reading
   (median-over-decisive, or censoring the round at cap) H2's wall
   failures stand unchanged, so the ambiguity does not affect the verdict.
3. Canaries ran their A0 baselines as separate tagged races
   (`*canarybase*`) rather than reusing campaign rounds, since the
   campaign had not started yet — no rule change.

## Problems encountered

- The proof-event emission path clones the full move stack per event —
  the dominant per-event cost on the search thread. This is inherited
  proof-tree-protocol machinery, not facts-specific; any future facts
  revival should either emit key+outcome only or batch through a lock-free
  ring.
- Reader-side file discovery polls at the fixed 2000 ms; racers that
  finish in ≈2 s (m22) import from only the subset of sibling files that
  existed at their first discovery pass (the winner in the m22 canary saw
  2 of 4 files). Harmless for soundness, wasteful for value on short
  races.

## Missing tests / gaps

- No cross-*process* end-to-end integration test (the unit tests cover
  the codec, importer, and hook within one process; the campaign exercised
  the real 4-process shape manually via the drivers). A cheap
  `tests/facts_smoke.rs` (two solver processes, one export dir, assert the
  importer's outcome) would pin the wiring; not written in-plan.
- The A2 arm was never exercised end-to-end at CLI level (only unit-level
  with a hand-built queue) — acceptable since it never ran as a campaign
  arm, but the env-var switch is therefore untested in situ.

## Backlog / index updates

- `parallel` backlog **#6: closed NO-GO** (this report); row updated in
  `parallel/initiative.md` (value demonstrated on m20 only; cost channel =
  plumbing wall tax + TT pollution; A2 fallback never triggered; m20
  winnerless tail recorded).
- `docs/plans/README.md` `parallel` row updated (close event).
- Backlog #7 (any-worker-can-finish SPDFPN) untouched — its premise is
  unchanged by this result.

## Next steps (options, owner's call)

1. **Close the cooperative-portfolio family.** #6's measured record joins
   plan2/5b/6/7 as the fourth measured parallelism mechanism; the shipped
   plan8/plan9 salt portfolio remains the only adopted lever.
2. **Re-draft the facts plumbing at lower tax as a new plan** (new
   pre-registration under pinned gate v1.0) *only if* the owner weights
   the m20 value channel (0.66× evals, 0.82× wall, winnerless tail)
   higher than the uniform tax: key-only emission (no path clone), batched
   import scheduling, and an import-facts flag on `examples/portfolio`
   would be the natural shape. The H1 mechanism itself is validated; only
   its carrier was too expensive.

## SESSION COMPLETE

- Deliverables: `src/facts/` + `search::dfpn::FactImporter` + `--facts-out`
  / `--facts-in` shipped (drift gate clean, `make test` green, fmt/clippy/doc
  clean); `measurements/plan10/` complete (drivers + `state/*.json` +
  `races10.json` + `solo_s0.json` + `env.json` + `README.md` provenance);
  `report10.md` written; backlog #6 + `docs/plans/README.md` updated.
- Follow-up options:
  1. `Execute docs/plans/parallel/plan11.md` — only if a plan11 (cheaper
     facts plumbing, key-only emission + batched imports) is first drafted
     and pre-registered; the H1 value channel (m20 0.66× evals) justifies
     drafting it, the 1.2–1.3× wall tax justifies not shipping A1 as-is.
  2. `Close the parallel initiative's cooperative-portfolio thread
     (backlog #6 NO-GO stands; update docs/plans/README.md to reflect that
     no plan11 is planned) and leave #7 (any-worker-can-finish SPDFPN) as
     the only open item` — the conservative default given the reject.
