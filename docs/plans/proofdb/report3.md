# Report 3 — coverage policy: frontier classes beyond the open nodes, measured A/B

Executes `plan3.md` (backlog item 4). All deliverables landed; all
pre-registered gates **pass**. The A/B verdict is decisive in both
directions: the sharpness gradient (`sharp-siblings`) produced **90 new
facts at 300M child-evals where the deepest-first baseline produced 0**,
and the *marginal* re-run of the same gradient after merging those facts
produced **0 facts at 500M** — the cheap head of the sibling class is real,
shallow, and small. Winner recorded as the default `--policy` (decision 7).
The DB grew 12,655 → 35,055 nodes; the standing shard dir 107 → 197 shards.

## Deliverables

- **D1** policy machinery, `examples/proofdb/` (+ tests via
  `tests/proofdb.rs`), all files ≤ 10 KB:
  - `db.rs` — read-only node extraction in path form (parent-chain walk,
    ply-consistency assert, `built_from` digest guard, startpos root check).
  - `frontier.rs` (+ `frontier/tests.rs`) — full-frontier extraction by DFS
    replay from the startpos: C1 open rows, C2 unexpanded children of proven
    win nodes, C3 unexpanded children of open nodes; class rankings
    (C2 by parent `depth_bound` ascending / C3 by ply / C1 deepest-first);
    the AND-completeness assert over every proven loss node; decision-3
    disjointness (job path ≠ any DB row, ≠ any manifest path) enforced at
    extraction time; C2/C3 legality by construction (positions come from
    the replay). O(rows + frontier): one movegen pass per win/open/loss
    row — the real 222k-job C2 enumeration builds in ~0.5 s.
  - `policy.rs` (+ `policy/tests.rs`) — the three pre-registered policies
    (`open-deepest`, `sharp-siblings`, `sharp-heavy-tail`), per-class
    budget resolution (C2 = 1M, C1/C3 = 4M), name parsing.
  - `batch.rs` — the screen-pass driver with the screen-budget cap
    (`--max-total-evals`, between-jobs check on cumulative child-evals) and
    the heavy-tier placement: for `sharp-heavy-tail` the C2-only censored
    sample runs where the C2 screen ends (first non-C2 job, frontier end,
    or budget stop), *before* C3/C1; not counted against the screen budget;
    user-facing stops skip it (plan2 finding 6 semantics preserved).
  - `fixture.rs` — shared merger-built fixture DB (both graft routes are
    legal replays — the pre-plan3 fixture's second graft was unphysical and
    only passed because the old extractor never replayed; the new
    extraction caught it immediately).
  - `harvest.rs` slimmed to job mechanics (`JobClass` on `Job`,
    classification, tags, replay-prefix contract); `session.rs` gained
    job-budget resolution and the new census fields.
- **D2** CLI (`proofdb_harvest`): `--policy` (default now `sharp-siblings`,
  decision 7; `open-deepest` reproduces plan2), `--max-total-evals`,
  `--budget-evals` as explicit override (0 = policy-resolved); census
  `job:` lines now carry `policy`, `class`, `parent_bound`; summary line
  carries policy, stop reason, and screen evals. No changes to
  session/export/manifest machinery; no `src/` changes.
