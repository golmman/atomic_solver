# plan15 — self-bootstrapping harvest: the `descend` exploration policy

**Initiative:** `proofdb` · **Executes:** backlog item 10 (added with this
plan) · **Size:** S–M (example-side policy + tests + docs; **no `src/`
changes**) · **Date drafted:** 2026-10-08 (plan session, docs-only)

> **Owner framing (2026-10-08 brainstorm):** the manual's production
> bootstrap currently copies committed artifacts (fixture shards + plan10
> ledger snapshot) into a fresh layer. The owner rejects artifact-copying
> as a production step — "the seeds were produced somehow; document how to
> create them." The seeds' method is known, cheap, and was implemented
> once (solve plan4's sharp-class enumeration); this plan productizes it
> as a harvest policy so a fresh clone bootstraps by running the pipeline,
> not by copying files.

## 0. State this plan starts from

- Items 5, 8, 9 closed (plan12/13/14): pipeline pinned (standing DB
  `0d929f4c…`, 55,703 nodes, byte-identical rebuild in the default gate),
  operator+user manual at `docs/proofdb_pipeline.md`, generated state
  centralized under `data/proofdb/`.
- **Cold start verified this session (read-only probes, 2026-10-08):**
  a fresh root-only layer under `--policy and-close` runs to
  `stop=exhausted jobs 20 decisive 0` (~80M evals) — the job set is
  exactly the root's 20 first-move replies, the deepest positions in the
  tree; a root reply censors at 268M child-evals; `breadth-pns` visits
  the same territory. Root cause: job selection is entirely DB-frontier-
  driven (`examples/proofdb/frontier.rs` C1/C2/C3) — the harvester is an
  *exploitation* loop and cannot generate off-tree jobs.
- **Seed provenance (traced this session):** the 95-shard seed set
  (`lad_*`/`p2_*`/`sib_*` tags) is the `solve` initiative's sharp-class
  campaign residue — plan1's self-play ladders (PV-walking) and plan4's
  **full-width ply-1/ply-2 enumeration** (~450 positions expected,
  bounded solves, decisive finds at 1–4.7k solver nodes, "seconds of
  find time"), implemented as a one-off measurement driver
  (`solve/measurements/plan4/`) and never productized. The proofdb
  pipeline has been growing by exploitation ever since; its bootstrap
  step was never brought in-repo.
- **Enumeration probed this session** (repo binaries `list_legal` +
  `replay`): startpos → 20 first moves; every first move → exactly 20
  legal replies → **400 ply-2 candidate paths** (plan4 estimated ~450;
  measured 400).
- **Precedent to respect (plan3 / backlog item 4):** frontier *coverage*
  policy deliberately rejected enumerative layers ("sharpness like
  plan4's, not enumerative layers"). This plan adds no coverage policy:
  `descend` generates **off-tree bootstrap candidates only**, hard
  width-capped, and self-retires on a grown layer (§1, decision D2).
- Machinery the policy needs already exists and is proven for arbitrary
  paths: the job pipeline replays each path from the startpos (the shard
  format is explicitly designed for 2–38-ply roots — the 95-seed set),
  the work ledger is path-keyed with a bump-only censor hook (off-tree
  keys work), the merge grafts shards at arbitrary manifest paths with
  open-ancestor insertion, and budgets are deterministic child-evals.

## 1. Proposed mechanism (the deliverable)

A new `--policy descend` in `proofdb_harvest`: enumerate all legal
startpos-rooted paths of length 1..=K, bounded-solve each under the
standard budget ladder, and export decisive ones as shards — exactly the
solve plan4 method, now inside the harvester.

**D1 — flag and scope.** New option `--descend-plies <n>` (default 2,
accepted range 1..=3; values above 2 documented as cost-prohibitive —
ply 3 enumerates ~10k paths, ply 4 ~hundreds of thousands). Effective
only under `--policy descend`; usage error otherwise (same convention as
the heavy options under `breadth-pns`).

**D2 — candidate set and skip rule.** Candidates are all legal paths of
length 1..=K from the startpos, in lexicographic UCI order
(deterministic). Skip a candidate iff

- the path itself is a stored DB row (any outcome, including open) or a
  manifest shard path, **or**
- any proper prefix of length ≥ 1 is a stored row or a manifest path.

The root row (id 0) is deliberately **not** a skip trigger — every path's
ultimate prefix is the root, and the root being stored must not suppress
ply-1 candidates (this is the subtle case; unit-test it). Rationale: a
stored intermediate ancestor means the candidate is already decided
(proven ancestor), already frontier territory (open ancestor → its
children are C3), or a C2/C3 member (row parent) — all exploitation
territory the existing policies own. On a grown layer the kept set
shrinks toward empty; `descend` is a bootstrap policy that self-retires.
The session asserts candidate disjointness against all DB rows and
manifest paths (decision-3 convention; a collision is a defect abort).

**D3 — budgets, ladder, ledger.** Identical to `and-close`: per-candidate
budget = `max(2^(k−1)·base, 2·work_done)` (base = `--budget-evals`,
default 4M), bump-only censor hook on a censored candidate. plan4's
measurement (decisive finds at 1–4.7k nodes) says base 4M screens the
sharp class with wide margin. The ledger's path keying needs no schema
change; censor records for off-tree paths are first-class records exactly
like and-close's.

**D4 — job pipeline and census.** Decisive candidates run the existing
export path (TT snapshot → reconstruct → validate → shard + manifest
entry). `job:` records carry the standard twelve fields with
`"class":"D"`, `"tier":"descend"`, and `pass`/`work_before` emitted
(ledger-integrated, same semantics as and-close). Session-start census:
one `descend:` line with kept/covered counts and a per-ply breakdown —
exact wording pinned by the executor in the report, shape:

```
descend: candidates N (kept M, covered K) — ply 1: m1 kept, ply 2: m2 kept; base budget B
```

Stop conditions, summary line, exit codes: unchanged tool-wide semantics
(§7.2 of the manual).

**D5 — interaction.** No `--pns-config` or heavy-option effect under
`descend`; no and-close gradient/exclusion consultation. `descend` and
`and-close` compose by construction: a decisive ply-2 shard grafted at
merge time inserts open-ancestor rows (e.g. the ply-1 parent), which the
next `and-close` batch picks up as C3/frontier work. Batch 1 = descend
(bootstrap), batch 2+ = and-close (grow) — the manual's production cycle.

## 2. Non-mechanism alternative considered (rejected)

A standalone `examples/proofdb_seed` tool writing shards+manifest.
Rejected: a second entry point duplicating the export/validation plumbing,
no ladder or ledger integration, and a boot path that still isn't "run the
pipeline". The policy form reuses session.rs, budgets, ledger, and census
for free and keeps one CLI.

## 3. Tasks

1. **Implement** the policy (selection module under `examples/proofdb/`,
   keep files ≤ 10 KB; enumerate-replay via the existing replay/movegen
   machinery, not new FEN string handling), the `--descend-plies` flag,
   census lines, and unit tests — including the root-row skip-rule test
   (D2) and an enumeration-count test pinned against the measured 400
   ply-2 candidates (20 × 20, verified via `list_legal`/`replay`).
2. **Boot gate (headline):** fresh `data/proofdb` → bare
   `proofdb_harvest --policy descend` (defaults) → `proofdb_merge` →
   `proofdb_flip`. Pre-registered: **≥ 1 decisive shard** (expectation
   band: tens — solve plan4 found 55 ply-2 refutations at trivial cost;
   yield is a report finding, not a gate), merged DB > 1 node, flip
   analysis `flips 0 verified 0`. Record census and summary verbatim.
3. **Determinism gate:** repeat task 2's boot in a second fresh layer →
   byte-identical manifest digest and ledger bytes across the two boots
   (wall times exempt).
4. **Handoff smoke:** on a grown layer (task 2's), run one small
   `--policy and-close` batch (`--max-jobs`-bounded) and confirm it
   selects the descend-opened frontier (census shows C3/active rows
   beyond the root). Smoke only — no production batch in this plan.
5. **Integration test** in `tests/proofdb.rs`: boot-from-empty at a small
   budget (`--budget-evals 20000`, well under the plan4 finds' cost) on a
   temp layer → assert ≥ 1 decisive shard and merged DB > 1 node. Fast
   tier — no wall-clock, no `#[ignore]`.
6. **Docs:** manual `docs/proofdb_pipeline.md` — §3 restructure (step 0
   artifact-copying **removed**; batch 1 = `descend`; cold-start
   paragraph rewritten as resolved by the policy; the plan10-snapshot
   ledger seed demoted to an optional §4 optimization note, no longer
   part of the boot path), §7.2 policy row + `--descend-plies` +
   output-grammar additions, §8.1 R0 simplification (bootstrap merge
   stays; the "seeding" section retired with a pointer to `descend`),
   §11 history. `AGENTS.md` `proofdb_harvest` line gains `descend`.
   The fixture keeps its role: development/validation only (R1, tests).
7. **`report15.md`** (final task): pinned measurements (candidate counts,
   census lines, boot yield, determinism digests), problems, deviations,
   next steps; update the backlog item's status.

## 4. Acceptance gates

- Task 2/3/5 gates above all green; `make test` green including the new
  integration test.
- `cargo clippy --release --examples`, `cargo fmt --check`, `cargo doc`
  clean.
- **Zero `src/` changes** (the product solver is untouched; policy work is
  example-side by the pivot's own convention).
- R1 fixture rebuild digest unchanged: `0d929f4c…`, 55,703 nodes.
- `git status` clean of `data/` litter.

## 5. Risks and non-goals

- **Risk: yield differs from plan4-era.** The solver has changed since
  solve plan4 (ordering, refine caps, budgets) — the sharp class may
  differ. Mitigation: the gate is ≥ 1 decisive; yield is reported, and
  the ladder (D3) absorbs the rest. A zero-yield boot is a plan failure
  and would be investigated as such (enumeration or budget defect first).
- **Risk: skip-rule defects on a grown layer** (over-skipping =
  missed candidates — a coverage gap, harmless; under-skipping =
  duplicate censor records for already-frontier paths — wasted work, and
  the ledger bump makes repeats progressively more expensive but never
  unsound: the ledger never changes DB facts). Merge-time disjointness
  and replay validation catch anything worse as a defect abort.
- **Risk: enumeration cost at `--descend-plies 3`** (~10k paths × base
  budget). Documented as user-controlled cost; default 2 is minutes.
- **Non-goal:** changes to `and-close`/`breadth-pns` semantics, the
  merger, the ledger schema, or `docs/spec/global_proof_store.md` —
  descend is a producer-side policy; schema v1 and the spec are
  untouched.
- **Non-goal:** parallel harvesters (item 6), subtree harvesting
  (item 7), DTM upgrades (item 3).
- **Non-goal:** making `descend` productive on deep grown layers — it is
  a bootstrap policy by pre-registration (D2); off-tree exploration
  beyond ply ≤ 3 is future work if ever measured as valuable.
- **Non-goal:** DB growth beyond tooling validation — the 2026-10-02
  pivot stands; the boot gate's shards are validation evidence.
