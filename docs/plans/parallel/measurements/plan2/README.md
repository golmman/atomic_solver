# plan2 raw measurements (2026-09-21)

Option-C sizing spike for `parallel` backlog #2. All runs on the release
build, reference container (4 cores / **8 GB cgroup memory limit** —
see "Environment surprise" below; the plan assumed 16 GB), default
scorer params, default TT (128 MB), default ε (0.125), default
refine-cap (0.25). The spike drives the unmodified release binary as a
black box; no `src/` or `examples/` change.

Validation FENs:

- `m22_white`: `4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22`
  (43 root children, none terminal)
- `shuffle-win`: `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`
  (41 root children, none terminal)
- validation-only (shallow, quick-suite): dec08, dec03 — see
  `validation_*.json`

## Command table

| file | command |
| --- | --- |
| `race_harness.py` | the harness (Python 3 stdlib-only; subcommands `children` / `root` / `solve-children` / `race` / `check` / `simulate`) |
| `m22_children.json`, `shuffle_children.json` | `race_harness.py children --fen <FEN> --out <file>` |
| `validation_dec08.json`, `validation_dec03.json` | `race_harness.py check --fen <dec08/dec03 FEN> --cap 5` (perspective-mapping cross-validation vs `find_winning_child`; exact agreement required, exit 1 otherwise) |
| `m22_root_baseline.json`, `shuffle_root_baseline.json` | `race_harness.py root --fen <FEN> --timeout 30/100` (sequential root solve; wall pass `--outcome-only` + work pass parsing `pre_exit: ... nodes=N`) |
| `m22_children_solved.json` (+ `.log`) | `race_harness.py solve-children --fen <m22> --children m22_children.json --cap 120 --out <file>` — every root child solved twice: `--outcome-only` (wall) + non-outcome-only (nodes work proxy), per-child cap 120 s |
| `m22_race_N{2,3,4}.json` (+ `.log`) | `race_harness.py race --fen <m22> --children m22_children.json --workers N --cap 120 --reps 5 --out <file>` |
| `m22_race_N4_winnerfirst.json` (+ `.log`) | same, but with `m22_children_winnerfirst.json` (oracle order: proven winner `a2a4` first), 3 reps |
| `shuffle_race_N4.log` | `race_harness.py race --fen <shuffle> --children shuffle_children.json --workers 4 --cap 120 --reps 2` — rep 0 exhausted all 41 children without a proven move (data in the log; rep 1 cut by the spike's time budget, so no `--out` JSON was written) |
| `simulate_N{8,16}.json` | `race_harness.py simulate --children m22_children_solved.json --workers N --seq-wall 2.629 --also-sjf` (**SIMULATION**, labeled) |
| `m22_drift_capture_stdout/stderr.txt` | post-spike m22 first-outcome capture (`--timeout 30 --first-outcome --outcome-only`); byte-identical across reruns |

`nodes` is the documented work proxy for the CLI-invisible
`child_evals`. For workers killed mid-flight, the last stderr chunk-log
`nodes=` is a **lower bound** (outcome-only mode prints no `pre_exit:`
line); finished workers report the exact `pre_exit:` count. Peak RSS
per worker is sampled from `/proc/<pid>/status` `VmHWM` every 20 ms.

## Environment surprise (recorded honestly)

The reference container exposes `nproc` = 4 and the plan assumed 16 GB
RAM, but the cgroup enforces `cpu.max = 400000/100000` (4 CPUs — fine)
and **`memory.max = 8 GiB`** (half the assumption). A harness bug (all
43 children spawned simultaneously — fixed during the spike) plus ~7 GiB
of page cache produced OOM kills (exit -9) that initially masqueraded
as solver behavior; the cgroup `memory.events` counter shows the kills.
All recorded numbers come from post-fix, verified-clean runs (2 live
workers max ≈ 0.5 GiB anon). Consequence for §4 of the note: the
"N×TT" envelope must be evaluated against the real 8 GiB limit, and the
per-worker peak RSS is the measured quantity that matters.

## Perspective-mapping validation (task 4)

`check` compares the harness's proven-move set (child terminal or child
`loss`) against `find_winning_child` (which stops at its FIRST winning
move in the same movegen order), so exact agreement means its reported
move equals the harness's first proven move:

- dec08 (`r4r1k/3q1P2/pp4pp/2pp4/N4Q2/2P3PP/PP2p3/R4R1K w - - 0 25`,
  expected Win): harness first proven `f4e5` == `find_winning_child` ✓
- dec03 (`r4r2/pp2p1Bk/2p3p1/1B1pP1bp/3P3P/1PN5/2P5/5R1K w - - 1 21`,
  expected Win): harness first proven `b5d3` == `find_winning_child` ✓

## Key results (details and analysis in `../design_space.md`)

- Sequential root baselines: m22 win 2.63 s / 858,117 nodes (95-ply PV);
  shuffle-win win 47.83 s / 13,907,467 nodes (477-ply PV).
- m22 root-child race (cap 120 s, first proven child stops all):
  median wall 9.00 / 9.37 / 9.75 s at N=2/3/4 → speedup **0.29× /
  0.28× / 0.27×** vs the sequential root; aggregate worker nodes
  9.93 M / 11.64 M / 13.33 M → inflation **11.6× / 13.6× / 15.5×**.
  Proven move `a2a4` in all 15 runs (zero nondeterminism, zero
  disagreement).
- The mechanism, visible in the raw data: the winning child `a2a4`
  alone needs 8.6 s / 8.59 M nodes **in isolation**, while the
  sequential root proves the win from the whole position in 2.6 s /
  0.86 M nodes — DF-PN's root-level sibling transposition sharing and
  best-first interleaving are exactly what per-child process isolation
  forfeits. More workers only add wasted partial work (killed
  children), hence inflation grows with N.
- Shuffle-win probe: at N=4 with 120 s per-child caps the race consumed
  all 41 children in 1320 s with **no proven child** — the sequential
  root proves the same position in 47.8 s. (Rep 1 of 2 was cut by the
  spike's time budget after rep 0 already exhausted the queue; rep 1
  added no information.)
- Peak RSS per worker at the default 128 MB TT: ≈ 226–230 MB
  (`workers[].peak_rss_mb` in the race JSONs) ≈ 1.8× the table size.
