# Upstream report: `atomic-movegen` 2.4.0 (feedback round 2)

Recorded 2026-11-03. Consumer-side update report for the release that
implements `docs/plans/movegen/feedback2.md`.

## Release

- **Published version:** `atomic-movegen` 2.4.0 on crates.io
  (2026-11-02), semver-minor: rules fixes plus additive API.
- The consumer requirement moved from `"2.3"` to `"2.4"`; only
  `Cargo.lock` otherwise changed (`cargo update -p atomic-movegen`,
  2.3.1 → 2.4.0). Compiles against the existing tree without source
  changes; the work below is deliberate adoption, not a fix-up.
- Upstream changelog: `CHANGELOG.md` in the crate, section `[2.4.0]`.

## What shipped upstream

The round-2 wishlist is essentially complete:

| Feedback item | 2.4.0 delivery |
| --- | --- |
| P0-1 rules fixes | touching-commoner `checkers` immunity, mate-outranks-clock, terminal-board movegen, self-explosion cleanup |
| P0-2 terminal API | corrected `outcome()`, `outcome_with_state()`, `outcome_with(state, moves_exist)`, `is_material_draw()`, strict `is_terminal()` |
| P0-3 no insufficiency table | `is_material_draw()` is commoner-only; K+B/K+N/K+R counterexamples pinned |
| P1-4 move wire/UCI | stable `Move::to_bits`/`from_bits`, `from_uci`, `write_uci`, `Display`, `Hash`/`Ord`, `Board::move_from_uci` |
| P1-5 single-move legality | `is_pseudo_legal`, `is_legal`, `is_legal_with_state` |
| P1-6 ep normalization | `legal_ep_square`, `EnPassantMode::{Always,Legal}`, `hash_with`/`fen_with`/`hash_after_with` |
| P1-7 validation | `validate()`, `from_fen_with`, `FenPolicy`, `ValidationErrors`, `FenLoadError` |
| P2-8 blast helper | `Board::blast_zone`, `Board::blast_casualties` |
| P2-9 conveniences | `attacks::attacks_from`, `attackers_to_color`, `Board::is_zeroing`/`is_double_push`, `MoveList::retain`, corrected `MAX_MOVES` docs |
| P2-10 perf | `hash_and_rule50_after`, opt-in `unsafe-optimizations` feature |

## Consumer changes

### Rules behaviour (the reason the dependency bump is not a no-op)

- **Touching-commoner immunity.** `Board::checkers` no longer reports a check
  when the two commoners touch. This is consumer-visible: `move_class`
  derives its `Check` class from `checkers()`, and the child terminal ladder
  uses the checkers bit for the mate/stalemate split. Both now agree with the
  move generator. `eval_guide`'s hand-evaluation touching exclusion is
  unchanged (it is a heuristic, not a rules query).
- **Terminal-board movegen.** `generate_legal[_with_state]` and the existence
  query return no moves once either commoner is extinct. The child ladder
  already evaluated extinction before the existence query, so its behaviour
  is unchanged.
- **Self-explosion.** `Board::legal` now rejects any move destroying one of
  the mover's commoners, not only the last one. This only affects artificial
  multi-commoner boards; the solver's permissive FENs still parse and the
  multi-commoner heuristic fixtures still run.

### Terminal taxonomy: one implementation

`src/position.rs` no longer owns a precedence. `Position::outcome()` is
`Board::outcome()`, and `Position::outcome_from_state(state, moves)` is
`Board::outcome_with(state, !moves.is_empty())` plus the local `Outcome`
mapping. `src/search/dfpn/children.rs` keeps only the cost-ordering fast
paths (extinction first, the rule50-expired branch, the outcome-less TT
reuse); the classification inside them is `Board::outcome_with`, and the
manual `occupied == 2` / checkers split is deleted.

