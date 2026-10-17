# report21 — OR-node sibling skip in `frontier-walk`

**Plan:** `plan21.md` (executes backlog item 20) · **Date:** 2026-10-16
**Gate:** `make test` green (all suites, 0 failures) · `cargo fmt` /
`clippy --all-targets` / `doc --no-deps` clean.

## 1. What shipped

Exactly the D1–D8 rulings of plan21, plus one split amendment (D9):

- **Rule A (D3, static).** `WalkFrontier::new` builds the scope-relative
  `or_closed` set from the DB rows: every in-scope node at even absolute
  ply (white to move) with a proven `Loss` child row. `drop_reason` gains
  `Drop::OrClosed` in the same shallowest-first prefix loop as
  lineage/ancestor, so the attribution rule ("the shallowest decisive
  prefix wins") is preserved — a record behind both a proven row and an
  or-closed parent counts whichever prefix is shallower (see the re-pinned
  `walk_scope_drops_are_relative_and_ignore_above_scope`, where an
  or-closed prefix at ply 2 beats a proven row at ply 3).
  `admit` gains the same check, plus the defensive under-or-closed-ancestor
  rejection. Scope rule (plan19 analogue): or-closedness above the scope
  root is ignored.
- **Rule B + D5 (dynamic).** After a decisive visit, `mark_decided`
  sweeps the decided node's whole subtree from candidacy (the in-session
  form of the cross-session lineage gate — this is what makes the
  or-close sound without a winner-subtree exception). A decisive `loss`
  at an odd-absolute-ply child additionally or-closes its parent
  (`WalkFrontier::or_close_parent`: parity + parent existence + scope
  check on the frontier; `or_close` complete-marks it *without* counting
  toward `completes` (D8), sweeps its subtree, and records it in
  `or_closed_dyn`). Found and fixed during testing: `or_close_parent`
  must map a single-token key to the empty root path (the root has no
  parent — `rsplit_once(' ')` alone would silently drop ply-1 triggers).
- **D6.** `root_or_closed()` (static ∪ dynamic; a plain complete-mark
  stays silent) ends the session `exhausted`, with the plan19-style
  stderr note on scoped sessions
  (`scope root "…" is or-closed by proven winning child — nothing to
  harvest`), checked after proven-stored, before complete-blocked.
- **D7.** No new flags; always on; `is_handled` unchanged; all ordering
  lex; no `src/` changes; harvest DB writes remain censor-rows-only.
- **D8.** Census grammar: `records R (lineage L, ancestor A, or_closed O,
  out_of_scope X, complete C) — root <state>; base budget B; scope
  <path|global>`. Summary: `… new_shards S or_closes O wall Ws`.
- **D9 (amended).** `walk.rs` stayed under the ~20 KB line only with a
  *second* peel beyond the plan's census one: census/`RootState`/`root_state`
  → `walk/census.rs` (3.7 KB), and the scope-relative key algebra +
  drop/handled classification → `walk/keys.rs` (4.8 KB). `walk.rs` is
  19.8 KB with an updated justification header.

New tests (G1–G3): 6 walk selection units (static drop + attribution,
win-child/black-parent non-triggers, proven-over-or-closed attribution,
D5 sweep + ledger retention + defensive admit, or-close mechanics on the
scope root, or-close-parent parity/scope/idempotence), 2 driver tests
(static or-closed layer visits only open records; in-session decisive
loss or-closes the scope root and exhausts, deterministically over a
re-run), 1 CLI-level test (synthetic or-closed row by SQL `UPDATE` on the
test DB copy — documented test-craft; seeded record never visited,
counted `or_closed 1`, scoped note, `stop=exhausted jobs 0`, record
preserved), plus census-string re-pins everywhere. Existing tests
re-pinned: the four pick-walk unit tests now use `mark_complete` for the
sequential walk (a decided mark sweeps the subtree — the D5 semantics
they predate), and `walk_scope_drops_…` pins the new attribution.

## 2. Probes (`measurements/plan21/p21-probe.py` → `p21-results.jsonl`)

