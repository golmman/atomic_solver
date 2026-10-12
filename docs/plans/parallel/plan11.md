# Plan 11 — Low-tax solved-facts carrier: key-only emission, chunk-boundary handoff, admission filter (backlog #6 follow-up)

**Status: READY (drafted 2026-10-13, pre-registered before any run).**
Executes report10's next-steps option 2 (the owner weights the m20 value
channel higher than the uniform tax) as backlog item **#12** (row added at
execution start; #6 stays closed NO-GO — this is its cheaper-carrier
follow-up, not a re-run). Reuses plan10's soundness surface, harness, and
race-pairing gate adaptation verbatim. Final task: `report11.md`.

## Motivation

Plan10's verdict split the mechanism from its carrier: the value channel
is real (**H1 fired on m20_white: 0.66× evals / 0.82× wall — the winner
finished below its own solo basin minimum via near-root subtree dedup**)
and soundness/fidelity were clean everywhere, but a uniform
facts-plumbing wall tax (**1.2–1.4× `W_first` at ≈1.0× `evals_first`**)
breached the 1.1× no-regression line on 5/6 cases. Report10 attributes
that tax to three plumbing artifacts of the pre-registered shape, not to
fact sharing itself:

1. **Emission path clone**: `emit_proof_node` clones the full
   `move_stack` per proven node (and the terminal-children site clones
   again), ≈124 k events/s on dec13 — +15–25 % wall on a *solo* sub-second
   run.
2. **Two extra threads per racer** (writer + reader): 8 threads on 4 CPUs
   under a 4-way race — +20–38 % wall on the pure racer in-race (37.9 s vs
   its 27.4 s solo anchor at identical evals).
3. **Per-record handoff**: the reader pushes each of the 5.45 M delivered
   records through a mutex+condvar queue individually.

Plan11 tests the attribution hypothesis mechanically: rebuild the carrier
so all three artifacts are structurally absent, pre-register solo and
in-race tax ceilings that plan10's carrier fails by an order of
magnitude, and re-run the same campaign. **If the tax persists at the new
carrier, the attribution is falsified and the cooperative-portfolio
thread closes** (this is the second and final pre-registration on this
mechanism — no plan12).

## Mechanism (the low-tax carrier)

