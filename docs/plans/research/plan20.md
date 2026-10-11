# plan20 — Item #20: salted TT-keeping restarts (single-core) under the pinned gate

Initiative: `research` (re-opened 2026-10-09). Executes reopened backlog
item **#20** — restarts that keep the TT but reseed the bucket index
(`reexamination.md` §6 step 4), unblocked since plan12. This is the
single-core counterpart of the `parallel` plan7 process portfolio (separate
processes, no shared TT — measured 0.99× wall) and the complement of the
measured-out single-chunk lever (`reexamination.md` §4: one unbounded chunk
4.53× worse on stress; chunk restarts are load-bearing). Independent of #21
(in-context child results) and #22 (fringe).

Structural template: plan19 (the behavior-changing-hook rollout), with two
additions the eviction re-score did not need:

- **A Phase 0 before the rollout.** The mechanism is new infrastructure
  (TT rehash) with a measurable *retention cost* the other hooks lacked:
  a rehash re-places every valid entry under the new index function, and at
  high occupancy the normal insertion policy must evict overflow entries —
  the "keep the TT" premise is then quantitatively imperfect. Phase 0
  measures the loss rate and proves liveness before any rollout run.
- **Pairing by *starting* salt, with a deterministic in-run salt sequence.**
  The candidate's internal salts are derived from its starting salt; the
  pinned comparison rule applies unchanged.

## Motivation

The per-salt distributions are heavy-tailed (plan12, pinned evidence):
stress spans 249.5 M → 758 M → censored across salts (≥ 3×, mixed
censoring); plan19 added m22 tail evidence (5.30× paired ratio, one
budget-censored salt). A single sequential run rides one collision draw to
the end; the `parallel` plan7 portfolio showed that *resampling the draw*
cuts tails — but it paid 4.4× CPU for fresh cold TTs per process and was
closed NO-GO. Item #20 tests the unmeasured middle: **restart the
collision draw, keep the accumulated TT.**

The solver already restarts DF-PN from the root, TT-keeping, at every
work-chunk boundary of `bounded_search` (chunk 500 k, geometric ×2 growth;
`reset_search_state()` per chunk, TT persists — `new_generation()` is never
called by the search). What a chunk restart does *not* redraw is the
collision/eviction pattern: every chunk re-probes and re-stores under the
same bucket index, so a run that entered an unlucky collision basin stays
in it. The lever: at a chunk boundary, **reseed the bucket-index salt** and
continue. Solved and unsolved bounds are retained (modulo overflow
eviction, Phase 0); only which keys share a bucket — and therefore the
future eviction pattern — is redrawn.

What is *hypothesis*, not promise: mid-run, the accumulated TT state
conditions the continuation, so a reseeded run is **not** an independent
draw from the plan12 per-salt distributions — the heavy-tail evidence
motivates the lever, it does not predict its effect. Whether redrawn
collision luck (plus retention loss) cuts expected first-outcome work is
exactly what the measurement decides.

## Objective

1. **Phase 0** — implement the env-gated reseed hook; measure the retention
   loss per reseed; prove liveness and trajectory divergence on a mini
   probe; run the pre-registered kill gates.
2. **Phase 1** (only if Phase 0 passes) — fresh paired rollout over the
   frozen plan12 corpus: 22 cases × 6 starting salts × 3 arms
   (base, `rs1`, `rsg`) = **396 runs**.
3. Per-case verdicts per pinned `gate_methodology.md` v1.0 applied
   verbatim; the paired-ratio dispersion recorded as the comparison rule's
   **sixth** real-arm calibration data point.
4. **No product change remains** — the hook (TT `reseed` + chunk-loop
   call site + env gate) is added, measured, and reverted;
   `git diff --exit-code` clean before the report.
5. With this plan, **#20 closes** (adopt → sized hand-off; reject/defer →
   recorded); the remaining open items are #21 and #22.

