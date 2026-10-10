# Lean Report 12 — #19 pre-pass keys via upstream `hash_after`

Executed 2026-10-10 per `plan12.md`, on top of HEAD `6328872` ("finish lean
plan11"). One lever: the plan11 prefetch pre-pass's
`do_move_with_scratch`/`undo_move_with_scratch` round-trip is replaced by
the non-mutating `Board::hash_after` / `Board::rule50_after` from
`atomic-movegen` 2.3.0. `cargo fmt --check`, `cargo clippy --all-targets`,
`cargo doc --no-deps` clean; `make test` (417 passed) and `make test-full`
(457 passed) fully green, 0 failures.

## Preconditions — all met

1. `report11.md` exists; the pre-pass is in `evaluate_all_children`.
2. `atomic-movegen` 2.3.0 is on crates.io (published 2026-09-10) with both
   functions and the planned equivalence tests; consumer-side notes written
   as `docs/plans/movegen/report_update_2.3.0.md` (same pattern as the
   2.2.0 report).
3. `report11.md` measures the pre-pass at ≈14% of wall (V12 probe; ≥ the
   ~5% bar) — no demotion.

## What landed

- `Cargo.toml`: `atomic-movegen = "2.3"`; `cargo update -p
  atomic-movegen` (2.2.0 → 2.3.0, the only dependency change).
- `src/position.rs`: `#[must_use] pub fn hash_after(&self, m: Move) -> u64`
  returning `self.board.hash_after(m) ^ zobrist::rule50_key(self.board
  .rule50_after(m))` — exactly the key `evaluate_child` probes (board hash ^
  rule50 key); `rule50_key` clamps at 100, matching `refresh_zobrist`.
- `src/search/dfpn/children.rs`: the pre-pass is now
  `for i in 0..moves.len() { self.tt.prefetch(pos.hash_after(moves[i])); }`
  — the local pre-pass `StateInfo` and its slot sentence are gone; the
  `ChildPrecompute` "Eval-scratch slots" docs updated (Boy Scout). No other
  change: probe/store/replacement, bucket layout, `ChildInfo`, and
  `evaluate_child` are untouched.
- `src/position.rs` tests: `hash_after_equals_do_move_hash_for_every_legal_move`
  (task 3, below).

## Key-equality test (task 3)

For startpos, m22, shuffle-win, an en-passant fixture (`… w KQkq f6 0 3`),
castling fixtures for both colors, a promotion-capture fixture (`2r5/1P6/…`),
and rule50 = 99 / rule50 = 100 clamp positions, every legal move satisfies
`pos.hash_after(m) == { let mut p = pos.clone(); p.do_move(m); p.hash() }`;
the test also re-checks `pos.fen()` per move (the non-mutation contract).
Green in both `make test` and `make test-full`, layered on top of upstream
2.3.0's own seeded-playout property test (8,592 positions × every
pseudo-legal move) and deterministic fixtures.

## Baseline capture (task 1) → `measurements/plan12/`

Plan11 binary (`6328872`, copied to `/tmp/plan12_base`, not kept):

- quick suite JSON (`--suite quick --json --first-outcome --timeout 3
  --runs 1`) → `quick_base.json`;
