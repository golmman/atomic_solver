# plan1 raw measurements (2026-09-22)

Reach-vs-depth spike + leaf men-count profile for `solve` backlog #1
(see `../../plan1.md`). Harness: `spike.py` (Python 3 stdlib-only,
black-box driver of the unmodified release binaries). Environment:
`env.json` — **4 cores (`cpu.max 400000/100000`), 8 GiB cgroup memory
limit, 15 GB RAM visible** — the plan assumed ≤~10 cores / 32 GB; the
gap is recorded honestly and only affects comfort margins, not the
work-based metric. Release build at git `34a19c0` (only
`docs/notes.md` dirty), defaults everywhere: 128 MB TT, ε=0.125,
default refine-cap.

## Metric conventions

- `nodes` = the `pre_exit: ... nodes=N` count. This is the documented
  work proxy; the finer `child_evals` counter is not CLI-visible.
- **Censored** = `pre_exit: reason=Timeout` (or the `timeout` line):
  unproven; such points enter the growth analysis as lower bounds only.
- Cost runs are the non-outcome-only pass (`--first-outcome`, stdin
  from DEVNULL), which yields outcome, PV, `pre_exit` nodes and wall
  together. The plan2 double-pass split is not needed (per plan).
- TT snapshot sizes come from the `tt_snapshot: ... bytes=N` stdout
  line. Snapshot **files are not versioned**: they are derived artifacts,
  byte-reproducible from the commands below (uncensored runs are
  deterministic; the instrumentation runs reproduced the clean p30
  snapshot's `solved/unsolved/bytes` counts exactly on a different
  binary). `snaps/*.keep` markers record which runs wrote a snapshot;
  all snapshot files were pruned before commit (the d4d5 p30 snapshot
  alone was 136 MB). To regenerate the verify evidence:
  re-run the d4d5 p30 cost command from the table row for
  `state/cost_d4d5_p30.json`, then the `verify` command from §Command
  table (~4 min total).

## Ladder protocol (and the one interpretation the plan left open)

8 seeds (startpos, 1.e4, 1.d4, 1.Nf3, 1.e4 e5, 1.e4 c5, 1.d4 d5,
1.Nf3 d5). Each step runs a bounded steer solve
(`--timeout 8 --first-outcome`, non-outcome-only, stdin=DEVNULL) and
advances the line **2 plies**: PV move + PV reply when the steer solve
printed a `pv:` with ≥2 moves; per-ply fallback to the **first move of
`list_legal` order** when the PV is missing, shorter than needed, or
illegal. Positions after 1-ply advances (terminal replies) are not
recorded as ladder positions.

