# Plan 2: Candidate-line verification (engine-guided PPV solve)

Initiative: `conversion` backlog #2, scoped to the **verification lever**
(backlog item 2b). The ordering-guidance lever (2a, seed history/killers from
the candidate line) is deliberately deferred — one lever per plan; if
measurements here justify it, it opens as plan3. Prerequisite reading:
`dfpn/plan7.md` (the constraint this plan operates under: engine-strength
*beliefs* must never enter the search as facts), `initiative.md` (motivation
section "What an alpha-beta engine does differently" — the line-oracle framing
is this plan's charter), and `examples/verify_ppv.rs` (the verification
machinery this plan generalizes).

## Goal

Use Fairy-Stockfish (submodule `libs/Fairy-Stockfish`, pinned at
`226c7f18c854372d5612be2a7d7f14449ae5a239`, fairy_sf_14-322) as a **line
oracle**: obtain a candidate winning line for the deep-conversion position
class, then *prove or refute it exhaustively with the solver's own machinery*.
If the line verifies, the decisive outcome costs only the verification work;
if it does not, fall back to the unguided solve. Primary metric: `child_evals`
to first decisive outcome on the `make stress` case, against the inherited
post-plan9 baseline (first-outcome 13,907,467 nodes / 249,480,478 child evals
/ 53.8 s; default mode 19,943,731 / 338,094,183 / 72.3 s). Engine wall time is
measured separately and reported alongside; it is a real cost of this mode.

Default binary behavior (no new flag given) must be byte-identical to
pre-plan2. The whole lever is opt-in.

Phase 0 is a measurement-only spike requiring **no production code changes**
(the existing `verify_ppv` example already verifies a supplied UCI line); it
carries a hard go/no-go.

## Background (one paragraph)

The stress position's first-outcome phase spends ~249M child evals exploring
wrong subtrees before the tempo/progression conversion is found. A strong
atomic-chess engine finds a plausible winning line in seconds, but its output
is a belief, not a proof — exactly what `dfpn/plan7` forbids reusing as fact.
The Kawano-simulation insight, applied at root scale (initiative.md
"motivation"), is that beliefs are still useful as *candidate generators*:
verify the candidate exhaustively and you get a proven result at
verification cost; fail to verify and you have lost only the (cheap) engine
query plus whatever partial verification work was done. All building blocks
already exist: `Search::search_depth_with_prefix` (bounded defender-refutation
search with repetition prefix), `examples/verify_ppv.rs` (the full PPV
verification loop), `uci_to_move`/`Position` replay (line parsing), and the
first-player-loss TT shortcut guarantees failed verifications leave only
path-independent base entries behind. What is missing is (a) extracting the
verify loop from the example into the library, (b) a CLI entry point that
takes a candidate line, and (c) an engine client that produces one — this plan
adds all three, plus the fallback semantics.

## Soundness contract (normative)

1. **Nothing from the engine is a fact.** Engine evaluations, scores, node
   counts, and move choices never enter the TT, proof events, snapshots, or
   any decisive claim. A decisive outcome in guided mode is established
   exclusively by solver-proven facts: the verify loop's bounded
   `search_depth_with_prefix` wins (genuine proofs — depth-truncation leaves
   store unsolved bounds and can never complete a proof, plan1 Fact 1),
   terminal evaluation of the line's final position, and complete enumeration
   of defender replies.
2. **Verification is exhaustive on the defense.** Every legal defender reply
   at every defender ply must be proven lost within the remaining line
   length; the defender's candidate move must be a longest defense; all moves
   must be legal on the replayed positions; the final position must be
   decisively terminal. No reply is skipped, pruned, or ordered by the
   engine. This is exactly today's `verify_ppv` logic, moved into the library
   unchanged in semantics.
3. **Repetition history is preserved.** Each refutation search receives
   `prefix_keys` computed from the full replayed path (as `verify_ppv` does
   today), so two-fold-repetition-as-draw and rule50 semantics are identical
   to a normal solve.
