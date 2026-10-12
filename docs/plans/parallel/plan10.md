# Plan 10 — Cooperative portfolio: cross-racer solved-fact sharing (backlog #6)

**Status: READY (drafted 2026-10-13, pre-registered before any run).**
Executes the `parallel` backlog item **#6** (cooperative portfolio, ManySAT
shape), sequenced behind and unblocked by plan8's ADOPT verdict (draw-luck
regime confirmed at gate grade) and plan9's shipped `examples/portfolio`
driver (the harness skeleton). Final task: `report10.md`.

## Motivation

Plan8/plan9 shipped the salt portfolio and proved its exact value shape:
**min-of-draws**. The stage-1 pre-computation held exactly — in all 18
races, `evals_first` equaled the predicted basin minimum, i.e. the winner
is always the racer that would have won solo, and the portfolio harvests
nothing beyond `min` over racer trajectories. The measured cost of that
shape is duplicated work:

- **m20_white**: three racers (s5/s3/s2) *independently proved the root*
  (791 M + 862 M + 1.38 B evals ≈ 3 B evals spent where 791 M suffices).
- **stress**: 543 of 606 core-s were two grace-killed racers burning
  censored trajectories that never contribute.

Backlog #6's mechanism adds the missing cooperation: racers share
**solved facts** (proven Win/Loss at an exact Zobrist key) through an
append-only per-race channel, so duplicated subtree proofs are
deduplicated *live* — the winning racer's work can shrink below its own
solo basin minimum, and a racer stuck in a bad region can be unlocked by
facts proven elsewhere. One **pure racer** (exports, never imports) keeps
the structural worst case at the sequential wall, exactly as the backlog
row specifies.

This is a **mechanism probe with genuine NO-GO risk**, and the risk is
measured, not hypothetical: `dfpn` report10/report11 measured solved-mass
injection into DF-PN destabilizing the threshold arithmetic (cross-clock
solved-entry reuse: stress −37.7 % but **m22 collapsed 3.1 s → 120 s**;
all three quarantine arms failed — "injected solved mass at volume
destabilizes"). Cross-worker exact-key injection is the *unmeasured*
variant: it cannot produce cross-clock folds (the key includes the
halfmove clock), but it multiplies the frequency of the same
solved-child-into-unsolved-parent fold channel. **m22_white is therefore
the designated collapse control**, and a cheap canary precedes the
campaign (stage 2), with one pre-registered fallback arm before any
NO-GO.

## Mechanism (arm A1)

Racers are the plan9 set of unmodified solver processes, identical racer
set and flags; the only delta is the facts plumbing. Two new CLI options
(the entire product-code surface):

- **`--facts-out <file>`** — the CLI sets the search's proof-event sender
  (already exists: `Search::set_proof_event_sender`); a writer thread
  drains `ProofEvent::NodeProven` events and appends fixed-size binary
  records to `file`. The search's semantics are untouched: emission is
  fire-and-forget (`let _ = sender.send(..)`), and the sender is already
  an inert observer by design.
- **`--facts-in <dir>`** — a reader thread tails every sibling
  `facts-*.bin` in `dir` (incremental offsets, never re-read), pushes
  checksum-verified records into a **bounded** swap buffer, and the search
  drains the buffer **on the search thread** between work chunks (a
  one-method importer hook defined in `search::dfpn`; `search` learns
  nothing about files — dependency direction preserved, codec lives in a
  new `src/facts.rs`, wiring in `main.rs`).

Record format (24 B fixed): `key: u64` (full Zobrist incl. halfmove
clock), `outcome: u8`, `depth: u32`, `crc32: u32` over the payload,
padding. **Checksum-verified or dropped** — a torn read is skipped, never
misread (a flipped outcome byte would be a false-fact injection; the CRC
closes that channel).

**Export site coverage** (verified against the code): `NodeProven` is
emitted at all three solved-result sites — frame exit (`core.rs` ~404),
terminal children (`children.rs` ~368), and TT-resolved re-proofs
(`core.rs` ~123). Draws never emit. So the stream covers the solved
Win/Loss mass; the TT-hit site re-emits duplicates on re-visits — **by
design, no exporter dedup**: duplicates are the re-delivery mechanism for
facts evicted from a consumer's TT (imported entries carry `work = 0` and
are the preferred eviction victims, protecting the racer's own entries).
The consumer imports idempotently: probe first, skip if the entry is
already solved, else store through the **same `tt.store` solved path**
(`remaining_depth = u32::MAX`, `depth` = record depth, `best_move =
Move::NONE`), so reuse semantics are bit-identical to a self-proven entry
(`resolved_from_entry` unchanged).

Admission rule (pre-registered, single arm): **import-all** — every
checksum-verified Win/Loss record at an exact key, no depth floor, no
direction filter.

**Fallback arm A2 (pre-registered escalation, not arm shopping):** import
**Loss-facts only** (Win records ignored on import). Rationale: `dfpn`
report11 — Loss-only was the least destabilizing direction at cross-clock.
Escalation rule in stage 2.

Fixed constants (not knobs): poll/drain interval 2000 ms, reader swap
buffer 1 MiB, one writer + one reader thread per racer.

## Soundness contract (why no false decisive outcome)

1. **A fact is exactly the TT's path-independent solved class.** Exports
   are Win/Loss only (Draw filtered at emission; repetition-suppressed
   results are Draws and never enter); the TT already stores and reuses
   precisely this class across paths within a run. Import stores through
   the same `tt.store` solved path — an imported fact is semantically
   *the same entry the consumer's own search would have written*, so reuse
   is "same as self-proven" by construction, and the
   `best_move_repeats_path` GHI guard applies to imported entries exactly
   as to native ones (consumer-side, unchanged).
2. **No cross-clock reuse — the report10 channel is closed by the key.**
   The Zobrist key includes the halfmove clock; a fact is valid exactly at
   the (position, clock) it was proven at, which is the only key it
   carries. What cross-worker sharing changes is fold *frequency*, not
   fold *validity* — that is a stability risk (H3), not a soundness risk.
3. **No Draw ever crosses a process boundary** — the GHI-sensitive class
   stays per-process (first-player-loss shortcut, per-worker repetition
   caches untouched). Soundness invariant 2 (zero decisive-outcome
   conflicts) is a hard gate below.
4. **Trust base is narrow and stated**: same binary, same root FEN, peer
   set = the driver's spawned racers, single machine. Checksums guard
   channel corruption; a dropped fact is a lost optimization, never
   unsound. The `--race`-style product flag and any persisted cross-solve
   store (the `research` plan10 hazard class) remain out of scope — the
   facts dir is per-race and ephemeral.
5. **Preflight non-interaction**: imports happen only during the search
   loop, after the pre-phase; the preflight's no-TT-interaction contract
   is untouched (and the raced cases are outside the preflight gate).
6. **Determinism restatement (constraint 3)**: with `--facts-in`, a
   racer's result is no longer a deterministic function of (FEN, config,
   salt) — it depends on interleaving. `ExitReason::BudgetExhausted` →
   `Draw` semantics are unchanged per run; the *value* becomes
   nondeterministic. Documented in `main.rs`'s module header.