- m22 FO stdout md5 `8109ff0d…` (matches report11's anchor), wall 2.11 s;
- m22 default stdout md5 `b965d37c…`, wall 2.61 s — byte-identical to
  `tests/fixtures/m22_default_stdout_golden.txt`;
- shuffle-win FO stdout md5 `cfc58bc4…` (matches report11's anchor), wall
  34.41 / 34.69 s (×2).

Note: report11 quotes `49a08a6d…` for "m22 default", while this session's
`--timeout 60 --outcome-only` run reproduces the *golden fixture*
byte-identically (`b965d37c…`). The golden fixture is the pinned reference,
so the discrepancy is attributed to an unspecified difference in report11's
capture command (likely a different timeout), not to any drift; within this
session the baseline is self-consistent and golden-anchored.

## Drift protocol (task 4) — fully green

| check | result |
| --- | --- |
| quick suite, 59 cases (`child_evals`/`nodes`/`outcome`) | identical (total 38,974,090 both) |
| m22 default stdout md5 (×5 interleaved) | `b965d37c…` identical ×5/×5, == golden fixture |
| m22 FO stdout md5 (×5 interleaved) | `8109ff0d…` identical ×5/×5 |
| shuffle-win FO stdout md5 (×2 interleaved) | `cfc58bc4…` identical ×2/×2 |

The key change feeds only prefetch hints, so bit-identical search was the
expected outcome; the direct key-equality test (task 3) additionally pins
the key itself.

## Wall A/B (task 5) — above the −5–10% expectation

Sequential interleaved base/new runs, `date` deltas, stdout md5 verified
per run:

| workload | plan11 base | plan12 | Δ |
| --- | --- | --- | --- |
| m22 default ×5 | 2.61–2.71 s (mean 2.65) | 2.29–2.35 s (mean 2.32) | **−12.5%** |
| m22 FO ×5 | 2.12–2.14 s (mean 2.13) | 1.85–1.86 s (mean 1.86) | −12.9% |
| shuffle-win FO ×2 | 34.38 / 34.38 s | 30.04 / 30.16 s | **−12.6%** |
| quick suite ×1 (`time_mean` total) | 5.606 s | 4.828 s | −13.9% |

Consistent with report11's probe arithmetic (pre-pass ≈14% of wall): the
round-trip is replaced by a handful of XORs/table loads, so most of that
share is recovered.

### Pre-pass residual share (fresh leaf profile + probe, task 5)

- Fresh shuffle-win FO `--timeout 20` leaf table →
  `measurements/plan12/shuffle_fo_post_plan12_leaves.txt`:
  `dfpn` 32.1% (top leaf), `evaluate_child` 26.1%, `do_move` 9.1% +
  `undo_move` 4.2% (eval-only round-trips now), existence cluster
  (`compute_checkers` + `has_legal_move_with_state` + `populate_state` +
  `legal`) ≈22.6%, `generate_legal_with_state` 4.0%, sort leaves ≈3.9%,
  TT store 0.5%. `hash_after` has no own leaf — it is fully inlined into
  `dfpn`/`evaluate_all_children`.
- V12-style throwaway probe (pre-pass loop duplicated in a `/tmp` build,
  deleted after; stdout md5 unchanged `b965d37c…`): m22 default 2.316 →
  2.437 s (+0.121 s) ⇒ **pre-pass residual ≈5.2% of wall** (was ≈14%).

The #19 lever is spent. Next levers by the post-plan12 pie: the `dfpn`
frame loop itself (the `dfpn` initiative's algorithmic items) and the
existence cluster.

## Gates (task 6)

- `cargo fmt --check`: clean.
- `cargo clippy --all-targets`: 0 warnings.
- `cargo doc --no-deps`: 0 warnings (after fixing one intra-doc link to the
  private `Position::refresh_zobrist` — link syntax replaced by plain text).
- `make test`: green (417 passed, 0 failed).
- `make test-full`: green (457 passed, 0 failed; includes the m22/shuffle
  wall-clock regression suites and the trajectory golden).

## Docs (task 7)

- Lean `initiative.md`: status header (plans 1–12 done, host caveat
  extended), backlog #19 → done, new "Post-plan12 profile" subsection,
  history line for plan12.
- Movegen `initiative.md`: `plan_hash_after.md` arc entry updated (shipped
  2.3.0, consumed by lean plan12) + `report_update_2.3.0.md`.
- `measurements/plan12/README.md` + `env.json` provenance (per the AGENTS.md
  measurement layout); raw transcripts not kept.

## x86_64 compile check

Not needed as a separate step: this session's host *is* x86_64 (rustc
1.99.0) — the release build is compiled, executed, profiled, and gated
here. No new architecture-specific code was added (`hash_after` is portable
upstream code).

## Additional tools used

`perf record/report` (cpu-clock, leaf table), `python3` (quick-suite JSON
diffing, pre-pass loop patching for the V12 probe), throwaway `/tmp` builds
(baseline binary + doubling probe), `bc` (wall arithmetic).

## Problems encountered

- **`measurements/plan11/` is missing from the tree** despite being
  referenced by `report11.md` (its leaf tables and `env.json` are absent;
  git status is clean, so they were never committed). No impact on this
  plan: the baseline was re-captured fresh from the plan11 binary and the
  anchors match report11's md5s. Flagged as an unresolved item — if the
  plan11 artifacts still exist outside the repo they should be restored;
  otherwise `plan11_phase0/` + `plan12/` carry the record.
  **Resolved 2026-10-10 (follow-up session):** the artifacts were never
  missing — the plan11 session had committed them at repo-root
  `measurements/plan11/` (a non-conventional location, hence invisible
  here); they re-verified byte-identical against their raw sources and
  are now relocated to `docs/plans/lean/measurements/plan11/`, and
  report11/initiative references updated.
- A transient edit mishap: the first `ChildPrecompute` doc edit pasted a
  fragment of unrelated commentary; caught by immediate re-read and
  repaired before commit. No code path affected.
- `cargo doc` initially warned about an intra-doc link to the private
  `Position::refresh_zobrist`; resolved by un-linking.
- The quick-suite JSON capture on the very first attempt ran with the
  wrong binary path (empty file); re-run correctly, no impact.

## Unresolved / follow-ups

- ~~`measurements/plan11/` artifacts missing (see above)~~ — resolved:
  relocated to `docs/plans/lean/measurements/plan11/` (see the problems
  note above); the drift anchors are additionally re-derived in
  `plan12/`.
- The aarch64 reference host has not returned; plan11/plan12 percentages
  remain x86_64-host self-consistent A/B pairs only (standing caveat in the
  initiative status header).
- Phase-0 finding F4 (TT `with_mb` over-allocation, backlog #20) remains
  open, as planned.

## Task-list compliance

All eight plan tasks executed in order. No scope expansion; AGENTS.md
needed no new convention (none expected).
