# Plan6 measurements — bounded scheduler/budget iteration (V0/V1/V2/V3)

Same harness and protocol as plan5 (`../plan5/README.md`); drivers adapted
from plan5's. Code delta: the master's registered V3 abandon trigger
(`--abandon`; `abandon_in_flight` in `examples/campaign/state.rs`, campaign
code only, product untouched) and this driver's arm table. env.json records
binary SHA-256s and the arm table; sandbox: 4-CPU cgroup quota, **8 GiB
memory limit** (one OOM kill hit the first V3 rep4 — re-run). Note: the S
denominators ran on the pre-V3 master build (its seq path is unchanged by
the V3 commit and reproduced plan5's node/child-eval counts bit-identically);
env.json's hashes are the post-V3 binaries used for every campaign arm.

## Positions (unchanged from plan5)

- `shuffle` (primary): `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`
- `m22` (sanity): `4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22`
- `d4d5` (exploratory): **skipped** — the optional equal-budget pair would
  have breached the §7 hard compute cap (~4 h spent when reached; both sides
  certain to censor per plan5's row); skip reported per §7.

## Command table

| artifact | command |
|---|---|
| `env.json` | `python3 run_arms.py env` |
| `drift_<pos>.json` | `python3 run_arms.py drift <name> <FEN>` |
| `seq_<pos>_rep<k>.json` (S) | `python3 run_arms.py seq <name> <FEN> 5` |
| `<pos>_<arm>_rep<k>.json` | `python3 run_arms.py arm <name> <FEN> <V0\|V1\|V2\|V3> <workers> 5 600` |
| 4-worker C4′ rows | `... <V1\|V2> 4 5 600` (state files `<pos>_<arm>w4_rep<k>.json`; see provenance note) |
| `summary.json` | `python3 summarize.py` |
| soundness audit | `python3 audit.py <position> state/<pos>_<arm>_rep0_proof_tree.bin state/seq_<pos>_rep0.json` |

Registered arms (`--tt-mb 128 --pt-mb 512` everywhere; plan5 flags except
where stated):

| arm | flags | meaning |
|---|---|---|
| S | `--mode seq` | sequential first-outcome baseline (denominators) |
| V0 | `--nf` | incumbent (plan5 C2-nf): flat 8M jobs, no re-selection |
| V1 | (feedback on) `--slice 4000000` | one-step ladder 4M→8M |
| V2 | (feedback on) `--slice 2000000` | two-step ladder 2M→8M (registered optional, run) |
| V3 | `--nf --abandon` | V0 + registered abandon trigger (the one code change) |

C4′ = winner at 4 workers on shuffle (see provenance note — run with V1
before the final winner re-designation, not re-run after).

## Provenance / deviations (full detail in `../report6.md` §5)

- **Driver tag collision**: the first C4′ runs reused the 2-worker state-file
  tags and overwrote the first V1/V2 2-worker records. Fix: workers go into
  the tag (`<pos>_<arm>w4_rep<k>.json`); both 2-worker arms were **re-run in
  full** with the fixed driver — the committed 2-worker rep JSONs are the
  re-run data (the re-run is also the registered record the gates were
  applied to).
- **C4′ rows are transcript-recovered**: both 4-worker samples predate the
  tag fix; their per-rep JSONs were overwritten. The numbers below are from
  the session transcript (recorded verbatim during execution); the C4′ re-run
  was skipped per the §7 cap. No gate hinged on them.
- **V3 rep4 OOM**: the first V3 rep4 master was OOM-killed by the sandbox
  (cgroup `oom_kill 1`, 8 GiB limit) at ~374 s; re-run clean.
- Worker transcripts/session dirs not committed (convention); committed
  per-rep JSONs (incl. `worker_logs`) are the record. All in-master
  `validate` fields of proven reps are `ok`; external audit covers every
  proven rep-0 artifact (6 artifacts, 144 dual-checked facts, 0
  contradictions).

## Gate results (pre-registered, plan6 §4)

- **SOUND: PASS** — 0 master `verify_failed` tripwires in all registered
  reps; 6 audited artifacts validate; root outcome equality with S; 144
  dual-checked facts, 0 contradictions, 0 inconclusive.
- **ECON (winner V1 @2 workers): NO-GO** — 5/5 proven ✓ but median wall
  73.09 s = 0.63× S (< 1.2× GO band and even < 1.0× MARGINAL band);
  inflation 2.64× (≤ 3.0 ✓). Completeness clause satisfied; wall band fails.
- **C4′ (transcript-recovered, V1 @4W): FAIL** — 4/5, proven median 261.6 s
  (worse than the winner's 2-worker 73.1 s), inflation ~26× (> 3.5×).
- **Verdict: Not-GO → no plan7.** Attribution recorded as A5 in
  `../../campaign_architecture.md` §11.
