# plan4b measurements (2026-10-04) — sharded-TT re-land: gate PASS, tax pinned

Stage-1b re-land of the plan4 sharded-TT refactor
(`../plan4/sharded_tt_attempt.patch` + `shard_rs_attempt.rs`) vs the
unsharded baseline. All runs on the release build, reference container
(Apple Silicon, 4 CPUs), default TT (128 MB), default ε (0.125), default
refine-cap (0.25). The refactor **stays landed** in the working tree
(revert-if-missed is plan5b's condition-1 call).

**Verdict: drift zero everywhere; tax m22 +5.36 % / shuffle-win +4.61 %
median interleaved wall — both inside the pre-registered ≤ +7 % budget,
consistent with plan4's +6.0 %/+4.4 % (Δ ≤ 0.64 pp, under the ~2 pp
profiling-trigger band). Gate PASS.**

## Command table

| file | command |
| --- | --- |
| `state/baseline_quick_pre.json` | `benchmark --suite quick --json --first-outcome --runs 1` (fresh pre-land capture with the saved pre binary; matched the committed plan4 baseline 59/59) |
| `state/post_quick.json` | same, landed build; `child_evals`/`nodes`/`outcome`/`pv_len`/`status`/`timeout`/`wrong` identical 59/59 |
| `state/drift_results.json` | stdout hashes for m22 / shuffle-win, snapshot sha256, golden-test status |
| `state/tax_interleaved.json` | the pinned tax: per-pair walls, medians, deltas, verdict |
| `../plan4/sharded_tt_attempt.patch` | the re-landed diff (applied verbatim with `git apply`) |

## Drift protocol (all green)

- quick suite: 59/59 cases identical (`benchmark --suite quick --json
  --first-outcome --runs 1`).
- m22 first-outcome stdout byte-identical:
  sha256 `b7c74f17c88a730777be98a1452d97aa4cbccee49525cd47278b0a05b05ac79a`.
- shuffle-win first-outcome stdout byte-identical:
  sha256 `64129ef0942a0446faa4e062512bf80b6f34c2979f657f9f11f3853975b04cd7`.
- snapshot dump identical: sha256
  `eaa5f2b97bded5e1050cd2e1eb3489186985015d525035ea52cc4918cef81fde`,
  195 B — same hash as plan4's record.
- `test_trajectory_golden` (m22 default-mode golden): ok in `make test`.
- stderr progress lines (`[bounded_search] chunk done …`) carry wall
  timings and are *not* part of the byte-identity surface.

## Interleaved wall A/B (pre vs landed, medians)

| case | pairs | pre median | post median | delta | budget |
| --- | --- | --- | --- | --- | --- |
| m22 first-outcome (30 s cap) | 10 | 3.009 s | 3.170 s | **+5.36 %** | ≤ +7 % ✔ |
| shuffle-win first-outcome (100 s cap) | 3 | 53.363 s | 55.822 s | **+4.61 %** | ≤ +7 % ✔ |

Per-pair delta spread: m22 +3.34 %..+6.90 %, shuffle-win +4.49 %..+5.06 %.

## Test-suite deviation

The revived 4-thread stress test
(`search::tt::tests::concurrent_store_and_probe_respect_solver_invariants`)
had a latent race in its own phase-2 assertion: a concurrent higher-work
solved store can legitimately evict a thread's Win entry (k=2
replacement), after which the thread's own phase-2 unsolved store refills
the key as a fresh unsolved entry — which the assertion misread as a
policy downgrade. Empirically confirmed (every observed failure had the
exact fresh-refill shape work=7/rd=0/depth=0; flaky 4/5 when run in
isolation). Fixed in `src/search/tt/tests.rs` only — the solver/store
code is bit-identical to the plan4 attempt; the invariant under test
("no in-place downgrade of a solved entry") is unchanged and now also
covers the refill shape. Deviation documented in `report4b.md`.
