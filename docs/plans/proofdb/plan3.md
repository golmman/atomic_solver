# Plan 3: coverage policy — frontier classes beyond the open nodes, measured A/B

Initiative: `proofdb`. Executes **backlog item 4** (coverage policy: which
frontier children to open next, cheap-first gradient, not enumerative
layers). This plan extends the harvest loop's job selection from "open
nodes only, deepest-first" to measured, pre-registered selection gradients
over the *full* frontier, and runs an A/B batch to pick the default policy
by cost-per-new-fact. Docs + `examples/`-side code only; the product solver
is untouched; the DB schema and spec are unchanged. Per repo convention the
final task is `report3.md`.

## 1. Background (self-contained)

**State after plan2.** The standing shard dir (`docs/plans/proofdb/shards/`)
holds 107 validated shards (95 seed + 12 harvest `h_*`); the merged DB
(`measurements/plan2/proofdb_grown.db`) has 12,655 nodes = 6,289 `win` +
6,291 `loss` + **75 open**. Open by ply: the root, 7 ply-1 nodes, then a
quiet-ladder tail out to ply 31. The plan2 screen (4M child-evals/job,
deepest-first) decided 12/87 jobs; the 5 deepest censored jobs were
re-verified undecided at 40M each — the remaining 75 open nodes are the
expensive quiet class under the deepest-first rule.

