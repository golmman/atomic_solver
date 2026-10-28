# Plan 9: `dfpn/children.rs` — `ChildInfo` constructors, one unsolved-bounds ladder, tests split out

Initiative: `cleanup` (housekeeping: DRY, YAGNI, lint, module sizing).
Opened by: the owner-requested code-size/readability review session
(2026-10-28). This plan touches the **hottest file in the program**
(`src/search/dfpn/children.rs`, every child evaluation goes through it),
so its acceptance bars are strictly identity- and wall-based, in the style
of `plan7.md`'s D3.

Plan numbering: successor to `plan8.md` (independent; either order works).
Depends on nothing and must be executed alone (no other `src/` diff in the
same session).

## Prerequisite reading (self-contained)

- `AGENTS.md` §"File size justifications" — `src/search/dfpn/children.rs`
  is already justified as oversized because "`ChildPrecompute` pooled
  frame movegen slots, existence-query terminal classification, the TT
  pre-probe …, TT reuse, and proof-event emission share one `Position`
  move/undo sequence". That justification covers the **production** part;
  it does not cover the file's inline test module.
- `src/search/dfpn/children.rs` — 1334 lines / 57.7 KB at HEAD:
  production lines 1–668 (31.8 KB), `#[cfg(test)] mod tests` lines
  669–1334 (666 lines / 25.9 KB). The production part is dominated by
  10 `ChildInfo { … }` literals written inline inside a nested
  classification ladder.
- `src/search/dfpn/selection.rs` + `src/search/dfpn/selection/tests.rs` —
  the repo's established pattern for a large module with a large test
  module (same for `ordering.rs`, `preflight/mod.rs`,
  `proof_tree/worker.rs`).
- `src/position.rs:60-77` — `Outcome::to_pn_dn` / `pn_dn_for(is_or_node)`;
  `zobrist::INF`.
- `src/search/dfpn/core.rs:420-430` — `resolved_from_entry`, whose
  `remaining_depth`/`depth` guards are why an entry that has an outcome
  may still be unusable (this matters for the helper in Phase B).

## Why this plan exists

1. **The `ChildInfo` literal can be written wrong, and it is written 10
   times.** Every literal repeats `repetition_seen: false,
   explored: false` (21 occurrences of `false` in the file are these two
   flags), and three sites compute the bounds by calling `pn_dn_for`
   **twice** and taking `.0` / `.1` in separate struct fields:

   ```rust
   pn: Outcome::Draw.pn_dn_for(child_is_or).0,
   dn: Outcome::Draw.pn_dn_for(child_is_or).1,
   ```

   (`children.rs:293-294`, `505-506`, `515-516`.) The compiler collapses
   the duplicate call, so this is a readability defect only — but it is
   exactly the kind of duplication that makes the ladder hard to audit.
2. **The "reuse unsolved bounds" ladder exists twice** (pre-probe path
   `children.rs:364-377`, played path `children.rs:628-641`) with the same
   five-part degeneracy guard and two copies of the same explanatory
   paragraph (the played-path copy adds the load-bearing
   `e.outcome.is_none()` conjunct).
3. **The file is 2.9× the "should normally be split" line (20 KB) and
   ~57 KB total**, of which 25.9 KB is a test module that the repo's own
   convention keeps in a sibling file.

## Scope

**In scope**

1. Phase A — `impl ChildInfo` with three constructors; rewrite the 10
   literal sites to use them.
2. Phase B — extract the unsolved-bounds ladder into one associated
   function used by both sites.
3. Phase C — **move-only** split of `#[cfg(test)] mod tests` into
   `src/search/dfpn/children/tests.rs`.

**Out of scope (non-goals)**

- No change to any search decision, threshold, TT interaction, pre-probe
  order, proof-event emission, or the `ChildPrecompute` pool.
- No change to the nested ladder's *structure* (the rule50 / repetition /
  extinction / terminal ordering is load-bearing and documented in place;
  this plan does not touch the ordering).
- No new `pub` item. `ChildInfo` is `pub struct` inside the private
  `children` module; the constructors are private (`fn`, not `pub fn`).
- No further split of the remaining ~29 KB production file (the
  `AGENTS.md` justification stands; revisit only if a future edit pushes
  it past ~35 KB).
