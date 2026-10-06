# Plan 10: Oracle-path ε — condition the DF-PN threshold padding on a proven principal line

Executes **backlog item #8** (opened with this plan). One lever, one session,
temporary env-gated spike, no product code lands.

## Background and motivation

The solver's 1+ε trick (Pawlewicz & Lew 2007; `dfpn/research_epsilon.md`)
pads the second-best child's bound at the DF-PN threshold sites with one
global ε (`DEFAULT_EPSILON = 0.125`, `src/search/dfpn/mod.rs:36`):

- OR site (`src/search/dfpn/core.rs:274`):
  `new_th_pn = min(th_pn, epsilon_ceil(second_pn))`
- AND site (`core.rs:282`):
  `new_th_dn = min(th_dn, epsilon_ceil(second_dn))`

The ε surface is closed for every *trajectory-derived* conditioning signal
(`research` plan4: no constant, no path schedule, no regional structure —
trajectory chaos; `research` plan8: no node-local signal converts — arms
failed catastrophically). Plan8's admissibility note leaves one class
untested: **exogenous** conditioning — ε as a function of information the
search does not generate itself.

This plan tests exactly that: ε conditioned on an **oracle principal
line** — the proven first-outcome PV of the position itself, captured from
an unguided baseline run and replayed into the spike build. At OR threshold
sites whose position lies on the oracle line, the second-best child's bound
is padded more aggressively (arms below), so the frame stays committed to
the known-winning child instead of cutting back to siblings after every
threshold hit.

Two measured facts motivate the lever:

1. **`conversion` report9 (ordering-guidance seed, closed no-go)** measured
   that even with the decisive root move promoted to descent rank 0, **80.9%
   of stress-FO child evals sit under sibling root moves**, and named the
   mechanism: *"the decisive child's DF-PN subtree search is threshold-cut
   until sibling refutations have grown the thresholds and populated the
   TT/repetition cache."* Ordering guidance cannot touch threshold pacing;
   this lever is precisely the pacing dial report9 left untested.
2. **`research` plan8's separation study** found the only node-local signal
   that separates rescued from dead-end cut frames is the composite bucket
   **OR ∧ depth 1–4** (2.42× lift at ε=0.375) — i.e. the exploitable-looking
   population is shallow OR frames, exactly where a known-correct oracle
   line passes. Plan8 could not exploit it because conditioning on it moves
   the trajectory that generated it; an exogenous oracle does not have that
   feedback loop.

**Two-question decomposition** (agreed 2026-10-06 analysis session):