- **Committed fixture** (rebuilt via a real `proofdb_merge`, 55,703
  nodes): 23,977 proven `Loss` rows at odd ply, **23,885 distinct
  or-closed parents**, covering **50,258 / 55,703 nodes (90.2%)** at-or-
  under an or-closed parent. Matches the plan's pre-registered
  expectation (`or_closed` in the thousands) — see §6 for the census
  caveat: the fixture copy has no censor records, so the *current*
  static drop count is 0 there too.
- **Production dev layer** (`data/proofdb/shards`, 4,314 nodes): 2,154
  loss rows at odd ply = 2,154 distinct or-closed parents (each loss row
  has its own parent here); **4,308 / 4,314 nodes** at-or-under one — only
  the 6 open frontier rows are outside the or-closed territory. The
  layer's current 368 censor records drop **0** (`or_closed 0` in the
  fresh census line re-pinned into the manual): the pre-plan21 sessions
  never created records inside or-closed subtrees, because the territory
  under them was already fully harvested before the or-close could fire.
  The skip will bite going forward: the moment a session censors inside
  what a later merge proves, the next session drops those records.
- **CLI crafted fixture**: no or-closed structure (the `d8h4` loss sits
  under a black-to-move parent) — its count pins are unchanged; only the
  census strings re-pinned, as pre-registered.
- **Reference boot re-run** (root-only layer, budget 4M, 2G cap, 1024 MB
  TT, this container): `stop=budget jobs 549 decisive 49 censored 500
  evals 2000051685 new_shards 49 or_closes 0 wall 414.3s`, layer grown to
  4,030 nodes, `flips 0 verified 0`. The isolated pre-plan21 HEAD build
  run on the same container reproduces the boot **byte-identically**
  (same censor rows, all 49 shards, merged DB, manifest digest
  `64301f268490ff71`) — the skip never fires on a fresh layer: all 49
  decisive boot jobs are wins at odd-ply (black-to-move) nodes, i.e.
  refutations, so `or_closes 0` and the static `or_closed` set is empty.
  The doc's old boot pin (553 jobs / 53 shards / 6,706 nodes) predates
  the plan20 chunk-accounting change and this container — the drift is
  not caused by plan21. See
  `measurements/plan21/p21-boot-comparison.jsonl`.

## 3. Quickstart §3.1 re-pin (pre-registered drift)

The plan pre-registered that the §3.1 child-eval pins would drift. They
did **not** drift because of plan21: the smoke session (root-only layer,
no proven rows, both jobs censored) has no or-closed structure and no
decisive job, so the walk behaves identically. What changed is the
census string (`or_closed 0`) and the summary line (`or_closes 0`);
both re-pinned.

Separate, pre-existing finding: the `child_evals` chunk-overshoot values
on this container measure **200000 / 200015** (summary `evals 400015`),
while the manual pinned **200000 / 200021** (report20's reference-host
re-pin) — and the pre-plan20 manual had exactly 200015. Verified against
the pristine HEAD build (stash → run → unstash): 200015 here too, so
this is a host/build-environment dependence of the chunk-overshoot
accounting, not a plan21 regression. The manual now pins the container
values and states that the small overshoot deltas are host-dependent
(alongside `wall`/`wall_s`); **recommend re-pinning on the reference
host** if that host is the canonical one.

## 4. The §3.2 reference boot under the skip

The manual's boot paragraph pinned the plan16 gate numbers (553 jobs, 53
decisive shards, 6,706 nodes, byte-identical double boot). Re-pinned to
the fresh probe (both builds, this container):

| run | jobs | decisive | censored | evals | nodes grown | wall |
| --- | ---- | -------- | -------- | ----- | ----------- | ---- |
| plan16 gate (reference host, pre-plan20 code) | 553 | 53 | 500 | 2,001,311,002 | 6,706 | 216.2 s |
| pre-plan21 HEAD (8f58b8c, this container) | 549 | 49 | 500 | 2,000,051,685 | 4,030 | 418.0 s |
| plan21 (this container) | 549 | 49 | 500 | 2,000,051,685 | 4,030 | 414.3 s |

