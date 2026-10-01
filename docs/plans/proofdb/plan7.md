# Plan 7: batch 3 — the shipped-default control vs the ledger-union arm

Initiative: `proofdb`. Executes report6's next-steps 1–2 as one
decision-bearing session: the third harvest batch runs as a two-arm A/B
over the standing post-plan6 state — **arm 1 (control)**: the shipped
default (rationed expansion, `reserve_share 0`, `layer_visit_cap 24`) from
the standing ledger as-is; **arm 2 (union)**: the identical config from a
*ledger-unioned* copy that recovers the censor knowledge report6's
standing-ledger advance dropped (finding 4). The measured question is the
union's marginal value as the standing-state merge mechanism — a mechanism
item 6 (parallel per-worker ledgers) needs anyway. Report6's finding 1
(caps ration breadth, not depth) is *accounted for* in the sizing and the
closure rule, not implemented here: the depth-rationing lever is its own
plan (§6).

Small `examples/`-side tool + docs only; the product solver (`src/`), DB
schema, and spec are untouched; no in-session re-propagation (plan4's
simplification stands). Per repo convention the final task is `report7.md`.

## 1. Background (self-contained)

**State after plan6 (verify, never assume).** The standing working layer:
`data/proofdb.db` sha256 `670e19e3…` (35,055 nodes, 197 shards, manifest
digest `37c1f5b2…` — unchanged since plan3) and `data/proofdb_work.json`
sha256 `058a6202…` — arm C's post-run ledger, 4,569 records = 225 censored
(`passes_failed` 1) + 4,344 fresh (pass 0):

| ply | fresh | censored | | ply | fresh | censored |
|----:|------:|---------:|---|----:|------:|---------:|
| 0 | 0 | 1 | | 4 | 469 | 24 |
| 1 | 0 | 20 | | 5 | 456 | 3 |
| 2 | 190 | 153 | | 6 | 64 | 0 |
| 3 | 3,165 | 24 | | | | |

**Why batch 3 is decision-bearing, not ritual (the sizing note report6's
kickoff asked for).** Five 300M-scale sessions have run to date (batches
1–2, plan6 arms A/A′/B/C); every one yielded **0 facts**, and the plan6 A/B
settled that neither the ladder nor uncapped breadth changes that. Two
measured findings shape this batch:

- **Finding 4 (re-censor waste).** Plan6 §4 advanced the standing ledger to
  the *winning arm's* post-run state (arm C), dropping arm A's censor
  knowledge: A visited 75 ply-2 nodes (73 fresh records + 2 open rows) that
  C's ledger holds as fresh or missing. The ply-2 fresh pool (190) therefore
  contains ~51 A-known-quiet nodes (censored at 4M by A, dropped to pass 0);
  with the ply-2 layer cap at 24, the control arm spends its *entire ply-2
  ration* re-censoring known-quiet nodes — **≤ 24 × 4M = 96M evals, ~32% of
  the session cap, plus the re-exposure of their already-once-exposed
  children** (~500 ply-3 records). Bounded by the layer cap, but material.
- **Finding 1 (caps ration breadth, not depth).** Layer visit caps made arm
  C reach ply 5 (24/24/24/3 across plies 2–5) — but each censor re-seeds the
  next layer at number 1 (~19–21 fresh exposures per censor, measured
  consistently across batches 1–2 and all plan6 arms), so sessions never
  complete a layer or an AND node and yield ≈ 0 facts. Deeper reach per se
  is not the yield lever; this plan does not chase it (§6), but its closure
  rule (§4.6) makes the next plan mandatory.

