# plan16 — the `frontier-walk` harvest policy: a self-advancing shallowest-first ledger walk

**Initiative:** `proofdb` · **Executes:** backlog item 13 (added with this
plan) · **Size:** S–M (example-side policy + tests + docs; **no `src/`
changes**) · **Date drafted:** 2026-10-13 (plan session, docs-only)

> **Owner framing (2026-10-13 design dialogue):** a new harvest policy that
> bootstraps itself from the sidecar ledger alone, verbatim spec:
> *(a)* of all frontier nodes in the ledger pick one with the shallowest
> depth so far (if empty: root); *(b)* if it was searched before with less
> than the given budget, search it; *(c)* if not, expand it and search one
> of the children with the given budget; *(d)* repeat.
> The five edge-case rulings below were proposed by the plan session and
> accepted by the owner without pushback (name: `frontier-walk`).

## 0. State this plan starts from

- Items 5, 8, 9, 10 closed (plan12–15): pipeline pinned (fixture DB
  `0d929f4c…`, 55,703 nodes, byte-identical rebuild in the default gate),
  operator+user manual at `docs/proofdb_pipeline.md`, generated state
  centralized under `data/proofdb/`, production boot = bootstrap merge +
  `descend` batch 1 (report15).
- **Machinery inventory (verified by code read this session, no probes
  needed):** everything the walk needs exists and is proven —
  the path-keyed work ledger (`examples/proofdb/ledger.rs`: `work_done`,
  `passes_failed`, bump, atomic save), the bump-only censor hook
  (`and_close/driver.rs::on_censored`), the budgeted job pipeline with
  startpos replay + TT retention (`session.rs::run_job_with_budget`),
  the stored-territory sets (DB rows via `load_db_rows`, manifest paths,
  in-session shards via `Session::has_path`), the stop-condition shape
  (`user_stop` in three drivers), and the census record fields
  (`session/record.rs`: free-form `kind: Option<&'static str>`, so
  `search`/`expand` need no record-schema change).
- **Fresh-layer shape (manual §3.1, verified):** the bootstrap merge over
  the empty manifest yields a root-only DB whose single row is the *open*
  root; manifest empty; ledger empty. This is exactly the walk's "(if
  empty: root)" input.
- **Precedent to respect:** stored rows (open *or* proven) are not jobs
  (plan8 D1); a censored record's children are not exposed — bump-only
  (plan8, decision-11 deviation); records on proven paths are dropped at
  session start (lineage gate, plan4 decision 6); jobs behind a proven
  ancestor are excluded (sibling-skip, plan4 decision 9); census
  determinism comes from lex UCI ordering + the sorted `BTreeMap` ledger.
- **Session protocol note:** this is a *live* selection policy (the
  breadth-pns/driver shape), not a precomputed sequence (the
  and-close/descend shape): the frontier mutates between iterations, so
  the selection loop and the batch driver are one module.

## 1. Proposed mechanism (the deliverable)

A new `--policy frontier-walk`: an iterative shallowest-first walk over
the ledger frontier with a flat per-visit budget. One search per
iteration; every search either decides (shard) or censors (ledger
bump); a worked node is expanded into fresh children instead of
re-searched; the frontier grows one record per censored fresh child.

**D1 — flag, name, scope.** `--policy frontier-walk` (owner's naming
ruling). **No new CLI flags**: the budget `B` is `--budget-evals`
(0 → the standing base `BUDGET_PNS_BASE_EVALS`, 4M); stop conditions,
`--tt-mb`, path flags: unchanged tool-wide semantics. Heavy options and
`--pns-config` have no effect under the walk (the standing
"effective only under their policy" convention; no new validation
needed — `--descend-plies` is already descend-only). Census class tags
**reuse the existing classes**: `L` for a search visit on a ledger
record, `C3` for an expansion visit on a fresh child — no `JobClass`
change (the classes carry the true territory meaning; the `policy`
field already identifies the session). No change to `policy.rs`'s
`budget_for` routing (the walk never flows through `jobs_for_policy`).

