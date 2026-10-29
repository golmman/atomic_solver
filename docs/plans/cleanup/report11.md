# Report 11: tooling DRY (validate field list, config drift, fixture parsers, proofdb fixture) + `--experiment` retired

Implements `docs/plans/cleanup/plan11.md` (initiative `cleanup`), the last
plan of the 8–11 set. Base commit `e28a08d`. All five phases were executed,
but **Phase B pivoted** on an owner decision (§3) and **Phase E's gate was
answered yes** (retire). Measurements:
`docs/plans/cleanup/measurements/plan11/`.

Gate: `make test` green (745 passed / 0 failed / 41 ignored; 1 m 48 s vs the
628 / 41 / 1 m 47 s baseline — the delta is the new config test plus the
proofdb tooling's inline unit tests now running in two more targets, §4).
The three search anchors are child-eval **exact**, `cargo fmt --check`,
`cargo clippy --release --all-targets` and `cargo doc` clean, and
`cargo build --release --all-targets` warning-free.

## 1. Census (before → after)

| file | lines | bytes | change |
| --- | --- | --- | --- |
| `src/search/ordering/params.rs` | 378 → 376 | 12,587 → 12,359 | Phase A |
| `examples/common.rs` | 103 → 53 | 3,431 → 1,826 | Phase C |
| `tests/common/mod.rs` | 343 → 294 | 12,953 → 11,376 | Phase C |
| `tests/common/fixture_parse.rs` | — → 70 | — → 2,541 | Phase C, new |
| `examples/reconstruct_pt.rs` | 662 → 336 | 22,999 → 11,511 | Phase E |
| `examples/proofdb/fixture.rs` | 86 → 97 | 3,193 → 3,689 | Phase D |
| `tests/proofdb.rs` | 1,277 → 1,250 | 48,334 → 47,454 | Phase D |
| `tests/proofdb_fold.rs` | 736 → 701 | 30,059 → 29,036 | Phase D |
| `tests/proofdb_merge_cli.rs` | 315 → 288 | 11,172 → 10,254 | Phase D |
| `tests/proofdb_harvest_cli.rs` | 444 → 417 | 16,474 → 15,556 | Phase D |
| `src/config.rs` | 119 → 129 | 3,606 → 4,141 | Phase B (one test) |
| `config.toml` | 35 → 39 | 847 → 1,117 | Phase B (header only) |

Duplicate censuses (`state/greps_before_after.txt`,
`state/duplication_report.txt`):

| measure | before | after |
| --- | --- | --- |
| `examples/common.rs` vs `tests/common/mod.rs`, normalized duplicated lines (blocks ≥ 2) | 45 in 4 blocks | 2 in 1 block (the `#[allow(unused_imports)] pub use` pair both files need) |
| retyped `("field", value)` literals in `params.rs` | 30 | 3 (checks 2–5, unchanged by design) |
| win-in-1 fixture FEN copies (`tests/` + `examples/`) | 6 | 2 (`fixture.rs` + `proofdb/merge/tests.rs`) |
| `M19_FEN` literal copies | 3 | 2 (`examples/common.rs` canonical + `tests/stress.rs`) |
| `--experiment` occurrences (code, docs, CLI) | 4 | 0 |

## 2. Phase A — validation field list generated from field names

`ScorerParams::validate`'s check 1 built a 27-entry `[(&'static str, i32)]`
array that retyped every field name next to its value. A module-local
`macro_rules! field_values!` (no `#[macro_export]`) now derives the names
with `stringify!`:

```rust
let non_negative = field_values!(self => score_winning_capture, …, and_approach_scale)
    .into_iter()
    .chain(field_values!(pieces: self.pieces => pawn, knight, bishop, rook, queen, commoner));
```

- **A-1 spelling preserved**: the second macro rule emits
  `concat!("pieces.", stringify!($field))`, so the messages stay
  `pieces.commoner must be non-negative` as
  `docs/spec/optimizer_interface.md` and the existing wording require. No
  test asserted the string (checked first), but the spec names it, so the
  prefix stayed.
