# report15 — self-bootstrapping harvest: the `descend` exploration policy

Executes `plan15.md` (backlog item 10). **All plan tasks completed; all
pre-registered gates green.** Zero `src/` changes (the product solver is
untouched); all work is example-side plus docs.

## 1. What was built

- **`examples/proofdb/descend.rs` (new)** — the `descend` policy per the
  plan's D1–D5: candidate enumeration (all legal startpos-rooted paths of
  length 1..=K, lexicographic UCI order; depth-limited DFS with
  `do_move`/`undo_move` over a shared `Position` — no FEN string
  handling), the D2 skip rule (self-or-proper-prefix stored territory,
  root row never a trigger), the decision-3 disjointness assert on kept
  candidates, the D3 ladder (`max(2^(k−1)·base, 2·work_done)` over each
  candidate's own ledger record, reusing
  `and_close::ladder_budget`), the bump-only censor hook (reusing
  `and_close::driver::on_censored`), the D4 census line, and the batch
  driver + session assembly (`run_descend_batch` / `run_descend_session`;
  records carry `"class":"D"`, `"tier":"descend"`, `pass`/`work_before`).
  ~11.7 KB — over the 10 KB convention; justification in the module docs:
  enumeration, skip rule, census, and driver share one indexing scheme
  and the module docs carry the normative D1–D5 contract (same pattern as
  `and_close.rs`'s 10.1 KB exception).
- **`examples/proofdb/harvest.rs`** — `JobClass::Descend` (`as_str` =
  `"D"`).
- **`examples/proofdb/policy.rs`** — `Policy::Descend` (parse/echo;
  `jobs_for_policy` yields nothing for it — the CLI branch owns sequence
  building); `budget_for(Descend)` arm is unreachable-by-construction,
  commented.
- **`examples/proofdb/harvest_args.rs`** — `--descend-plies <n>` (default
  2; usage error when given outside `--policy descend` or outside
  1..=3), usage/policy-list text.
- **`examples/proofdb_harvest.rs`** — the descend CLI branch (loads the
  DB rows for the skip rule, resolves the base budget like `and-close`,
  same summary-line shape) + module docs.
- **`tests/proofdb.rs`** — 7 new fast-tier tests: enumeration counts
  (pinned 20 / 400 / 8,902; lex order; legal replay), the root-row
  skip-rule case (D2 subtle case), skip rule vs rows + manifest prefixes
  (covered 43 / kept 377 on a crafted DB), ladder budgets from the
  ledger, pinned census lines + `plies_in_range` + policy round-trip,
  driver censor/ledger-growth + byte-determinism, and the boot-from-empty
  integration test (`descend_boot_from_empty_layer`: root-only DB →
  420-candidate descend batch at 20k → ≥ 1 decisive shard → merge →
  DB > 1 node; ~1 s).
- **Docs** — `docs/proofdb_pipeline.md` per task 6 (§3.1 pointer, §3.2
  restructure: step 0 artifact-copying removed, batch 1 = descend, cold
  start rewritten as resolved; §7.2 usage/policy row/`--descend-plies`
  row/output grammar + a pinned descend job record + summary; §8.1 R0
  gains the batch-1 recipe, seeding section retired with a pointer, the
  plan10 ledger snapshot demoted to an optional §4 optimization; §2.2
  tool bullet; §11 history). `AGENTS.md` `proofdb_harvest` line gains
  `descend`. The committed fixture keeps its development/validation role
  (R1, tests).

## 2. Boot gate (task 2, headline) — GREEN

Protocol: fresh `data/proofdb` → empty manifest → `proofdb_merge`
(root-only DB, `built_from d801aa1fb7ddcc33`) → bare
`proofdb_harvest --policy descend` (defaults) → `proofdb_merge` →
`proofdb_flip`. Pinned lines (boot A):

```
descend: candidates 420 (kept 420, covered 0) — ply 1: 20 kept, ply 2: 400 kept; base budget 4000000
harvest: policy descend stop=exhausted jobs 420 decisive 52 censored 368 evals 1474704787 new_shards 52 wall 361.9s
manifest: 52 entries, digest b90b62b86efb9d04f51aa7aaa7e12273de6fffe91e8d78d69c9d596c7be10541
```

- **Pre-registered gate ≥ 1 decisive shard: 52** (expectation band
  "tens" hit exactly; see yield analysis below).
- Merged DB: **4,314 nodes** (> 1 ✓) — 1 root + 4,256 overlay + 57
  open-ancestor insertions; 6 open rows (the ply-1 parents + root).
- `proofdb_flip`: **`flips 0 verified 0`** ✓.
- Yield split: ply 1 **0/20 decisive** (as expected — the deepest
  positions), ply 2 **52/400**. All 52 are `win` shards, 38–334 nodes,
  0–2.5M child-evals (the 0s are TT-retention hits on positions already
  proven in earlier jobs' TTs).
- Wall time ~6 min per boot on the reference host (defaults; no knobs).

**Yield vs the plan4 precedent.** plan4's 8 s screen found 55 decisive
ply-2 positions; the boot finds a subset of exactly 52 — the 3 missing
(`e2e3 g7g6`, `e2e3 h7h5`, `g1h3 g7g5`) are precisely plan4's heavy tail
(36.7k–1.05M solver nodes there), beyond the 4M first ladder rung. The
ladder picks them up at pass 2 (the ledger records their pass-1 censors);
no yield anomaly to investigate.

## 3. Determinism gate (task 3) — GREEN

A second fresh-layer boot (identical protocol) reproduced:

- manifest **byte-identical** (digest `b90b62b8…` both boots);
- ledger **byte-identical** (sha `5b30335e…` both boots);
- all 52 shard files **byte-identical** (`diff -rq` clean);
- all 420 `job:` records identical after the `wall_s` strip (evals total
  `1474704787` both boots; only `wall` differs, 361.9 s vs 353.3 s).

## 4. Handoff smoke (task 4) — GREEN

One `and-close` batch on the grown layer (`--budget-evals 200000
--max-jobs 5`): the frontier is now `C1 6 / C2 69137 / C3 63` (root-only
was `C1 1 / C2 0 / C3 20`), the census reports `active rows 6` — exactly
the descend-opened ply-1 parents — and the selected jobs are C3 replies
of those rows resuming at `pass 2` from the descend censor records
(`work_before` = the boot's 4M spends, budget = 2 × work). Composition
per D5 confirmed; smoke only, the layer was restored to the pristine
boot-B ledger bytes after the probe.

## 5. Measurements and records

`docs/plans/proofdb/measurements/plan15/`: `README.md` (protocol +
pinned results), `env.json` (rev, protocol, output-state digests,
determinism flags), `boot_census.json` (boot A census + summary + 420
job records), `verdicts.json` (boot A vs B comparison). Raw transcripts
live in `/tmp` (regenerable byte-identically by the protocol; not
committed, per the measurement-layout convention). `data/proofdb/` is
left in the boot-B grown state (gitignored working layer).

## 6. Problems, deviations, findings

- **Boot protocol requires the R0 bootstrap merge.** Task 2's "bare
  `proofdb_harvest --policy descend`" presupposes the R0 manifest +
  root-only DB — a missing default manifest still aborts cleanly
  (plan14 D3), and the skip rule needs the DB rows. Documented in §3.2/R0;
  the *artifact-copying* step is what plan15 removed. No code deviation.
- **`descend.rs` is 10.6 KB** (~5% over the 10 KB convention) — justified
  in the file header (see §1); splitting would separate the enumeration
  index scheme from its only consumers.
- **No defects found by the gates.** The D2 skip rule, root-row case,
  ledger bump on off-tree paths, and merge-time disjointness all held on
  both the unit tests and the 420-candidate real boots.
- **Determinism is stronger than the gate asked**: not just manifest +
  ledger bytes but the full shard set and job records reproduce
  byte-identically. Worth relying on in future harvest tooling tests.
- Minor observation: TT retention across jobs makes some decisive jobs
  cost 0 child-evals (already proven in the session's private TT). The
  `job:` records stay truthful (`child_evals: 0`); the census consumer
  should expect this under every policy with a retained session (not
  descend-specific).

## 7. Missing tests / unresolved parts

None known. Not covered and deliberately out of scope per the plan:
`--descend-plies 3` end-to-end (cost-prohibitive; the enumeration count
8,902 is pinned by unit test), `breadth-pns` interaction with a grown
descend layer (unchanged machinery), parallel harvesters (item 6),
subtree harvesting (item 7), DTM upgrades (item 3).

## 8. Gate receipts

- `make test` — all green (68 proofdb integration tests incl. the 7 new
  + boot integration test; standing-layer rebuild digest unchanged
  `0d929f4c…`, 55,703 nodes ⇒ R1 fixture untouched).
- `cargo clippy --release --examples --tests` — 0 warnings.
- `cargo fmt --check` — clean.
- `cargo doc` — clean.
- `git status` — no `data/` litter (gitignored); only the intended
  source/doc changes.

## 9. Next steps

1. **Recommended:** item 6 — parallel harvesters. The sequential
   pipeline is now complete end-to-end (bootstrap → grow → validate);
   `descend`'s 420 independent candidates are also a natural sharding
   probe for a parallel boot.
2. Alternative: run one real production `and-close` batch over the
   descend-grown layer (the manual's batch cycle) to confirm the
   production cadence on the fresh 4,314-node substrate.
