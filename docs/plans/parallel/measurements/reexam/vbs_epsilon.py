"""Virtual-best-solver (VBS) portfolio estimate over the research plan4 global-epsilon sweep.

Reads docs/plans/research/measurements/plan4/plan4_derived.csv (phase0 rows:
4 cases x 7 epsilon values, first-outcome child_evals) and prints, for fixed
epsilon portfolios, the per-case ratio min-over-portfolio / default (0.125).
Timeouts are excluded from the min. Run from the repo root. Read-only.
"""

import csv
import sys

sys.dont_write_bytecode = True

SRC = "docs/plans/research/measurements/plan4/plan4_derived.csv"
PORTFOLIOS = [
    ["0.125", "0.25"],  # the plan7 shape (P0-P2 identical under --first-outcome, P3 = 0.25)
    ["0.125", "0.5"],
    ["0.125", "0.375", "0.5"],
    ["0.125", "0.375", "0.5", "1.0"],
    ["0.125", "0.375", "0.5", "0.0625"],
    ["0.125", "0.375", "0.5", "0.25"],
]

data = {}
for row in csv.DictReader(open(SRC)):
    if row["plan4_derived"] != "phase0":
        continue
    eps = row["arm_or_epsilon"].split()[0][1:]
    data.setdefault(row["case"], {})[eps] = (int(row["child_evals"]), row["timeout"] == "True")

for portfolio in PORTFOLIOS:
    cells = []
    for case, by_eps in data.items():
        base = by_eps["0.125"][0]
        best = min(by_eps[e][0] for e in portfolio if not by_eps[e][1])
        cells.append(f"{case} {best / base:.2f}")
    print(portfolio, " | ".join(cells))
