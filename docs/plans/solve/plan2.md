# Plan 2: Solved-Set Frontier Push (SSFP) — substrate gate, mechanism spec, pilot loop

Initiative: `solve`. Implements the pivot decision taken after
`report1.md` §10 (2026-09-22 session): the initiative pivots from the
top-down ply-frontier campaign to a **demand-driven solved-set frontier
push** with the **proof-cost gradient**. Product-surface changes are
explicitly in scope for this plan (user sign-off 2026-09-22).

## 1. Design rationale (session record, condensed)

Three `report1.md` facts constrain the design:

1. Work is bimodal: tactical shots are cheap at any ply; quiet defense
   systems censor at a flat ~2×10⁷ nodes / 120 s regardless of depth.
   The hardness axis is distance-to-tactical-resolution, not ply.
2. The top-down leaf population is centered at ~19 men (median); ≥95%
   coverage needs 23 men. No generable EGTB depth reaches it.
3. Self-play ladders die from steering (PV requires proof), not depth.

**Why enumerative bottom-up is rejected.** The classic retrograde
reading — solve all of layer k (positions with ≤k men), decide layer
k+1 from it, push k upward — dies by counting. Calibrated against
chess-class growth (×16 per added man): 3 men ≈ 5×10⁶ entries (built),
4 men ≈ 4×10⁸, 5 men ≈ 7×10⁹, 6 men ≈ 4×10¹¹, 7 men ≈ 4×10¹³ — while
the proof population lives at 19–23 men. The frontier would need ~14
more ×16 steps to touch where proofs actually live. Enumerative
layer-push cannot meet top-down demand; any viable push must invert the
demand direction: prove *the needed positions*, not generate *the
layers*.

**Framing.** With the halfmove-clock augmentation the atomic game graph
is a finite DAG (a cycle would need a clock reset; resets — captures
and pawn moves — are irreversible state changes). Top-down DFPN and
bottom-up retrograde are two evaluation orders plus memory policies
over the same DAG. The design question is: *which evaluation order
grows a verified solved region fastest toward the startpos value, and
what gets stored.*

## 2. Mechanism spec (normative for this initiative's campaign shape)

**SSFP** maintains a persistent, disk-backed **solved set S**: exact
Zobrist keys (position + halfmove clock — the TT's own key semantics)
with verified WDL values and artifact pointers. The campaign is a
priority queue of solve runs:

1. **Solve a target** with S preloaded as proven anchors (proofs cut at
   S entries at O(1), like TT base entries).
2. **On success**, the run deposits into S; **on censor**, the run
   deposits its **frontier** (cutoff candidates) into the target queue.
   Targets breed sub-targets; the queue drains in expected-cost order.
   The startpos run is not special — it is the last (most expensive) job.
3. **Verified values per wall-hour** is the campaign's progress rate;
   |S| and its men-count histogram are the progress meters.

### Soundness classes (constraint 1 instantiation)

- **S (verified class)**: keys/values extracted only from
  replay-validated proof trees (`reconstruct_pt --validate: ok`). These
  compose into the master artifact; the final startpos proof tree's
  leaves are terminals, table anchors, or S references, each
  independently re-verifiable offline.
- **Provisional class**: solved TT-snapshot records not covered by a
  validated tree (search-semantics proven, not independently verified).
  Usable as in-campaign anchors for speed; **excluded from artifact
  composition** unless later covered by a validated tree.
- **Repetition discipline**: inherited from the TT's snapshot contract —
  solved records are path-independent (repetition-dependent results are
  never stored; first-player-loss GHI shortcut). An entry proven with a
  repetition-qualified edge cannot enter S. Keys are exact (clock
  included), so anchor reuse is exact-key only — sound by construction;
  clock-derivation table semantics is a later extension, not v0.

### Gradient (user decision 2026-09-22)

**Proof-cost gradient**: prefer cheapest-to-prove frontier targets
(tactical-shot class) first. Disproofs are first-class (a disproved
target is a full WDL value too). The bet: cheap proofs deposit values in
regions that later quiet-position proofs traverse. This bet is
substrate-dependent — it is exactly what gate **M1** measures before any
engineering (§4).

### v0 component grounding (verified 2026-09-22)

- **Frontier extraction**: snapshot v1 unsolved records carry
  `pn`/`dn`/`work`/`best_move` but **no FEN** (key only), so raw
  post-processing cannot produce runnable targets. v0 uses a pre-exit
  **frontier walk** in-process: after the search exits, replay from the
  root through the TT (best moves / highest-work children), collecting
  the top-K unsolved nodes with FEN + pn/dn + work → `--frontier-dump`.
- **Anchor preload**: the snapshot's solved section is imported as base
  entries → `--tt-load-path` (the read path exists for `reconstruct_pt`
  seeding; the search CLI gains the value-preserving import mode).
- **S store v0**: merged snapshot solved-sections + validated-tree
  extractions, deduped by key, as a plain binary store produced by a
  campaign-side merge tool (no server, no database).
- **Deposit pipeline**: target solve → TT snapshot →
  `reconstruct_pt --validate` → validated-tree node keys/values → S
  (verified class); snapshot solved-section remainder → provisional
  class.

## 3. Product-surface changes (in scope, gated on M1 GO)

Minimal, individually documented; none change search semantics or
defaults:

1. `Search` + CLI: `--tt-load-path <FILE>` — import a snapshot's solved
   section as base entries before the search. Preflight is
   detector-gated (≤3-men roots) and does not interact with preloaded
   anchors at campaign root sizes; note the interaction contract in the
   module header regardless.
2. Search + CLI: `--frontier-dump <FILE>` — the pre-exit frontier walk
   (§2) writing K candidates (default K tuned in the pilot; the CLI
   flag takes the count) with FEN, pn, dn, work.
3. Campaign tooling under `examples/` (or a `campaign/` dir if sizing
   demands): snapshot-merge (S store), frontier-queue runner (v0 =
   Python driver of the CLI; no persistent workers yet).
4. `AGENTS.md` CLI option list + module headers updated with the two
   new flags (product contract: unknown options still exit with an
   error; the new ones are first-class).

## 4. Pre-registered gates (fixed before any run — do not tune after seeing data)

### M1 — transposition substrate (decides §3 GO/NO-GO)

Question: do quiet defense systems' searches traverse the regions that
other systems' cheap proofs resolved?

- **Data**: regenerated TT snapshots of the 48 censored plan1 cost runs
  (~107 min sequential, commands replayed from
  `measurements/plan1/README.md` + `state/cost_*.json`) and the
  validated proof trees `reconstructed_d4d5_p30.bin` / `_p32.bin`
  (already committed).
- **Systems** = the seed lines {1.e4, 1.d4, 1.Nf3, 1.e4 e5, 1.e4 c5,
  1.d4 d5, 1.Nf3 d5}. **Cross-system pair** = two snapshots from
  different seeds, both with ≥1000 solved records (degenerate
  tactical-only sections excluded).
- **Metric**: directed value share
  `vshare(A→B) = |A.solved ∩ (B.solved ∪ B.unsolved)| / max(1, |A.solved|)`.
  M1 = median over all cross-system directed pairs.
- **Secondary (non-gating)**: share of p30/p32 proof-tree node keys
  covered by each quiet snapshot's solved section (does the tactical
  region anchor quiet searches).
