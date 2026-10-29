# Initiative: `provari` — adopt provari's search ideas, step by step

## Status

**Active (opened 2026-10-29).** The first plan is
[`plan1.md`](plan1.md): a reproducible head-to-head baseline and target
corpus. No product code is changed by plan1.

This initiative owns the **search-performance** side of the external
comparison with [`aiorla/provari`](https://github.com/aiorla/provari). The
existing [`interop`](../interop/initiative.md) initiative owns independent
proof validation/interchange and remains the home of the external checker
oracle. Provari is GPL-3.0-or-later; this initiative imports **ideas**, never
code, binaries, networks, or models.

## Goal

Make `atomic_solver` **faster than provari** on a fair, reproducible target
corpus while keeping every existing correctness contract.

Provari is a specialized Atomic win-prover: attacker-only proof search,
atomic-specific move classes and static tactics, an evaluation-scaled
proof-number initialization, quiet-move grouping, and an optional learned
provability model. Our solver is a general DF-PN+ game solver with a
material/positional move scorer.

The comparison is deliberately narrow and honest:

- **Latency** on cases both solvers prove: wall time and work to the first
  decisive outcome (our `--first-outcome`, provari's default `prove`; no PV
  shortening on either side).
- **Coverage** on the same corpus: number of positions won within the same
  time budget.
- **Correctness first**: every claimed win must remain sound; outcome
  regressions or a false win veto an adoption regardless of speed.

The eventual target is: on the shared-win set, geometric-mean wall
`atomic_solver / provari < 1.0`, and coverage at least as high as provari's,
with all drift/soundness gates green. A negative result is a valid outcome
for any step.

## Why this initiative exists

Opening measurement (2026-10-29, ad hoc, `/tmp`; plan1 makes it reproducible
and committed):

| Run on `r1b1kbnr/ppp3pp/n4q2/3pppN1/1P1PP3/8/P1P2PPP/RNBQKB1R w KQkq - 0 1` | Wall | Work | First win |
| --- | ---: | ---: | ---: |
| provari 0.1.0, bundled model | 0.33 s | 169,485 nodes | mate 13 |
| provari 0.1.0, hand eval | 0.49 s | 342,289 nodes | mate 17 |
| provari 0.1.0, no eval | 0.91 s | 670,136 nodes | mate 16 |
| `atomic_solver --first-outcome` | 39.6 s | 11.77 M nodes / 305.7 M child evals | win 41 |

The gap is not a constant-factor Rust problem. Even provari without its
neural model is ~80× faster on this position. The dominant differences are
algorithmic:

1. **Move-class starting proof numbers.** Provari starts checks at the number
   of surviving evasions, double threats at 1, threats at 4, quiet moves at
   9, and cheaply-blowable quiet moves at 15. Our unresolved children start at
   `(1, 1)` (`src/search/dfpn/children.rs`), so DF-PN+ has almost no
   tactical signal beyond move order.
2. **Evaluation-scaled proof numbers (EF-PN).** Provari scales new children
   by a hand evaluation or a learned provability model. Our scorer orders
   moves but never prices the subtree.
3. **Static atomic tactical closure.** Provari settles immediate explosions,
   exposed kings, no-safe-evasion checks, double threats, mates in one, and
   cannot-win material without expansion. We only detect terminal positions.
4. **Quiet-move grouping.** Provari keeps all plain attacker moves as one
   delayed child and opens the group only if needed; we generate and score
   every legal move.
5. **Sibling proof replay / simulation.** Provari replays a proven defence or
   threat on sibling positions.
6. **Rule50-free transposition key + proof length/limit.** Provari shares a
   board across clocks and records the proof's length and the highest clock
   at which it holds; our key includes the halfmove clock.
7. **Attacker-only semantics.** Provari does not carry our general
   draw/loss/refinement machinery in the same proof search.

These are multiplicative, not alternatives. A temporary provari ablation
flattening only the starting numbers was still 2.3× slower; disabling only
quiet grouping was 1.4× slower; removing the guide was 1.8× slower. A single
static shortcut prototyped in our solver (immediate-explosion detection) did
not move the 40 s needle. The plan sequence below therefore builds a
purpose-designed win search and adds the levers together, measuring each.

## Non-goals

- **No provari code, dependency, binary, network, or model in this repo.**
  No `Cargo.toml` entry, no vendored GPL source, no committed oracle binary.
- **No tablebase integration.** Atomic Syzygy belongs to `egtb`; the baseline
  is tablebase-free so the comparison is like-for-like. A later plan may add
  tablebases only as an explicit, separately-measured option.
- **No NNUE training or product dependency on a learned model.** A model may
  be evaluated as an optional, licensable external artifact in a later plan;
  the hand-evaluation path must stand on its own.
- **No weakening of soundness.** Repetition, rule50, terminal, and proof
  contracts stay intact. A faster false win is a failure.
- **No general-solver outcome regressions.** Behavior-changing levers use the
  existing drift protocol, quick suite, repetition soundness gate, and
  anchors.
- **No parallel/portfolio work** (owned by `parallel`), **no proofdb/proof-tree
  format work** (owned by `proofdb`/`proof`), and **no unrelated
  micro-optimization** (owned by `performance`/`lean`).

## Working agreement

1. **Measure before planning.** Every plan starts from the committed baseline
   and a read-only probe; sizing spikes are temporary and reverted.
2. **One lever per plan.** A plan may add one mechanism (or one deliberately
   coupled pair, stated up front), not a bundle.
3. **Correctness gates are vetoes.** `make test` by default; `make test-full`
   for search/TT/proof changes. Behavior-changing plans must pass the drift
   protocol on unaffected cases, the quick suite, and the repetition
   soundness gate (`tests/test_repetition.rs -- --include-ignored`).
4. **Deterministic budget contract.** `child_eval_budget` and
   `ExitReason::BudgetExhausted` semantics stay exactly as documented. New
   front-ends must account their evals in the same counters or document why
   they are outside the budget.
5. **Fallback, not replacement.** A provari-style win front-end may return a
   proven win early; when it does not, the existing solver runs unchanged.
   The general solver remains authoritative for draws/losses and proof
   construction.
6. **Fair comparison.** Same host, release builds, one thread, no
   tablebases, same time budget, first decisive outcome on both sides.
   Provari runs from `/tmp`; only drivers and parsed results are committed.
7. **Negative results are recorded.** A lever that loses its gate closes with
   the measured evidence; it is not silently retained behind a default.

## Backlog (stepwise, re-ranked after each report)

Plan numbers are sequential and independent of item numbers. Each item is a
candidate step; a failed item does not block later items unless its report
says so.

| # | Item | Mechanism | Why / target | Metric | Status |
| --- | --- | --- | --- | --- | --- |
| 1 | Head-to-head baseline and target corpus | Reproducible driver for provari (pinned commit, bundled model and hand control) vs `atomic_solver --first-outcome`; core FEN plus a fixed sample of `bench/high200.jsonl`; parsed results and provenance | Establish the target and stop ad hoc comparisons; identify the shared-win set | wall, nodes/child evals, coverage | **planned — `plan1.md`** |
| 2 | Win-only front-end skeleton | Opt-in attacker-only DF-PN/win prover that proves a win and otherwise falls back; no new heuristics yet | Isolate provari's attacker-only proof semantics from our general outcome/PV machinery; provide the home for later levers | correctness (fallback identity), evals, wall | open (gated by #1) |
| 3 | Move-class starting numbers + quiet grouping | Checks by surviving evasions; double threat 1, threat 4, quiet 9, hanging 15; one delayed plain-move group | Provari's two largest non-eval levers; measured 2.3× and 1.4× in provari ablations | evals, first-win length | open |
| 4 | Static atomic tactical closure | Immediate exploding capture, exposed king, no-safe-evasion check, double threat, mate-in-one, cannot-win material | Avoid whole expansions on forced tactics; the complement to #3 | evals, expansion census | open |
| 5 | Evaluation-scaled proof numbers (EF-PN) | Hand evaluation first; optional learned provability model only if licensing and evidence justify it | Provari's hand guide is 1.8×; the bundled model another ~1.5× and a shorter first line | evals, first-win length | open |
| 6 | Rule50-free key + proof length/limit | Key boards without the clock; store proof length and highest holding clock; replay/verify conservatively | Recover move-order transpositions without breaking rule50/repetition soundness | TT hit rate, evals | open (soundness-sensitive) |
| 7 | Head-to-head convergence | Tune the adopted stack on the target corpus; require shared-win wall < provari and coverage ≥ provari | The initiative's success condition; no new mechanism, only measured tuning and final evidence | wall ratio, coverage | open |

Cross-references: `interop` owns the independent checker and proof
interchange; `dfpn` remains the general DF-PN semantics initiative and is
dormant. If a lever changes general DF-PN semantics rather than only the
win front-end, it must also satisfy `dfpn`'s soundness/measurement
conventions.

## Measurement conventions

- **Primary metric:** `child_evals` to the first decisive outcome
  (deterministic) and wall time (the product metric); nodes/expansions are
  secondary and are not compared one-to-one across implementations.
- **Core case:** the FEN above.
- **Corpus:** the fixed sample defined by plan1 from provari's
  `bench/high200.jsonl`, plus any cases added by a later plan with a recorded
  selection rule. Do not silently change a committed corpus.
- **Budgets:** 10 s and 192 MB for both sides unless a plan states otherwise.
  Provari: `prove --seconds 10 --memory 192` (bundled model default). Our
  solver: `--timeout 10 --first-outcome --tt-size 192`.
- **Repeats:** three runs for the core case and any reported wall ratio;
  one run per sample case, with censoring/timeouts reported, never dropped.
- **Correctness:** `make test` for docs-only plans; `make test` plus the
  quick-suite drift and repetition gate for search changes. A sampled proven
  win must replay/validate before a lever is declared adoptable.
- **Artifacts:** drivers, corpus definitions, parsed results (`*.json`,
  `*.csv`), and provenance READMEs are committed under
  `measurements/planN/`. Raw transcripts, provari source/binaries, network
  and model files are not.

## History

- **2026-10-29** — initiative opened after a read-only provari review and
  ablation on the motivating FEN. `plan1.md` drafted for the baseline and
  corpus; no product code changed.