The plan's stop rule ("to ply 44 or until the step solve hits the step
cap") is ambiguous for shallow seeds: measured on this build, startpos,
1.e4, 1.d4, 1.Nf3 and 1.d4 d5 cannot be *proven* within 8 s (they time
out), so a literal "stop at the first capped steer solve" would kill
half the ladders at ply ≤ 1 and contradict the plan's ply-44 target.
Protocol as run (second revision, after the first run showed that a
"stop after 4 consecutive capped steps" rule truncates every line at
ply 6–10 because quiet fallback continuations never re-enter the
solver's reach): capped steer solves do **not** stop the line — the
fallback advance applies — and the ladder runs to **ply 44** or a
**terminal position** (forced mate played out from a PV, or rule50
draw). All lines ended terminal at ply 6–38; no line reached ply 44.
The per-step steer status (`pv` / `capped`) is recorded, so per-line
**reach = deepest PV-steered ply** remains measurable (10, 27, 27, 25,
10, 6, 38, 6). Raw steer solves: `logs/step_<line>_p<ply>.log`; full
line records: `state/ladder_<line>.json`.

Seed positions with ply < 2 (startpos, 1.e4, 1.d4, 1.Nf3) are included
in the cost batch but flagged `informational_seed_position`; the
pre-registered growth fit uses ply ≥ 2 only.

## Command table

| file | command |
| --- | --- |
| `env.json` | `spike.py env` |
| `random_playouts.json` | `spike.py random --games 200 --seed 20260922` (uniform-random legal moves from startpos, via `list_legal`/`replay`; 1000-ply safety cap) |
| `state/ladder_<line>.json`, `logs/step_*.log` | `spike.py ladder` (8 self-play ladders to ply 44 / step-cap saturation / terminal) |
| `state/cost_<line>_p<ply>.json`, `logs/cost_*.log`, `snaps/*.tt` | `spike.py cost` (one solve per ladder position: `--timeout 120 --first-outcome --tt-dump-path ...`, sequential — wall is recorded, so the plan's ≤2-concurrent allowance is not used here) |
| `state/cost_<line>_p<ply>_1024mb.json`, `logs/cost_*_1024mb.log` | `spike.py cost --tt-size 1024` (1 GB-TT addendum at the deepest uncensored position) |
| `state/menhist_*.json`, `logs/menhist_*.log` | temporary men-count instrumentation runs (see below); **source reverted afterwards** |
| `drift_pre_{1,2}.txt` | pre-spike m22 first-outcome captures (`atomic_solver --fen <m22> --timeout 30 --first-outcome --outcome-only`), byte-identical |
| `drift_post_1.txt` | post-revert m22 capture, byte-identical to the pre-spike ones |
| `state/verify.json`, `logs/verify_*.log` | `spike.py verify` (`reconstruct_pt --snapshot ... --out ...` on the 5 deepest uncensored positions (d4d5 p30–p38; the plan asked for the 2 deepest, but p36/p38 are 1–9-node searches — the meaningful verify/find datapoints are p30/p32); `validate:` status + walker `nodes:` recorded) |
| `state/cost_d4d5_p30_1024mb.json`, `logs/addendum_1gb_*` | 1 GB-TT addendum at d4d5 p30 (`--tt-size 1024`, otherwise the cost command) |
| `fit.json` | growth-model fit + gate computation (`fit.py`) |

## Temporary men-count instrumentation

Per the `egtb` plan1/report1 method (its `MenHistogram` counters were
removed post-report; re-derived here): atomic counters in
`src/search/dfpn/children.rs` bin every `evaluate_child` call by the
child position's men count (`occupied().count()`), split by
side-to-move and by resolved (`outcome = Some(_)`: terminal,
TT-resolved, rule50, repetition) vs non-terminal (`outcome = None` —
the leaf-probe-replaceable population, per the egtb semantics). The
dump (`SPIKE ...` lines on stderr, plus the root men count at search
exit) is printed by a temporary hook in `src/main.rs`.

**Deviation (documented):** the plan says "the 3 deepest uncensored
ladder positions", which are d4d5 p34/p36/p38 — 6/7/9-node searches
whose histograms would hold 6/7/9 samples. The three deepest uncensored
positions **with non-trivial search work** were instrumented instead:
d4d5 p30 (23.2M nodes), d4d5 p32 (900), e4e5 p2 (4,749). The
instrumentation was proven inert: all three runs reproduce the clean
cost runs' node counts exactly (23,228,233 / 900 / 4,749) and
byte-identical stdout (modulo the tt-dump path).

Applied, measured, fully reverted: `git diff src/` clean, and the
post-revert m22 first-outcome capture (`drift_post_1.txt`) is
byte-identical to the pre-spike captures. The exact applied edits are
preserved in this README's git history and in
`logs/menhist_*.stderr.txt` (the `SPIKE` line format documents the
binning).

## Pre-registered analysis formulas (fixed before the cost data was fitted)

- Growth factor b: least squares on log10(nodes) vs ply over plies
  where the pooled uncensored fraction is ≥ 50% (per-line b over each
  line's own ≥50% window); censored points excluded from the fit,
  reported as lower bounds.
- d_e (EGTB anchor depth, in plies before liquidation): the
  plies-to-terminal horizon within which ≥95% of instrumented
  non-terminal child-eval sites sit at men ≤ m*, with m* = the largest
  men count whose EGTB is deemed generable (5 or 6 — the histogram
  decides); cross-checked against random-playout men-count
  trajectories.
- D_liq: mainline liquidation depth from the ladder (depth at which
  lines reach terminal/EGTB-class positions) cross-checked with the
  random-playout length distribution.
- W = b^(D_liq − d_e). Gate: GO ≤ 10^15; MARGINAL ≤ 10^17 (needs a
  named ≥100× work-reduction plan); RETHINK above 10^17, or ≥half the
  lines censored before ply 30.
