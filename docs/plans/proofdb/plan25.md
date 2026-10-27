# plan25 — Deep-hole export resilience (option A: keep the export abort fatal, make it recoverable)

**Initiative:** `proofdb` · **Executes:** backlog item 22 (added 2026-10-27 by
this plan session) · **Size:** S (one product module + proofdb example
tooling + docs; no solver, proof-tree, shard, or DB-format changes) ·
**Date drafted:** 2026-10-27.

> **Item 22's text:** "Deep-hole export resilience: a reconstruction hole
> below a deep parent must not be rejected by the fixed
> `fill_depth_cap`; a budget-censored fill attempt must be reported as a
> budget abort, not as depth exhaustion; the harvest gains the export fill
> budget knobs (`--fill-attempt-budget`, `--fill-total-budget`) and the
> export-rejection path carries the walk census; the operator manual
> documents the two abort classes, the recovery ladder, and the
> pre-registered three-strike quarantine rule. The harvest keeps aborting
> (exit 2) on a rejected export — the skip-and-censor alternative is
> parked as item 23 and is drafted only if the three-strike rule fires."

## 0. Why this plan

A production harvest run aborted on its first deep decisive job:

```text
[bounded_search] chunk done: work_done=16000005 next_chunk=32000000 …
proofdb_harvest: ABORT: job g1f3 d7d6 b1c3 c7c6 f3g5 f7f6 g2g4 f6g5 f2f4:
  export rejected: unfillable hole at root.e7e5.a2a3: fill depth cap 32 exhausted
```

Investigation (2026-10-27, read-only) root-caused it end to end:

- The job FEN is `rnbqkbnr/pp2p1pp/2pp4/8/5PP1/2N5/PPPPP2P/R1BQKB1R b KQkq f3 0 5`
  (solved decisive at ~16–32 M child-evals in the session; proven at
  63,954,107 evals / `Complete` in a cold run — identical at the 64 M and
  100 M child-eval budgets).
- Snapshot records along the walk: root `Win depth 47` (best move `e7e5`),
  `e7e5` `Loss depth 46`, `e7e5.a2a3` `Win depth 1` — the hole was a
  **mate in 1** whose record was evicted from the session's retained TT.
- `fill_hole` seeds the fill bound from the parent's proven depth and then
  *refuses to search it* when that bound exceeds the fixed config cap:
  `src/reconstruct/walker.rs:272-280`. Bound `46 > 32` ⇒ error with
  **zero child-evals spent** (hence no `[bounded_search]` lines between the
  job search and the abort). Deleting only the `e7e5.a2a3` record from a
  fresh snapshot reproduces the identical message and `fill_evals=0`;
  raising the cap to 200 exports the same snapshot (`fill_evals=7`, tree
  replay-valid, 14,394 nodes).
- The cap is an inherited prototype default (`examples/campaign_worker.rs:146`)
  hardcoded into the harvest's export config
  (`examples/proofdb/session.rs:82-89`) with **no CLI knob**, so an operator
  cannot get past it without editing source. Deep records are rare in a
  snapshot (16 of 261,893 solved records had depth > 32) but sit exactly on
  the proof path, so cross-job TT eviction makes these holes likely, and
  every one of them is currently fatal to the session.
- Two secondary defects in the same function: `bound *= 2` overshoots the
  cap instead of clamping (`walker.rs:298`, so a start of 20 with cap 32
  never tries 21…32), and a **budget-censored** attempt is
  indistinguishable from a genuine "no decisive line within bound" Draw —
  the loop then doubles (more expensive, still censored) and reports the
  wrong cause. Measured: attempt budget 2 M on a censored hole ⇒
  3 attempts / 6,000,051 evals burned, then "depth cap exhausted".
- The parent-derived bound is *already documented* in the walker as a valid
  upper bound for the child (`walker.rs:138-141`: `Win = min(child)+1`,
  `Loss = max(child)+1` ⇒ child depth < parent depth). The cap therefore
  contradicts the walker's own invariant.

**Owner decision (2026-10-27): option A** — keep "a rejected export aborts
the session (exit 2)" (`examples/proofdb/session.rs:122-124`, plan2 D2
"rejected, never patched"), and make that abort (a) rare, (b) accurately
diagnosed, and (c) recoverable by an operator without a rebuild. The
skip-and-censor alternative is parked as item 23 and is drafted only
if the three-strike rule fires.