**D2 — candidate set and selection.** Candidates = ledger records,
minus: (a) records whose own path is a proven DB row (lineage drop,
decision 6); (b) records with a proven DB row on a proper prefix
(sibling-skip, decision 9); (c) in-session *complete* nodes (D3) and
in-session *decided* nodes. Selection: the minimum of
`(ply, space-joined path)` — shallowest first, path-lexicographic
tie-break (the codebase convention). **Root rule:** when the candidate
set is empty, the root becomes a virtual candidate (ply 0, work 0) —
*unless* the root is proven-stored (a proven DB row or a manifest/shard
path), in which case the session stops `exhausted`. A stored **open**
root row is a legitimate first target (the fresh-layer bootstrap; its
censor creates the root's ledger record via the bump hook — the same
"a censored open DB row gets its record on first censor" semantics pns
already has). Note the accepted consequence: on a grown-but-not-merged
layer an empty ledger always spends one root search at `B` unless the
root is proven; that is the spec's bootstrap behavior, not a defect.

**D3 — the two visit kinds; the complete rule.**

- **search visit** (picked candidate with `work_done < B`): run the job
  at `B` (retained TT, so a revisit is never a byte-identical repeat —
  the plan9 §2 concern does not arise here). Decisive → standard shard
  pipeline; the node is dropped from candidacy *in-session*, its ledger
  record left in place (and-close precedent: the next session's lineage
  gate drops it once the merger made the path a proven row). Censored →
  bump its own record (`passes_failed += 1`, `work_done += child_evals`)
  and save atomically.
- **expand visit** (picked candidate with `work_done ≥ B`): replay the
  path, enumerate the legal children (lex UCI order), and search the
  **first unhandled child** at `B`. *Handled* = a stored DB row (any
  outcome) ∪ a manifest path ∪ an in-session shard path ∪ a ledger
  record with `work_done ≥ B`. An underworked ledger child is therefore
  *not* handled — searching it is exactly what D2's rule (b) prescribes,
  so the two visit kinds never contradict. A censored fresh child gets
  its record created by the bump (the frontier-growth step); a decisive
  one exports its shard.
- **complete rule** (livelock design-out, owner ruling 2): a candidate
  with `work_done ≥ B` and *no* unhandled child is marked **complete**
  — removed from in-session candidacy, ledger record retained. A
  complete-marking is a pick-only iteration (no job, no evals); each
  candidate completes at most once, so the loop cannot stall.
- **No intra-session AND/OR propagation:** a decisive child does not
  decide its parent in-session; the merger's closure (and, later, item
  12's promotion) owns that. The walk is a producer of facts, not an
  inference engine.

**D4 — budget and ledger semantics.** Flat budget `B`, **no ladder**
(owner ruling 4): a node censored at `B` is never re-searched deeper by
this policy — expansion replaces deepening (a parent is proven via a
decisive child rather than a bigger budget at the parent), and deeper
revisits remain the ladder policies' territory (and-close/breadth-pns
own them at the next session). Bump-only censor hook
(`on_censored`), **no child exposure** (the decision-11 deviation
verbatim — the walk derives children by movegen at expansion time).
Ledger rewritten between jobs (crash-safe convention). Measurable
signature: the session's ledger growth equals its fresh-node censor
count (an H1-style check for the report). **Accepted deviations from
the pns conventions, recorded as limitations:** no structural-number
exclusions (decision 10 is not applied — implied-decided open nodes are
searched like any other candidate), and the search visit's re-search
can re-spend up to `B − work_done` evals on a node that censors just
under `B` (the retained TT softens this; the yield measurement records
it). The walk is deliberately number-free — that simplicity is its
design point, not an oversight.

**D5 — job pipeline, census, summary.** The existing session pipeline
unchanged (replay → budgeted prefix search → classify → export →
validate → shard + manifest entry). `job:` records: the twelve standard
fields with `policy: "frontier-walk"`, `tier: "frontier-walk"`,
`kind: "search" | "expand"` (reusing the free-form pns field; `number`
stays null), `pass` = `passes_failed + 1` and `work_before` = the
record's `work_done` before the visit (and-close semantics; a fresh
child's first visit is `pass 1`, `work_before 0`). Session-start census
line, shape (exact wording pinned by the executor in the report):