- **A-2 allocation-free**: the two const-sized arrays are chained by
  iterator, not `concat()`ed into a `Vec`.
- **A-3** checks 2–5 are byte-identical (still holding their 3 literals).
- **A-4** module-local macro, placed directly above `impl ScorerParams`;
  the doc comment says call sites keep struct declaration order (A-5).
- `params::tests::*` green unchanged (incl. `negative_score_rejected`,
  `commoner_must_dominate_other_pieces`); clippy clean.

**Bar D1 ("`params.rs` loses ≥ 45 lines") is not achievable by
construction** — reported rather than gamed. The 30-line literal block
becomes a 7-line dense field list (each name must still be written once)
plus 12 lines of `macro_rules!`; the file moves 378 → 376. The bar's
substantive clauses do hold: the 27 names now appear in the struct and in
one macro invocation, so a new parameter cannot be silently
un-validated unless someone forgets to add it to the invocation — and
that list is 7 lines instead of 27, in the same file as the struct.

## 3. Phase B — `config.toml` drift: found, and **the claim was the stale part**

Plan B-2 anticipated "fix `config.toml` to match the defaults". The
measurement found the opposite:

- `ScorerParams::default()` holds the original hand-tuned weights
  (`score_threat = 1_000`, `pieces.commoner = 20_000`, …).
- `config.toml` was replaced **after** `move_order plan5` created it as a
  defaults mirror, by `171d97a "update tuned config"` (a cherry-pick), with
  the external optimizer's tuned set (`score_threat = 20086`,
  `commoner = 36616`, …). Those values exist **nowhere else in the tree**.
- `tests/test_benchmark_json.rs` passes `--config config.toml` precisely to
  exercise non-default loading, so the file is a live optimizer artifact,
  not a stale defaults copy.

So the plan-literal fix would have deleted the only in-tree copy of the
tuned weights. **Owner decision (this session): option B — keep the tuned
values, fix the claim, do not pin values.** Implemented as:

- `config.toml` header rewritten: it now states that the file is the
  optimizer's tuned set, that the compiled defaults live in
  `ScorerParams::default()`, and that the solver uses those unless a config
  file is named.
- `src/config.rs` gains `repo_config_toml_loads_and_validates`: the
  committed file must load and pass `validate` (the optimizer contract
  admits only validated sets). It deliberately does **not** compare against
  the defaults, so an optimizer run may commit a new tuned set without a
  test edit.
- No `PartialEq` derive was added to `ScorerParams`/`PieceValues`: it was
  test-only support for the equality test that this decision dropped.

Consequence recorded for the future: **a drift between `config.toml` and
the compiled defaults is now allowed and undetectable by design**; the
authoritative defaults are `ScorerParams::default()`, and a config file is
an opt-in experiment surface (`--config` / `SCORER_CONFIG`). If the tuned
set is ever to become the default, that is a search-behaviour change and
needs its own plan with anchor re-baselining — not a cleanup edit.

## 4. Phases C/D — one fixture parser, one win-in-1 tree

**Phase C.** `tests/common/fixture_parse.rs` (new) holds
`MoveOrderCase`, `parse_move_order_fixture`, `load_move_order_suite`,
`load_decisive_suite` and the two `include_str!` fixtures. Both sides
include it by path — `tests/common/mod.rs` as an ordinary child module,
`examples/common.rs` via `#[path = "../tests/common/fixture_parse.rs"]` —
and re-export it with a glob so every `common::load_*` call site is
untouched. Verified points from the plan: the `include_str!` paths resolve
relative to the file containing the macro, i.e. `tests/fixtures/…` for both
users (a single source, no per-user copies); the `#![allow(dead_code)]`
inner attribute of each host module does **not** cover the re-export, so
both `pub use` sites carry `#[allow(unused_imports)]` with a comment
(each binary embeds a subset of the items — the same per-binary artifact
as the `dead_code` allow). `tests/common/mod.rs` keeps its test-only
loaders and calls `fixture_parse::parse_move_order_fixture` directly.
`M19_FEN` is single-sourced: `examples/benchmark.rs` now uses
`common::M19_FEN`; `tests/stress.rs` keeps its literal (it cannot include
`examples/common.rs`) and its module header now names
`common::M19_FEN` as the canonical source.