The pre-phase's `classify_terminal` remains a clock-0 copy by design: it
answers WDL at halfmove 0 for the region fixpoint, where the upstream
`outcome*` family would apply the real clock. It now uses
`Board::is_material_draw()` for the last branch, so "commoner-only" has one
definition. `examples/egtb_gen3` likewise uses `is_material_draw()` instead
of `occupied().count() == 2`.

### Stable move codec and UCI lookup

`src/notation.rs` keeps its public wrappers, but:

- `move_to_bits` / `bits_to_move` delegate to `Move::to_bits` / `Move::from_bits`
  (the layout is now upstream's semver-stable contract, not a local doc
  comment). `bits_to_move(0)` is `Move::NONE`; the TT snapshot's `0xFFFF`
  sentinel remains unambiguous.
- `uci_to_move` delegates to `Board::move_from_uci`: one syntax parse, a
  board-aware castling/en-passant resolution, and a single-move legality
  check. It no longer builds and scans a move list, and no longer allocates a
  `String` per candidate.

### Single-move legality and combined child keys

- `Position::try_do_move` uses `Board::is_legal`; the reconstruct walker's
  `best_move` guard uses the same predicate instead of
  `legal_moves_vec().contains(..)` (the walker still builds the full list when
  it needs the proof's reply set).
- `Position::keys_after` uses `Board::hash_and_rule50_after`, classifying the
  move once instead of calling `hash_after` and `rule50_after` separately.

### Small conveniences

- `src/search/ordering.rs` and `src/search/dfpn/eval_guide.rs` drop their
  private `attacks_from` matches in favour of `attacks::attacks_from`.
- `src/search/dfpn/move_class.rs` and `src/search/ordering.rs` use
  `Board::attackers_to_color` where they previously intersected
  `attackers_to` with a color bitboard.

### Validated FEN ingestion

`Position::from_fen_with_policy(fen, FenPolicy)` is the opt-in validated
constructor; `Position::from_fen` stays permissive for artificial fixtures.
It is wired into the proof-artifact ingestion points:

- `examples/proofdb/merge/fold.rs` validates shard metadata and tree root
  FENs with `FenPolicy::AllowTerminal` (proof roots may be terminal) before
  the existing replay/hash-fidelity check;
- `src/proof_tree/validate.rs` validates a dump's root FEN with the same
  policy.

New unit coverage in `src/position.rs` pins the split: a multi-commoner FEN
is accepted permissively and rejected by `AllowTerminal`; a one-commoner
terminal root is accepted by `AllowTerminal` and rejected by `Strict`.

### Repinned dec10 anchor

The dec10 portfolio smoke anchor moved from 3,901,847 to **2,717,601**
evals. The drift comes from the rules fix (a deep child class that used to
look like a check is now a draw), not from the port. The new line was
validated end-to-end: a fresh TT snapshot was reconstructed with
`reconstruct_pt`, which reported `validate: ok` on 52,198 nodes and
`anomalies=0`. The winner salt remains unpinned (a scheduling artifact).

## Measurements

### UCI lookup

External probe (release, same host), comparing the removed
generate-and-compare fallback with `notation::uci_to_move`:

| Position | Legal moves | Old | New | Speedup |
| --- | ---: | ---: | ---: | ---: |
| startpos | 20 | 129.3 ns | 15.8 ns | 8.2× |
| m22 | 43 | 327.3 ns | 14.1 ns | 23.3× |
| dec10 | 16 | 79.4 ns | 13.9 ns | 5.7× |
| castling fixture | 26 | 245.7 ns | 18.4 ns | 13.4× |

The feedback's 1.1–1.5 µs figure came from an implementation that
allocated per candidate; this tree's fallback was already leaner, but the
new path still removes the move-list generation and all per-candidate
`to_uci()` allocations.

### Gates

- `make test` (fast gate) — green.
- `make test-full` / `cargo test --release -- --include-ignored` — green
  except **two pre-existing dec02 slow tests**, reproduced at HEAD with
  `atomic-movegen` 2.3.1 and with a clean 2.4.0 build:
  - `tests/reconstruct.rs::dual_build_oracle_deep_fixture_dec02` — the
    fixture no longer solves inside the test's 60 s / 64 MB budget
    (`proof-tree root was never realized` exit);
  - `tests/test_proof_validate.rs::dec02_end_to_end_validates_clean` —
    `assert_ne!(outcome, Draw)` at 60 s.
  A standalone CLI at 64 MB / 70 s also times out on dec02 under both
  versions (2.3.1: 895,827,072 evals; 2.4.0: 992,318,255 evals), so this is
  a stale slow-test fixture/search-regression question, not a 2.4.0
  regression. It is recorded here rather than silently repaired
  (scope discipline).
- Targeted slow tier: `test_move_order` 3/3, `test_trajectory_golden` 1/1.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo doc --no-deps` — green.

## Deliberately deferred

- **`EnPassantMode::Legal` / `legal_ep_square` as the default key mode.**
  This is the one feedback P1-6 item not consumed. Switching the solver's
  board hash/repetition key to `Legal` changes TT keys, TT snapshots, and
  repetition detection — a search-semantics change, not a mechanical API
  swap. It needs a dedicated A/B (transposition gain versus
  `legal_ep_square` cost on every `refresh_zobrist`) and a snapshot
  compatibility note. The upstream helper is available when that plan is
  cut.
- **Blast-helper refactor.** `blast_zone`/`blast_casualties` duplicate
  semantics the solver already re-derives in `ordering`, `static_tactics`,
  `quiet_group`, and `move_class`, but those call sites use ring (not
  centre-inclusive) shapes and hot-path value tables. A mechanical
  replacement would either add masks or change the aSEE arithmetic; it is
  not worth the regression surface in an adoption session.
- **`unsafe-optimizations` feature.** Left off. The project's audit stance
  (`avoid unsafe by default`) wins unless a controlled measurement justifies
  it.
- **Lazy `populate_state`.** Not requested by us; no consumer change.

### Missing coverage

- No proofdb shard fixture exercises the new validated-FEN rejection path; the
  policy split is unit-tested at `Position`, and the existing proofdb tests
  cover the accepted path.
- No timing test pins the `outcome_from_state` delegation cost; the search
  gate is behavioural only.
- `EnPassantMode::Legal` and the blast helpers have no consumer tests because
  they are deferred (above).

## Tools used

- `examples/reconstruct_pt` + a fresh `--tt-dump-path` snapshot for the dec10
  proof validation.
- `examples/verify_ppv` to confirm the TT PV is informational (it correctly
  refuted the line as a PPV; not used as proof evidence).
- A throwaway external probe crate for the UCI-lookup comparison (not
  committed).
- Unmodified `git archive HEAD` copies of the tree, built once against
  2.3.1 and once against 2.4.0, to separate upstream behaviour drift from
  this session's port (used for the dec02 slow-test triage).

## Problems encountered

- `Move::from_bits(0)` now returns `Some(Move::NONE)` by contract; the old
  local decoder happened to produce the same packed value, so the binary
  format is unchanged. Existing `binary.rs` tests already pinned this.
- The dec10 anchor drift is expected but required proof validation before
  re-pinning; the `--first-outcome` PV is informational and was not a valid
  PPV under the new search (`verify_ppv` correctly reported a shorter
  defence), so the evidence used is the reconstructed tree's validator, not
  the TT PV.
- The pre-existing dec02 slow-test failure was found only because the
  release-grade tier was run; it should be triaged separately (either a new
  anchor/budget or a search-diagnostic follow-up).

## Next steps

1. Triage the dec02 slow-test failure at HEAD (search trajectory/censoring
   vs stale 5 s fixture claim); decide whether to raise its budget, re-pin,
   or open a search-diagnostic plan.
2. If transposition pressure is a priority, cut a dedicated `EnPassantMode::Legal`
   A/B plan (keys + repetition + snapshot compatibility).
3. Optional cleanup: one `ring(sq)` wrapper for the four blast call sites,
   measured against the exact-score pins before landing.
