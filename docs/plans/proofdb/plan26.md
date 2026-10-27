# plan26 — Flip analysis OR-witness semantics (`proofdb_flip` stops demanding AND-completeness for OR claims)

**Initiative:** `proofdb` · **Executes:** backlog item 24 (added 2026-10-27 by
this plan session) · **Size:** S (one read-only example tool + its unit/CLI
tests + docs; no `src/`, no schema, no spec, no shard/DB-format changes) ·
**Date drafted:** 2026-10-27.

> **Item 24's text:** "Flip analysis OR-witness semantics: `proofdb_flip`
> must accept an OR-witness for side-to-move-win flips (one resolved loss
> reply proves the row) and require full reply coverage only for
> side-to-move-loss flips; the report carries coverage and bound-mode
> (`or-witness` upper bound vs `and-complete` exact) in stdout and JSON; the
> operator manual, the tool's module header and `AGENTS.md` state the
> semantics — superseding the plan8 §4.5 'AND-completeness' gate wording and
> retiring the false 'a flip is a contradiction' reading; the new semantics
> are pinned by fixtures that are red against today's code. Read-only: the
> tool still writes only its report."

## 0. Why this plan

### 0.1 The measured failure

A production layer grown by a post-plan21 harvest contains an open ply-8 row

```text
g1f3 d7d6 b1c3 c7c6 f3g5 f7f6 c3b5 f6f5     fen: rnbqkbnr/pp2p1pp/2pp4/1N3pN1/8/8/PPPPPPPP/R1BQKB1R w KQkq - 0 5
```

with **one proven loss child** `b5c7` (bound 22) and its sibling `a2a3`
censored at 100 M child-evals with **no row** (a censor is scheduling state,
not a tree row — `docs/spec/global_proof_store.md` §2.1). `proofdb_flip` dies:

```text
proofdb_flip: FLIP VERIFICATION FAILED: flip "g1f3 … f6f5": reply a2a3 has no DB row
```

The fixpoint is not the problem: it already derives the OR-win
(`∃` resolved loss reply ⇒ the row's mover wins) and its bound. The
**verifier** is: `Analysis::verify` walks *every* legal reply and demands a
DB row with a resolved status before it accepts any claim — AND-completeness,
the plan8 §4.5 gate, applied to an OR claim.

### 0.2 Why it is now permanent, not transient

Plan21's OR-node sibling skip (`Drop::OrClosed`, the D4 dynamic or-close,
the D5 decided-subtree sweep) makes a censored sibling of a proven loss
child a **permanent non-candidate**: once one reply is proven `loss`, the
walk never searches the siblings again. So the missing row will never
materialise by harvesting — the analysis can only be fixed by verifying the
claim it actually made. (The same structure is why "0 flips in a sound DB"
is a wrong health check rather than a safety property: an or-closed parent
is a *legitimate derived result*, not an inconsistency — §0.3.2.)

### 0.3 Two further defects found by reading the verifier/claim path

1. **Claims are derived once and never revised.** `Analysis::pass` computes
   an open row's claim the first time it becomes derivable and never
   recomputes it, while `verify` recomputes from the *final* state. A row
   first derived in round 1 (witness bound 10 ⇒ claim 11) whose *second*
   loss witness is resolved only in round 2 (bound 3 ⇒ true claim 4) is
   therefore reported with a stale bound and then fails verification —
   `FLIP VERIFICATION FAILED … want …11…, verified …4…`. Latent today (both
   live layers have 0 claims), but it becomes the *first* thing an OR-witness
   world trips over, because or-witness claims are the ones whose witness set
   keeps growing. The fixpoint must be converged on **values and metadata**,
   not merely on "no new claims".
2. **The documented meaning of a flip is wrong.** The manual calls a flip "a
   row whose implied outcome contradicts its stored outcome (0 in a sound
   DB)" — a flip is by construction an **open** row (`verify` explicitly
   rejects "flip at proven row"); nothing is contradicted, a decisive value
   is *derived*. The "expect flips 0 / any flip is a soundness signal"
   operator reading is the same error and must be retired together with it.

