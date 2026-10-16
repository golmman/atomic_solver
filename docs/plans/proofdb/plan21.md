# plan21 — OR-node sibling skip in `frontier-walk`: at white-to-move nodes, one proven winning move prunes the siblings

**Initiative:** `proofdb` · **Executes:** backlog item 20 (added with this
plan) · **Size:** S–M (examples-side selection change + tests + docs; **no
`src/` changes**) · **Date drafted:** 2026-10-16 (plan session, docs-only)

> **Owner framing (2026-10-16 design dialogue), verbatim:** *"In positions
> where it is white to move, once one move is proven a decisive win we
> don't explore their sibling moves. (…) We assume white is ultimately
> winning atomic chess. Our goal is to narrow the corridors. To be more
> efficient we only need to prove one winning move for white (attacking
> OR-node)."*
>
> **Orientation correction (this session, applied as D1):** in the
> pipeline's outcome convention (`Outcome` is from the side-to-move
> perspective), a winning white move `m` at a white-to-move node `P` is
> proven when the **child** position after `m` (opponent to move) is
> proven **`Loss`**. A child proven `Win` is a *refutation* of `m` — it
> must **not** trigger the skip (the walk must keep searching `P`'s other
> moves). The owner's phrase "one move is proven a decisive win" is
> implemented as "one child is proven a decisive loss for the defender".

## 0. State this plan starts from

- plan18 made `frontier-walk` the only harvest policy; plan19 added the
  `--moves` scope re-anchor; plan20 moved censor state into the DB. The
  walk's normative contract is the D1–D7 module doc in
  `examples/proofdb/walk.rs`; the live loop is `walk/driver.rs`.
- **Machinery inventory (verified by code read this session):** the
  session-start drop rules live in `WalkFrontier::drop_reason` (lineage =
  own path a proven row; ancestor = proven proper prefix), fed by the
  scope-relative `proven` set built in `WalkFrontier::new` from DB rows
  with `outcome.is_some()`; `admit` re-evaluates the same rules after a
  censor creates a record; `mark_decided`/`mark_complete` retire the node
  itself only (no subtree effect); `pick` returns the shallowest
  `(relative ply, relative path)` candidate; the root rule (driver) fires
  when the candidate set is empty and is gated by `root_proven_stored` +
  `root_blocked`. The expansion visit replays the position (so the side
  to move is available there for free) and searches the first unhandled
  child. Census classes and the census line are pinned by report16/report19
  and by tests in `walk/tests.rs`, `tests/proofdb.rs`, and
  `tests/proofdb_harvest_cli.rs`.
- **Or-closed structure is already everywhere (read-only probes this
  session):**
  - production layer `data/proofdb/proofdb.db`: 4,314 nodes, **2,154**
    white-to-move nodes with a proven `loss` child (probe: child
    `outcome='loss'` at odd absolute ply ⇒ parent at even ply = white to
    move; e.g. `g1f3 a7a5` → proven winner `f3e5`). Observation, out of
    scope: this production DB has no `censor` table (it predates the
    plan20 schema-v2 migration) — noted for the report, untouched here.
  - committed fixture (rebuilt via a real `proofdb_merge` into a temp DB):
    55,703 nodes, **23,977** or-closed parents. The static drop will bite
    immediately on the standing layers.
  - the CLI-test crafted fixture (`tests/proofdb_harvest_cli.rs`) has **no**
    or-closed structure: its only `loss` row (`d8h4`) sits under a
    black-to-move parent — its pinned *counts* are unaffected; only the
    census *string* gains a field.
- **Consequence, pre-registered:** the manual quickstart's session pins
  (§3.1 child-eval counts) **will drift** — future sessions skip
  or-closed territory by design — and must be re-pinned by this plan.
  Global sessions on or-closed layers are *not* byte-identical to
  pre-plan21 behavior (that is the deliverable); bit-identity claims are
  scoped to layers without or-closed structure.

## 1. Design rulings (proposed by this plan session; owner may veto any)

- **D1 — Trigger orientation.** The skip triggers on a child of a
  white-to-move node being proven **`Loss`** (defender to move loses the
  child ⇒ the white move into it wins). A child proven `Win` (defender
  wins) refutes the white move and does **not** trigger; draws censor and
  never trigger. The walk's own data model already stores exactly one
  proving child at an OR node — the skip makes the *search policy* match
  the *tree semantics*.
