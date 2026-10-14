# Brief — the proofdb browser (external project)

**For:** the coding agent building the exploratory proof-line website.
**From:** the `atomic_solver` repo, `proofdb` initiative (item 5 handoff).
**Status of this brief:** informational handoff; the normative contract is
`global_proof_store.md` in this directory. Nothing in this brief overrides
the spec.

## 1. Mission

Build a read-only exploratory website over the startpos proof-line database
of atomic chess: a user navigates moves from the standard starting position
and sees, at every node, what is *proven* (win/loss, within how many plies,
exact or bounded), what is *undecided*, and which line the proof follows.
The product idea behind the data: a partial AND/OR proof tree of proven and
disproven atomic-chess lines, grown incrementally by a harvester pipeline
(out of scope here) and delivered as one SQLite file.

## 2. Inputs (everything you need is in this directory)

| file | role |
| --- | --- |
| `proofdb.db` | the database — the only data artifact. SQLite, read-only, schema v1 |
| `global_proof_store.md` | **the sole normative contract**: schema, field semantics, invariants, example queries. Read it fully before writing code |
| `HANDOFF.md` | version stamp + provenance + read-only consumption statement |

Verify on receipt: `sha256(proofdb.db) =
0d929f4c3c62e4bbb87e9b043b582716297b23e2f7332a36e82763b1486070b8`, and the
meta census below (§5).

Optional background (copy from the source repo if desired, not part of the
contract): `docs/proofdb_pipeline.md` — §5 is a consumer data-model primer,
§8.2 has ready-to-run consumer SQL recipes that were verified against this
exact DB.

## 3. The data in one paragraph

Every row of `nodes` is a position reached by a specific UCI move sequence
(the *path*) from the startpos; the same position via a different route is
a different row. `outcome` is from the side-to-move perspective: `'win'`
(OR-node: exactly one proving `'loss'` child is stored), `'loss'` (AND-node:
every legal non-terminal reply is stored as a `'win'` child), `NULL` (open /
undecided). `depth_bound` proves "decisive within ≤ d plies"; `depth_status
= 'exact'` asserts minimality. Terminality is derivable (`outcome NOT NULL
AND depth_bound = 0`), never stored. Every row is replay-verifiable from its
parent chain on a rules-correct atomic-chess implementation.

## 4. Semantics you must not get wrong

These are the two failure modes the data model primer calls out explicitly:

1. **The tree is partial, and absence is not evidence.** A missing child row
   means "no proven fact recorded (yet)" — not illegal, not refuted, not
   decided. The root has 7 stored children; its other ~35 legal replies
   simply have no rows. Any UI that renders a missing child as refuted or
   illegal is wrong.
2. **Open is open.** `outcome IS NULL` covers both "searched, no decisive
   result within some bounded budget" and "never searched"; the DB does not
   distinguish them. A non-terminal draw is also `NULL` — there is no
   `'draw'` outcome in v1. Display open nodes as *undecided*, never as
   draws.

Also: node identity is the path (never merge across paths); a bound
(`'bound'`) is an upper bound, only `'exact'` is minimal; `provenance` is
comma-separated shard tags (display as "evidence", no semantics implied).

## 5. Version stamp of the delivery (verify after receiving)

```text
sha256(proofdb.db) = 0d929f4c3c62e4bbb87e9b043b582716297b23e2f7332a36e82763b1486070b8
schema_version = 1
root_fen       = rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1
node_count     = 55703  (27768 win + 27860 loss proven, 75 open; 24833 terminals)
built_from     = e91ad57b44008fee65ab0b124fab51365c17d08f891777cde525cfecb61fb5fd
root children  = g1f3, g1h3, a2a3, c2c3, d2d4, e2e3, e2e4 — all open; root undecided
max ply        = 48; exact-status rows = 24833; shards table = 262 tags
```

Future deliveries carry a different `built_from` and census — that is
growth, not drift. Surface `built_from` (+ node census) in the UI as the
data version stamp.

## 6. The one real design decision: the rules engine

The DB stores no FENs, no legal-move lists, no positions — only paths and
facts. To display a board and to distinguish "legal reply with no row" from
"illegal", the browser needs an **atomic-chess rules implementation** (move
generation, atomic explosion capture rules, check/checkmate, halfmove
clock). Two options:

- **A. Port the rules to TypeScript/JS.** Self-contained, no toolchain
  beyond the web build. Risk: rules drift. Mitigation: the spec's invariant
  1 is your test oracle — replay every stored path and the movegen must
  reproduce a position where the stored facts are consistent (invariant 2's
  AND/OR completeness is checkable without hashes); invariant 4 (depth
  monotonicity) and invariant 7 (id ordering) are DB-side structural checks.
