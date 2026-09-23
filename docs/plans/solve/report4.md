# Report 4: Sharpness-first pilot — the cheap class is real, enumerable, and saturates; option comparison for the pivot

Initiative: `solve`. Executes `plan4.md` (feasibility test of the
sharpness-first rescope, report2 §9 option 2). Runs of 2026-09-23
06:39–08:0x UTC, strictly sequential. Harness:
`measurements/plan4/spike4.py` + command table in
`measurements/plan4/README.md`; environment `env.json` (4-core quota,
8 GiB cgroup, release build at `8e46e52` + the plan4 `pt_keys` fix,
§5). **No product changes** (the one source fix is the `pt_keys`
measurement helper, §5 — campaign tooling, not solver surface).

## TL;DR

- **DENSITY gate: GO.** 55 of 400 ply-2 positions (13.75%) are cheap
  forced wins at the 8 s screen (≥ 10% threshold) — all 55 are White
  wins, i.e. a *refutation book of Black's replies*. Concentration is
  extreme: 1.Nf3 refutes 17/20 replies, 1.e4 13/20, 1.e3 13/20,
  1.Nh3 11/20; **every d-pawn/c-pawn/king-pawn-side first move
  refutes nothing** (0/200). The class is one-sided and
  structurally patterned (Qh5/Bb5/Nxe5/Ng5 sorts), not noise.
- **VALIDATE gate: PASS (hard, 100%).** All 95 artifact members
  reconstruct with `validate: ok` and matching outcomes. The artifact
  comprises **12,592 tree nodes = 11,669 member-unique keys =
  10,177-key union** (position-keyed, cross-tree transpositions
  deduped), DTM 1–13, **82,644 bytes total**, find+verify wall
  14.6 s + 2.4 s.
- **The cheap class saturates.** The screen's 345 censored ply-2
  positions and all 20 ply-1 positions are the plan1 plateau class
  (promotion sample: 5/5 still censored at 120 s, ~20–23.5M nodes).
  The ladder's deep tactical wins (p22–38) are reachable only through
  censored quiet territory — the artifact built here is
  **self-contained at startpos** and further growth requires exactly
  the quiet-class mechanism that plans 1–3 measured unavailable.
- §4 compares all four options on all accumulated data; my
  recommendation: **adopt the rescope in its measured, saturated
  form — freeze the plan4 artifact as the initiative's deliverable,
  record the quiet class as sandbox-unreachable, close.** Final call
  is the user's.

## 1. Task A — full-width density screen

420 runs (20 first moves + 400 replies), 8 s screen, `--first-outcome`,
snapshots dumped, max-RSS recorded; ~63 min sequential.

- **Ply 1: 0/20 decided** (all censored at 1.6–1.8M nodes; plan1's
  120 s cost runs measured the same positions at 21.7–24.3M nodes).
- **Ply 2: 55/400 decided (13.75%)** — outcome `win` for all 55 (no
  cheap Black wins, no cheap draws). Decided node quantiles: 82 /
  133 / 704 / 1,762 / 1,047,199. Only two positions needed > 40k
  nodes (1.e3 g6 1.05M, 1.e3 h5 0.90M — found just inside the screen
  budget; their proof trees are 1,044/1,112 nodes).

Refutation book (decided replies per first move):

| first | refuted | example (reply? → winning move) |
| --- | --- | --- |
| 1.Nf3 | **17/20** | 1...a6/h6/Nh6? 2.Nxe5! (82 n); 1...d5? 2.? (194 n, known); undecided: d6, e5, f6 |
| 1.e4 | **13/20** | 1...g5? 2.Bb5! (158 n); 1...Nc6? 2.Qh5! (316 n); 1...e5? 2.Qh5! (4,749 n, known — but see §2: 2.Bb5! also wins); undecided: d5, d6, e6, f6, Nf6, g6, h5 |
| 1.e3 | **13/20** | same patterns; 1...g6? / 1...h5? win at ~1M nodes |
| 1.Nh3 | **11/20** | 1...Nf6? 2.Ng5! (84 n) |
| 1.c3 | **1/20** | 1...g5? 2.Qa4! (20,737 n) |
| all 15 others | **0/200** | — |