**The union insight.** Ledger records are *selection state only*
(`path`, `work_done`, `passes_failed` — no outcomes, no facts), so the
dropped censor knowledge is recoverable losslessly: the committed plan6 arm
snapshots (`ledger_snapshot_A.json` sha `7fd92ea2…`, `ledger_snapshot_B.json`
sha `9aa0c326…` — byte-identical to report6's post-run states) union with the
standing ledger under a conservative max-rule. Facts live in shards; nothing
here touches the soundness surface. The same N-way union is exactly what
item 6's per-worker ledger merge needs, so the tool built here is a
prerequisite measurement, not a one-off.

**Pinned censuses (read-only probes on throwaway `/tmp` copies,
2026-10-01, pre-registered here).** Both arms start from staging copies of
the standing DB + manifest + the arm's ledger (fresh TT, cap 300M, base 4M,
root = startpos):

- **Arm 1 (standing ledger):** `pns: rows 35055 (open 75), ledger records
  4559 (4559 new, 0 dropped as decided); exclusions proven-ancestor r26/l0,
  implied-win r0/l0, implied-loss r0/l0; jobs 4608 (rows 49, ledger 4559);
  base budget 4000000; cfg reserve_share 0, layer_visit_cap 24,
  interleave_k 4, eligibility no-virgin-child, rung_growth geometric,
  max_rung_passes 3, rotation fewest-passes`
- **Arm 2 (union ledger, 5,950 records):** `ledger records 5938 …; jobs
  5987 (rows 49, ledger 5938); …` — otherwise identical modulo the record
  counts (12 union records merge into open rows via the lineage gate,
  same rule as always).

Union composition (measured in the probe): 5,950 = 4,569 standing + 1,381
new paths from A (arm A's re-exposed ply-3 children of its 51 non-C ply-2
censors — the recovered exposure) + 0 new from B; 49 standing-fresh
records upgraded to pass 1 from A, 8 from B (B's rung censors: root pass 2,
seven ply-1 nodes pass 2, c2c3 pass 2 — all in the number-≥2 pool, no
expansion effect). Union ply-2 split: fresh 141 (was 190), censored 204
(was 153).

## 2. The ledger-union mechanism (normative, pre-registered)

**Rule (per path key):** `passes_failed = max(inputs)`;
`work_done = max(inputs at the winning pass)` — equivalently: take the
record with the highest `passes_failed`, ties broken by the higher
`work_done`. Total, deterministic, order-independent, associative
(N-way = pairwise iteration), idempotent.

**Properties and gates (fixed now):**

1. **Selection state only.** The ledger schema has no outcome field; a
   union cannot create, destroy, or contradict a fact. Soundness surface:
   empty (the shard/manifest layer is untouched by §2).
2. **Coverage:** for every input and every path in it, the union's record
   has `passes_failed` ≥ that input's (monotone knowledge merge). Gate
   check, not aspiration.
3. **Determinism:** the union run twice (any input order) → byte-identical
   output; idempotent (union(u, inputs) = u).
4. **Base compatibility is the operator's contract, checked by pinned
   digests:** the ledger format carries no base stamp, so this plan pins
   the exact input files and their sha256 digests (§1); the union task
   verifies them before merging. A future base-stamp field is a report7
   finding/next-step (item 6 will want it), not a format change here.
5. **Scope:** new standalone example `examples/proofdb_ledger_union`
   (`--out <file> <inputs…>`, N ≥ 1; N = 1 normalizes/validates only),
   public lib API untouched, product solver untouched.

## 3. Deliverables

- **D1 — union tool**: `examples/proofdb_ledger_union.rs` (≤ 10 KB) +
  tests in `tests/proofdb.rs` (rule edges: pass upgrades, work tie-breaks,
  disjoint paths, N-order independence, idempotence, coverage property,
  empty/1-input normalization).
- **D2 — the A/B** under `measurements/plan7/`: per-arm census JSON + the
  union ledger + post-run ledger snapshots + `env.json` + `README.md`
  (provenance, command table, verdicts, the re-censor accounting of §4.2).
- **D3 — `report7.md`**: arm verdicts, the union's marginal value per
  §4.5, the re-censor waste measured (finding 4), gate verdicts, findings,
  and the §4.6 closure decision for plan8.

