#!/usr/bin/env python3
"""plan8 stage 1: pre-computation freeze (evals-space portfolio table).

Recomputes the evals-space portfolio prediction from the committed plan12
rollout (docs/plans/research/measurements/plan12/state/runs.json) BEFORE any
race is run, per plan8.md stage 1. For every pre-registered raced case:

  - per-racer-salt {0,2,3,5} evals (or censored flag) from plan12,
  - evals_min  = min over uncensored racer salts (the portfolio's
    deterministic evals-space result; the portfolio's evals_first depends on
    wall order and is measured, not predicted),
  - salt-0 baseline evals (or censored),
  - predicted portfolio/salt-0 ratio (censored-win where salt 0 is censored
    and some racer is not),
  - plan12 basin count (0.25 % relative clustering anchored at each basin's
    first member, salt order; censored runs excluded) and the racer set's
    basin coverage.

Output: state/derived_evals.json. Deterministic; no solver runs.
"""

import json
import os
import sys

sys.dont_write_bytecode = True

HERE = os.path.dirname(os.path.abspath(__file__))
RUNS_JSON = os.path.join(
    HERE, "..", "..", "..", "research", "measurements", "plan12", "state", "runs.json"
)
OUT = os.path.join(HERE, "state", "derived_evals.json")

RACER_SALTS = [0, 2, 3, 5]  # pre-registered in plan8.md; no post-selection swaps
RACED_CASES = ["m20_white", "stress", "dec10", "m22_white", "dec13"]
SALT_INVARIANT_14 = [
    "dec02", "dec03", "dec04", "dec05", "dec06", "dec07", "dec08", "dec09",
    "dec11", "dec12", "dec15", "dec16", "dec17", "dec18",
]
NOT_RACED = ["dec01", "dec14"] + SALT_INVARIANT_14


def load_runs():
    with open(RUNS_JSON) as f:
        runs = json.load(f)["runs"]
    table = {}
    for r in runs:
        table.setdefault(r["case"], {})[r["salt"]] = r
    return table


def basin_count(values):
    """Basins at 0.25 % relative granularity, anchored at each basin's first
    member (gate_methodology.md). Censored runs are excluded upstream."""
    anchors = []
    for v in values:
        for a in anchors:
            if abs(v - a) <= 0.0025 * a:
                break
        else:
            anchors.append(v)
    return len(anchors)


def main():
    table = load_runs()
    out = {"racer_salts": RACER_SALTS, "cases": {}, "not_raced": {}}
    for case in RACED_CASES:
        per_salt = table[case]
        racer_rows = []
        uncensored = []
        for s in RACER_SALTS:
            r = per_salt[s]
            row = {
                "salt": s,
                "evals": r["evals"],
                "censored": r["censored"],
                "outcome": r["outcome"],
                "wall_s": r["wall_s"],
            }
            racer_rows.append(row)
            if r["censored"] is None:
                uncensored.append(r["evals"])
        salt0 = per_salt[0]
        evals_min = min(uncensored) if uncensored else None
        all_draws = [per_salt[s]["evals"] for s in sorted(per_salt)
                     if per_salt[s]["censored"] is None]
        entry = {
            "racer_rows": racer_rows,
            "salt0": {
                "evals": salt0["evals"],
                "censored": salt0["censored"],
                "outcome": salt0["outcome"],
            },
            "evals_min": evals_min,
            "prediction": (
                "censored-win (salt-0 censored at cap; racer salt with the min "
                f"uncensored draw: {min(uncensored)} evals)"
                if salt0["censored"] is not None and uncensored
                else f"{evals_min / salt0['evals']:.4f}x vs salt-0"
                if evals_min is not None
                else "all racers censored"
            ),
            "basin_count_plan12": basin_count(all_draws),
            "basin_anchors_plan12": all_draws,
            "racer_basin_coverage": sorted(
                next(i for i, a in enumerate(all_draws) if abs(e - a) <= 0.0025 * a)
                for e in uncensored
            )
            if all_draws else [],
        }
        out["cases"][case] = entry
    # Not-raced cases: record the pre-computed portfolio fact (racers are the
    # same computation on salt-invariant cases; dec01/dec14 read from plan12).
    for case in NOT_RACED:
        per_salt = table[case]
        draws = [per_salt[s]["evals"] for s in sorted(per_salt)
                 if per_salt[s]["censored"] is None]
        out["not_raced"][case] = {
            "salt0_evals": per_salt[0]["evals"],
            "salt0_censored": per_salt[0]["censored"],
            "basin_count_plan12": basin_count(draws) if draws else 0,
            "salt_invariant": len(basin_count_safe(draws)) <= 1 if draws else None,
        }
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w") as f:
        json.dump(out, f, indent=1)
    print(f"wrote {OUT}")
    for case, e in out["cases"].items():
        print(f"{case}: salt0={e['salt0']['evals']} censored={e['salt0']['censored']} "
              f"min={e['evals_min']} pred={e['prediction']} basins={e['basin_count_plan12']} "
              f"racer_coverage={e['racer_basin_coverage']}")


def basin_count_safe(values):
    anchors = []
    for v in values:
        for a in anchors:
            if abs(v - a) <= 0.0025 * a:
                break
        else:
            anchors.append(v)
    return anchors


if __name__ == "__main__":
    main()
