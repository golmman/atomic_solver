# Plan5 measurements — two-job campaign prototype (arms, metrics, audit)

Prototype: `examples/campaign_master` / `examples/campaign_worker` (campaign
code only; the product CLI is untouched except one read-only TT accessor,
`TtEntry::advisory_pn_dn`, used for the advisory worker feedback — no
behavior change). Soundness contract: `../campaign_architecture.md`.

## Environment

`env.json` — written by `run_arms.py env` before the runs: host, kernel,
nproc, RAM, binary SHA-256 prefixes, slice budgets. Reference sandbox:
4-core cgroup quota (`cpu.max` = 400000/100000), ~8 GB available.

## Positions (pre-registered, plan5 §4)

- `m22` (secondary, sanity scale): `4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22`
- `shuffle` (primary): `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`
- `d4d5` (exploratory, no gate): `rnbqkbnr/ppp1pppp/8/3p4/3P4/8/PPP1PPPP/RNBQKBNR w KQkq d6 0 2`

Note: `shuffle-win`/`m22_white` are not decisive-suite fixtures; FENs come
from `docs/plans/parallel/measurements/plan2/README.md` (the design_space §3
baseline protocol).

## Command table

| artifact | command |
|---|---|
| `env.json` | `python3 run_arms.py env` |
| `drift_<pos>.json` (CLI wall/outcome cross-check) | `python3 run_arms.py drift <name> <FEN>` — `atomic_solver --fen <FEN> --first-outcome --outcome-only --timeout 600` (in `--outcome-only` mode the CLI prints no `pre_exit:` line, so nodes is null here; work counts come from the in-process seq baseline) |
| `seq_<pos>_rep<k>.json` (S arm) | `python3 run_arms.py seq <name> <FEN> 5` — in-process first-outcome solve, reports wall/nodes/**child_evals** (the inflation denominators) |
| `<pos>_<arm>_rep<k>.json` (arms) | `python3 run_arms.py arm <name> <FEN> <C2|C4|C2-nr|C2-nf> <workers> 5 <maxwall> [nf] [nr]` |
| `d4d5` exploratory pair | `seq` with `--seq-timeout 1800` + `arm d4d5 <FEN> C2 2 1 1800` |
| soundness audit | `python3 audit.py <position> <session-rep0-dir>` (artifact validation + dual-check) |

Arms (all with `--slice 1000000 --max-slice 8000000 --tt-mb 128 --pt-mb 512`):

| arm | flags | meaning |
|---|---|---|
| S | `--mode seq` | sequential first-outcome baseline |
| C2 | `--workers 2` | persistent workers, feedback on, bounded slices |
| C4 | `--workers 4` | scaling slope at the sandbox ceiling |
| C2-nr | `--workers 2` + worker `--retention off` | retention ablation (fresh TT per job) |
| C2-nf | `--workers 2 --nf` | feedback ablation (jobs to completion, static order) |

Per rep the master's JSON summary is copied to `state/` (worker stderr logs
are transcripts and are not committed, per convention; the per-job lines they
contain are summarized into the committed `worker_logs` field).

## Metrics

- **wall speedup** = median(S wall) / median(arm wall), 5 reps.
- **work inflation** = median(aggregate worker child_evals) / median(S
  child_evals); node counts reported alongside (S nodes from the in-process
  baseline; CLI-drift wall as a cross-check against `parallel/design_space.md`
  §3's 2026 numbers).
- **retention benefit** = inflation(C2) vs inflation(C2-nr); **feedback
  benefit** = inflation(C2) vs inflation(C2-nf) (pre-registered attribution
  gates, plan5 §4).
- **soundness audit** (`audit.py`): `inspect_pt --validate` on the composed
  rep-0 artifact; root outcome equality with the sequential baseline; spot
  dual-check of ≥ 20 sampled decisive facts re-derived by the sequential CLI
  solver from the replayed fact FEN (zero contradictions required).

## Raw transcripts

Worker stderr, session directories (`/tmp/plan5_arms/...`), and TT snapshots
are not committed (`.gitignore`d class); the committed per-rep JSONs (incl.
`worker_logs` job lines) are the reproducible record.
