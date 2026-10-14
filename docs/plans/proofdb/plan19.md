# plan19 — `--moves <path>`: the scoped harvest (frontier-walk re-anchored at a startpos path)

**Initiative:** `proofdb` · **Executes:** backlog item 16 (added with this
plan) · **Size:** S (example-side selection layer + tests + docs; **no
`src/` changes**) · **Date drafted:** 2026-10-14 (plan session, docs-only)

> **Owner framing (2026-10-14 design dialogue):** new harvest flag,
> verbatim: *"`--moves 'a2a4 b7b5 b1a3'`"* — happy path: "`--moves` is
> given a path of uci moves from the starting position; only the resulting
> node or its descendants are searched / expanded"; unhappy path:
> "nonsense / illegal moves → error".
>
> **Design rulings proposed by the plan session and accepted by the owner:**
> (a) the scope is a **re-anchor**, not a filter — the scope node *is* the
> walk's root for the session; (b) a scope node with a stored decisive
> outcome ends the session (`exhausted`), with a stderr note explaining why
> — growing beneath proven nodes stays out of scope (plan4 decision-9
> territory unchanged); (c) tokens are whitespace-separated, exactly the
> path-key join, no comma dialect; (d) the census line gains an
> out-of-scope count and a `scope` field so its numbers describe the
> session's actual world.

## 0. State this plan starts from

- Plan18 consolidation: the only harvest policy is `frontier-walk`
  (`examples/proofdb/walk.rs` D1–D7 + `walk/driver.rs`); the CLI arg
  surface is `examples/proofdb/harvest_args.rs` (slim, `--policy` kept for
  compatibility). Operator manual: `docs/proofdb_pipeline.md`.
- **Why this is a selection-layer change only:** every ledger/DB key is an
  absolute space-joined UCI path from startpos, and every job already
  replays the *full* startpos→node path (`harvest.rs::replay_job_path`,
  the repetition context contract). Scoping therefore never touches
  search, shards, manifest, ledger format, or the merger — the searches
  and their results are bit-identical wherever the walk starts.
- **Anchor inventory (verified by code read this session):** the walk is
  anchored at the empty path `""` in exactly these places:
  `WalkFrontier::new` (drop-rule prefix loop, `ply_of`), `pick` /
  `root_blocked`, `root_state`, and the driver's virtual-root rule +
  `root_proven_stored`. External call sites of `WalkFrontier::new` /
  `root_state`: `driver.rs::run_walk_session` and five sites in
  `tests/proofdb.rs` (which includes `walk/tests.rs` verbatim).
- **The trap this plan exists to avoid (challenge 1 of the dialogue):** a
  naive *filter* keeps the game start as the anchor, and three mechanisms
  then misbehave under scoping — (i) the ledger is global and shared, so
  an empty/exhausted subtree bootstraps *at the game start* and burns
  budget above the scope; (ii) the drop rules read the whole path back to
  the game start, so a scope node behind settled territory is silently
  excluded; (iii) the census counts a world outside the scope. Hence
  ruling (a).
- **Bit-identity requirement:** with `--moves` absent the session must be
  behaviorally identical to today's (`scope` = empty ⇒ the re-anchored
  drop loop degenerates to the current one). The default gate's pinned
  fixture rebuild is the enforcement point.

## 1. Proposed mechanism (the deliverable)

`proofdb_harvest --moves 'a2a4 b7b5 b1a3'`: the frontier-walk runs with
its world rooted at the node reached by replaying the given moves from the
startpos. Everything else (budget, caps, stop file, ledger, manifest,
shards) keeps its tool-wide semantics.

**S1 — flag, parsing, validation.** `--moves <string>`; whitespace-
separated UCI tokens; absent (or zero tokens) ⇒ global session (exact
today behavior). Validation happens in `parse_args`, before any manifest/
DB/ledger state is loaded (fail fast, usage-class **exit 1**): the token
list is replayed sequentially from the startpos with
`harvest::replay_job_path`-equivalent logic — a malformed token or a line
that becomes illegal at any prefix is rejected with a message naming the
offending token. `--moves ''` is rejected (the flag's absence already
means global).

**S2 — re-anchoring semantics (normative).** Scope key = the moves joined
by a single space (empty = global). For the session:

- *Candidacy:* only ledger records whose key equals the scope key or
  starts with `"<scope> "` are admitted. Out-of-scope records are never
  candidates, never visited, and never written — but stay in the shared
  ledger file untouched (censors only bump visited nodes), so later
  global sessions pick them up unchanged. They are counted in the census
  as `out_of_scope`.
- *Drops:* evaluated **relative to the scope root**. The record's path is
  stripped of the scope prefix; a proven DB row on the scope-relative
  path (the scope node itself, incl. Lineage) or on a relative proper
  prefix drops the record. Proven rows *above* the scope root are
  ignored: the operator chose the scope deliberately, and those nodes are
  unreachable in this session.