- **D3** `measurements/plan3/`: `proofdb_input.db` + `nodes_input.txt`
  (the A/B input, byte-identical to plan2's grown DB, sha256 `949d6e59…`),
  per-policy `census_<policy>.json` (765 job records total),
  `policy_comparison.json`, `proofdb_grown2.db` + `nodes_grown2.txt`
  (197 shards, 35,055 nodes; two merger runs byte-identical, sha256
  `670e19e3…` / `e3060d14…`), `proofdb_after_p1.db` (= P2's exact input,
  byte-identical to grown2 since P2 added nothing), `sample_lines.txt`,
  `assemble_census.py` (gate checks + census assembly driver), `env.json`,
  `README.md` (provenance + command table). Standing shard dir + manifest
  grown append-only (90 new `h_*` shards). Raw transcripts not committed
  (measurement conventions).
- **D4** this report.

## The A/B (pre-registered metric: new facts per 100M child-evals)

| policy | screen | new facts | evals | facts/100M | cost/fact median | stop |
| --- | --- | --- | --- | --- | --- | --- |
| P0 `open-deepest` | 75 C1 jobs @ 4M | 0 | 300.0M | 0.0 | — | exhausted |
| **P1 `sharp-siblings`** | 385 C2 jobs @ 1M | **90** | 300.1M | **29.99** | 44.7k evals | budget |
| P2 `sharp-heavy-tail` (marginal) | 300 C2 @ 1M + 5 heavy @ 40M | 0 | 300.0M + 200M | 0.0 | — | budget |

- **P0** confirmed its pre-registered expectation exactly: the 75 remaining
  open nodes (the plan2-depleted quiet plateau) are all undecidable at 4M;
  300M evals bought the measurement, not a fact.
- **P1** won exactly where the hypothesis pointed: all 90 facts are
  **unexpanded siblings of mate-in-1 proving children** — every
  `… d1a4 <try>` under the 1.f3 e6 2.g4 fool's-mate family, each a `loss`
  refutation proving White's try throws the win away. All 90 at ply 5,
  all under `parent_bound = 1` win parents; cheapest 65 evals, median
  44.7k, mean 56.6k. The screen spent 300.1M but the facts themselves cost
  ≈ 5.1M — the rest is 295 censored 1M jobs (see finding 3).
- **P2** measured where the class stops being cheap, and the answer is
  *immediately*: after merging P1's facts, the C2 frontier **doubled** to
  574,596 jobs (P1's 90 new win nodes contribute their own sibling sets),
  and the next 300 bound-1-ranked C2 jobs produced 0 facts at 1M each; the
  5-job heavy tail (first censored C2 in job order) stayed undecided at
  40M each. The cheap stratum is the *direct* siblings of mate-in-1 proofs;
  one step deeper the class is as quiet as the C1 plateau.

**Winner (decision 7): `sharp-siblings`** — highest facts/100M (29.99 vs
0/0; no tie-break needed). Recorded as the default `--policy`.

## Gate verdicts

| gate | verdict | notes |
| --- | --- | --- |
| H1 job/shard integrity | **pass** | every shard replay-validated + manifest cross-check + hash fidelity inside the merger (197/197, conflicts 0); AND-completeness assert walked all 6,291 (input) / 17,536 (P2 input) loss rows with zero violations — a free full-tree soundness check; decision-3 disjointness enforced by the extractor on all classes, all runs; every C2/C3 job path replayed legally at run time (385 + 300 + 5 + the H5 replay's 385) |
| H2 merge-after-batch determinism | **pass** | two full merger runs → DB sha256 `670e19e3…` both, dump `e3060d14…` both |
| H3 no-regression vs input DB | **pass** | all 12,580 input proven nodes: outcome kept, `depth_bound` not increased, none proven→open (12,655 → 35,055 nodes; open stays 75 — C2 facts only add proven descendants, as the plan's model predicted) |
| H4 new-fact spot-checks | **pass** | 90/90 new shard roots: DB row outcome == manifest outcome; skeleton re-validations 197/197 in the final merge; `--sample-lines 10` recorded in `sample_lines.txt`; parent-chain replay is the merger's per-shard hash-fidelity check |
| H5 policy determinism | **pass** | P1 re-run from `proofdb_input.db` + fresh TT (throwaway manifest/shard-dir; pre-P1 manifest state rebuilt by excluding the 90 census tags → digest `61866d6c…` exact): 385/385 screen records identical on every field except wall time, identical job sequence |
| H6 hygiene | **pass** | `make test` green (all 32 test binaries, incl. 8 new policy/frontier engine tests); `cargo clippy --all-targets` 0 warnings; `cargo fmt --check` clean; no `src/` changes; all new/edited example files ≤ 10 KB (largest: `frontier.rs` 9.3 KB) |

## Findings

1. **The sharpness gradient holds — but only for the direct-sibling
   stratum.** Cost-per-fact at the C2 head (median 44.7k evals, floor 65)
   matches plan2's cheapest decided jobs and the solve-plan4 refutation
   class. But P2's marginal 0/500M shows the gradient is not a gradient
   down the class: it is a thin cheap head (siblings of bound-1 wins)
   followed by plateau. The `depth_bound`-ascending ranking correctly
   prioritized that head, and equally correctly kept serving bound-1 jobs
   after it was exhausted — the ranking cannot distinguish "sibling of a
   1-move mate" from "sibling of a wide quiet win that happens to carry
   bound 1". A next-generation policy would need a structural feature
   (e.g. parent bound == 1 **and** the proven child is a rule-terminal) to
   stop after the head instead of censoring 295 × 1M jobs.
2. **Screen budget dominates the spend.** P1's 90 facts cost ≈ 5.1M evals;
   the censoring of the other 295 jobs at 1M each cost ≈ 295M. A cheap
   screen (≈ 100k evals) followed by a confirm pass on survivors would
   likely have harvested the same 90 facts at ~10× lower total spend.
   The fixed 1M per-class budget was pre-registered (decision 5, no
   mid-run tuning), so this is recorded as a measurement, not applied.
3. **Marginal-yield accounting worked as designed.** Because each policy
   ran against the re-merged DB, P2 measured the true marginal yield of
   the C2 class (0 at 500M) rather than re-finding P1's facts — a job path
   that gained a shard left every frontier class by construction. The A/B
   numbers are directly comparable despite P2 running over a 35k-node DB.
4. **The frontier explodes with growth.** C2 went 222,084 → 574,596 after
   90 facts (each new win node adds its own sibling set; loss nodes add
   none — AND-completeness). Coverage of the *full* unexpanded class is
   therefore hopeless by enumeration; the value of a policy is entirely in
   its head. This strengthens the case for the item-5 website handoff:
   the DB already answers 34,980 proven positions, and the questions users
   actually ask (non-stored tries under sharp wins) are exactly the class
   P1 harvested.
5. **The AND-completeness assert is cheap enough to run every session**
   (17,536 loss rows + movegen ≈ 0.3 s) and is now a standing invariant of
   every extraction — any future merger regression that drops a refutation
   row aborts the harvest session before it can build on the defect.
6. **File-size convention forced the planned split** (as in plan2):
   `policy.rs` was designed as one module, but extraction + policies + the
   batch driver exceeded 10 KB, so they landed as `db.rs` / `frontier.rs` /
   `policy.rs` / `batch.rs`. Pure code motion; behavior verified by the
   unit tests and the H5 replay. One test fixture was *fixed*, not moved:
   the shared fixture's second graft (`e2e4 f7f6 g2g4` + `d8h4`) was an
   unphysical row (the queen path is blocked there) that the old
   never-replaying extractor tolerated; the new replay-based extraction
   rejected it, and the fixture now uses a second legal move order to the
   same tactic position (`g2g4 e7e6 f2f3`).

## Problems encountered

- The H5 replay initially aborted on the digest guard: the throwaway shard
  copy carried the *grown* manifest (197 entries) against P1's input DB
  (built_from `61866d6c…`). Correct behavior — the guard did its job; the
  pre-P1 manifest state had to be reconstructed by excluding the 90 census
  tags (deterministic manifest writer reproduced `61866d6c…` exactly).
- No other defects: the batch ran clean end-to-end; the only tool-side
  change after the runs was flipping the `--policy` default (decision 7).

## Unresolved / next steps

1. **Website handoff (item 5)** is now the highest-leverage step: 34,980
   proven positions, a spec'd schema, and the user-facing coverage class
   (siblings of sharp wins) freshly harvested. `sharp-heavy-tail`'s
   saturation also means there is no cheap growth left before the DTM
   work — hand off, gather real query patterns, then decide.
2. **Screen-budget split** (finding 2): a 2-tier screen (100k probe +
   confirm) would cut the cost/fact of future batches ~10×; small CLI
   change, worth doing before the next large batch.
3. **Structural head-detection** (finding 1): stop the C2 screen once the
   bound-1-with-terminal-child stratum is exhausted, instead of censoring
   the plateau; needs a cheap DB-side feature, not a search change.
4. **Cross-session TT seeding** (`--tt-load`) remains deferred (non-goal
   here); with 197 shards and 35k nodes, deep-prefix retention across
   sessions would compound — but it confounds determinism bookkeeping and
   should ride on a replayable snapshot design.
5. **DTM-upgrade pass (item 3)** unchanged; the 90 new loss facts are all
   `bound`-labeled with `depth_status = exact` at the rule-terminals only.

## Tools used

- `cargo build/test/clippy/fmt` (release), `python3` (census assembly, H3
  diff, manifest reconstruction), `sqlite3` via python for spot queries;
  the merger/harvest binaries. No new external tooling. One throwaway
  probe run in `/tmp` (extraction statistics; wrote nothing durable).
