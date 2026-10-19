# Initiative: `proofdb` — the startpos proof-line database

## Status

**Opened 2026-09-28.** A new initiative, deliberately *not* a reopening of
`solve` (see Relationship to other initiatives below). Founding framing:
build and grow a **startpos-rooted, machine-verifiable database of proven
and disproven lines** of atomic chess, structured as a partial AND/OR proof
tree, consumable by an external exploratory website.

**Pivoted 2026-10-02 (owner decision).** The founding framing above was a
mis-statement of the intent: the initiative's actual deliverable is the
**tooling**, not the database contents. Goal: deliver, harden, and document
the pipeline that can *build and grow* such a database (harvester, merger,
ledger tooling, flip analysis, website handoff). Harvesting batches are
**validation of the tooling** — evidence that the pipeline works at scale —
not deliverables in their own right; DB growth beyond what validation
requires is out of scope until the tooling is complete. The data-model
decisions below are unchanged: they are the contract surface the tooling
implements.

**Extended 2026-10-09 (owner decision).** Two goal extensions on top of the
tooling-first framing (both low-prio backlog items; neither reopens the
data-model decisions): (1) **ledger leaves are the harvest frontier** — a
`descend` run is a deliberate exploration commitment, and its censored
off-tree candidates (the ledger leaves) are the recognized starting points
for subsequent harvests, not a byproduct to re-enumerate every run (item
11); (2) **materialized closure promotion** — when a node's children's
proven facts entail its value under the parity rules, the merger may
promote it to a proven row instead of leaving the implication forever
derived (item 12). The constraint amendments ride with item 12; the
tooling-first deliverable and the handover rule are unchanged — promotion
is what eventually makes the handover rule executable.

## Motivation

**Extended 2026-10-15 (owner decision).** The durable-layer architecture
is pivoted: the shard set does not scale to the campaign sizes the owner
targets (10⁵–10⁷ shards; the per-decisive-job manifest rewrite is already
O(N²) over a campaign, and a million-file durable layer fails on
filesystem grounds regardless of tooling). The **DB becomes the single
source of truth** — one `proofdb.db` file growing to multiple GB — and
shards are reclassified as ephemeral transport, deletable after merge.
Correction established in the design dialogue: the work ledger is *not*
laddered-harvest legacy — its censor history (`passes_failed`/
`work_done`) records the facts of absence of censored draw jobs, which
are not representable in the tree model and not derivable from the tree
tables; it is therefore **relocated into the DB** (a `censor` table),
not dropped. Program: item 17 (censor into the DB, retire `work.json` +
`proofdb_ledger_union`; plan20 drafted) → item 18 (incremental merge
fold, deletable shards, `proofdb_verify` replacing the byte-identical
rebuild contract) → item 19 (spec v2 + manual rewrite + standing-layer
migration). Item 18 shipped 2026-10-16 (plan22): the DB is the single
source of truth, shards are sidecar transport deletable after their fold
under the manual's fold → verify → backup → delete recipe, and item 19's
standalone contract rewrite runs next (plan22 patched the spec/manual to
leave nothing actively wrong).

## Motivation (pre-pivot record)

The `solve` initiative's plan4 pilot demonstrated that a real, one-sided,
cheap tactical class of startpos-provable lines exists (55 ply-2 refutations;
a validated 10,177-position proof artifact at 83 KB) — but that the artifact
is a one-shot deliverable with no growth mechanism. The user's product idea:
harvest proven/disproven lines incrementally (sessions started and stopped at
will) into a *global proof* that a small external website lets users replay
and explore. The website itself is out of scope; this initiative owns the
orchestrator side: shards → merger → derived SQLite tree → spec'd schema.

