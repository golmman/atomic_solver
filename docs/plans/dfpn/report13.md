# Report 13 — plan13 Session B: bounded pre-phase implementation (architecture R)

Plan: `plan13.md` (backlog #6). Phase 0: `report13_phase0.md` (Session A,
2026-09-16 — G-A GO via R, G-B PASS, checkpoint record at its end). This
report covers **Session B** (Phase 1 implementation) and subsumes the Phase 0
report per repo convention. Date: 2026-09-16.

## TL;DR

- The region-closure pre-phase is implemented as `src/search/preflight/`
  (`mod.rs` orchestration + soundness contract, `region.rs` packed keys +
  closure + exact-rank fixpoint + strategy/PV extraction, `verifier.rs`
  standalone replay verifier) and hooked into `Search::solve` and
  `Search::search_depth` before the DF-PN loop, with the `--no-preflight` CLI
  flag and the `preflight:` line (R5) as decided at the checkpoint.
- **The KQvK ladder is fixed**: the root is decided as a replay-verified,
  cycle-free Win at **5,866,690 child evals / 0.76 s** (exact DTM 15, region
  420,532 — the Phase 0 numbers reproduced in integration within <1%); all
  four ladder positions certify at dtm 15/13/11/9. plan12's criterion (root
  ≤ 20M evals / ≤ 60 s) is met at 0.29× evals and ~1% of the wall bar,
  against the >756M-node / 600 s unproven baseline.
- **Soundness battery is clean**: quick-suite drift 59/59 byte-identical
  (detector never fires on the suite); m22_white and the stress case
  byte-identical to HEAD with `preflight: deferred reason=detector evals=0`
  (verified by counter); `test_repetition --include-ignored` green — the
  cyclic rook is now **claimed Draw** by the pre-phase (sound, see below);
  G-B oracle re-run on the release binary: 240 samples, **0 contradictions**
  (94 certified wins at dtm ≤ 6, 120/120 draw claims, 26 sound deferrals);
  `make test` and `make test-full` green (387 passed / 0 failed incl. the
  slow suites and the new ladder integration tests).

## What was implemented (against the checkpoint record)

### Module layout (`src/search/preflight/`, all files under 20 KB)

- **`mod.rs`** — module doc carrying the normative soundness contract; the
  R3 detector (`occupied ≤ 3 && no pawns && no castling`; en passant
  transitively excluded); `PreflightReport` (decided/reason/evals/region/
  outcome/rank) surfaced via `Search::preflight_report()`; the claim/defer
  orchestration: closure → fixpoint → D3 rank guard → bound check →
  strategy extraction → replay verification → principal-line PV → claim.
- **`region.rs`** — exact collision-free packed keys (`stm` + up to three
  `(square, piece-code)` slots sorted by square, `Option`-guarded for >3
  pieces); `board_from_key` round-trip; clock-0 board-only terminal
  classification mirroring `Position::outcome_from_state` minus the rule50
  branch (parity-tested); the closure BFS with per-child eval accounting;
  the side-to-move-perspective AND/OR fixpoint (WIN iff ∃ LOSS child, LOSS
  iff all children WIN, remainder = DRAW) with delta-free exact-distance
  relaxation; rank-decreasing strategy extraction and the A5 principal line
  (winner: dist−1 child; loser: max-dist child; ties by child order).
- **`verifier.rs`** — the Phase 0 G-B verifier, standalone on purpose: it
  consumes only the root `Position` and the strategy map. At winner-to-move
  nodes the certified move is played (found by exact child-key match over
  real movegen), at loser-to-move nodes *every* legal reply is covered, every
  line must end in a terminal win for the winner within the rank bound, and
  no board may repeat anywhere on any line (the defense-in-depth gate — a
  cyclic position cannot yield a cycle-free certificate). Budget/stop aborts
  are reported separately from strategy defects; both defer.

### Integration points

- `Search::solve_with_progress` and `Search::search_depth` run the pre-phase
  hook after `begin_run` (A3): a claim returns `(outcome, pv, nodes)`
  directly with `PvStatus::PreflightProof` (D1) — no refinement rounds run
  (A4); a Draw claim keeps `PvStatus::None` with an empty PV and is reported
  via the preflight report. `search_depth` passes `Some(max_depth)` as the
  certificate bound (A3). `search_depth_with_prefix` skips the pre-phase
  (the root-only closure does not model the supplied repetition prefix).