7. **RAM restatement (constraint 4)**: with facts options on,
   RAM = TT + ≤ ~2 MiB of I/O buffers (bounded swap buffer, no dedup
   structures). Without them, RAM = TT only, unchanged. Same restatement
   pattern as the preflight's `REGION_BUDGET` exception.

## Gate adaptation (declared, with reason)

The pinned v1.0 rule pairs candidate/baseline per salt. Arm A1 breaks the
pairing's null (identical arms): an imported fact perturbs the racer's
trajectory, so `candidate_s` is not a rerun of `baseline_s`. Adaptation
(stated per the methodology's revision clause):

- **Pairing unit = the race, not the salt.** Each round runs one fresh A0
  race (facts off — the shipped plan9 behavior, deterministic per racer)
  and one A1 race (facts on), interleaved, same case, same machine
  conditions. Case statistics are medians over rounds of
  `evals_first`/`W_first` for A1 vs A0.
- **Per-racer anchors survive where determinism survives**: A0 racers and
  the A1 **pure racer R0** must reproduce their plan12/plan8 anchors
  bit-for-bit (fidelity gates). Cooperative racers (R1–R3) have no anchor
  by design — their solo plan12 evals are recorded as the *reference
  draw*, and per-racer evals inflation vs that reference is reported as a
  diagnostic (chaos makes it non-gating), never as a verdict input.
- **Verdict categories** map 1:1 onto the pinned rule (censored-win/
  censored-loss/win/regression via `W_LO = 0.9` / `W_HI = 1.1` +
  direction agreement), applied to the A1-vs-A0 race medians.
- **Diversity clause** as pinned: basin counts (plan8/plan12) reported
  next to every verdict; **dec13 (2 basins) is low-diversity** — veto-only.

Soundness invariants apply unchanged (censoring rules; budget-censored
never a proven draw; under-budgeted rule with caps below).

## Hypotheses (pre-registered, mechanical)

- **H1 (value beyond the portfolio floor; the adopt trigger).** On ≥ 1
  **hard-class** case with A0 basin count ≥ 3 (candidates: m20_white,
  m22_white, stress): median `evals_first(A1)` ≤ 0.9 ×
  `evals_first(A0)` **and** median `W_first(A1)` ≤ 0.9 × median
  `W_first(A0)` (wall must track the work reduction). This is the
  mechanism's entire point: the winner finishes with strictly less work
  than the best solo draw, or a censored racer converts faster than A0's
  conversion. A1 matching A0 everywhere is a **defer**, not an adopt —
  the plain portfolio already ships.
- **H2 (no regression vs A0; the pinned no-regression clause).** On every
  raced case: median `evals_first(A1)` ≤ 1.1 × `evals_first(A0)` and
  median `W_first(A1)` ≤ 1.1 × median `W_first(A0)`. The pure-R0 floor
  bounds the evals regression on salt-0-min cases (stress, m22, dec13) at
  the sequential value; the real regression channel is dec10/rem12
  (multi-basin, where A0's winner beats salt-0) — imports perturbing the
  winning basin negatively. That channel is exactly what H2 gates.
- **H3 (destabilization canary; hard gate).** Stage-2 abort signals
  (single canary race each on m22_white and stress): any decisive-outcome
  conflict, any panic, or m22 `W_first` > 2× the A0/frozen value (the
  report10 collapse signature is m22 3.1 s → 120 s ≈ 39×; 2× is a
  generous early-warning line). In the campaign, H3 = no m22/stress
  regression beyond H2's 1.1× and zero outcome conflicts anywhere.
  A canary firing routes to the A2 escalation, never to a verdict.
- **H4 (fidelity/soundness; hard gate; halt-and-diagnose on failure).**
  (a) **R0 export-inertness**: the A1 pure racer reproduces its plan12/
  plan8 anchor (evals + outcome) bit-for-bit in every round — exporting
  must not perturb the search. (b) Zero decisive-outcome conflicts across
  racers in any race, either arm. (c) Censoring discipline: budget-
  censored racers recorded as censored, wall-killed as killed, evals
  excluded from verdicts. (d) Codec invariants (unit-tested): records are
  Win/Loss only; checksum rejection drops corrupt records; an imported
  entry resolves via `resolved_from_entry` exactly as a self-proven entry
  at the same key/depth, and never enters the unsolved-bounds TT read
  branch (`children.rs`, excluded by `remaining_depth = u32::MAX`).
  (e) `--facts-out` alone is byte-inert: stdout, outcome, and evals
  identical to the same run without it.

**Verdict rule.** *adopt* ⇔ H1 ∧ H2 ∧ H3 ∧ H4 (H1's case must be
hard-class, per gate v1.0). *reject* ⇔ H2 fails on any case or the A2
escalation also fails its canary — the dfpn destabilization record then
extends to the exact-key cross-worker variant and **#6 closes NO-GO**.
*defer* ⇔ H2/H3/H4 clean but no H1 case fires: the mechanism is stable
but adds no measured value over the shipped portfolio at this scale;
recorded as such (a negative result is a valid completion). H4 failure ⇒
soundness defect → halt and diagnose, not a lever verdict.

## Fixed parameters

Corpus: the frozen plan12 case set (FENs/caps in
`../../research/measurements/plan12/driver.py`); raced subset = the plan8
raced table plus rem12, with one change: dec13 is retained **veto-only**
(low-diversity, 2 basins; it can never support an adopt).

| case | evals cap | race wall cap | rounds | role |
| --- | --- | --- | --- | --- |
| m20_white | 2 B | 500 s | 5 | hard class, 4 basins — primary H1 candidate (A0 winner s5, 791,401,553) |
| stress | 2.5 B | 450 s | 5 | hard class, 3 basins — H1 candidate + canary (A0 = salt-0 min, 249,480,478) |
| m22_white | 1 B | 60 s | 5 | hard class, 6 basins — **collapse control** + H1 candidate |
| dec10 | 1 B | 120 s | 5 | multi-basin regression channel (A0 winner s2, 2,918,636 vs salt-0 4,262,128) |
| rem12 | 1 B | 120 s | 5 | out of sample (6 basins; A0 winner s3, 59,573,726) |
| dec13 | 1 B | 60 s | 3 | veto-only (2 basins) |

Racer set — identical to plan9, fixed: R0 = salt 0 (**pure**: `--facts-out`
only), R1 = salt 2, R2 = salt 3, R3 = salt 5 (cooperative: `--facts-out`
+ `--facts-in <race dir>`). All racers: default config, TT 128 MB,
`--budget <case cap>`, `--first-outcome --outcome-only`,
`--timeout <race wall cap>`. Facts dir per race, deleted after.

Estimated compute: A0 walls are the plan8 medians (m20 ≈ 96 s, stress
≈ 105 s, others ≤ 16 s); A1 walls unknown, capped. Worst case
≈ 5 × (100 + 500) s (m20) + 5 × (105 + 450) s (stress) + small cases ≈
2.5–3 h. The m20/stress A1 caps bind only if facts *hurt* — itself a
measured signal, recorded, not a failure.

## Stages

1. **Stage 0 — implementation + drift gate.** `src/facts.rs` (codec,
   writer/reader threads), the importer hook in `search::dfpn`, CLI
   options + wiring in `main.rs` (module-header docs: determinism and
   RAM restatements). `cargo fmt`/`clippy`/`doc` clean; `make test` green
   (new unit tests: codec round-trip + checksum rejection, import
   idempotence, `resolved_from_entry` equivalence, no-Draw invariant,
   export-inertness). Drift protocol: facts options off ⇒ quick suite
   59/59 `child_evals` identical, m22/shuffle-win stdout byte-identical,
   TT-snapshot sha unchanged.
2. **Stage 1 — export-inertness + facts-volume probe.** Solo runs with
   `--facts-out`: (m22_white, s0), (dec10, s0), (dec10, s2) — evals/
   outcome/stdout must match the plan12/plan8 anchors exactly (H4a/H4e).
   Record facts stats per run: records written, distinct keys (dedup
   ratio), file bytes, writer wall overhead. This freezes the volume
   picture before any cooperative run.
3. **Stage 2 — canary (A1; escalation to A2 on failure).** One A1 race
   each on m22_white and stress (machine exclusive). Clean ⇔ no outcome
   conflict, no panic, `W_first` ≤ 2× the A0/frozen value on both, all
   four racers within their wall caps. If A1 aborts: run the same canary
   once with **A2** (Loss-only import); A2 clean ⇒ A2 becomes the
   campaign arm (declared adaptation, recorded); A2 aborts ⇒ **NO-GO,
   initiative #6 closes** (report documents the destabilization extension).
4. **Stage 3 — campaign.** R rounds per case as tabled; per round: one A0
   race, then one A1 race, interleaved (A0/A1 alternate order across
   rounds to decorrelate ambient drift). Per-racer records: wall,
   outcome, evals, censored/wall-killed flag; per-race facts stats
   (records read, checksum failures, imports accepted/skipped) and the
   winner's identity. Kill policy unchanged: stop-when-proven (first
   decisive ⇒ kill the rest; their evals excluded).
5. **Stage 4 — analysis.** `analyze_portfolio10.py`: per case, A1 vs A0
   medians (`evals_first`, `W_first`), full per-round distributions,
   per-racer evals vs solo references, facts stats (imports accepted,
   skip rate, dedup ratio), basin counts, H1–H4 evaluated mechanically,
   CPU cost (Σ racer core-seconds; A1 vs A0). Results →
   `state/races10.json` + `README.md` provenance (drivers + parsed
   results committed; raw transcripts never).
6. **Final task — report.** `report10.md`: verdict against the
   pre-registered rule, mechanism diagnosis (where the value came from —
   near-root subtree dedup vs censored conversion — or why it didn't),
   facts-channel accounting, deviations; update backlog #6 and the
   `docs/plans/README.md` `parallel` row only on open/pivot/close events.
   On adopt, the report records the ship shape (integrating `--facts`
   into `examples/portfolio` — the `RaceTracker` skeleton takes the facts
   dir as one more input; S effort) as the follow-up, per the plan9
   precedent that ship shapes are their own decision.

## Scope boundary

- Product surface = exactly the two CLI options + the importer hook +
  `src/facts.rs`; the search's sequential path is untouched (drift gate
  in stage 0). No `--race` product flag; the `examples/portfolio` driver
  change is the adopt follow-up, not in-plan.
- No persistence: facts are per-race, ephemeral, never written into the
  proofdb pipeline, TT snapshots, or any cross-solve store.
- Stays closed (non-goals unchanged): deterministic sibling parallelism,
  process-per-child option C, Čížek option D, cross-worker reuse of
  Draw/repetition-dependent facts, cross-solve solved-fact stores.
- One machine, 4 CPUs; races monopolize the container (plan7/8
  convention). N = 4 is real concurrency; nothing above is claimed.
- Backlog #7 (any-worker-can-finish SPDFPN) is **not** in scope; #6 and
  #7 remain independent pre-registrations on the same harness skeleton.

## Deviation policy

Mid-campaign mechanism, arm, racer-set, cap, or admission-rule changes are
forbidden; the only permitted in-plan escalation is the pre-registered A1
→ A2 canary rule. Anything else discovered goes into `report10.md` as a
finding, and any follow-up is a new plan with its own pre-registration
under the pinned gate.
