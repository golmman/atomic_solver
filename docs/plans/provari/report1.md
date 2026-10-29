# Provari plan1 report — reproducible head-to-head baseline

Initiative: `provari`, backlog item 1. Executed 2026-10-29. This plan changed
no product code; all artifacts are under
[`measurements/plan1/`](measurements/plan1/).

## Result in one paragraph

The ad hoc order of magnitude reproduced. On the core FEN provari proves in
**0.39 s median** with its bundled model and **0.52 s median** with the hand
guide, while `atomic_solver --first-outcome` **times out at 10 s** (it had
taken 39.6 s for a full win in the ad hoc 60 s probe). On the fixed
`sample20`, provari wins **4/20** cases within 10 s and our solver wins
**1/20**; the shared-win set is exactly one case, where provari is ~65× faster
by wall time. The gap is both a latency gap and a coverage gap, and the sample
is mostly too hard for both solvers (16/20 censored on both sides). The
baseline therefore supports testing a search-shaping lever next, not an
attacker-only front-end refactor.

## Provenance and commands

Full provenance is in
[`measurements/plan1/README.md`](measurements/plan1/README.md). Summary:

| Item | Value |
| --- | --- |
| Date / host | 2026-10-29; container `bc5b4ea98f93`, AMD Ryzen 9 5950X, 32 vCPU |
| Provari | commit `48686bc6c786d6ea70568f0b2ad4002cf0520206` (`v0.1.0`), bundled build |
| Network / model | `network.nnue` sha256 `99dc67ea…a04a6`; `model.json` sha256 `8822a85f…7f987` |
| `atomic_solver` | commit `9ae627a958283db76f52eb6127fbf45c1ee3207a` |
| Budget | 10 s and 192 MB label per side; no tablebases; one process at a time |

Exact commands:

```sh
/tmp/provari-oracle/target/release/provari prove "<FEN>" \
  --seconds 10 --memory 192 --guide model

/tmp/provari-oracle/target/release/provari prove "<core FEN>" \
  --seconds 10 --memory 192 --guide hand

./target/release/atomic_solver \
  --fen "<FEN>" --timeout 10 --first-outcome --outcome-only --tt-size 192
```

The bundled build was available; no fallback to hand-only was needed. The
core proof was separately generated with `--proofs /tmp/provari-core-proof.pvr`
and accepted by `provari check` (`valid:true`, 13 moves, 9043 won positions,
highest counter 99). That check run's wall time is not part of the baseline.
Our solver's first-outcome line is informational here; no proof tree was
built or validated in this plan.

## Core case — all repeats

FEN: `r1b1kbnr/ppp3pp/n4q2/3pppN1/1P1PP3/8/P1P2PPP/RNBQKB1R w KQkq - 0 1`.

| Run | Config | Wall | Outcome | First win | Provari nodes / solver evals | First-win / exit |
| ---: | --- | ---: | --- | ---: | ---: | --- |
| 1 | provari bundled model | 0.390 s | win | mate 13 → 26 plies | 169,485 nodes | 293 ms |
| 2 | provari bundled model | 0.392 s | win | mate 13 → 26 plies | 169,485 nodes | 297 ms |
| 3 | provari bundled model | 0.381 s | win | mate 13 → 26 plies | 169,485 nodes | 285 ms |
| **median** | **provari bundled model** | **0.390 s** | **win** | **26 plies** | **169,485 nodes** | |
| 1 | provari hand | 0.515 s | win | mate 17 → 34 plies | 342,289 nodes | 443 ms |
| 2 | provari hand | 0.518 s | win | mate 17 → 34 plies | 342,289 nodes | 446 ms |
| 3 | provari hand | 0.520 s | win | mate 17 → 34 plies | 342,289 nodes | 448 ms |
| **median** | **provari hand** | **0.518 s** | **win** | **34 plies** | **342,289 nodes** | |
| 1 | atomic_solver | 10.018 s | draw | — | 67,663,190 child evals | Timeout |
| 2 | atomic_solver | 10.026 s | draw | — | 67,782,666 child evals | Timeout |
| 3 | atomic_solver | 10.024 s | draw | — | 67,310,675 child evals | Timeout |
| **median** | **atomic_solver** | **10.024 s** | **draw** | **—** | **67,663,190 child evals** | **Timeout** |

Provari's `mate` is attacker moves; `first_win_plies = 2 * mate` follows the
convention in `plan1.md`. The hand control is 1.33× slower than the bundled
model on the median and needs a longer proof (17 vs 13 attacker moves), but it
is still **~19× faster than our 10 s timeout** and ~76× faster than the
previously measured 39.6 s full win (the bundled model is ~100× faster than
that probe). The learned model is therefore not the
source of the order-of-magnitude gap.