- **R4 budget accounting**: pre-phase child evaluations (closure BFS +
  verification + PV extraction) are added to `child_evals`, count against
  `child_eval_budget`, and on exhaustion the pre-phase defers with
  `eval-budget` — the ordinary search then immediately reports
  `ExitReason::BudgetExhausted` (contract verified by a dedicated test). The
  pre-phase never consults the wall clock (A8: the stop flag is checked
  between phases and inside the verifier walk).
- **Defense in depth on the PV**: the hook additionally runs
  `Search::validate_pv(&pv, pos, outcome, Some(rank))` before claiming; any
  defect downgrades to `cert-fail`.
- **CLI (R5)**: `--no-preflight` (parsed in `src/cli.rs`, wired in
  `src/main.rs`, added to `-h` and the module doc); one
  `preflight: decided outcome=… evals=… region=… rank=…` /
  `preflight: deferred reason=… evals=…` line in non-`--outcome-only` mode;
  `pv_status: preflight-proof` label. Benchmark JSON keys untouched. The
  main.rs doc documents the pre-phase closure as the bounded exception to
  "RAM = TT only" (D4: default region budget 1,000,000; the measured ladder
  region is 420,532 positions — ~60 MB transient; ~100–120 MB worst case at
  the full budget).
- **R2 honored**: no `ProofEvent` emission — pinned by a dedicated test
  (claiming solve with a wired worker sends no events); documented in the
  module docs and AGENTS.md.

## Integration numbers (vs the Phase 0 prediction)

Ladder, release build, default TT/ε, pre-phase enabled (`--outcome-only`):

| position | Phase 0 R (spike) | integrated | exact DTM | region | wall |
| --- | --- | ---: | ---: | ---: | ---: |
| root `8/2K5/k7/…/4Q3 w - - 0 1` | 5,827,440 evals | **5,866,690 evals** | 15 | 420,532 | 0.76 s |
| step1 `8/1k1K4/… w - - 2 2` | 5,827,440 | 5,838,965 | 13 | 420,532 | 0.73 s |
| step2 `8/8/2k1K3/… w - - 4 3` | 5,827,440 | 5,834,211 | 11 | 420,532 | 0.73 s |
| step3 `8/5K2/8/3k4/… w - - 6 4` | 5,827,440 | 5,827,778 | 9 | 420,532 | 0.74 s |

The small eval delta over Phase 0 is the verification and PV-extraction
evaluations, which the spike reported separately and the integration counts
honestly (R4). Every root is `pv_status: preflight-proof` with PV length =
exact DTM, and each PV passes `Search::validate_pv(…, Some(rank))` in the
slow integration tests. plan12's success criterion is met with margin
(0.29× the 20M-eval bar; ~1% of the 60 s bar), against the >756M-node /
600 s unproven DF-PN baseline.

## Soundness argument as implemented

1. **Detector** (R3): the gated class (≤ 3 men, no pawns, no castling) has
   no hidden state affecting movegen — no en passant without pawns, no
   castling by predicate, so `(placement, side to move)` fully determines
   the board-only game. The `region_key` codec is exact (no hashing):
   collision-freedom is structural, and the round-trip is unit-tested.
2. **Closure + fixpoint**: the closure enumerates every legal continuation
   (Loss coverage is structural); the fixpoint's Win claims are
   rank-decreasing by construction, so every extracted strategy is
   cycle-free (a rank-decreasing cycle would contradict the exact, unique
   `dist` per position). The undecided remainder is a board-only Draw ⇒ a
   genuine Draw under the solver's repetition/rule50 semantics by the
   monotonicity lemma (D2, clock-independent).
3. **rule50** (D3): decisive claims require `halfmove + rank ≤ 99`; the
   clock grows by exactly one per ply on these lines (only extinction
   captures occur, which reset it), so every certificate position stays
   below the engine's rule50 boundary (verified against
   `Position::outcome_from_state`/`evaluate_child` semantics in a unit
   test, including the mate/extinction-outranks-rule50 precedence). The
   guard is exercised end-to-end at the exact boundary (rank 15: clock 84
   claims, clock 85/99 defer) in the slow tier.