Structural reading: the sharp class requires the a2–g8 / h5–e8 queen–
bishop diagonals that only e-pawn and Nf3/Nh3 openings open; the
d/c-pawn systems (the real atomic theory mainlines) sit entirely in
the plateau class. 52 of the 55 refutations are new content (plan1
knew 3: e5, c5, Nf3-d5).

Plateau transfer: the pre-registered 5-position seeded promotion
sample (8 s-censored → 120 s) stayed censored (19.6–23.5M nodes vs
1.4–1.7M at 8 s) — the 8 s screen's "unresolved" is the same class
plan1/plan3 measured bottomless.

## 2. Task B — sibling thickness

All legal moves at the two pre-registered tactical roots (8 s screen;
child `loss` = the sibling move wins for the mover):

| root | moves | winning moves | losing moves | censored |
| --- | --- | --- | --- | --- |
| e4e5 p2 (W to move) | 29 | **2** (Qh5⁺, Bb5⁺) | 13 | 14 |
| d4d5 p32 (W to move) | 32 | **2** (d6, Bb5) | 5 | 25 |

Wins are thin but not isolated: every tactical root probed has a
second winning move, and `Bb5` is the alternative at both. Breadth
enumeration adds ~1 extra PPV per root — PV-walking plus a one-sibling
check captures essentially all of it.

## 3. Task C — artifact build + validation census

Union: 55 screen + 20 sibling decided positions + 25 plan1 ladder
tactical positions after FEN-dedup (the 3 known p2 refutations were
re-derived identically by the screen — plan1 node counts match
exactly, a free drift check) = **95 members**.

- **95/95 `reconstruct_pt` → `validate: ok`, 95/95 outcome match.**
- 12,592 tree nodes; 11,669 member-unique keys; **10,177-key union**
  (421 pairs of trees share keys — e.g. the 1.e4 e5 refutation tree is
  largely contained in the Qh5-line tree; 34 members have internal
  transposition duplicates, the known finalize-on-transposition
  property report2 §5 noted — now measured with position-true keys).
- DTM distribution: 1×9, 3×22, 5×37, 6×1, 7×7, 8×2, 9×12, 10×1,
  11×3, 13×1.
- Total cost: find wall 14.6 s + verify wall 2.4 s; **artifact bytes
  82,644**. Deliverable: `measurements/plan4/artifact/index.json` +
  95 binary trees.

## 4. The option comparison (all data on the table)

| | **A. Close with artifact** | **B. Sharpness-first rescope (piloted)** | **C. Step-by-step narrowing** | **D. Stop / park** |
| --- | --- | --- | --- | --- |
| Startpos value | no | no (by construction) | premise was per-step finishability | no |
| Measured status | the artifact now exists (§3) | pilot passed both gates | plan3: root NOT finishable at 2 h (1.43G nodes, linear; 8× TT worse); ±1 substrate GO only on the tactical class | plans 1–3 negative results stand |
| New work needed | format/publish the §3 artifact (~1 session) | extend screen deeper? — **measured saturated**: deeper reach requires crossing the 345-censored-p2 / 20-censored-p1 moat, i.e. the quiet mechanism | needs quiet-class ±1 substrate (measured empty at ±2, unmeasurable at ±1 with PV rules) + per-step finishability on censored children | none |
| Mechanism risk | none | none (pipeline proven 95/95) | high — three plans measured its premises false or empty | none |
| Deliverable size | 10,177 proven positions, 83 KB, verifiable | same + a written refutation book (§1 content is real opening knowledge: 1.Nf3 refutes 17/20 replies) | hypothetical global artifact | nothing |
| Cost so far | — | ~1.5 h (this plan) | n/a | — |