- **Gate**: M1 ≥ 5% → **GO** (§3 lands; pilot runs). M1 < 1% →
  **NO-GO** (no substrate; gradient (b) dead as campaign substrate — the
  initiative rethinks again: close with the artifact or sharpness-first
  rescoping). 1% ≤ M1 < 5% → judgment call; the pilot loop may run with
  the caveat recorded, and its pre-registered metric decides.

### Pilot loop (runs only on M1 GO; metrics fixed now)

Root: d4d5 p2 (`rnbqkbnr/ppp1pppp/8/3p4/3P4/8/PPP1PPPP/RNBQKBNR w KQkq
d6 0 2`, censored at 23.4M nodes / 120 s in plan1).

- Budget B = 2 h wall per arm, sequential, defaults everywhere.
- **Arm A (baseline)**: one `--timeout 7200` solve of the root.
- **Arm B (push)**: queue iterations summing to ≤ B: root solve →
  frontier dump → select cheapest targets (expected-cost order: pn/dn
  heuristic, work as tiebreak; cheapest-first) → solve isolated →
  validate → deposit → repeat.
- **Primary metric**: validated-value yield = verified-class node
  count per wall-hour. Expected asymmetric (Arm A deposits nothing
  unless it completes) — reported as such, not celebrated alone.
- **Substrate metric (the honest one)**: re-solve the pilot root with
  the accumulated S preloaded vs. fresh TT, equal wall (120 s):
  node-count delta and outcome delta. S only matters if it moves this.
- **Gate**: substrate metric ≥ 2× node reduction (or an outcome change)
  → campaign prototype GO; < 1× → the mechanism is measured empty at
  pilot scale; initiative rethinks again. Between: judgment call with
  the numbers on record.

## 5. Tasks

1. Harness under `measurements/plan2/` (Python stdlib, black-box CLI
   driver + snapshot/proof-tree binary readers; README command table
   following the plan1 layout).
2. Regenerate the 48 censored-run snapshots (replayed plan1 commands);
   compute M1 + secondary share; apply the gate. No product changes
   before this verdict.
3. On GO: implement §3 (store merge tool, `--tt-load-path`,
   `--frontier-dump`, queue runner) with unit tests
   (import round-trip, key-exactness, provenance class separation,
   frontier-walk FEN validity); `make test` green; AGENTS.md updated.
4. Run the pilot loop (§4); record all raw captures.
5. Write `report2.md`: gate verdicts, yield + substrate numbers, |S|
   growth, problems, missing tests, next steps (campaign prototype vs.
   rethink).

## 6. Non-goals

- No enumerative EGTB layer generation (dead by §1's counting; the
  `egtb` thread stays closed except as SSFP's eventual low-men anchor
  class, which needs DTZ-class clock-aware semantics — explicitly not
  v0).
- No distributed workers, no persistent worker processes, no master
  process (v0 queue runner is sequential; the Čížek job-level shape is
  the successor prototype, not this plan).
- No change to search algorithms, ordering, budgets, or defaults; no
  benchmark/drift-gate impact (the new flags are inert unless used).
- No claim about the startpos value.

Per repo convention, the final task of this plan is writing its
`report2.md` in this directory.