The 60 s ad hoc result (39.6 s, 11.77 M nodes, 305.7 M child evals) was not
re-measured here because plan1 fixes the budget at 10 s. The 10 s run ending
without an outcome is the expected form of the deviation, not a contradiction:
at 10 s our solver has spent only ~68 M child evals, about 22% of its earlier
full-win work.

## `sample20` — every case

Selection: every 10th line of provari's pinned `bench/high200.jsonl`
(1-based indices 1, 11, …, 191). `provari` = bundled model; `solver` =
`--first-outcome`. Censored means no decisive outcome within 10 s; no row is
omitted.

| Case | P wall | P mate | P nodes | P result | S wall | S first win | S child evals | S result |
| --- | ---: | ---: | ---: | --- | ---: | ---: | ---: | --- |
| `2a0443d446ee` | 10.06 s | — | 5,411,840 | censored | 10.02 s | — | 66,903,593 | censored |
| `b9a7ed7eb368` | 4.46 s | 29 | 2,452,328 | win | 10.01 s | — | 72,033,521 | censored |
| `2e8276dc4530` | 10.06 s | — | 5,531,648 | censored | 10.02 s | — | 69,786,386 | censored |
| `1eb80b92f92c` | **0.09 s** | **20** | **13,779** | **win (shared)** | **6.15 s** | **37 plies** | **43,266,033** | **win (shared)** |
| `6559fbdddb47` | 10.06 s | — | 5,410,816 | censored | 10.02 s | — | 70,724,351 | censored |
| `3004d09cef79` | 10.06 s | — | 5,426,176 | censored | 10.02 s | — | 70,689,092 | censored |
| `5862b1958212` | 10.06 s | — | 5,203,968 | censored | 10.02 s | — | 70,423,202 | censored |
| `dd3f89766a40` | 1.86 s | 29 | 911,709 | win | 10.02 s | — | 67,207,433 | censored |
| `8271708709d4` | 10.06 s | — | 5,399,552 | censored | 10.02 s | — | 71,145,355 | censored |
| `60af5542800b` | 10.06 s | — | 5,224,448 | censored | 10.02 s | — | 71,913,248 | censored |
| `db6a741c723f` | 1.70 s | 27 | 810,140 | win | 10.02 s | — | 70,051,517 | censored |
| `531810cc7d4f` | 10.06 s | — | 5,328,896 | censored | 10.02 s | — | 73,044,064 | censored |
| `8862dbcb70d4` | 10.06 s | — | 5,137,408 | censored | 10.02 s | — | 73,079,860 | censored |
| `3a7f66e6b796` | 10.06 s | — | 5,477,376 | censored | 10.02 s | — | 70,137,786 | censored |
| `bcc5c3ebe435` | 10.06 s | — | 5,630,976 | censored | 10.01 s | — | 68,947,657 | censored |
| `e4cca32930e4` | 10.06 s | — | 4,966,400 | censored | 10.02 s | — | 70,257,028 | censored |
| `9da3ab788e86` | 10.07 s | — | 5,563,392 | censored | 10.02 s | — | 71,606,406 | censored |
| `5d89e584375f` | 10.06 s | — | 5,388,288 | censored | 10.01 s | — | 72,340,820 | censored |
| `eefc6b5c0aa5` | 10.06 s | — | 5,507,072 | censored | 10.01 s | — | 71,775,623 | censored |
| `43f66371de39` | 10.06 s | — | 5,444,608 | censored | 10.02 s | — | 71,404,331 | censored |

### Coverage and shared set

- Provari wins: `b9a7ed7eb368`, `1eb80b92f92c`, `dd3f89766a40`,
  `db6a741c723f` — **4/20 = 20%**.
- Solver wins: `1eb80b92f92c` — **1/20 = 5%**.
- Shared wins: `1eb80b92f92c` — **1/20 = 5%**.
- Provari-only wins: 3. Solver-only wins: 0. Neither: 16/20.
- On the one shared case, wall ratio `solver / provari` is
  **6.153 / 0.094 ≈ 65×**. Provari's first-mate latency is 25 ms, so the
  whole-command ratio is not a first-win ratio. One case is not a geometric
  mean; the number is evidence of a latency gap, not a stable estimate.
- Core is covered by provari at 10 s but not by the solver; it becomes a
  shared win only at the longer budget used by the ad hoc probe.

## Diagnosis: latency and coverage

**Both, with latency dominating the evidence.**

