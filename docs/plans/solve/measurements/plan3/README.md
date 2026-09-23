# plan3 raw measurements (2026-09-2x)

Quiet-root finishability probe (Task A: 2 h arms A1/A2 on the d4d5 p2
root) + ±1-ply substrate measurement (Task B), per `../../plan3.md`.
Harness: `probe.py` (Python 3 stdlib-only, black-box driver of the
unmodified release binaries; **no product changes** in this plan — the
`--tt-load-path` hook stays reverted). The snapshot reader is reused
from `../plan2/ssfp.py` verbatim (`import ssfp`). Environment:
`env.json`. Layout follows the plan1/plan2 conventions: raw
stdout/stderr captures under `logs/` (`<stem>.out` / `<stem>.err`),
state JSONs under `state/`, snapshots under `snaps/`.

Pre-registered gates (fixed in `../../plan3.md` before any run):

- **Task A** — FINISHABLE if either arm completes decisively with
  `validate: ok` from `reconstruct_pt`; NOT FINISHABLE if both censor;
  SPLIT if exactly one completes (TT-mediated finishability).
- **Task B** — median `avail(P→C) = |P.solved ∩ C.all| / |C.all|`
  over the 5 parent/child pairs: ≥ 5% GO (follow-up anchor-preload
  proposal, separate user approval required), < 1% dead, between:
  judgment call.

Run protocol: stdin from DEVNULL, raw stdout/stderr captured, max-RSS
recorded per process (`os.wait4` ru_maxrss — no `/usr/bin/time` in this
container; the plan's "when available" fallback). Snapshots kept until
the report, then pruned to counts + `.keep` markers (plan1/plan2
convention); counts live in the state JSONs.

## Command table

| file | command |
| --- | --- |
| `env.json` | `probe.py env` |
| `state/arm_a1.json`, `logs/arm_a1.*`, `snaps/arma1.tt` | `probe.py arma --arm a1` (`atomic_solver --fen <d4d5 p2 root> --timeout 7200 --first-outcome --tt-dump-path snaps/arma1.tt`) |
| `state/arm_a2.json`, `logs/arm_a2.*`, `snaps/arma2.tt` | `probe.py arma --arm a2` (same + `--tt-size 1024`) |
| `arma1_reconstructed.bin` / `arma2_reconstructed.bin` | `reconstruct_pt --snapshot snaps/arma1.tt --out arma1_reconstructed.bin` — only on decisive completion; `validate: ok` required for the artifact to count. **Not exercised: both arms censored** (see `../../report3.md`) |
| `state/tb_<line>_p<ply>.json`, `logs/tb_*`, `snaps/tb_*.tt` | `probe.py armb` (per quiet line: parent = shallowest plan1 `steer: "pv"` ladder position, regenerated 120 s cost run) |
| `state/tc_<line>.json`, `logs/tc_*`, `snaps/tc_*.tt` | (same invocation) child = parent + first steer-PV move via `examples/replay`, 120 s cost run |
| `substrate.json` | (written by `probe.py armb`) avail/carry/vshare per pair + medians + gate verdict |

## Execution status (2026-09-22/23)

All pre-registered runs completed: A1 and A2 both censored (Task A
verdict NOT FINISHABLE, `state/arm_*.json`); Task B ran 10/10 runs and
its gate fired GO (`substrate.json`) — but on the cheap tactical class
(see the caveat below and `../../report3.md`). Snapshots were pruned
after the analysis (plan1/plan2 convention): `snaps/*.tt.keep` markers
+ the counts in the state JSONs + the exact commands above reproduce
them (censored runs are wall-clock bounded → content-equivalent, not
byte-identical).

## Notes

- `probe.py status` prints progress; `arma` is resumable per arm
  (state-file existence), so an interrupted 2 h run can be relaunched
  without redoing a finished arm.
- Task B parent selection is pre-registered (shallowest
  `steer: "pv"` position per line). All five selected parents are cheap
  tactical completions in plan1 (the 8 s steer proved a win there), so
  the ±1 measurement lands on the cheap tactical class — the plan's §3
  caveat ("a realistic case, not necessarily the best case") applies and
  is discussed in `../../report3.md`.
- Snapshots are pruned at report time: `snaps/*.tt` → `*.tt.keep`
  markers; the state JSONs carry the counts and the exact commands.