The design was converged in the 2026-09-28 brainstorm session (this
initiative's founding conversation). Key decisions:

- **Structure: a partial AND/OR tree keyed by path from the startpos**, not a
  position-keyed fact store. At an OR-node (winning side to move) exactly one
  proving child is stored; at an AND-node (opponent to move) every refuted
  reply is stored with its own refutation subtree; nodes searched without a
  decisive result are `open` frontier. OR/AND roles flip with the proven
  outcome and are derivable — not stored. Path identity (not position-hash
  identity) naturally represents clock-dependent repetition verdicts.
- **Durable truth = validated shards (`proof_tree.bin` + manifest); the
  SQLite DB is a derived, rebuildable view**, produced by a single-writer
  merger. Nothing unverified enters the DB; a proven-outcome conflict between
  shards is a soundness bug and aborts the merge (hard tripwire, never a
  patch). The DB can be deleted and rebuilt from shards at any time.
- **Depth vocabulary: bound-by-default, exactness as an annotation.** Every
  proven node's subtree proves "forced outcome within ≤ d plies" (`depth_
  bound`); `depth_status = exact` is attached only where minimality is proven
  (1-move wins, preflight-certified ≤3-men subtrees, or future ladder
  passes). An optional background DTM-upgrade pass promotes `bound` →
  `exact` opportunistically; each upgrade is an independent unit of work
  (resume-friendly by construction, unlike the solve plan9 frozen-frontier
  resume, which was the measured failure mode there).
- **Draws are displayed as undecided** (`open`), except rule-terminal draws.
  A non-terminal draw claim cannot be finitely expanded in this tree model
  (repetition draws are path-dependent).

## Goal

**Post-pivot reading (2026-10-02):** the layers below describe what the
*tooling* must be able to build and consume — they are the deliverable's
contract, not a production target. Item 8 (tooling completeness audit +
hardening) owns closing the gap; harvesting runs justify themselves only as
pipeline validation.

A growing, verifiable, startpos-rooted partial proof tree:

- **Durable layer**: append-only shard directory. One shard = one validated
  `proof_tree.bin` subtree + manifest entry (root FEN, UCI path from the
  startpos, outcome, validator pass, provenance).
- **Working layer**: one SQLite file (`proofdb.db`), the merged tree:
  parent-linked nodes with `move_uci`, `outcome` (`win|loss|NULL=open`),
  `depth_bound`, `depth_status` (`bound|exact`), `provenance`, terminality
  derivable (`outcome NOT NULL AND depth_bound = 0`).
- **Contract layer**: `docs/spec/global_proof_store.md` — the SQLite schema
  and its semantics as the *sole* external contract (the website project
  reads only the DB; `proof_tree_dump.md` stays internal).

The seed is the existing solve plan4 artifact (95 validated shards, 12,592
tree nodes, 83 KB, all `validate: ok`). Its shards are rooted at their job
positions, not at the startpos, so the merger **grafts**: replay the manifest
path from the startpos, create open ancestor nodes, and attach the shard
tree at the graft node (fidelity check: shard root Zobrist hash == replayed
position hash).

## Design constraints (normative)

1. **Verifiable composition** (inherited verbatim from `solve`): no
   in-flight or unverified fact reaches the DB. A shard enters the merge
   only after replay validation passes; decisive outcomes are
   replay-verified facts; conflicts abort the session. Rejected, never
   patched.
2. **Path identity, no cross-path merging.** Node identity is the move
   sequence from the startpos. Two nodes with the same position at different
   halfmove clocks are distinct nodes (repetition/GHI discipline). Same-path
   facts from different shards are merged only under the agreement rules
   below.
3. **Derived-DB discipline — amended 2026-10-16 (item 18, plan22):** the
   SQLite file is the **single source of truth**, grown by the incremental
   fold — it is never rebuilt from shards. The retired deterministic
   rebuild gate (identical shard set → identical canonical dump) is
   replaced by (i) the deterministic fold (same standing DB + same shard
   set ⇒ identical content; a fresh fold is id- and value-identical to the
   old rebuild), (ii) `proofdb_verify` (whole-DB replay validation from
   the DB alone), and (iii) the documented `VACUUM INTO` offsite-backup
   duty (the deletion recipe's mandatory precondition). The same standing
   DB + shard set folds deterministically; two different fold sequences
   over the same facts may differ in row ids only (ids are stable
   surrogate keys; identity is the path).
4. **Merge rules (pre-registered here):**
   - *Agreement*: same path, same proven outcome → union children, dedupe.
   - *Refinement*: same path, both proven, different `depth_bound` → keep
     the tighter (smaller) bound; `exact` dominates `bound` at equal depth.
   - *Contradiction*: same path, different proven outcomes → hard abort
     (soundness bug). Also abort if an `exact` depth is contradicted by a
     strictly smaller proven bound.
5. **Scope discipline.** All code lives in `examples/` (campaign
   precedent); the product solver (`src/`, CLI, benchmark gates) is
   untouched. Merger uses the public lib API only
   (`ProofTree::from_bin`, `validate_proof_tree`, `notation`, `Position`).

## Handover rule (to `solve`)

This database is a precursor of the startpos proof: if the merged tree ever
closes the startpos root (all ~41 children resolved, OR-node proven), the
artifact hands over to `solve`, which reopens per its own status rule with
the proof already built (shards are its master proof state's content).
Until then, no DB milestone is claimed as progress toward the startpos
value.

## Relationship to other initiatives

- `solve` (dormant): predecessor and eventual beneficiary; its plan4
  artifact is the seed; its constraint-1 soundness text is inherited.
- `proof` (dormant): the shard format is its `proof_tree_dump.md` (v1,
  unchanged); no reopening.
- `parallel` / campaign tooling (`examples/campaign*`): the future harvest
  loop (backlog item 2) reuses the campaign worker shape (persistent
  workers, deterministic budgets, TT retention) but runs *fresh roots per
  shard*, which is exactly the regime plan9 did not falsify (it resumed one
  frozen quiet root).

## Backlog

| # | Item | Mechanism | Potential | Effort | Status |
|---|------|-----------|-----------|--------|--------|
| 1 | **Merger MVP + schema + spec + seed graft** | `examples/proofdb_merge`: manifest+shard reader, per-shard replay validation, grafting, overlay into SQLite, canonical dump; `docs/spec/global_proof_store.md`; seed = the 95 plan4 shards | the working layer exists; website can be built against the schema | M | **done (plan1, 2026-09-28)**: 95/95 shards merged, 12,081 nodes, gates G1–G5 pass |
| 2 | **Harvest loop** | campaign-worker-shaped runs producing shards (fresh root per job → solve → reconstruct → validate → manifest entry); merger runs after each batch | the DB grows at will, stoppable anytime | M | **done (plan2, 2026-09-29)**: standing shard dir + `proofdb_harvest`; first batch 12/87 decisive (all wins, plies 4–27), DB 12,081 → 12,655 nodes; gates H1–H5 pass; censored tail measured ≥ 40M evals at the 5 deepest quiet nodes |
| 3 | **DTM-upgrade pass** | background jobs re-searching bound ladders to promote `bound` → `exact` where affordable | depth quality improves without re-harvesting outcomes | M | open |
| 4 | **Coverage policy** | which frontier children to open next (cheap-first gradient; sharpness like plan4's, not enumerative layers) | keeps per-fact cost low as the tree deepens | S–M | **done (plan3, 2026-09-29)**: frontier classes C1/C2/C3 + policies in `proofdb_harvest` (`--policy`); measured A/B at 300M evals each: `sharp-siblings` 90 facts/300M (29.99 facts/100M, median 44.7k evals/fact) vs `open-deepest` 0/300M; winner is the default; gradient saturates after its head (marginal P2: 0 facts) |
| 5 | **Website handoff** | ship `global_proof_store.md` to the external website project; agree on read-only DB consumption | the product surface of this initiative | S | **done (plan13, 2026-10-02)**: handoff package at `measurements/plan13/handoff/` (digest-pinned rebuilt DB `0d929f4c…` + `HANDOFF.md` + verbatim spec copy); runbook extended into the operator **and user** manual (`docs/proofdb_pipeline.md`: data-model primer, shard provenance, per-tool output grammars, operator + consumer SQL recipes, troubleshooting); spec gained §7 (example consumer queries, additive). **Caveat: the agreement with the external project itself is the owner's follow-up action** — forward the package. |
| 6 | **Independent harvesters** | partition the frontier across worker processes (by root-child subtree or class; per-worker shard staging, serialized manifest merge); campaign-prototype precedent (`examples/campaign*`, solve plans 5/9) | wall-time scaling of harvesting without touching solver semantics | M | open — the sequential prerequisite is done (plan4, 2026-09-30: the ledger holds exactly the pick-up state a per-worker partition needs); note report4 finding 3: within-layer selection needs a work-aware key before large batches are worth scaling |
| 7 | **Subtree-scoped harvesting** | `--root-fen` (+ optional `--root-path`) plumbs the harvest root by FEN, resolved within the merged DB tree (no match or ambiguous match aborts); a subtree *view* over the same startpos-rooted tree — shards, manifest paths, and grafting stay startpos-relative unchanged | harvest any position, the primary user surface for item 6's per-worker subtree partition | S | open — parked from plan4 (2026-09-30, docs-only amendment, plan4 decision 12 tombstone): the first batch runs at the startpos root, and the MVP limitation ("FEN must be a node of the merged DB tree") is expected to be revisited under item 6 |
| 8 | **Tooling completeness audit + hardening** | audit the pipeline end-to-end as a third party would run it (shards → merger → DB, harvest loop from a clean checkout): missing CLIs/options, undocumented contracts, error-path gaps, packaging/docs; fix what it finds | the pivot's next plan; defines "tooling complete" so harvesting runs can be justified as validation and the website handoff (#5) has a stable surface to hand over | S–M | **done (plan12, 2026-10-02)**: audit re-run and pinned (durable layer 262/262 self-consistent; merger reproduces the standing DB byte-identically, `0d929f4c…`, 55,703 nodes; harvest smoke 2/2 censored, manifest byte-identical; flip 75/0/Null). One defect found and fixed (the stale-DB `built_from` truncation panicked on a short DB value — now length-safe, regression-tested); deliverables: operator runbook `docs/proofdb_pipeline.md`, derived-DB rebuild regression test (`tests/proofdb.rs::standing_layer_rebuilds_byte_identical`, in the default gate), AGENTS.md entries for all four tools; ledger-from-clean-checkout recorded as a decision (fresh or seed — the ledger stays scheduling state), startpos-rooting/orphan-shard/merge-after-batch recorded as limitations. Plan11 (parked 18B `g1f3` rung) retired. Next levers: items 5 and 6. |
| 9 | **Centralized generated-data layout** | default root `data/proofdb/` for **all** generated state, production shards and manifest included (`proofdb.db`, `work.json`, `flip.json`, `shards/manifest.json`); tool defaults updated (`proofdb_merge --manifest/--shard-dir/--db`, `proofdb_harvest --db/--ledger`, `proofdb_flip --db/--out`) so a production run needs no path flags; the committed `docs/plans/proofdb/shards/` set reclassified as development/validation fixture (explicit-flags runs, tests) — production ignores it; manual/AGENTS/.gitignore-comment updates | one canonical home for generated data; cwd never littered; production = bare invocations | S | **done (plan14, 2026-10-02)**: defaults compiled in `examples/proofdb/mod.rs` (single source of truth, pinned by `tests/proofdb.rs::cli_defaults_are_centralized_under_data_proofdb`); parent-dir creation on write paths only; quickstart re-shaped to production (bare invocations, re-pinned verbatim); R1 fixture digest unchanged (`0d929f4c…`, 55,703 nodes); migration one-liner in manual §4/R5; backup of the unversioned `data/proofdb/shards/` is the operator's responsibility (manual §2.1). `report14.md` |
| 10 | **Self-bootstrapping harvest (`descend` policy)** | off-tree exploration policy in `proofdb_harvest`: enumerate all legal startpos-rooted paths of length ≤ `--descend-plies` (default 2; 400 measured ply-2 candidates), skip row/manifest-covered ones (root-row-aware rule), bounded-solve under the and-close ladder, export decisive as shards — the solve plan4 sharp-class method productized; batch 1 = descend (bootstrap), batch 2+ = and-close (grow) | a fresh clone bootstraps by running the pipeline; no artifact-copying production step; the fixture reverts to validation-only | S–M | **executed (plan15, 2026-10-08)** — shipped; boot gate 52 shards, determinism gate byte-identical; see `report15.md` |
| 11 | **Ledger-leaf harvest policy (the descend tail as the frontier)** | a scheduling mode whose job set is exactly the ledger's censored off-tree candidates (the deliberate descend leaves), ordered by the shared ladder; fresh shallow enumeration demoted to bootstrap-only. Decisive leaves export shards exactly as `descend` does today (graft + open-ancestor spine — already shipped with item 10), so this item is scheduling only, no new capability | a descend run is a deliberate exploration commitment: its censored leaves are the measured frontier, and subsequent harvests should resume from them instead of re-enumerating the whole candidate set every run (today the ladder is the only pacing mechanism); the leaf set is also a natural work partition for item 6 | S | open — added 2026-10-09 (owner pivot), low prio |
| 12 | **Materialized closure promotion (implied AND/OR nodes become proven rows)** | merger-side promotion fixpoint: an undecided row whose children's proven facts entail its value under the side-to-move parity rules (either case: one proving child, or all replies decided) is rewritten as proven. Every promoted row must either carry a synthesized composite proof subtree that passes the existing replay validator, or carry an explicit derived provenance class — the implementing plan decides. Deterministic rebuild from the shard set preserved (the fixpoint is deterministic); `depth_bound`/`depth_status` derived under the same discipline; a promotion contradicting a shard outcome aborts (extends merge rule 4); `proofdb_flip` updated so promoted rows count as verified facts; spec `docs/spec/global_proof_store.md` amended (contract-surface change, standalone doc). Amends constraints 1 and 3: "no unverified fact" is preserved by validating/classing every promotion; "DB = cache of shards" becomes "cache of shards + deterministic promotion" | closes the derived-vs-materialized gap: proven leaf spines propagate closure upward automatically instead of only informing job exclusion and flip analysis; makes the handover rule ("all startpos children resolved, OR-node proven") executable by the merger itself; compounds item 11's leaves into closure | M | open — added 2026-10-09 (owner pivot), low prio |
| 13 | **Frontier-walk harvest policy (self-advancing shallowest-first ledger walk)** | a live selection policy in `proofdb_harvest` (`--policy frontier-walk`, no new flags): per iteration pick the shallowest ledger-frontier candidate (ply, path lex tie-break; empty ledger → the root, exhausted if the root is proven-stored); if its `work_done < B` search it at the flat budget `B` (`--budget-evals`, no ladder), else expand it — search its first unhandled child (not a stored row/shard/record ≥ B) at `B`; censors bump the shared ledger (bump-only, no exposure); a node with no unhandled child is marked complete in-session (livelock design-out); no intra-session AND/OR propagation, no structural-number exclusions (recorded limitations) | a grow policy that bootstraps from the ledger alone and composes with `and-close`/`breadth-pns` through the shared ledger; deliberately number-free and flat-budgeted (the owner's spec) — the re-search waste and the decision-10 gap are accepted, measured limitations | S–M | **done (plan16, 2026-10-13)**: shipped; boot gate 53 shards / 6,706 nodes / flips 0 at a 2G eval cap (first decisive ~620M evals in), determinism gate byte-identical (manifest digest, ledger bytes, all shards — even the merged DBs), grown-layer smoke shows real frontier selection (8,446 records, 6 lineage drops); re-search waste measured 0 in the boot; zero-eval no-progress guard added (repetition censors); see `report16.md` |
| 14 | **Crash-consistent manifest (per-job durable manifest sync)** | the harvest session rewrites the manifest atomically (tmp + rename, the ledger's pattern) after every decisive job, inside the single `Session::produce_shard` choke point — no signal handling, no format change, all five policy drivers covered at once; end-of-batch rewrite and stdout grammar unchanged (idempotent byte-wise) | an unexpected crash or ctrl-c loses at most the in-flight job's work instead of every shard exported in the session; every subsequent bare `proofdb_merge` folds in everything collected up to the last finished job; the operator "merge after a batch" discipline is unchanged (the stale-`built_from` guard still applies) | S | **done (plan17, 2026-10-13)**: per-job atomic manifest sync in `Session::produce_shard` (`sync_manifest`, tmp + rename writer); abort/error window test-pinned (previous manifest intact, one orphan); censored jobs byte-pin the manifest as untouched; boot tests assert the pre-rewrite current manifest; gate green, `0d929f4c…` digest pin unchanged; see `report17.md` |
| 15 | **Policy consolidation — `frontier-walk` becomes the only harvest policy** | remove `breadth-pns`, `and-close`, `descend`, and the plan3 legacy gradients (plans 2–15 surface) from `proofdb_harvest`; the walk absorbs the boot (its root rule bootstraps a root-only layer, §3.2 of the manual) and the flat budget replaces the ladder as the deepening mechanism (raising `--budget-evals` between sessions re-prices underworked records); the `--policy` flag is kept but accepts only `frontier-walk` | every new harvest feature must work and be tested against all policies — an unreasonable maintenance/extensibility cost for surface the owner does not use (owner request 2026-10-14); accepted trade-offs: `descend`'s fast ply-2 boot enumeration and the `and-close` auto-deepening ladder are gone | S | **done (plan18, 2026-10-14)**: ~5.4k lines removed (`pns/`, `and_close/`, `descend.rs`, `batch.rs`, `frontier.rs`, `policy.rs` modules; the legacy CLI flags); walk-only CLI, operator manual rewritten to the walk-only boot-and-grow cycle, job-JSON grammar drops `parent_bound`/`number`; ledger pick-up and durable-layer formats unchanged; gate green; see `report18.md` |
| 16 | **Scoped harvest (`--moves` — the walk re-anchored at a startpos path)** | `proofdb_harvest --moves 'a2a4 b7b5 b1a3'`: the frontier-walk session runs with its world rooted at the node reached by replaying the given moves from the startpos — only that node and its descendants are searched/expanded. Re-anchor, not filter (the scope node is the walk's root: virtual-root bootstrap, drop rules evaluated scope-relative, census scoped + out-of-scope count); a proven-stored scope node ends the session `exhausted` with an explanatory note; tokens whitespace-separated and prefix-validated by startpos replay (nonsense/illegal → usage exit 1, fail-fast before any state load); absent flag = exact today behavior (bit-identity) | targeted growth without disturbing the global frontier: an operator can commit work to one line's subtree (deepen/resolve a chosen opening) while the shared ledger and DB stay untouched elsewhere — scoping is a selection-layer change only since every job already replays the full startpos path | S | **done (plan19, 2026-10-14)**: shipped; the frontier keys its order/complete/decided sets scope-relative (`rel_of`/`join_scope`; the scope root is relative `""`), proven rows above the scope invisible, `pick` returns absolute keys (job JSON unchanged); census line gains `out_of_scope O …; scope <path\|global>`; proven-scope stderr note; parser validated by startpos replay with the offending token named; CLI-level test target `tests/proofdb_harvest_cli.rs` (exit-1 unhappy paths touch no state, in-scope-only jobs, proven-scope exhausted, absent-flag global shape); global call sites re-pinned bit-identical; see `report19.md` |
| 17 | **Censor state into the DB — one-artifact frontier (retire `work.json` and `proofdb_ledger_union`)** | a `censor` table in the DB (schema v2, additive: `path`/`work_done`/`passes_failed`, the exact ledger relocation — the ledger is censor-history only, verified by code read); the in-memory `Ledger` type and all walk selection code stay untouched, only the persistence backend swaps (JSON file → SQLite, per-bump committed transaction = today's per-censor atomic save); harvest gains a narrow writer carve-out (censor rows only — merge stays sole writer of `nodes`/`shards`/`meta`); merge preserves `censor` across its full rebuild; one-shot `proofdb_merge --import-censor` migrates a legacy ledger under the union max-rule | the frontier state lives in the one file the owner wants as truth; the first step of the 2026-10-15 durable-layer pivot (items 17→18→19); kills two artifacts (`work.json`, `proofdb_ledger_union`) and un-blocks item 6 (parallel censor merging becomes SQL) | S–M | **done (plan20, 2026-10-15)**: shipped as drafted (backend swap, D3 carve-out, D5 preservation, D6 import with the plan's per-field max rule); standing layer migrated — 3,615 censor records imported, `work.json` deleted, schema v2, flip clean; fixture rebuild digest re-pinned `406752cc…` (was `0d929f4c…`); see `report20.md` |
| 18 | **DB as the source of truth: incremental merge fold, shards deletable post-merge, `proofdb_verify` audit** | `proofdb_merge` becomes an incremental fold: validate a shard, graft into the *standing* DB in one transaction keyed on the `shards`-table sha256 (idempotent, conflicts abort without writes); shards are reclassified from durable truth to ephemeral transport — deletable after merge; the byte-identical full-rebuild contract (constraint 3) is retired and replaced by `proofdb_verify` (replay-validate the whole DB tree read back from the DB itself — validation is path replay, it needs no shards) plus a documented `VACUUM INTO` offsite-backup duty; the `built_from` manifest-digest staleness gate is replaced by unmerged-shard detection (a `*.bin` present in the shard dir but absent from the DB `shards` table → harvest/flip refuse) | the scaling fix: removes the O(N²) per-decisive-job manifest rewrite and the millions-of-files durable layer the owner called out; crash consistency *simplifies* (one SQLite transaction per shard/batch replaces the atomic manifest dance); accepted losses, each deliberately replaced: byte-identical rebuild → verify + backup; post-deletion excision of a wrong proven fact → impossible, append-only truth (soundness bugs were never patched over anyway) | M | executed and closed 2026-10-16 (plan22); decisions settled in the plan: manifest retired to a legacy metadata/migration source (per-shard sidecars are the transport — shard-format v2 rejected to keep `proof_tree_dump.md` v1 closed), no DB→shardset export (portability = the DB file; audit = `proofdb_verify` from the DB alone) |
| 19 | **Contract migration: spec v2, manual rewrite, standing-layer consolidation** | `docs/spec/global_proof_store.md` rewritten as the v2 contract (DB = truth; `censor` table semantics; per-shard provenance as the audit trail; deletion/backup policy) — standalone, no plans vocabulary; `docs/proofdb_pipeline.md` rewritten around the new artifact graph (R-recipes: backup replaces rebuild; no more re-merge-from-shards); AGENTS.md; migration of the standing layer (fold existing shards once, adopt the new contract) | the external agreement (`solve` handover, website) tracks the reality; closes the 2026-10-15 pivot | S–M | open — added 2026-10-15 (owner direction); runs after item 18 |
| 20 | … | S | **done (plan21, 2026-10-16)**: shipped as drafted (D1–D9; D9's split grew to two peels — census/`RootState` into `walk/census.rs`, key algebra + drop/handled classification into `walk/keys.rs`); probes: standing layer 6,306 or-closed parents (12,612/43,092 nodes at-or-under one), current censor records drop 0 (frontier outside the or-closed subtrees — the drop bites as sessions create records there); committed fixture 23,885 or-closed parents over 50,258/55,703 nodes; see `report21.md` |

## Non-goals

- The website itself (external project; consumes the spec'd SQLite schema).
- Product-solver features or CLI changes; no claim of progress toward the
  startpos value (handover rule above).
- Any parallel-search mechanism for a *single* position (the measured no-gos
  stand); the harvest loop is many independent roots, not a parallel solve.
- Exact-DTM everywhere; exactness is an annotation earned opportunistically.

## Measurement conventions

- Reference hardware: this sandbox. Seeds and merged DBs are small (KB–MB);
  the validated seed DB is committable under `measurements/plan<N>/` as a
  reusable validated artifact. Raw run transcripts are not committed.
- Every DB build records a census (shards, nodes, ancestors added,
  conflicts, validator pass) next to it; a census without a validator pass
  is not a result.

## History

- **2026-10-16 — plan23 drafted (docs-only plan session): item 19 scoped
  against the real post-plan22 state.** Three probes reshaped the item.
  (i) The standing layer is still pre-fold (`built_from 70a160a6…`, the
  manifest-digest stamp; the old transport sits parked at
  `data/proofdb/shards-bak/` — 405 bins + the legacy manifest, no
  sidecars), and the migration recipe was validated end-to-end on a copy:
  an in-place no-op fold (`folded 0 (405 already merged)`, sha
  cross-checks pass) re-stamps `built_from` to the fold digest
  `47b25861d0f56776…` with `nodes`/`censor`/`shards` byte-equal,
  `verify: ok — nodes 43092 (proven 43028, open 64)`, `flips 0`. (ii)
  New probe finding: a *missing* shard dir aborts merge/harvest/flip
  (only an existing empty dir bootstraps) — the migration recipe and R6
  must recreate the dir after deleting the transport. (iii) The handoff
  package (`measurements/plan13/handoff/`) was re-cut today, post-plan22
  but pre-migration: it ships the pre-fold DB with a v1-flavored spec
  copy (`built_from` = manifest digest, "exactly one proving child") and
  a HANDOFF.md calling `nodes` "deterministically rebuildable" — the
  v2-reverted claim item 19 exists to retire; plan23 supersedes it with a
  post-migration spec-v2 re-cut. Honest scoping recorded in the plan:
  plan22's D10 patch already landed the v2 semantics, so the spec/manual
  work is a coherence pass (numbering, audit-trail sentence, transitional
  seams), not a from-scratch rewrite; no code changes planned. Plan:
  `plan23.md`.
- **2026-10-16 — item 18 executed and closed (plan22).** The DB became
  the single source of truth: `proofdb_merge` is the incremental fold
  (one transaction per invocation; replay-validated standing-tree load;
  row ids preserved and appended — a fresh fold is id- and value-identical
  to the retired rebuild, pinned by the R1 migration-equivalence test);
  shards are self-describing `<tag>.bin` + `<tag>.meta.json` sidecar
  pairs (sidecar = commit marker; bin-without-sidecar = ignorable orphan)
  and are deletable after their fold (the manual's fold → verify → backup
  → delete recipe; no `--prune` flag); `proofdb_verify` replaces the
  byte-identical rebuild (whole-DB replay validation from the DB alone,
  `--backup` = the `VACUUM INTO` duty); `built_from` = the fold digest
  over the `shards` table; the manifest-digest staleness gate became
  unmerged-shard detection (harvest + flip refuse on a sidecar without a
  `shards` row). The manifest is retired to a legacy migration/metadata
  source (merge backfills sidecars; harvest's `--manifest` refuses); the
  harvest session seeds its collision sets from the DB's shard rows. No
  `src/` changes. Gate green (`make test`, fmt/clippy/doc); new test
  target `tests/proofdb_fold.rs` (R1–R8), CLI fixtures moved to the
  sidecar world, the plan17 manifest-sync tests retired with the
  manifest. Fixture fold digest re-pinned `14fd0753…` (55,703 nodes;
  was `406752cc…`). Item 19 (spec v2 + manual rewrite + standing-layer
  migration) runs next; plan22 already patched the spec/manual to leave
  nothing actively wrong. Plan: `plan22.md`; report: `report22.md`.
- **2026-10-16 — item 20 executed and closed (plan21).** The OR-node
  sibling skip shipped exactly as drafted: `Drop::OrClosed` in the D2 drop
  loop (shallowest decisive prefix wins the attribution), the static
  or-closed set built from in-scope proven `Loss` child rows at even
  absolute ply, the D4 dynamic or-close after a decisive `loss` job
  (parity + scope checked on the frontier), the D5 decided-subtree sweep,
  the D6 or-closed scope-root exhaustion with the plan19-style scoped
  note, census field `or_closed`, summary field `or_closes` (or-close
  complete-marks do not count toward `completes`), always-on, no new
  flags, no `src/` changes. D9's file-size split needed a second peel
  beyond the plan's census one (`walk/keys.rs`) — walk.rs ~20 KB. The
  quickstart §3.1 pins re-pinned (census string + summary line; the
  child-eval overshoot re-measured 200015 on this container vs the doc's
  reference-host 200021 — a pre-existing host-dependence finding, see
  report21); the §7.2 example census line re-pinned to a fresh probe of
  the regenerable dev shard set. **Post-closure finding:** the file at
  `data/proofdb/proofdb.db` during both plan21 sessions was a 4,314-node
  schema-v1 dev rebuild, not the plan20-migrated standing layer (which
  the owner restored afterwards) — the real standing layer was re-probed
  in report21 §2/§6. Plan: `plan21.md`; report: `report21.md`.
- **2026-10-16 — item 20 added; plan21 drafted (docs-only plan session, owner
  request).** OR-node sibling skip in `frontier-walk`: at a white-to-move
  node, one child proven `Loss` prunes all sibling exploration — statically
  (new `Drop::OrClosed` candidacy drop + `or_closed` census field) and
  dynamically (in-session or-close of the parent, decided-subtree sweep,
  or-closed scope root ⇒ `exhausted`). Orientation correction settled in
  dialogue: a winning white move is a proven-**loss** child (side-to-move
  perspective), not a proven-win child (that is a refutation). White-only
  by absolute ply parity; loss nodes and black-to-move nodes unchanged;
  censor rows preserved (candidacy-only drops). Read-only probes this
  session: the standing layers are already full of the structure (2,154
  or-closed parents in the production DB's 4,314 nodes; 23,977 in the
  55,703-node fixture; the CLI-test crafted fixture has none — count pins
  survive, census strings re-pin), so the quickstart session pins are
  pre-registered to drift and will be re-pinned at execution. `walk.rs`
  crosses the ~20 KB line → the deferred census/`RootState` split ships
  with it. Plan: `plan21.md`.
- **2026-10-15 — item 17 executed and closed (plan20).** Censor state into
  the DB shipped exactly as drafted: schema v2's additive `censor` table
  (spec amended; invariant 8: scheduling state, no proof facts), the
  in-memory `Ledger` and all walk selection code untouched — only the
  persistence backend swapped (`Ledger::load_db`; commit-then-bump per
  censor, one autocommit transaction each, so the D4 abort window shrank
  to the in-flight censor). Harvest's DB writes are censor rows only
  (D3 carve-out; RW open without CREATE — a missing DB stays a clean
  abort); merge preserves the table across its full rebuild (D5) and
  gained `--import-censor` under the plan's per-field max rule (D6;
  supersedes the retired union tool's tuple rule — strictly more
  conservative on `work_done`). `proofdb_ledger_union` and `work.json`
  are retired; the `--ledger` flag aborts with a migration hint. Standing
  layer migrated: 3,615 censor records imported (idempotence verified on
  the live layer), schema v2, 43,092 nodes, flip clean, sidecar deleted —
  the working layer is now one DB file plus the durable shard set.
  Fixture rebuild digest re-pinned `406752cc…` (was `0d929f4c…`). Gate
  green (`make test`, fmt/clippy/doc); new tests: censor roundtrip/
  durability/commit-failure/overflow/clamp units, rebuild preservation,
  second-session pick-up, and CLI-level preservation + import via two new
  harness runs. Finding: the quickstart §3.1 child-eval pins had drifted
  pre-plan20 (200015 → 200021 chunk overshoot) — re-pinned. Slip:
  the repo-root operator `STOP` file was deleted during a smoke run and
  restored (`touch STOP`). Plan: `plan20.md`; report: `report20.md`.
- **2026-10-15 — durable-layer pivot; items 17–19 added; plan20 drafted
  (docs-only plan session, owner dialogue).** Owner direction: the shard
  set will not scale (10⁵–10⁷ shards), the DB should be the single source
  of truth, `work.json` looks obsolete under the walk-only policy, and
  merged shards should be deletable. Design dialogue settled: (i) the
  ledger's censor history is facts-of-absence (censored draw jobs are not
  representable in the tree model) — it moves *into* the DB as a `censor`
  table rather than being dropped; (ii) shard deletion requires the
  incremental fold + `proofdb_verify` + backup duty first, so deletion
  stays unsafe until item 18; (iii) the byte-identical rebuild contract
  is retired deliberately, replaced rather than silently lost. Backlog:
  item 17 (censor into DB; plan20 drafted, S–M), item 18 (incremental
  fold + deletable shards + verify, M), item 19 (spec v2 + manual +
  standing-layer migration, S–M). Plan: `plan20.md`.
- **2026-10-14 — item 16 executed and closed (plan19).** Scoped harvest
  shipped: `proofdb_harvest --moves '<uci path>'` re-anchors the
  frontier-walk at a subtree root — a selection-layer change only (no
  `src/` changes; searches bit-identical; the shared ledger, DB, shards,
  and manifest untouched outside the scope). The frontier stores
  scope-relative keys (the scope root = relative `""`, ply 0), drops are
  evaluated against proven rows within the scope (rows above invisible by
  ruling), out-of-scope records are census-counted and preserved;
  a proven-stored scope root exhausts with the stderr note. `--moves` is
  whitespace-split and startpos-replay-validated before any state load
  (usage exit 1 naming the token). Census grammar: `… out_of_scope O,
  complete C) — root <state>; base budget B; scope <path\|global>`. New
  test target `tests/proofdb_harvest_cli.rs` (real-binary harness via
  `cargo run --example`; `CARGO_BIN_EXE_*` does not cover example
  targets). Stray-`STOP` finding: the repo root's operator `STOP` file
  zeroed CLI-test sessions until the tests passed an explicit stop file.
  Plan: `plan19.md`; report: `report19.md`.
- **2026-10-14 — item 16 added; plan19 drafted (docs-only plan session).**
  Scoped harvest: `--moves <startpos path>` re-anchors the frontier-walk at
  a subtree root. Design dialogue settled: re-anchor (not filter — the
  walk's bootstrap/drop/census anchoring at the game start would otherwise
  spend budget above the scope or silently exclude it), proven-stored
  scope root ⇒ `exhausted` + note (no work beneath proven nodes,
  decision-9 philosophy unchanged), whitespace-separated tokens, census
  gains `out_of_scope` + `scope` fields. Selection-layer only: no `src/`
  changes, searches bit-identical; absent flag = today's behavior.
- **2026-10-14 — item 15 added and closed (plan18; owner request).** Policy
  consolidation: the owner uses only `frontier-walk`, and every new harvest
  feature had to work and be tested against all policies — the removal cost
  of the other five is paid once instead of taxing every future feature.
  Removed: `breadth-pns` (plans 4–6), `and-close` (plans 8–10), `descend`
  (plan15), the plan3 legacy gradients, and their shared machinery
  (`pns/`, `and_close/`, `descend.rs`, `batch.rs`, `frontier.rs`,
  `policy.rs`; ~5.4k lines incl. tests). Relocated into surviving modules:
  the bump-only censor hook (`ledger::on_censored`), the path-key helper
  (`harvest::child_key`), and the standing base budget (`walk::BASE_EVALS`).
  The walk absorbs the boot (root rule, proven by the plan16 boot gate) and
  the flat budget replaces the ladder as the deepening mechanism. Manual
  (`docs/proofdb_pipeline.md`) rewritten to the walk-only cycle; AGENTS.md
  bullet updated. Plan: `plan18.md`; report: `report18.md`.
- **2026-10-13 — plan17 drafted (docs-only plan session): item 14 added
  (owner request).** Crash-consistent manifest: after every finished job
  the durable layer (shards + manifest on disk) must be consistent, so an
  unexpected crash or ctrl-c loses at most the in-flight job's work and
  the next bare `proofdb_merge` still folds in everything collected —
  with **no SIGINT handler** (the stop conditions stay the only graceful
  exits; the plan only narrows what an unclean exit costs). Machinery
  verified by code read: the manifest is batch-end-only today (one
  `rewrite_manifest` call at `proofdb_harvest.rs:300`, entries in memory
  in `Session`), all five policy drivers funnel through
  `Session::produce_shard`, and `write_manifest` is deterministic but not
  atomic — the ledger's tmp+rename save is the pattern to reuse.
  Plan: `plan17.md`.
- **2026-10-13 — item 14 closed (plan17).** Per-job atomic manifest sync
  shipped exactly as drafted: `write_manifest` writes tmp + rename (D3);
  `Session::new` takes the manifest path (D2) and `produce_shard` syncs
  silently after the entry push (D1/D4), aborting on write failure; no
  `src/` changes, no format change, stdout grammar unchanged. Tests: tmp
  residue + I/O-failure-intact-previous-manifest (writer), censored jobs
  leave manifest bytes untouched, the D4 interrupted-write window
  (previous manifest + one orphan), boot tests assert the manifest is
  already current before the batch-end rewrite. Gate green; fixture
  rebuild pin `0d929f4c…` unchanged (manifest bytes format-frozen).
  Report: `report17.md`.
- **2026-10-13 — plan16 executed; item 13 closed.** The `frontier-walk`
  policy shipped in `proofdb_harvest` (`examples/proofdb/walk.rs` +
  `walk/driver.rs`; zero `src/` changes): D2 selection over the ledger
  frontier with the lineage/ancestor drops and the virtual-root rule, D3's
  two visit kinds (flat-budget search / first-unhandled-child expansion)
  and the pick-only complete rule, D4's bump-only ledger semantics, D6's
  caps-are-the-binder operating model. Gates: boot (root-only layer → 53
  decisive shards, merged 6,706 nodes, flips 0), determinism (two boots
  byte-identical incl. the merged DBs), grown-layer smoke (8,446-record
  ledger, 6 lineage drops, real frontier selection), integration test at
  20k/10M-cap in the fast gate. Findings: the plan's uncapped-boot reading
  is non-terminating by design (caps are the binder — D6); a zero-eval
  censor guard was added beyond the plan (repetition censors cannot
  advance a flat budget); the accepted re-search waste measured 0 in the
  boot (search visits re-did no known work). See `report16.md`.
- **2026-10-13 — plan16 drafted (docs-only plan session): item 13 added
  (owner request).** New harvest policy `frontier-walk` designed in dialogue
  from the owner's four-line spec (shallowest ledger-frontier pick →
  search-at-`B` if underworked → else expand into the first unhandled child →
  repeat); five edge-case rulings accepted by the owner (lex tie-break +
  first-unhandled-child expansion, in-session complete rule against
  livelock, root rule incl. exhausted-on-proven-root, flat budget with no
  ladder, policy name). Deliberately number-free (no decision-10
  exclusions) and bump-only; composes with the ladder policies through the
  shared ledger; boot stays `descend`. See `plan16.md`.
- **2026-10-09 — goals extended (owner pivot, docs-only session): items 11
  and 12 added.** The owner reframed descend censoring: a descend run is a
  deliberate commitment, so its censored ledger leaves are the harvest
  frontier (item 11 — scheduling only; decisive leaves already graft via
  item 10), and AND/OR closure should be materialized in the DB rather
  than remaining derived-only (item 12 — amends constraints 1/3, spec
  change included). Both open, low prio; the next execution item remains
  item 6 (independent harvesters).
- **2026-10-08 — plan15 drafted (docs-only plan session): item 10 added.**
  Owner rejected artifact-copying as the production bootstrap (manual §3.2
  step 0); the seed provenance was traced to solve plan4's sharp-class
  enumeration (method, cost, and candidate count re-verified with read-only
  probes: 400 ply-2 candidates, cold-start censoring reproduced), and the
  method was pre-registered as the `descend` harvest policy (plan15).
  Executing plan15 closes item 10 and rewrites the manual's production
  cycle to descend-then-and-close.
- **2026-10-02 (latest) — plan14 executed; item 9 closed.** All four
  tools' generated-state flags now default wholly under `data/proofdb/`
  (`proofdb.db`, `work.json`, `flip.json`, `shards/manifest.json` +
  shard dir); production runs are bare invocations, development runs pin
  explicit flags against the committed fixture (untouched, regression
  test unchanged). Gates green: `make test`, clippy/fmt/doc, quickstart
  re-pinned verbatim, R1 digest `0d929f4c…` / 55,703 nodes unchanged,
  default-path smokes clean. See `report14.md`.
- **2026-10-02 — plan14 drafted (docs-only plan session): item 9 added;
  amended same session.** Owner request: all generated proofdb data
  centralized in `data/proofdb/` by default. Drafted scope was
  working-layer only (`proofdb_merge --db`, `proofdb_harvest
  --db/--ledger`, `proofdb_flip --db/--out` defaults);
  **owner clarification amended the plan**: the committed shard set at
  `docs/plans/proofdb/shards/` is a development/validation fixture, not
  a production artifact, and production has not started — so production
  runs will keep **all** generated data (shards and manifest included)
  under `data/proofdb/` and ignore the fixture entirely. Final decision
  set: defaults `--manifest data/proofdb/shards/manifest.json`,
  `--shard-dir data/proofdb/shards/`, plus the drafted DB/ledger/flip
  defaults — a production run is bare `proofdb_merge` / `proofdb_harvest`
  with no path flags; development/validation runs keep explicit flags
  against the fixture (regression test untouched). Accepted consequence
  (decision 9): the production durable layer lives outside git — backup
  of `data/proofdb/shards/` is the operator's responsibility. Spec
  verified path-free (no change needed); quickstart becomes the
  production-shaped walkthrough inside the checkout; a migration
  one-liner covers existing operator working dirs. See `plan14.md`.
- **2026-10-02 (latest) — plan13 executed; item 5 closed.** The handoff is
  now shippable as one directory: `measurements/plan13/handoff/` holds a
  DB rebuilt from the committed shard set (digest `0d929f4c…` = the plan12
  pin, 55,703 nodes), a consumer-facing `HANDOFF.md` (digest + census +
  read-only consumption statement), and a verbatim spec copy. The runbook
  `docs/proofdb_pipeline.md` became the operator **and user** manual (owner
  decision: one canonical doc, no separate manual file): new data-model
  primer, shard-provenance section, per-tool output grammars with worked
  invocations (all re-run this session; the decisive `job:` example is the
  pinned plan10 census record), operator + consumer SQL recipes, extended
  troubleshooting. Spec §7 (example consumer queries) added, verified
  additive-only and standalone. One runbook correction found: §2's smoke
  expectation said `job:` lines carry `"kind": null` — the observed lines
  omit `number`/`kind` entirely (and-close emits `pass`/`work_before`);
  fixed to the observed bytes. The read-only DB probe corrected the plan's
  root-child sanity item (all 7 root children are open — the root is
  undecided; 55,628 proven nodes live deeper). Gates: `make test` green,
  no code changes. Next lever: **item 6 (parallel harvesters)**; the
  owner forwards the handoff package (external agreement). See
  `report13.md` and `measurements/plan13/`.
- **2026-10-02 — plan12 executed; item 8 closed.** The audit
  probes were re-run against the working tree and their results pinned:
  clean-checkout merge digest `0d929f4c…` / 55,703 nodes (1.2 s), harvest
  smoke 2/2 censored with a byte-identical manifest rewrite (`e91ad57b…`),
  flip census 75 open / 0 flips / root Null, ledger-union N=1 over the
  committed plan10 snapshot → 8,446 records. G1 fixed: the stale-DB guard
  in `examples/proofdb/db.rs` now renders a short/malformed DB-stored
  `built_from` length-safely (previously a Rust panic, exit 134) and the
  malformed-DB probe aborts cleanly (exit 1, message names `built_from`);
  regression test added (short, empty, non-ASCII values). G2: operator
  runbook `docs/proofdb_pipeline.md` written and its quickstart executed
  verbatim (all pinned expectations hold). G3: AGENTS.md now lists all
  four proofdb tools and points at the runbook. G4:
  `standing_layer_rebuilds_byte_identical` merges the committed manifest
  into a temp DB and asserts the byte-identical digest + node counts in
  the default gate (1.2 s; verified red on digest drift). G5/G6 recorded
  as decision/limitations in the runbook. Gates: `make test` green,
  clippy/fmt/doc clean, no `src/` changes. No new M-sized gap — item 8
  closes with plan12; next levers are items 5 (website handoff) and 6
  (parallel harvesters). One S-sized finding for the backlog:
  `tests/proofdb.rs` (~35 KB) is past the file-size split threshold and
  grows with every proofdb plan. See `report12.md`.
- **2026-10-02 — plan12 drafted (plan session): item 8's audit run at plan
  time, all read-only.** The clean-checkout audit is measured: the
  durable layer is complete and self-consistent (262 shards, unique
  tags/paths, no orphans, seeds included), the merger reproduces the
  standing DB byte-identically from the committed manifest (1.2 s,
  `0d929f4c…`), the harvest loop starts cleanly on a fresh/missing ledger,
  and the flip/ledger-union tools work over the rebuilt DB. Gaps: one
  error-path defect (the stale-DB check panics on a short DB-stored
  `built_from` — `db.rs:50` truncation, exit 134 instead of a clean abort),
  no operator runbook (the only run instructions live in plan/report
  prose), AGENTS.md omits all four proofdb tools, and the derived-DB
  rebuild gate exists only as narrative (no repeatable test). plan12 fixes
  all four, documents the ledger-from-clean-checkout decision (fresh or
  seed from the committed plan10 snapshot — the ledger stays scheduling
  state, out of the durable layer), and closes item 8 if execution finds no
  new M-sized gap. Plan11 is retired (parked 18B `g1f3` rung idea). See
  `plan12.md`.
- **2026-10-02 — initiative pivoted (owner decision): tooling over
  database contents.** The founding goal ("build and grow a proof-line
  database") is re-scoped: the deliverable is the **tooling pipeline**
  (harvester, merger, ledger union, flip analysis, website handoff);
  harvesting batches are validation of that tooling, not deliverables.
  Motivation: plan6–plan10 were compute-heavy DB-growth batches (plan10's
  accidental pilot alone spent ~28B child-evals / ~98 min for 3 facts),
  which the owner did not intend. Consequences: item 8 added (tooling
  completeness audit + hardening — the next plan); plan11's pre-registered
  18B `g1f3` rung and further ladder sweeps are parked unless re-framed as
  tooling validation; items 5/6/7 stay open as the tooling gaps. The status
  index row is updated; the data-model decisions above are unchanged (they
  are the tooling's contract surface). Plan10's executed record (6 facts,
  falsified pilot pre-registration, arm A re-baselined by full in-session
  replay; standing layer DB `0d929f4c…`, 262 shards, ledger `de690bc1…`)
  stands as the pipeline's latest validated state.
- **2026-10-02 (newest) — plan10 executed (amended §9)**: the tier sweep
  (arm A, `--and-close-max-budget 100M` filter shipped as D1) ran its full
  1,080-job set and yielded **6 facts / 1,074 censors** (0.55 %/job) —
  **falsifying the plan-session pilot's 3-fact pre-registration**: the
  pilot's three 6B head probes warmed the harvest session's cross-job
  private TT, so its tier jobs ran under a different TT history
  (child_evals diverge on 1,053/1,080 shared jobs; 3 censors flip to wins).
  Arm A was re-baselined by a full in-session replay (byte-exact job lines,
  ledger, all 6 shards, manifest `e91ad57b…`); normative lesson: per-arm
  in-session replay is the only sound determinism gate — censor records are
  scheduling state, not bounds. Arm B measured the **`g1f3` 6B rung at TT
  1024 MB**: 3 censors at ≈ 6B each, 0 facts, work ≈ 9B, next rung **18B**
  (§4.4's corrected ladder; plan9's "12B" prose slip stays corrected).
  Standing layer advanced: DB `0d929f4c…` (55,703 nodes), manifest 262
  entries `e91ad57b…`, ledger `de690bc1…` (8,446 records; pass mix
  7,113/250/1,029/49/5); post-batch census 1,077 = 0 fresh + 1,077
  censored. 0 flips, root undecided, handover not triggered; the
  completion-critical-order candidate confirmed subsumed. Next: the 18B
  `g1f3` rung vs a head-only batch (plan11 sizing, ≈ 83B ≈ 5 h full-sweep
  alternative), website handoff (#5), parallel harvesters (#6, now with a
  TT-history note), DTM-upgrade pass (#3). See `report10.md` and
  `measurements/plan10/` (the pilot committed as a falsified
  pre-registration).
- **2026-10-01 (newest) — plan9 drafted (docs-only plan session)**: the
  §4.3 standing shape's first batch, with committed probe pins (report8
  finding 1's lesson applied — the census probe output is in the plan).
  The plan session **discovered a ladder defect**: plan8 §2's monotone
  floor (`max(2^(k−1) × base, work_done)`) revisits a deep probe at
  exactly its accumulated work with the standing base — a byte-identical
  deterministic repeat (plan8 H5's replay is the evidence), ~3B wasted
  evals per completion arm; the 2B rung was reachable only at base 1B,
  which blindly 500×-jumps the 4M-censored tail. Fix (normative): the
  **strict-growth ladder**, floor `2 × work_done` (supersedes plan8's
  floor clause for `and-close` only). Two arms pre-registered: the **fresh
  sweep** (the 257 never-visited replies at 4M, the measured-yielding
  territory; head probed at ply 19) and the **completion head
  escalation** (`g1f3`'s three defenses at 2B each via the new floor,
  then `e2e3`/`e2e4`/`g1h3`'s 23 replies at their first 8M rung; cap
  6.5B). The **factless-batch escalation policy** is set: fresh base ×10
  per factless batch (plan8 verbatim); the censor tier escalates
  automatically (every revisit ≥ 2 × cumulative work — the `g1f3` rung
  ladder 1B → 2B → 6B → 12B toward the 7–15B-equiv plateau band). Union
  ledger advance = the plan7 mechanism's second production use. See
  `plan9.md`.
- **2026-10-01 (latest before plan9)** — **plan8 drafted (docs-only plan session)**:
  the §4.6 closure-rule response (report7: sixth consecutive 0-fact batch;
  "another identical default batch is not a valid plan8"). The design
  dialogue **rejects the depth-rationing lever on arithmetic** (exposure
  caps cannot shrink the structural number-1 pool — unvisited children are
  counted via movegen; layer completion is unaffordable at ~27× layer
  growth; the ladder is plan6's measured no-go) and adopts the
  **fact-yield-oriented job-set change**: the new policy `and-close` whose
  job set is the tree's actual completion work — the missing replies of
  undecided rows, completion-gradient ordered. The plan names the
  **starvation mechanism** behind the six 0-fact sessions (the number ≥ 2
  pool — where all completion work sits — is unreachable under the fresh
  cascade) and pins the tree's near-closure structure by probe:
  `g1f3` (3 missing replies), `e2e3` (7), `e2e4` (7), `g1h3` (9), the root
  (13); completing any head row implies the root (either direction — the
  handover contingency is pre-registered with a conservative trigger).
  Two arms pre-registered: the **deep probes** (1B child-evals on `g1f3`'s
  three defenses — the first budgets one-to-two orders above the measured
  censor band on those positions: pilot 8-s ≈ 10–20M-equiv, 120-s sample
  ≈ 100–250M-equiv, 2-h ≈ 7–15B-equiv, in-harness 4M censors) and the
  **fresh-tail screen** (970 never-searched C3 replies at 4M — the
  class-closing probe). Normative deltas: the monotone revisit budget
  `max(2^passes × base, work_done)`, bump-only censors (decision 11
  superseded for this policy — no child exposure), flips stay derived
  (decision 10 stands, no propagation pass). `and-close` becomes the
  harvest default in every outcome; 0 facts everywhere escalates the base
  ×10 per factless batch. Ledger advance = union(arm1, arm2) — the plan7
  union mechanism's first production use. See `plan8.md`.
- **2026-10-01** — **plan7 drafted (docs-only plan session)**:
  batch 3 pre-registered as a **two-arm A/B over the standing post-plan6
  state** — arm 1 (control): the shipped default (rationed expansion,
  layer caps 24, reserve 0) from the standing ledger as-is; arm 2 (union):
  the identical config from a **ledger-unioned** copy that recovers the
  censor knowledge plan6's standing-ledger advance dropped (report6
  finding 4). Ledger records are selection state only (no outcomes), so
  the union is a conservative max-rule merge (`passes_failed` max, tie by
  `work_done`) with a soundness-empty surface — and the N-way primitive
  item 6's per-worker ledger merge needs. Probes on throwaway copies
  pinned both censuses (arm 1: 4,608 jobs; arm 2: 5,987 jobs) and the
  union composition (5,950 records = 4,569 standing + 1,381 recovered
  paths from arm A + 57 upgrades from A/B; ply-2 fresh 190 → 141). The
  re-censor metric (finding 4 made measurable: expected 24 vs 0,
  ≈ 96M-eval waste bound) and the decision rule are pre-registered;
  **§4.6 fixes the plan8 closure rule**: if the expected 0-fact result
  holds (sixth consecutive), another default batch is invalid — the
  depth-rationing lever (finding 1) or a yield-oriented selection change
  is mandatory next. Finding 1 is otherwise deferred to its own plan. See
  `plan7.md`.
- **2026-10-01 (later still)** — **plan6 executed (the selection
  mechanism + the A/B)**: the plan5 §2 decision implemented as the
  pre-registered config surface (`--pns-config`: eligibility,
  `interleave_k`, `reserve_share`, per-ply-layer visit caps, growth,
  rotation, rung cap; census gains `kind: expand|rung`) and A/B'd over
  the identical post-batch-2 state: arm A (plan4 selector control) ≡ arm
  A′ (degenerate config) **byte-for-byte** (the equivalence contract
  holds at full-batch scale); arm B fired exactly 9 reserve-bound rungs
  (root + seven ply-1 nodes eligible at start + c2c3's mid-session
  eligibility flip — the trigger works, all rungs censored); arm C (no
  ladder) reached ply 5 vs B's 4 at the same cap. **§4.5 decision rule →
  the ladder is a measured no-go**: the shipped default config is arm
  C's (compiled `reserve_share = 0.0`; the mechanism stays
  config-reachable). 0 facts in every arm; the standing ledger advanced
  to arm C's post-run state (`058a6202…`, 4,569 records). Findings:
  layer caps ration breadth per layer, not depth (arm B's ply-8 reach
  prediction wrong — measured 4); the reserve, not the rung cap or the
  eligibility, binds. Gates H1–H6 pass. See `report6.md`,
  `measurements/plan6/`, and `research_pns_ladder.md` (theory linkage:
  df-pn threshold escalation is the ladder's ancestor; the eligibility
  trigger is the original part).
- **2026-10-01 (later)** — **plan6 drafted; plan5's decision refined
  (docs-only, design dialogue)**: the within-layer decision became an
  explicit hybrid mechanism — breadth-PNS expansion stays the default;
  ladder escalation is granted only to nodes whose children are all
  visited (the eligibility trigger, from the user's argument that
  expansion *is* the parent's proof work), paced by an interleave and
  rationed by a reserve share + per-ply-layer visit caps; full config
  surface (`--pns-config` TOML) so rule variants are benchmarkable, with
  census gaining a `kind: expand|rung` field so transcripts trace where
  the split fires. plan6 pre-registers the A/B: status-quo control from
  the post-batch-2 state, its degenerate-config equivalence replay,
  mechanism defaults, and the reserve=0 ladder-deletion arm — the
  ladder's marginal value is the measured question, with a measured
  no-go as a valid outcome. Research note `research_pns_ladder.md`
  (plan6 D3) mines the vendored PNS/DF-PN theory (threshold escalation,
  Nagai) and records the provenance chain and rejected alternatives.
  See `plan5.md` (amendment) and `plan6.md`.
- **2026-10-01** — **plan5 pre-registered (docs-only plan session)**: the
  within-layer selection question (report4 finding 3) decided as a
  **per-layer budget split with a scheduled ladder reserve**; work-aware
  effective numbers rejected by a degeneracy lemma (every fresh number-1
  record has zero ledger state, so any ledger-derived key collapses to the
  current `(number, ply, path)`), and the pool-growth lemma shows the
  ladder can never fire in-session under any ordering (censor exposure
  ≈ +18.2 fresh records/visit vs ≤ 1 removal). Batch 2 is pre-registered
  as the unchanged-policy control over the exposed frontier (measured
  census: 1,483 jobs / 1,408 number-1 / 75 number-2, superseding report4's
  1,417 figure); the split itself is plan6 implementation work, A/B'd
  against the batch-2 control. See `plan5.md`.
- **2026-09-30** — **plan4 executed** (breadth-first PNS harvester):
  `--policy breadth-pns` shipped as the default (live `(number, ply, path)`
  queue over the open frontier; sidecar work ledger `data/proofdb_work.json`
  with censor-time frontier exposure and the lineage gate; working layer at
  gitignored `data/`). First batch: **0 facts / 300.0M child-evals — exactly
  the pre-registered expectation**; the live queue swept plies 0–2 fully
  (75 censored jobs at 4M), exposed 1,368 undecided children into the
  ledger (1,443 records total), and the first ladder rung never fired
  in-session. Gates H1–H6 pass (H5: 75/75 records + byte-identical
  ledger). Findings: plan4 §1's pre-registered census (7 implied wins +
  1 implied loss + 41 jobs) came from a no-movegen probe — the normative
  §2 rule (unvisited = 1 via movegen) yields 49 jobs, the 8 extra rows
  genuinely undecided; and a factless session degenerates to pure BFS —
  within-layer selection needs a work-aware key before the next large
  batch (see `report4.md` findings 1/3). DB unchanged (35,055 nodes,
  197 shards); `data/proofdb.db` byte-identical to plan3's committed
  grown2 DB.
- **2026-09-30** — **plan4 slimmed before execution (docs-only, second
  amendment)**: `--root-fen`/`--root-path` subtree scoping (checkpoint
  decision 12) parked as **item 7**. Reasons: the first batch runs at
  the startpos root (dead weight for the measurement), and the
  feature's only near-term consumer is item 6's per-worker subtree
  partition, whose requirements will likely reshape the MVP limitation
  anyway. Decision numbering in plan4 is unchanged (12 stays as a parked
  tombstone); the kickoff prompt and deliverables D2/task 1/§8 are
  updated in place. Frontier exposure (decision 11) and the live queue
  (decision 4 reworded) are untouched — they define what plan4 measures.
- **2026-09-30** — **checkpoint design review** (plan4 unexecuted,
  amended in place): the initiative was checked against the user's
  top-down harvest vision (N-thread runs from root or from any FEN,
  24h stoppable runs, merge appends, website consumes the extended
  tree). Agreed deltas, all folded into `plan4.md`: (1) the sidecar
  ledger is the pick-up state — a censored job's expanded-but-undecided
  children are recorded as known-open records, so the exposed frontier
  survives the run (no open-leaf shards; shard contract unchanged);
  (2) selection became a **live priority queue** with censor-time
  frontier exposure, replacing plan4's fixed passes — freshly exposed
  children (number 1) are explored before a censored node's doubled
  revisit, which is the layer discipline "next depth only when the
  current depth is explored/in flight"; (3) `--root-fen` (+ optional
  `--root-path`) plumbs the harvest root by FEN, resolved within the
  merged DB tree (ambiguity/absence aborts) — a subtree *view* over the
  same startpos-rooted tree, shards/grafting unchanged; (4) parallel
  harvesters stay item 6, sequenced after plan4's sequential
  measurement. pn/dn stay derived (DB tree + ledger), never stored —
  the ledger records exactly the non-derivable state. See `plan4.md`
  amendment note and decisions 3/4/11/12.
- **2026-09-29** — **pivot decided after plan3**: job selection moves from
  class gradients (plan3's C1/C2/C3) to **breadth-first PNS over the open
  frontier** (plan4): structural pn/dn numbers (unvisited = 1), min-pn at
  OR / min-dn at AND, prefer-closer-to-root ties, censored nodes re-visited
  at doubled work (geometric ladder). Rationale: the C2 cheap head is
  measured-exhausted, and the most proving lines are what the website's
  users want explored — the initiative goal stays *coverage AND proof*
  (metric = harvesting value, not root-solver speed; the handover rule
  stands). Numbers are selection state: sidecar work ledger under the new
  **`data/`** working layer (gitignored, with the derived DB), never in the
  DB, never facts. Parallel harvesters added as item 6 (sequential PNS
  measured first; campaign-prototype precedent). See `plan4.md`.
- **2026-09-29** — **item 4 executed** (plan3): coverage policy measured
  and shipped. `proofdb_harvest` now extracts the full frontier classes
  (C1 open / C2 unexpanded siblings of proving children / C3 unexpanded
  children of open nodes) with a free AND-completeness assert over every
  proven loss node, and orders jobs by pre-registered policies. The A/B
  at 300M child-evals each (fresh TT per policy, marginal-yield design):
  `open-deepest` 0 facts / 300M (baseline, as pre-registered);
  **`sharp-siblings` 90 facts / 300M (29.99 facts/100M, median 44.7k
  evals/fact, all ply-5 loss refutations under bound-1 win parents)**;
  `sharp-heavy-tail` marginal after P1's head: 0 facts at 300M screen +
  200M heavy — the sibling gradient saturates immediately after its head.
  Winner is now the default `--policy`. DB 12,655 → 35,055 nodes (75 open
  unchanged); standing shard dir at 197 shards. Gates H1–H6 pass; H5
  replay 385/385 screen records identical. See `report3.md`.
- **2026-09-29** — **item 2 executed** (plan2): standing shard directory
  (`shards/` + standing manifest), `proofdb_harvest` CLI
  (deterministic child-eval budgets, path-context replay, TT retention,
  screen + heavy tiers, stop conditions), first batch over the seed's 87
  open frontier nodes: 12 decisive (all wins, even plies 4–27 on quiet
  ladder lines), 75 censored (deepest 5 re-verified undecided at 40M
  evals); merge-after-batch → 12,655 nodes (75 open); gates H1–H5 pass;
  full batch replay byte-identical. One harvest-tool defect found and
  fixed (root-job false repetition from a self-seeded prefix; censored,
  no bad fact; see `report2.md` finding 1 — same latent issue noted in
  the campaign worker's `replay_path`).
- **2026-09-28** — **item 1 executed** (plan1): merger MVP in
  `examples/proofdb_merge` + `examples/proofdb/`, spec
  `docs/spec/global_proof_store.md`, seed DB built from the 95 plan4
  shards (12,081 nodes; 87 open); gates G1–G5 pass. One merger defect
  found and fixed by the pre-registered gates (parent-row id inversion —
  the run DB was wrong; see `report1.md`).
- **2026-09-28** — **initiative opened** (docs only): brainstorm converged
  on the path-keyed partial AND/OR tree, shards-as-truth / SQLite-as-view,
  single-writer merger in `examples/`, bound-by-default depth vocabulary,
  and the handover rule; `plan1.md` drafted (merger MVP + spec + seed
  graft). No code changed; product surface untouched.