## 4. Pre-registered arms and decisions (fixed before any run)

Both arms: `--policy breadth-pns` (the shipped default; no `--pns-config`
— the compiled defaults are arm C's), base 4M, cap 300M, root = startpos,
staging copies, fresh TT; replayed per H5. Validated facts from either arm
promote into the standing shard set after the verdict (facts are
arm-independent truth); a same-path outcome contradiction between arms
aborts the session (the standing soundness tripwire).

1. **Arm 1 — shipped-default control from the standing state.** Census as
   pinned (§1). Expectations: 75 visits; ply distribution 24/24/24/3
   across plies 2/3/4/5 (identical shape to plan6 arm C — the standing
   ledger is C's post-run state advanced only by the ledger advance, and
   the ply-2 fresh head is A-known); **re-censor count 24** (all ply-2
   visits land on A-censored nodes; accepted range 20–24), re-censor waste
   ≤ 96M; ≈ 0 facts (sixth consecutive — any fact is reported as data, not
   a gate failure); ledger growth ≈ +1,400–1,600 records (cascade
   exposures ~20/censor, of which ~500 ply-3 are re-exposures of
   A-once-known records).
2. **Arm 2 — union arm.** Same config from the §2 union ledger. Census as
   pinned. Expectations: 75 visits; ply distribution 24/24/24/3; **0
   re-censors** (the 51 A-known ply-2 nodes sit at pass 1 = number-2 pool;
   the 141 fresh ply-2 head is genuinely virgin); ≈ 0 facts; ledger growth
   ≈ +1,400–1,600 (all new knowledge — no re-exposures).
3. **The re-censor metric (both arms, computed in the census assembly):**
   a job is a *re-censor* iff its path had `passes_failed = 0` at session
   start **and** appears censored in any committed plan6 arm snapshot
   (`ledger_snapshot_{A,B,C}.json`). This is finding 4 made measurable.
4. **No mid-run tuning** (plan6 §4.6 rule stands verbatim); the census
   `cfg` echo + `kind` field make both transcripts self-documenting.
5. **Decision rule (the union's marginal value), fixed now:** the union
   mechanism ships — the standing `data/proofdb_work.json` advances to
   **arm 2's post-run ledger** (over the union base) and
   `proofdb_ledger_union` becomes the standing-state merge tool (the
   N-way primitive item 6 builds on) — **iff** arm 2 has strictly fewer
   re-censors than arm 1, facts ≥ arm 1, and max ply reached ≥ arm 1.
   Expected to fire on all three (24→0, 0=0, 5=5). If it does not fire,
   keep the current advance rule, standing ledger advances to arm 1's
   post-run state, and the union result is investigated before any further
   use (a model defect per §1 — stop, do not loosen).
6. **Closure rule for plan8 (fixed now):** this is the sixth consecutive
   0-fact-expected 300M session. If both arms yield 0 facts (expected),
   **plan8 must change the yield outlook** — the depth-rationing lever
   (finding 1: fresh-exposure cap or ply-front bias) or a fact-yield-
   oriented selection change — pre-registered with its own design dialogue;
   *another identical default batch is not a valid plan8*. If either arm
   yields facts, the closure rule re-opens (the default is not inert and
   the yield question must be re-asked against measured data).

## 5. Pre-registered gates (fixed before any run)

Plan5/plan6 gates, adapted:

- **H1 integrity:** the pinned census match per arm (§1 — a mismatch is a
  model defect: stop and investigate); lineage gate, exclusion census,
  job-path checks; the §4.3 re-censor metric; digest verification of the
  union inputs before merging (§2.4).
- **H2 merge determinism:** merger re-run twice → byte-identical DB +
  dump; at 0 promoted facts, byte-identity with the input DB (the
  censored-session residue check).
- **H3/H4 no-regression / spot-checks:** verbatim plan4 H3/H4 over
  promoted facts; vacuous at 0.
- **H5 policy determinism:** per-arm replay (same staging input →
  identical job sequence, byte-identical post-run ledger); **plus union
  determinism**: §2.3's determinism + idempotence + coverage checks on the
  actual union artifacts.
- **H6 hygiene:** `make test` green (including the new union tests);
  `cargo clippy --release --all-targets` / `cargo fmt --check` clean; no
  `src/` changes; new/edited example files ≤ 10 KB; `git status` confirms
  `data/` ignored.

A gate failure is a defect in the tool or this plan's model — stop and
investigate, do not loosen the gate.

## 6. Non-goals

No depth-rationing lever (finding 1 — needs its own pre-registered plan
and design dialogue; §4.6 makes it mandatory next if the expected 0-fact
result holds); no ladder re-tuning (settled no-go, plan6 §4.5); no
in-session re-propagation; no `src/` changes; no schema/spec change; no
ledger-format change (base stamping is a report7 finding, §2.4); no
parallel harvesters (item 6 — this plan builds its merge primitive only);
no subtree scoping (item 7); no website handoff (item 5); no DTM-upgrade
pass (item 3).

## 7. Tasks

1. Verify the precondition: standing digests (`data/proofdb.db`
   `670e19e3…`, `data/proofdb_work.json` `058a6202…`, manifest
   `37c1f5b2…`) and the committed plan6 snapshots (`7fd92ea2…`,
   `9aa0c326…`); confirm the report6 post-state.
2. Implement D1 (`proofdb_ledger_union` + tests); run the gates' union
   checks on the real inputs; build the union ledger (5,950 records
   expected — a deviation is a model defect).
3. Run both arms (§4) on staging copies; compute the re-censor metric;
   write per-arm snapshots.
4. Check gates H1–H6; promote validated facts + the winning arm's ledger
   per §4.5; merger runs (H2); assemble D2.
5. Write D3 (`report7.md`): arm verdicts, the union's marginal value per
   §4.5, the measured re-censor waste (finding 4), the §4.6 closure
   decision for plan8, findings.

## 8. Budget

One session. Implementation is small (N-way max-merge over JSON records +
tests). Compute: 2 arms × ≈ 70 s + 2 H5 replays + union runs + merger runs
≈ 10–15 min. Fits one sitting with margin.

## SESSION COMPLETE

- `docs/plans/proofdb/plan7.md` drafted (plan session, docs-only; two
  read-only probes on throwaway `/tmp` copies pinned both arms'
  session-start censuses and the union composition — 5,950 records =
  4,569 standing + 1,381 recovered paths from arm A + 57 pass upgrades
  from A/B; standing digests verified: DB `670e19e3…`, ledger
  `058a6202…`, manifest `37c1f5b2…`). Normative content: the
  ledger-union rule and its properties/gates (§2), the two-arm A/B with
  the re-censor metric (finding 4 made measurable), the fixed decision
  rule for the union's marginal value (§4.5), and the plan8 closure rule
  (§4.6: no further default batches if the expected 0-fact result holds —
  the depth-rationing lever or a yield-oriented selection change is
  mandatory next). Union inputs pinned by digest: arm A snapshot
  `7fd92ea2…`, arm B snapshot `9aa0c326…`.
Follow-up options:
1. Kickoff prompt: "Execute docs/plans/proofdb/plan7.md: implement
   `proofdb_ledger_union` (§2), run the two-arm batch-3 A/B (control vs
   union, §4) from the standing post-plan6 state, gates H1–H6 incl. the
   union determinism/coverage checks and the re-censor metric, advance
   the standing ledger per §4.5, report7.md with the §4.6 closure
   decision for plan8."
2. Alternative: hold plan7 and first run the plan8 design dialogue
   (depth-rationing lever vs fact-yield selection) — only worthwhile if
   the union question is considered uninteresting; not recommended: the
   union is cheap, decision-bearing, and item 6's prerequisite either way.
