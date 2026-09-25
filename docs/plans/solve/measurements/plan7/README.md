# plan7 raw measurements (2026-09-24)

Sequential d4d5-p2 sizing ladder — item 8, stage 1 of 3, per
`../../plan7.md`.  Harness: `ladder.py` (Python 3 stdlib-only,
black-box driver of the unmodified release binaries; **no product
changes**).  Arms (pre-registered in `../../plan7.md` §2 before any
run, strictly sequential L1 → L4, nothing else running concurrently):

| arm | wall | status |
| --- | --- | --- |
| L1 | `--timeout 3600`  | fresh (this plan) |
| M2 | `--timeout 7200`  | on record: plan3 A1 (`../plan3/state/arm_a1.json`), reused — not re-run; admission gated by the §3 comparability check |
| L4 | `--timeout 14400` | fresh (this plan) |

Reference config: default 128 MB TT, `--first-outcome`, TT snapshot
dump; **no `--outcome-only`** (the `pre_exit: nodes=` line is the
primary metric).  Command-identical to plan3 A1 modulo `--timeout`, so
every recorded datapoint is comparable.  Environment: `env.json`
(matches `../plan3/env.json`: 4-CPU cgroup quota, 8 GiB memory.max).

The snapshot reader and binary paths are reused from
`../plan2/ssfp.py` verbatim (`import ssfp`).  Layout follows the
plan1–plan3 conventions: raw stdout/stderr captures under `logs/`
(`<stem>.out` / `<stem>.err`), state JSONs under `state/`, snapshots
under `snaps/`.  Max-RSS per child process via `os.wait4` ru_maxrss.

Pre-registered gate (`../../plan7.md` §3, fixed before any run):
COMPLETED (any arm finishes decisively with `reconstruct_pt`
`validate: ok`) / LINEAR (η ∈ [0.9, 1.1], monotone floors) / DEGRADING
(η < 0.9) / SUPERLINEAR (η > 1.1), η = log(N_L4/N_L1)/log 4 (fresh
two-point; three-point L1/M2/L4 fit iff L1's and L4's rates are both
within ±10% of A1's 198.4k nodes/s).

## Command table

| file | command |
| --- | --- |
| `env.json` | `ladder.py env` |
| `state/arm_ladder1.json`, `logs/arm_ladder1.*`, `snaps/ladder1.tt` | `ladder.py ladder --arm ladder1` (`atomic_solver --fen <d4d5 p2 root> --timeout 3600 --first-outcome --tt-dump-path snaps/ladder1.tt`) |
| `state/arm_ladder4.json`, `logs/arm_ladder4.*`, `snaps/ladder4.tt` | `ladder.py ladder --arm ladder4` (same with `--timeout 14400`) |
| `analysis.json` | `ladder.py analyze` (rate law + η fit + comparability check + TT occupancy/fill point; writes the gate verdict) |
| `<arm>_reconstructed.bin` | `reconstruct_pt --snapshot snaps/<arm>.tt --out <arm>_reconstructed.bin` — only on decisive completion; `validate: ok` required for the artifact to count.  **Not exercised if the arms censor** |

Run protocol: stdin from DEVNULL, raw stdout/stderr captured, max-RSS
recorded per process (`os.wait4` ru_maxrss).  Per-arm resumability via
state-file existence (a completed arm is skipped on relaunch); an arm
interrupted > 30 min in is not re-run (compute cap, `../../plan7.md`
§7).  Snapshots kept until the report, then pruned to counts +
`.keep` markers; counts live in the state JSONs.