Same semantics as plan10 arm A1 (export every solved Win/Loss fact,
import siblings' facts into the TT, one pure racer), three structural
changes, zero new CLI options:

- **Key-only emission (export).** A new one-method hook
  `search::dfpn::FactExporter { fn export(&self, key: u64, outcome:
  Outcome, depth: u32, path_len: u8) }` (Send, pure observation, must
  return promptly and never panic — the `FactImporter` contract's
  mirror). `--facts-out` wires a `facts::FactsExporter` via
  `Search::set_fact_exporter`; **the proof-event sender stays `None`**, so
  no `NodeProven` path clone ever happens on the facts path and the
  `ProofEvent` protocol is untouched (proof-tree layer unaffected). The
  exporter **writes synchronously on the search thread** into a
  process-owned `BufWriter` (the search is single-threaded per process —
  no lock, no channel, no writer thread): encode (24 B, CRC-32) + buffered
  write, flush every 256 records (~6 KB ≈ one syscall per 256 events) and
  at pre-exit. `path_len` = `move_stack.len()` (frame exit, TT re-proof)
  or `len + 1` (terminal children) — a scalar already at hand, no clone.
  A write failure stops exporting silently (lost optimization, never a
  search failure).
- **Chunk-boundary handoff (import).** The reader thread, queue, and
  condvar are deleted. The `FactsImporter` (still the one-method
  `search::dfpn::FactImporter` hook, interface unchanged) reads the
  sibling `facts-*.bin` files **directly, on the search thread, at work
  chunk boundaries**: per call, stat every discovered file, read new
  bytes incrementally (persistent offsets, torn tail retried, checksum
  failure skips exactly one record stride), decode, admit, store —
  bounded by a **1 MiB per-call read cap across all files** (~1 ms; the
  hook's "must return promptly" contract holds). Discovery at the first
  call + rescan no more often than once per 1 s of search time.
  **Freshness analysis (why this suffices)**: work chunks start at 500 k
  evals and double, so the hook fires ~4–11 times per raced case — but
  the value plan10 measured is **near-root** subtree dedup, and near-root
  facts are proven inside every racer's *first* chunk: by the winner's
  2nd–3rd boundary (~0.2–0.5 s in), its siblings' early exports are on
  disk (writer flushes every 256 records). The facts that arrive late are
  the deep-path mass — exactly what the admission filter removes anyway.
  Plan10's 2000 ms discovery staleness finding (m22 canary saw 2 of 4
  files) disappears: discovery at the first boundary, rescan every ≥1 s,
  and files are created at racer startup.
- **Admission filter (fixed constant, not a knob).** The exporter emits a
  record **iff `path_len ≤ PATH_LEN_CAP = 24`** (plies from the racer's
  root, saturating cast for the u8 field; the filter compares the real
  length). Rationale: all racers search the same root with near-identical
  static ordering, so the near-root region is the shared mass — and
  plan10's measured value came precisely from near-root subtree dedup
  (m20 winner accepted 24–28 k imports), while deep-path records are
  trajectory-specific noise that pollutes the consumer TT (the m20
  winnerless round channel). Import side stays **import-all within the
  emitted stream** (every checksum-verified Win/Loss record; no depth
  floor, no direction filter). The filter cuts both cost channels at
  once: delivered volume (plan10's ≈5× re-delivery is mostly deep-path
  re-proofs) and TT pollution. Pass rate is *reported*, never tuned
  mid-plan.

Record format v2 (24 B fixed, as plan10 but byte 9 repurposed):
`key: u64 LE | outcome: u8 (1=Win, 2=Loss) | path_len: u8 | depth: u32 LE
| crc32: u32 LE (over bytes 0..16) | padding: 6 B zero`. Checksum-verified
or dropped, exactly as plan10. **Not compatible with plan10 v1 files**
(no persisted consumers; facts dirs are per-race and ephemeral).

## Soundness contract (plan10 surface, carried over)

Items **unchanged from plan10** (normative, restated by reference to the
`src/facts` module docs after rework): a fact is exactly the TT's
path-independent solved class; import stores through the same
`tt.store` solved path a self-proven result takes (`remaining_depth =
u32::MAX`, `best_move = Move::NONE`, `work = 0`), probe-first idempotent;
no cross-clock reuse (the key includes the halfmove clock); **no Draw
ever crosses a process boundary**; narrow trust base (same binary, same
root FEN, spawned peer set, one machine; a dropped fact is a lost
optimization, never unsound); preflight non-interaction (imports only at
hook calls, after any pre-phase); eviction treats `work = 0` as the
preferred victim.

Items **changed by this plan**, re-documented at implementation:

1. **Export observation path.** The `FactExporter` hook is a pure
   observer at the same three solved-result sites; it reads only values
   already computed (`hash`, `outcome`, `depth`, `move_stack.len()`), so
   the search's trajectory is bit-identical with it on or off (H4e gates
   this at the CLI level, M1 at the micro-probe level). The
   `emit_proof_node` Draw filter stays the first gate; the exporter
   re-checks as defense in depth.
