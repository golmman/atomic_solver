# report16 — the `frontier-walk` harvest policy: shallowest-first ledger walk

Executes `plan16.md` (backlog item 13). **All plan tasks completed; all
pre-registered gates green.** Zero `src/` changes (the product solver is
untouched); all work is example-side plus docs.

## 1. What was built

- **`examples/proofdb/walk.rs` + `walk/driver.rs` (new)** — the
  `frontier-walk` policy per the plan's D1–D7, with the module doc as the
  normative contract:
  - **D2 selection** — the live candidate set over the ledger records
    (`BTreeSet<(ply, key)>`), minus the lineage drop (record path is a
    proven DB row, decision 6) and the ancestor drop (proven DB row on a
    proper prefix — the empty prefix included, so a proven root row drops
    everything, decision 9), minus the in-session complete/decided sets;
    pick = minimum `(ply, space-joined path)`. The **root rule**: an empty
    candidate set makes the root a virtual candidate (ply 0, work 0)
    unless the root is proven-stored (proven DB row or manifest/shard
    path) — then the session stops `exhausted`.
  - **D3 visit kinds** — a *search visit* (`work_done < B`) runs the job
    at the flat `B` (no ladder); an *expansion visit* (`work_done ≥ B`)
    replays, enumerates the legal children in lex UCI order, and searches
    the **first unhandled child** at `B` (handled = stored DB row of any
    outcome ∪ manifest/in-session shard path ∪ ledger record at
    `work_done ≥ B` — so an underworked ledger child is a target, per the
    plan). A candidate with no unhandled child is **complete-marked** by a
    pick-only iteration (no job, no evals; record retained). No
    intra-session AND/OR propagation.
  - **D4 ledger semantics** — bump-only censor hook (reusing
    `and_close::driver::on_censored`), atomic save per censor, no child
    exposure; the summary tracks `fresh_censored` (the ledger-growth
    signature). Flat `B` = `--budget-evals` (0 → the 4M base); no new
    flags.
  - **D5 census/summary** — session-start census line
    (`frontier-walk: records N (lineage A, ancestor B, complete C) — root
    <fresh|open|proven>; base budget B`); job records carry
    `policy`/`tier` = `frontier-walk`, `kind` = `search`|`expand`,
    `pass`/`work_before` (and-close semantics); class tags reused: `L`
    (search on a ledger record), `C3` (expansion on a fresh child), `C1`
    (the virtual root — an open DB row, the true territory).
  - **D6 termination** — stop conditions between iterations only;
    `max_jobs` counts searches (pick-only complete iterations count
    toward neither cap); the walk does not self-retire.
  - `walk.rs` is ~14.6 KB (justified header: the D1–D7 contract doc plus
    the selection state share one indexing scheme; driver split out, the
    `and_close.rs` + `and_close/driver.rs` pattern).
- **`examples/proofdb/policy.rs`** — `Policy::FrontierWalk`
  (`as_str`/`parse`; `jobs_for_policy` yields nothing for it — the CLI
  branch owns the live loop; `budget_for` untouched per D1).
- **`examples/proofdb/harvest_args.rs`** — usage/policy-list text (no new
  flags).
- **`examples/proofdb_harvest.rs`** — the frontier-walk CLI branch
  (mirrors the descend branch: DB rows loaded, standard summary shape) +
  module docs.
- **`tests/proofdb.rs` + `examples/proofdb/walk/tests.rs`** — 11 new
  fast-tier tests: selection order + tie-break; lineage/ancestor drops
  (incl. the proven-root-drops-everything case); the handled-rule
  categories incl. the underworked-ledger-child rule (D3); frontier
  admission + live work + complete-mark accounting; census line + all
  three root states (fresh/open/proven, incl. the manifest-shard-at-root
  proven case); driver root-expansion + ledger growth + byte-determinism;
  the complete rule with no livelock (root completes pick-only, then its
  children expand — capped); proven-root exhaustion; the zero-eval censor
  guard (see findings); and the boot-from-empty integration test
  (`walk_boot_from_empty_layer`: root-only DB → 20k budget, 10M eval cap →
  ≥ 1 decisive shard → merge → DB > 1 node; ~0.8 s).