4. **Certificate replay**: no decisive claim is returned unless the
   standalone verifier replays the full strategy from the real root — all
   defender replies covered, mate within the rank on every line, zero board
   repeats. Negative cases (missing entry, corrupted entry, injected cycle,
   bound violation, budget abort) are unit-tested and all defer.
5. **No TT interaction**: verified by test — a claiming solve leaves the TT
   with zero live/solved entries; the offline proof pipeline cannot
   reproduce a pre-phase proof (R2 gap, documented).

## Validation battery

- `cargo fmt` / `cargo clippy --all-targets` clean; `cargo doc` not run
  (see Missing tests).
- `make test` (fast gate, release): green — 0 failures across all suites.
- `make test-full`: green — **387 passed / 0 failed** (exit 0), including
  the slow regression/stress suites and the new `tests/test_preflight.rs`
  ladder tests (5 slow integration tests, all `#[ignore = "slow: ..."]`).
- `test_repetition --include-ignored`: green. Note the factual correction
  below: the cyclic-rook position is **3 men** and now gates; the pre-phase
  proves it a **Draw** by full closure (region 451,192, 5.2M evals,
  egtb3-r.bin-consistent), so all three gates (never a Win / stays Draw /
  solve-reset-solve) hold.
- **Drift protocol**: `benchmark --suite quick --json --first-outcome` on a
  HEAD checkout vs the implementation: **59/59 cases byte-identical**
  (outcome, status, nodes, child_evals, pv_len, wrong, timeout; aggregate
  evals equal). The detector provably never fires on the suite (no
  ≤3-men-no-pawn fixture; counter-checked on m22:
  `preflight: deferred reason=detector evals=0`).
- **m22_white and the stress case**: byte-identical stdout to a HEAD-built
  binary in first-outcome mode (m22 95-ply line; stress 477-ply line — the
  post-plan11 baselines), i.e. exactly baseline as required.
- **G-B oracle protocol on the release binary** (Phase 0 protocol re-run):
  240 sampled legal attacker-to-move KQvK placements (side-not-to-move not
  in check) classified by `egtb3-q.bin` — **0 contradictions**
  (94 certified Wins at dtm ≤ 6 with replay-validated PVs, 120/120 Draw
  claims, 26 deferrals, all with sound deferral reasons). Instrument
  archived as `measurements/plan13/plan13_oracle_release.rs`, log
  `measurements/plan13/oracle_release_binary.log`.

## Findings and deviations

1. **plan13 R3 contains a factual error about the cyclic rook**: the
   report7 safe-area position `8/8/8/8/2k5/8/8/4KR2 w` is **3 men**, not 4,
   so the detector *does* fire on it. The plan's defense-in-depth argument
   anticipated exactly this ("a cyclic position cannot yield a cycle-free
   certificate, so even a misgate degrades to 'deferred'") — and better: the
   fixpoint is sound here, proving the position a Draw (a claim, but the
   correct one; egtb3-r.bin agrees). The `test_repetition` gates are
   unchanged and green. No code change needed; the "4 men" margin argument
   is superseded by the soundness argument.
2. **Engine semantics discovery (documented in the test module)**: commoners
   are pseudo-royal with *adjacency immunity* — a lone commoner adjacent to
   the enemy commoner is never "in check"
   (`atomic_movegen::board::Board::legal`). Consequence: KQvK positions with
   adjacent kings (e.g. `8/8/8/8/8/1K6/1Q6/k7 b`) are genuine Draws — the
   pre-phase proves one by full closure as the fast-tier end-to-end D2 test
   (table-confirmed). Several pre-existing tests use such 3-men fixtures for
   DF-PN-specific invariants; they were recalibrated to the 4-men two-rook
   fixture (`4k3/8/8/8/8/8/8/4KRR1 w`) with plan13 notes:
   `src/proof_tree/worker/tests.rs::solve_populates_proof_tree_with_nodes`
   (needs proof events; a pre-phase claim emits none — R2),
   `src/reconstruct/tests.rs::seeded_entry_resolves_a_mate_in_1_search`
   (needs TT stores; a claim stores nothing),
   `src/search/dfpn/tests.rs::set_timeout_zero_causes_immediate_exit` /
   `::solve_with_progress_calls_closure` (timeout semantics and the progress
   closure are DF-PN-path behavior). `tests/test_cli.rs` mate-in-1 pins
   `proven-shortest` via `--no-preflight` and gained a sibling test pinning
   the new default (`preflight-proof`, plus the `--outcome-only` suppression
   of the `preflight:` line).
3. **Two integration bugs found and fixed before validation** (both in the
   new key codec, caught by the round-trip unit test + CLI smoke):
   `board_from_key` truncated the 10-bit slot with `slot as u8` (losing the
   square bits), and the piece-char table was off by one (commoner decoded
   as queen). Both would have produced garbage closures; neither survived
   the key round-trip test.
4. **`timeout 0` still claims**: the pre-phase never consults the wall clock
   (R4 as decided), so a zero-timeout solve of a gated position returns the
   pre-phase claim. This is the documented contract; the affected test was
   recalibrated (above) rather than the behavior changed.
5. A1–A8 assumptions implemented as recorded; no overrides were needed.
   R1 (binding) landed as `PvStatus::PreflightProof` exactly as decided.

## Problems encountered

- The `#[cfg(test)]` reorganization (moving tests between module files to
  respect the sizing rules) caused transient compile churn; resolved and
  verified by the full battery afterwards.
- Phase 0's oracle harness bounded R with the region budget only (evals
  unbounded); my first oracle re-run applied a 3M eval cap and deferred all
  240 samples — corrected to the Phase 0 protocol before recording results.