### 0.4 Probes run in this plan session (read-only, on this container)

| probe | command | result |
| --- | --- | --- |
| cited path is legal, `a2a3` legal, `b5c7` legal | `list_legal rnbqkbnr/pp2p1pp/2pp4/1N3pN1/8/8/PPPPPPPP/R1BQKB1R w KQkq - 0 5` | 30 legal replies, `a2a3` **first** in movegen order — the production message reproduces in a synthetic fixture |
| standing layer | `proofdb_flip --db data/proofdb/proofdb.db --shard-dir <empty tmp>` | `open_rows 64 flips 0 verified 0 root Null (fixpoint 1 rounds)`; 43,092 nodes; **0** open rows with a proven loss child |
| committed fixture set (R1 fold) | `proofdb_merge --manifest docs/plans/proofdb/shards/manifest.json …; proofdb_flip` | `open_rows 75 flips 0 verified 0 …` (the manual §7.3 example, verbatim); 55,703 nodes; **0** open rows with a proven loss child |

Consequence used below: the new semantics **do not change** the flip output
of either live layer (their open rows are walk-expansion spines — 0 or 1
resolved child), so every existing pinned count survives and only the
stdout line's grammar needs re-pinning.

## 1. Decisions (fixed before implementation)

1. **D1 — OR-witness verification (the asymmetry), and a fixpoint converged
   on values *and* metadata.**
   For an open row `R` (ply `p`, side to move `S`) with legal replies
   `r_1…r_n`, each with a resolved status `σ_i ∈ {resolved(win|loss, bound),
   unresolved}` (`σ` = the DB row's outcome+bound, or the implied claim):

   - **stm_win claim** — sound iff `∃ i: σ_i = resolved loss`. Claimed bound
     = `1 + min{bound_i : σ_i resolved loss}`. Unresolved siblings do not
     weaken the *outcome* (they can only hold a shorter win, so the bound is
     an upper bound — D2). `coverage = and-complete` iff every reply is
     resolved, else `or-witness`.
   - **stm_loss claim** — unchanged and strict: sound iff **every** reply is
     resolved and none is a loss; bound = `1 + max{bound_i}`;
     `coverage = and-complete` always.

   `Analysis::verify` gains the mode: for a stm_win claim it recurses only
   into the loss witnesses (implied ones — their own claim chain must hold)
   and bounds-checks `min + 1`; for a stm_loss claim it keeps the
   full-coverage requirement and names the offending reply:

   - missing row: `flip {path:?}: reply {uci} has no DB row (a loss claim requires and-complete coverage)`
   - row present, no claim: `flip {path:?}: reply {uci} unresolved (a loss claim requires and-complete coverage)`
   - stm_win claim with no loss witness (defensive; the fixpoint never emits one): `flip {path:?}: no loss witness (a win claim requires at least one resolved loss reply)`

   **Recompute-to-fixpoint replaces derive-once.** Each pass recomputes
   *every* open row's claim **from the previous pass's state** (never from
   the pass in progress — that keeps the pass order-independent), and the
   loop stops when the whole claim map (value + bound + metadata) is
   unchanged. Monotonicity invariant, `debug_assert`ed and named in the
   error path: claims are never dropped and bounds never increase (a new
   witness only tightens the min). The magic `500`-pass ceiling becomes
   `open_rows + 8` with the message `fixpoint did not converge in N passes
   (open rows M)` — a defect tripwire, not a work bound. Rationale: only a
   converged state is *independently recomputable*, which is the tool's
   stated contract ("every reported flip is recomputed by an independent
   recursive verifier"); derive-once is history-dependent and can emit a
   claim (and a bound) the verifier cannot reproduce.

2. **D2 — coverage + bound-mode in stdout and JSON, with honest exactness.**
   Per claim: `coverage ∈ {or-witness, and-complete}` and
   `bound_mode ∈ {upper, exact}`, plus the diagnostics `loss_witnesses`
   (resolved loss replies) and `unresolved_replies` (replies with no
   resolved status).
   - `or-witness` ⇒ `bound_mode = upper`, always (an unresolved reply may
     hide a shorter decisive line).
   - `and-complete` ⇒ `exact` **iff every contributing child's own bound is
     exact** — proven children with `depth_status = 'exact'`, implied
     children whose claim is itself `exact`. Contributors: for stm_win, the
     min-achieving loss witness(es) (the bound is exact iff the min is
     achieved by an exact witness); for stm_loss, **all** replies (the bound
     is a max over them).
   - **Deliberate refinement of the item's short wording** ("and-complete =
     exact"): the DB's own vocabulary forbids it. 22,808 of the standing
     layer's 43,092 rows are `depth_status = 'bound'` (spec invariant 4: the
     stored bound is an upper bound unless `exact`), so an and-complete flip
     over bound-status children would claim a minimality it cannot derive —
     a correctness-first violation. The honest rule above still delivers the
     requested distinction; the fallback, if review prefers the literal item
     text, is to report `coverage` only and drop `bound_mode` (never to emit
     an unsound `exact`).
   - Plumbing: `DbRow` (`examples/proofdb/db.rs`) gains
     `depth_status: Option<DepthStatus>` (the type already exists in
     `proofdb::merge`), populated by `load_db_rows`'s `SELECT` (an
     unrecognised non-NULL value aborts, naming the row). Additive for the
     harvest/frontier code (which ignores it); three test literals gain the
     field (`examples/proofdb/walk/tests.rs`, `tests/proofdb.rs` ×2).
   - **Stdout** (one line, superseding the pinned grammar):
     `flip_analysis: open_rows <n> flips <F> verified <V> root <Null|l> (fixpoint <R> rounds; or-witness <O>, and-complete <A>, exact <E>)`
     with `O + A = F`, `E ≤ A`.
   - **JSON** (`--out`): additive keys only — top level
     `flips_or_witness`, `flips_and_complete`, `flips_exact`; per flip
     `coverage`, `bound_mode`, `loss_witnesses`, `unresolved_replies`.
     Every existing key keeps its meaning. A claim whose recomputed metadata
     differs from the stored claim aborts like a bound mismatch
     (`FLIP VERIFICATION FAILED: …`).
   - Exit codes unchanged (`0` normal — including with flips, `1` usage/DB
     load/unmerged-shard gate/any verification failure).