- **Docs** — `docs/proofdb_pipeline.md`: §7.2 policy row + `frontier-walk`
  bullet + census lines + pinned job records + field-table updates
  (`class`/`tier`/`pass`/`work_before`/`kind`) + the `exhausted` semantics
  note; §3.2 "Alternative grow sweep" subsection (positioning per D7:
  composes through the shared ledger; boot stays `descend`; caps are the
  binder); §11 history. `AGENTS.md` `proofdb_harvest` line gains
  `frontier-walk`.

## 2. Boot gate (task 2, headline) — GREEN

Protocol (both boots): fresh layer → empty manifest → `proofdb_merge`
(root-only DB, `built_from d801aa1fb7ddcc33`) →
`proofdb_harvest --policy frontier-walk --max-total-evals 2000000000`
(4M base default; see finding 1 on the cap) → `proofdb_merge` →
`proofdb_flip`. Pinned lines (boot A):

```
frontier-walk: records 0 (lineage 0, ancestor 0, complete 0) — root open; base budget 4000000
harvest: policy frontier-walk stop=budget jobs 553 decisive 53 censored 500 evals 2001311002 new_shards 53 wall 216.2s
manifest: 53 entries, digest 8a7644b15822054bbe87a39ad68a8230fdefe953ad11b1d0529544525d0b6860
```

- **Pre-registered gate ≥ 1 decisive shard: 53** ✓ (all `win` shards,
  38–2,316 nodes; 44 of 53 zero-eval TT-retention hits).
- Merged DB: **6,706 nodes** (> 1 ✓) — 1 root + 6,647 overlay + 58
  open-ancestor insertions; 6 open rows.
- `proofdb_flip`: **`flips 0 verified 0`** ✓.
- The root state was `open` (the fresh-layer bootstrap target), and the
  very first visit was the virtual-root search (class `C1`, kind `search`)
  — the D2 root rule exercised end-to-end.
- **Yield shape:** the walk's visit mix was 1 search visit + 552 expansion
  visits — after the root censor, *expansion replaces deepening* exactly
  as D3 prescribes, and the sweep is breadth-first by `(ply, lex)` (each
  candidate runs to completion before the next at its ply). First decisive
  in walk order: fresh search #135 (`c2c3 g7g5`, 6,990 evals) at ~620M
  cumulative child-evals (~2.5 min); then a decisive cascade (the `e2e3`
  children, mostly 0-eval TT hits). The plan's Risk 1 (fresh-layer yield
  differs from descend's) resolved favorably: the walk reaches decisive
  ply-2 territory later than descend's wide ply-2 enumeration (620M vs
  descend's ~30M to its first fact) but then harvests the same class
  densely.

## 3. Determinism gate (task 3) — GREEN

Boot B in a second fresh layer: identical job counts (553/53/500), and
byte-identical **manifest digest** (`8a7644b1…`), **ledger bytes**, **all
53 shard files** — and, beyond the pre-registered gate, the **merged DBs
themselves** are byte-identical.

## 4. Grown-layer smoke (task 4) — GREEN

Fixture copy into a temp layer (the committed fixture only read; its
digests untouched — R1 rebuild test in the default gate still passes):
262-shard manifest (`e91ad57b…`) + the 55,703-node rebuilt DB + the plan10
ledger snapshot (8,446 records) seeded as `work.json`; explicit flags;
`--budget-evals 200000 --max-jobs 20 --tt-mb 128`:

```
frontier-walk: records 8446 (lineage 6, ancestor 0, complete 0) — root open; base budget 200000
harvest: policy frontier-walk stop=max-jobs jobs 20 decisive 0 censored 20 evals 4000134 new_shards 0 wall 2.4s
```

- Real ledger-frontier selection: the pick was `f2f3` (a plan10 record at
  ply 1, work 36M ≥ B → expansion stage); its plan10-covered children were
  skipped as handled, and the work-0 plan4-era records (class `L`, kind
  `expand`, `work_before 0`) were re-searched at 200k and re-bumped.
- Ledger accounting: record count unchanged (8,446) — the 20 censors all
  bumped *existing* records, so growth 0 == fresh censors 0 (the D4
  signature holds in both directions).
