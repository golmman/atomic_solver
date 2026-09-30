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
- **Independent harvesters become an explicit backlog item** (item 6,
  already added to the initiative at pivot time),
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

**Validation refinements (folded in after a read-only probe of the repo,
same day as the pivot; no gate or schema change):** (i) the job set
excludes open rows with a proven ancestor (sibling-skip — decision 9)
and open rows already decided by child facts (structural number 0 or ∞ —
decision 10); (ii) decision 7's disjointness rule is reworded for
row-harvesting (plan3's "neither a DB row" wording was written for
child-path jobs and is unsatisfiable for open rows — the extractor's
actual C1 check, manifest-only, was always the intent); (iii) pass-1
arithmetic updated to the post-exclusion job set (41 jobs × 4M = 164M);
(iv) D5 pre-registers the pass-1 yield expectation (≈ 0 facts; plan3 P0
measured the identical per-node spend over the same class at 0). The
handover rule is deliberately left untouched: a root proof is a
months-or-years contingency (established in `solve`), and planning
toward it is deferred until the condition actually arises.

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

**Job set (sibling-skip and decided-row exclusion).** The harvest targets
are the open rows that are (a) undecided and (b) not behind a settled OR
choice. An open row is *excluded* when it has a proven ancestor (its line
runs through a proven OR node's non-proving branch — alternative-move
territory, not proof work: once one move of an OR node is proven, the
others are skipped), or when its structural numbers are already decisive
— pn = 0 (a proven root-win child implies the row's win) or pn = ∞
(every reply refuted implies its loss). Excluded rows are counted in the
census with their exclusion reason and left to the future
upward-propagation pass (decision 4); they are never jobs and the ladder
never touches them. In the current DB: 75 open rows → 26 behind a proven
win, 7 implied wins, 1 implied loss, **41 jobs** (including the root;
plies 0–25, all undecided trunk rows).

**Effective number.** Every job's relevant structural number is 1 (by
induction from the frontier: a leaf-most undecided row has an unvisited
reply, and an undecided child carries 1 upward), so a node with no failed
visits has `eff = max(structural number, 1)` = 1; each failed pass raises
its number by one: `eff = max(structural, 1 + passes_failed)`. (Minimal
sound reading of
"update pn/dpn with the work done": the work itself is recorded in the
ledger for analysis; a work-proportional variant is a non-goal this
plan.)
**Selection**: rank by effective number
ascending — OR-role nodes by pn, AND-role nodes by dn (the PNS
duality), merged into one list — ties: ply ascending (closer to root),
ties: path lexicographic ascending. Fully deterministic.

**The ladder.** Pass 1 visits every number-1 node at the base budget
(**4M child-evals**, the measured scale). A censored visit records
`(work_done, passes_failed += 1)` in the ledger and the node leaves the
number-1 pool. Pass k+1 (re-visiting all failed nodes at `2^k × 4M`)
begins when no unvisited node remains — within a session this happens
after pass 1 exhausts the current frontier; across sessions the ledger
carries the pass state. **Session cap: 300M child-evals** (the plan2/3
batch scale; pass 1 is 41 jobs × 4M = 164M worst case, leaving ≈ 136M —
enough for the first ladder rung, ≈ 17 nodes at 8M, within the same
session). Stop conditions
(`--max-jobs`, `--max-runtime`, `--stop-file`) stay between-jobs-only
emergency brakes.

**Simplification, documented**: numbers are computed once at session
start — every row's structural pn/dn from the DB tree as it stands
(proven rows fixed; open rows by the min/sum formulas; unvisited replies
default to 1), then each job's effective number merges the ledger's
`passes_failed` (`eff = max(structural, 1 + passes_failed)`); the job
queue is then fixed. No upward re-propagation mid-session: the only
number that moves is the visited node's own ladder bump (its ledger
record gains `passes_failed` immediately); ancestors and siblings keep
their start-time numbers. The PNS re-propagation happens at the session
boundary instead: every session recomputes all structural numbers from
the freshly merged DB (all prior sessions' facts included) and merges
the ledger rungs, so a multi-session harvest has full PNS dynamics at
session granularity; within a session the frozen snapshot can only
shuffle marginal *ordering*, never the visited set (pass 1 visits every
job exactly once regardless of order). The one real staleness effect:
a mid-session fact can imply a *queued* job's outcome (its parent
becomes implied-decided), and that job may still be visited — bounded
waste (at most a few of the 41 jobs, only in a session that lands
facts), accepted. A decided node's shard grafts after the batch anyway.
Full in-session re-propagation is future work if the pass ladder makes
it matter.

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
    the session (the plan2 replay contract, now covering persisted
    state). Tests for the number pass, ladder, job-set exclusion, and
    lineage gate go through `tests/proofdb.rs` (the established proofdb
    convention); `pns.rs` stays ≤ 10 KB.
- **D2 — CLI extension**: `--policy breadth-pns` (new default),
  `--ledger <path>` (default `data/proofdb_work.json`), census fields on
  the `job:` lines (`pass`, `number` — the node's effective number at
  selection time —, `work_before`); `--budget-evals` remains the explicit
  override; the session cap is `--max-total-evals` (300M, as in plan3's
  batch driver). No changes to session/export/manifest machinery.
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
  verdicts, findings. **Pre-registered yield expectation: pass 1 ≈ 0
  facts** — plan3 P0 spent the identical per-node budget over the same
  open class at 0; only the visit order changes. A measured ≈ 0 with the
  ledger baseline delivered is a success outcome, not a gate failure; if
  the in-session pass-2 rung fires (≈ 136M residual cap), its yield is
  reported as the first escalation data point.