- **D2 — White-only, by absolute color.** "White to move" is determined
  by absolute path parity: a node at even ply (even number of moves from
  the startpos) is white-to-move (strict alternation; the startpos is
  white). Not "the attacking side", not scope-relative parity. The
  symmetric black-side skip (at black-to-move OR nodes, one proven `Win`
  child for black prunes siblings) is a **non-goal** — recorded as a
  possible follow-up; under the white-wins assumption those lines are
  refutation corridors whose full storage is wanted.
- **D3 — Rule A (static, session start + admission).** Let `P` be an
  in-scope white-to-move node with a proven DB-row child of outcome
  `Loss` (`P` is *or-closed*). Every in-scope ledger record **at or under
  `P`** is dropped from candidacy (`Drop::OrClosed`, third reason after
  `Lineage`/`Ancestor`; the shallowest decisive prefix wins, mirroring the
  existing attribution). Rationale: `P`'s value is established by the
  winning child — searching `P` re-derives it; records under the winner
  itself are already lineage/ancestor-dropped (the winner is a proven
  row); records under sibling branches are unnecessary by D1. Scope rule
  (plan19 analogue): or-closedness of nodes **above** the scope root is
  ignored — the operator chose the scope deliberately. `admit` gains the
  same check (defensive; unreachable today because no visit happens under
  an or-closed parent). Censor rows are **never deleted** — drops are
  candidacy-only (bump-only discipline preserved; the next policy change
  or scope re-picks them up).
- **D4 — Rule B (dynamic, in-session).** After a decisive visit at key
  `K` with outcome `loss`, let `P` be `K`'s parent (strip the last move
  token). If `P` is in-scope and white-to-move (even absolute ply), the
  driver **or-closes** `P`: a dedicated frontier method complete-marks
  `P` (so the scope root's or-close makes `root_blocked()` fire) and
  sweeps all candidates at-or-under `P` from the order set. The winner's
  own subtree needs no exception — it was already swept by D5 when `K`
  was decided. The expansion visit needs no separate DB-row check: a
  DB-row winner or-closes `P` statically (D3), so `P` is never picked.
- **D5 — Decided-subtree drop (generalization; owner veto point).**
  `mark_decided(K)` additionally sweeps all candidates at-or-under `K`
  from the order set. This is the in-session application of the
  cross-session lineage gate: every node under an in-session decided node
  is either inside the decided node's stored proof subtree (proven
  territory — re-searching it is pure waste; the next session's lineage
  gate drops it anyway) or an off-proof branch (unnecessary under this
  policy). It is what makes D4's "no exception for the winner subtree"
  sound. Beyond the owner's literal ask, but required to avoid
  immediately re-searching the winner's stored subtree in-session.
- **D6 — Root rule extension.** When the candidate set is empty: if the
  scope root is or-closed (statically per D3 — white-to-move with a
  proven `Loss` child row — or dynamically per D4 via the complete set),
  the session stops `exhausted`. A scoped session prints the plan19-style
  stderr note (`scope root … is or-closed by proven winning child … —
  nothing to harvest`); the global session stays silent. The census
  `root` field keeps its three states (an or-closed root row *is* stored
  open).
- **D7 — Surface, determinism, composition.** No new CLI flags; the skip
  is always on (flag-gating rejected: more surface to test, and the owner
  asked for the behavior). `is_handled` unchanged (a stored sibling row —
  any outcome — was never a job, plan8 D1 precedent: nothing to prune
  there). Determinism: the static drop derives from the deterministic row
  set; the dynamic winner is the first decisive `loss` in walk order; all
  ordering stays lex. Scoped sessions evaluate everything scope-relative
  as today; `--moves` and the global shape compose unchanged. No `src/`
  changes; searches bit-identical; harvest DB writes stay censor-rows-only.
- **D8 — Census and summary (accepted format change, plan19 precedent).**
  The census line gains `or_closed O` after `ancestor`:
  `records R (lineage L, ancestor A, or_closed O, out_of_scope X,
  complete C) — root <state>; base budget B; scope <path|global>`. The
  session summary gains `or_closes N` (dynamic or-closes; complete-marks
  via or-close do not count toward `completes`). Job-record grammar
  unchanged (or-closes are pick-only, no job).