- Census shows the 6 lineage drops (records that became proven rows since
  plan10) — the D2 drop rules live on grown layers.

## 5. Re-search waste measurement (task 7)

Over boot A's 553 job records: search-kind visits = 1 (the virtual root,
`work_before` 0); expansion-kind visits = 552, `work_before` sum 0. **The
accepted ~2× re-search waste (D4) did not materialize in the boot: 0 of
2.0G evals re-did known work.** Mechanism: a censor at the flat `B`
overshoots slightly (`child_evals ≥ B`), so `work_done ≥ B` immediately
and the node moves to the expansion stage — there is no second search
visit at `B`. The waste scenario requires a record left *under* `B` by
another policy's smaller budget (e.g. and-close's 200k screens): the
grown-layer smoke shows exactly those records being re-searched once at
the walk's `B` (20 jobs, `work_before` 0 — those records were work-0
exposure records, so even there no known work was re-done). A layer with
partially-worked records would show the waste; the measurement code
(`work_before` vs `child_evals` per visit) is in the census records and
the check is cheap to re-run on any batch.

## 6. Problems and deviations

1. **The plan's boot command cannot be run literally uncapped** (task 2
   says "defaults"): the walk does not self-retire (D6 says so itself), so
   a bare `--policy frontier-walk` on an undecided layer never terminates.
   The boot ran at the default base budget with `--max-total-evals 2G`
   added — consistent with D6's "session caps are the binder", documented
   in the manual (§3.2, §7.2) and in `env.json`.
2. **Zero-eval censor guard (beyond the plan, defensive).** A censor can
   spend **zero** child-evals when the Draw is not budget-caused — an
   on-path repetition returns instantly (`g1f3 g8f6 f3g1 f6g8` reproduces
   the startpos; verified: `child_evals 0`, `exit_reason Complete`,
   censored). Under the flat budget such a node can never advance
   (`work_done += 0`), so the spec loop would re-pick it forever. The
   driver complete-marks a zero-eval censor's node (record retained, pass
   count still bumped) — the same no-ladder territory rule as the complete
   rule, applied to a degenerate censor. Regression-tested
   (`walk_zero_eval_censor_guard_completes_the_node`); recorded as a
   deviation for the owner to ratify.
3. **Census `complete C` is 0 at session start** — the complete/decided
   sets are in-session state (the ledger schema is unchanged, per the
   non-goals), so the session-start line always shows 0; the session's
   final count is in `WalkSummary::completes` (reported here, not in the
   pinned summary line). The executor's wording latitude (plan16 D5) spent
   on keeping the plan's field list verbatim.
4. **`cargo clippy` `too_many_arguments`** on the visit bookkeeping
   helper — allowed with a comment (the codebase precedent on
   `run_and_close_session`).
5. No defects found by the gates; the fixture rebuild digest is unchanged
   (`0d929f4c…`, 55,703 nodes) and `git status` carries no `data/` litter
   (all runs in `/tmp` layers).

## 6b. Missing tests / unresolved parts

- The grown-layer smoke exercised the expansion stage on under-B records
  but produced no decisive shard (20 jobs at 200k) — the walk's grown-layer
  *yield* is unmeasured at production budgets; the boot gate covers yield
  on a fresh layer only.
- The `exhausted`-by-complete-root path is tested at unit scale (crafted
  DB + ledger); it was not observed in the gates (the boot never
  completes its frontier within 2G evals — expected, D6).
- The zero-eval guard is exercised for the search-visit case; the
  expansion-target variant (a fresh child censoring at 0 evals) is covered
  by the same code path but not by a dedicated test.

## 7. Next steps

1. **Item 6 (parallel harvesters)** — the standing next lever; the walk's
   ledger-frontier candidate set is a natural per-worker partition input
   (the plan's non-goal note points there).
2. Optional small follow-up: a `--max-jobs`-bounded production-shape walk
   batch on the standing layer at the 4M base to measure grown-layer yield
   per eval against `and-close`'s (the composition claim in D7 is
   untested at production scale).
3. If the owner wants the census's `complete` field live, re-emit the
   census line at session end (a one-line change in the driver; wording
   would need pinning).