**The unmeasured frontier class.** The DB is a partial AND/OR tree keyed by
path: a proven `loss` node (AND) stores a refutation for **every** legal
reply — no gap by construction — but a proven `win` node (OR) stores
exactly **one** proving child. Every other legal move under a `win` node is
*unexpanded*: not a row, derivable only by replay + movegen. With 6,289 win
nodes at ~20–40 branching this is a ~150k+-position class the website hits
the moment a user replays a non-stored move (e.g. "after 1.e3 g5, what if
White tries something other than the stored proof move?"). Solving an
unexpanded sibling of a proving child yields either a new `loss` fact (the
opponent is still lost — White's try also wins) or a new `win` fact (the
try throws the win away — exactly the coverage a user needs). Both graft
through the existing merger unchanged.

**Why expect these facts to be cheap.** The sharpness gradient measured in
solve plan4 (55 ply-2 refutations at seconds each) and the plan2 batch
(12 decisive jobs at 1.4–2.4k evals once TT retention carried the deeper
censored prefixes) both say: facts are cheap where tactics are sharp, and
sharpness is already encoded in the DB as the parent's `depth_bound`. The
coverage-policy hypothesis to test: ranking the unexpanded-sibling class by
parent `depth_bound` ascending produces new facts at a far lower
cost-per-fact than sinking the same budget into the 75 quiet open nodes.

**Report2's hooks this plan builds on:**
- finding 2 — deepest-first + TT retention is a strong gradient (deep
  prefixes precede their shallow relatives); job order stays fully recorded
  and replayable.
- next-step 1 — "cheap-first or sibling-of-proof gradients may beat
  deepest-first" (this plan).
- next-step 2 — cross-session TT seeding (`--tt-load`) is explicitly *not*
  done here (non-goal); it would confound the A/B by making runs
  order-dependent across processes.

**Search-side contracts** (unchanged from plan2, restated for a fresh
executor): deterministic child-eval budgets (`set_child_eval_budget`;
exhaustion → `Draw`/`BudgetExhausted`, mapped to *censored*, no DB write);
path context mandatory (`search_depth_with_prefix` with the ancestors-only
repetition prefix — the plan2 root-job fix); `first_outcome_only` on;
only reconstruction-validated trees become shards; TT retained across jobs
within one session (census records job order and budgets, so the batch is
replayable from a fresh TT).

## 2. Frontier classes and policies (pre-registered)

**Classes** (all jobs by path identity; extracted read-only from the DB):

- **C1 — open nodes** (`outcome IS NULL` rows). 75 today. The plan2 job
  class.
- **C2 — unexpanded children of proven `win` nodes**: parent is a row with
  `outcome='win'`; the child path (parent path + legal move) is **not** a
  DB row. Ranked by parent `depth_bound` ascending, tie ply ascending, tie
  path lexicographic ascending (the *sharpness gradient*).
- **C3 — unexpanded children of open nodes**: parent is a row with
  `outcome IS NULL`; child path not a DB row. Same ranking as C2 (parent
  has no `depth_bound`; rank by ply ascending then path).
- **Excluded**: children of proven `loss` nodes — AND-completeness says
  every legal reply is already a row; the extractor *asserts* this and any
  violation aborts the session (a merger defect, stop and investigate —
  this assert is a free soundness check over all 6,291 AND nodes).

**Policies** (deterministic job sequences; compared at a fixed total
screen budget):

- **P0 `open-deepest`** — status quo: C1 only, ply descending, ties
  lexicographic (plan2's rule). Re-measured over the current 75-node
  frontier as the honest baseline; expectation (pre-registered): near-zero
  decisive at the baseline budget, since the cheap C1 tail was already
  harvested in plan2.
- **P1 `sharp-siblings`** — C2 (sharpness order), then C3, then C1
  deepest-first; per-class screen budgets: C2 = 1M child-evals (cheap
  territory — censor fast and move on), C1/C3 = 4M.
- **P2 `sharp-heavy-tail`** — P1's order, but after the C2 screen the
  first N (=5, plan2's shape) censored C2 jobs are re-run at 40M as the
  censored-tail sample (measures where the sibling class stops being
  cheap), before C3/C1 start.

Each policy runs as a **separate process with a fresh TT** over the same
input DB (plan2's screen pass was exactly deterministic under that shape).
Facts produced by an earlier policy in the comparison are merged into the
standing shard set first, and later policies run against the re-merged DB —
so each policy measures the *marginal* yield at its turn, and the analysis
must report per-policy new-fact counts from that policy's own census
(overlapping facts are impossible by construction: a job path that gained a
shard is no longer in any frontier class).

**Primary metric**: new facts per 100M child-evals. Secondary: cost/fact
distribution, per-class decisive rate, ply distribution of new facts
(product coverage depth), wall time. The winner becomes the default
`--policy` value; the loser stays available (both are cheap to keep).

## 3. Deliverables

- **D1 — `examples/proofdb/policy.rs`** (+ `policy/tests.rs`, keeping every
  file ≤ 10 KB): frontier-class extraction (C2/C3 by replay of parent paths
  + legal movegen through `Position`, dedupe against all DB rows and
  manifest paths — the manifest is already loaded by the CLI), AND-
  completeness assert over `loss` nodes, the three policy rankings, and
  per-class budget resolution. Extraction cost is O(rows + frontier); the
  150k-class C2 enumeration must be lazy enough to build in well under a
  second (it is: one movegen pass over the 6,289 win nodes).
- **D2 — `proofdb_harvest` CLI extension**: `--policy <open-deepest|
  sharp-siblings|sharp-heavy-tail>` (default `open-deepest`, so plan2's
  behavior is byte-reproducible), plus census fields (`class`, `policy`,
  `parent_bound` for C2) on the `job:` JSON lines. The heavy-tier flag
  semantics stay as documented; `--heavy-sample 0` disables. No changes to
  session/export/manifest machinery — a C2/C3 job is just a `Job` with a
  deeper parent chain, and the graft/upgrade path is the merger's existing
  machinery.
- **D3 — measured A/B batch** under `measurements/plan3/`: per-policy
  `census_<policy>.json`, the merged grown DB after all batches
  (`proofdb_grown2.db` + `nodes_grown2.txt`), a comparison table
  (`policy_comparison.json` + human-readable in the README), `env.json`,
  `README.md` (provenance and command table). Standing shard dir + manifest
  updated (append-only, as in plan2); the derived DB snapshot lives under
  `measurements/plan3/`.
- **D4 — `report3.md`**: per-policy yields, the pre-registered metric
  table, gate verdicts, findings (including whether the sharpness
  gradient's cost-per-fact holds at the new frontier and where C2 stops
  being cheap).

## 4. Pre-registered decisions (fixed before the run)

1. **Draw = censored, always** (plan2 decision 1, unchanged). A censored
   C2/C3 job leaves no durable trace this plan (no schema v2 / frontier
   marks — non-goal §6); re-search cost is accepted and the census records
   it.
2. **Only reconstruction-validated trees become shards** (plan2 decision 2,
   unchanged); any validator defect aborts the session before any write.
3. **Job path disjointness extended**: a job path must be neither a DB row
   (open *or* proven) nor a manifest path — for all classes. The C2/C3
   extractor enforces this at extraction time.
4. **AND-completeness assert**: every legal reply of every `loss` row is a
   row. Violation → abort (merger/model defect), never a workaround.
5. **Per-class budgets as in §2**, fixed before the run; no mid-run tuning.
6. **Policy comparison fairness**: same input DB lineage, fresh TT per
   policy run, fixed total screen budget per policy (**300M child-evals**,
   matching plan2's screen spend), stop conditions disabled during the A/B
   except the budget itself.
7. **Winner rule** (pre-registered): highest new-facts-per-100M-evals wins;
   ties broken by lower median cost-per-fact; a tie there keeps
   `open-deepest` (status quo bias). The winner is recorded as the default
   `--policy` value for future batches; no other behavior change rides on
   this plan.

## 5. Pre-registered gates (fixed before any run)

- **H1 job/shard integrity**: as plan2 (every shard parses, replay-validates,
  manifest↔tree cross-check, both hash-fidelity checks; no tag/path
  collisions), extended with decision 3's class disjointness. C2/C3 job
  paths must replay legally from the startpos (the parent chain is the DB
  row's path; the added move is checked by movegen).
- **H2 merge-after-batch determinism**: `proofdb_merge` over the grown
  manifest twice → byte-identical DB and dump (plan1 G3/plan2 H2, re-run).
- **H3 no-regression vs. the input DB**: every proven node keeps its
  outcome, `depth_bound` not increased; no proven node became open; open
  nodes may become proven or stay open (C2/C3 facts only *add* nodes and
  proven descendants — an upgrade of an existing open node can only come
  from a C1 job or an overlay inside a new shard's subtree).
- **H4 new-fact spot-checks**: as plan2 (DB row ↔ manifest agreement for
  every new shard root, `--sample-lines 10` replays, parent-chain replay
  from the startpos).
- **H5 policy determinism**: the winning policy's batch is re-run from the
  same input DB and a fresh TT → identical job sequence and identical
  screen-pass records (the plan2 full-batch-replay leg, now per policy).
- **H6 hygiene**: `make test` green (including the new policy unit tests);
  `cargo clippy --all-targets` / `cargo fmt --check` clean; no `src/`
  changes; all new/edited example files ≤ 10 KB.

A gate failure is a defect in the tool or this plan's model — stop and
investigate, do not loosen the gate.

## 6. Non-goals

No `src/` changes; no schema/spec change (v1 stands; no frontier-marks
table — the censored re-search cost is accepted and recorded); no
cross-session TT seeding (`--tt-load`; deferred — it would confound the
A/B); no DTM-upgrade pass (item 3); no website handoff (item 5); no
parallel/multi-worker harvest; no incremental merger (full re-merge per
batch; sizes stay tiny); no enumeration *policy* for C2's full 150k class
beyond the sharpness ranking (the budget cuts the tail; deeper policies are
future work if the gradient saturates).

## 7. Tasks

1. Implement D1 (`policy.rs` + tests: class extraction on a DB fixture,
   AND-completeness assert, sharpness ordering, budget resolution, C2/C3
   path disjointness), then D2 (CLI flag, census fields).
2. Build the A/B input state: re-merge the standing shard set into the D3
   input DB; verify `built_from` digest and node count against plan2's
   grown state (12,655 nodes) — this is the H3 diff base.
3. Run P0, then merge its shards, re-merge, run P1, repeat for P2
   (decision 6's fixed budgets; `--stop-file` as the emergency brake
   only).
4. Check gates H1–H6; assemble D3 (`policy_comparison.json`, census,
   README).
5. Write D4 (`report3.md`); record the winner as the default `--policy`
   and the per-class budget table for future batches.

## 8. Budget

One session. Compute: P0 ≈ 300M evals over expensive C1 jobs (the known
plateau — pre-registered to be near-zero yield; it is the baseline
measurement); P1/P2: C2 cheap-sibling jobs at 1M each ≈ minutes at the
measured ~5M evals/s; three merger runs ≈ 1 min; unit tests minutes. The
dominant session cost is P0's baseline spend, which is deliberate and
capped. Fits `make test` plus the batch inside one sitting.

## SESSION COMPLETE

- `docs/plans/proofdb/plan3.md` written (coverage policy, item 4); A/B
  design pre-registered; initiative history not yet updated (that happens
  with report3 at execution)
Follow-up options:
1. Kickoff prompt: "Execute docs/plans/proofdb/plan3.md (coverage policy,
   item 4): policy.rs + --policy in proofdb_harvest, pre-registered P0/P1/P2
   A/B at 300M child-evals each over the current frontier, merge after each
   policy batch, gates H1–H6, report3.md with the winner as default."
2. Alternative: run the item-5 website handoff first (ship
   `docs/spec/global_proof_store.md` + the grown DB) so schema feedback
   arrives before the frontier classes multiply the DB's branching — the
   coverage batch can then target what the website actually exposes.
