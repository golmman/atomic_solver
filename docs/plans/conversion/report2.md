# Report 2 — Candidate-line verification (engine-guided PPV solve)

**Verdict: NO-GO.** The verification lever (backlog #2b) is closed with a
negative result. No Fairy-Stockfish candidate line verified on the stress
case at any measured node budget, and the two verification attempts that got
past the terminal check each burned their entire budget (>1.0B and >1.4B
child evals) without proving the root. On the m22 control the guided mode
loses to the unguided baseline by two orders of magnitude. On the easy dec
sample the lever works mechanically (6/6 lines verified) but the engine
query cost erases the gain. Per the plan's Phase 0 contract, no production
code was kept: the working tree is byte-identical to pre-plan2 and the fast
gate (`make test`) is green.

## What was run

Phase 0 only — no production code changes. Tools used:

- `libs/Fairy-Stockfish` (submodule, pinned `226c7f18c854372d5612be2a7d7f14449ae5a239`)
  driven over stdio UCI.
- `target/release/examples/list_legal` — FEN/legal-move cross-check.
- `target/release/examples/verify_ppv` — the pre-plan2 PPV verifier, unchanged.
- `target/release/examples/benchmark --suite quick --json --first-outcome` —
  pre-change drift baseline (moot after the revert; recorded in
  `/tmp` during the session, not committed).
- A temporary stdio UCI driver script (not committed; the protocol is fully
  described below and is trivially reproducible with `printf | stockfish`).

### Engine build & reproducibility

- Build line: `make -C libs/Fairy-Stockfish/src build ARCH=armv8 COMP=gcc -j4`.
  **Deviation from the plan:** the plan says `ARCH=x86-64-modern`, but this
  container is **aarch64** (`uname -m` → aarch64, g++ 16.2.1 targeting
  aarch64-redhat-linux); x86-64 targets fail at compile with
  `unrecognized command-line option '-msse'` etc. `armv8` (generic ARMv8-A,
  popcnt+NEON) is the closest supported target.
- Binary: `libs/Fairy-Stockfish/src/stockfish` (Fairy-Stockfish names the
  binary `stockfish`, not `fairy-stockfish` — a second, benign plan
  deviation that any plan3 must inherit).
- sha256: `cf65ef2f30ba0026922214928aa4899175611d282a49cc6ba2a88010df60309b`
- Reported version string: `Fairy-Stockfish 130926 by Fabian Fichter`.
- Eval: `info string classical evaluation enabled` — NNUE embedding is off
  (`-DNNUE_EMBEDDING_OFF`), so all measurements below are classical
  (handcrafted) eval, per the plan's NNUE exclusion.
- Query settings (identical for every engine run below):
  `setoption name UCI_Variant value atomic`, `setoption name Threads value 1`,
  `setoption name Hash value 256`, `setoption name MultiPV value <K>`,
  `position fen <FEN>`, `go nodes <N>`; parse every `info … multipv k … pv …`
  line (last PV per rank wins) and stop at `bestmove`.

### Sanity check (Phase 0 step 1)

The stress FEN `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21` is
accepted under `UCI_Variant=atomic` (`d` echoes it back) and both sides
agree on movegen: `list_legal` reports 41 legal moves, `go perft 1` reports
41. FEN compatibility confirmed.

## Line quality sweep (Phase 0 step 2)

### Stress case (`make stress` FEN; inherited baseline: first-outcome
53.8 s / 249,480,478 child evals, 13,907,467 nodes)

| engine nodes | engine wall | PV length | verify result | verification cost |
|---|---|---|---|---|
| 1M   | 0.82 s  | 1  | rejected instantly: final position not decisive (a 1-ply PV cannot end in a terminal) | ~0 |
| 10M  | 6.60 s  | 19 | **not verified**: verification ran its full 600 s budget; defender ply 8's bounded refutation search (reply a5a4, depth ≤ 11) ended resource-cut (`Draw` at the deadline), reported `is_ppv: false` | > 1.02B child evals, 600 s |
| 100M | 68.78 s | 17 | **not verified**: full 900 s budget; defender plies 6–16 verified, ply 4 (a5a4, depth ≤ 13) ended resource-cut | > 1.44B child evals, 900 s |

