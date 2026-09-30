# Plan 4: breadth-first PNS harvester — selection state in a sidecar, working layer at `data/`

Initiative: `proofdb`. Executes the plan3 pivot (agreed 2026-09-29, see
`report3.md` next-steps and the initiative history): job selection moves
from class gradients over the full frontier to **breadth-first PNS over
the open frontier**, with proof/disproof numbers as the priority. The
initiative goal is unchanged — *coverage AND proof* of startpos-rooted
lines, harvested as fast as possible — the metric stays harvesting value
(facts per cost, most-proving lines first), not root-solver speed; the
handover rule stands. Docs + `examples/`-side code only; the product solver
is untouched; the DB schema and spec are unchanged. Per repo convention the
final task is `report4.md`.

## 1. Background (self-contained)

**State after plan3.** The standing shard dir (`docs/plans/proofdb/shards/`)
holds 197 validated shards (95 seed + 12 plan2 + 90 plan3); the derived DB
(`measurements/plan3/proofdb_grown2.db`) has 35,055 nodes = 34,980 proven +
**75 open**. The plan3 A/B (300M child-evals per policy, fresh TT each)
decided: `sharp-siblings` 90 facts / 300.1M (29.99 facts/100M, median
44.7k evals/fact, all ply-5 loss refutations under bound-1 win parents);
`open-deepest` 0 / 300M; the marginal re-run of `sharp-siblings` after
merging its own facts 0 / 500M. The C2 (sibling-of-proof) cheap head is
exhausted; the open frontier (75 nodes) is a quiet class, ≥ 4M evals each,
deepest 5 ≥ 40M (plan2).

**The pivot (user decisions, 2026-09-29):**
- The DB serves *both* coverage and proof; the unexpanded-sibling class
  (C2) was worth harvesting once (measured) but is not the future: selection
  should follow **breadth-first PNS** — explore the most proving lines
  first, which is also what the website's users (atomic chess players
  probing opening ideas) most want to see.
- Selection numbers are proof/disproof numbers (min/sum recursion,
  unvisited = 1); after a budget-censored visit the node's number is
  updated from the work done and the next node is picked; when no
  number-1 nodes remain, failed nodes are re-visited with **doubled work**
  (geometric ladder). Numbers are selection state only: they never enter
  the DB (schema v1 stands) and never emit as facts — they live in a
  **sidecar work ledger**.
- The working layer moves to **`data/`** (gitignored): the derived DB and
  the ledger live there, kept out of application code; the DB grows and
  the repo should not conflate gigabytes of positions with source.
- **Independent harvesters become an explicit backlog item** (item 6, new),
  not part of this plan: sequential PNS is measured first; the job
  structure (independent roots, deterministic budgets) makes a later
  partition (by root-child subtree or class, per-worker shard staging,
  serialized manifest merge) almost mechanical.

**Precedent.** The `solve` campaign prototype (`examples/campaign*`,
plans 5/9) already ran multi-worker PNS-style harvesting (pseudo-MPN job
selection, workers claiming jobs via atomic rename, verify-on-merge); it
was frozen after the plan9 resume-path failure mode. `proofdb`'s plan2
reused its session shape single-process. This plan re-uses the *ideas*
(structural numbers, work ledger) with proofdb's cleaner deterministic
shard/merge layer, and keeps the resume surface minimal (see D1).

**Search-side contracts** (unchanged from plan2/plan3): deterministic
child-eval budgets (`set_child_eval_budget`; exhaustion → `Draw`/
`BudgetExhausted`, mapped to *censored*, no DB write); path context
mandatory (`search_depth_with_prefix`, ancestors-only repetition prefix);
`first_outcome_only` on; only reconstruction-validated trees become
shards; TT retained across jobs within one session (fresh TT per process).

## 2. The selection policy (pre-registered)