**Phase D.** `examples/proofdb/fixture.rs` gains
`pub(crate) fn win_in_1_tree(fen: &str) -> ProofTree` (root Win/bound 1 +
terminal `d8h4` Loss child, validated); `fixture_db` itself now builds on
it. `tests/proofdb.rs` (`fixture_bin`) and `tests/proofdb_fold.rs`
(`tactic_tree`/`tactic_tree_at` → one `tactic_tree` helper) call it and
take `TACTIC_FEN` from the module. **D-3 taken (preferred option)**:
`tests/proofdb_merge_cli.rs` and `tests/proofdb_harvest_cli.rs` declare
`#[path = "../examples/proofdb/mod.rs"] mod proofdb;` and use the shared
helper, dropping their copies. Cost measured: no new warnings, no
noticeable build slowdown — but the tooling module's 58 inline unit tests
now also run in those two targets (7 → 65 and 3 → 61 passed, ≈0.5 s each),
which is the bulk of the gate's +117 test count. Worth it: the fixture is
one line at each call site and four literal copies are gone.

## 5. Phase E — `reconstruct_pt --experiment` retired (owner-gated: yes)

Deleted: `Options::experiment`, the `--experiment` arm, the `if
opts.experiment` branch (main calls `run_single` directly), `LiveBuild`,
`live_build`, `run_experiment`, `CaseRow`, `Summary`, `aggregate`, the
`mod common;` (its only use was the decisive suite) and the now-unused
imports (`Position`, `ProofTreeWorkerHandle`, `Search`, `write_tt_snapshot`,
`Arc`, `AtomicBool`, `Ordering`, `Cursor`). 662 → 336 lines / 23.0 →
11.5 KB (bar D4 asked ≤ 380).

Deliberately **kept**:

- `HoleRow` / `hole_row` — serialized by the single-run `--json` output.
- `validate_status` — moved next to `hole_row`; `run_single` still prints
  `validate: ok|FAILED n` and exits non-zero on defects.
- `--snapshot`, `--fen`, `--out`, `--tt-size`, `--pt-size`, `--fill-*`,
  `--timeout`, `--json`, `--oracle` — all shipping surfaces.
- No `src/` item was deleted (E-4): `Search::set_memory_limited`,
  `set_proof_event_sender` and `write_tt_snapshot` remain the documented
  reconstruct-side contracts (`ExitReason::MemoryLimit`) and are exercised
  by integration tests.

Header updated (the go/no-go phrase and the `--experiment` usage line are
gone, the size justification is reworded for what remains) and
`AGENTS.md`'s `reconstruct_pt` bullet now reads "`--oracle` cross-checks
the rebuilt tree against a reference dump".

Regressions checked: `make test` green (incl. `tests/reconstruct.rs`), and
a manual run over a fresh mate-in-3 snapshot reproduces plan10's pinned dump
byte-for-byte — `ab94c69f…`, 70 bytes
(`state/reconstruct_dump.txt`).

## 6. Decision F — `chunk_growth` / `RestartSchedule` configurability: keep

No action, as recorded in the plan: `Growth::Linear`,
`RestartSchedule::{linear, geometric, NEVER, with_cutoff}` and
`Search::set_restart_schedule` cost ~40 tested lines, `cleanup plan7` added
deterministic end-to-end tests for two of them last session (answering
`conversion/report12`), and `examples/chunk_growth.rs` is the documented
probe for a `conversion` item that could re-open. Deleting and re-adding
them is churn. The paragraph in `plan11.md` is the record; this report
repeats it so the decision is not re-litigated in a future pass.

## 7. Remaining known duplication (judgement per item)