## 1. Decisions (fixed before implementation)

1. **D1 — the parent-derived bound dominates the cap.** In
   `fill_hole`, `cap = max(config.fill_depth_cap, start)` where
   `start = parent_depth.unwrap_or(config.fill_base).max(1)`. The config cap
   then governs **root holes only** (no parent depth exists there, so the
   doubling search still needs a ceiling). Rationale: a non-root hole's
   required proof depth is strictly below its parent's proven depth, so the
   parent-derived bound is a sufficient bound by construction; a cap below
   it can only produce false failures. No config default changes.
2. **D2 — doubling clamps instead of overshooting.** The bound advances by
   `min(2*bound, cap)` through a small pure helper
   (`fn next_fill_bound(bound: u32, cap: u32) -> u32`, unit-testable
   without a drawn fixture), and the walk aborts only after an attempt
   **at** `cap` returned a genuine exhaustion. Consequently a hole with
   `parent_depth > cap` gets exactly one attempt (at its true bound), and a
   shallow root hole keeps today's 8/16/32 ladder but reaches the cap
   exactly (8 → 16 → 20 for `cap = 20`, where today's code jumps to 32 and
   gives up after two attempts).
3. **D3 — a resource cut is not a depth result.** After a `Draw` attempt,
   read `Search::exit_reason()` (`src/search/dfpn/config.rs:252`);
   `ExitReason::Complete` alone escalates the bound. `BudgetExhausted`
   (the only other reachable one: the fill's timeout is 100 years and the
   fill `Search` carries no stop/memory flag) aborts with a distinct
   `BudgetExhausted: fill at <path> censored at bound B (attempt budget A
   evals, parent depth P)` error; any other reason aborts naming the
   reason. `stats.unfillable` is **not** incremented by a resource cut —
   it keeps its meaning "no decisive line within the cap", so
   `holes_line()`'s grammar (pinned in the manual) is unchanged.
4. **D4 — the harvest gains exactly the two actionable knobs**
   (`--fill-attempt-budget`, default `2_000_000`; `--fill-total-budget`,
   default `100_000_000`) — today's hardcoded values, so a default
   invocation is behaviorally identical. `--fill-base`/`--fill-depth-cap`
   are *not* exposed: after D1 the cap only bounds root holes, and a root
   hole already fails earlier with "snapshot does not prove a decisive
   root".
5. **D5 — the rejection line carries the census.** `shard_export.rs`
   appends the walk census and the fill accounting to the rejection message
   (`<error> (holes: …; fill_evals=N)`), so the fatal abort line is
   diagnosable after the fact without a rerun. No grammar change to the
   `job:`/`holes:` lines themselves.
6. **D6 — option A's operator story is pre-registered, not improvised.**
   `docs/proofdb_pipeline.md` §10 gains, verbatim as a rule: the two abort
   classes with their triage, the recovery ladder (fold-then-retry ⇒
   `--tt-mb` bump ⇒ censor-row quarantine ⇒ `--moves` re-scope), and the
   **three-strike rule** — the same path aborting a second time is
   quarantined and the session retried with a larger `--tt-mb`; the third
   *distinct* path aborting the same way stops the campaign and becomes a
   report, not another retry. Quarantine is a manual `INSERT OR REPLACE
   INTO censor(path, work_done, passes_failed)` with `work_done` = the
   session budget and `passes_failed = 0`, which is within the contract:
   `censor`/`drawn` rows are "replay-independent scheduling hints…
   deleting them costs re-searched work, never soundness"
   (`docs/spec/global_proof_store.md` §2.1).
7. **D7 — no new scheduling state, no automatic escalation.** The harvest
   does not auto-quarantine, does not retry the export with a bigger budget
   inside the session, and writes no new rows. The abort stays fatal and
   deterministic; recovery stays an operator action (option A).
8. **D8 — no product-solver or proof-tree changes.** `search/`,
   `proof_tree/`, the shard/dump format, the DB schema, and the
   reconstruction's soundness surface are untouched. The tree recomputes
   depths in `finalize()` (`src/proof_tree/worker.rs:549`), so the walker's
   advisory filled-Loss depth (`= bound`) remains harmless and is left as
   is (a `// advisory` comment only).