3. **D3 — the documentation set states the semantics (and supersedes the old
   gate wording).** Three surfaces, one normative text:
   - `docs/proofdb_pipeline.md` §7.3 rewritten: the asymmetry (OR-witness vs
     AND-complete), `coverage`/`bound_mode` meanings, the outcome/bound
     split ("coverage qualifies the *bound*, never the *outcome*"), the
     re-pinned stdout line and JSON example (zero values — probe §0.4), and
     the worked invocation. Plus the operator reading fixes: §2.2's
     `proofdb_flip` one-liner, §3.2's cycle comment (`expect: flips ≥ 0,
     verified == flips`), recipe R3 (a flip is a derived claim, **not** a
     soundness signal; the defect signals are `verified < flips` / a named
     verification error / a claim you cannot explain), and a §11 history
     entry. Explicitly corrected: the false bullet "`flips` — rows whose
     implied outcome contradicts their stored outcome (0 in a sound DB)"
     becomes "rows whose stored facts already imply a decisive outcome — open
     rows by construction; a proven row is never flipped, so a flip never
     contradicts a stored outcome".
   - `examples/proofdb/flip/mod.rs` header (D5's new module) carries the
     same semantics as the code's normative statement, with the sentence
     "supersedes the plan8 §4.5 AND-completeness gate for side-to-move-win
     claims (an OR claim needs one witness, not coverage)" and the routing
     note (post-plan21 layers legitimately carry or-witness claims; or-close
     makes them permanent), plus the root-claim caveat (an or-witness root
     claim is a *sound* startpos outcome claim; only its bound is an upper
     bound — the initiative's handover rule stays untouched, see §5).
   - `AGENTS.md`: the `proofdb_flip` bullet gains OR-witness/AND-complete
     semantics, `coverage`/`bound_mode` reporting, "a flip is a derived
     claim, not a contradiction", and the split module location.
   No change to `docs/spec/global_proof_store.md` (the flip report is not a
   DB surface — spec §7's example consumer queries are unaffected).

4. **D4 — fixtures: the new semantics are pinned red-first.** Unit tests in
   `examples/proofdb/flip/tests.rs` (fast gate — see D5), all built by
   *declaring* rows over a real startpos in `Vec<DbRow>` form (no solver, no
   DB, no search):
   - `or_witness_flip_needs_one_loss_reply_not_coverage` — root (20 legal
     replies) with `g1f3` declared proven `loss` bound 5, `e2e4` declared
     proven `loss` bound 9, the other 18 missing ⇒ claim `stm_win` bound 6,
     coverage `or-witness`, `bound_mode upper`, `loss_witnesses 2`,
     `unresolved_replies 18`, and `verify` clean. **Red against today's
     code** (`reply a2a3 has no DB row`).
   - `or_bound_is_the_tightest_resolved_loss` (the "wrong OR bound" red) —
     an **and-complete** OR row (so that a pre-fix run gets *past* the
     coverage check and reaches the bound comparison): all 20 root replies
     declared, 18 proven `win` bound 2, one proven `loss` witness bound 10,
     one open child whose 20 replies are all declared proven `win` bound 2 ⇒
     round 1 claims 11, the converged claim is **4** (`min(10, 2+1) + 1`),
     `coverage and-complete`, `bound_mode upper` (the min-achieving witness
     is itself only `upper`). **Red against today's code** on exit 1 with
     `want … bound 11 …, verified … bound 4 …` (derive-once).
   - `loss_flip_requires_and_complete_coverage` (the "loss flip with a
     missing reply" red-guard) — root with 19 of 20 replies declared proven
     `win` ⇒ **no claim at all**; `verify` with a fabricated `stm_loss` claim
     ⇒ `Err` containing `requires and-complete coverage`; the positive
     variant (all 20 declared `win` bound 3) ⇒ claim bound 4,
     `and-complete`, `bound_mode exact` when every child is `exact`, `upper`
     when one child is `bound`. Turns red under a naive
     "OR-witness-for-everything" relaxation — that is what it guards.
   CLI fixtures in the new `tests/proofdb_flip.rs` (harness shape of
   `tests/proofdb_harvest_cli.rs`: `cargo run --example` under a run lock;
   both DBs are **synthetic**, built with `schema::ensure_fold_schema` + SQL —
   a `built_from` meta row, a startpos-rooted node set, an empty `shards`
   table, an empty shard dir — because neither shape is fold-producible: a
   shard never carries an open parent, and no live layer has an open row with
   a proven loss child, §0.4). Both run against today's binary **before** any
   code change; the report records their exact messages:
   - `or_witness_flip_reports_coverage_and_bound` — the spine of the cited
     path above plus one proven `loss` child `b5c7` with `depth_bound 22`:
     exit 0, stdout carries `or-witness 1, and-complete 0`, JSON flip
     `{ply 8, implied "root-win", side_to_move "win", bound 23, coverage
     "or-witness", bound_mode "upper", loss_witnesses 1,
     unresolved_replies 29}` (30 legal replies — probe §0.4). **Today**:
     exit 1, stderr matching `has no DB row`.
   - `or_bound_is_recomputed_to_the_fixpoint` — D4's second unit fixture
     instantiated as a DB (all root replies declared; two loss witnesses,
     one proven at bound 10, one implied at bound 3): JSON bound 4,
     `coverage "and-complete"`, `verified 1`. **Today**: exit 1,
     `FLIP VERIFICATION FAILED … want … bound: 11 …, verified … bound: 4 …`.

5. **D5 — file split: yes.** `examples/proofdb_flip.rs` is already 13.3 KB
   with a size-justification header; D1/D2 add the asymmetry, the converged
   fixpoint, the metadata recomputation and the report fields, and D4 needs
   unit tests inside the logic — which **`cargo test` does not run for
   example targets** (`tests/proofdb.rs`'s header records this; verified
   again this session: `cargo test --release --example proofdb_merge --
   --list` lists 55 tests that the fast gate never reaches). So:
   - `examples/proofdb_flip.rs` — CLI, gate, load, `Analysis::solve`, report
     emission (target ≲ 7 KB)
   - `examples/proofdb/flip/mod.rs` — `Flip`, `FlipEntry`, `Coverage`,
     `BoundMode`, `Analysis` (`new`, `solve`, `claims`, `root`,
     `open_rows`), the shared reply-resolution helper, the normative module
     header (target ≲ 8 KB)
   - `examples/proofdb/flip/verify.rs` — the independent verifier +
     `verify_all` metadata comparison (target ≲ 5 KB)
   - `examples/proofdb/flip/tests.rs` — D4's unit fixtures (target ≲ 7 KB)
   - `tests/proofdb_flip.rs` — D4's CLI fixture
   `examples/proofdb/mod.rs` gains `pub mod flip;` (so the existing
   `#[path = "../examples/proofdb/mod.rs"]` harnesses compile the unit tests
   into the fast gate for free — the repo's established pattern). Every
   resulting file is under the 10 KB convention limit, so the tool's
   12 KB justification header is retired (replaced by the factorisation note
   in `flip/mod.rs`). Placement is codegen-neutral (examples only).