4. **Failure falls back, never claims.** Verification failure (refuted
   defense, illegal move, non-decisive terminal, or budget/timeout
   exhaustion) prints the reason and runs the unguided solve. The fallback's
   decisive outcome must equal a fresh-process solve of the same position
   (property test; working-agreement #5). Partial verification work persists
   only as plan7-legal path-independent TT entries.
5. **Budget contract unchanged.** Verification and fallback share the
   configured `child_eval_budget` / timeout; `ExitReason::BudgetExhausted`
   semantics are untouched. Budget exhaustion during verification is a
   *fallback* condition, never a verification success.
6. **Opt-in.** Without `--guide-moves`, the binary's behavior is
   byte-identical to pre-plan2 (no new code paths execute, no engine contact,
   no extra output lines).

## Design

### Library: `src/verify.rs` (new)

Extract the PPV-verification loop from `examples/verify_ppv.rs` into a public
library module (re-exported from `lib.rs` alongside `notation`/`position`/…);
the example refactors to call it (must stay green: `tests/verify_ppv.rs`
guards it). Semantics preserved exactly — this is a move, not a rewrite.

```rust
/// Defect that refuted a candidate line (the `is_ppv: false` reasons of
/// today's example, structured).
pub enum VerifyDefect {
    IllegalMove { ply: usize, uci: String },
    PrematureTerminal { ply: usize },
    NonDecisiveFinal,
    ReplyNotLost { ply: usize, reply_uci: String, outcome: Outcome },
    NotLongestDefense { ply: usize, chosen: u32, longest: u32 },
    OutOfBudget { ply: usize },
}

/// Verify that `line` is a PPV for `root`. Returns the proven root outcome
/// and per-ply depth report on success. Bounded by the shared `search`
/// deadline/budget; budget exhaustion is a defect (fallback condition), not
/// a success.
pub fn verify_line(
    root: &Position,
    line: &[Move],
    search: &mut Search,
    deadline: Instant,
) -> Result<(Outcome, Vec<u32>), VerifyDefect>;
```

Implementation mirrors `verify_ppv.rs` lines 111–269 (replay the line with
`uci_to_move`, iterate defender plies in reverse, enumerate **all** legal
replies, `search_depth_with_prefix` each with the replay prefix). Estimated
size ~6–8 KB — inside the file-size guideline; the example drops ~5 KB.

### CLI: `--guide-moves "<uci>"` (search CLI, opt-in)

- Mutually exclusive with nothing; composes with the existing flags
  (`--timeout` bounds verification + fallback; `--tt-dump-path` still dumps
  whatever TT state the run produced; `--first-outcome` is orthogonal).
- Parse the UCI tokens against the root position; on an illegal token, error
  out (exit 1) — a malformed guide is a user error, not a fallback condition.
- Run `verify_line`. On success: print the usual
  `outcome: …` / `pv: …` lines plus `guided: verified`, with the verified
  line as the returned PV, and a new `PvStatus::VerifiedPpv` variant
  (CLI label `verified-ppv`): *the PV is a verified proof, but its length is
  not proven minimal*. Refinement is **skipped** in this plan when
  verification succeeds — the shortest-PV product is not this lever's target,
  and mixing a TT-extracted (unvalidated) PV with a verified-proof status
  would blur the `pv_status` contract. Document this in the `pv_status` docs.
- On `VerifyDefect`: print `guided: failed (<reason>)` to stderr and run the
  normal unguided solve path (identical to pre-plan2 behavior, including
  `pv_status`), from the same `Search` instance.
- `main.rs` gains no process-spawning code and no engine knowledge; it
  consumes a move list.

### Example: `examples/sf_guide.rs` (new)

Fairy-Stockfish UCI client + guided solve in one command:

- Flags: `--fen`, `--engine-path <BIN>` (default
  `libs/Fairy-Stockfish/src/fairy-stockfish`), `--engine-nodes <N>`
  (node-limited `go`, see determinism below), `--multipv <K>` (default 1;
  see fallback ladder below), `--solve` (also run the guided solve;
  default: print the candidate line in `--guide-moves` form and exit, so the
  two steps stay composable in a shell).
- Engine session (subprocess, std only): `setoption name UCI_Variant value
  atomic`, `setoption name Threads value 1`, `setoption name Hash value
  <MB>`, `position fen <FEN>`, `go nodes <N>`; parse the last `info … pv …`
  line and `bestmove`.
- **Candidate ladder:** with `--multipv K`, collect the top-K PV lines from
  the info stream and try `verify_line` on them in reported order (best
  first) until one verifies. Engine belief order only; each candidate is
  verified from scratch (no cross-candidate facts). Bounded by the same
  deadline. If all K fail, report and (with `--solve`) fall back to the
  unguided solve. K=1 keeps the default deterministic and simple; K is a
  measurement knob for Phase 0, keep the report honest about which K earned
  its cost.
- NNUE: not part of this plan. Fairy-Stockfish runs its classical (handcrafted)
  eval for atomic; variant nets are not bundled with the submodule. If a
  stable, hash-recorded atomic net source exists, record the URL in
  `report2.md` as a follow-up; do not block on it and do not commit nets.

### Fairy-Stockfish build & reproducibility (documented in report2.md)

- One-time manual build (cargo must not depend on it):
  `make -C libs/Fairy-Stockfish/src ARCH=x86-64-modern` (g++ 16.2 available in
  the container). Record the exact make line, the binary path/name, and the
  binary's sha256.
- Determinism: single thread + node-limited `go nodes <N>` + fixed Hash make
  the engine query reproducible for a fixed binary; still, treat the engine
  output as *external input* (like `--guide-moves` typed by hand) — nothing
  in the solver depends on its determinism. Record engine settings in the
  report for reproducible measurement.
- FEN compatibility: the solver's standard FENs are accepted by Fairy-Stockfish
  under `UCI_Variant=atomic`; sanity-check one position end-to-end in
  Phase 0 step 1 (`list_legal` vs `d`-command output) before trusting parses.

### Alternatives considered (and rejected)

- **Engine query inside the search CLI** (`--engine-path`): couples the
  resource-bounded product CLI to a subprocess and drags UCI parsing into
  `main.rs`. Rejected; the example provides the one-command workflow instead.
- **Seeding ordering from the engine line in the same plan** (backlog 2a):
  a different lever (what gets searched first vs where the line comes from),
  different drift surface; deferred per the one-lever agreement.
- **Trusting the engine's score to skip verification on "clearly winning"
  lines**: violates soundness contract #1 outright. Rejected.
- **Verification via TT-snapshot reconstruction / proof-tree worker**: the
  guided run's TT dump remains reconstructable by the standard offline chain;
  verification itself stays in-process and needs no tree. Rejected for scope.

## Phase 0 — measurement spike (go/no-go; zero production code changes)

Everything below runs against existing binaries plus the freshly built
Fairy-Stockfish.

1. **Build & sanity.** Build FSF; record version/pin/sha256. Confirm
   `UCI_Variant=atomic` accepts the stress FEN and produces legal PV moves
   (cross-check a few plies with `list_legal` / `replay`).
2. **Line quality sweep.** Query the engine on the stress FEN with
   `go nodes ∈ {1M, 10M, 100M}` (Threads=1). For each: extract the PV, run
   `verify_ppv --fen <stress> --moves "<pv>" --timeout 600`, record
   `is_ppv`, verification child evals / nodes / wall time, and line length.
   Repeat on the m22 control
   (`4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22`, baseline 3.1 s
   / 858,117 nodes / 14,156,269 evals — guidance must not *lose* to 3 s on
   easy positions) and on the ~10 hardest `dec*` suite cases.
3. **Fallback cost accounting.** For each refuted candidate, the mode's cost
   is engine time + verification work + full unguided solve. Compute this
   for the sweep, including the `--multipv 3` ladder shape (3 verification
   attempts then fallback).
4. **Go/no-go:** proceed only if
   (a) on the stress case, engine time + verified-line verification cost is
   ≤ 50% of the 249,480,478-eval unguided baseline (the lever's premise is
   collapse to near-verification cost; a thin win is not worth a new mode),
   and the line verifies at some node budget ≥ 1M;
   (b) across the dec-suite sample, guided cost (engine + verify + expected
   fallback share) does not exceed unguided cost on average, and success
   rate ≥ 50%;
   (c) no candidate ever *passes* `verify_ppv` on a line the solver would
   call a draw (spot-check: verify a deliberately perturbed defender move in
   the stress PV — `verify_ppv` must say false; it does, but demonstrate it).
   On no-go: write `report2.md` with the sweep tables, mark backlog #2's
   verification lever closed with the negative result (ordering guidance 2a
   stays open and is re-ranked), revert nothing (Phase 0 changed no code),
   and stop.

## Tasks

1. Phase 0 spike per above; record all numbers in the report. On go,
   continue; on no-go, stop after task 10.
2. Extract `src/verify.rs` from `examples/verify_ppv.rs` (move, semantics
   unchanged); refactor the example onto it; `tests/verify_ppv.rs` green;
   re-export from `lib.rs`. Unit tests in `src/verify.rs`: accept a known
   short PPV; refute a line with one defender reply swapped; refute a
   non-longest defense; illegal move; non-decisive final position; budget
   exhaustion → defect, not success (mutant tests: each rule broken one at a
   time must be caught).
3. Add `PvStatus::VerifiedPpv` (doc: proof-valid, length-unproven; CLI label
   `verified-ppv`), wire through `main.rs`'s label match and
   `src/search/dfpn/mod.rs` docs.
4. Add `--guide-moves` to `src/cli.rs` + `main.rs`: parse → `verify_line` →
   `guided: verified` / `guided: failed (…)` + fallback path; without the
   flag, behavior byte-identical (add a CLI test asserting identical output
   shape to pre-plan2 on a decisive quick case).
5. Add `examples/sf_guide.rs` (UCI client + candidate ladder + optional
   `--solve`), following `examples/common.rs` conventions.
6. Property tests (working-agreement #5): guided success on a hand-verified
   line returns the line unchanged with `VerifiedPpv`; fallback after
   refutation produces the same decisive outcome as a fresh-process solve on
   (a) m22_white, (b) one repetition-heavy case from `tests/test_repetition.rs`
   (the cyclic rook must never claim a win in any guided configuration).
7. Soundness gate: `cargo test --release --test test_repetition --
   --include-ignored` green; `cargo test --release --test test_ghi --
   --include-ignored` green (TT contamination from failed verifications must
   not disturb repetition semantics).
8. Drift protocol: `benchmark --suite quick --json --first-outcome` before
   vs. after — all 59 outcomes unchanged and non-guided runs bit-identical
   (contract #6); plus `cargo test --release --test test_plan6 --
   --include-ignored` (budget contract) and `cargo test --release --test
   test_move_order -- --include-ignored` (ordering untouched by this plan).
9. Stress measurement: guided first-outcome run (engine nodes from the
   Phase 0 sweet spot) — record engine wall time, verification
   `child_evals`/nodes, total wall time, and compare against the inherited
   baseline (first-outcome 13,907,467 / 249,480,478 / 53.8 s); also one
   default-mode guided run and the m22 control. Record whether the verified
   PV length matches the unguided solve's shortest PV (expected: it is
   longer; state the difference).
10. Update AGENTS.md (main.rs flag list, `examples/` list, lib.rs re-export
    list, `pv_status` label set) and `initiative.md` (backlog #2 status,
    History entry, plan3 pointer for the ordering-guidance lever).
11. Write `report2.md` in this directory (tools/examples used, problems,
    unresolved parts, missing tests, next steps).

## Validation checklist (summary)

- `cargo fmt`, `cargo clippy --all-targets`, `cargo test --release` (fast
  gate), `cargo doc --no-deps`.
- `tests/verify_ppv.rs` + new `src/verify.rs` unit/mutant tests.
- Repetition + GHI soundness gates with `--include-ignored`.
- Quick-suite drift: non-guided byte-identical; guided is opt-in and cannot
  appear in the suite.
- Budget-contract and move-order suites green.
- Stress measurement recorded vs inherited baseline; m22 control clean.

## Non-goals

- **Ordering guidance** (backlog 2a: seeding history/killers from the
  candidate line) — candidate for plan3; do not sneak it in here.
- Any engine artifact as fact: scores, evals, node counts, bestmove *without*
  verification (dfpn plan7 constraint; initiative non-goals).
- NNUE for the atomic variant, engine parameter tuning, strength settings.
- Wiring the FSF build into cargo, committing nets, or vendoring engine
  output; the submodule stays a manual, pinned dependency.
- Parallelism (backlog #4), cross-clock/cache levers (plan1's closed
  surface), shortest-PV refinement of verified lines.
- Changing solver semantics the verification leans on: two-fold-as-draw,
  rule50-in-key, budget contract, `verify_ppv` logic itself.
