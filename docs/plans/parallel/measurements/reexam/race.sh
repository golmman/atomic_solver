#!/bin/bash
# One concurrent race: launch one solver process per epsilon value, all at
# once, and print wall time + the head of each stdout. Read-only probe used by
# docs/plans/parallel/reexamination.md (2026-10-09).
#
# Usage (repo root, release build current):
#   FEN='<fen>' CAP=<seconds> docs/plans/parallel/measurements/reexam/race.sh 0.125 0.375 0.5 1.0
#
# The first decisive finish is the portfolio's W_first; the default racer
# (epsilon 0.125) inside the same race is the in-race baseline.
BIN=${BIN:-target/release/atomic_solver}
for e in "$@"; do
  (
    s=$(date +%s.%N)
    out=$("$BIN" --fen "$FEN" --timeout "${CAP:-100}" --first-outcome --outcome-only \
      --epsilon "$e" 2>/dev/null | head -c 30 | tr '\n' ' ')
    t=$(date +%s.%N)
    printf "eps=%-6s wall=%6.2fs  %s\n" "$e" "$(echo "$t-$s" | bc)" "$out"
  ) &
done
wait