6. **D6 — read-only, examples-only, no contract change.** No `src/` change,
   no schema change, no spec change, no shard/dump-format change, no new
   flag, no new DB write: the tool still opens the DB read-only and writes
   only `--out`. `proofdb_verify`, `proofdb_merge`, and the harvest are
   untouched (`DbRow`'s additive field is the only shared-type change, and
   it is `#[allow(dead_code)]`-style tooling plumbing).

7. **D7 — no retroactive edit of `plan8.md`.** Plan8 §4.5/§5's
   "flip claim without AND-completeness is a defect" was the correct gate
   for the tree it was written against (no or-close, every claim a
   completion claim). Plans are historical records; the supersession is
   recorded where it binds — the module header, the manual, `AGENTS.md`, the
   item 24 row and `report26.md` — not by rewriting plan8.

## 2. Implementation tasks

1. **`examples/proofdb/db.rs`** — add `pub depth_status:
   Option<super::merge::DepthStatus>` to `DbRow`; extend `load_db_rows`'s
   `SELECT` with `depth_status` and map `NULL | 'exact' | 'bound'` (any other
   value: `Err(format!("nodes row {id}: unknown depth_status {v:?}"))`).
   Update the three `DbRow { … }` literals (`examples/proofdb/walk/tests.rs`,
   `tests/proofdb.rs` ×2) with `depth_status: None`.
