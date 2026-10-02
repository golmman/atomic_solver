# plan10 probe artifacts (plan-session, 2026-10-01) — FALSIFIED PRE-REGISTRATION (amended 2026-10-02, plan10 §9)

> **Status after the official run.** Arm A's official run (2026-10-02) did not
> reproduce this pilot: 6 facts / 1,074 censors, not 3/1,077. Root cause: the
> pilot ran all 1,083 jobs in one session whose private TT is retained across
> jobs — its three 6B head probes ran first and warmed the 128 MB TT for the
> 1,080 tier jobs; arm A drops those probes, so the tier jobs run under a
> different TT history (child_evals differ on 1,053/1,080 shared jobs; 3
> censored jobs flipped to wins). The byte-exact baseline digests below
> (`f8cb4277…`, `47892f84…`, `5df15da5…`) and the shard byte-equality gate are
> **falsified**; the pilot's per-job *budget* bookkeeping (0 budget
> divergences vs the base-4M ladder) remains valid. Arm A was re-baselined by
> a full in-session replay (plan10 §9) — see `../README.md`.

Provenance: a plan10 **plan-session census probe** on throwaway `/tmp/plan10probe`
staging copies of the standing layer (digests pinned in `plan10.md` §1) ran the
**entire and-close job sequence at its real ladder budgets** — the probe's
`--budget-evals 1000` was intended to make jobs near-free, but the
strict-growth floor (`max(2^(k−1) × base, 2 × work_done)`, plan9 §2) made
`2 × work_done` dominate for every censored record, so every job ran at its
true base-4M budget. Verified: **0 budget divergences** against the base-4M
formula over all 1,083 jobs. The probe therefore measured the full plan10
batch (27,695,668,981 child-evals, ~98 min wall) as a *pilot*. The standing
layer was restored to its pinned state after the probe (the probe's manifest
rewrite target had been the standing manifest — see plan10 §1.3); the pilot's
3 shards were saved here before restoration.

These artifacts are the **pre-registered cross-run determinism baseline** for
plan10 arm A (and the union-advance target for the batch), not a substitute
for the official run — gates and promotion belong to the execution session.

| file | content | sha256 |
| --- | --- | --- |
| `census_jobs.jsonl` | all 1,083 `job:` records (path, budget, child_evals, outcome, exit_reason, pass, work_before; `wall_s` nondeterministic) | `faedf9edb95782276e65012b564e57e5b266455daf61b0b3bffa99d051174377` |
| `post_ledger.json` | the probe's post-run sidecar work ledger (8,446 records; bumps in place, 0 new) | `d0653c3cff0d2807fb76c33077385e721a9ded858b4d83790ee650a7ee6f2e72` |
| `probe_shards/h_82e976f53648002b.bin` | fact 1 shard (`a2a3 a7a6 b2b3 a6a5 c2c3 a5a4 d2d3 a4b3 e2e3 g7g6`, 8,438 B) | `6e71cbd793cf8313302dcf3f7ec1821cfba1316557a0560a7137c288b3f5364d` |
| `probe_shards/h_ec130d3671bd78e4.bin` | fact 2 shard (`… e2e3 h7h5`, 8,594 B) | `c32391f304fdcf874bd5a0b31dda3853bf784f351653a8031bd286f60196bcfc` |
| `probe_shards/h_403356ff0aaa0db0.bin` | fact 3 shard (`d2d4 d7d5 … c7c6 e1e2`, 29,710 B) | `e4db19b4d3353553b62dfa36049e39abd0e4de3eaa188cca899066a624e697f2` |

Canonical digests used by plan10's gates (the exact line format is pinned in
plan10 §4.1):

- arm-A tail (1,080 lines, all jobs except the three `g1f3` head probes):
  `f8cb42777b1b94c1a9bb7d24d183fbd545e557c888744a09fa04f921419e25ac`
- the three head probes (path|budget|outcome|pass|work_before only — arm B's
  `child_evals` are not pinned, its TT differs from the pilot's):
  `3e7cf3c7f3d693531ffc9ac004656de29690063f7bdb24a6ba39de8517766825`
- the probe's grown manifest (standing manifest + the 3 fact entries, 259
  entries, all `validate: ok`): `af49fce349f2ccb9db8ad7e3444fcb317968ef1b478cb83eb5e22aec33f22409`

Caveat for the union gate: the pilot ran the three `g1f3` head probes at
TT 128 MB; plan10 arm B runs them at TT 1024 MB (plan8/9 climb precedent), so
the head records' `work_done` may differ by the run-to-run overshoot delta
(`pass` 3 is pinned exactly; `work_done` range-checked). All other 8,443
records must compare byte-identically.
