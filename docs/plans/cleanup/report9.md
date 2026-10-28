# report9 — `dfpn/children.rs`: `ChildInfo` constructors, one unsolved-bounds ladder, tests split out

Plan: `docs/plans/cleanup/plan9.md` (cleanup initiative).
Session date: 2026-10-28. Execution commit base: `3c7fab1`
(`finish cleanup plan8`); all work is a worktree diff on top.

## Outcome

All three phases landed in one session. The production file is a pure
refactor (no search decision, threshold, TT interaction, pre-probe order,
proof-event emission or pool change), and the test module moved to a
sibling file. Every identity bar is exact; the wall cells are within the
+2% bar.

- **Phase A** — added `impl ChildInfo` with `solved`, `unsolved`,
  `unevaluated`; rewrote all 11 literal sites from the plan's mapping
  table.
- **Phase B** — added `ChildInfo::unsolved_bounds`; both duplicated
  ladders now call it.
- **Phase C** — moved the test module verbatim (modulo dedent and two
  rustfmt line joins, see Findings) into
  `src/search/dfpn/children/tests.rs`; `children.rs` now ends in
  `#[cfg(test)] mod tests;`.

## Census

| metric | before | after |
| --- | --- | --- |
| `ChildInfo {` occurrences (incl. struct + fn signatures) | 15 | 5 |
| `repetition_seen: false` | 10 | 2 (the neutral constructors) |
| `.pn_dn_for(..).0` / `.1` pairs | 3 | 0 |
| unsolved-bounds ladder bodies | 2 | 1 (`unsolved_bounds`) |
| `#[test]` in the children module | 21 | 21 (moved) |

The plan text estimated 21 `repetition_seen: false` occurrences; the
actual HEAD count was 10 (21 was the combined `false`-flag count, since
each literal also has `explored: false`). The direction and conclusion
are unchanged.

## File split

| file | before | after |
| --- | --- | --- |
| `src/search/dfpn/children.rs` | 1,334 lines / 57,771 B | 624 lines / 29,210 B |
| `src/search/dfpn/children/tests.rs` | — | 649 lines / 24,155 B |

Bar D4 asked for the production file at ≤ 700 lines / ≤ 33 KB: pass
(624 / 29,210 B). The moved test text is identical modulo the dedent and
the two rustfmt reflows documented below; the `#[test]` name set is
unchanged (21 = 21).

## Phase evidence (D4 identity)

Read-only Phase 0 captured `HEAD` anchors; after the refactor the same
commands reproduce them exactly:

| anchor | expected | measured |
| --- | --- | --- |
| quick @ 256 | 59 cases, 42,109,768 child evals, 0 wrong | 59 / 42,109,768 / 0 |
| quick @ 128 | 59 cases, 40,042,745 child evals, 0 wrong | 59 / 40,042,745 / 0 |
| dec31 FO | `evals: 510426`, loss length 22 | identical |
| m22 FO | `evals: 15023911`, win length 75 | identical |
| m23_white FO | `evals: 11621376`, win length 27 | identical |
| m22 default stdout golden | pass without re-baselining | pass |

The quick JSON files and the parsed summary are under
`measurements/plan9/state/`.

### Wall

7 interleaved HEAD-vs-after repetitions, medians:

| cell | HEAD | after | ratio |
| --- | --- | --- | --- |
| dec01 FO | 0.9216 s | 0.9289 s | 1.0080 |
| dec05 FO | 0.0163 s | 0.0156 s | 0.9596 |
| m22 FO | 2.1037 s | 2.1228 s | 1.0091 |

All within bar D3's +2%. A-2 held: no `#[inline]` was added to the
constructors, and no wall-driven attribute change was needed.

## Gates

- `cargo fmt --check` — clean.
- `cargo clippy --all-targets` — clean.
- `cargo doc --no-deps` — clean, no new warnings.
- `make test` — green.
- `make test-full` — green (6m37s on this host), exit 0.

## Findings / deviations

1. **Rustfmt reflowed two moved test call sites.** The plan asked for a
   verbatim move. Dedenting the module makes two
   `evaluate_child_played(...)` argument lists fit the 100-column width,
   so `cargo fmt` (required by D5) joined them. This is formatting only;
   no assertion, fixture, or test name changed. The raw-vs-moved diff is
   exactly the `mod tests` wrapper removal plus those two reflows.
2. **The helper is an associated function on `ChildInfo`, not on
   `Search`.** Both call sites therefore spell
   `ChildInfo::unsolved_bounds(...)`; the plan's pseudo-code left the
   receiver implicit. Semantics are exactly the plan's `match`.
3. **The pre-probe call site gained the B-2 comment** stating that its
   enclosing branch already established `outcome.is_none()`, so the
   helper's guard is a provable no-op there. The played-path site passes
   `entry` directly (`Option<&TtEntry>`); both guard sets are preserved
   verbatim.
4. **No site differed from the plan's mapping table.** The two
   `pn_dn_for(..).0/.1` pairs collapsed into the `solved` constructor;
   the `unevaluated` site keeps `(INF, 0)` exactly, not `(1, 1)`.

## Unresolved / next steps

- `children/tests.rs` is 24.2 KB, slightly above the repo's ~20 KB
  "normally split" line. Plan C explicitly made the move-only split the
  end of scope; a future cleanup plan could split the pre-probe tests
  from the terminal/classifier tests if that file keeps growing. Not a
  blocker for this plan's bars.
- No test was added or changed. Existing coverage exercises every
  rewritten constructor site through `evaluate_child`; the unit suite and
  both full test tiers are green.
- Remaining drafted cleanup work: plan10 (`proof_tree` layer) and plan11
  (tooling DRY + owner-gated `reconstruct_pt --experiment` retirement).

## Tools / examples used

- `target/release/examples/benchmark --suite quick --first-outcome
  --timeout 3 --runs 1 --tt-size {256,128} --json`
- `target/release/atomic_solver` for the dec31/m22/m23 anchors, the m22
  default golden, and the wall cells.
- `git archive HEAD` into `/tmp/head-src` with a separate
  `CARGO_TARGET_DIR` to build the HEAD binary for interleaved timing.
- `make test` and `make test-full` for the gates.

Artifacts: `docs/plans/cleanup/measurements/plan9/` (`README.md`,
`env.json`, `state/`).