- No touch of `core.rs`, `selection.rs`, `children`'s callers.

## Normative design

### Phase A — constructors

```rust
impl ChildInfo {
    /// A child whose outcome is decided: bounds derived from the outcome at
    /// the child's side (`child_is_or` is the *child's* node kind, i.e.
    /// `!is_or_node` of the parent).
    fn solved(mv: Move, outcome: Outcome, depth: u32, repetition_seen: bool, child_is_or: bool) -> Self {
        let (pn, dn) = outcome.pn_dn_for(child_is_or);
        Self {
            mv,
            pn,
            dn,
            outcome: Some(outcome),
            depth,
            repetition_seen,
            explored: false,
        }
    }

    /// A non-terminal child carrying TT bounds (or the neutral `(1, 1)`).
    fn unsolved(mv: Move, pn: u64, dn: u64) -> Self {
        Self {
            mv,
            pn,
            dn,
            outcome: None,
            depth: 0,
            repetition_seen: false,
            explored: false,
        }
    }

    /// A move that was never evaluated: the early-decisive cut in
    /// `evaluate_all_children` fills the remaining slots with it (OR-side
    /// neutral bounds, no outcome).
    fn unevaluated(mv: Move) -> Self {
        Self {
            mv,
            pn: INF,
            dn: 0,
            outcome: None,
            depth: 0,
            repetition_seen: false,
            explored: false,
        }
    }
}
```

Site-by-site mapping (HEAD line → replacement; the *semantics* column is
the contract, verify it per site while editing):

| HEAD lines | site | replacement |
| --- | --- | --- |
| 173–181 | early-decisive placeholder, `pn: INF, dn: 0` | `ChildInfo::unevaluated(moves[j])` |
| 291–298 | path repetition | `ChildInfo::solved(mv, Outcome::Draw, 0, true, child_is_or)` |
| 326–344 | pre-probe, outcome-less entry without hint | `ChildInfo::solved(mv, resolved.outcome, resolved.depth, false, child_is_or)` |
| 371–386 | pre-probe, live unsolved entry | `ChildInfo::unsolved(mv, pn, dn)` (pn/dn from Phase B) |
| 480–491 | played path, hinted solved entry | `ChildInfo::solved(mv, resolved.outcome, resolved.depth, false, child_is_or)` |
| 503–511 | played path, own commoner extinct (`Loss`) | `ChildInfo::solved(mv, Outcome::Loss, 0, false, child_is_or)` |
| 513–521 | played path, opponent extinct (`Win`) | `ChildInfo::solved(mv, Outcome::Win, 0, false, child_is_or)` |
| 538–547 | played path, rule50 terminal | `ChildInfo::solved(mv, outcome, 0, false, child_is_or)` |
| 566–575 | played path, resolved entry | `ChildInfo::solved(mv, resolved.outcome, resolved.depth, false, child_is_or)` |
| 617–626 | played path, terminal (existence query) | `ChildInfo::solved(mv, outcome, 0, false, child_is_or)` |
| 650–658 | played path, unsolved bounds | `ChildInfo::unsolved(mv, pn, dn)` (pn/dn from Phase B) |

Rules:

- A-1. `INF` must remain the same constant (`crate::zobrist::INF`) used
  today; `unevaluated`'s `(INF, 0)` pair is the OR-side neutral value and
  must not be "simplified" to `(1, 1)`.
- A-2. Constructors are plain `fn`s (not `#[inline]`-annotated): the
  release crate is one CGU with fat LTO (`Cargo.toml`), so an in-crate
  call boundary is not an inlining barrier; **do not add `#[inline]`** —
  if D3 shows a wall regression, the correct response is to
  `#[inline(always)]` the constructor and re-measure, and to record it
  (this is what performance plan3 did for its pre-probe fallback).
- A-3. `ChildInfo::default` is **not** introduced (the fields' neutral
  values differ per site; a `Default` impl would invite exactly the
  copy-paste the constructors remove).

### Phase B — one unsolved-bounds ladder

