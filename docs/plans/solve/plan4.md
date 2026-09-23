# Plan 4: Sharpness-first pilot (feasibility test of the rescope option)

Initiative: `solve`. Follows `report3.md` §4's recommendation and the
2026-09-23 session decision to *test* report2 §9 option 2 before
pivoting. The option: drop the startpos-value goal in favor of
exhaustively proving (and artifact-ing) the sharp class — cheap
decisive proofs within a fixed, well-defined universe — using only the
existing pipeline (search → snapshot → reconstruct → replay-validated
dump). No plan so far measured whether that class has *enumerable
material*: plan1 found 26 tactical positions but only by walking one
PV line per seed (self-play ladders); the siblings were never probed.

**No product changes.** The whole plan is a black-box driver
(`measurements/plan4/`) over the unmodified release binaries. Final
task: `report4.md`, which must contain the full option comparison
(close-with-artifact / sharpness-first / step-by-step narrowing / stop)
with all measured data on the table — that comparison is the
decision input the user asked for.

## 1. Background (self-contained)

- plan1: bimodal work landscape — 48/75 quiet cost runs censor at a
  20.5–46.6M-node / 120 s plateau; the only uncensored class is cheap
  tactical proofs (26 positions, 1–4,749 nodes, wins in 1–9 plies).
  Known cheap opening content: after 1.e4 e5 White wins in 9; after
  1.e4 c5 in 5; after 1.Nf3 d5 in 5. The ladders never measured any
  sibling of these lines.
- plan2: cross-run TT substrate measured empty (NO-GO).
- plan3: a quiet root does not finish at 2 h (linear ~198k nodes/s,
  both 128 MB and 1 GB TT censor; memory-mediated finishability ruled
  out). The ±1-ply substrate gate fired GO but only on the tactical
  class (the pre-registered selection rule structurally cannot sample
  a censored position).

## 2. Task A — full-width sharp-class density screen (primary)

Universe: every position at ply 1 (all 20 first moves) and ply 2
(all legal replies to each first move), enumerated from startpos with
`examples/list_legal` + `examples/replay` (clock semantics preserved by
construction). ~450 ply-2 positions expected (no captures exist at
plies 1–2).

Protocol: per position one screen solve
`atomic_solver --fen <FEN> --timeout 8 --first-outcome --tt-dump-path
…` (plan1's steer budget, so results are comparable to the ladder
data), stdin DEVNULL, raw stdout/stderr captured, max-RSS recorded.
Runs strictly sequential, defaults otherwise. Decided = exit_reason
Complete (any outcome). Censored positions are recorded as unresolved
at the screen budget; a seeded random sample of 5 censored ply-2
positions is promoted to a 120 s cost run to verify the plan1 plateau
transfers at the 8 s scale (if any completes at 120 s, the screen
under-proves and that is reported as such).

Metrics (pre-registered):
- `density(ply2)`: fraction of ply-2 positions decided at 8 s.
- `refutations`: per first move, the count and list of replies whose
  8 s solve decides (win for White = the reply is refuted; win for
  Black = the reply is a cheap winning reply; draw = cheap draw) —
  this is the *opening-content yield* of the artifact.
- first moves with ≥ 1 decided reply.

### Pre-registered gates (fixed before any run)

- **VALIDATE (hard, correctness first)**: every decided position in
  Tasks A–C that is reconstructed must print `validate: ok` with the
  search's outcome. Any mismatch stops interpretation — the artifact
  backbone has a bug and nothing else in this plan counts.
- **DENSITY**: `density(ply2) ≥ 10%` → the sharp class has enumerable
  material at the sharpest plies; a scaled sharpness plan is worth
  drafting. `< 2%` → the class is thin at ply ≤ 2 and breadth
  enumeration there is dead; between → judgment call.

## 3. Task B — sibling-thickness probe (secondary)

The ladders found wins by PV-walking; breadth enumeration only adds
value if wins are *not* isolated. For two known tactical roots —
e4e5 p2 (`rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0
2`, win in 9) and d4d5 p32 (`rnbqkbnr/4pppp/8/P2P4/4PPPP/2p5/8/
2BQKBNR w Kkq - 0 17`, win in 7) — solve **all** legal moves at the
root (8 s screen, same protocol): how many siblings also decide
cheaply, and with what outcomes. Metric: `sibling_density` per root.
(A win's PPV needs only one move; this measures whether enumeration
multiplies content or the class is measure-zero.)

## 4. Task C — artifact build + validation census (the deliverable test)

Build the sharpness artifact exactly as the rescope would, over the
union of:
- all decided positions from Tasks A and B,
- the 26 plan1 ladder tactical positions (re-solved at 120 s — each
  completes in ≤ 4,749 nodes; deduped by FEN against Task A/B).

Per position: reconstruct_pt from the search snapshot; `validate: ok`
required. Census: total positions, total tree nodes, unique Zobrist
keys (via `examples/pt_keys`), duplicate-key parity conflicts across
the union (report2 §5 flagged 884 in one tree — the artifact must key
by position, not node id), total find+verify wall, bytes. Deliverable:
`artifact/index.json` (FEN, move path, outcome, DTM length, nodes,
validate, tree file, key count) + per-position binary trees. This is
a pilot of the artifact, not the artifact itself — scope stays inside
`measurements/plan4/`.

## 5. Tasks

1. Harness `measurements/plan4/` (`spike4.py`: env / screen / promote
   / siblings / artifact / status; README command table; reuses the
   plan2 snapshot reader verbatim; plan3's max-RSS method).
2. Run Task A (screen, then the seeded promotion sample), apply the §2
   gates.
3. Run Task B, apply §3 metrics.
4. Run Task C, apply the VALIDATE gate; prune snapshots per convention
   (counts live in state JSONs).
5. Write `report4.md`: gate verdicts, density/refutation/sibling
   tables, artifact census, **the full option comparison with all
   accumulated data (reports 1–4) and a concrete recommendation** —
   the decision input requested.

## 6. Non-goals

- No claim about the startpos value; no completeness claim beyond the
  screen budgets; censored = unresolved at budget, never a value.
- No product changes, no `--tt-load-path`, no proof-tree layer
  changes, no benchmark/drift-gate impact.
- No scaled sharpness campaign (that is the successor plan, only on a
  DENSITY GO plus user sign-off).

## 7. Budget

~470 screen runs × 8 s ≈ 63 min + promotions 10 min + siblings
~60 × 8 s ≈ 8 min + artifact build (≤ 60 cheap reconstructions) ≈
minutes. Total ≈ 1.5–2 h sequential on the reference sandbox; nothing
else runs concurrently.