## 2. Implementation tasks

1. **Fill loop** — `src/reconstruct/walker.rs` (`fill_hole`, lines
   267-320).
   - D1: compute `start` then `cap = self.config.fill_depth_cap.max(start)`;
     drop the pre-loop `bound > cap` check in favour of the in-loop rule of
     D2.
   - D2: add the private helper `next_fill_bound(bound, cap) -> u32`
     (`bound.saturating_mul(2).min(cap)`) with a one-line doc comment ("the
     doubling search may not skip a bound between the current one and the
     cap"), used at the single advance site.
   - D3: match the attempt's `Draw` on `self.fill.exit_reason()`:
     `Complete` ⇒ clamp-double (D2) or unfillable at `cap`;
     `BudgetExhausted` ⇒ `"BudgetExhausted: fill at {path} censored at
     bound {bound} (attempt budget {A} evals, parent depth {P})"`;
     other ⇒ `"fill at {path} aborted by {reason} at bound {bound} (…)"`.
   - Unfillable message: `"unfillable hole at {path}: no decisive line
     within bound {bound} (cap {cap}, parent depth {P})"` — keeps the
     `unfillable hole at <path>` prefix operators grep for.
   - Keep the existing `fill_total_budget` pre-check and its
     `"BudgetExhausted: fill total budget exhausted"` message unchanged.
   - `parent_depth` rendering: `"none"` for the root hole.

2. **Config docs** — `src/reconstruct/mod.rs`
   (`ReconstructConfig::fill_depth_cap`, line 163-164, plus the struct's
   `fill_base` doc): restate the field as "depth ceiling for **root**
   holes; non-root holes are bounded by their parent's proven depth and
   are never capped below it (D1)". Same wording in
   `examples/reconstruct_pt.rs`'s `--fill-depth-cap` handling comment.

3. **Harvest knobs** — `examples/proofdb/harvest_args.rs` +
   `examples/proofdb/session.rs`.
   - `Args` gains `fill_attempt_budget: u64 = 2_000_000` and
     `fill_total_budget: u64 = 100_000_000`; parse `--fill-attempt-budget`
     / `--fill-total-budget`; usage text lists both with their defaults.
   - `Session::new` takes the two values (a small
     `ExportBudgets { attempt, total }` struct is acceptable) and builds
     `ReconstructConfig` from them; `fill_base: 8` and `fill_depth_cap: 32`
     stay as they are. No other session field changes. The call site in
     `examples/proofdb_harvest.rs` (`Session::new(...)`) passes
     `args.fill_attempt_budget` / `args.fill_total_budget`.
   - `tests/proofdb_harvest_cli.rs`: assert the two defaults (an absent
     flag leaves the harvest's export config at 2 M / 100 M) and that the
     flags parse; extend the usage-text pin if the file has one.

4. **Rejection diagnostics** — `examples/proofdb/shard_export.rs`.
   - On a reconstruction failure (both the `root_outcome.is_none()` branch,
     line 44, and the `tree.ok_or_else` branch, line 49), append
     `format!("({}, fill_evals={})", out.stats.holes_line(),
     out.fill_evals)` to the returned error. The validator branch keeps its
     existing defect-count message.

5. **Fast-tier unit tests** — `src/reconstruct/tests.rs`.
   - `deep_parent_bound_is_not_capped_by_the_fill_depth_cap`: build the
     `MATE3_FEN` snapshot via a live search, drop the *root's best-move
     child* record, and set the root record's depth to
     `fill_depth_cap + 1` (33). Assert `error.is_none()`, `filled == 1`,
     `unfillable == 0`, tree present. (Fake depths are safe here: the walk
     uses them only as bounds and `finalize` recomputes.)
   - `censored_fill_attempt_reports_budget_exhaustion`: same fixture with
     `fill_attempt_budget: 1` and the root record dropped; assert the error
     starts with `BudgetExhausted:` and `stats.unfillable == 0`.
   - `fill_bound_doubling_clamps_at_the_cap` (pure helper, no search):
     `next_fill_bound(8, 20) == 16`, `(16, 20) == 20`, `(20, 20) == 20`,
     `(1, 1) == 1`, `(u32::MAX, u32::MAX) == u32::MAX`. (The clamp cannot be
     observed through `fill_evals` in a `Complete`-draw fixture: every
     realistic fixture either proves a decisive line on the first attempt
     or is not cheaply `Complete`. The helper is the seam that makes D2
     directly testable; the loop's use of it is covered by the two
     behaviour tests above.)
   - Update `fill_recovers_a_dropped_root_record` only if its message
     assertions touch the changed text (it asserts on `error.is_none()`,
     so it should pass unchanged — verify).

6. **Slow-tier incident pin** — `tests/reconstruct.rs`.
   `#[ignore = "slow: deep-proof child hole (reported harvest abort,
   ~64M child evals)"]`
   `deep_child_hole_exports_with_the_default_fill_cap`:
   - `Search::new(128)`, `set_timeout(3600)`, `set_first_outcome_only(true)`,
     `set_child_eval_budget(100_000_000)` (the production
     `proofdb_cycle_run` budget; the cold run peaks at 63,954,107 evals, so
     100 M leaves headroom), `search_depth_with_prefix(&mut pos,
     u32::MAX, &[])` — the job FEN above, no prefix (the job is 9 plies
     from the startpos with clock 0).
   - Assert `outcome == Outcome::Win`; read the root record from the
     snapshot, apply its `best_move`, drop that child's key from the solved
     list, and assert `root.depth > ReconstructConfig::default()
     .fill_depth_cap` (the regression precondition — a future root that is
     shallow must fail this assertion loudly rather than silently stop
     testing D1).
   - `reconstruct(job_fen, &solved, &ReconstructConfig::default())` ⇒
     `error.is_none()`, `filled >= 1`, and
     `validate_proof_tree(&tree)` clean (add
     `use atomic_solver::proof_tree::validate_proof_tree;` to the file's
     imports).
   - Provenance to record in the report: with the pre-fix walker this test
     fails with `unfillable hole at root.e7e5: fill depth cap 32
     exhausted` and `fill_evals == 0`; post-fix it fills for ~7 evals.

7. **Documentation**
   - `docs/proofdb_pipeline.md` §7 (the harvest flag table) gains the two
     new flags with their defaults; §10: the two abort classes (depth/
     contradiction vs budget), the recovery ladder, the three-strike rule
     including the quarantine SQL, `--tt-mb` as the eviction lever, the
     measured note that raising the *attempt* budget can cost up to ~9×
     more evals for the same holes (restart-chunk pacing: 20 M attempts
     spent 60 M evals where 2 M attempts spent 6.7 M), and the fact that a
     decisive-but-unexported job leaves no record (so the same path is
     re-selected next session).
   - `AGENTS.md`: `proofdb_harvest` bullet gains the two fill flags; the
     `reconstruct`/`walk` bullet gains one sentence on the parent-derived
     bound rule.
   - `docs/plans/proofdb/initiative.md`: item 22 row (open → done at report
     time) and a history line. `docs/plans/README.md` is **not** updated
     (no open/pivot/close).

8. **Report** — the final task (§7).

## 3. Acceptance criteria

- A1 — the reported incident is a passing test (task 6) and its
  documented pre-fix failure mode is the reported message with
  `fill_evals=0`.
- A2 — no behavior change for shallow snapshots or default configs: every
  existing `reconstruct`/`proofdb` test passes **unmodified** (no golden or
  grammar re-baseline), i.e. `make test` green.
- A3 — a budget-censored fill reports `BudgetExhausted: … censored at
  bound …` and does **not** increment `stats.unfillable`; `holes_line()`'s
  format is byte-identical.
- A4 — `next_fill_bound` clamps at the cap (`8→16`, `16→20` for cap 20,
  `20→20`, no overflow at `u32::MAX`).
- A5 — `proofdb_harvest --fill-attempt-budget N --fill-total-budget M`
  parse, appear in the usage text, and reach `ReconstructConfig`; the
  default invocation is behaviorally identical to today.
- A6 — a rejected export's abort line contains `holes: …` and
  `fill_evals=…`.
- A7 — manual §10 documents both abort classes, the recovery ladder and
  the three-strike rule with the quarantine SQL; item 22 and the history
  line exist; `report25.md` written.

## 4. Test plan

```bash
cargo fmt --all
cargo clippy --all-targets
cargo test --lib reconstruct
cargo test --test reconstruct            # fast tier
cargo test --test reconstruct -- --include-ignored   # + the incident pin (~10 s)
cargo test --test proofdb_harvest_cli
make test
make test-full                            # the slow tier: required by A1/A2 here
cargo doc --no-deps
```

`make test-full` is required: the change is in the offline reconstruction
builder and its regression pin is a slow-tier test. No search, move-ordering,
TT/GHI, or proof-tree code changes, so no golden re-baseline is expected;
if one appears, that is a finding for the report, not an edit.

Manual confirmation (optional, and the operator's own recovery rehearsal):

```bash
cargo build --release --examples
BIN=target/release/examples
$BIN/proofdb_merge && $BIN/proofdb_flip              # after any aborted batch
$BIN/proofdb_harvest --policy frontier-walk --budget-evals 100000000 \
  --moves "g1f3 d7d6 b1c3 c7c6 f3g5 f7f6"
```

## 5. Risks and limitations

- **The parent-derived bound can be deep** (up to the job's proven depth,
  ≈47 plies here). The work is still bounded: child-eval budgets only,
  deterministic, and the seeded snapshot makes filled holes cheap in
  practice (7 evals for the incident hole; 6.3–8.9 M total per export under
  a heavily-evicted snapshot). A pathological hole can still hit the
  attempt/total budget — that is now a *named* abort (D3) with an operator
  knob (D4).
- **The 9× attempt-budget trap.** A larger `--fill-attempt-budget` can make
  an export *more* expensive (restart-chunk pacing). It is documented as
  the escalation of last resort, only after a `BudgetExhausted` abort, and
  `--fill-total-budget` remains the guard that keeps an export from costing
  more than the search it exports.
- **Option A stays a wedge risk.** A deterministic deep-hole-without-budget
  failure re-selects the same path every session (a decisive-but-unexported
  job leaves no record). The three-strike rule (D6) plus the quarantine
  recipe is the mitigation; item 23 (skip + censor on export rejection) is
  the successor to draft if the rule ever fires. This plan deliberately
  does not pre-empt it.
- **A `Complete`-draw fill at the parent's bound is a contradiction, not a
  resource limit** (the parent's record claims the child is decisive at a
  smaller depth). It is reported as `unfillable … no decisive line within
  bound B (cap B, parent depth P)`; if it ever appears in production, the
  likely cause is a `ClockHit` adoption or a repetition-context mismatch and
  it belongs in a report, not a patch.
- **`fill_evals` accounting** stays per-run cumulative (`fill_evals +=
  child_evaluations()` per attempt); the new messages report the *attempt*
  budget, not the run's spend — the census in D5 covers the run's total.

## 6. Out of scope

- Item 23 (option B): treating a budget-censored/unfillable export as a
  logged skip + censor row instead of a fatal abort; auto-quarantine; any
  new scheduling state or verdict/grammar change.
- Escalating the *attempt* budget automatically on a censored fill, or
  re-running the job's search against a fresh TT before giving up.
- Raising `ReconstructConfig::fill_depth_cap`'s default, or exposing
  `--fill-base`/`--fill-depth-cap` in the harvest.
- Any change to `search/`, `proof_tree/`, the shard/dump format, the DB
  schema, `proofdb_merge`/`proofdb_verify`/`proofdb_flip`, the walk policy,
  budgets, or frontier selection.
- Item 3 (DTM-upgrade pass) and any depth-quality work.

## 7. Final task

Write `report25.md`: what shipped per task, the exact gate result
(`make test` + `make test-full`), the incident pin's pre-fix vs post-fix
evidence (message, `fill_evals`), any deviation from the plan, findings
(including anything the fast tier could not cover), and the next steps
(operator rehearsal of the three-strike rule; item 23 only if the rule
fires). Update `docs/plans/proofdb/initiative.md`: item 22 row → done with
the plan↔item mapping and a history line. Do not touch
`docs/plans/README.md`.