**Structural proof numbers.** For every node of the merged DB tree,
computed bottom-up (post-order over the tree; the frontier extraction's
DFS replay already carries each row's `Position`, outcome, and ply):

- *Unvisited* child (a legal move of an open node with no DB row): pn = dn = 1.
- *Proven* node: root-player-win (even ply + `win`, or odd ply + `loss` —
  outcomes are side-to-move perspective) → pn = 0, dn = ∞; root-player-loss
  → pn = ∞, dn = 0.
- *Open* node with children: parity formula — even ply (root player to
  move, OR): pn = min over children pn, dn = sum over children dn; odd ply
  (AND): pn = sum over children pn, dn = min over children dn.
- Unexpanded legal replies of an open node count as unvisited children
  (movegen at the replayed position), so an AND node's sum and an OR
  node's min include them.

**Effective number.** A node with no failed visits has
`eff = max(structural number, 1)`; each failed pass raises its number by
one: `eff = max(structural, 1 + passes_failed)`. (Minimal sound reading of
"update pn/dpn with the work done": the work itself is recorded in the
ledger for analysis; a work-proportional variant is a non-goal this plan.)
**Selection**: among open rows (the harvest targets), rank by effective
number ascending — OR-role nodes by pn, AND-role nodes by dn (the PNS
duality), merged into one list — ties: ply ascending (closer to root),
ties: path lexicographic ascending. Fully deterministic.

**The ladder.** Pass 1 visits every number-1 node at the base budget
(**4M child-evals**, the measured scale). A censored visit records
`(work_done, passes_failed += 1)` in the ledger and the node leaves the
number-1 pool. Pass k+1 (re-visiting all failed nodes at `2^k × 4M`)
begins when no unvisited node remains — within a session this happens
after pass 1 exhausts the current frontier; across sessions the ledger
carries the pass state. **Session cap: 300M child-evals** (the plan2/3
batch scale; pass 1 alone is 75 × 4M = 300M). Stop conditions
(`--max-jobs`, `--max-runtime`, `--stop-file`) stay between-jobs-only
emergency brakes.

**Simplification, documented**: numbers are computed once at session start
from the DB + ledger; no upward re-propagation after in-session decisions
(a decided node's shard grafts after the batch anyway). Full
re-propagation is future work if the pass ladder makes it matter.

**What is dropped under the PNS policy**: C2 (unexpanded siblings of
proven wins) and C3 (unexpanded children of open nodes) are no longer job
targets — C2 because proving a proven parent's alternatives is not proof
work (the plan3 measured head is spent), C3 because unvisited replies are
already *counted* in their parent's AND-sum/OR-min, which is where they
belong in PNS. Both gradients stay available as `--policy sharp-siblings` /
`open-deepest` (built, measured, cheap to keep); `breadth-pns` becomes the
default.

## 3. Deliverables

- **D1 — `examples/proofdb/pns.rs`** (+ tests, every file ≤ 10 KB):
  structural pn/dn computation over the extracted DB tree (extends the
  frontier DFS with a post-order number pass), the effective-number and
  ladder rules, the merged priority list, and the sidecar ledger:
  - `data/proofdb_work.json` — one record per harvested node
    (`path`, `work_done` (child-evals), `passes_failed`), deterministic
    bytes, rewritten between jobs (crash-safe), gitignored alongside the
    DB. **Lineage gate at load**: a ledger record whose path is now a DB
    row is dropped (the node was decided after the ledger was written);
    a ledger path that does not replay legally from the startpos aborts
    the session (the plan2 replay contract, now covering persisted state).
- **D2 — CLI extension**: `--policy breadth-pns` (new default),
  `--ledger <path>` (default `data/proofdb_work.json`), census fields on
  the `job:` lines (`pass`, `number` — the node's effective number at
  selection time —, `work_before`); `--budget-evals` remains the explicit
  override. No changes to session/export/manifest machinery.
- **D3 — working layer at `data/`**: `.gitignore` entry for `data/`; the
  canonical working DB built there from the standing shard set
  (`data/proofdb.db`, rebuilt from committed shards at any time); the
  per-batch archived copy stays under `measurements/plan4/` for
  provenance (measurement conventions).
- **D4 — measured batch** under `measurements/plan4/`: per-batch
  `census_breadth-pns.json` (job records: path, policy, class, pass,
  number, work_before, budget, child_evals, outcome, exit_reason, tag,
  shard_nodes), the grown DB copy + dump, `ledger_snapshot.json` (the
  post-batch ledger — the escalation state for the next session),
  `env.json`, `README.md` (provenance and command table).
- **D5 — `report4.md`**: pass-1 yields (facts/100M, ply distribution,
  which nodes got harvested first — verify the breadth-first gradient),
  the escalation outlook (measured work curve vs the ladder), gate
  verdicts, findings.

## 4. Pre-registered decisions (fixed before the run)

1. **Draw = censored, always** (plan2 decision 1, unchanged).
2. **Only reconstruction-validated trees become shards** (decision 2,
   unchanged); any validator defect aborts before any write.
3. **Numbers are selection state, never facts**: not written to the DB
   (schema v1), not emitted as census facts; the ledger is gitignored
   working state. A censored node's number update is the minimal
   `+1 per failed pass` rule; no work-proportional variant this plan.
4. **No upward re-propagation within a session** (numbers fixed at
   extraction; documented simplification, §2).
5. **Ladder and budgets as in §2**, fixed before the run: base 4M, pass
   k budget `2^(k-1) × 4M`, session cap 300M; no mid-run tuning.
6. **Ledger lineage gate**: decided paths are dropped at load; illegal
   ledger paths abort; the ledger is deterministic and replayable
   (same DB + same ledger → same job sequence, gate H5).
7. **Job path disjointness** (plan3 decision 3, unchanged): a job path is
   neither a DB row nor a manifest path; the extractor asserts it.
8. **Parallel harvesters are out of scope** (item 6, new backlog entry):
   sequential PNS is measured first.

## 5. Pre-registered gates (fixed before any run)

- **H1 job/shard integrity**: as plan3 (every shard parses,
  replay-validates, manifest↔tree cross-check, hash fidelity; no
  tag/path collisions), extended with the ledger lineage gate (decision
  6): dropped decided records counted in the census, illegal paths abort.
- **H2 merge-after-batch determinism**: `proofdb_merge` over the grown
  manifest twice → byte-identical DB and dump (plan1 G3/plan2 H2/plan3 H2,
  re-run).
- **H3 no-regression vs. the input DB**: every proven node keeps its
  outcome, `depth_bound` not increased; no proven node became open.
- **H4 new-fact spot-checks**: DB row ↔ manifest agreement for every new
  shard root, `--sample-lines 10` replays, parent-chain replay from the
  startpos.
- **H5 policy determinism**: the batch re-run from the same input DB +
  same ledger snapshot and a fresh TT → identical job sequence and
  identical screen-pass records (per policy, as plan3 H5).
- **H6 hygiene**: `make test` green (including the new pns unit tests);
  `cargo clippy --all-targets` / `cargo fmt --check` clean; no `src/`
  changes; all new/edited example files ≤ 10 KB; `git status` confirms
  `data/` is ignored.

A gate failure is a defect in the tool or this plan's model — stop and
investigate, do not loosen the gate.

## 6. Non-goals

No `src/` changes; no schema/spec change (v1 stands; the numbers live in
the sidecar, not the DB); no work-proportional number updates (the `+1 per
failed pass` rule is the pre-registered minimal reading); no upward
re-propagation within a session; no C2/C3 harvesting under the new default
(both gradients remain selectable); no parallel harvesters (item 6, new);
no cross-session TT seeding (`--tt-load`; still deferred — the ledger, not
TT residue, is the cross-session state); no DTM-upgrade pass (item 3); no
website handoff (item 5); no incremental merger.

## 7. Tasks

1. Implement D1 (`pns.rs`: structural numbers over the DB tree, effective
   numbers, ladder, priority list, sidecar ledger with the lineage gate)
   and D2 (`--policy breadth-pns` default, `--ledger`, census fields).
2. Set up D3: `.gitignore` for `data/`; build `data/proofdb.db` from the
   standing shard set (byte-identity check vs the plan3 grown DB); empty
   ledger init.
3. Run the pass-1 batch (decision 5 budgets; `--stop-file` as the
   emergency brake only); write the post-batch ledger snapshot.
4. Check gates H1–H6; assemble D4 (`census_breadth-pns.json`,
   `ledger_snapshot.json`, README).
5. Write D5 (`report4.md`); record the pass-1 yield and the escalation
   outlook (which pass the frontier's quiet class reaches).

## 8. Budget

One session. Compute: pass 1 = 75 open nodes × 4M = 300M child-evals
(≈ 60–70 s at the measured ~5M evals/s, plus per-job replay overhead —
shallow paths are cheaper than plan3's); one merger run ≈ 1 min; unit
tests minutes. Implementation is the dominant session cost: the structural
number pass reuses the existing DFS replay (one post-order pass over
35k rows), the ledger is a small JSON file. Fits `make test` plus the
batch inside one sitting.

## SESSION COMPLETE

- `docs/plans/proofdb/plan4.md` written (breadth-first PNS harvester,
  sidecar selection state, working layer at `data/`); pivot decisions
  recorded; parallel harvesters added to the initiative as item 6;
  initiative history not yet updated (that happens with report4 at
  execution).
Follow-up options:
1. Kickoff prompt: "Execute docs/plans/proofdb/plan4.md (breadth-first PNS
   harvester): pns.rs numbers + sidecar ledger under data/, --policy
   breadth-pns default, pass-1 batch at 300M child-evals, gates H1–H6,
   report4.md with the pass-1 yield and escalation outlook."
2. Alternative: run the item-5 website handoff first (ship
   `docs/spec/global_proof_store.md` + the grown DB) so schema feedback
   arrives before the PNS ladder escalates the frontier's cost — the
   coverage the website needs is already harvested; the ladder mainly
   serves proof depth.