- **D9 — File-size discipline.** `walk.rs` is at ~19.8 KB, at the
  documented "next natural split" line; this feature crosses it. The
  census/`RootState` types are peeled into `walk/census.rs` (the split
  the module header defers until "a further feature actually crosses
  it"), keeping `walk.rs` under ~20 KB.

## 2. Soundness and termination

- **Selection-layer only:** no solver, merger, shard, manifest, or DB
  schema change; the DB stays the sole truth. An or-closed parent's row
  stays `open` until item 12's closure promotion materializes it — the
  walk or-closes *work*, it never claims a fact.
- **Proof-theoretic basis:** at an OR node, one proving child suffices —
  the standard AND/OR rule; the white-wins assumption motivates the
  corridor narrowing but is not required for soundness. If `P` is in
  fact a loss for white, no child is ever proven `Loss`, nothing
  triggers, and the walk explores all of `P`'s children exactly as
  today (all of them are needed for a loss proof at a white-to-move
  node). Black-to-move nodes: behavior unchanged everywhere.
- **Termination:** each node or-closes at most once (complete set);
  sweeps only shrink the order set; or-close/complete iterations are
  pick-only and bounded by the once-per-node marks; stop conditions and
  the zero-eval censor guard are untouched. The walk's "does not
  self-retire on an undecided layer" operating model is unchanged —
  except that a fully or-closed world now legitimately exhausts (D6),
  which is the corridor-narrowing endgame.

## 3. Tests and gates

- **G1 — selection units** (`walk/tests.rs`, crafted rows — the crafted
  helpers need no replay legality):
  static `Drop::OrClosed` drop of a record under an or-closed parent;
  attribution order (lineage/ancestor win when shallower); a `loss` child
  under a **black-to-move** parent does not trigger; a `win` child under
  a white-to-move parent does not trigger; D5 sweep keeps non-descendant
  candidates; D4 or-close of the scope root sets `root_blocked`;
  `admit`'s defensive check; census string with `or_closed`.
- **G2 — driver tests** (`tests/proofdb.rs`, crafted DB + real tiny-budget
  session, the plan16 harness style): a session over a crafted or-closed
  layer visits only non-pruned records; after an in-session decisive
  `loss` child, no further visit happens at-or-under the parent (assert
  the visited job paths); or-closed scope root ⇒ `exhausted` with the
  scoped note; a full re-run is byte-identical (determinism).
- **G3 — CLI-level tests** (`tests/proofdb_harvest_cli.rs`): all census
  string pins re-pinned with `or_closed 0` (the crafted fixture has no
  or-closed structure — counts unchanged); one new test seeding a
  synthetic or-closed row by SQL `UPDATE` on the test's DB copy (the
  `seed_censor` SQL precedent; documented as synthetic test-craft) and a
  seeded ledger record under it — asserted never visited, counted
  `or_closed 1`; the scoped-note case.
- **G4 — the gate:** `make test` green; `cargo fmt`/`clippy`/`doc` clean.

## 4. Docs

- `examples/proofdb/walk.rs` module doc: D2 gains the or-closed drop, D3
  the dynamic rules, D5 the census fields (with the new split note of D9).
- `docs/proofdb_pipeline.md`: the walk policy block gains the OR-node
  sibling-skip paragraph (what triggers, what is pruned, records
  preserved); census grammar line updated; quickstart §3.1 session pins
  re-run and re-pinned (pre-registered drift).
- `AGENTS.md`: the `proofdb_harvest` bullet gains a short clause
  ("white-to-move OR nodes close on one proven winning child").
- `docs/plans/proofdb/initiative.md`: item 20 row + history entry (done
  in this session for the draft; the execution session appends the
  closure entry).

## 5. Execution tasks

1. D9 split: peel census/`RootState` into `walk/census.rs`.
2. Rule A (D3): `Drop::OrClosed` in `drop_reason` (white-parity helper),
   `WalkFrontier::new` + `admit`, census field + attribution.
3. Rule B + D5 (D4): `mark_decided` subtree sweep, `or_close` frontier
   method, driver hook after decisive visits, `or_closes` summary field.
4. Root rule extension (D6) + scoped stderr note.
5. Tests G1–G3; gate G4.
6. Docs (§4), quickstart re-pin probe, standing-layer census probe
   (before/after `or_closed` counts on the committed fixture).
7. **Final task:** `report21.md` (probe numbers, re-pinned pins,
   findings, unresolved parts) + initiative history entry.

## 6. Pre-registered expectations

- Quickstart session pins drift (fewer visits on the or-closed standing
  layer); re-pinned, not defended.
- Standing-fixture census shows `or_closed` in the thousands (23,977
  or-closed parents exist; the drop set is bounded by the censor-record
  count).
- Global sessions on or-closed layers are **not** byte-identical to
  pre-plan21 (the deliverable); the CLI-test fixture's counts are.