```
frontier-walk: records N (lineage A, ancestor B, complete C) — root <fresh|open|proven>; base budget B
```

Summary line: the standard `harvest: policy frontier-walk stop=… jobs …
decisive … censored … evals … new_shards … wall …` shape.

**D6 — termination, exhaustion, determinism.** Stop conditions are
checked between iterations only (`max_total_evals` on cumulative
child-evals, `max_jobs` counting *searches* — pick-only complete
iterations count toward neither, `max_runtime`, `stop_file`; 0 =
unlimited); a fired cap is a normal stop. Natural `exhausted` (empty
`stop_reason`) = no candidate can yield a visit: every candidate
complete and the root proven-stored. The walk **does not self-retire**
(unlike `descend`): on any undecided layer it always has work, so the
session caps are the binder — documented as the policy's operating
model. Determinism: lex ordering everywhere, sorted-ledger bytes,
deterministic replay; two identical boots must produce byte-identical
manifest digests and ledger bytes (wall times exempt) — the plan15
determinism gate, reused.

**D7 — interaction and composition.** Walk censors enter the same
shared ledger, so the next `and-close`/`breadth-pns` session picks them
up with their ladder state intact; walk shards graft at merge like any
shard's. The walk consults no gradient, no queue, no PNS numbers. It
composes with the production cycle as an *alternative grow policy*
(ledger-frontier-driven, budget-flat) beside `and-close`
(completion-gradient-driven, ladder-paced); the manual's boot path
stays `descend` (batch 1) — the walk is not a boot replacement, though
it *can* bootstrap a fresh layer via the root rule (D2), which the boot
gate below verifies.

## 2. Alternatives considered (rejected)

- **Ladder budgets for search visits** (and-close's strict-growth
  floor): deviates from the owner's explicit spec; the flat `B` with
  retained TT is the mechanism under test. The near-`2×` re-search
  waste at `work_done ≈ B − ε` is accepted and measured (report).
- **Static-scorer child order on expansion** (`StaticAtomicScorer`):
  might find decisive children sooner, but adds a second ordering
  scheme to maintain and weakens reproducibility reasoning; lex-first
  kept (owner ruling 1).
- **Building the walk on the PNS tree** (numbers, eligibility,
  decision-10 exclusions): re-implements `breadth-pns` with a different
  key; the walk's value is being number-free. The decision-10 gap is a
  recorded limitation (D4), revisit-able as a finding if the yield
  measurement shows budget burned on implied-decided nodes.
- **Exposing children at censor time** (pns-style): no consumer — the
  walk derives children by movegen at expansion; bump-only stands.

## 3. Tasks