- **B. Compile the existing Rust rules crate to WASM.** The proofs were made
  with the `atomic-movegen` crate (v2.3, crates.io; used by
  `atomic_solver`). Compiling it (or a thin wrapper) to WASM gives
  bit-exact fidelity with the proof producer, including the Zobrist keys
  (which include the halfmove clock) — enabling true replay verification.
  Cost: a Rust→WASM toolchain in the build.

Recommendation: B if a Rust/WASM build is acceptable (fidelity is this
project's top quality attribute); A otherwise, with the structural
verification gates of §8 as mandatory compensation. Decide before writing
UI code — everything downstream depends on it.

## 7. Suggested architecture (not normative)

The whole DB is 3.7 MB — it fits in a browser tab trivially. A static,
backend-free site is feasible:

- Load `proofdb.db` once (sql.js / wa-sqlite WASM SQLite, or ship a
  pre-converted JSON blob built from the DB at build time — acceptable only
  if the conversion is a deterministic, re-runnable step over the delivery
  file).
- Navigation state = the current path (list of UCI moves). Resolve it by
  the per-ply indexed lookup (`SELECT id FROM nodes WHERE parent_id = ?
  AND move_uci = ?`) — there is no path column by design; node identity is
  the path.
- Per node render: position (rules engine), stored children with
  outcome/depth/depth_status, legal replies without rows shown as
  "unknown", provenance tags, and the parent chain breadcrumb.

Feature sketch, ascending:

1. Board + move navigation along stored lines; status coloring per child
   (proven win / proven loss / open / no-row-unknown).
2. Node facts panel: outcome, depth_bound, depth_status (exact/bound),
   terminality, provenance.
3. Principal-line rendering: from an OR-node follow its unique `'loss'`
   proving child; from an AND-node any reply — a walk to a `depth_bound = 0`
   terminal renders a complete proof line (24,833 terminal rows exist).
4. Views: open frontier (`outcome IS NULL ORDER BY ply, id`), shallowest
   proven wins (min `depth_bound` among `'win'`), a search by UCI line,
   shard/provenance browser (`shards` table).
5. Data version stamp + a self-check page running the §8 gates in-browser.

## 8. Acceptance gates (build these as automated checks, not vibes)

1. **Version stamp gate** — on load: `schema_version = 1`, `node_count`
   matches `COUNT(*)`, display `built_from`. Abort with a clear error on a
   schema_version ≠ 1 file.
2. **Structural invariant checks** (run over the whole DB at load or in CI;
   all are cheap SQL/application checks against the spec §4):
   - depth monotonicity: proven non-terminal OR-nodes have exactly one
     `'loss'` child and `depth_bound = child.depth_bound + 1`; AND-nodes
     have `depth_bound = max(child.depth_bound) + 1` over all children.
   - terminal nodes (`depth_bound = 0`) have no children rows.
   - `depth_status` NULL iff `outcome` NULL; ids strictly increasing in
     ply-order along `parent_id` chains (invariant 7 prefix ordering).
3. **Replay spot-checks** — replay a sample of paths (or all, if the
   engine is WASM-exact) through the rules engine; every node must remain
   consistent with its stored facts (e.g. a `depth_bound = 0` terminal must
   be statically decided in the replayed position).
4. **Absence discipline** — a code-review/manual gate: no code path may
   render a missing child as anything other than unknown/undecided.

## 9. Constraints

- **Read-only.** The producing pipeline owns all writes; updates arrive as
  a new `proofdb.db` delivery (new `built_from`). Never write to the DB,
  never cache derived state that outlives a delivery without keying it on
  `built_from`.
- The spec (`global_proof_store.md`) is the guarantee surface — nothing
  outside §4's invariants is promised. Do not build features that need
  facts the schema does not carry (e.g. "which unvisited replies are
  actually losing" — unknowable from the DB).
- The producer repo (`atomic_solver`) is out of scope for changes; if the
  data seems wrong, that is a defect report back to the owner, not a local
  patch.

## 10. Suggested kickoff prompt for the coding agent

> Read `BROWSER_BRIEF.md`, then `global_proof_store.md` (the normative
> contract) and `HANDOFF.md` in full. Verify the `proofdb.db` sha256 and
> meta census against §5 of the brief. Decide the rules-engine option
> (§6: TS port vs Rust/WASM of `atomic-movegen` 2.3) and record the
> decision + rationale in a short `DECISIONS.md` before writing UI code.
> Then scaffold the static site: DB load + version-stamp gate + structural
> invariant checks (§8) first, board navigation second, node facts panel
> third. The tree is a partial AND/OR proof tree — re-read §4 of the brief
> before rendering any missing child or open node.
