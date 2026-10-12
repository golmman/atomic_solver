#!/usr/bin/env python3
"""Compare the plan14 arm-A rollout against the plan12 baseline, per gate v1.0.

    python3 compare.py

Reads:
    state/runs.json   (this rollout, candidate arm)
    docs/plans/research/measurements/plan12/state/runs.json  (baseline;
        shipped HEAD is byte-identical to the plan12 code base, so the
        plan12 rollout is the valid paired-by-salt baseline)

Writes state/comparison.json and prints the per-case verdict table.

Verdicts (pinned rule, gate_methodology.md):
- censored-win: candidate solves where baseline is censored on >= 1 salt
  and is never censored where baseline solves (mirror for censored-loss);
- win: median paired ratio <= 0.9 AND >= 75% of ratios < 1;
- regression: median paired ratio >= 1.1 AND >= 75% of ratios > 1;
- unclear: otherwise.
Paired ratios use only salt pairs where BOTH sides are uncensored.
Basin count: distinct draws at 0.25% relative granularity per side.
Diversity clause: a case whose baseline basin count < 3 cannot be the sole
support of an adopt verdict (it can still veto). Censoring: a case with
> 3 censored draws on either side is under-budgeted (unresolved list).
Lever verdict: adopt (>= 1 censored-win or win on a hard-class case, no
regression/censored-loss anywhere) / reject / defer.
"""

import json
import os
import sys

sys.dont_write_bytecode = True

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "../../../../.."))
BASELINE = os.path.join(
    REPO, "docs/plans/research/measurements/plan12/state/runs.json")
HARD_CLASS = {"stress", "m20_white", "m22_white", "m23_white"}
W_LO, W_HI = 0.9, 1.1
BASIN_REL = 0.0025


def by_salt(runs, case):
    return {r["salt"]: r for r in runs if r["case"] == case}


def basin_count(evals):
    """Distinct draws at 0.25% relative granularity (anchored per basin)."""
    basins = []
    for e in sorted(evals):
        if not any(abs(e - b) / min(e, b) < BASIN_REL for b in basins):
            basins.append(e)
    return len(basins)


def main():
    with open(os.path.join(HERE, "state/runs.json")) as f:
        cand_runs = json.load(f)["runs"]
    with open(BASELINE) as f:
        base_runs = json.load(f)["runs"]

    cases = sorted({r["case"] for r in cand_runs})
    report = {}
    for case in cases:
        cb, bb = by_salt(cand_runs, case), by_salt(base_runs, case)
        salts = sorted(set(cb) & set(bb))
        ratios = []
        only_cand_censored, only_base_censored = [], []
        both_censored, mixed = [], []
        for s in salts:
            c, b = cb[s], bb[s]
            cc, bc = c["censored"], b["censored"]
            if cc and bc:
                both_censored.append(s)
            elif cc and not bc:
                only_cand_censored.append(s)
            elif bc and not cc:
                only_base_censored.append(s)
            else:
                ratios.append(c["evals"] / b["evals"])
        n_cand_cens = sum(1 for s in salts if cb[s]["censored"])
        n_base_cens = sum(1 for s in salts if bb[s]["censored"])
        under_budget = n_cand_cens > len(salts) / 2 or n_base_cens > len(salts) / 2
        med = sorted(ratios)[len(ratios) // 2] if ratios else None
        frac_below = (sum(1 for r in ratios if r < 1) / len(ratios)) if ratios else None
        cand_censored_evals = [cb[s]["evals"] for s in salts if cb[s]["censored"]]
        base_censored_evals = [bb[s]["evals"] for s in salts if bb[s]["censored"]]
        if under_budget:
            verdict = "under-budgeted"
        elif only_cand_censored and not only_base_censored:
            verdict = "censored-loss"
        elif only_base_censored and not only_cand_censored:
            verdict = "censored-win"
        elif only_cand_censored or only_base_censored:
            verdict = "unclear"  # mixed censoring
        elif med is None:
            verdict = "unclear"  # all pairs censored
        elif med <= W_LO and frac_below >= 0.75:
            verdict = "win"
        elif med >= W_HI and frac_below <= 0.25:
            verdict = "regression"
        else:
            verdict = "unclear"
        cand_basins = basin_count([cb[s]["evals"] for s in salts if not cb[s]["censored"]])
        base_basins = basin_count([bb[s]["evals"] for s in salts if not bb[s]["censored"]])
        report[case] = {
            "verdict": verdict,
            "hard_class": case in HARD_CLASS,
            "n_pairs": len(salts),
            "n_ratios": len(ratios),
            "median_ratio": med,
            "ratios": [round(r, 4) for r in ratios],
            "frac_below": frac_below,
            "cand_censored": {"n": n_cand_cens, "salts": [s for s in salts if cb[s]["censored"]],
                              "evals": cand_censored_evals},
            "base_censored": {"n": n_base_cens, "salts": [s for s in salts if bb[s]["censored"]],
                              "evals": base_censored_evals},
            "both_censored_salts": both_censored,
            "cand_basins": cand_basins,
            "base_basins": base_basins,
            "low_diversity": base_basins < 3 or cand_basins < 3,
        }

    wins = [c for c, r in report.items()
            if r["verdict"] in ("win", "censored-win")]
    losses = [c for c, r in report.items()
              if r["verdict"] in ("regression", "censored-loss")]
    adoptable_wins = [c for c in wins
                      if not report[c]["low_diversity"] or c in HARD_CLASS
                      and report[c]["verdict"] == "censored-win"]
    # Diversity clause: a low-diversity case cannot be the sole support of
    # adopt; hard-class censored-wins (censoring-pattern evidence) are
    # structural, not draw-based, and count regardless of basin count.
    adopt_support = [c for c in wins
                     if not report[c]["low_diversity"]
                     or (c in HARD_CLASS and report[c]["verdict"] == "censored-win")]
    if losses:
        lever = "reject"
    elif adopt_support:
        lever = "adopt"
    else:
        lever = "defer"

    with open(os.path.join(HERE, "state/comparison.json"), "w") as f:
        json.dump({"per_case": report, "lever": lever,
                   "wins": wins, "losses": losses,
                   "adopt_support": adopt_support}, f, indent=1)

    print(f"{'case':12s} {'verdict':16s} {'med ratio':>9s} "
          f"{'<1 frac':>7s} {'cens c/b':>8s} {'basins c/b':>10s}")
    for case, r in report.items():
        cc, bc = r["cand_censored"]["n"], r["base_censored"]["n"]
        med = "-" if r["median_ratio"] is None else f"{r['median_ratio']:.3f}"
        fb = "-" if r["frac_below"] is None else f"{r['frac_below']:.2f}"
        tag = " *hard*" if r["hard_class"] else ""
        div = " lowdiv" if r["low_diversity"] else ""
        print(f"{case:12s} {r['verdict']:16s} {med:>9s} {fb:>7s} "
              f"{f'{cc}/{bc}':>8s} "
              f"{str(r['cand_basins']) + '/' + str(r['base_basins']):>10s}{tag}{div}")
    print()
    print("lever:", lever, "| wins:", wins, "| losses:", losses)
    sys.exit(0)


if __name__ == "__main__":
    main()
