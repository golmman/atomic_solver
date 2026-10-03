# plan5 measurements (2026-10-04) — stage 2a: `--threads N` mechanism + smoke

Implementation session for `plan5.md` (SPDFPN stage 2a). The mechanism landed;
the GO/NO-GO campaign is **plan5b** — the numbers below are the smoke the plan
asks for (outcome agreement, no panic, plausible work split), not the
pre-registered verdict.

**Smoke verdict: soundness clean — 15/15 outcome agreement with the sequential
solver (m22, first-outcome, threads 2 and 4), zero panics, zero torn-state
symptoms. Speed is not there yet: median 1.42× at 4 threads / 0.92× at
2 threads, work inflation ~2.2–2.4×, high variance at 2 threads (one 15 s
outlier). plan5b's W sweep and interleaved campaign decide GO/MARGINAL/NO-GO.**

## Command table

| file | command |
| --- | --- |
| `state/baseline_quick_pre5.json` | `benchmark --suite quick --json --first-outcome --runs 1` on the pre-plan5 build (sha256 `7672edeb…`) |
| `state/drift_results.json` | final N=1 drift check: quick suite 59/59, m22/shuffle-win stdout, snapshot sha256 |
| `state/smoke_m22.json` | 5×3 interleaved runs (threads 1/2/4), m22 first-outcome, 30 s cap; per-run wall, outcome, helper/coordinator evals from the `[parallel]` stderr line |

## Case FENs (same as plan4b)

- m22: `4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22` (`--timeout 30`)
- shuffle-win: `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21` (`--timeout 100`, drift check only)
- snapshot: `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1 --tt-size 16 --tt-dump-path …`

## Drift (N = 1) — all green, hashes identical to plan4/plan4b records

- quick suite 59/59 identical; m22 stdout `b7c74f17…`; shuffle-win stdout
  `64129ef0…`; snapshot `eaa5f2b9…`/195 B.

## Smoke (m22 first-outcome, medians over 5 interleaved reps)

| N | median wall | speedup vs N=1 | outcome agreement | work inflation (median) |
| --- | --- | --- | --- | --- |
| 1 (ref) | 3.256 s | — | — | — |
| 2 | 3.535 s | 0.92× | 5/5 win | 2.17× |
| 4 | 2.288 s | 1.42× | 5/5 win | 2.41× |

Per-run spread at 4 threads: 1.597–3.467 s. At 2 threads: 2.460–15.127 s —
the 15 s outlier had coordinator_evals 98 M (7× sequential) — helper-stored
TT entries can steer the coordinator's trajectory into a bad region
(nondeterministic, accepted envelope, but a real quality finding).

Work split (representative t2 run): coordinator 17.7 M evals, helper
15.8 M evals / 6037 jobs — helpers do real work; total inflation ~2.4×.

## Interpretation for plan5b

- The GO band (≥ 2.0×/4 threads, inflation ≤ 2×) is **not met by the smoke**;
  the mechanism is functional and sound but the helper pool perturbs the
  coordinator more than it helps at N = 2 and only helps ~1.4× at N = 4.
- Known levers for the plan5b sweep: W (20000 was picked from a coarse
  4k/20k/100k probe — see `report5a` deviations), helper eviction pressure on
  the coordinator's entries (k=2 replacement scores helper work equally),
  helper count vs 4 CPUs (+ forwarder thread oversubscription), and possibly
  capping helper participation once the coordinator's trajectory is deep.