```rust
    /// Bounds to reuse from a live, outcome-less TT entry, or the neutral
    /// `(1, 1)`.
    ///
    /// Only non-degenerate entries are reusable: a previous work-bounded
    /// search may have stored a candidate terminal-like bound (`pn == 0 ||
    /// dn == 0`) without an outcome, and propagating such values can trick
    /// the parent search into treating an unproven node as solved. A
    /// depth-suppressed entry (`remaining_depth == u32::MAX`) and an entry
    /// solved deeper than the current bound are equally unusable.
    fn unsolved_bounds(entry: Option<&TtEntry>, child_max_depth: u32) -> (u64, u64) {
        match entry {
            Some(e)
                if e.outcome.is_none()
                    && e.pn > 0
                    && e.dn > 0
                    && e.remaining_depth != u32::MAX
                    && e.remaining_depth <= child_max_depth =>
            {
                (e.pn, e.dn)
            }
            _ => (1, 1),
        }
    }
```

- B-1. The played-path site (`children.rs:628-641`) passes
  `entry.as_ref()` and needs no guard change: its `e.outcome.is_none()`
  conjunct is preserved by the helper.
- B-2. The pre-probe site (`children.rs:364-377`) passes `entry.as_ref()`
  too. Its enclosing branch already established
  `entry.as_ref().is_some_and(|e| e.outcome.is_none())`, so the helper's
  extra conjunct is **provably true there** — the extraction is
  semantics-preserving. Assert this in a code comment at the call site
  ("the enclosing branch established `outcome.is_none()`; the helper's
  guard is therefore a no-op here") so a future reader does not "simplify"
  one of them away.
- B-3. The two duplicated explanatory paragraphs collapse into the
  helper's doc comment; each call site keeps a one-line comment saying
  *why* it consults bounds (`children.rs`'s ladder documentation stays).

### Phase C — tests split (move-only)

- C-1. Create `src/search/dfpn/children/tests.rs` containing the current
  lines 669–1334 **verbatim** (the `mod tests { … }` body, dedented by one
  level), and replace lines 669–1334 in `src/search/dfpn/children.rs`
  with:

```rust
#[cfg(test)]
mod tests;
```

  (Exactly the `selection.rs:355-357` / `preflight/mod.rs:67-69` idiom.)
- C-2. The moved module keeps `use super::*;` plus its existing
  `use crate::position::Position; use atomic_movegen::board::StateInfo;
  use atomic_movegen::types::Square;` header; visibility of the test
  module stays private.
- C-3. No test body changes in Phase C (if Phase A/B renamed nothing
  public, nothing needs editing — `ChildInfo` field reads in tests keep
  working because the fields are unchanged).

## Phase 0 — baseline capture (read-only)

From the drafting session at commit `7c339bb` (re-verify; recap the
anchors exactly as `plan7.md` §Phase 0 does):

| anchor | command | expected |
| --- | --- | --- |
| quick @ 256 | `./target/release/examples/benchmark --suite quick --first-outcome --timeout 3 --runs 1 --tt-size 256 --json` | 59 cases, total `child_evals` **42,109,768**, 0 wrong |
| quick @ 128 | same with `--tt-size 128` | total **40,042,745**, 59 cases |
| dec31 FO | `--fen 'r1bq1k2/ppN3p1/2p1p2P/6P1/1b1P1p2/8/PP3P2/R2K1B1R b - - 0 15' --timeout 20 --first-outcome` | `evals: 510426` |
| m22 FO | `--fen '4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22' --timeout 60 --first-outcome` | `evals: 15023911` |
| m23_white FO | `--fen '4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23' --timeout 60 --first-outcome` | `evals: 11621376` |
| m22 default | `--fen <m22> --timeout 20 --outcome-only` | stdout == `tests/fixtures/m22_default_stdout_golden.txt` |

Additionally capture, into `measurements/plan9/state/`:

1. `wc -l -c src/search/dfpn/children.rs`.
2. `grep -c 'ChildInfo {' src/search/dfpn/children.rs` and
   `grep -c 'repetition_seen: false' …` (the duplication census: 15 and
   21 at HEAD).
3. Wall medians of 3 (interleaved HEAD-vs-after where possible) for two
   cheap decisive cells (both measured in the drafting session):

   | cell | command | cost at HEAD |
   | --- | --- | --- |
   | dec01 FO (deep) | `--fen 'r5r1/5N1k/2p2p2/pp1p3p/3Pp3/2P1P3/P7/2bQ1R1K w - - 0 30' --timeout 20 --first-outcome` | 6,021,944 evals, ~0.92 s |
   | dec05 FO (ladder-dense) | `--fen 'rnbqkbnr/pppp1ppp/4p3/8/7q/NP6/P1PPPPPP/R1BQKBNR w KQkq - 1 3' --timeout 20 --first-outcome` | 17,088 evals, ~17 ms |

   Do **not** use stress FO (~300 M evals, ~90 s) or the m19 case (it
   times out at 20 s even with `--no-preflight`, 152 M evals — measured),
   and do not rely on the pre-phase for these cells (they are outside its
   ≤3-men detector, so it never runs).

## Execution phases and bars

**Bar D1 (Phase A)** — `ChildInfo {` literal count in `children.rs`
drops from 15 to ≤ 6 (the 3 constructor bodies + the struct definition +
any test literal); zero occurrences of the `.pn_dn_for(..).0`/`.1` pair;
`cargo fmt` + `clippy` clean; **D4 identity holds**.

**Bar D2 (Phase B)** — exactly one ladder body (the helper); both call
sites pass `entry.as_ref()`; `cargo fmt` + `clippy` clean; **D4 identity
holds**.

**Bar D3 (whole plan)** — the quick suite at both TT sizes reproduces
Phase 0 exactly (42,109,768 / 40,042,745, 59/59, zero wrong); dec31,
m22 FO and m23_white FO are eval-identical (510,426 / 15,023,911 /
11,621,376); the m22 default-mode golden passes **without**
re-baselining; the wall medians of dec01 FO, dec05 FO and m22 FO are
within +2% of Phase 0 (a wall bar, not a metric, because this is the hot
path).

**Bar D4 (Phase C)** — `src/search/dfpn/children.rs` ≤ 700 lines and
≤ 33 KB; `src/search/dfpn/children/tests.rs` — the moved text is
identical modulo the dedent (verify by normalizing leading whitespace and
diffing against `git show HEAD:src/search/dfpn/children.rs` lines
669–1334); the test-name set is unchanged (`grep -c '#\[test\]'` equal
before/after across the two files); `make test` green.

**Bar D5 (gates)** — `cargo fmt --check`, `cargo clippy --all-targets`,
`cargo doc` (no new warnings), `make test` green, and `make test-full`
green (`children`'s tests are cheap; the full tier is required because the
file's `evaluate_child` unit tests sit next to search-semantics
regressions).

**Rollback rule** — if D4 identity fails at any phase, fix forward only
if the failure is a mechanical transcription slip at the offending site
(compare against `git show HEAD:src/search/dfpn/children.rs`); otherwise
revert that phase entirely. Do **not** re-baseline the golden or an
anchor. If D3's wall bar fails, add `#[inline(always)]` to the
constructors (A-2), re-measure, and record the precedent
(performance plan3 did this for the pre-probe fallback).

## Tests to add / change

- None added. No test body is edited (Phase C moves them; Phase A/B are
  covered by the existing 666 lines of `evaluate_child`/`ChildPrecompute`
  unit tests, which exercise every constructor site through
  `evaluate_child`'s public-in-module entry).
- If a moved test fails to compile because it constructed a
  `ChildInfo { … }` literal directly, rewrite that literal with the new
  constructors **only if the semantics are identical**, and note it in
  the report.

## Measurement artifacts

`docs/plans/cleanup/measurements/plan9/`: `README.md`, `env.json`,
`state/` (line/census counts, anchors, wall medians). No snapshots
(rename any `.tt` produced to an empty `.keep` tombstone or delete it
before committing).

## Final task

Write `docs/plans/cleanup/report9.md`: the census before/after
(`ChildInfo {` literals, `repetition_seen: false`, `pn_dn_for(..).0`,
duplicated ladder bodies), the file split's byte/line numbers, the
phase-by-phase D4 identity evidence (quick suite + three anchors + golden),
the wall medians, and any site whose semantics differed from the plan's
mapping table (that is a finding, not a silent fix).
