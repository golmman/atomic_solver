# plan4 raw measurements (2026-09-23)

Sharpness-first pilot (feasibility test of report2 §9 option 2): Task A
full-width ply-1/2 sharp-class density screen, Task B sibling-thickness
probe, Task C artifact build + validation census, per `../../plan4.md`.
Harness: `spike4.py` (Python 3 stdlib-only, black-box driver of the
unmodified release binaries; **no product changes**). Snapshot reader
reused from `../plan2/ssfp.py` verbatim; max-RSS via plan3's
`os.wait4` method. Gates pre-registered in `../../plan4.md` §2
(VALIDATE hard 100%; DENSITY ≥ 10% GO / < 2% thin) — fixed before any
run.

Run protocol: stdin DEVNULL, raw stdout/stderr captured
(`logs/<stem>.out` / `.err`), max-RSS per process in the state JSONs,
runs strictly sequential. Snapshots kept until the report, then pruned
to empty `.tt.keep` markers; counts live in the state JSONs.

## Command table

| file | command |
| --- | --- |
| `env.json` | `spike4.py env` |
| `state/p1_*.json`, `state/p2_*.json`, `logs/p*_*`, `snaps/p*_*` | `spike4.py screen` (all 20 first moves + all legal replies, `--timeout 8 --first-outcome --tt-dump-path`; enumeration via `examples/list_legal` + `examples/replay`) |
| `state/prom_*.json` | `spike4.py promote` (seed 20260923, 5 censored ply-2 positions → 120 s; plateau-transfer check) |
| `state/sib_*.json` | `spike4.py siblings` (all legal moves at e4e5 p2 and d4d5 p32, 8 s) |
| `artifact/index.json`, `artifact/*.bin`, `state/lad_*.json` | `spike4.py artifact` (reconstruct_pt + `validate: ok` census over decided Tasks A/B positions + the 26 plan1 ladder tactical positions re-solved at 120 s, FEN-deduped) |

## Execution status (2026-09-23)

All pre-registered runs completed: Task A 420/420 screen runs + 5/5
promotions, Task B 61/61 sibling runs, Task C 95/95 artifact members
all `validate: ok` (`artifact/index.json`). Gate verdicts: DENSITY
**GO** (13.75% ≥ 10%), VALIDATE **PASS** — see `../../report4.md` for
the full analysis, the option comparison, and the `pt_keys` off-by-one
fix found by the key census. Snapshots pruned to empty `.keep` markers
(plan1/plan2 convention); counts live in the state JSONs and
`artifact/index.json`.

## Notes

- Screen budget = plan1's steer budget (8 s) so results are directly
  comparable to the ladder data; censored = unresolved at 8 s, never a
  value. Decided = `exit_reason: Complete` (any outcome, including
  proven draws).
- `spike4.py status` prints progress; every subcommand is resumable
  from its state JSONs.
- The artifact directory is a *pilot* of the sharpness deliverable
  (per-position validated proof trees + index), not the artifact
  itself; scope stays inside `measurements/plan4/`.