2. **`examples/proofdb/flip/mod.rs`** — carve the analysis out of
   `examples/proofdb_flip.rs` (types, `Analysis`, reply resolution,
   `solve()`); implement D1's converged fixpoint (fresh map per pass from the
   previous state, monotone `debug_assert`, `open_rows + 8` cap, named
   non-convergence error) and D2's metadata computation; root FEN comes from
   the DB content instead of the `Position::STARTPOS_FEN` constant
   (behaviour-neutral: `load_db_rows` already refuses a non-startpos root).
   Write the normative header (D3).
3. **`examples/proofdb/flip/verify.rs`** — `verify` with the D1 asymmetry and
   the exact error strings; `verify_all()` recomputing every claim and
   comparing value **and** metadata, returning the verified count or the
   first defect.
4. **`examples/proofdb_flip.rs`** — shrink to CLI (`--db`, `--shard-dir`,
   `--out`, unchanged usage text), unmerged-shard gate, `load_db_rows`,
   `solve()`, `verify_all()`, the D2 stdout line and JSON (existing keys
   unchanged, new keys added), the root note, exit codes unchanged.
5. **`examples/proofdb/mod.rs`** — `pub mod flip;`.
6. **Red capture first, then code.** Write `tests/proofdb_flip.rs` (D4's two
   synthetic DBs + assertions) and run it against the **unmodified** binary,
   recording both failures in the report; only then touch the tool. (The unit
   fixtures cannot be red-first — the API they pin does not exist yet — so
   the CLI twins are the pre-fix evidence and the unit tests are the lasting
   pins.)