- The oracle instrument itself had a key bug (king squares assigned by piece
  case instead of strong side, swapping roles for black-strong samples) that
  first surfaced as 7 "WIN on draw sample" contradictions; decoding the keys
  showed the samples were misclassified by the instrument, not misclaimed by
  the pre-phase. Fixed and re-run to 0 contradictions. Recorded because it
  is a good reminder that oracle bugs look exactly like solver bugs.

## Missing tests

- `cargo doc` was not run in this session (no public-API doc build in the
  battery); the preflight module's docs are written but unlinted by rustdoc.
- No integration test asserts the transient closure memory bound (~60 MB on
  the ladder region); the budget is documented, not measured in CI.
- The popcount-4 tier (KRvK+P etc.) remains unimplemented by design (round 2
  candidate); nothing exercises the region budget's upper range (a >1M
  3-man component does not exist, so the D4 default budget can only be
  exercised via the direct `region::analyze` unit test with a small budget).
- The `--no-preflight` flag has no dedicated CLI end-to-end test beyond the
  recalibrated `proven-shortest` test (which passes the flag).

## Next steps

1. **Backlog re-rank**: the ladder class is closed (root + steps 1–3 now
   solve instantly with certificates). What the bake-off taught about
   general DF-PN threshold pricing stands (see `report13_phase0.md`): a
   closure fixpoint prices every node exactly once, PN's failure was
   path-tree memory rather than pricing, and context-free fact reuse has a
   board-set size wall — the lean9-class lever remains open but with
   sharpened priors against threshold-arithmetic tweaks.
2. **Closure draw proofs for the general class** (recorded in Phase 0 as
   next-step (a)): the pre-phase now *returns* certified Draws (D2), which
   the 3-men smoke positions and the cyclic rook exercise; a deliberate
   design for extending draw proofs beyond the detector (e.g. a
   popcount-4 tier with the region budget as the safety gate) is a
   plausible follow-up plan — measure before building.
3. **Proof-pipeline asymmetry (R2)**: pre-phase claims are invisible to the
   offline proof pipeline (no events, no TT entries). If proof trees are
   ever required for gated positions, a follow-up plan must synthesize
   events from the certificate strategy (the Phase 0 R2(b) option).
4. The four recalibrated test fixtures and the `preflight-proof` CLI
   surface should be watched in the next slow-tier run on the reference
   host (the fast gate now includes one full-closure draw test at ~1 s
   release; the debug tier pays ~10 s for it).

Per repo convention, every plan ends with the task of writing its
`report13.md` — this document. The initiative's History and the backlog #6
row are updated accordingly.