Reading: the engine's PVs end in *genuine* terminal positions (the parity
arithmetic accepts them and verification proceeds), but earlier defenses
escape within the bounded remaining length. These are cooperative-horizon
lines — exactly the belief-vs-fact gap `dfpn/plan7` warned about, now
measured at root scale. Deeper engine thought (10M → 100M) changed the
root move (g3g4 → b1b8) and shortened the PV but did not make the line
provable. Note also that engine time alone at 100M nodes (68.8 s) already
exceeds the entire unguided first-outcome baseline (53.8 s).

### m22 control (`4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22`;
inherited baseline 3.1 s / 858,117 nodes / 14,156,269 evals)

| engine nodes | engine wall | PV length | verify result |
|---|---|---|---|
| 1M  | 0.77 s | 1  | rejected instantly (not decisive) |
| 10M | 6.49 s | 15 | **not verified within 300 s**: defender plies 4–14 all proven lost, ply 2's refutation searches (11 replies, depth ≤ 14) exceeded the budget |

This is the control the plan required ("guidance must not *lose* to 3 s on
easy positions"): guided cost = 6.5 s engine + 300 s failed verification +
3.1 s fallback ≥ 310 s vs 3.1 s unguided — **~100× worse**. Structural
reason: a 15-ply candidate forces ~7 defender plies × up to 11 replies,
each a separate bounded proof search, re-paying work the 3.1 s unguided
solve deduplicates inside a single search.

### dec-suite sample (6 of the hardest-looking cases; `go nodes 10M`)

| case | engine wall | PV len | verify result | verification cost |
|---|---|---|---|---|
| dec37 | 4.56 s | 8  | is_ppv: true | 0.044 s / 2,361 nodes |
| dec38 | 3.46 s | 12 | is_ppv: true | 0.551 s / 897,752 nodes |
| dec40 | 3.00 s | 10 | is_ppv: true | 0.474 s / 768,240 nodes |
| dec41 | 4.03 s | 10 | is_ppv: true | 0.049 s / 1,597 nodes |
| dec42 | 1.16 s | 8  | is_ppv: true | 0.151 s / 134,072 nodes |
| dec44 | 5.26 s | 10 | is_ppv: true | 0.113 s / 141,149 nodes |

On tactical positions the machinery works exactly as hoped — engine lines
verify at near-zero solver cost. But the engine query costs 1.2–5.3 s while
these positions unguided-solve within the 5 s default budget, so guided
cost ≈ engine time + verify ≥ unguided cost. No win on easy positions; the
lever's value was supposed to be the hard class, where it failed.

## Fallback cost accounting (Phase 0 step 3)

Guided-mode total = engine wall + verification work + full unguided solve:

- stress @1M: 0.8 + 0 + 53.8 ≈ **54.6 s** (1.01× baseline, and it did not
  solve — the candidate was rejected before any proof)
- stress @10M: 6.6 + 600 + 53.8 ≈ **660 s** (12.3× baseline wall; > 1.0B
  verification evals + 249.5M fallback evals ≈ 5× baseline evals)
- stress @100M: 68.8 + 900 + 53.8 ≈ **1023 s** (19× baseline wall)
- `--multipv 3` ladder @10M (ladder candidates recorded: a 21-ply line with
  first move d6e5, a 14-ply g3g4 line that diverges from the MultiPV-1 line
  at ply 3, and a 2-ply line): 3 sequential verification attempts (the
  g3g4-class attempt is the same subtree family as the 600 s measurement
  above) + fallback ⇒ strictly worse than K=1; the ladder can only add cost
  when every candidate fails, which is the measured outcome on the target
  class.

## Go/no-go (Phase 0 step 4)

- **(a) FAIL.** No candidate verifies at any node budget ≥ 1M (1M: 1-ply PV
  fails the terminal check; 10M/100M: budget exhausted with the defense
  unproven). The ≤ 50%-of-baseline total-cost bar cannot be met — engine
  time alone at the only budgets that produce multi-ply PVs (≥ 6.6 s, up to
  68.8 s) approaches or exceeds the 53.8 s unguided baseline.
- **(b) FAIL.** m22: guided ≈ 310 s vs 3.1 s unguided. dec sample: success
  rate 6/6 ≥ 50%, but average guided cost (engine + verify) does not beat
  unguided cost, and the stress success rate is 0/3.
- **(c) PASS.** No candidate ever passed verification on a non-proof line:
  both stress refutations and the m22 failure are rejections; the deliberate
  defender-perturbation spot-check was demonstrated (verified-PPV fixture
  line with the defense g8f7 swapped to g8h7 → `is_ppv: false`,
  *not a longest defense (depth 3, longest 5)*). The verifier's
  false-positive direction was never observed.

Decision per the plan: write this report, close backlog #2's verification
lever with the negative result, leave ordering guidance (2a) open and
re-ranked, stop. No code to revert (Phase 0 changed nothing); the
drift protocol is vacuously satisfied (tree byte-identical to pre-plan2,
`make test` green).

## Problems encountered

1. **aarch64 container.** The plan's `ARCH=x86-64-modern` build line cannot
   work here; `armv8` was used. Any future engine work in this repo must
   record the ARM build (or the launcher's architecture) explicitly.
2. **Binary naming.** Fairy-Stockfish produces `src/stockfish`; the plan's
   default `--engine-path libs/Fairy-Stockfish/src/fairy-stockfish` would
   have been wrong.
3. **Resource-cut Draw ambiguity in the pre-plan2 verifier.** A bounded
   refutation search that hits its deadline returns `Draw`, which
   `verify_ppv` (correctly, per its contract) reports as `is_ppv: false` —
   but it is *unproven*, not *refuted*. Both failed stress verifications are
   of this kind. This does not affect soundness (both are fallback
   conditions), but reports should not describe them as "the line was
   refuted". If the verification machinery is ever revived, distinguishing
   `ExitReason::Complete` draws from resource-cut draws (as
   `VerifyDefect::OutOfBudget` vs `ReplyNotLost`) would make reports honest
   at zero soundness risk.
4. **Session-scale cost.** Two verification attempts alone consumed
   ~25 minutes of CPU; the sweep is bounded by the 600/900 s verification
   budgets, not by curiosity.

## Parallel-prototyped and reverted code

While engine runs were in flight, task 2 (`src/verify.rs` extraction from
`examples/verify_ppv.rs`) was prototyped: public `verify_line` /
`VerifyDefect` / `parse_uci_line` with per-rule mutant unit tests (9 tests:
accept a known PPV, refute illegal move / premature terminal / non-decisive
final / swapped defense / budget exhaustion / zero deadline), the example
refactored onto the library with `tests/verify_ppv.rs` green (7/7). On the
no-go decision this was reverted per the plan's stop contract (the tree
must stay pre-plan2; an unused public module would be surface without a
consumer). If plan3 (ordering guidance) or any future consumer wants the
extraction, it is a mechanical re-application; the design notes are in
`plan2.md` and the one deviation worth keeping (resource-cut classification
from problem 3) is described above.

## Unresolved parts / missing tests

- The plan's implementation tasks 3–9 (PvStatus::VerifiedPpv, `--guide-moves`,
  `sf_guide`, property tests, soundness gates, drift, stress measurement of
  the guided mode) were intentionally not executed — they are contingent on
  the go decision.
- NNUE follow-up (plan §Design): no atomic variant net was investigated or
  bundled; the build runs classical eval. If a stable, hash-recorded atomic
  net source surfaces, it would *not* change this no-go (the failure mode —
  cooperative-horizon lines that fail exhaustive defense verification — is
  structural, not eval-strength-bound), but it could matter to lever 2a.
- Criterion (b)'s "expected fallback share" over the dec suite was computed
  from the measured K=1 failures; the multipv-3 ladder was recorded
  (candidates listed above) but only costed analytically, since every
  ladder rung is dominated by the already-measured failures.

## Next steps

- Backlog #2 stays **open for the ordering-guidance lever (2a)** only,
  re-ranked; it opens as plan3 if sized. Note the weakened premise: the
  engine's stress root move is unstable across budgets and modes (g3g4
  @MultiPV-1/10M, b1b8 @MultiPV-1/100M, d6e5 @MultiPV-3/10M) — seeding
  ordering from a candidate line must justify itself against the lean
  initiative's measured ordering floor, not against the verification
  lever's failed economics.
- Backlog #3 (clock-pressure ordering signal) and #4 (parallelism) are
  untouched by this result and remain the higher-ceiling levers for the
  deep-conversion class.
