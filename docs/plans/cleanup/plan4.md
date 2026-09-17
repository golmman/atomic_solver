# Cleanup Plan 4 — post-plan13 housekeeping: docs sync, AGENTS.md condensation, module sizing, test-gap closure

## Context and goal

Seven plans landed since the last housekeeping pass (lean 4–9, dfpn 10–13,
proof 4–7, conversion 1–5, egtb 1), including a whole new module
(`src/search/preflight/`). The accumulated debt is small but real:

- The `docs/plans/README.md` index has **stale rows** (egtb still lists
  plan1 as the next lever; dfpn still says "Session B next" although
  plan13 is closed).
- `src/search/dfpn/selection.rs` is at **19,855 bytes** — right at the
  20 KB soft limit the initiative has been watching.
- report13's "Missing tests" named two concrete gaps (the third,
  `cargo doc`, was verified clean on 2026-09-16 and is closed by this
  plan's gate).
- AGENTS.md has grown to **413 lines**; much of the Architecture section
  now duplicates module-level docs. Target: **≤ 200 lines**, keeping
  `## Conventions` intact.

No game-theoretic behavior changes. The plan is sized to one session.

Baseline: commit `2bca731` ("finish dfpn plan13"), working tree clean,
`make test` green (387/0 incl. ignored at plan13 close), `cargo doc`
warning-free.

## Decisions (agreed with the user)

1. **AGENTS.md is condensed, not rewritten**: keep `## Conventions`
   essentially verbatim; every other section may be shortened or
   restructured toward the ≤ 200-line target, subject to the
   no-normative-loss guardrail below.
