# Report 7: Housekeeping pass — `dfpn/mod.rs` split, restart-schedule test gap, slow-test decisions

Initiative: `cleanup`. Executes `docs/plans/cleanup/plan7.md` (Phases 0–3).
HEAD at start: `e2127358476bf310e01d33cb5b79d7b448c905c4`
("add cleanup plan7"). Search semantics, thresholds, ordering, TT, the
repetition cache, the pre-phase, and the restart-schedule constants were
not changed; the plan is evaluable as "no result changed".

## Phase 0 — baseline capture

`git status --porcelain` was empty at session start (`docs/notes.md` was
clean, contrary to the plan's warning), so no stop-and-investigate event.
`cargo build --release` succeeded. All anchors reproduced exactly:

| anchor | expected | measured |
| --- | --- | --- |
| quick @ tt 256 | 42,109,768, 59/59, 0 wrong | 42,109,768, 59/59, 0 wrong |
| quick @ tt 128 | 40,042,745, 59/59 | 40,042,745, 59/59, 0 wrong |
| dec31 FO | 510,426, loss | 510,426, loss |
| m22 FO | 15,023,911, win | 15,023,911, win |
| m23_white FO | 11,621,376, win | 11,621,376, win |
| stress FO | 287,872,755, win | 287,872,755, win |
| m22 default | golden match, 18,779,890 | golden match, 18,779,890 |

`cargo test --release --test test_trajectory_golden -- --include-ignored`
passed. `state/functions_before.json` (48 `fn`s) and
`state/public_surface_before.txt` (36 `pub fn`s) were captured; Phase-0
wall medians of 3 and the single-run wall were written to `state/`.

## Phase 1 (A) — restart-schedule tests

Added the four tests exactly as specified:

- `restart_schedule::cutoff_fires_at_the_boundary_it_is_crossed` (A-1)
- `restart_schedule::first_slice_ignores_nonzero_cutoff` (A-2)
- `dfpn::tests::linear_schedule_restarts_end_to_end` (A-3)
- `dfpn::tests::cutoff_fires_mid_run_and_solves` (A-4)

Measured values (pinned in the tests):

| test | `restarts()` | `child_evaluations()` |
| --- | ---: | ---: |
| linear schedule (m22 FO, tt 256) | 7 | 17,411,547 |
| `SHIPPED` + cutoff 2 M (m22 FO, tt 256) | 3 | 16,967,609 |

`restarts()` matched the plan's structural model exactly (`>= 7`, `== 3`).
The cutoff run cost 16.97 M rather than moving all the way toward the
no-restart ~19.4 M; this is recorded as the measured trajectory, not a
failure — the plan pre-registered only the boundary counts.

A-5's `restarts()` doc line was added (a cutoff boundary still counts and
re-enters `dfpn` with `INF`/`INF`). A-6 cost guard: the two new tests
together add ~3.6 s (each ~1.8 s), below the ~5 s threshold, so both stay
in the fast tier. `make test` runs in ~42 s on the reference host.

**Bar D1: pass.** A-1…A-4 pass, `cargo fmt` clean, `make test` green
(36 suites, exit 0), fast-tier time 42 s.

## Phase 2 (B) — recorded decisions

- **B-1 `m21_white`**: stays excluded from the unproven assertion. The
  existing `tests/stress.rs` comment now states this is a deliberate
  decision, not a pending TODO: promoting it would add ~52 s /
  596,102,838 evals (~3.5% of the slow tier), while `dec31` (0.51 M),
  `m23_white` (11.6 M), and `m22_white_wins` (15.0 M) already cover the
  restart-sensitive class.
- **B-2 `m22_white_wins`**: 60 M budget / 256 MB TT stands; no change.
- **B-3 `AGENTS.md`**: added the `dfpn/{mod,config,run}.rs` and
  `dfpn/tests.rs` justifications.

**Bar D2: pass** (docs-only: `AGENTS.md` + `tests/stress.rs` comment).
`state/status_after_phase_b.txt` captured the D3-1(c) baseline.

## Phase 3 (C) — the split

Layout after the move:

| file | lines | bytes | contents |
| --- | ---: | ---: | --- |
| `src/search/dfpn/mod.rs` | 360 | 14,224 | module docs/`mod`s, consts, `ExitReason`/`Display`, `PvStatus`, `RoundTermination`, `Search` fields, `new`, proof-node emission, state/clock/path helpers |
| `src/search/dfpn/config.rs` | 273 | 10,508 | 29 configuration/accessor/exit methods |
| `src/search/dfpn/run.rs` | 379 | 17,061 | `bounded_search`, four entry points, `begin_run`, `preflight_phase`, `refinement_round_cap` |

Combined: 1,012 lines vs HEAD 987 + 25 added `mod`/`use`/`//!`/header
lines. `mod.rs` is 14.2 KB (D3-2 bar ≤15 KB), all three files are under
20 KB (largest 17.1 KB). The false "intentionally larger than 10 KB …
for performance" header paragraph was replaced by the truthful
change-reason rationale (`lto = true`, `codegen-units = 1` make module
placement codegen-neutral).

**D3-1 move-only: pass.** `verify_move.py --check` reports every one of
the 48 HEAD functions present exactly once with an identical normalized
body and an unchanged fn-name multiset:

```
ok   fn-name multiset unchanged (48 functions)
MOVE-CHECK: PASS
```

Only the two required visibility tokens changed (`bounded_search` and
`begin_run` → `pub(super)` for `dfpn/tests.rs`); `refinement_round_cap`
and `preflight_phase` stayed private in `run`. Phase 3 added no changed
path beyond `mod.rs`, `config.rs`, and `run.rs`.

**D3-2 layout: pass.** 36 `pub fn`s before and after; name-set diff empty
(a name-extraction diff was used because `rustfmt` wraps signatures, which
breaks a raw-line `grep` diff).

**D3-3 bit-identity: pass.** Quick suite at tt 256 and 128 reproduced
42,109,768 / 59 and 40,042,745 / 59 with zero `wrong`; the six anchors are
eval-identical; the m22 default golden and the trajectory-golden test pass
without re-baselining.

**D3-4 wall neutrality: pass under controlled comparison.** Absolute
post-split medians of 3 drifted +2.94% (stress FO) and +4.52% (m22
default) from the Phase-0 medians, i.e. outside the naive +2% bar. Because
the eval counts are bit-identical and the crate is one fat-LTO CGU, this
was treated as host drift and checked properly: a pristine HEAD binary was
built from `git archive HEAD` and run interleaved with the split binary,
5 runs each. Medians then differ by **−0.42%** (stress FO) and **+0.97%**
(m22 default) — inside +2%. No `#[inline]` or codegen help was needed.
Artifacts: `state/wall_before.json`, `wall_after.json`,
`wall_interleaved_head_vs_after.json`.

**D3-5 gates: pass.** `cargo fmt --check` clean; `cargo clippy
--all-targets` clean; `cargo doc --no-deps` no new warnings; `make test`
exit 0 (42 s); `make test-full` exit 0 (~8 min on this host).

## Tools used / problems encountered

- `measurements/plan7/verify_move.py` — custom move-only checker. Its
  first revision mistakenly included multi-line signature continuation
  lines in the body hash (so `bounded_search`'s visibility change tripped
  it); fixed to start the body after the opening brace, then the HEAD
  snapshot was regenerated before the check.
- `use super::*;` does **not** import a parent's private `use` aliases
  (only items/definitions), so `run.rs` needed explicit
  `use crate::search::preflight::{self, PreflightExit};` and
  `use crate::zobrist::INF;`. `config.rs` compiled with the glob alone.
  `mod.rs`'s now-unused `INF`/`preflight::{self, PreflightExit}` imports
  were removed.
- Wall-clock measurements in this container drift by a few percent
  between minutes; the interleaved HEAD-vs-working-tree protocol is the
  reliable D3-4 measurement.

## Unresolved / missing tests / follow-ups

- None outstanding for plan7. The split itself has no dedicated test; the
  bit-identity anchors, the golden, and the move-only checker are the
  guards, as planned.
- Remaining `cleanup` follow-up from earlier reports (the `M19_FEN` /
  `STARTPOS_FEN` duplicate) is unchanged and not in this plan's scope.
- `docs/plans/conversion/report12.md` was not edited; its closure thread
  is recorded complete here.

## Next steps

1. Keep the `dfpn/tests.rs` watch: revisit only if it crosses ~30 KB
   (currently ~26 KB) per the recorded decision.
2. No further action on `dfpn/mod.rs`; the size watch is cleared.
