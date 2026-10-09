# reexam measurements (2026-10-09) — read-only probes for `reexamination.md`

Evidence for `docs/plans/parallel/reexamination.md`. Zero product-code
changes; release binary built from the clean tree at `2081e8a`. All probes
are read-only (solver runs, `perf`, and re-analysis of committed data from
other initiatives).

| artifact | role |
| --- | --- |
| `race.sh` | one concurrent K-racer race over `--epsilon` values (first-outcome) |
| `vbs_epsilon.py` | VBS portfolio estimate over the committed research plan4 ε sweep |
| `vbs_history_arms.py` | VBS portfolio estimate over the committed lean plan10 history/killer arms |
| `state/races.json` | parsed race results (3 cases × 4 racers) |
| `state/derived.json` | perf summary + the two VBS analyses' outputs |
| `env.json` | environment/config provenance |

Raw transcripts and `perf.data` are not committed (regenerable).

## Commands

```bash
cargo build --release
FEN='4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21' CAP=100 \
  docs/plans/parallel/measurements/reexam/race.sh 0.125 0.375 0.5 1.0
FEN='rnbqkbnr/8/6pp/pppppp1B/3PPP2/N5PN/PPP4P/R1BQ1RK1 w kq - 0 10' CAP=60 \
  docs/plans/parallel/measurements/reexam/race.sh 0.125 0.375 0.5 1.0
FEN='4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22' CAP=30 \
  docs/plans/parallel/measurements/reexam/race.sh 0.125 0.375 0.5 1.0

perf record -e cpu-clock -o /tmp/prof/m22.data -- target/release/atomic_solver \
  --fen '4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22' \
  --timeout 30 --first-outcome --outcome-only
perf report -i /tmp/prof/m22.data --stdio --no-children
perf annotate -i /tmp/prof/m22.data --stdio -s '<atomic_solver::search::dfpn::Search>::evaluate_child'

python3 docs/plans/parallel/measurements/reexam/vbs_epsilon.py
python3 docs/plans/parallel/measurements/reexam/vbs_history_arms.py
```

## Results

| case | default ε=0.125 (in race) | W_first | winner | ratio |
| --- | --- | --- | --- | --- |
| shuffle-win | 50.42 s | 31.91 s | ε=0.5 | **0.63×** |
| rem12 (out of sample) | 15.10 s | 11.33 s | ε=0.375 | **0.75×** |
| m22 | 2.85 s | 2.59 s | ε=0.375 | **0.91×** |

All decisive racers returned `win`; the only non-decisive racer was
shuffle-win ε=1.0 (cap-hit timeout `draw`, documented resource-cut
semantics). One race per case (deterministic solver; see `env.json`).