7. **Unit fixtures** — `examples/proofdb/flip/tests.rs` (D4's three tests,
   with a small `row(path, outcome, bound, status)` helper and a
   movegen-driven "declare every legal reply" helper so the startpos's 20
   replies are generated, not typed).
8. **Docs** — D3's manual/AGENTS/module-header changes; re-pin §7.3's
   stdout example and JSON block from a fresh run of the fixture fold
   (expect the zero suffix); update §3.2, R3, §2.2, §11. Do **not** touch
   `docs/spec/global_proof_store.md` or `plan8.md`.
9. **Measurements** — `docs/plans/proofdb/measurements/plan26/README.md`:
   the probe table of §0.4 re-run post-change (standing layer + fixture fold
   `flip_analysis:` lines verbatim, both `flips 0`), the pre-fix red
   messages, and the CLI fixture's post-fix JSON. No transcripts.
10. **Report** — the final task (§7).

## 3. Acceptance criteria

- **A1** — the cited production shape is a passing CLI test: exit 0, flips 1,
  bound 23, `or-witness`; its documented pre-fix state is exit 1 with
  `reply a2a3 has no DB row`, recorded from a run against the pre-change
  binary (the red is captured, not asserted post-change).
- **A2** — a stm_loss claim still requires and-complete coverage (missing
  reply ⇒ no claim; fabricated claim ⇒ named error); an OR-witness claim
  over partially resolved replies verifies.
- **A3** — a claim converged across rounds reports the tightest resolvable
  bound (`or_bound_is_the_tightest_resolved_loss` / the CLI twin: 4, not 11)
  and recomputation agrees; under today's code both fail (documented red).
- **A4** — `coverage`/`bound_mode`/`loss_witnesses`/`unresolved_replies`
  appear in JSON and the counts in stdout (`or-witness O, and-complete A,
  exact E`); `and-complete` + a `bound`-status contributor is never
  `exact`; all pre-existing JSON keys unchanged.
- **A5** — both live layers still report `flips 0 verified 0` (with the new
  suffix); the pinned manual example is verbatim-correct after re-pinning.
- **A6** — the false "flips contradict stored outcomes" statement is gone
  from the manual, and the §3.2/R3 "any flip is a soundness signal" reading
  is replaced; the module header and `AGENTS.md` state the asymmetry and the
  plan8 §4.5 supersession.
- **A7** — every touched file ≤ 10 KB; the flip unit tests run under plain
  `cargo test` (they are visible in `cargo test --release proofdb`).
- **A8** — `make test` green, `cargo fmt --check` / `cargo clippy
  --all-targets` / `cargo doc --no-deps` clean, `git diff --stat src/` empty;
  the tool writes only `--out`.

## 4. Test plan

```bash
cargo fmt --all
cargo clippy --all-targets
cargo test --release proofdb                    # unit fixtures land here via the #[path] harness
cargo test --release --test proofdb_flip        # the synthetic-DB CLI fixture
make test                                       # the default gate
cargo doc --no-deps
```

