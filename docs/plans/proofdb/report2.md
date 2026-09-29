# Report 2 — harvest loop: standing shard dir, `proofdb_harvest`, first batch, merge-after-batch

Executes `plan2.md` (backlog item 2). All deliverables landed; all pre-registered
gates H1–H5 **pass**. One tool defect was found by the run itself (root-job false
repetition), fixed, unit-tested, and corrected; the batch was then shown fully
reproducible by replay. **12 of the 87 seed-frontier nodes decided** at the screen
budget (all wins) — far above the plan's near-zero expectation — growing the DB
from 12,081 to 12,655 nodes.

## Deliverables

- **D1** standing shard dir `docs/plans/proofdb/shards/`: the 95 seed `.bin`
  files copied from the plan4 artifact + this batch's 12 `h_*` shards +
  `manifest.json` (required fields only, deterministic bytes via
  `proofdb::write_manifest`; seed tags unchanged). Sanity merge over the
  standing dir reproduced the seed DB's `nodes` content **byte-for-byte**
  (12,081 rows identical; only `built_from`/`generator` meta differ, as the
  plan allows). Driver: `measurements/plan2/build_standing_manifest.py`
  (adapted from plan1's; two runs byte-identical).
- **D2** tooling, all in `examples/proofdb/` (+ the CLI):
  - `shard_export.rs` — TT snapshot → `reconstruct` → `validate_proof_tree`
    → `ProofTree`, adapted from the campaign worker's
    `export_via_reconstruction` with an attribution header;
    `campaign_worker.rs` untouched (accepted duplication, finding 4).
  - `manifest.rs` — `write_manifest` (required fields only, sorted by tag,
    deterministic bytes, duplicate tag/path rejected at write time; returns
    the `built_from` digest).
  - `harvest.rs` (+ `harvest/tests.rs`) — the DB-facing engine: read-only DB
    extraction with the `built_from` digest guard, path reconstruction with
    parent-chain/ply asserts, deepest-first frontier (ply desc, ties
    lexicographic path), `tag_for_path` (`h_` + sha256(path)[..16]),
    `classify` (Draw → censored), `replay_job_path` (prefix = ancestors
    only; **empty for the root job** — finding 1).
  - `session.rs` — the retained-`Search` session: budgeted
    `search_depth_with_prefix` per job, `first_outcome_only` on,
    `produce_shard` (export → validate → write bin → manifest entry;
    collision asserts), stop conditions, manifest rewrite.
  - `proofdb_harvest.rs` — the CLI exactly as spec'd; every option
    exercised at least once except `--max-runtime`/`--out-db`/`--dump`
    (the latter two are recorded in the summary only — the harvest tool
    never writes a DB or dump; interpretation recorded as finding 5).
- **D3** `measurements/plan2/`: `proofdb_grown.db`, `nodes_grown.txt`,
  `census.json` (93 job records incl. the root-job correction; tiers,
  merge arithmetic, session shape, gate verdicts), `env.json`, `README.md`
  (provenance/command table), `proofdb_input.db` + `nodes_input.txt` (the
  batch's exact input DB, kept for the H3 diff), `sample_lines.txt`.
  Raw run transcripts were deleted after census assembly (measurement
  conventions); the census is the record.
- **D4** this report.

## Census (the batch)

Single in-process worker, one `Search` (TT 128 MB) retained across jobs,
budgets as pre-registered. Job order deepest-first over the 87 seed open
nodes. Wall ≈ 106 s.

| tier | jobs | decisive | censored | evals total | wall |
| --- | --- | --- | --- | --- | --- |
| screen (4M child-evals) | 87 | **12** (all `win`) | 75 | 296.4 M | 62.8 s |
| heavy (40M child-evals, first 5 censored) | 5 | **0** | 5 | 200.0 M | 42.3 s |

- The 12 decisive jobs are wins at **even plies 4–27** on the quiet
  ladder lines (`g1f3`/`d2d4` + `a2a3 a7a6 b2b3 a5a4 …` chains) — i.e. the
  plan4 censored plateau *is* decidable at 4M child-evals once the retained
  TT carries the residue of the deeper (censored) prefixes: the deepest
  decisive jobs spent only 1.4–2.4k evals (order-of-magnitude retention
  effect, order recorded in the census). Marginal cost of the facts
  themselves: ~373k evals for 12 shards (≈31k/fact, median 37.7k); the
  screen's 296M spend mostly bought the *measurement* that the remaining 75
  nodes are not decidable at 4M.
- The censored tail is genuinely expensive: the 5 deepest quiet-ladder
  positions (plies 26–31) are still undecided at 40M child-evals each — the
  "bottomless quiet class" measurement from `solve` plans 1–3 reproduces
  under path-context budgets.
- All 12 new shards are `win` (OR-side proofs). The four `loss` seed classes
  saw no new loss facts this batch — expected: the frontier was quiet-ladder
  ancestors of existing shards.
- Merge after batch (`proofdb_merge` over the grown manifest): 107/107
  shards merged, conflicts 0; 12,655 nodes = 1 root + 12,479 overlay + 175
  ancestors; open nodes 87 → 75 (12 frontier upgrades; 101 open upgrades
  total incl. cross-route upgrades inside the new subtrees); proven
  11,994 → 12,580 (win bound 5,996 → 6,289; loss bound 331 → 346, loss
  exact 5,667 → 5,945 — the new win subtrees contribute their AND-children
  as loss facts, and the refinement fixpoint re-classifies); skeleton
  re-validations 107/107.
- Root-job correction: the batch's root job (path `""`) returned an instant
  false repetition-`Draw` (0 evals — finding 1); harmlessly censored. Re-run
  standalone after the fix: censored `BudgetExhausted` at 4M evals (0.79 s).

## Gate verdicts

| gate | verdict | notes |
| --- | --- | --- |
| H1 job/shard integrity | **pass** | every shard: merger replay validation + manifest cross-check + hash fidelity (all inside `load_shard`); in-tool asserts: no tag/path collision with the standing manifest, job paths disjoint from proven DB paths and from manifest paths |
| H2 merge-after-batch determinism | **pass** | two full merger runs → DB sha256 `949d6e59908fccbb…` both, dump `f787c2f92719e4ca…` both |
| H3 no-regression vs. seed | **pass** | all 11,994 proven seed nodes: outcome kept, `depth_bound` not increased (checked per path over the DBs); 12 open upgrades; no proven node became open |
| H4 new-fact spot-checks | **pass** | all 12 new shard roots: DB row outcome == manifest outcome, provenance carries the tag; `--sample-lines 10` replays recorded in `sample_lines.txt`; 0 bad parent links, 0 orphans over all 12,655 rows |
| H5 hygiene | **pass** | `make test` green (365 passed / 0 failed, incl. 6 new engine + manifest-writer tests); `cargo clippy --all-targets` 0 warnings; `cargo fmt --check` clean; no `src/` changes; all new files ≤ 10 KB |

**Beyond the gates — full batch replay.** After the finding-1 fix, the whole
batch was re-run from the regenerated input manifest (fresh TT, same
budgets): 92/92 records identical (paths, outcomes, tags, shard sizes,
screen evals) except the superseded root record; shards, manifest, grown DB
and dump **byte-identical**. The heavy-tier records differ by ±30 child-evals
at the budget cut — explained, not noise: the root fix changes the TT residue
entering the heavy tier (root screen search now actually runs; its unsolved
bounds are advisory-only). Screen pass is exactly deterministic.

## Findings

1. **Harvest-tool defect: root-job false repetition (caught by the run,
   fixed).** `replay_job_path` originally always seeded the prefix with the
   startpos key — for the root job (empty path) the search then saw the
   startpos twice on its own path and returned `Draw`/`Complete` with 0
   evals. Soundness was never at risk (decision 1 censored the `Draw`; a
   repetition verdict can only be `Draw`, never a wrong win/loss), but the
   job's work measurement was wrong and the frontier's most important node
   went unsampled. Fixed (prefix = ancestors only, empty for the root),
   unit-tested (`replay_prefix_excludes_the_node_itself`), root job
   corrected standalone. **The same latent issue exists in the campaign
   worker's `replay_path`** (`examples/campaign/mod.rs`): a job equal to the
   campaign root would double the root key on its own path. Left as-is
   (frozen prototype); recorded here for whoever reopens the campaign work.
2. **Pre-registered budgets were too conservative for this frontier** — in
   the good direction. The plan expected a near-zero decisive rate on the
   graft-ancestor plateau; 12/87 decided at the screen budget. The decisive
   subset is exactly the "deepest ancestors nearest the sharp tactical
   end" the plan flagged as most plausible — but they sit on quiet ladder
   lines, not tactical ones, and only become cheap *with TT retention from
   the censored deeper prefixes* (job-order effect, order recorded).
   Implication for item 4 (coverage policy): deepest-first + retention is
   a stronger gradient than its parts.
3. **Manifest extras dropped from the standing manifest.** The standing
   manifest keeps required fields only (it is the file
   `proofdb::write_manifest` rewrites); the seed entries' provenance extras
   (`tier`, `dtm_length`, `tree_nodes`, `path_source`) remain in plan1's
   `manifest_seed.json` and in the DB `shards` table's own claims. The seed
   DB's `built_from` therefore changes on the first growth batch (digest
   `089bcc1d…` → `61866d6c…`); `proofdb_input.db` + the deterministic
   generator keep the old state reproducible.
4. **Accepted duplication: `shard_export.rs` vs `campaign_worker.rs`'s
   `export_via_reconstruction`** (~40 lines). `campaign_worker.rs` is a
   frozen measured prototype (plan5/plan9); dedupe only if that work
   reopens.
5. **`--out-db` / `--dump` interpretation.** The plan lists them as harvest
   CLI flags while also stating the harvest CLI never merges. Implemented
   as *recorded-only*: they name the caller's `proofdb_merge` targets and
   are echoed in the session summary (no file writes). If the intended
   semantics was something else (e.g. copying the input DB), it is a
   one-line change.
6. **Heavy tier runs only after an uninterrupted screen pass.** If a stop
   condition fires during the screen pass, the heavy sample is skipped
   (the pre-registered "first N censored in job order" is defined over the
   full pass; a partial-pass sample would not be the pre-registered
   sequence). Stopping mid-screen then simply ends the session; the next
   session redoes the order deterministically.
7. **Search-noise asymmetry at the budget cut.** Screen jobs reproduce
   exactly; heavy-tier child-evals overshoot the 40M budget by 1–36 evals
   varying with TT residue. Noted for anyone using `child_evals` as a
   deterministic metric near budget cuts (the `benchmark`/optimizer
   contract is unaffected — it never mixes TT residue across runs).

## Problems encountered

- The session's file-size convention (`≤ 10 KB`) forced one refactor
  mid-plan: `harvest.rs` tests → `harvest/tests.rs`, manifest writer →
  `manifest.rs`, session state → `session.rs`. Pure code motion; verified
  behavior-neutral by the full batch replay (finding above).
- During that refactor one intermediate state briefly truncated the
  harvest test module; reconstructed in full and confirmed by
  `make test` (365/365).

## Unresolved / next steps

1. **Item 4 (coverage policy)** is now the natural next step: the 75
   remaining open nodes are all ≥ 40M-eval quiet positions under the
   deepest-first rule; cheap-first or sibling-of-proof gradients may beat
   it. The measured retention effect (finding 2) suggests ordering jobs so
   deep prefixes precede their shallow relatives.
2. **Cross-session TT seeding** (`--tt-load` reuse of prior sessions'
   snapshots) — cheap follow-up, explicitly a non-goal this plan; would
   make heavy-tier spend cumulative across sessions.
3. **DTM-upgrade pass (item 3)** unchanged.
4. **Website handoff (item 5)**: the grown DB is the first DB produced by
   the harvest loop; shipping spec + DB now includes 12 harvest tags.
5. The root `""` node remains the single most important open fact (and is
   now known to cost > 4M evals under path context); the eventual
   startpos handover rule (initiative) hinges on the ~20 ply-1 children
   class, of which the quiet ones are the expensive tail.

## Tools used

- `cargo build/test/clippy/fmt` (release), `python3` (census assembly,
  DB diffs, manifest generation), the merger/harvest binaries; no new
  external tooling. Two throwaway probe binaries were built under
  `examples/` for the finding-1 diagnosis and deleted afterwards.
