# Plan 2: harvest loop — shard production from the DB frontier, merge after batch

Initiative: `proofdb`. Executes **backlog item 2** (the harvest loop).
This plan builds the shard-production tool, runs the first real harvest
batch against the seeded DB's frontier, and grows the DB through the
existing merger. Docs + `examples/`-side code only; the product solver is
untouched. Per repo convention the final task is `report2.md`.

## 1. Background (self-contained)

**State after plan1.** `examples/proofdb_merge` + `examples/proofdb/` merge
validated proof-tree shards into `proofdb.db` (schema v1,
`docs/spec/global_proof_store.md`). The seed (solve plan4's 95 shards) is
merged: 12,081 nodes = 11,994 proven (5,996 win bound / 5,667 loss exact at
rule-terminals / 331 loss bound) + **87 open** (the startpos root + 86 graft
ancestors); `measurements/plan1/proofdb_seed.db` is the committed seed DB,
built from `measurements/plan1/manifest_seed.json`.

**What the harvest loop is.** Per the initiative backlog: campaign-worker-
shaped runs producing shards — *fresh root per job* → solve → reconstruct →
validate → manifest entry — with the merger re-run after each batch. This is
exactly the regime solve plan9 did **not** falsify (it resumed one frozen
quiet root; harvest uses fresh roots per shard).

**Job definition (the frontier).** A harvest job is one **open node** of the
current DB (`outcome IS NULL`, by path identity). Rationale: open nodes are
the tree's undecided frontier; a decisive solve at an open node produces a
shard that grafts exactly there (the merger's ancestor-upgrade machinery
from plan1 handles the overlay: the node flips from open to proven, its
subtree appears, agreement/refinement rules apply). Proven nodes are never
jobs (their path is already decided; no same-path contradiction is possible
by construction — asserted, see gate H1). Unexpanded siblings of proven
OR-nodes (children of open/proven nodes that are not rows) are a *different*
frontier class — that selection gradient is coverage policy (item 4) and is
deliberately out of scope here.

**Honest expected outcome.** The seed's 86 non-root open nodes are graft
ancestors: ~20 ply-1 positions and quiet intermediates on the plan4 ladder
paths (plies ~3–37). Solve plan4 measured this territory as the censored
plateau (ply-1: all 20 undecided at 8 s ≈ 1.6–1.8 M nodes, still undecided
at 120 s ≈ 21–24 M nodes; the quiet class measured bottomless in
`solve` plans 1–3). The deepest ancestors (nearest the tactical sharp end)
are the most plausible cheap wins. **A near-zero decisive rate is a valid,
pre-registered measured result** — the loop's value is the *mechanism*
(stoppable anytime, deterministic budgets, validated growth) plus the rate
measurement itself; a zero-growth batch still exercises every gate below.

**Search-side facts the executor must know:**
- Deterministic budgets: `Search::set_child_eval_budget(budget)` — on
  exhaustion the search returns `Draw` with `ExitReason::BudgetExhausted`,
  never `Timeout`. `Draw` from any cause maps to **censored** (no fact, no
  DB write); only `Win`/`Loss` proceeds to the shard pipeline.
- Path context is mandatory: run
  `Search::search_depth_with_prefix(pos, u32::MAX, &prefix_keys)` with the
  full startpos→node key path (the campaign worker's context contract).
  Repetition verdicts are path-dependent (GHI); solving the bare position
  without the path could produce a claim that does not correspond to the
  DB's path-keyed node. Note: the detector-gated pre-phase deliberately does
  not run under a supplied prefix — acceptable (conservative); preflight-
  certified `exact` claims stay an item-3 thread.
- Use `first_outcome_only` (`Search::set_first_outcome_only(true)`) for
  harvest jobs: the PV-shortening budget is wasted work for shard
  production; the reconstruct-exported proof subtree is unaffected.
- TT retention across jobs within one session (campaign worker precedent,
  `--tt-mb`): solved TT entries are path-independent facts; unsolved bounds
  are advisory-only (GHI journal contract). Retention makes the *census of
  a session* order-dependent (which positions decide within budget depends
  on prior jobs' TT residue) — soundness is unaffected because every fact
  crosses the boundary only through reconstruct + `validate_proof_tree`.
  The census records the job order and budgets so the batch is replayable.
- Offline export: TT snapshot (`write_tt_snapshot`) →
  `reconstruct(sub_fen, &solved, config)` → `validate_proof_tree`. With a
  retained TT a raw DF-PN event stream is not always self-contained
  (holes at TT-resolved nodes proven under earlier jobs), so only a
  reconstruction-verified subtree crosses the boundary. The campaign
  worker's `export_via_reconstruction` (examples/campaign_worker.rs) is the
  proven shape.

## 2. Deliverables

- **D1 — standing shard directory** `docs/plans/proofdb/shards/`:
  append-only durable layer of the initiative (one dir, not per-plan —
  per-plan `measurements/plan2/` holds only the derived DB snapshot and
  census; the shards themselves are the reusable validated artifact class
  AGENTS.md commits). Contents after this plan: the 95 seed shards copied
  from `docs/plans/solve/measurements/plan4/artifact/` + the batch's new
  shards + the standing `manifest.json` (all entries, seed tags unchanged).
  The standing manifest is generated by adapting
  `measurements/plan1/build_seed_manifest.py` (path handling for the new
  location); its bytes must satisfy `read_manifest` and the
  `global_proof_store.md` manifest contract (unique tag, unique path).
  A sanity merge over the standing dir must reproduce the seed DB's
  `nodes` content byte-for-byte (meta `built_from`/`generator` rows may
  differ) before any harvest job runs.
- **D2 — manifest writer + harvest tooling in `examples/proofdb/`**:
  - `examples/proofdb/shard_export.rs` — the offline export helper
    (TT snapshot → reconstruct → validate → `ProofTree`), copied from the
    campaign worker's `export_via_reconstruction` with an attribution
    header. `campaign_worker.rs` itself stays untouched this plan (frozen
    measured prototype; the small duplication is accepted and noted in the
    report — dedupe only if the campaign work reopens).
  - manifest writer (`write_manifest`) in `examples/proofdb/mod.rs`
    (required fields only; entries sorted by `tag`; deterministic bytes).
  - `examples/proofdb_harvest.rs` — CLI:
    ```sh
    proofdb_harvest --db <proofdb.db> --manifest <manifest.json>
                    --shard-dir <dir> [--budget-evals 4000000]
                    [--heavy-budget-evals 40000000] [--heavy-sample 5]
                    [--tt-mb 128] [--max-jobs 0] [--max-runtime 0]
                    [--stop-file STOP] [--out-db <grown.db>] [--dump <nodes.txt>]
    ```
    Pipeline: read the DB read-only (abort unless `meta.built_from` equals
    the standing manifest's SHA-256 — no silent divergence between DB and
    shards) → extract open nodes as jobs (deepest-first: ply descending,
    ties by lexicographic path; rationale: deep ancestors sit nearest the
    sharp tactical class, and TT retention then helps the shallower
    ancestors later in the same session) → per job: replay the path from
    `STARTPOS_FEN`, `search_depth_with_prefix` under `--budget-evals`;
    `Draw` → censored (job record only); `Win`/`Loss` → export via D2's
    helper → write `<tag>.bin` → append a manifest entry
    (`tag = "h_" + sha256(path)[..16]` hex, `moves` = the node's UCI path,
    `outcome`, `validate: "ok"` only on a validator pass — any validator
    defect aborts the session, nothing is written) → between jobs check
    `--max-jobs` / `--max-runtime` / stop file (stop *between* jobs only;
    a job is never abandoned mid-search — budgets are small enough that
    losing one interrupted job's work is cheaper than nondeterministic
    interruption) → rewrite the manifest (sorted, deterministic) → caller
    then runs `proofdb_merge --manifest … --shard-dir …` over the grown
    set (merger stays a separate tool; the harvest CLI does not merge).
  - The **heavy tier** (`--heavy-budget-evals`, `--heavy-sample N`):
    after the screen pass, the first N censored jobs in job order are
    re-run at the heavy budget as a pre-registered censored-tail sample
    (measures the plateau's cost curve; any decision there is a shard like
    any other). Default N = 5, heavy budget = 40 M child-evals
    (~10× the screen).
- **D3 — grown DB + census** under `measurements/plan2/`:
  `proofdb_grown.db`, `nodes_grown.txt`, `census.json` (per-job records:
  path, budget tier, child-evals spent, wall, outcome or `censored`;
  merge arithmetic; replayable session description), `env.json`,
  `README.md` (provenance). The standing shard dir + manifest are the
  durable copy (D1); `measurements/plan2/` holds the derived snapshot.
- **D4 — `report2.md`.**

## 3. Pre-registered decisions (fixed before the run)

1. **Draw = censored, always.** No distinction between budget exhaustion
   and a "genuine" draw claim; nothing from a `Draw` enters the DB.
2. **Only reconstruction-validated trees become shards.** A validator
   defect or manifest↔tree cross-check mismatch aborts the session with a
   non-zero exit and no manifest/shard writes (constraint 1: rejected,
   never patched).
3. **Depth-status mapping unchanged:** the merger's structural finalize
   stays the sole `exact` source (rule-terminals); harvest never claims
   `exact` (see §1 note on the pre-phase).
4. **Job order = deepest-first, ties lexicographic** — recorded in the
   census; coverage-policy work (item 4) may replace the rule later, but
   this plan's batch is a fixed, replayable sequence.
5. **Outcome perspective** is side-to-move at the node (schema semantics);
   the search's `Outcome` maps directly; `loss` shards refute every legal
   reply (AND semantics) — the merger's skeleton re-validation covers this.

## 4. Pre-registered gates (fixed before any run)

- **H1 job/shard integrity**: every produced shard parses, replay-validates
  (`validate: ok`), passes manifest↔tree cross-check and both hash-fidelity
  checks (replayed path == manifest FEN == tree root, by Zobrist hash); no
  new tag or path collides with the standing manifest; job paths are
  disjoint from proven DB paths (assert against the read DB).
- **H2 merge-after-batch**: `proofdb_merge` over the grown manifest:
  conflicts 0; node arithmetic reconciles from the canonical dump; **two
  full merger runs from the same manifest bytes produce byte-identical DB
  and dump** (G3 of plan1, re-run per batch as the backlog requires).
- **H3 no-regression vs. the seed**: every proven node of the seed DB keeps
  its outcome in the grown DB and its `depth_bound` does not increase
  (the refinement fixpoint guarantees ≤; verify over the DBs, not by
  trust). Open seed nodes may become proven; no proven seed node may
  become open.
- **H4 new-fact spot-checks**: every new shard root's row in the grown DB
  matches its manifest (`outcome`, `depth_bound`); `--sample-lines 10`
  replay checks pass; for each new shard root, the parent chain from the
  startpos replays (this is H1's fidelity check, re-run over the DB).
- **H5 hygiene**: `make test` green (including the new unit tests);
  `cargo clippy` / `cargo fmt --check` clean; no `src/` changes; all new
  example files ≤ 10 KB.

A gate failure is a defect in the tool or this plan's model — stop and
investigate, do not loosen the gate.

## 5. Tasks

1. D1: build the standing shard dir + manifest; sanity merge; confirm the
   `nodes` content matches the seed DB (H2's determinism leg doubles as
   this check's second run).
2. Implement D2 (shard_export.rs → manifest writer → proofdb_harvest.rs).
   Unit tests (in `examples/proofdb/merge/tests.rs` style, included via
   `tests/proofdb.rs`'s `#[path]` pattern or inline `#[cfg(test)]`):
   frontier extraction from a DB fixture (deepest-first order), censored
   mapping (`Draw` → no manifest entry), tag determinism/uniqueness,
   manifest writer determinism, manifest-digest guard (stale DB aborts).
3. Run the batch (D3) at the screen budget over all 87 open nodes, then
   the heavy tier; then the merger; check H1–H5; iterate on failures.
4. Write D4 (`report2.md`): census (decisive rate at each tier, cost per
   new fact, censored-tail curve), gate verdicts, findings (including the
   accepted export-helper duplication).

## 6. Non-goals

No `src/` changes; no coverage policy (item 4 — unexpanded siblings and
cheap-first gradients); no DTM-upgrade pass (item 3); no website handoff
(item 5); no parallel/multi-worker harvest (single worker this plan);
no cross-session TT seeding (`--tt-load` reuse of prior sessions' snapshots
is a cheap follow-up, noted for the report's next steps); no incremental
merger (full re-merge per batch; sizes are tiny); no compression.

## 7. Budget

One session. Compute: 87 jobs × ≤ 4 M child-evals at ~2·10⁵ nodes/s ≈
10–25 s worst case per job → ~15–30 min screen; heavy tier 5 × 40 M ≈
10–15 min; two merger runs ≈ 30 s; unit tests minutes. Fits `make test`
plus the batch inside one sitting.

## SESSION COMPLETE

- `docs/plans/proofdb/plan2.md` written (harvest loop, item 2); initiative
  history not yet updated (that happens with report2 at execution)
Follow-up options:
1. Kickoff prompt: "Execute docs/plans/proofdb/plan2.md (harvest loop):
   standing shard dir, proofdb_harvest, first batch over the 87 open
   frontier nodes with the pre-registered screen/heavy budgets, merge after
   batch, gates H1–H5, report2.md."
2. Alternative: run the item-5 website handoff first (ship
   `docs/spec/global_proof_store.md` + the seeded DB) so schema feedback
   arrives before the DB grows under the harvest loop.