`make test-full` is **not** required (no search, move-ordering, TT/GHI,
proof-tree or reconstruction change — AGENTS.md's trigger list), but running
it is harmless. Manual confirmation of the zero-impact claim:

```bash
cargo build --release --examples
BIN=target/release/examples
$BIN/proofdb_flip --db data/proofdb/proofdb.db --shard-dir "$(mktemp -d)" \
    --out /tmp/flip_standing.json      # expect flips 0 verified 0 … or-witness 0, and-complete 0, exact 0
cp -r docs/plans/proofdb/shards /tmp/fx26 && \
  $BIN/proofdb_merge --manifest /tmp/fx26/manifest.json --shard-dir /tmp/fx26 --db /tmp/fx26.db && \
  $BIN/proofdb_flip --db /tmp/fx26.db --shard-dir /tmp/fx26 --out /tmp/flip_fixture.json
```

## 5. Risks and limitations

- **An or-witness claim is weaker than it looks — in the bound, never in the
  outcome.** The manual must say so, because the two classes now appear in
  one report: `or-witness` bounds are upper bounds (unresolved siblings can
  hide shorter wins), `and-complete` bounds are exact only when every
  contributor is. An operator reading only `bound` is misled; the D2 fields
  are the remedy, and the JSON pairs `coverage` with every flip.
- **Or-witness claims are permanent on post-plan21 layers.** Because or-close
  never revisits a censored sibling of a proven loss child, `flips > 0` is
  the *normal* steady state of a grown layer. The "soundness sanity: 0
  flips" framing is retired with this plan (D3); the remaining health check
  is `verified == flips` plus explainability of every claim.
- **The root verdict's reading changes.** An or-witness root claim is a
  *sound* startpos outcome claim (that is exactly what or-close relies on);
  only its bound is an upper bound. The initiative's handover rule ("all ~41
  children resolved") stays untouched — it is conservative, not wrong — but
  a future plan should decide whether an or-witness root claim is a
  sufficient trigger. Recorded as a finding, not a change here.
- **Recompute-to-fixpoint costs one replay per open row per pass.** Both
  live layers have ≤ 75 open rows, so the wall-clock change is invisible;
  a DB with 10⁵ open rows would want an incremental version. Recorded as a
  limitation, not a design commitment.
- **`bound_mode` needs `depth_status`**, which means touching the shared
  `DbRow` (three test literals). If review rejects that ripple, D2's fallback
  (coverage only) keeps the OR-witness fix — never relax to an unsound
  `exact`.
- **The synthetic CLI fixture is not fold-producible** (a shard cannot carry
  an open parent with one proven child), so it is pinned as a hand-built
  schema-v2 DB. It must state that in its header so nobody "fixes" it into a
  merge test.
- **Latent-defect fixtures are latent by definition.** Neither live layer
  exhibits the derive-once bound divergence (§0.4: 0 claims), so A3's red is
  a construction, not a production incident; the plan says so rather than
  implying a field failure.

## 6. Out of scope

- Materialising flips as proven rows, or any DB/spec/schema change (item 12).
- Changing or-close, the walk, or harvest scheduling to resolve a censored
  sibling of a proven loss child (it is deliberately permanent — plan21).
- The handover rule's trigger wording, and any claim of startpos progress.
- `proofdb_verify`'s invariants (they already split OR/AND correctly and are
  AND-complete only at AND-nodes — no change needed).
- Any performance work on the fixpoint beyond the note in §5.
- Editing `plan8.md` (D7).

## 7. Final task

Write `report26.md`: what shipped per task, the gate result (`make test`, the
targeted `cargo test` invocations, fmt/clippy/doc), the D4 evidence in both
polarities (pre-fix failure message per fixture, post-fix assertion), the
§0.4 probe table re-run after the change, the file sizes before/after the
split, any deviation from the plan (including the D2 exactness refinement and
its ripple), the findings (§5's root-claim question and the fixpoint-cost
note), and the next steps. Update `docs/plans/proofdb/initiative.md`: item 24
row → done with the plan↔item mapping, plus a history line. Do **not** touch
`docs/plans/README.md` (no open/pivot/close).