1. **Implement** `examples/proofdb/walk.rs` (module doc = the normative
   D1–D7 contract, descend.rs pattern; keep ≤ 10 KB, justified header if
   over): candidate filtering, selection, the two visit kinds, the
   complete rule, the root rule, census + summary; the
   `Policy::FrontierWalk` variant (`as_str`/`parse`/`jobs_for_policy`
   arm returning the empty sequence); the CLI branch in
   `proofdb_harvest.rs` (mirroring the descend branch's assembly). Unit
   tests inline (run via `tests/proofdb.rs`'s verbatim include):
   selection order + tie-break; each handled-rule category; the
   underworked-ledger-child-is-a-target rule (D3); the complete rule
   (no livelock on a synthetic all-children-handled node); all three
   root states (fresh / open row / proven → exhausted); lineage and
   ancestor drops.
2. **Fresh-layer boot gate (headline):** empty manifest → bootstrap
   merge (root-only DB) → `proofdb_harvest --policy frontier-walk`
   (defaults) → `proofdb_merge` → `proofdb_flip`. Pre-registered:
   **≥ 1 decisive shard**, merged DB > 1 node, `flips 0 verified 0`.
   Yield is a report finding, not a gate (the walk's first-child descent
   is a different sweep than descend's wide ply-2 enumeration — see
   Risks). Record census + `job:` lines + summary verbatim.
3. **Determinism gate:** repeat task 2 in a second fresh layer →
   byte-identical manifest digest and ledger bytes across the two boots
   (wall times exempt).
4. **Grown-layer smoke:** on the committed fixture layer (explicit
   flags, per the plan14 convention), one capped walk batch
   (`--max-jobs`-bounded, small `--budget-evals`): confirm the census
   shows real ledger-frontier selection (lineage/ancestor drops if any)
   and the summary accounts censor growth. Smoke only — no production
   batch, fixture digests unchanged.
5. **Integration test** in `tests/proofdb.rs` (fast tier, no
   `#[ignore]`): fresh temp layer (empty manifest + bootstrap merge),
   `--policy frontier-walk --budget-evals 20000` (the descend test's
   budget; well under plan4-era find costs) → assert ≥ 1 decisive shard
   and merged DB > 1 node. If the walk yields 0 at 20k (plausible —
   different sweep), the pre-registered fallback is to escalate the test
   budget stepwise (200k → 2M) until decisive or bounded at 2M, and
   record the escalation as a report finding.
6. **Docs:** manual `docs/proofdb_pipeline.md` — §7.2 policy row +
   output-grammar additions (`kind` values, census line), a short
   "frontier-walk" subsection in the growth-cycle section (positioning
   per D7: alternative grow policy, boot stays descend), §11 history.
   `AGENTS.md` `proofdb_harvest` line gains `frontier-walk`.
7. **`report16.md`** (final task): pinned census/summary lines, boot
   yield, determinism digests, grown-layer smoke output, the re-search
   waste measurement (fraction of search-visit evals spent re-doing
   known work, from `work_before` vs `child_evals`), problems,
   deviations, next steps; update the backlog item's status and the
   `docs/plans/README.md` proofdb row (item 13 closes with this plan).

## 4. Acceptance gates

- Task 2/3/5 gates above all green; `make test` green including the new
  integration test.
- `cargo clippy --release --examples`, `cargo fmt --check`, `cargo doc`
  clean.
- **Zero `src/` changes** (policy work is example-side by the pivot's
  convention).
- R1 fixture rebuild digest unchanged: `0d929f4c…`, 55,703 nodes; the
  fixture manifest/shards untouched by task 4 (explicit-flags run into a
  temp layer, or read-only census — executor's choice, documented).
- `git status` clean of `data/` litter.

## 5. Risks and non-goals

- **Risk: fresh-layer yield differs from descend's.** The walk descends
  first-child-first (depth-flavored) where descend sweeps all 400 ply-2
  candidates (breadth-flavored); the decisive class may be reached
  later or not at a small budget. Mitigation: the gate is ≥ 1 decisive
  with the pre-registered budget-escalation fallback (task 5); the boot
  gate runs at defaults where the ladder… no ladder here — where the
  4M base has plan4-era margin. A zero-yield boot at defaults is a plan
  failure investigated as such (root-rule or expansion defect first).
- **Risk: re-search waste** (up to ~2× on near-`B` censors; D4).
  Accepted per spec, measured in the report; if the measurement shows
  it dominating, the finding proposes the strict-growth floor as a
  follow-up — not a silent scope expansion.
- **Risk: frontier growth without bounds.** Every censored fresh child
  deepens the frontier; the walk never exhausts an undecided layer.
  Mitigation: session caps are the documented binder (D6); the ledger
  stays the cross-session pick-up state, so capped sessions resume
  cleanly.
- **Risk: budget spent on implied-decided nodes** (no decision-10
  exclusions, D4). Accepted limitation; the number-aware policies own
  that filtering.
- **Non-goal:** changes to `and-close`/`breadth-pns`/`descend`
  semantics, the merger, the ledger schema, or
  `docs/spec/global_proof_store.md` — the walk is producer-side only.
- **Non-goal:** intra-session AND/OR propagation or any closure logic
  (item 12 owns materialized promotion, merger-side).
- **Non-goal:** parallel harvesters (item 6), subtree harvesting
  (item 7), DTM upgrades (item 3), and the item 11 descend-leaf
  scheduling mode (the walk is a general ledger-frontier policy, not
  the descend-leaf mode; item 11 stays open).
- **Non-goal:** DB growth beyond tooling validation — the 2026-10-02
  pivot stands; the boot gate's shards are validation evidence.
