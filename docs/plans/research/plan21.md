# plan21 — Item #21: in-context child results (dec10 class) under the pinned gate

Initiative: `research` (re-opened 2026-10-09). Executes reopened backlog
item **#21** — in-context child results for the dec10 class
(`reexamination.md` §6 step 5), the last open item of substance (#22
fringe is explicitly not taken here). Independent of the salt channel's
*existence* (dec10 is the one control whose baseline sits mid-distribution,
`reexamination.md` §1), but reported under the pinned gate like every
behavior-changing lever plan since plan16.

Structural template: plan19 (the single-mechanism 2-arm rollout), with a
Phase 0 in the plan20 style because the mechanism is new search machinery
with its own fidelity surface — a repetition-cache probe at a site
(`evaluate_child`) that has never read the cache before.

## Motivation

The first-player-loss shortcut (`core.rs` frame exit) stores a
repetition-dependent proven `Draw` as an *unsolved* `(1, 1)` TT entry
(`outcome = None`, `remaining_depth = u32::MAX`); the actual value goes to
the per-search repetition cache (plan9), keyed `(tt_key, context_hash)`.
The repetition cache is probed at exactly one site — `dfpn` frame entry,
after the TT resolved-result check — so the *value* survives within a run
under the identical ancestor set, but the parent's own child-re-evaluation
never sees it:

- `evaluate_child` (`children.rs`) probes only the TT. For a child whose
  proof was repetition-dependent, the entry is the stale `(1, 1)`
  unsolved record (the depth-guard and the `u32::MAX` degeneracy guard both
  exclude it from bounds reuse), so the parent re-prices the child at
  `(1, 1)` even though the parent's own descendant just proved it `Draw`
  under the *same* ancestor context (the path stack is unchanged across the
  parent's loop).
- The reexamination's C8 counter measured this mass: **38.1% of dec10's
  child evals are spent in frames after a stale child re-eval** (8,898
  events / 1.62 M descendant evals of 4,262,128), versus stress 1.7%,
  dec13 0.7%, m22 ~0% (`reexamination.md` §4,
  `measurements/reexam/state/derived.json`). dec10 is the one gate case
  where the first-player-loss surface dominates.

The mispricing is churn, not waste on draw re-proofs (the plan9 cache
already short-circuits re-descents): a child that the parent refuted one
iteration ago looks *cheap* again (`(1, 1)`), is re-selected, recursed
(the child's `dfpn` hits its own rep-cache probe and returns instantly),
re-evaluated stale, and the cycle repeats until threshold exhaustion. The
lever gives the parent the in-context value it already owns.

## Mechanism (pre-registered design)

**Rep-cache probe at `evaluate_child`'s unsolved site.** In the final
(non-terminal, non-repetition, rule50-unexpired) branch of
`evaluate_child`, after the TT resolved-result check and the terminal
classification, *before* the unsolved-bounds reuse / `(1, 1)` fallback:
probe `self.repetition_cache` at `(child_key, child_context)`. On a hit,
return `ChildInfo { outcome: Some(Draw), (pn, dn) = Draw.pn_dn_for(child_is_or)`
= `(INF, 0)`, `depth = cached_depth`, `repetition_seen: true,
explored: false }`.

- **Sound by construction, per the plan9 contract** (`repetition_cache.rs`
  module header): only proven repetition-dependent `Draw`s ever enter the
  cache; a hit requires the exact `(tt_key, context)` pair, where the
  context is the order-independent hash of the ancestor repetition keys —
  at `evaluate_child` time this is exactly the context the child's own
  `dfpn` frame computes (the path stack is identical: the parent pushed
  before the loop, the child pushes after its frame-entry checks). The
  reuse is a semantic identity: precisely the value a fresh re-descent
  would recompute. A hit consumes no child-eval budget beyond the
  `evaluate_child` call it answers (same accounting as the existing
  TT-resolved path).
- **`repetition_seen: true` is load-bearing.** `select_from_children`
  aggregates `repetition_seen` only for `Draw` outcomes (`selection.rs`);
  the flag makes a parent `Draw` proven *through* this child itself
  repetition-dependent, so the parent's own first-player-loss suppression
  stays correct.
- **The explicitly rejected alternative:** memoizing "whatever the child
  recursion returned" frame-locally. A `max_depth`-capped leaf returns
  `Outcome::Draw` *without a proof* (`core.rs` leaf store), so an
  outcome-memo converts an unproven frontier into a proven draw — a
  false-value defect. The rep-cache route is immune because only proven
  rep-draws enter the cache. The frame-local variant is recorded as a
  fallback only if Phase 0 shows the cache route misses the measured
  stale mass.
- **Ordering mirrors `dfpn` exactly:** TT resolved → terminal → rep-cache
  probe → unsolved bounds → `(1, 1)`. The hit is *not* depth-guarded,
  matching `dfpn`'s rep-cache probe. The terminal classification keeps its
  place ahead of the probe (a proven `Draw` never overrides a decisive or
  terminal value — it can't: only draws enter the cache, but the terminal
  value is cheaper to trust and the order must match `dfpn`'s).
- **Coverage is a superset of the C8 class.** The probe fires at both
  re-evaluation surfaces — the loop's `children[idx]` re-eval (the C8
  site) *and* `evaluate_all_children`'s first-sweep table build for
  children proven rep-draws under this context in *earlier* frame visits
  or chunks. Both production call sites funnel through `evaluate_child`
  (the lean-plan12 prefetch pre-pass reads no values), so one probe site
  covers all reads.
- **Child context derivation (O(1), no new `Search` state).** In `dfpn`,
  `context_hash` (the frame's pre-push ancestor hash) and `rep_key` (the
  frame's own key) are loop-invariant locals; the child's context is
  `context_hash.wrapping_add(RepetitionCache::mix_key(rep_key))`, computed
  once before the frame loop and passed to both `evaluate_all_children`
  and `evaluate_child` (signature gains one `u64`; the ~15 `#[cfg(test)]`
  call sites pass `RepetitionCache::context_hash` of their stack, or `0`
  — the hook is off under tests by default). A per-probe
  `RepetitionCache::context_hash(&self.path_stack)` recompute
  (O(depth), single source of truth) is the recorded fallback if the
  derivation proves wrong in Phase 0.

## Objective

1. **Phase 0** — implement the env-gated hook; prove liveness, context
   fidelity, and soundness on cheap probes (dec10 is 4.26 M evals —
   sub-second; stress 29 s); run the pre-registered kill gates.
2. **Phase 1** (only if Phase 0 passes) — fresh paired rollout over the
   frozen plan12 corpus: 22 cases × 6 salts × 2 arms (`base`, `inctx`) =
   **264 runs**.
3. Per-case verdicts per pinned `gate_methodology.md` v1.0 applied
   verbatim; the paired-ratio dispersion recorded as the comparison
   rule's **sixth** real-arm calibration data point.
4. **No product change remains** — hook, side-band, and signature churn
   are added, measured, and reverted; `git diff --exit-code` clean before
   the report.
5. With this plan, **#21 closes** (adopt → sized hand-off; reject/defer →
   recorded); #22 fringe is then the only open reopened-backlog item.

## Pre-registered scope decisions

- **D1 — corpus, caps, salt set (inherited verbatim, frozen).** The 22
  plan12 fixtures and per-case caps (stress 2.5 B, m20_white 2 B, others
  1 B; `--timeout 600` secondary); salts {0, 1, 2, 3, 4, 5}; salt 0
  always runs. Corpus and caps as pinned in
  [`measurements/plan12/driver.py`](measurements/plan12/driver.py).
  Extensions pre-registered only (D5).
- **D2 — arms (fixed) and the hook.** Two arms, the 264-run shape
  (baseline arm first and identity-checked before the candidate):
  - **`base`** — shipped behavior, env unset.
  - **`inctx`** — the mechanism above.
  - **Hook spec.** (i) Env gate `ATOMIC_INCTX=1`, unset ⇒ shipped
    behavior ⇒ bit-identical; the rep-cache itself is untouched
    (keying, capacity, plan9 store sites, clear-per-run semantics). (ii)
    Probe site: `evaluate_child`'s final branch, between terminal
    classification and unsolved-bounds reuse, as above. (iii) Signature:
    the child context is computed once per `dfpn` frame (loop-invariant)
    and passed down; no `Search` field, no per-probe stack recompute.
    (iv) Side-band `ATOMIC_INCTX_STATS=1` (default off, **off during all
    rollout measurement runs**) prints one stderr line per run:
    `inctx: probes=<n> hits=<m>` (probe count at the hook site, hit
    count).
  - **Command line per run:**
    `atomic_solver --fen <FEN> --tt-size 128 --first-outcome --outcome-only
    --salt S --budget C --timeout 600` (`evals:` on stderr); the
    candidate arm additionally sets `ATOMIC_INCTX`. No other flag
    changes.
- **D3 — soundness and fidelity invariants (hard gates; a violation
  halts). Liveness-first ordering per the report11-erratum lesson: every
  identity claim is recorded only after hook liveness is demonstrated
  (c0), and after every lib change `cargo build --release` AND
  `cargo build --release --examples` run before any anchor run (plan18
  finding).**
  - (a) **Pristine pre-hook identity.** The unhooked build at HEAD
    reproduces the plan12 salt-0 values exactly (stress 249,480,478; m22
    14,156,269; m23 9,673,403; dec13 3,822,602; dec10 4,262,128; m20
    censored). **Fully pre-verified in this plan session (2026-10-11,
    read-only, clean tree at 5f2b210, existing release build): all six
    anchors reproduced exactly** (m20 budget-censored at 2 B,
    `evals: 2000000027`, no decisive outcome — the recorded shape).
    The execution session re-runs the full (a) set on its own build.
  - (b) **Baseline-arm identity.** Every fresh `base` cell reproduces
    plan12's recorded per-salt value exactly, all 132 cells — the null
    is degenerate; any mismatch is an identity failure, not noise.
  - (c) **Hook fidelity anchors (checked before any rollout; a mismatch
    halts for hook debugging, not a verdict):**
    - (c0) **Liveness first.** `ATOMIC_INCTX=1` at salt 0 on dec10 with
      the side-band shows `hits > 0` **and** a cumulative `evals:` value
      different from the baseline 4,262,128, with the outcome preserved
      (`win`). An env-set run that is bit-identical to the baseline with
      `hits > 0` is the dead-code-path/false-liveness signature → HALT;
      an env-set run matching the baseline with `hits = 0` means the
      hook never fired → same as K1.
    - (c1) **Env-unset quick-suite byte-identity.** The hook build at
      default settings, `benchmark --suite quick --json --first-outcome
      --timeout 3`, must equal the unhooked run bit-for-bit (archived
      total 38,974,090, 59/59 cases).
    - (c2) **Determinism.** A rerun of any (case, salt, arm) reproduces
      stdout and `evals:` bit-for-bit.
    - (c3) **Context fidelity.** In a Phase-0-only instrumented probe
      (assert variant, never in rollout runs), the passed child context
      must equal `RepetitionCache::context_hash(&self.path_stack)`
      computed directly at the probe site — zero assertion failures.
      Plus one throwaway unit test: a constructed repetition position
      where `evaluate_child` (hook on) returns the same `Draw` depth the
      child's `dfpn` frame returns.
    - (c4) **Side-band neutrality.** The same run with and without
      `ATOMIC_INCTX_STATS` (hook on, salt 0, dec10): identical
      stdout/`evals:`; retention of verdict quality is the (d) surface.
    - (c5) **No adopted-claim sub-study (recorded inapplicability).** The
      hook asserts nothing new: every hit returns a value the search
      itself proved under the exact same context (plan9 contract). The
      soundness surface is (d).
  - (d) **Outcome invariants (substantive).** Zero decisive-outcome
    conflicts within any arm across salts, and every uncensored run of
    every arm matches its fixture `expected` outcome; a budget-censored
    `draw` is never read as a proven draw. A hit can only ever
    substitute a proven draw for a stale `(1, 1)` — any outcome flip is
    a soundness defect (halt and diagnose, never noise).
  - (e) **Cyclic-rook invariant per arm:** `8/8/8/8/2k5/8/8/4KR2 w - - 0 1`
    must never claim `Win` under `inctx`, run with `--no-preflight`
    (3 men — the pre-phase closure would claim Draw first and make the
    spot-check vacuous). Post-revert sanity:
    `cargo test --release --test test_repetition -- --include-ignored`
    green.
  - (f) **Revert:** hook + side-band + signature churn deleted;
    `git diff --exit-code` clean; release build re-verified; (a)/(c1)
    anchors re-run exact after the revert.
- **D4 — verdict machinery (pinned v1.0, applied verbatim).** Per case:
  per-salt paired ratios `inctx_s / base_s` over salt pairs where **both**
  sides are uncensored; censoring pattern per salt; baseline basin count
  from plan12. Verdicts: censored-win / censored-loss / win (median
  ≤ 0.9 ∧ ≥ 75% of ratios < 1) / regression (median ≥ 1.1 ∧ ≥ 75% > 1) /
  unclear. Lever verdict: **adopt** = ≥ 1 censored-win or win on a
  hard-class case (stress, m20_white, m22_white, m23_white) ∧ no
  regression / censored-loss anywhere; **reject** = any regression or
  censored-loss; otherwise **defer**. Low-diversity baselines (basin
  count < 3) cannot solely support an adopt. Under-budget rule: > 3 of 6
  censored salts on a case for either arm ⇒ case unresolved, contributes
  nothing. Note the asymmetry the rule itself encodes: dec10 (basin
  count 4) is the lever's target class but cannot solely support an
  adopt — the item's own framing anticipated this ("gated by it for
  reporting").
- **D5 — defer extension rule (pre-registered).** A defer is re-judged
  only after adding salts **{6, 7}** on the affected cases (both arms),
  fresh runs; the pooled verdict is recomputed. One extension; a second
  defer stands.
- **D6 — hand-off rule (pre-registered).** An **adopt** becomes a sized
  proposal to `dfpn` (reopen under trigger (a) — a new measured
  search-semantics lever for the repetition-dominated class; the
  plan16/plan17 pattern), containing the per-case paired tables, the hook
  specification as the implementation seed (product change in
  `evaluate_child` + the context-derivation plumbing, env hook deleted,
  the `repetition_cache.rs` module contract extended with the new probe
  site), the unit tests to extend (`src/search/dfpn/children.rs` /
  `repetition_cache.rs` test modules), and the landing contract
  (`cargo fmt`/`clippy`, `make test`, `make test-full`). A **reject** is
  recorded with its tables; #21 closes and `reexamination.md` §6 step 5
  gains the gate-grade pointer.
- **D7 — dispersion prediction (pinned rule verbatim, falsifiable).**
  The hook fires only where the rep-cache holds an entry for the probed
  `(child_key, context)` — repetition-region work only. **Pre-registered
  predictions:** sparse-effect shape (plan17/plan19-like): exact-null
  mass ≥ **40%**, frac-outside [0.9, 1.1] ≈ **10–40%**. dec10 is the
  only case with a pre-registered *directional* expectation (H1: the
  mispricing correction lets frames cut or solve earlier — median ratio
  < 1 with direction agreement); the alternative H2 (the `(INF, 0)`
  bounds distort threshold carry-over arithmetic adversely on some
  class) is what the rollout decides, with no directional commitment
  anywhere else. m22 is flagged as the open wildcard: its C8 share is
  ~0%, but plan15's repetition-region thrash amplifier lives at ply > 90
  where the `evaluate_all_children` surface (a C8-superset) may fire;
  direction-agnostic. No methodology revision is proposed from within
  this plan.

## Phase 0 (kill gates before any rollout)

Tooling: `measurements/plan21/` — `driver.py`/`parse.py`/`verdicts.py`
adapted from plan19/plan20; the hook implemented in a working tree and
validated **in this order** before any rollout run: (c0) liveness → (c1)
env-unset quick identity → (c2) determinism → (c3) context fidelity →
(c4) side-band neutrality → (d)/(e) invariants.

Probes (canonical caps, ≤ 20 min total):

1. **Liveness probe** (side-band on): dec10 salt 0 `inctx` (the (c0)
   anchor).
2. **Effect probes** (side-band off): dec10 salts {0, 1, 2}, stress
   salt 0, m22 salt 0, dec13 salt 0 — `inctx` vs the freshly re-run
   baseline at the same salt (6 paired runs).
3. **Coverage quantification** (side-band on, mechanism-understanding
   deliverable): dec10 salt 0, hook off + stats, hook on + stats —
   stale-re-eval events and evals-after-stale before/after, plus
   probes/hits. This prices how much of the measured 38.1% mass the
   cache route actually captures (the C8 class vs the superset claim).

Kill gates (any firing ⇒ no Phase 1):

- **K1 — inertness:** no effect-probe run diverges from its baseline
  cumulative evals (with outcomes preserved) ⇒ the mechanism never
  fires or fires without effect ⇒ HALT and debug (the coverage probe
  distinguishes never-fires from fires-but-inert).
- **K2 — soundness spot:** any uncensored probe run not `win`, or any
  stdout/stderr parse failure ⇒ HALT.
- **K3 — context fidelity:** any (c3) assertion failure ⇒ the
  O(1) derivation is wrong ⇒ HALT; fix the derivation (the O(depth)
  fallback is pre-registered) before any verdict thinking.

## Method

- **Phase 0** as above; verdicts: PASS → Phase 1; HALT (K1–K3 or any D3
  violation) → Phase 3 directly with the finding.
- **Phase 1 — rollout.** 264 runs, ≤ 3 concurrent processes; baseline arm
  first and identity-checked (D3(a)/(b)) before the candidate;
  transcripts to a non-committed RAW dir; parse to `state/runs.json` +
  `state/summary.json`; run the D3(d) checks.
- **Phase 2 — verdicts.** `verdicts.py` → `state/verdicts.json` (D4 table
  + paired-ratio dispersion summary + coverage profile from Phase 0).
- **Phase 3 — report.** D6 hand-off if any; revert and D3(f); write
  `report21.md` (final task); refresh the #21 row in `initiative.md`
  (item closes) and the history entry; update the
  `docs/plans/README.md` research row if the index summary changes.

## Gates

- **PASS Phase 0 → adopt / reject / defer** per D4 on the Phase-1
  rollout.
- **HALT** → any D3 invariant violated or K1–K3 fired: soundness or
  fidelity investigation, not a measurement. Fidelity mismatches are
  debugged before any candidate number is read.

## Out of scope

- The frame-local outcome-memo variant (fallback only, per the Mechanism
  section — and unsound in its naive form). Changes to the rep-cache
  itself (keying, capacity, store sites, serialization contract — the
  cache stays out of TT snapshots, proof events, and any artifact). The
  first-player-loss shortcut. The TT path-independence contract. TT
  capacity or eviction (`lean` territory). Chunk schedules, ε, ordering
  constants (all measured-closed surfaces).
- #22 fringe (solve-ignoring-rule50 seeding; Kaneko root threads — owned
  by `parallel`). Productization (the D6 landing contract is the
  owner's, post-adopt). Proof-tree layer, `ProofEvent` protocol,
  optimizer interface; methodology revisions.

## Budget envelope

Phase 0: hook build + anchor chain ≈ 30 min; probes ≈ 15 min (dec10 runs
are sub-second at 4.26 M evals; stress 29 s; quick identity ~1 min).
Phase 1: 264 runs at ≤ 3 concurrent processes — the baseline arm re-runs
plan12's recorded grid (~1.2 h wall at ≤ 3 concurrency, plan19/plan20
experience); the candidate arm's wall cost is the trajectory change
itself, bounded by the caps (censoring as in plan19). Total ≈ 3–5 h CPU;
no expected wall explosion (the hook adds one hash-map probe per
non-terminal TT-miss child eval — no per-probe stack recompute).

## Final task

Write `report21.md` in this directory: deliverables, the Phase-0 liveness/
fidelity/coverage results and kill-gate outcome (if Phase 0 ran), the
per-case verdict table with basin counts and censoring patterns,
fidelity-anchor results, the coverage quantification (how much of the
38.1% stale mass the cache route captures), paired-ratio dispersion
summary with the D7 prediction's fate, problems encountered, missing
tests, next steps (kickoff prompt: #22 fringe or initiative closure —
with #21 closed, #22 is the last open reopened-backlog item — or, on an
adopt, the `dfpn` reopen execution). Refresh the #21 row in
`initiative.md` (item closes), add the history entry, and update the
`docs/plans/README.md` research row if the index summary changes.
