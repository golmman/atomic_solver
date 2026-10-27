# report25 — Deep-hole export resilience (option A: the fatal abort stays, made rare, diagnosed, recoverable)

**Initiative:** `proofdb` · **Executes:** item 22 (plan25) · **Session date:**
2026-10-27 · **Gate:** `make test` green, `make test-full` green (588 tests
passed, 0 failed, 36 test binaries, 8m12s wall on the reference host).

## 1. What shipped

| Plan task | Delivered | Where |
| --------- | --------- | ----- |
| 1. fill loop (D1/D2/D3) | `fill_hole` seeds `start = parent_depth.unwrap_or(fill_base).max(1)`, sets `cap = fill_depth_cap.max(start)` and drops the pre-loop `bound > cap` check; the loop advances via `next_fill_bound(bound, cap) = bound.saturating_mul(2).min(cap)` and stops only after an attempt **at** `cap` returned a `Complete` draw; a `Draw` attempt is classified by `Search::exit_reason()` (`Complete` escalates, `BudgetExhausted` aborts as `BudgetExhausted: fill at <path> censored at bound B (attempt budget A evals, parent depth P)`, anything else as `fill at <path> aborted by <reason> …`); the unfillable message is now `unfillable hole at <path>: no decisive line within bound B (cap C, parent depth P)`; the `fill_total_budget` pre-check and its message are unchanged; `parent_depth` renders as `none` for the root hole | `src/reconstruct/walker.rs` |
| 2. config docs | `ReconstructConfig::fill_depth_cap` doc restated as a *root-hole* ceiling (non-root holes are bounded by their parent's proven depth and never capped below it); `fill_base` described as the parentless-hole base; the same statement as a doc comment on `examples/reconstruct_pt.rs`'s `--fill-depth-cap` field | `src/reconstruct/mod.rs`, `examples/reconstruct_pt.rs` |
| 3. harvest knobs (D4) | `--fill-attempt-budget` (default `2_000_000`) and `--fill-total-budget` (default `100_000_000`) parsed into `Args` via the new `DEFAULT_FILL_ATTEMPT_BUDGET` / `DEFAULT_FILL_TOTAL_BUDGET` consts, listed in the usage text; a new `ExportBudgets { attempt, total }` (+ `Default`, `config(tt_mb) -> ReconstructConfig`) feeds `Session` through the new `Session::new_with_export_budgets`; `fill_base: 8` / `fill_depth_cap: 32` unchanged | `examples/proofdb/harvest_args.rs`, `examples/proofdb/session.rs`, `examples/proofdb_harvest.rs` |
| 4. rejection diagnostics (D5) | both reconstruction-failure branches (`root_outcome.is_none()` and the missing-tree branch) append `(<holes_line>, fill_evals=N)` to the message; the validator branch keeps its defect-count message | `examples/proofdb/shard_export.rs` |
| 5. fast-tier unit tests | `deep_parent_bound_is_not_capped_by_the_fill_depth_cap`, `censored_fill_attempt_reports_budget_exhaustion`, `fill_bound_doubling_clamps_at_the_cap` (+ the `snapshot_with_child_hole` helper); `fill_recovers_a_dropped_root_record` verified unchanged | `src/reconstruct/tests.rs` |
| 6. slow-tier incident pin | `deep_child_hole_exports_with_the_default_fill_cap`, `#[ignore = "slow: deep-proof child hole (reported harvest abort, ~64M child evals)"]` | `tests/reconstruct.rs` |
| 7. documentation | manual §7.2 usage block + flag table (two new rows, `--tt-mb` marked as the eviction lever), §10 subsection "A rejected export aborts the session" (two abort classes, five-rung recovery ladder, three-strike quarantine rule with the quarantine SQL, the ~9× attempt-budget trap, the decisive-but-unexported-leaves-no-record note), §11 history entry; `AGENTS.md` (`proofdb_harvest` bullet gains the two fill flags and the abort-line census; the `src/proof_tree/` bullet gains the parent-derived-bound sentence) | `docs/proofdb_pipeline.md`, `AGENTS.md` |
| 8. report + index | this file; `initiative.md` item 22 row → done with the plan↔item mapping + history line (`docs/plans/README.md` untouched, as instructed) | `docs/plans/proofdb/` |

No changes to `search/`, `proof_tree/`, the shard/dump format, the DB schema,
`proofdb_merge`/`proofdb_verify`/`proofdb_flip`, the walk policy, budgets,
frontier selection, or `stats.unfillable`'s meaning.

## 2. Evidence

### 2.1 The incident pin (A1) — pre-fix vs post-fix, measured

Both runs use the reported job FEN
`rnbqkbnr/pp2p1pp/2pp4/8/5PP1/2N5/PPPPP2P/R1BQKB1R b KQkq f3 0 5`, a live
`Search::new(128)` at a 100 M child-eval budget (2 chunks, peak
`work_done=32,000,026` in this run), snapshot taken afterwards, the root's
best-move child (`e7e5`) record dropped, root depth asserted `> 32`.

| | pre-fix walker | post-fix walker |
| - | - | - |
| `error` | `unfillable hole at root.e7e5: fill depth cap 32 exhausted` | `None` |
| `fill_evals` | **0** (bound 46 > cap 32 ⇒ rejected before any search) | **29** |
| stats | `hit=1 absent=1 unfillable=1 filled=0` | `hit=8877 terminal=5509 clock_hit=1 absent=7 filled=7 unfillable=0 anomalies=0` |
| tree | none (walk aborted) | exported, `validate_proof_tree` clean |
| pin runtime | 9.5 s | 10.2 s |

The pre-fix line reproduces the production message verbatim (the incident
reported `root.e7e5.a2a3` because its snapshot also lost that mate-in-1
record; this fixture loses only the `e7e5` record, so the abort names that
node). `fill_evals == 0` confirms the plan's root cause: the old code never
searched.

The post-fix run also shows the D1 chain in action: the walk needed 7 holes
(not 1) on its way through the tree — 29 child evals total, versus zero
work but a dead session before.

### 2.2 The fast-tier unit tests (A3/A4)

Pre-fix, both new behaviour tests fail with the *misclassification* the plan
predicted:

- `deep_parent_bound_is_not_capped_by_the_fill_depth_cap` →
  `fill failed: Some("unfillable hole at root.f1f7: fill depth cap 32 exhausted")`.
- `censored_fill_attempt_reports_budget_exhaustion` →
  `unexpected: unfillable hole at root: fill depth cap 32 exhausted` — an
  attempt-budget censorship reported as depth exhaustion, exactly defect D3.

Post-fix the censored-fill message is
`BudgetExhausted: fill at root censored at bound 8 (attempt budget 1 evals, parent depth none)`
with `fill_evals=19` (restart-chunk overshoot) and `stats.unfillable == 0`.

The clamp test pins `next_fill_bound`: `8→16`, `16→20`, `20→20` (cap 20),
`1→1`, no overflow at `u32::MAX`.

### 2.3 The harvest knobs (A5)

`tests/proofdb.rs::export_budgets_reach_the_reconstruct_config` pins the
defaults (2 M / 100 M), the fixed base/cap (8 / 32), and the override path
(`attempt: 20 M, total: 0` → exactly those two config fields);
`harvest_args`' unit tests (which run inside `tests/proofdb.rs`) pin the
defaults on `Args` and the two flags' parsing. The built binary's usage text
lists both flags with their defaults (checked by hand:
`target/release/examples/proofdb_harvest --bogus`).

### 2.4 The rejection census (A6)

`tests/proofdb.rs::export_rejection_carries_the_walk_census` produces a real
rejection (empty search TT, one-eval attempt budget) and pins the message
shape; the observed line:

```text
BudgetExhausted: fill at root censored at bound 8 (attempt budget 1 evals, parent depth none) (holes: hit=0 terminal=0 clock_hit=0 clock_miss_draw=0 repetition=0 absent=1 filled=0 unfillable=0 anomalies=0, fill_evals=19)
```

which the session prints as
`proofdb_harvest: ABORT: job <path>: export rejected: <that line>`.

## 3. Gate

```text
cargo fmt --all                       clean
cargo clippy --all-targets            clean
cargo doc --no-deps                   clean
cargo test --lib reconstruct          14 passed
cargo test --test reconstruct         2 passed, 2 ignored (fast tier)
cargo test --test reconstruct -- --include-ignored
                                      4 passed (incl. the incident pin, 10.1 s)
cargo test --test proofdb             75 passed
cargo test --test proofdb_harvest_cli all passed
make test                             green, no ignored-test re-baseline
make test-full                        green: 588 passed, 0 failed, 36 binaries, 8m12s
```

No golden or grammar re-baseline was needed (A2): every existing
`reconstruct`/`proofdb` test passes unmodified.

## 4. Deviations from the plan

1. **`Session::new` keeps its four-argument signature.** The plan said
   `Session::new` should take the two budget values. Instead the budgets go
   through a new `Session::new_with_export_budgets(…, ExportBudgets)` and
   `Session::new` delegates to it with `ExportBudgets::default()`. Reason:
   A2 asks that the existing tests pass unmodified, and 16 existing call
   sites (12 in `tests/proofdb.rs`, 4 in `walk/tests.rs`) would otherwise
   have needed a mechanical edit. The default path is the same
   `ReconstructConfig` as before, byte for byte
   (`export_budgets_reach_the_reconstruct_config` pins it).
2. **The knob unit tests live in `harvest_args.rs`, not
   `tests/proofdb_harvest_cli.rs`.** `parse_args` reads `std::env::args`, so
   `parse_args` was split into `parse_args()` → `parse_from(iter)`, and the
   assertions sit next to the existing `moves_*` unit tests in the same
   module — which `tests/proofdb.rs` already compiles via
   `#[path = "../examples/proofdb/mod.rs"]`, so they run in the gate. No
   usage-text pin exists in `proofdb_harvest_cli.rs`, so nothing was
   extended there. The `ReconstructConfig` wiring assertion went to
   `tests/proofdb.rs` (where the session module is compiled).
3. **The censored-fill fixture is an empty snapshot, not "the MATE3 snapshot
   with the root dropped" (budget 1).** Measured: the MATE3 root fill costs
   3 child evals and one restart chunk completes it even at
   `fill_attempt_budget: 1`, and *any* seeded child hole costs ≤ 1 eval — a
   consensus fixture cannot be censored at a realistic budget. An empty
   snapshot makes the root the only hole and censors reliably at budget 1
   with the same message shape. The pre-fix behaviour of that test is
   exactly the D3 misclassification (see §2.2), so the seam still pins the
   decision.
4. **Two extra tests beyond the plan's list**:
   `export_budgets_reach_the_reconstruct_config` and
   `export_rejection_carries_the_walk_census` (A5/A6 were otherwise not
   covered by any executable assertion).

## 5. Findings

- **The incident fixture needs 7 fills, not 1.** Post-fix the export of the
  dropped-record snapshot needed 7 holes (8877 hits, 5509 terminals) for 29
  evals. The plan's provenance note ("post-fix it fills for ~7 evals") was
  measured on a different drop (only the mate-in-1 record, with the cap
  raised to 200). The order of magnitude holds; the exact number is
  fixture-dependent.
- **`Fill` attempt budgets are chunk-granular.** `exit_reason()`'s
  `child_eval_budget_exceeded` is `child_evals >= budget`, but the budget is
  only consulted between refinement chunks, so a one-eval attempt spent 19
  evals. This is pre-existing behaviour of the bounded search, not new; it
  means a small `--fill-attempt-budget` cannot be used as a hard cap, and it
  is why the recovery ladder treats the attempt budget as a coarse knob.
- **A `Complete`-draw fill at the parent-derived bound is still reported as
  a depth/contradiction abort** (`unfillable hole … no decisive line within
  bound B (cap B, parent depth P)`) — as the plan's risk section predicted.
  No such abort appeared in any test or smoke run in this session.
- **`docs/notes.md` carries an open operator question** (not mine, present in
  the working tree): after a merge, `proofdb_flip` reports
  `FLIP VERIFICATION FAILED: flip "g1f3 d7d6 b1c3 c7c6 f3g5 f7f6 c3b5 f6f5": reply a2a3 has no DB row`.
  That is a coverage/soundness statement over stored rows, unrelated to the
  export path this plan touched — it belongs to a `proofdb` verification
  session, not to plan25. Flagged here so it is not lost.

## 6. Next steps

1. **Operator rehearsal of the three-strike rule** (manual §10): run a batch
   with a deliberately small `--tt-mb` until an export abort appears, then
   walk the ladder once (fold → retry → `--tt-mb` bump → quarantine SQL →
   `--moves` re-scope). This is the only part of D6 that no test can pin.
2. **Item 23 (skip + censor) stays parked.** It is drafted only if the
   three-strike rule fires in production (three distinct deep-hole aborts).
3. The `proofdb_flip` failure quoted in `docs/notes.md` should be triaged in
   its own (read-only) probe session before the next production batch.

## 7. Artifacts

- Code: `src/reconstruct/{walker,mod,tests}.rs`, `examples/proofdb/
  {harvest_args,session,shard_export}.rs`, `examples/proofdb_harvest.rs`,
  `examples/reconstruct_pt.rs`, `tests/{reconstruct,proofdb}.rs`.
- Docs: `docs/proofdb_pipeline.md` §7.2/§10/§11, `AGENTS.md`.
- No measurements directory was created (the numbers above are reproducible
  through the named tests; no driver scripts were needed).