## Pre-registered scope decisions

- **D1 — corpus, caps, salt set (inherited verbatim, frozen).** The 22
  plan12 fixtures and per-case caps (stress 2.5 B, m20_white 2 B, others
  1 B; `--timeout 600` secondary); starting salts {0, 1, 2, 3, 4, 5};
  salt 0 always runs. Corpus and caps as pinned in
  [`measurements/plan12/driver.py`](measurements/plan12/driver.py).
  Extensions pre-registered only (D5).
- **D2 — arms (fixed) and the hook.** Three arms, the 396-run shape
  (baseline arm first and identity-checked before the candidates):
  - **`base`** — shipped behavior, env unset.
  - **`rs1`** — reseed at **every** chunk boundary: after a `bounded_search`
    chunk call that used its full work cap and returned no decisive
    outcome, rehash the TT to salt `s + 1` and continue (natural
    exhaustion, decisive, and resource-cut exits end the loop; the final
    partial chunk does not reseed). Maximum redraw frequency, maximum
    retention exposure.
  - **`rsg`** — reseed only at boundaries where the chunk just used was
    **≥ 32 M** child evals (the "deep struggle" regime; early chunks run
    under the starting salt, so an `rsg` trajectory is bit-identical to
    the baseline until its first reseed — a built-in control and a D3(c3)
    anchor). Late-only redraw, minimal retention exposure.
  - **Hook spec.** (i) `TranspositionTable::reseed(new_salt)`: set the salt,
    then re-place every `valid` entry (native bucket order, deterministic)
    under the new index function through the normal insertion path
    (`insert_new`); overflow is resolved by the **incumbent eviction
    policy** — the rehash never invents values and never writes to a key's
    wrong bucket; generation fields are preserved. (ii) Call site: the
    `bounded_search` chunk loop (all `bounded_search` callers — `solve`,
    `search_depth`, `search_depth_with_prefix` — behave uniformly; the
    pre-phase runs before any chunk and is unaffected; history/killers and
    the repetition cache are untouched, so the redraw covers only the TT
    collision pattern). (iii) Salt sequence: `s_{k+1} = s_k + 1`
    (wrapping), deterministic in the starting salt. (iv) Env gate
    `ATOMIC_RESEED = rs1 | rsg`, unset ⇒ shipped behavior ⇒ bit-identical.
    (v) The `set_salt` doc contract ("set once, before the first
    probe/store/prefetch") is temporarily amended by the hook's `reseed`;
    on an adopt, `reseed` becomes the documented operation and the contract
    text is updated by the owner — never within this plan.
  - **Command line per run:**
    `atomic_solver --fen <FEN> --tt-size 128 --first-outcome --outcome-only
    --salt S --budget C --timeout 600` (`evals:` on stderr); the candidate
    arms additionally set `ATOMIC_RESEED`. No other flag changes.
- **D3 — soundness and fidelity invariants (hard gates; a violation halts).
  Liveness-first ordering per the report11-erratum lesson: every identity
  claim is recorded only after hook liveness is demonstrated (c0).**
  - (a) **Pristine pre-hook identity.** The unhooked build at HEAD
    reproduces the plan12 salt-0 values exactly (stress 249,480,478; m22
    14,156,269; m23 9,673,403; dec13 3,822,602; dec10 4,262,128; m20
    censored). **Partially pre-verified in this plan session (2026-10-11,
    read-only, clean tree at 087e72f, existing release build):** stress
    249,480,478 and m22 14,156,269 reproduce exactly (`git diff
    --exit-code` clean); the execution session re-runs the full (a) set on
    its own build.
  - (b) **Baseline-arm identity.** Every fresh `base` cell reproduces
    plan12's recorded per-salt value exactly, all 132 cells — the null is
    degenerate; any mismatch is an identity failure, not noise.
  - (c) **Hook fidelity anchors (checked before any rollout; a mismatch
    halts for hook debugging, not a verdict):**
    - (c0) **Liveness first.** An identity or fidelity claim is void unless
      the hook is proven live on the *same binary* that produces it:
      `ATOMIC_RESEED=rs1` at salt 0 on stress shows `reseed` events
      (> 0) and a cumulative `evals:` value different from the baseline —
      with outcomes preserved. An env-set run matching the baseline count
      with a positive reseed counter is the stale-binary/false-identity
      signature → HALT. Standing procedure (plan18 finding): after every
      lib change run `cargo build --release` **and** `cargo build --release
      --examples` before any anchor run.
    - (c1) **Env-unset quick-suite byte-identity.** The hook build at
      default settings, `benchmark --suite quick --json --first-outcome
      --timeout 3`, must equal the unhooked run bit-for-bit (archived total
      38,974,090, 59/59 cases).
    - (c2) **Reseed determinism.** A rerun of any (case, starting salt,
      arm) reproduces stdout and `evals:` bit-for-bit.
    - (c3) **Prefix identity.** `rsg` on stress salt 0 must be
      bit-identical to the baseline through the 16 M-chunk boundary
      (cumulative evals after the 6th chunk = 31,500,xxx class — exact
      equality required); divergence may begin only at the first reseed.
      `rs1`'s prefix identity holds through the first boundary (500 k).
    - (c4) **Reseed stats side-band.** Env `ATOMIC_RESEED_STATS=1` (default
      off, and **off during all rollout measurement runs**) prints one
      stderr line per reseed: `reseed: k=<i> salt=<s> live A→B solved
      C→D unsolved E→F`. The side-band must be demonstrated
      trajectory-neutral once (same run with and without, salt 0, `rs1`:
      identical stdout/evals); retention numbers come from side-band runs.
    - (c5) **No adopted-claim sub-study (recorded inapplicability).** The
      reseed asserts nothing: every entry it moves or drops carries values
      the search computed exactly. The soundness surface is (d).
  - (d) **Outcome invariants (substantive).** Zero decisive-outcome
    conflicts within any arm across starting salts, and every uncensored
    run of every arm matches its fixture `expected` outcome; a
    budget-censored `draw` is never read as a proven draw. The reseed only
    re-places entries — any outcome flip is a soundness defect (halt and
    diagnose, never noise). This is the family's whole soundness surface.
  - (e) **Cyclic-rook invariant per arm:** `8/8/8/8/2k5/8/8/4KR2 w - - 0 1`
    must never claim `Win` under either candidate arm, run with
    `--no-preflight` (3 men — the pre-phase closure would claim Draw first
    and make the spot-check vacuous). Post-revert sanity:
    `cargo test --release --test test_repetition -- --include-ignored`
    green.
  - (f) **Revert:** hook deleted; `git diff --exit-code` clean; release
    build re-verified; (a)/(c1) anchors re-run exact after the revert.
- **D4 — verdict machinery (pinned v1.0, applied verbatim).** Per case:
  per-salt paired ratios `arm_s / base_s` over starting-salt pairs where
  **both** sides are uncensored (the candidate's evals are cumulative
  across its chunks — the same `evals:` quantity the baseline reports);
  censoring pattern per salt; baseline basin count from plan12. Verdicts:
  censored-win / censored-loss / win (median ≤ 0.9 ∧ ≥ 75% of ratios < 1)
  / regression (median ≥ 1.1 ∧ ≥ 75% > 1) / unclear. Lever verdict:
  **adopt** = ≥ 1 censored-win or win on a hard-class case (stress,
  m20_white, m22_white, m23_white) ∧ no regression / censored-loss
  anywhere; **reject** = any regression or censored-loss; otherwise
  **defer**. Low-diversity baselines (basin count < 3) cannot solely
  support an adopt. Under-budget rule: > 3 of 6 censored salts on a case
  for either arm ⇒ case unresolved, contributes nothing. Per-arm verdicts;
  a combined #20 verdict is recorded only if both arms agree in direction,
  otherwise the per-arm split is the record.
- **D5 — defer extension rule (pre-registered).** A defer is re-judged only
  after adding starting salts **{6, 7}** on the affected cases (both
  arms + base), fresh runs; the pooled verdict is recomputed. One
  extension; a second defer stands. No third arm exists; a defer on
  mechanism grounds stands as recorded.
- **D6 — hand-off rule (pre-registered).** An **adopt** becomes a sized
  proposal to `dfpn` (reopen under trigger (a), the plan16 pattern: a new
  measured search-semantics lever), containing the per-case paired tables,
  the hook specification as the implementation seed (product
  `TranspositionTable::reseed` + a `--reseed <mode>` CLI option, env hook
  deleted, contract text updated), the TT unit tests to extend
  (`src/search/tt/tests.rs` rehash fixtures), and the landing contract
  (`cargo fmt`/`clippy`, `make test`, `make test-full`). If the owner
  prefers, `lean` is the recorded alternative (node-count lever, wall-time
  relevant) — an owner decision, not made here. A **reject** is recorded
  with its tables; #20 closes and the `reexamination.md` §6 step-4 bullet
  gains the gate-grade pointer.
- **D7 — dispersion prediction (pinned rule verbatim, falsifiable).** The
  reseed fires only at chunk boundaries, and its effect requires bucket
  pressure: plan7 Phase-0 occupancy (salt 0) stress 95.4%, m22 4.6%, dec13
  0.25%, dec10 0.09% full. **Pre-registered predictions:** sparse-effect
  shape (plan17/plan19-like): exact-null mass ≥ **40%**, frac-outside
  [0.9, 1.1] ≈ **20–50%**; effects concentrated in the hard class and any
  dec case whose run reaches high occupancy; `rs1` near-inert on m22-class
  occupancy (its m22 salt-0 run passes 4 boundaries — all low-chunk), and
  `rsg` never fires on cases that never reach a 32 M chunk at their
  starting salt (m22 salt-0: 0 fires — a built-in null control). Re-seed
  counts measured today (salt 0): stress 8 boundaries (`rs1` = 8 reseeds,
  `rsg` = 2), m22 4 boundaries (`rs1` = 4, `rsg` = 0); counts vary by
  salt. The retention loss (D3(c4) data) is pre-registered as a mechanism
  *cost*: it works against both arms at high occupancy, and a
  retention-driven m22-class loss would appear as null, not regression.
  No methodology revision is proposed from within this plan.

## Phase 0 (kill gates before any rollout)

Tooling: `measurements/plan20/` — `driver.py`/`parse.py`/`verdicts.py`
adapted from plan19; the hook implemented in a working tree and validated
**in this order** before any rollout run: (c0) liveness → (c1) env-unset
quick identity → (c2) determinism → (c3) prefix identity → (c4)
side-band neutrality → (d)/(e) invariants.

Probes (canonical caps, ≤ 30 min total):

1. **Retention probe** (side-band on): stress salt 0 `rs1`, stress salt 1
   `rs1`, stress salt 0 `rsg`, m22 salt 0 `rs1`. Records the per-reseed
   retention table (live/solved/unsolved before→after).
2. **Effect probe** (side-band off): stress salts {0, 1, 2} × {`rs1`,
   `rsg`} (6 runs), m22 salt 0 `rs1` (1 run).

Kill gates (any firing ⇒ no Phase 1):

- **K1 — retention:** on the stress-class runs, any reseed with live-entry
  retention < 85% or solved-entry retention < 90% ⇒ **PIVOT**: the
  "TT-keeping" premise is materially violated at the occupancies where the
  lever is supposed to act; the loss table is recorded as the #20 finding
  and the item closes at Phase 0 (a mechanism that drops ~15%+ of its
  working set per boundary is a partial-clear restart, not this item).
- **K2 — liveness/effect:** no (case, salt, arm) probe run diverges from
  its baseline cumulative evals (with outcomes preserved) ⇒ the mechanism
  is inert ⇒ HALT and debug before any verdict thinking.
- **K3 — soundness spot:** any uncensored probe run not `win`, or any
  stdout/stderr parse failure ⇒ HALT.

## Method

- **Phase 0** as above; verdicts: PASS → Phase 1; PIVOT (K1) / HALT
  (K2, K3, or any D3 violation) → Phase 3 directly with the finding.
- **Phase 1 — rollout.** 396 runs, ≤ 3 concurrent processes; baseline arm
  first and identity-checked (D3(a)/(b)) before the candidates; transcripts
  to a non-committed RAW dir; parse to `state/runs.json` +
  `state/summary.json`; run the D3(d) checks.
- **Phase 2 — verdicts.** `verdicts.py` → `state/verdicts.json` (per-arm
  D4 tables + paired-ratio dispersion summary + reseed-count profile).
- **Phase 3 — report.** D6 hand-off if any; revert and D3(f); write
  `report20.md` (final task); refresh the #20 row in `initiative.md`
  (item closes) and the history entry; update the
  `docs/plans/README.md` research row if the index summary changes.

## Gates

- **PASS Phase 0 → adopt / reject / defer** per D4 on the Phase-1 rollout.
- **PIVOT (K1)** → #20 closes at Phase 0 with the retention table as the
  no-go record; `reexamination.md` §6 step 4 annotated.
- **HALT** → any D3 invariant violated or K2/K3 fired: soundness or
  fidelity investigation, not a measurement. Fidelity mismatches are
  debugged before any candidate number is read.

## Out of scope

- Process-level portfolios and any multi-processing (`parallel` is closed;
  this item is the single-core counterpart). Chunk-schedule re-tuning
  (`chunk_growth` territory; the single-unbounded-chunk point is already
  measured out in `reexamination.md` §4 — this plan keeps the shipped
  chunk schedule and changes only the salt at boundaries). TT capacity
  (`lean` #16 territory). Snapshot format or entry layout changes (the
  rehash re-places entries; the snapshot contract is untouched — a
  reseeded run dumps under its final salt via `entries()`, keys are raw).
- #21 in-context child results, #22 fringe. Productization (the D6 landing
  contract is the owner's, post-adopt). Proof-tree layer, `ProofEvent`,
  optimizer interface; methodology revisions.

## Budget envelope

Phase 0: hook build + anchor chain ≈ 30 min, 10 probe runs ≈ 20 min
(stress-class runs measure minutes at the canonical caps; today's salt-0
stress run = 29 s, m22 = 1.8 s). Phase 1: 396 runs at ≤ 3 concurrent
processes — the baseline arm re-runs plan12's ~1.2 h grid; the candidate
arms add rehash wall-time only in the metric sense (rehash is outside
`child_evals`; its wall cost is proportional to table size, ~tens of ms
per reseed at 128 MB, negligible against chunk lengths) plus unknown eval
inflation, bounded by the caps (censoring as in plan17/plan19). Total
≈ 4–6 h CPU; no expected wall explosion.

## Final task

Write `report20.md` in this directory: deliverables, the Phase-0 retention
table and kill-gate outcome (if Phase 0 ran), per-case per-arm verdict
tables with basin counts and censoring patterns, fidelity-anchor results,
reseed-count profile, paired-ratio dispersion summary with the D7
prediction's fate, problems encountered, missing tests, next steps
(kickoff prompt: #21 in-context child results or #22 fringe — the last two
open reopened-backlog items — or, on an adopt, the `dfpn` reopen
execution). Refresh the #20 row in `initiative.md` (item closes), add the
history entry, and update the `docs/plans/README.md` research row if the
index summary changes.