2. **egtb pivot-vs-close** (pending since `egtb/report1.md`): the plan
   proposes **refocus-to-dormant** — the generator lives on as the
   `egtb_gen3` example with correctness-oracle duties (it already caught
   a solver defect and validated plan13's draw claims); the 4-man
   faster-solves goal stays closed. Any future 4-man work reopens the
   initiative with a fresh plan. The implementer confirms this one-liner
   with the user at session start before editing the rows.
3. **`selection.rs` tests are split now**, not when it crosses 20 KB —
   the split is mechanical and the file is at the limit; waiting buys
   nothing.

## Task 1 — Docs/bookkeeping sync

- `docs/plans/README.md`:
  - **dfpn row**: replace "Session B next" with the closed status
    (backlog #6 done, plan13, `report13.md`); open levers are #3
    (refinement after cap-cut) and the threshold-cut-frame observation.
  - **egtb row**: reflect plan1 done (NO-GO for faster solves) and the
    refocus-to-dormant status per decision 2.
- `docs/plans/egtb/initiative.md`: update `## Status` (plan1 done, no-go,
  refocused/dormant; next plan number **plan2** if ever reopened) and add
  a decision record line for decision 2. The backlog row's
  "pivot-vs-close decision pending" note is resolved accordingly; the
  open decisions (4-man coverage, DTZ granularity, #5 anchoring) stay
  recorded as reopen triggers.
- `docs/plans/cleanup/initiative.md`: move the addressed follow-ups out
  of "Open follow-ups", add plan4 to the done list (at report time).

## Task 2 — AGENTS.md condensation (≤ 200 lines)

Section-by-section guidance (current line ranges in parentheses):

- **Goal** (3–6): keep as is.
- **Architecture** (7–164, the big one): condense to a per-module
  overview of ~3–5 lines each (~40–50 lines total). **Guardrail:** any
  fact deleted must already be documented in the owning module's docs
  (`src/search/dfpn/`, `src/search/preflight/mod.rs`, `src/proof_tree/`,
  `src/main.rs` CLI doc, …) or must be moved there in the same session.
  Known AGENTS-only facts to relocate, not drop: the refine-cap factor /
  `--refine-cap` semantics, the `child_eval_budget` /
  `ExitReason::BudgetExhausted` contract, the preflight R2 gap (no
  `ProofEvent`s ⇒ offline pipeline cannot reproduce pre-phase proofs),
  the pre-phase closure as the bounded exception to "RAM = TT only",
  `PvStatus` variant meanings. Verify each lands in the module doc it
  belongs to; `src/main.rs` already carries most CLI facts.
- **Dependency direction** (165–172): keep, it is short and normative.
- **Examples** (173–222): one line per example (~15 lines total); the
  runnable-example list must stay complete.
- **Output priorities** (223–251): keep the priority order and the
  `PvStatus` semantics; condense prose around them.
- **Testing tiers** (252–271): keep — normative and already tight.
- **Profiling in this container** (272–305): keep — this section is the
  only in-repo record of the host-side launcher setup. Condense prose,
  keep the verified-what/when, the works/not-available lists, and the
  example session.
- **Conventions** (306–364): keep verbatim (decision 1).
- **Conversational Guidelines** (365–371): keep.
- **File size justifications** (372–397): keep the justifications for
  every file that exceeds a limit (the size rule requires them);
  condense wording. Add a line for `src/search/dfpn/selection.rs` if the
  post-split main file still exceeds 10 KB.
- **Tuning workflow** (398–413): condense to the contract essentials
  (~8 lines): evaluator role, `--json`, `child_evals` metric,
  `WRONG_PENALTY` dominance.

The result stays one file, same path; no facts may be moved into
`docs/spec/` (spec files are standalone external contracts; process
vocabulary belongs in `docs/plans/`).

## Task 3 — `selection.rs` module sizing

Split the `#[cfg(test)]` tests out of `src/search/dfpn/selection.rs`
(19,855 bytes) into `src/search/dfpn/selection/tests.rs` (precedent:
`src/search/ordering.rs` + `src/search/ordering/tests.rs`). No logic
changes; the non-test content stays byte-identical apart from the moved
block and any needed `use` adjustments. Record the post-split size in
the report.

## Task 4 — preflight test gaps (from report13 "Missing tests")

1. **`--no-preflight` CLI e2e test**: in `tests/test_cli.rs`, add a test
   on a gated ≤3-men FEN (e.g. the ladder root
   `8/2K5/k7/8/8/8/8/4Q3 w - - 0 1`): default run prints
   `preflight: decided …` and `pv_status: preflight-proof`; the
   `--no-preflight` run prints no `preflight:` line. Model the
   assertions on the existing mate-in-1 pin tests.
2. **Closure memory measurement**: run the ladder root under
   `/usr/bin/time -v` (max RSS, release build, default TT) and record the
   number in the report. If the measurement contradicts the documented
   "~60 MB on the ladder region / ~100–120 MB worst case" claims in
   AGENTS.md and `src/main.rs`, correct the docs to the measured value.
   No CI assertion — the budget stays a documented bound.

## Task 5 — pedantic clippy triage (carried from report2)

Run `cargo clippy --all-targets -- -W clippy::pedantic`, triage the
remaining warnings (documentation lints are the largest category):
fix the mechanical ones; explicitly allow the rest with a scoped
`#[allow(...)]` + one-line reason, or leave them unfixed if the churn
outweighs the value — record the counts and the decision in the report.
`clippy --all-targets` (default lints) must stay clean either way.

## Gates

- `cargo fmt --check`, `cargo clippy --all-targets`, `cargo doc` — all
  clean (the latter closes report13's `cargo doc` item for good).
- `make test` green.
- No search/source behavior change is intended; if any Task-5 lint fix
  touches non-test code, additionally run
  `benchmark --suite quick --json --first-outcome` before/after and
  confirm byte-identical results.
- AGENTS.md line count ≤ 200 with `## Conventions` unchanged
  (`git diff` shows no hunks inside that section).

## Out of scope

- `dfpn` #3, `proof` #8, `lean` #7/#10 — they stay with their
  initiatives; the next feature session is expected to be `proof` #8
  (PPV from the finalized tree), per the post-plan13 re-ranking.
- Any change to `docs/spec/` files.
- Search-behavior or PV-semantics changes of any kind.

## Final task

Write `docs/plans/cleanup/report4.md`: what was condensed where (with the
AGENTS.md section-by-section relocation table from Task 2 — which fact
went to which module doc), post-split file sizes, the measured closure
RSS, clippy triage counts, gate results, problems encountered, and next
steps. Update `docs/plans/cleanup/initiative.md` and the README rows as
part of Task 1 (done list at report time).
