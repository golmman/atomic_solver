# Report 9 — Portfolio ship shape: `examples/portfolio` driver (backlog item #11)

Executes `plan9.md` (2026-10-12). Zero product-code changes: `git diff
--exit-code src/` asserted at stage 0, re-asserted below. The plan8
campaign record stands as the lever's evidence; this plan shipped the
driver and validated it against two of its deterministic anchors.

## What landed

- **`examples/portfolio.rs`** (new, ~24 KB with the in-file unit-test
  module; documented justification in the header — the `RaceTracker`
  state machine and its tests share private types, same pattern as
  `src/search/ordering.rs`): launches K unmodified solver racers (default
  salts `0,2,3,5`, `--salts` overridable) at the fixed plan8 racer flags
  (`--first-outcome --outcome-only` + `--salt`), consumes stderr
  (`outcome:` progress lines, `evals:`) and stdout (`outcome:`, `pv:`,
  `pv_status:`, `budget exhausted`) concurrently, and implements:
  - **Stop-when-proven kill policy** (report8's cost finding): at the
    first decisive `outcome:` line from any racer, that racer runs to
    natural completion and all remaining racers are SIGKILLed
    immediately — no grace period.
  - **plan8 censoring discipline in code**: `budget exhausted` →
    `censored=budget`, never reported as a proven draw; wall-killed
    racers' evals are excluded from the decision; natural-exit proven
    draws are legitimate `none` results.
  - Output grammar per the plan contract (racer lines sorted by salt,
    `portfolio:` summary, winner's `pv:`/`pv_status:` echoed verbatim;
    exit 0 on decisive, 1 on `none`).
  - Solver discovery via the cargo layout + `--solver` override;
    `--timeout`, `--budget`, `--tt-size` pass-throughs.
  - **Code hygiene for #6/#7**: the race logic is split into separable
    functions — racer-set/launch (`spawn_racer`), stream consumption
    (reader threads), first-decisive selection and accounting
    (`RaceTracker`: `on_line`/`on_exit`/`mark_killed`/`records`) — so
    the cooperative portfolio (#6, shared-facts table) and
    any-worker-can-finish SPDFPN (#7) extend the same skeleton without
    a rewrite. The kill decisions are returned by the tracker, not
    embedded in the event loop.
- **`tests/portfolio_smoke.rs`** (new, fast tier): the two fidelity smoke
  tests described below. Locates the driver via `current_exe()` (fails
  loudly — absence is a gate defect) and the solver via
  `env!("CARGO_BIN_EXE_atomic_solver")`.
- **Docs**: AGENTS.md examples-list entry (one line); initiative.md item
  #11 → closed; `docs/plans/README.md` `parallel` row ship-shape note.
- **Measurements**: `measurements/plan9/` (`env.json`,
  `state/m20_white_race.json`, provenance `README.md`); raw transcripts
  under `logs/` (gitignored, never committed).

## Gate

`cargo fmt` clean, `cargo clippy --all-targets` clean, `make test`
green (33 test binaries, 0 failures; the two smoke tests add ≈ 0.5 s).
9 unit tests in the example cover: salt-list parsing (defaults +
malformed), arg parsing, first-decisive kill, budget-censored-never-a-
proven-draw, wall-killed evals exclusion, natural proven draw,
malformed-exit → `error` record (no panic), later-decisive also-ran
records, verbatim PV retention. `src/` diff after the plan: **zero
lines**.

## Smoke / validation results vs the plan8 anchors

| check | anchor (plan8) | driver result | verdict |
| --- | --- | --- | --- |
| dec10 smoke (in `make test`) | winner salt 2, evals 2,918,636, `win` | identical, exit 0, wall ≈ 0.5 s | **exact** |
| dec10 censoring path | budget-censored never a proven draw | all 4 racers `draw censored=budget`, `outcome=none`, exit 1 | **exact** |
| m20_white operator validation (stage 3) | winner salt 5, evals 791,401,553, `win`, exit 0 | identical, exit 0 | **exact** |

Wall times are timing-dependent by design (m20 race 64.3 s vs plan8's
95.89 s median under lighter ambient load); the evals anchors are the
fidelity gate and both reproduced bit-exactly, consistent with plan8
H4b (racers deterministic and concurrency-independent).

**Stop-when-proven measured.** The m20 validation race cost ≈ 248 core-s
(4 × 64 s wall, `user` 4m07s) vs plan8's ~747 core-s per race with the
240 s grace — the report8 stress-cost finding (543 of 606 core-s) is now
reclaimed by policy: three racers were killed at first-decisive.

## Deviations

None against the deviation policy (fixed racer flags, kill policy, and
output grammar all landed as specified). Implementation findings:

1. **Watch-exit mechanism**: a blocking `Child::wait()` in a watcher
   thread would deadlock `kill()` on the same mutex; the watcher polls
   `try_wait()` every 2 ms and reports `Exited` after the loop (also on
   a poisoned lock, so the event loop always settles).
2. **`outcome:` provenance**: decisive detection accepts the line from
   either stream — the stderr progress hook fires at the moment the root
   is proven (earliest stop-when-proven signal), the stdout line at
   natural completion. The plan said stderr for `outcome:`, stdout for
   `pv:`/`pv_status:`/`budget exhausted`; accepting both streams for
   decisiveness only ever kills *earlier*, never changes who wins, and
   the winner's evals anchor is unaffected (verified exactly).
3. **Killed-racer wall semantics**: a killed racer keeps its kill-time
   wall (the exit-confirmation event arrives a few ms later and does
   not overwrite it); locked by a unit test.

## Follow-ups

- **#6 (cooperative portfolio) / #7 (any-worker-can-finish SPDFPN)**:
  the driver's `RaceTracker` (events → kill decisions → records) is the
  intended harness skeleton — a shared-facts table slots in as an
  additional event source; any-worker-can-finish changes only the
  winner-selection rule. Their pre-registrations remain separate plans
  that read plan8's draw-luck regime.
- **Option-3 door (`--race K` product flag) stays open but shut**:
  revisit only if #6/#7 produce a genuinely scalable mechanism; the
  racer set {0, 2, 3, 5} remains a PoC-scale pre-registered constant.
- The driver is not spec'd (part of no contract in `docs/spec/`), per
  plan.
- `make test-full` not required by the plan (no search/ordering/TT/
  proof-tree change; `src/` diff zero) — the fast gate plus the two
  anchor-reproducing smoke tests are the complete validation surface.

## Effort

Single session: implementation + unit tests ≈ 40 min, gate ≈ 5 min,
m20 operator validation 64 s, docs/report ≈ 20 min.

SESSION COMPLETE
- plan9.md executed: `examples/portfolio.rs` + `tests/portfolio_smoke.rs` landed; gate green (fmt/clippy/`make test`, 33 binaries); zero `src/` diff; both plan8 anchors reproduced exactly; m20_white validation race parsed and recorded under `measurements/plan9/`; AGENTS.md, initiative.md (#11 closed), `docs/plans/README.md` updated; report9.md written
Follow-up options:
  1. "Execute backlog #6: draft plan10 for the cooperative portfolio (private per-worker TTs + shared solved-facts table) extending the `RaceTracker` skeleton, pre-registration reading plan8's draw-luck regime" — the harness this plan shipped is its ready-made skeleton.
  2. Alternative: defer #6/#7 and close the `parallel` initiative here (the salt portfolio is shipped and validated; #6/#7 only harvest cross-racer facts and bad tails, both speculative) — a clean stopping point if the owner wants to bank the win.