1. The core case alone rules out a constant-factor Rust problem: provari's
   hand guide is 0.518 s, while our solver cannot finish 0.39 s worth of
   provari work in 10 s. The neural model is only a 1.33× median difference on
   this position, so the missing mechanism is in search shaping, not the eval
   artifact.
2. Coverage also differs (4 vs 1 sample wins), but the sample is largely
   saturated at the hard end: 16/20 cases are censored for both solvers.
   Three provari-only wins show a real coverage advantage, yet they do not
   explain the 26× core latency gap by themselves.
3. The shared case is too easy for provari (0.094 s, 13,779 nodes) and still
   costs our solver 6.15 s and 43.3 M child evals. The ratio may shrink on
   longer proofs, but there is no evidence for that here.
4. The corpus is therefore better at exposing **coverage** than at producing a
   large shared-win set for latency ratios. That is a finding for plan2/plan7,
   not a reason to re-pick `sample20`.

## Deviations and environment notes

- The plan-session probe measured a 39.6 s full win at 60 s; this plan's 10 s
  budget yields a timeout. The order of magnitude (provari sub-second, solver
  tens of seconds or timeout) reproduced. The exact 39.6 s number is not a
  plan1 result.
- The bundled model and both fetched artifacts were available and
  digest-checked; no hand-only fallback was needed.
- `solver_nodes` is `null` for all our rows because `--outcome-only`
  suppresses the `pre_exit:` line that carries it. `child_evals` is the
  committed work metric. Node counters on the two sides are not comparable:
  provari counts expansions/simulation steps, we count DF-PN entries/child
  evaluations.
- 192 MB is a label on both sides but not the same quantity: provari splits
  it between table and proofs; our TT label is a reservation bound. Exact
  parity was not attempted.
- The host has 32 vCPUs but the protocol runs one process at a time. No
  background load was intentionally introduced; wall times are single-host,
  single-run values and are not suitable for sub-percent comparisons.
- The driver was run to completion twice with identical outcomes; the
  committed `results.json` is the final run. Only parsed rows were written;
  raw transcripts stayed in `/tmp`.
- No `src/`, `examples/`, `tests/`, or `Cargo.*` file changed. The corpus
  check confirmed 20 `sample20` rows, exact every-10th source rows, and a
  byte-identical core FEN.

## Recommendation for plan2

**Run the smaller move-class initialization spike (backlog item 3), not the
win-only front-end skeleton (item 2), as the next plan.**

Evidence and trade-offs:

- The baseline gap is already present with provari's hand guide, so the next
  lever should target search shaping. The move-class starting numbers are the
  largest non-model lever in the initiative's prior ablation (2.3×), while a
  skeleton would be a refactor with no speed mechanism of its own.
- The sample gives almost no latency signal (one shared win), but it gives a
  clear target: the core case and provari's 4 sample wins. A default-off,
  opt-in initialization change can be measured on core at a budget where the
  current solver actually finishes (the prior 60 s probe) and on sample20
  coverage/child evals. Do not edit `sample20`; add a separately recorded
  easier corpus only if a later plan needs more shared wins.
- Scope the spike narrowly: checks initialized from surviving evasions,
  double-threat/threat/quiet classes, and one delayed plain-move group only
  if the plan explicitly chooses that coupled pair. Keep the existing solver
  authoritative for draws/losses and fallback identity when the flag is off.
- Because initial proof numbers affect general DF-PN selection, the plan must
  run `make test`, the quick-suite drift check, and the repetition soundness
  gate, and must validate any claimed win before adoption. This is also why
  the skeleton is not automatically the safer first step: it duplicates the
  same soundness surface without yet having a lever to isolate.
- If the initialization change cannot be isolated from general semantics
  without outcome drift, or if its measured effect is small, pivot to the
  win-only front-end skeleton as the following plan. The skeleton becomes the
  right home once at least one attacker-only lever has shown signal.

## Explicitly not measured

- Atomic Syzygy tablebases (the comparison is tablebase-free by design).
- NNUE training, model licensing/product integration, or any learned model
  artifact beyond provari's bundled evaluation.
- Full `high200`; only the fixed every-10th `sample20` was run.
- Provari `--guide none` (the plan's control was hand; the no-eval point was
  measured ad hoc in the plan session, not in this baseline).
- Long-budget (60 s) core repeats, a larger consistent shared-win set, or
  geometric-mean wall ratios. Those need a later plan's recorded corpus and
  3-run protocol.
- Our proof-tree construction/validation and PPV verification for these wins;
  `--first-outcome` output is informational in plan1. Provari's own proof was
  independently checked (`valid:true`) to confirm the oracle claim.