| duplication | judgement |
| --- | --- |
| `M19_FEN` literal in `tests/stress.rs` | **accepted** — an integration test cannot include `examples/common.rs`; the module header names the canonical source and the literal is one line in the slow tier. |
| win-in-1 FEN in `examples/proofdb/merge/tests.rs` | **accepted** — that inline unit test builds its own opponent-legal fixture; sharing would couple a merger unit test to the test-harness module. |
| `src/config.rs`'s `DEFAULT_TOML` fixture vs `ScorerParams::default()` | **intentional** (plan B-3): it is a deliberately independent TOML with different numbers, proving that non-default values load and validate. The plan's "third copy of the same numbers" premise was already false for the values. |
| `#[allow(unused_imports)] pub use fixture_parse::*;` in both hosts | **accepted** — the one line each host needs to expose the shared parser; a build-system boundary, not duplicated logic. |
| proofdb tooling's 58 unit tests now running in 4 targets | **accepted** — the documented `#[path]` inclusion pattern, ~0.5 s per target, no warnings. |
| `tests/proofdb.rs` at 1,250 lines / 47 KB (and `tests/proofdb_fold.rs` at 701 / 29 KB) | **named, not fixed** — over the ~20 KB split threshold without an `AGENTS.md` justification. It shrank only by the fixture extraction here; splitting it is its own session (candidate for the next `cleanup` pass). |

## 8. Problems encountered

1. **`self` cannot be matched by a `macro_rules!` `:ident`** (the Rust
   reference excludes `crate`/`self`/`super`/`Self`). The plan's sketch
   `($this:ident => …)` would not compile; the macro takes `$this:expr`
   instead, and local variables are hygienic inside a macro body, so `self`
   must be passed in explicitly (it is).
2. **`#![allow(dead_code)]` does not cover re-exports** — the first
   `#[path]` wiring produced `unused import` warnings in every example and
   test binary; fixed with a scoped `#[allow(unused_imports)]` and a comment
   (the plan's C-3 fallback was not needed).
3. **Phase B's premise was inverted** (§3): the plan assumed `config.toml`
   was the stale copy of the defaults; it is the only copy of the tuned
   set, and the *header claim* was the stale part.
4. **Bar D1's ≥ 45-line target is unreachable** (§2); the durable result is
   that the names are generated, not that the file shrank.

## 9. Missing tests / next steps

- No test pins `config.toml`'s values anymore (decision in §3). If the
  tuned set is promoted to the compiled defaults, do it as a search-change
  plan with anchor re-baselining, and only then re-introduce equality
  pinning (the defaults side would be the pinned one).
- `examples/common.rs`'s re-export is now the single funnel for fixture
  helpers; if a future example needs a *different* fixture field, extend
  `tests/common/fixture_parse.rs`, never the hosts.
- The `cleanup` initiative has no open plans after this one; it stays
  "active as-needed" (new plans are opened per pass, not as a backlog).
- The largest remaining housekeeping item found but not taken: `tests/proofdb.rs`
  (1,250 lines / 47 KB) and `tests/proofdb_fold.rs` (701 / 29 KB) are over the
  file-size convention without a recorded justification.

## 10. Bar scoreboard

| bar | result |
| --- | --- |
| D1 (Phase A) | names generated, 27 → 0 retyped in check 1; file −2 lines (**size sub-bar unreachable**, §2); tests/clippy green |
| D2 (Phase B) | pivoted by owner decision; header truthful, load-and-validate test added (**no equality pin**) |
| D3 (Phase C+D) | all targets build and pass; duplication 45 → 2 normalized lines; `M19_FEN` 3 → 2 copies; win-in-1 FEN 6 → 2 |
| D4 (Phase E) | 662 → 336 lines (≤ 380); `--experiment` gone everywhere; dump sha256 identical; `cargo doc` clean |
| D5 (whole plan) | three anchors child-eval exact; fmt/clippy/doc clean; `make test` green; `git status` limited to the files named in the executed phases plus these deliverables |