Key structural fact that reshapes the choice: **A and B have
converged.** The sharpness pilot *is* the artifact that option A
wanted to close with; and the rescope's growth path is measured
blocked by the same plateau that motivates closing. The ladder's deep
tactical wins (p22–38) sit on the far side of censored territory —
they are in the plan1/plan2 record but are **not connectable** to the
startpos-rooted artifact by proven paths.

## 5. `pt_keys` off-by-one (found by Task C, fixed)

plan4's key census exposed that `examples/pt_keys` printed each node's
hash *before* applying the node's incoming move — every non-root node
reported its parent's position key (the win-in-1 tree's two nodes
printed one key). The validator (`proof_tree::validate`) applies the
incoming move before evaluating, so node key = position after its
incoming move; `pt_keys` now does the same (one-block fix, re-verified:
root/child keys distinct; artifact census re-run with position-true
keys). Impact: the plan2 **primary** M1 metric used TT-snapshot keys
only — unaffected; the **secondary** tree-key-coverage numbers
(`m1.json`, report2 §5, incl. the 884-duplicate observation) used
shifted keys — qualitatively robust (the shifted set stays inside the
same tactical region), numerically not exact; a caveat note is added
to `measurements/plan2/README.md`. No solver-surface code touched.

## 6. Deviations, problems, missing tests

- Deviation: reconstruct timeout raised 120 s → 900 s (verify wall is
  not a small constant on top of find wall; the two ~1M-node wins
  must not be verification-capped — plan1 §combined-metric lesson).
  In the event the largest verification took 0.42 s (proof trees are
  ~1000× smaller than the searched node count).
- Deviation: `pt_keys` fixed (§5) — measurement tooling, but a source
  change beyond the plan's "no product changes"; justified by the
  VALIDATE/census task and documented here and in plan2's README.
- `dtm_length` was not parseable from the screen stdout (outcome
  token only); recovered from the proof trees' root depths.
- Minor: the screen's two ~1M-node wins completed within 8 s by
  margin of <1 s; at slightly slower hardware they would censor and
  be recorded unresolved — the artifact's membership is
  hardware-budget-dependent at the tail (documented; the core class
  ≤ 4.7k nodes is robust).
- No test-suite impact (no product code).

## 7. Recommendation

**Amended post-session (2026-09-23, user decision): the `solve`
initiative is the project's umbrella and does NOT close — nor is it
dormant (it has pending plans).** What closes is the plan1–4 *campaign
line*: the measured mechanisms (PV-ladder, SSFP anchors, step-by-step
narrowing at sandbox budgets) are dead, and the sharpness-first
rescope, as piloted, saturated. The initiative is **re-scoped onto the
return path** (status: active, plans 5–7 pending; roadmap in
`../initiative.md`), with the plan4 artifact frozen as the interim
deliverable:

1. This artifact + index are the initiative's canonical interim
   record; a human-readable refutation book (§1 with PVs) is a
   follow-up documentation task.
2. The startpos value remains the goal; the quiet class is recorded as
   measured **sandbox-unreachable**, not unreachable — plan3's linear
   node rate makes a larger-machine sizing run meaningful.
3. The roadmap (recorded in `../initiative.md`): plan5 = GHI-correct
   job-level campaign design spike (assets mined: `pdfpn-2010`,
   `pns-pdfpn-2025`, `ppn2-2011`, `ghi-journal-2005`); plan6 = off-
   sandbox resource sizing (20–24 h d4d5-p2 rerun); plan7 = campaign
   product surface (`--tt-load-path` to the plan5 soundness spec,
   resume, store v1) — campaign return only on both gates.
4. The step-by-step narrowing scheme stays closed: both premises
   measured (finishability: false at the root, 2 h; substrate: empty
   where it would matter).
5. Status rule: `solve` falls to **dormant** only if plan5 and plan6
   both fire negative (no open items, reopeners only: mechanism
   innovation or resource step-change); it never closes while the
   startpos value is the project goal.