The plan21 and HEAD rows are byte-identical across every durable
artifact; the manual now pins the fresh numbers and names the drift
causes. `or_closes 0` on the boot: no white-to-move node was ever
decided by a decisive `loss` child — the boot's wins are all refutation
wins at black-to-move nodes. The skip's effect is expected on grown
layers (the fixture/production probes in §2), not on boots.

## 5. Tools and files

- No new tools; probes via `proofdb_merge` (+ `--import-censor` for the
  production layer's legacy `work.json`) and python `sqlite3` queries
  (the container has no `sqlite3` CLI). Committed under
  `docs/plans/proofdb/measurements/plan21/`: `README.md` (protocol +
  tables), `env.json`, `p21-probe.py` (the read-only or-closed census
  probe), `p21-results.jsonl` (the two standing-layer probe lines), and
  `p21-boot-comparison.jsonl` (the plan21 vs HEAD boot comparison). No
  solver transcripts committed (regenerable from the manual's recipes).
- Code: `examples/proofdb/walk.rs` (D2/D3/D5/D6 + census `or_closed`),
  `walk/census.rs` (new), `walk/keys.rs` (new), `walk/driver.rs`
  (or-close hook, root rule, `or_closes`), `examples/proofdb_harvest.rs`
  (summary line). Tests: `walk/tests.rs`, `tests/proofdb.rs`,
  `tests/proofdb_harvest_cli.rs`.

## 6. Problems encountered / unresolved parts

- **The D5 sweep's blast radius on unit tests** (predictable in
  hindsight): the four pick-walk unit tests retired candidates with
  `mark_decided`, which now sweeps subtrees — deciding the root empties
  the world. Re-pinned to `mark_complete` for the ordering walks; the
  sweep semantics are pinned by the new dedicated tests. No driver-level
  test needed changing: in-session, a decided node's subtree never
  carries censor records unless a previous session left them there (the
  budget-raise re-search scenario) — the boot test passes unchanged.
- **`is_multiple_of` clippy lint** on the parity check — adopted.
- **Unresolved:** the symmetric black-side skip (plan21 D2 non-goal) is
  recorded as a possible follow-up. Item 12's closure promotion (an
  or-closed open row staying `open` in the DB until then) is untouched,
  as scoped. The production DB at `data/proofdb/` still predates the
  plan20 schema-v2 migration (no `censor` table; the probe rebuilt it
  from its shards into a temp DB with `--import-censor`) — noted by
  plan21, untouched here.
- **Unresolved (2):** the static drop does **not** bite on either
  standing layer's *current* censor state (0 records dropped on both —
  the frontiers sit outside the or-closed subtrees), so the plan's
  pre-registered "or_closed in the thousands" expectation applies to
  future sessions once records exist inside or-closed territory, not to
  today's census lines. The manual's census paragraph states this
  explicitly. Correspondingly, the quickstart smoke is byte-identical at
  the job level (counts unchanged) and differs only at the
  census/summary strings (the accepted format change); the plan's "not
  byte-identical" claim holds for global sessions on grown layers, not
  for boot/smoke shapes.
- **Missing tests:** none known; the boot determinism claim is
  re-verified directly (the isolated HEAD build reproduced the plan21
  boot byte-identically, §4), stronger than a pinned test would be —
  but the determinism gate is a manual protocol, not an automated test,
  so the regression protection stays as before (the
  `standing_layer_rebuilds_byte_identical` and in-test determinism
  pins).

## 7. Next steps

1. Re-pin the quickstart chunk-overshoot numbers on the reference host
   (one session; only if the reference host is the canonical doc source).
2. Consider the black-side symmetric skip as a separate item if the
   corridor-narrowing results justify it (measure `or_closes`/job-yield
   over a few real sessions first — the skip's effect on the production
   layer's 368-record frontier is currently zero).
3. Item 12 (closure promotion) remains the natural next durable-layer
   step once enough or-closed territory accumulates un-proven.