2. **Synchronous I/O on the search thread.** The exporter's buffered
   writes and the importer's capped reads now run on the search thread at
   chunk boundaries. Bounded work per boundary (flush every 256 records;
   ≤ 1 MiB reads); a blocking-I/O stall is a bounded latency cost, never
   a correctness cost. The hook contract ("only between work chunks,
   never during a recursion, never during the pre-phase") is preserved
   verbatim — the *location* of the I/O changed, not the *boundaries*.
3. **RAM restatement (constraint 4).** With facts options on: RAM = TT +
   ~1 MiB (importer scratch + read cap buffers) + ~8 KiB (exporter
   BufWriter) — **smaller** than plan10's ~2 MiB (no queue, no threads).
   Without them: RAM = TT only, unchanged.
4. **Determinism restatement (constraint 3).** Unchanged from plan10:
   with `--facts-in`, a racer's result depends on interleaving;
   `BudgetExhausted → Draw` semantics unchanged per run; documented in
   `main.rs`'s module header.
5. **Removed measurement scaffolding.** The plan10 A2 arm
   (`ATOMIC_SOLVER_FACTS_LOSS_ONLY=1`) and the writer/reader threads are
   deleted. Plan10's fallback is history; **plan11 has no escalation arm**
   (see H3) — a second carrier failure must close the family, not spawn
   a third shape.

## Gate adaptation (declared: reuse plan10's verbatim)

The race-pairing adaptation from plan10 carries over unchanged: pairing
unit = the race (one fresh A0 race + one A1 race per round, interleaved,
order alternating across rounds); per-racer anchors survive only where
determinism survives (A0 racers and the A1 pure racer R0); cooperative
racer evals inflation vs solo references is a diagnostic, never a verdict
input; verdict categories map 1:1 onto pinned gate v1.0 (`W_LO = 0.9` /
`W_HI = 1.1` + direction agreement); diversity clause as pinned, dec13
(2 basins) veto-only. One sharpening, pre-registered now to fix plan10's
deviation-2 ambiguity in advance: **a round with no decisive racer
contributes a winnerless flag, not a median sample; ≥ 2 winnerless A1
rounds on any case fails H2 for that case** (1 matches plan10's observed
tail; 2 is a pattern).

## Hypotheses (pre-registered, mechanical)

- **H-mech (the report10 wall-tax attribution; the plan's reason to
  exist).** The tax is plumbing-shaped and the new carrier removes it:
  - **M1 (solo tax)**: paired interleaved solo runs (5 pairs each, plain
    vs `--facts-out`) on dec13 s0 (highest emission rate), dec10 s0, m22
    s0 (largest volume): median wall ratio ≤ **1.03** on every probed
    case (plan10's carrier measured 1.19–1.25 solo), with stdout
    byte-identical and evals identical in every pair.
  - **M2 (in-race contention tax)**: pure R0's in-race wall vs its fresh
    same-binary solo anchor, median over all A1 rounds where R0 finishes
    decisive, ≤ **1.06** on every case (plan10: 1.38×; plan8 measured
    contention-only at 1.02–1.05, so ~1–4 % carrier headroom).
  **M1/M2 failure ⇒ the report10 attribution is falsified** (the tax is
  intrinsic to sharing volume, not the plumbing shape) ⇒ **NO-GO, family
  closes**. M1/M2 pass is a *necessary* condition, not value: see H1.
- **H1 (value beyond the portfolio floor; the adopt trigger).** Identical
  to plan10's: on ≥ 1 hard-class case with A0 basin count ≥ 3
  (candidates: m20_white, m22_white, stress): median `evals_first(A1)` ≤
  0.9 × `evals_first(A0)` **and** median `W_first(A1)` ≤ 0.9 × median
  `W_first(A0)`. Pre-registered prediction: fires on **m20_white** (the
  only case plan10's H1 fired on; the filter deliberately preserves the
  near-root channel that produced it). Note the filter makes H1 *harder*
  than plan10 in one respect — fewer imports means less dedup mass — and
  *easier* in the one that matters: the winner's wall now tracks its
  evals (M2) instead of carrying a 1.2–1.4× tax.
- **H2 (no regression vs A0; the pinned no-regression clause).** On every
  raced case: median `evals_first(A1)` ≤ 1.1 × `evals_first(A0)` and
  median `W_first(A1)` ≤ 1.1 × median `W_first(A0)`, and ≤ 1 winnerless
  A1 round. Plan10 passed the evals conjunct everywhere (1.000–1.093);
  the wall conjunct is exactly what the new carrier must recover. The
  residual regression channel is TT pollution (imports perturbing the
  winning basin) — the filter targets it; H2 gates whatever remains.
- **H3 (destabilization canary; hard gate).** Same as plan10: stage-2
  canaries on m22_white and stress (one race each); abort signals = any
  decisive-outcome conflict, any panic, or m22 `W_first` > 2× the A0
  value (the dfpn report10 collapse signature is ~39×; 2× stays a
  generous early-warning line). **No escalation arm: a canary abort ⇒
  NO-GO, family closes.** In the campaign, H3 = no m22/stress regression
  beyond H2's 1.1× and zero outcome conflicts anywhere.
- **H4 (fidelity/soundness; hard gate; halt-and-diagnose on failure).**
  Plan10's surface, with fresh in-session anchors (report10 deviation-1
  protocol — cross-binary anchors are never used):
  (a) R0 export-inertness: the A1 pure racer reproduces its fresh
  same-binary solo salt-0 anchor (evals + outcome) bit-for-bit in every
  round where it finishes, and its censored evals sit at the budget cap
  where it is censored solo. (b) Zero decisive-outcome conflicts across
  racers in any race, either arm. (c) Censoring discipline: budget-
  censored recorded censored, wall-killed as killed, evals excluded from
  verdicts. (d) Codec/filter invariants (unit-tested): v2 round-trip +
  CRC rejection; Win/Loss only (no Draw encodes or imports); path_len
  filter passes/fails exactly at the cap; importer incremental tail,
  torn-tail retry, corrupt-record skip, catch-up across the read cap;
  import idempotence; an imported entry resolves via
  `resolved_from_entry` exactly as a self-proven entry and never enters
  the unsolved-bounds TT read branch. (e) `--facts-out` alone is
  byte-inert at the CLI level: stdout, outcome, and evals identical to
  the same run without it (stage-1 pairs).

**Verdict rule.** *adopt* ⇔ H-mech (M1 ∧ M2) ∧ H1 ∧ H2 ∧ H3 ∧ H4. *reject*
⇔ M1 or M2 fails (attribution falsified), or H2 fails on any case, or a
canary aborts — **in every reject case the cooperative-portfolio thread
closes for good** (second strike; the shipped plan8/plan9 portfolio
remains the family's only adopted lever). *defer* ⇔ H-mech/H2/H3/H4 clean
but no H1 case fires: stable and cheap but no measured value at this
scale — the thread still closes, with the soundness record standing.
H4 failure ⇒ soundness defect → halt and diagnose, not a lever verdict.

## Fixed parameters

Corpus, racer set, caps, and rounds — **identical to plan10's table**
(FENs/caps in `../../research/measurements/plan12/driver.py`):

| case | evals cap | race wall cap | rounds | role |
| --- | --- | --- | --- | --- |
| m20_white | 2 B | 500 s | 5 | hard class, 4 basins — primary H1 candidate |
| stress | 2.5 B | 450 s | 5 | hard class, 3 basins — H1 candidate + canary |
| m22_white | 1 B | 60 s | 5 | hard class, 6 basins — **collapse control** + H1 candidate |
| dec10 | 1 B | 120 s | 5 | multi-basin regression channel |
| rem12 | 1 B | 120 s | 5 | out of sample (6 basins) |
| dec13 | 1 B | 60 s | 3 | veto-only (2 basins) |

Racer set identical to plan9/plan10, fixed: R0 = salt 0 (**pure**:
`--facts-out` only), R1 = salt 2, R2 = salt 3, R3 = salt 5 (cooperative:
`--facts-out` + `--facts-in <race dir>`). All racers: default config,
TT 128 MB (explicit — matches plan10 for comparability, despite the lean
plan15 default bump), `--budget <case cap>`, `--first-outcome
--outcome-only`, `--timeout <race wall cap>`. Facts dir per race,
deleted after.

Fixed constants (not knobs): `PATH_LEN_CAP = 24`; importer read cap
1 MiB/call; discovery rescan ≥ 1 s; exporter flush every 256 records;
scratch buffer 1 MiB.

**A0/A1 baselines are re-run in-session** (fresh paired anchors; the
report10 stale-anchor finding makes cross-binary anchors unusable by
policy). Estimated compute ≈ plan10's: ~2.5–3 h (A0 walls ≈ plan10
medians; A1 walls capped by the table).

## Stages

1. **Stage 0 — implementation + drift gate.** `src/facts/` rework: record
   v2, `FactsExporter` (synchronous buffered writer, counters), threadless
   `FactsImporter` (direct capped reads at hook calls), delete the
   writer/reader threads + queue + A2 env arm; module docs updated (the
   changed soundness items above). `search::dfpn`: `FactExporter` trait +
   `set_fact_exporter` + the two emission-site calls (frame exit / TT
   re-proof via `emit_proof_node`, terminal children in `children.rs`);
   the `FactImporter` hook interface unchanged. `main.rs`: `--facts-out`
   wires the exporter (sender stays `None`), `facts_export:` /
   `facts_import:` stderr lines kept (exporter line gains
   emitted/filtered counts), module-header restatements. `cargo
   fmt`/`clippy`/`doc` clean; `make test` green (new/updated unit tests
   per H4d + export-inertness). Drift protocol: facts options off ⇒
   quick suite 59/59 `child_evals` identical, m22/shuffle-win stdout
   byte-identical, TT-snapshot sha unchanged.
2. **Stage 1 — mechanism probes (M1) + volume probe.** 5 interleaved
   pairs per case (dec13 s0, dec10 s0, m22 s0): plain vs `--facts-out`;
   wall medians, stdout md5 equality, evals equality. Facts stats per
   run: records seen, emitted (post-filter), filtered count, distinct
   keys, bytes; pass rate and volume vs plan10's stage-1 numbers
   reported. M1 evaluated mechanically here; failure ⇒ NO-GO without
   running stages 2–3.
3. **Stage 2 — canary (A1 only).** One race each on m22_white and stress
   (machine exclusive; A0 canary baselines as separate tagged races, per
   plan10 deviation-3). Clean ⇔ no outcome conflict, no panic,
   `W_first` ≤ 2× the A0 value on both, all four racers within wall
   caps. M2's R0-anchor ratios are collected here and in stage 3. Abort
   ⇒ NO-GO, family closes.
4. **Stage 3 — campaign.** Rounds per the table; per round one A0 race,
   one A1 race, interleaved with alternating order; stop-when-proven
   kill policy; per-racer wall/outcome/evals/censoring records; per-race
   facts stats (bytes read, checksum failures, accepted/skipped) and
   winner identity.
5. **Stage 4 — analysis.** Adapt `analyze_portfolio10.py` →
   `analyze_portfolio11.py`: per case A1 vs A0 medians (`evals_first`,
   `W_first`), full per-round distributions, winnerless flags, M2 R0
   ratios, facts-channel accounting, CPU cost (Σ racer core-seconds,
   A1 vs A0), filter pass rates; H-mech/H1–H4 evaluated mechanically.
   Results → `state/races11.json` + `README.md` provenance (drivers +
   parsed results committed; raw transcripts never).
6. **Final task — report.** `report11.md`: verdict against the
   pre-registered rule; attribution resolution (did the carrier remove
   the tax — M1/M2 numbers vs plan10's 1.19–1.25× / 1.38×); where the
   value came from or why it didn't; filter pass-rate analysis; facts
   accounting; deviations. Update backlog #12 and the
   `docs/plans/README.md` `parallel` row (any verdict is a close event
   for the cooperative-portfolio thread per the verdict rule).

## Scope boundary

- Product surface = the same two CLI options with re-implemented
  internals + the `FactExporter` hook + the reworked `src/facts/`. The
  `ProofEvent` protocol, the proof-tree layer, and `examples/portfolio`
  are untouched (the driver's CLI interface is unchanged — it spawns the
  same two flags). The sequential path is untouched (drift gate, stage 0).
- No `--race` product flag; no persistence (facts are per-race,
  ephemeral, never in the proofdb pipeline, TT snapshots, or cross-solve
  stores); no new CLI options; the admission filter is a fixed constant,
  not a flag or knob.
- Stays closed (non-goals unchanged): deterministic sibling parallelism,
  process-per-child option C, Čížek option D, cross-worker reuse of
  Draw/repetition-dependent facts, cross-solve solved-fact stores,
  backlog #7 (any-worker-can-finish SPDFPN — independent, untouched).
- One machine, 4 CPUs; races monopolize the container (plan7/8
  convention). N = 4 is real concurrency; nothing above is claimed.

## Deviation policy

Mid-campaign mechanism, racer-set, cap, admission-filter (`PATH_LEN_CAP`),
or read-cap changes are forbidden; there is **no escalation arm**. Stage-1
findings (e.g. a very low filter pass rate) are recorded, never tuned.
Anything else discovered goes into `report11.md` as a finding; any
follow-up is a new plan under the pinned gate — except that per the
verdict rule, a non-adopt outcome closes the cooperative-portfolio thread
rather than drafting plan12.
