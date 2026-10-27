# Plan 7: Housekeeping pass — `dfpn/mod.rs` split, restart-schedule test gap, slow-test decisions

Initiative: `cleanup` (housekeeping: DRY, YAGNI, lint, module sizing).
Opened by: the closure block of `docs/plans/conversion/report12.md`
("Housekeeping/cleanup pass — revisit the `dfpn/mod.rs` size watch and the
two re-baselined slow tests"). This plan is the successor to that
follow-up; it is **not** a `conversion` lever and does not touch search
semantics.

Plan numbering: `cleanup` plans 1–6 are done (`report1.md`–`report6.md`);
this is the next free number.

## Prerequisite reading (all self-contained pointers)

- `AGENTS.md` §"Conventions" — the file-size rule: under ~10 KB, a
  documented justification >10 KB, "files larger than ~20 KB should
  normally be split into submodules"; §"File size justifications" — the
  running list of what is currently justified (`children.rs`,
  `preflight/mod.rs`, `preflight/region.rs`, `worker.rs`, `ordering.rs`,
  `selection.rs`, `main.rs` — **`src/search/dfpn/mod.rs` is not on it**).
- `src/search/dfpn/mod.rs` — 40,857 bytes / 986 lines: module docs + `mod`
  declarations, `ExitReason` / `PvStatus` / `RoundTermination`, the
  `struct Search` field list, `Search::new`, 36 `pub fn` setters/getters,
  the private run driver (`bounded_search`, `begin_run`,
  `preflight_phase`), the four solve entry points, and the state/clock
  helpers (`nodes`, `child_evaluations`, `reset_search_state`,
  `log_chunk`, `time_exceeded`, `path_*`).
- `src/search/dfpn/restart_schedule.rs` — `RestartSchedule`,
  `Growth::{Geometric, Linear}`, `first_slice`, `next`, and the three
  existing pure unit tests.
- `src/search/dfpn/tests.rs` — 24,411 bytes; the integration point for the
  new schedule tests (`super::{ExitReason, PvStatus, RestartSchedule,
  RoundTermination}`, `Search::new(64)`, `search.bounded_search(..)`).
- `docs/plans/conversion/report12.md` §"Unresolved / missing tests",
  §"Pre-existing stale tests found by the full gate", §"Next steps" — the
  provenance of every item here.
- `tests/stress.rs:29-58` (`move_order_hard_positions_unproven_in_60s` and
  its `m21_white` exclusion) and `tests/test_deep_outcomes.rs:203-217`
  (`m22_white_wins`, `assert_solves_within_evals_with_tt`).
- `Cargo.toml` — `[profile.release] lto = true, codegen-units = 1` (the
  reason a module split cannot change codegen).

## Why this plan exists

Three thread ends were left open by `report12.md`, and one of them is now
the single largest maintenance item in the repo:

1. **`src/search/dfpn/mod.rs` is 40.8 KB / 986 lines** — 4× the 10 KB rule
   and 2× the "should normally be split" line, and not in the
   `AGENTS.md` justification list. Its header currently says it is
   "intentionally larger than 10 KB because the search loop, public entry
   points, and move ordering all live in a single `Search` implementation
   for performance". That rationale is **false for this build**:
   `lto = true` + `codegen-units = 1` means a within-crate module boundary
   is not an inlining barrier. A false justification in a module header is
   worse than no justification, because it discourages the fix.
   (`docs/plans/README.md`'s `cleanup` row already names this watch:
   "`dfpn/mod.rs` size watch (40 KB)".)
2. **The restart schedule has no end-to-end cutoff test.**
   `restart_schedule.rs`'s pure tests pin `SHIPPED`'s geometric and linear
   formulas, and pin `cutoff: Some(0)` ≡ `NEVER`, but **nothing exercises a
   non-zero cutoff firing mid-run**, and **nothing exercises
   `Growth::Linear` through `bounded_search`**. `report12.md` records this
   as its own unresolved gap ("A cheap deterministic cutoff test would
   close this").
3. **Two slow tests were re-baselined as maintenance** (`m21_white`
   excluded from the unproven assertion; `m22_white_wins` moved to a 60M
   budget at the 256 MB TT). Their *numbers* are documented, but the
   decision "leave `m21_white` excluded instead of promoting it to a
   positive regression" was never recorded as a decision, so the next
   session will re-litigate it.

## Scope

**In scope**

1. Phase A — deterministic tests for a non-zero cutoff firing mid-run and
   for `Growth::Linear` end-to-end (plus a doc clarification on
   `restarts()`).
2. Phase B — record the `m21_white` / `m22_white_wins` decisions and add
   the missing `AGENTS.md` size justifications (`dfpn/tests.rs`).
3. Phase C — split `src/search/dfpn/mod.rs` into
   `mod.rs` + `config.rs` + `run.rs` by **move-only** code motion, with a
   mechanically verified move-only bar and a behavior/bit-identity bar.

**Out of scope (non-goals)**

- Any change to search semantics, thresholds, ε, ordering, TT, the
  repetition cache, the pre-phase, the restart schedule's constants, or
  the budget contract. This plan must be evaluable as "no result
  changed".
- Renaming or re-scoping existing public API. `Search`'s **public** method
  set must be identical before and after (36 `pub fn`s in `mod.rs`).
- Splitting `src/search/dfpn/tests.rs` (24.4 KB). Decision recorded in
  Phase B: keep it as one file with an `AGENTS.md` justification, because
  its fixtures (`FEN` consts, `Search` construction idioms, the shared
  `bounded_search` calls) are shared across schedule/budget/ordering/
  repetition tests; splitting would duplicate the harness. Revisit only if
  it crosses ~30 KB.
- `examples/`, `src/reconstruct`, `src/proof_tree`, `src/search/tt`,
  `src/search/preflight` — untouched.
- Rewriting historical reports (`report12.md` stays as written).

## Normative design

### Phase A — schedule tests (the only correctness-surface work)

A-1. `restart_schedule.rs` unit test
`cutoff_fires_at_the_boundary_it_is_crossed` (pure, fast):

```rust
let s = RestartSchedule::SHIPPED.with_cutoff(Some(1_000_000));
assert_eq!(s.first_slice(), 500_000);            // non-zero cutoff keeps the base
assert_eq!(s.next(500_000, 500_000), 1_000_000); // not yet fired
assert_eq!(s.next(1_000_000, 999_999), 2_000_000);// still not fired (spent < cutoff)
assert_eq!(s.next(1_000_000, 1_000_000), u64::MAX); // fires at spent == cutoff
assert_eq!(s.next(2_000_000, 5_000_000), u64::MAX); // stays fired
```

A-2. `restart_schedule.rs` unit test
`first_slice_ignores_nonzero_cutoff`: `first_slice() == base` for
`with_cutoff(Some(c))` with `c > 0`, and `== u64::MAX` only for `Some(0)`
(this pins the "cutoff fires before the first slice" special case to `0`
alone, which is what `NEVER` relies on).

A-3. `dfpn/tests.rs::linear_schedule_restarts_end_to_end` — `Growth::Linear`
wired through `bounded_search`. m22 first-outcome at tt 256, slices
`500_000, 1_000_000, 1_500_000, …` ⇒ cumulative `0.5, 1.5, 3.0, 5.0, 7.5,
10.5, 14.0, 18.0 M`. Whatever the entry needs after 14 M, **at least 7
boundaries are necessarily crossed before any slice can be large enough**
(the 8th slice is 4 M; a cold-start m22 solve is ~19.4 M under
`RestartSchedule::NEVER`), so assert the structural lower bound and pin
the exact deterministic cost:

```rust
let mut search = Search::new(256);
search.set_timeout(60);
search.set_restart_schedule(RestartSchedule::linear(500_000, 500_000));
let (outcome, _pv, _nodes) = search.search_depth(&mut pos, u32::MAX);
assert_eq!(outcome, Outcome::Win);
assert!(search.restarts() >= 7, "linear growth stalled: {}", search.restarts());
assert_eq!(search.child_evaluations(), /* pinned measured value */);
```

A-4. `dfpn/tests.rs::cutoff_fires_mid_run_and_solves` — a **non-zero**
cutoff firing mid-run. Same m22 FEN and TT; `SHIPPED.with_cutoff(Some(2_000_000))`.
`SHIPPED`'s boundaries on this case are at spent `0.5, 1.5, 3.5, 7.5 M`;
with a 2 M cutoff the third boundary (spent 3.5 M) returns `u64::MAX`, so
**exactly 3** boundaries are ever counted and the final unbounded slice
solves:

```rust
let mut search = Search::new(256);
search.set_timeout(60);
search.set_restart_schedule(RestartSchedule::SHIPPED.with_cutoff(Some(2_000_000)));
let (outcome, _pv, _nodes) = search.search_depth(&mut pos, u32::MAX);
assert_eq!(outcome, Outcome::Win);
assert_eq!(search.restarts(), 3);   // structural: no boundary after the cutoff
assert_eq!(search.child_evaluations(), /* pinned measured value */);
```

`restarts() == 3` (vs 4 for the uncut `SHIPPED` run) is the observable
that proves the cutoff **fired**. Do **not** expect the eval count to equal
`SHIPPED`'s 15,023,911: with the cutoff, the entry at 3.5 M runs unbounded
instead of being restarted at 7.5 M, so its cost moves toward the
no-restart figure (~19.4 M) — pin the measured value, and record it in
`report7.md` as evidence of the trajectory change.

A-5. Doc clarification, `mod.rs` `restarts()`:
> Restart boundaries crossed in the current run (reset by `begin_run`).
> A boundary at which the schedule's cutoff fires still counts: the run
> re-enters `dfpn` with `INF`/`INF` and an unbounded final slice.

A-6. Cost guard: A-3 and A-4 each run m22 first-outcome at a 256 MB TT
at roughly the no-restart cost, i.e. ~1.5–3 s each on the reference host.
If the two together exceed ~5 s, mark the slower one
`#[ignore = "slow: ..."]` (it then runs only under `make test-full`) and
record the measurement in `report7.md`. Do **not** shrink the FEN's TT to
64 MB: m22 needs the product TT (`report12.md` §"Pre-existing stale
tests"), and do not weaken the assertion to a cheap FEN that never
restarts (the point is the boundary count).

### Phase B — decisions, recorded

B-1. **`m21_white` stays excluded from the unproven assertion.** Do not
add a positive regression: it needs ~52 s / 596,102,838 evals at a 64 MB
TT, which would add ~3.5% to an already ~25-minute slow tier, and the
restart-sensitive class it guards is already covered by `dec31` (0.51 M
evals, `Loss`), `m23_white` (11.6 M, `Win`) and `m22_white_wins` (15.0 M,
`Win`). Action: extend the existing doc comment in `tests/stress.rs` with
one sentence stating that this is a **decision**, not a pending TODO
("do not promote unless the slow tier's budget is deliberately raised"),
and record it in `report7.md`.

B-2. **`m22_white_wins`' 60M budget / 256 MB TT stands.** No change; the
4× headroom (15.0 M measured) is documented in the test and in
`report12.md`.

B-3. **`AGENTS.md` file-size justifications** — add two bullets to
§"File size justifications" (the `dfpn/mod.rs` one is replaced by the
split, see Phase C):

```markdown
- `src/search/dfpn/{mod,config,run}.rs` — the `Search` state definition and
  exit/PV types (`mod.rs`), the configuration surface (`config.rs`), and the
  run driver plus solve entry points (`run.rs`); split by change reason, not
  for performance (`lto = true`, `codegen-units = 1` make module placement
  codegen-neutral).
- `src/search/dfpn/tests.rs` — one fixture set and one `Search`-construction
  idiom serve the schedule/budget/ordering/repetition unit tests; splitting it
  would duplicate the harness.
```

No historical report is edited.

### Phase C — the split (move-only)

Target layout, with the exact method inventory (line numbers are the
HEAD `986`-line file; byte counts measured at HEAD and expected to be
preserved within ±1 KB after the split):

| file | contents | HEAD lines | expected size |
| --- | --- | --- | --- |
| `mod.rs` | module docs, `mod` decls, `use`, consts, `ExitReason` + `Display`, `PvStatus`, `RoundTermination`, `struct Search`, `Search::new`, helpers `emit_proof_node`, `nodes`, `child_evaluations`, `reset_search_state`, `log_chunk`, `time_exceeded`, `path_contains`/`path_push`/`path_pop` | 1–257, 505–524, 901–986 | ~13.8 KB |
| `config.rs` | `set_preflight_enabled`, `preflight_report`, `set_first_outcome_only`, `set_scorer`, `scorer`, `set_timeout`, `set_child_eval_budget`, `set_refine_cap_factor`, `set_refine_round_cap_for_test`, `first_outcome_evaluations`, `refinement_rounds`, `refinement_evaluations`, `pv_status`, `last_refine_round_cap_cut`, `child_eval_budget_exceeded`, `set_max_ply`, `set_restart_schedule`, `restart_schedule`, `restarts`, `set_epsilon`, `set_salt`, `set_stop_flag`, `set_memory_limited`, `set_proof_event_sender`, `tt`, `tt_mut`, `tt_stats`, `tt_best_child_counts`, `exit_reason` | 258–387, 398–540 | ~10.1 KB |
| `run.rs` | `bounded_search`, `search_depth`, `search_depth_with_prefix`, `solve`, `solve_with_progress`, `begin_run`, `preflight_phase`, **and `refinement_round_cap`** (it is used only by `solve_with_progress`, line 761) | 388–397, 545–900 | ~16.6 KB |

Rules:

1. **Move-only.** No line inside a moved function body changes. The only
   permitted edits in the moved text are: the signature's visibility token
   where required by (2), and the new file's `use` header. Line count of
   the three files combined must equal HEAD's 986 ± the added
   `mod`/`use`/`//!` lines only.
2. **Visibility.** Rust privacy is module-ancestor scoped: an item
   declared in `dfpn` (i.e. `mod.rs`) is visible in `dfpn` and all its
   descendants, so everything **staying** in `mod.rs` needs no change. An
   item moved **into** `dfpn::run` and referenced outside it must become
   `pub(super)` — never `pub`. Known instances:
   - `bounded_search` (called by `dfpn/tests.rs`),
   - `begin_run` (called by `dfpn/tests.rs`),
   - `refinement_round_cap` is private-in-`run` and used only there
     (**no** visibility token needed).
   `preflight_phase` is private-in-`run`, called only from `run.rs`.
3. **Public surface unchanged.** The set of `pub fn`/`pub const fn`
   methods on `Search` must be identical (36 at HEAD) and no `pub` item
   may be added or removed. Verify with a name-set diff, not by eye.
4. `mod.rs` declares `mod config; mod run;` (alphabetical, next to the
   existing `mod children;` … block) and keeps `#[cfg(test)] mod tests;`.
   Each new file starts with a `//!` line stating its change reason.
5. Replace the false header paragraph in `mod.rs`
   ("intentionally larger than 10 KB because … for performance") with a
   truthful one-liner + pointer: the split is by change reason; module
   placement is codegen-neutral here (`lto = true`,
   `codegen-units = 1`). Keep the existing `//!` contract text about the
   restart schedule and per-node budget (it is accurate; `report12.md`
   wrote it).
6. Do not touch `docs/notes.md` (it is user scratch; it is currently dirty
   in `git status` and must stay out of the session's diff review).
7. Naming: `mod config;` inside `dfpn` declares `dfpn::config`, distinct
   from the crate-root `atomic_solver::config`. Any reference to the
   crate-root module inside the moved code must be spelled `crate::config`
   (grep for it before moving).

## Phase 0 — baseline capture (read-only, ~10 min)

1. `git status --porcelain` must show only `docs/notes.md`; anything else
   is a stop-and-investigate event. All anchor commands below report the
   child-eval count as a final `evals: <n>` line on **stderr** (both with
   and without `--outcome-only`); capture stderr.
2. `cargo build --release`, then capture the anchor set and record it in
   `measurements/plan7/state/anchors.json`:

   | case | command | expected |
   | --- | --- | --- |
   | quick @ tt 256 | `cargo run --release --example benchmark -- --suite quick --first-outcome --timeout 3 --runs 1 --tt-size 256` | total 42,109,768, 59/59, zero `wrong` |
   | quick @ tt 128 | same with `--tt-size 128` | total 40,042,745, 59/59 |
   | dec31 FO | `./target/release/atomic_solver --fen 'r1bq1k2/ppN3p1/2p1p2P/6P1/1b1P1p2/8/PP3P2/R2K1B1R b - - 0 15' --timeout 20 --first-outcome --outcome-only` | 510,426, `loss` |
   | m22 FO | `--fen '4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22' --timeout 60 --first-outcome` | 15,023,911, `win` |
   | m23_white FO | `--fen '4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23' --timeout 60 --first-outcome` | 11,621,376, `win` |
   | stress FO | `--fen '4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21' --timeout 200 --first-outcome` | 287,872,755, `win` |
   | m22 default | `--fen <m22> --timeout 20 --outcome-only` | stdout == `tests/fixtures/m22_default_stdout_golden.txt` (18,779,890, `pv_status: cap-cut`) |

   The dec31 FEN is copied verbatim from
   `src/search/dfpn/tests.rs::shipped_schedule_restarts_past_the_first_slice`
   (line 607) and from the plan12 anchor table. Any anchor that does not reproduce
   exactly is a **stop-and-investigate** event (it invalidates bars D3-3
   and D3-4); record the deviation before proceeding.
3. `cargo test --release --test test_trajectory_golden -- --include-ignored`
   green.
4. Write the move-check driver (`measurements/plan7/verify_move.py`, see
   Measurement artifacts), then snapshot the "before" function inventory:
   `verify_move.py --snapshot HEAD` writes `state/functions_before.json`
   (fn name → normalized body hash + line count) by parsing
   `git show HEAD:src/search/dfpn/mod.rs`.
5. Record wall medians of 3 for stress FO and m22 default-mode (`time`,
   interleaved) into `state/wall_before.json`.
6. Capture `state/public_surface_before.txt`
   (`grep -nE '^\s+pub (const )?fn' src/search/dfpn/mod.rs`).

## Phase 1 — Phase A tests (implementation)

Add A-1, A-2 (pure) and A-3, A-4 (end-to-end) + A-5's doc line.
Run `cargo test --release dfpn:: -- --nocapture` and record the measured
`restarts()`/`child_evals` values in `report7.md`; the `child_evals`
assertions in A-3/A-4 must be the **measured** ones (the arithmetic above
is the expectation; if `restarts()` disagrees with A-4's structural `== 3`
— the one count the plan does assert — the schedule semantics differ from
the docs and that is a finding).

Bar **D1**: A-1…A-4 pass, `cargo fmt` clean, `make test` green, and the
fast tier's test time does not exceed ~60 s (A-6's guard).

## Phase 2 — Phase B decisions (docs only)

B-1's comment sentence, B-2 (no change), B-3's `AGENTS.md` bullets.
Bar **D2**: `git diff` touches only `tests/stress.rs` (comment) and
`AGENTS.md`; no source behavior change. Capture
`git status --porcelain` at the end of this phase into
`measurements/plan7/state/status_after_phase_b.txt` (the D3-1(c) baseline).

## Phase 3 — Phase C split (implementation)

Apply the layout above. Suggested order (each step compiles):

1. Create `config.rs`, move the 29 configuration/accessor methods +
   `use` header. `mod.rs` gains `mod config;`.
2. Create `run.rs`, move `bounded_search` (→ `pub(super)`), the four
   entry points, `begin_run` (→ `pub(super)`), `preflight_phase`,
   `refinement_round_cap`. `mod.rs` gains `mod run;`.
3. Replace `mod.rs`'s false header paragraph.
4. `cargo fmt`, `cargo clippy --all-targets`, `cargo doc` (no new
   warnings), then the move check and the behavior bars.

### D3 — pre-registered acceptance rule (does the split ship?)

The split ships **iff all** of:

1. **Move-only, mechanically verified.** `measurements/plan7/verify_move.py --check`
   reports: (a) every function body in `state/functions_before.json` is
   present exactly once in the new `dfpn/` tree with an identical
   normalized body (whitespace-normalized per line, braces matched, the
   signature line excluded); (b) the `fn`-name multiset is unchanged (no
   function added, removed, or renamed); (c) relative to the end of
   Phase 2 (whose diff inventory is captured in
   `state/status_after_phase_b.txt`), Phase 3 adds no changed path other
   than `src/search/dfpn/mod.rs` and the new
   `src/search/dfpn/{config,run}.rs`.
2. **Layout.** No resulting file exceeds 20 KB; `mod.rs` ≤ 15 KB; the
   three files' combined line count equals 986 + `mod`/`use`/`//!` lines
   (report the delta). Public surface name-set diff is empty against
   `state/public_surface_before.txt` (captured in Phase 0 with
   `grep -nE '^\s+pub (const )?fn' src/search/dfpn/mod.rs`).
3. **Bit-identity.** Quick suite at both TT sizes reproduces Phase 0
   exactly (42,109,768 / 59 and 40,042,745 / 59, zero `wrong`); all six
   single-case anchors are eval-identical (dec31 510,426; m22 FO
   15,023,911; m23_white 11,621,376; stress FO 287,872,755); the m22
   default-mode golden passes **without** re-baselining.
4. **No wall regression.** stress FO and m22 default-mode wall within
   +2% of Phase 0's medians of 3. (Expected: identical codegen, because
   the crate is one CGU with fat LTO; this bar exists to catch a
   `#[inline]`/monomorphization surprise, not to measure anything.)
5. **Gates.** `cargo fmt --check`, `cargo clippy --all-targets`,
   `cargo doc` warnings unchanged, `make test` green, `make test-full`
   green (the bit-identity anchors and the golden live in the slow tier).

If D3-1 or D3-3 fails → **revert Phase C entirely** (keep Phases A/B);
the item closes as "split deferred, justification added to `AGENTS.md`"
with the failing bar recorded. Do not "fix" the golden or an anchor. If
D3-4 fails → try `#[inline]` on nothing (there are none today); instead
revert and record — a pure move must not need codegen help. If D3-2
fails (a file still >20 KB) → move the state/clock helpers out of
`mod.rs` into a fourth file `state.rs` (~3.4 KB) and re-run D3-1…5.

## Tests to add / change

1. `restart_schedule::cutoff_fires_at_the_boundary_it_is_crossed` (A-1).
2. `restart_schedule::first_slice_ignores_nonzero_cutoff` (A-2).
3. `dfpn::tests::linear_schedule_restarts_end_to_end` (A-3).
4. `dfpn::tests::cutoff_fires_mid_run_and_solves` (A-4).
5. No test is removed or weakened; `m21_white`'s exclusion stays (B-1).

## Gates and validation order

1. Phase 0 anchors + golden + `functions_before.json` + wall medians.
2. Phase 1: A-tests + `make test` (bar D1).
3. Phase 2: docs-only diff review (bar D2).
4. Phase 3: `cargo fmt` → `clippy` → `doc` → `verify_move.py --check` →
   quick suite (both TT) → six anchors → golden → wall medians of 3 →
   `make test` → `make test-full`.
5. Final `git status`/`git diff --stat` review: only Phases A/B/C files
   (plus `docs/plans/` deliverables) changed; `docs/notes.md` untouched.

## Measurement artifacts

`docs/plans/cleanup/measurements/plan7/` (per `AGENTS.md`'s fixed layout):

- `README.md` — provenance table (what each file is, HEAD commit, how to
  reproduce).
- `env.json` — commit hash, host, rustc/cargo, CPU count, TT sizes,
  timeouts, epsilon.
- `verify_move.py` — `--snapshot HEAD` / `--check`; parses `fn` bodies by
  brace matching from `git show HEAD:src/search/dfpn/mod.rs` and from the
  working tree, normalizes per-line leading whitespace, and compares
  body hashes. Sets `sys.dont_write_bytecode = True`.
- `state/anchors.json`, `state/functions_before.json`,
  `state/public_surface_before.txt`, `state/status_after_phase_b.txt`,
  `state/wall_before.json`, `state/wall_after.json`.

Commit only these plus the plan/report. Raw run transcripts stay
untracked (`.gitignore`d).

## Risks

1. **Move-only discipline decays under a "small fix" impulse.** Any edit
   inside a moved body silently widens the change from "code motion" to
   "semantics + motion", and the bit-identity bar is the only detector.
   Mitigation: D3-1 is mechanical (body hashes), not review-by-eye; if it
   fails, revert rather than justify.
2. **The m22-based phase-A tests are the slowest fast-tier additions.**
   Two runs at a 256 MB TT. Mitigation: A-6 measures them; if the pair
   exceeds ~5 s, the slower one becomes `#[ignore = "slow: ..."]` and
   moves to the full tier. They must not be weakened to make the fast gate
   faster (that would leave the cutoff surface untested, which is the whole
   point).
3. **The expected `restarts()` counts may not match the arithmetic.** The
   `bounded_search` loop counts a boundary when `next_chunk > 0`, so a
   cutoff-detected boundary (returning `u64::MAX`) **does** count. If a
   measured count contradicts A-4's structural `== 3` (or falls below
   A-3's `>= 7`), the plan's model of the loop is wrong: relax the
   assertion to what the loop actually does, add the discrepancy to
   `report7.md` as a documentation finding, and fix the doc, not the code.
4. **`mod.rs` still >15 KB after the split** because the state/clock
   helpers are bigger than measured. Mitigation: the documented D3-2
   fallback (fourth file `state.rs`).
5. **A hidden `pub` re-export or doc-link path breaks.** `proof_tree/validate.rs`
   documents `search::dfpn::emit_proof_node`; keeping that method in
   `mod.rs` (as designed) preserves the path. `cargo doc` in the gate
   catches intra-doc-link breakage.
6. **Session overrun.** Phases 0–2 are ~45 min; Phase C plus the full
   gate is ~1.5–2 h. The plan is splittable at the Phase-2 boundary: if
   the session runs out there, write `report7.md` covering Phases 0–2 +
   D1/D2 and open `plan8.md` for the split (Phase C, D3), recording the
   split in both the report and `docs/plans/cleanup/initiative.md`.
   Do not start Phase C without time for the full gate.

## Deliverables

1. Phases A/B/C implemented as above.
2. `docs/plans/cleanup/report7.md` — **final task**. It must contain: the
   Phase 0 anchor reproduction (or the deviation), D1/D2/D3 verdicts
   against the pre-registered bars, the three files' sizes and the
   move-check output, the measured `restarts()` values from A-3/A-4, the
   `m21_white` decision and its cost rationale, the `AGENTS.md` bullets
   added, tools used, problems encountered, unresolved parts, missing
   tests, and next steps.
3. Index/initiative updates: `docs/plans/cleanup/initiative.md` gains
   plan7 under **Done**; `docs/plans/README.md`'s `cleanup` row's "Next
   lever(s)" cell is updated (the `dfpn/mod.rs` size watch is resolved or
   the remaining item is restated) — per `AGENTS.md`, only when the
   initiative opens/pivots/closes, which this plan does (it clears the
   row's only named watch item).
4. `docs/plans/conversion/report12.md` is **not** edited; the closure
   thread is recorded complete in `report7.md`.

## Expected verdicts (pre-registered expectations, not bars)

D1: pass (the tests are deterministic and cheap). D2: pass (docs only).
D3: ship — move-only verified, eval-identical, wall-neutral, both tiers
green, and `mod.rs` down from 40.8 KB to ~14 KB with the false header
rationale replaced by a true one. The session is a success if the report
can answer "is the biggest source file still a maintenance risk, and is
the restart cutoff actually covered?" with a verified yes/no.