- **Question A (this plan):** does path-ε conditioning help with a
  *perfect* oracle (the solver's own proven PV)? Mechanism question,
  cheaply testable, deterministic.
- **Question B (conditional plan11, not drafted here):** is an externally
  produced PV (Fairy-Stockfish MultiPV on idle cores, or a proofdb/book
  line) a faithful enough proxy? Only draftable if A lands GO — a negative
  on A makes B moot; a B-first test would conflate mechanism failure with
  guide-quality failure (the confound report9's bar (c) exists to exclude).

Note on "guidance must be free": in this spike the oracle PV is free (the
baseline run that produces it is also the measurement baseline). The
free-guidance *deployment* story (parallel engine, proofdb seeding) belongs
to question B and is out of scope here.

## Soundness contract

- ε only shapes thresholds; thresholds are never stored in TT state
  (verified in research plan8 §Soundness), so per-site ε creates no
  GHI/reuse hazard. Outcomes remain full proofs; nothing decisive is ever
  claimed from the guide.
- The `inf` arm pads to the **parent threshold** (`new_th_pn = th_pn`),
  which is finite — commitment is bounded by the frame's own budget, never
  unbounded.
- The guide affects only *where the search commits*, never what it may
  claim. A wrong guide can only cost work (bounded by thresholds), never
  soundness.
- `child_eval_budget` / `ExitReason::BudgetExhausted` semantics untouched;
  repetition handling (`best_move_repeats_path`, first-player-loss
  shortcut, plan9 cache) untouched.
- With the env unset, `src/` behavior is bit-identical (spike hygiene).

## Spike design

### Step 0 — oracle PV capture (no code changes, existing binary)

For each case below, run the current release binary:

```
cargo run --release -- --fen <FEN> --first-outcome --timeout 300
```

Parse the `pv: <UCI moves>` line from stdout (the first decisive line,
`pv_status: FirstOutcome`). Validate each line with
`examples/verify_ppv` (the plan2 precedent for verifying a UCI move list
against a FEN; if its acceptance semantics differ from a principal-line
check, fall back to replaying the line with `Position::from_fen` +
`do_move` and asserting the final position's outcome is `Win` — this
assertion is mandatory either way). Store under
`measurements/plan10/oracle_pv/<case>.txt` (one UCI path per file).

Cases (report9's exact set, for comparability; FO baselines at ε=0.125,
128 MB TT, post-plan9 binary — child_evals are deterministic and must
reproduce bit-for-bit on this host):

| case | FEN | baseline FO child_evals |
|---|---|---:|
| stress (gate object) | `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21` | 249,480,478 |
| m22_white | `4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22` | 14,156,269 |
| dec13 | `r1bq1k1r/ppN4p/n1p1p3/3p1n1P/1b1P2P1/2P5/PP6/RNBQKB1R w KQ - 1 14` | 3,822,602 |
| dec01 | `r5r1/5N1k/2p2p2/pp1p3p/3Pp3/2P1P3/P7/2bQ1R1K w - - 0 30` | 5,713,706 |
| dec10 | `3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41` | 4,262,128 |

If any baseline count does not reproduce exactly, stop and investigate
before any arm runs (host/config drift must be resolved first; the counts
are host-independent by design).

### Step 1 — env-gated spike (`CONV10_*`), temporary, reverted after measuring

1. **Guide loading.** `CONV10_GUIDE=<path>`: read the UCI path, replay it
   from the root FEN with the solver's own `Position`, and collect
   `pos.hash()` at every position including the root into a path-key set.
   (Zobrist keys include the halfmove clock; replaying the exact PV line
   reproduces the search-visited keys exactly along that line.)
2. **Conditioned threshold site.** At the OR site only (`core.rs:274`):
   if `pos.hash()` is in the path-key set, replace the global-ε padding:
   - `CONV10_PAD=0.5` / `1.0`: `epsilon_ceil` computed with the pad factor
     (`ceil(x · (1+pad))`, precomputed as an exact integer fraction like
     `fraction_from_f64`);
   - `CONV10_PAD=inf`: skip the `min` clamp — `new_th_pn = th_pn`
     (pad-to-parent-threshold).
   The AND site (core.rs:282) is untouched in round 1 (single lever;
   AND-side conditioning is `research` plan8's measured-closed territory).
3. **Validation arm.** `CONV10_PAD=0.125` (pad == global base): thresholds
   are mathematically identical to baseline — the run must be
   **bit-identical** to the unguided baseline on every case. This isolates
   guide-loading bugs from arm effects.
4. **Counters.** `CONV10_STATS=1` (composable with a PAD arm, and usable
   alone with no PAD set = pure diagnostics, zero behavior change): stderr
   totals of (a) on-path OR-site evaluations, (b) clamp engagements there
   (`ε(second) ≥ th`, padding would not engage), (c) cut-backs at on-path
   OR frames (site re-evaluations after a child return), (d) own child
   evals at on-path OR frames (frame-local via the existing
   `child_evals_start`), (e) total child evals (partition sanity: the sum
   of the frame-local accounting must equal the run total, delta 0 —
   lean plan9's attribution check).
5. **child_evals reporting.** The stats line is the per-run `child_evals`
   source (the CLI stdout prints only nodes); without `CONV10_STATS` the
   baseline cross-check uses `benchmark --json` or the counters themselves.

### Step 2 — arm matrix (all FO, ε=0.125 base, 128 MB TT, timeout 300 s)

| arm | env | purpose |
|---|---|---|
| BASE | none | baseline reproduction (must equal the Step-0 counts) |
| HYGIENE | GUIDE + PAD=0.125 | bit-identical to BASE (instrumentation check) |
| P050 | GUIDE + PAD=0.5 | moderate path padding |
| P100 | GUIDE + PAD=1.0 | aggressive path padding |
| PINF | GUIDE + PAD=inf | pad-to-parent-threshold (tests report9's load-bearing claim directly) |
| G050 | `--epsilon 0.5` | global control: is path-conditioning better than the same ε everywhere? |
| G100 | `--epsilon 1.0` | global control for P100 |

7 arms × 5 cases ≈ 35 runs (dominated by stress at ~1–5 min FO; PINF/G
arms may time out — record timeouts as censored, plan8 `AND_TIGHT`
convention). Counters collected on every arm.

### Step 3 — pre-registered gates

**Hygiene (must all pass before arms are read):**
- H1: env unset ⇒ quick suite bit-identical (`benchmark --suite quick
  --json --first-outcome`, 59/59 identical `child_evals`) and stress FO
  stdout byte-identical to the pre-spike binary.
- H2: HYGIENE arm bit-identical to BASE on all five cases.
- H3: BASE reproduces the Step-0 counts exactly on all five cases.

**Soundness:**
- S1: every finishing run reports outcome `win` matching the case
  expectation; zero `wrong` anywhere.
- S2: `cargo test --release --test test_repetition -- --include-ignored`
  green on the spike build (env unset).
- S3: stress (the repetition-heavy case) must not flip outcome under any
  arm that finishes.

**Decision (conjunctive):**
- **GO** if the best pad arm achieves ≥10% reduction on stress FO
  child_evals (≤ 224,532,430) **and** every other case is within +5% of
  its BASE count **and** it beats the equal-value global control (P100 vs
  G100, P050 vs G050) on stress by ≥5 percentage points (marginal-value
  clause: path machinery must buy something the closed global constant
  does not).
- **PARTIAL** if stress improves ≥10% but a control regresses >5%: record
  the mechanism reading (phase-0 counters); refinement shapes (root-only
  padding, depth-capped padding) are report material and a possible
  plan11 re-arm — explicitly not silent scope expansion.
- **NO-GO** otherwise. A NO-GO closes the *exogenous-conditioning* leg of
  the ε surface (fifth closure leg) and plan11 (question B) is not
  draftable.

## Interpretation guidance (phase-0 diagnostics, not gates)

- If PINF ≈ BASE on stress: report9's "sibling work is load-bearing
  (TT/repetition-cache warming)" claim is confirmed and the pacing is not
  overhead — the strongest possible negative, worth stating plainly.
- If cut-back counts at on-path OR frames are tiny or clamps dominate
  (research plan8 measured the parent clamp binding at 32.1% of OR cuts
  globally), the harvestable surface is structurally thin and that is the
  recorded mechanism regardless of arm outcomes.
- The trajectory caveat applies to any positive: plan8's separated bucket
  existed at ε=0.375 and collapsed at ε=0.125; a GO here must be read as
  "works with this oracle on these cases", not as a general constant.

## Out of scope

- Question B in any form: Fairy-Stockfish, MultiPV, concurrent guidance
  processes, proofdb/book seeding (conditional plan11, only on GO).
- AND-site conditioning (research plan8's closed territory).
- Default-mode PV refinement (the ~45% refinement tail is a separate,
  already-recorded lever).
- Any product code change: the spike is reverted after measuring; if GO,
  a follow-up implementation plan decides the product surface (e.g. a
  deterministic `--guide-pv` flag), soundness contract above carried over.

## Deliverables

1. Spike implementation (env-gated, `src/` diff reverted after measuring;
   post-revert stress FO stdout byte-identical to pre-spike).
2. Raw outputs under `measurements/plan10/` (oracle PVs, per-arm counts,
   stats dumps).
3. `report10.md`: Step-0 reproduction table, phase-0 diagnostics, full arm
   matrix, gate verdict, and an explicit statement whether question-B
   (plan11) drafting is justified.
4. Backlog #8 row updated in `initiative.md` (this plan's outcome); fast
   gate (`make test`) green on the reverted tree.

Per repo convention, the plan ends with writing `report10.md`.