- *Root rule:* the virtual-root bootstrap (walk.rs D2) visits the **scope
  key**; the root census state is the scope row's state. If the scope
  node is proven-stored (a proven DB row, or a manifest/shard path per
  the existing `root_proven_stored` check), the session stops
  `exhausted` **and** prints a stderr note
  (`scope root <path> is proven-stored — nothing to harvest`) — ruling
  (b): no work beneath proven nodes, decision-9 philosophy unchanged.
- *Ordering:* the `(ply, path)` lex order is computed on scope-relative
  values (equivalent to today's absolute order for in-scope records —
  they share the prefix — but specified relative so the scope root is
  ply 0, the same special position the game start has globally).
- *Unchanged:* the expansion visit, the handled rule (children of
  in-scope nodes are in-scope by construction), `JobClass` semantics
  (C1/C3/L relative to the scope root), the complete rule, the censor
  bump hook, ledger save policy, stop conditions, summary line shape.

**S3 — observability.** Additive output changes only:

- The session-start census line gains the out-of-scope count and the
  scope: `frontier-walk: records N (lineage L, ancestor A, out_of_scope O,
  complete 0) — root <state>; base budget B; scope <path|global>`.
- The `harvest: db …` stderr echo gains `; scope <path>` when scoped.
- Job records are unchanged (their `path` field is absolute already).

## 2. Implementation map

- `examples/proofdb/harvest_args.rs` — `Args.moves: Vec<String>`; parse +
  validate per S1 (reuses `super::harvest::replay_job_path`; usage exit 1
  on any rejection). Usage text updated.
- `examples/proofdb/walk.rs` — `WalkFrontier::new(ledger, db, scope:
  &str)`; scope-relative drop rules, `ply_of`/`path_of` relative,
  `out_of_scope` census field + `describe` update; `root_state(db,
  session, scope)`; `root_blocked(scope_key)`. Existing call sites
  (driver + `tests/proofdb.rs`) pass `""`.
- `examples/proofdb/walk/driver.rs` — scope threading through
  `run_walk_session`/`run_walk_batch`; virtual-root rule at the scope
  key; scope-aware `root_proven_stored`; the S2 proven-scope note.
- `examples/proofdb_harvest.rs` — pass the scope through; stderr echo.
- `tests/proofdb.rs` (+ `walk/tests.rs` since it is included verbatim) —
  unit: scope candidacy filtering, relative drop anchoring (incl. the
  ignored-above-scope case), relative ordering, scope root state; the
  five existing call sites updated to `""` (behavior-pinning).
- Integration (binary-level, existing harness style): `--moves` with a
  nonsense token and with a prefix-illegal line → exit 1 before any state
  is written; a scoped session on the fixture DB emits only in-scope job
  paths; the absent-flag fixture rebuild stays byte-identical.
- `docs/proofdb_pipeline.md` — option-table row for `--moves`; the
  selection-policy paragraph gains the S2 semantics (re-anchor, proven
  scope root ⇒ exhausted + note); output-grammar census line updated.

## 3. Edge cases (normative)

- Scope with no ledger/DB presence at all: the virtual-root bootstrap
  starts the subtree from scratch — the documented operating model, just
  rooted deeper.
- Scope at a stored **open** row: a legitimate first target (the
  fresh-layer `root open` bootstrap, transplanted).
- Scope equal to an existing ledger record's full path: that record is
  the scope root; its `work_done` is respected (underworked ⇒ search
  visit, worked ⇒ expansion), exactly the global root's semantics.
- Shared ledger with concurrent/past global sessions: no conflict — keys
  are absolute; out-of-scope records are read, preserved, and re-pickable.
- Proven scope root: exhausted + note (S2), a valid cheap session.
- Zero in-scope records and nothing reachable: natural `exhausted`.

## 4. Tasks

1. S1 flag + validation + usage text (with unit test for the parser:
   good path, nonsense token, illegal prefix, empty string).
2. S2 re-anchored frontier + census (+ unit tests; existing call sites
   updated to `""` and re-pinned).
3. Driver root rule + proven-scope note (+ unit tests: virtual root at
   scope key, proven scope root exhausted).
4. Integration tests (binary-level): exit-1 unhappy paths, in-scope-only
   job paths, absent-flag bit-identity on the pinned fixture.
5. Doc updates (manual: option table, selection policy, output grammar).
6. Gate: `cargo fmt`, `cargo clippy`, `make test`.
7. **Final task:** write `report19.md` (tools used, problems, unresolved
   parts, missing tests, next steps) and update `initiative.md` (item 16
   added/closed, the log entry).

## 5. Risks

- **Census wording is pinned by report16** — the additive S3 fields are a
  format change for any external parser of that line; the manual and the
  plan both record the new shape, and no in-repo test pins the string.
- **Silent scope regressions:** the bit-identity requirement (§0) plus
  the `""`-pinned call-site tests are the guard; the default gate covers
  both.
- **Out-of-scope proven prefixes ignored (S2 drop rule):** a scope chosen
  inside alternative-move territory can harvest a node whose outcome is
  game-theoretically implied by an ancestor above the scope. Accepted by
  ruling (a)/(b) — the operator chose the scope deliberately; the merger
  stores such facts normally.