## 4. Pre-registered decisions (fixed before the run)

1. **Draw = censored, always** (plan2 decision 1, unchanged).
2. **Only reconstruction-validated trees become shards** (decision 2,
   unchanged); any validator defect aborts before any write.
3. **Numbers are selection state, never facts**: not written to the DB
   (schema v1), not emitted as census facts; the ledger is gitignored
   working state. A censored node's number update is the minimal
   `+1 per failed pass` rule; no work-proportional variant this plan.
   Decided-by-child rows (structural 0/∞) and proven-ancestor rows are
   never jobs (§2 job set); their implied facts await the future
   propagation pass, not a harvest re-derivation.
4. **No upward re-propagation within a session** (numbers fixed at
   extraction; documented simplification, §2).
5. **Ladder and budgets as in §2**, fixed before the run: base 4M, pass
   k budget `2^(k-1) × 4M`, session cap 300M; no mid-run tuning. The
   job set follows §2's exclusion rules (decisions 9/10).
6. **Ledger lineage gate**: decided paths are dropped at load; illegal
   ledger paths abort; the ledger is deterministic and replayable
   (same DB + same ledger → same job sequence, gate H5).
7. **Job path disjointness (reworded for row-harvesting)**: a job path is
   never a manifest path and never a *proven* DB row; open rows are the
   job set itself. Plan3 decision 3's "neither a DB row" wording was
   written for child-path jobs and is unsatisfiable for open rows; the
   extractor's actual C1 check (manifest-only) was always the intent.
   Child-path jobs (the retained legacy policies) additionally assert
   they are not DB rows.
8. **Parallel harvesters are out of scope** (item 6): sequential PNS is
   measured first.
9. **Sibling-skip**: an open row with a proven ancestor is never a job —
   once one move of an OR node is proven, the others are skipped (user
   product decision, validation session 2026-09-29: one winning move per
   OR node is the product; alternative-move facts are garnish, not proof
   work). The C2 gradient stays selectable for the day coverage matters.
10. **Decided-row exclusion**: an open row implied decided by child facts
   (structural pn = 0 or ∞) is never a job; harvesting would re-derive
   an already-implied fact. Its DB row awaits the future
   upward-propagation pass. The DB ships the raw tree only — every
   derivation (implied OR wins included, display) is the client's job;
   no derived information leaves the store, so the spec needs no
   derivation note (user decisions, validation session 2026-09-29).

## 5. Pre-registered gates (fixed before any run)

- **H1 job/shard integrity**: as plan3 (every shard parses,
  replay-validates, manifest↔tree cross-check, hash fidelity; no
  tag/path collisions), extended with the ledger lineage gate (decision
  6): dropped decided records counted in the census, illegal paths abort;
  and with the job-set exclusion census (decisions 9/10): per-reason
  excluded-row counts reported, and no excluded path appears as a job.
- **H2 merge-after-batch determinism**: `proofdb_merge` over the grown
  manifest twice → byte-identical DB and dump (plan1 G3/plan2 H2/plan3 H2,
  re-run).
- **H3 no-regression vs. the input DB**: every proven node keeps its
  outcome, `depth_bound` not increased; no proven node became open.
- **H4 new-fact spot-checks**: DB row ↔ manifest agreement for every new
  shard root, `--sample-lines 10` replays, parent-chain replay from the
  startpos.
- **H5 policy determinism**: the batch re-run from the same input DB +
  same ledger snapshot and a fresh TT → identical job sequence, identical
  exclusion census, and identical screen-pass records (per policy, as
  plan3 H5).
- **H6 hygiene**: `make test` green (including the new pns tests via
  `tests/proofdb.rs` — cargo does not run unit tests inside example
  targets, the established proofdb convention);
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
(both gradients remain selectable); no parallel harvesters (item 6);
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

One session. Compute: pass 1 = 41 jobs × 4M = 164M child-evals
worst case (≈ 35 s at the measured ~5M evals/s, plus per-job replay
overhead — shallow paths are cheaper than plan3's); the 300M session cap
leaves ≈ 136M for the first ladder rung in-session (≈ 17 nodes at 8M);
one merger run ≈ 1 min; unit
tests minutes. Implementation is the dominant session cost: the structural
number pass reuses the existing DFS replay (one post-order pass over
the input DB's row count — ≈ 35k today, growing with the DB), the
ledger is a small JSON file. Fits `make test` plus the
batch inside one sitting.

## SESSION COMPLETE

- `docs/plans/proofdb/plan4.md` written and validated (read-only probe;
  refinements folded in: job-set exclusion rules 9/10, reworded decision
  7, updated pass-1 arithmetic and pre-registered yield expectation);
  pivot decisions recorded; the initiative already carries the pivot
  history entry and item 6.
Follow-up options:
1. Kickoff prompt: "Execute docs/plans/proofdb/plan4.md (breadth-first PNS
   harvester): pns.rs numbers + sidecar ledger under data/, --policy
   breadth-pns default, pass-1 batch (41 jobs × 4M = 164M, cap 300M),
   gates H1–H6, report4.md with the pass-1 yield and escalation outlook."
2. Alternative: run the item-5 website handoff first (ship
   `docs/spec/global_proof_store.md` + the grown DB) so schema feedback
   arrives before the PNS ladder escalates the frontier's cost — the
   coverage the website needs is already harvested; the ladder mainly
   serves proof depth.
