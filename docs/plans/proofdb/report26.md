# report26 — Flip analysis OR-witness semantics (execution of `plan26.md`)

**Initiative:** `proofdb` · **Executes:** backlog item 24 · **Date:**
2026-10-27 · **Status:** done (read-only, examples-only; no `src/`, schema,
spec, shard or DB-format change).

## 1. What shipped, per plan task

| task | result |
| --- | --- |
| 1. `DbRow.depth_status` | `DbRow` gained `pub depth_status: Option<DepthStatus>`; `load_db_rows`'s `SELECT` reads `depth_status` and maps `NULL`/`'exact'`/`'bound'`, aborting on any other value (`nodes row {id}: unknown depth_status {v:?}`). Three `DbRow { … }` literals updated (`examples/proofdb/walk/tests.rs`, `tests/proofdb.rs` ×2). |
| 2. `examples/proofdb/flip/mod.rs` | Types (`Flip`, `Coverage`, `BoundMode`, `Claim`), the `Analysis` state, the replay helper, the accessors, and the normative module header (D1/D2/D3 text incl. the plan8 §4.5 supersession and the root-claim caveat). Root FEN now comes from the DB (`Analysis::new(rows, root_fen)`), not the `Position::STARTPOS_FEN` constant. |
| 3. `examples/proofdb/flip/verify.rs` | Independent recursive verifier: fresh replay + movegen per claim, the D1 asymmetry (loss claims demand and-complete coverage and name the offending reply; win claims recurse only into their loss witnesses), the three named error strings, and full `Claim` comparison (value **and** metadata). `verify_all()` returns the verified count. |
| 4. `examples/proofdb_flip.rs` | Shrunk to CLI/gate/load/report: the D2 stdout line, the additive JSON keys, the root note, unchanged usage text and exit codes (`0` including with flips, `1` on usage/load/gate/verification). |
| 5. `examples/proofdb/mod.rs` | `pub mod flip;` — the `#[path]` harnesses compile the unit fixtures into the fast gate. |
| 6. Red-first CLI fixtures | `tests/proofdb_flip.rs` written and run **before** any tool change; both failed with the predicted messages (§3). |
| 7. Unit fixtures | `examples/proofdb/flip/tests.rs`: the three D4 tests, with a `row(...)` helper and movegen-driven "declare every legal reply" helpers (the startpos's 20 replies are generated, never typed). |
| 8. Docs | `docs/proofdb_pipeline.md` §2.2, §3.2 (batch cycle), §7.3 (full rewrite), R3, §11 history entry; `AGENTS.md`'s `proofdb_flip` bullet; the module header. `docs/spec/global_proof_store.md` and `plan8.md` untouched (D6/D7). |
| 9. Measurements | `docs/plans/proofdb/measurements/plan26/README.md` (probe re-run, red capture, post-fix JSON, file sizes). No transcripts committed. |
| 10. Report + index | this file; `initiative.md` item 24 row → done with the plan↔item mapping plus a history line. `docs/plans/README.md` untouched (no open/pivot/close). |

## 2. Gate results

```text
cargo fmt --all --check                        clean
cargo clippy --all-targets                     0 warnings, 0 errors
cargo doc --no-deps                            clean
cargo test --release proofdb                   ok (unit fixtures visible as proofdb::flip::tests::*)
cargo test --release --test proofdb_flip       ok, 60 passed (incl. the two CLI fixtures)
make test                                      ok, 37 test binaries green, ~31 s of test time
git diff --stat src/                           empty
```

`make test-full` was not run (no search/move-ordering/TT/proof-tree/
reconstruction change — AGENTS.md's trigger list; the plan agrees).

The new CLI test costs ~3.5 s of the fast gate (two `cargo run --example`
invocations under the run lock) — the same shape as the existing
`proofdb_harvest_cli`/`proofdb_merge_cli` harnesses.

## 3. D4 evidence, both polarities

**Pre-fix (the unmodified binary, `cargo test --release --test proofdb_flip`
on the pre-change tree):** both CLI fixtures exit 1.

```text
or_witness_flip_reports_coverage_and_bound:
  proofdb_flip: FLIP VERIFICATION FAILED: flip "g1f3 d7d6 b1c3 c7c6 f3g5 f7f6 c3b5 f6f5": reply a2a3 has no DB row
or_bound_is_recomputed_to_the_fixpoint:
  proofdb_flip: FLIP VERIFICATION FAILED: flip "": want Flip { stm_win: true, bound: 11 }, verified Flip { stm_win: true, bound: 4 }
```

The first is the production incident verbatim (item 24); the second is the
latent derive-once divergence. Both match the plan's prediction exactly.

**Post-fix:** both pass with the asserted shapes —

- fixture 1: exit 0, `flip_analysis: open_rows 9 flips 1 verified 1 root Null
  (fixpoint 2 rounds; or-witness 1, and-complete 0, exact 0)`, JSON flip
  `{ply 8, implied "root-win", side_to_move "win", bound 23, coverage
  "or-witness", bound_mode "upper", loss_witnesses 1, unresolved_replies 29}`.
- fixture 2: exit 0, `flip_analysis: open_rows 2 flips 2 verified 2 root
  root-win (fixpoint 3 rounds; or-witness 0, and-complete 2, exact 0)`, root
  flip `{bound 4, coverage "and-complete", bound_mode "upper",
  loss_witnesses 2, unresolved_replies 0}` — the converged bound, not the
  round-1 `11`.

The unit fixtures (the lasting pins, in the default gate) cover the same
asymmetry at the API level and additionally pin the derive-once contrast
directly: `classify("", &HashMap::new())` yields bound `11`, the converged
claim `4`; the loss-claim fixture pins all three defect strings (`has no DB
row`, `unresolved`, `no loss witness` — the last one tested defensively, since
the fixpoint never emits a witness-less win claim) plus the `exact`/`upper`
split under a `bound`-status contributor.

## 4. §0.4 probe table re-run after the change

| layer | nodes | open rows | post-fix stdout | exit |
| --- | --- | --- | --- | --- |
| standing `data/proofdb/proofdb.db` | 43,092 | 64 | `flip_analysis: open_rows 64 flips 0 verified 0 root Null (fixpoint 1 rounds; or-witness 0, and-complete 0, exact 0)` | 0 |
| committed fixture fold (`docs/plans/proofdb/shards`, digest `14fd0753…`) | 55,703 | 75 | `flip_analysis: open_rows 75 flips 0 verified 0 root Null (fixpoint 1 rounds; or-witness 0, and-complete 0, exact 0)` | 0 |

Zero-impact confirmed (A5): every pre-existing JSON key is unchanged on both
layers; the only delta is the additive counters and the stdout suffix.
`list_legal` re-confirms 30 legal replies from the cited ply-8 FEN with
`a2a3` first, so the fixture reproduces the production message.

## 5. File sizes before/after the split (A7)

| file | before | after |
| --- | --- | --- |
| `examples/proofdb_flip.rs` | 13,322 | 5,391 |
| `examples/proofdb/flip/mod.rs` | — | 7,949 |
| `examples/proofdb/flip/fixpoint.rs` | — | 7,051 |
| `examples/proofdb/flip/verify.rs` | — | 8,766 |
| `examples/proofdb/flip/tests.rs` | — | 7,663 |
| `tests/proofdb_flip.rs` | — | 8,963 |

Every touched source file is under the 10 KB convention; the tool's 12 KB
size-justification header is retired.

## 6. Deviations from the plan

1. **An extra split: `flip/fixpoint.rs`.** D5 planned three files under
   `flip/`; `mod.rs` measured 13.9 KB with the normative header, the
   analysis state *and* the converged pass. The converged pass, the reply
   resolution, and the `Resolved`/`ReplyStatus` types moved to
   `flip/fixpoint.rs` (7.1 KB), leaving `mod.rs` at 7.9 KB. Placement is
   codegen-neutral (examples only); it is the same "split by change reason"
   reasoning the `dfpn` split used.
2. **The root word in the stdout line is now formatted explicitly.** The
   pre-existing code printed `{:?}` of a `serde_json::Value`, which renders
   `Null` for an undecided root but `String("root-win")` for a decided one.
   The pinned grammar (`root Null`) is unchanged; a decided root now prints
   `root root-win`. No other output changed.
3. **CLI fixture 2 reports `flips 2`, not the plan's `verified 1`.** The
   plan's fixture text counted only the root flip; the open child is itself a
   converged and-complete loss claim (all 20 of its replies are resolved
   wins), so it is a legitimate second flip. The test asserts the root
   entry's fields plus `flips_verified_independently == flips_total`; the
   report records the actual numbers.
4. **The D2 exactness refinement ripened into the shipped rule** (already a
   plan decision): `and-complete` yields `bound_mode = exact` only when every
   contributing child bound is exact. Its ripple is exactly the planned one
   (`DbRow.depth_status` + three test literals), and no live layer changes.
5. **Fixpoint round semantics are now explicit in the output.** A layer with
   no claims still prints `fixpoint 1 rounds`; a layer whose claims appear in
   the first pass prints `2` (derive, then confirm the map is unchanged);
   fixture 2 prints `3` (root bound tightens on pass 2, confirmed on pass 3).
   The manual documents `fixpoint R rounds` as "passes until the claim map
   stopped changing", which is now literally true.

## 7. Findings

- **The root-claim question is open (plan §5, unchanged).** An `or-witness`
  root claim is a *sound* startpos outcome claim — the same structure
  harvest or-close relies on — while only its bound is an upper bound. The
  initiative's handover rule ("all ~41 children resolved") stays
  conservative; whether an or-witness root claim should trigger handover is
  a follow-up decision, not this plan's.
- **The fixpoint costs one replay per open row per pass.** Both live layers
  have ≤ 75 open rows (a pass is invisible in wall clock); a DB with 10⁵ open
  rows would want an incremental version. Recorded, not designed.
- **`bound_mode = exact` is currently unreachable in practice.** Every
  non-terminal proven row in the layers we have is `depth_status = 'bound'`
  (22,808 of 43,092), so `exact` would require a layer whose contributing
  children are `exact`-claimed. The code path is pinned by unit fixture 3
  (all-exact children ⇒ `exact`), so the vocabulary is not dead — it is
  simply honest about today's layers.
- **The verifier's recursion is bounded at 200 claims deep** (unchanged from
  the pre-plan8-era code); a claim chain that long in a real layer would be a
  defect, but the guard is a magic number, now named `MAX_DEPTH`.
- **`Analysis::rounds()` and the monotonicity tripwires are cheap.** The
  monotone checks run over existing claims only (≤ open rows), so the
  recompute-to-fixpoint cost stays where §5 says it is.

## 8. Next steps

1. Decide the handover-trigger question: does an `or-witness` root claim
   suffice for handover, or does the rule keep requiring and-complete
   coverage? (Initiative-level decision; would touch the handover text, not
   this tool.)
2. Harvest further layers and re-read R3 on a layer that actually grows an
   `or-witness` claim, to confirm the operator reading end-to-end (no live
   layer exercises it yet, as §0.4 measured).
3. If the flip report becomes a routine ingestion input, consider the
   materialize-as-proven-rows item (item 12) — explicitly out of scope here.

## 9. Tools/examples used

- `target/release/examples/proofdb_flip` (the tool under test), `proofdb_merge`
  (the fixture fold), `list_legal` (the legal-reply probes).
- `python3` for the mechanical doc edits and the three `DbRow` literal
  updates; `cargo fmt`, `cargo clippy --all-targets`, `cargo doc`,
  `cargo test --release`, `make test`.
- `TMPDIR` redirection of the CLI test so its synthetic DBs and JSON reports
  could be quoted in `measurements/plan26/README.md`.

## 10. Missing tests

- No test pins the non-convergence tripwire (`fixpoint did not converge in N
  passes`) or the monotonicity errors; both are unreachable by construction
  on a consistent row set (each pass only adds resolved statuses), so a
  fixture would have to fake a state machine defect. Recorded, not chased.
- No test asserts the `ROOT implied …` stderr note (unchanged behaviour,
  informational).
- The `fixpoint_round` value is only pinned indirectly (the unit test's
  `rounds()` assertion); a claim whose metadata changes after discovery keeps
  its discovery round by design, which no fixture exercises.
