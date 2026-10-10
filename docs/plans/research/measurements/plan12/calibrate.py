#!/usr/bin/env python3
"""plan12 Phase 2: derive the pinned gate thresholds from the rollout.

    python3 calibrate.py [state-dir]   (default: ./state)

Reads state/summary.json (from parse.py) and writes state/calibration.json
with the D8(1)–D8(6) tables:

- per-case baseline record: per-salt draws, censored pattern, distinct-basin
  count (draws clustered at 0.25% relative granularity, anchored at the
  basin's first member), median, spread, loud/quiet, hard-class flag;
- the quiet-case cross-salt noise band: over every quiet case with >= 2
  basins, all ratios of distinct draws — how far pure noise moves a
  *between-salt* comparison (the reason single-draw gates are retired);
- the pinned constants (D8(3)–D8(6)).
"""

import json
import os
import statistics
import sys

sys.dont_write_bytecode = True

HERE = os.path.dirname(os.path.abspath(__file__))
STATE = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "state")

# ---- pinned constants (D8) -------------------------------------------------
W_LO = 0.9           # win: median paired ratio <= W_LO
W_HI = 1.1           # regression: median paired ratio >= W_HI
AGREEMENT = 0.75     # direction agreement over paired salts (>= 75%)
BASIN_REL = 0.0025   # distinct-draw granularity (quarter of the W band)
HARD_MIN_MEDIAN = 10_000_000   # hard class: m-family + median >= 10 M
M_FAMILY = {"m20_white", "stress", "m22_white", "m23_white"}
LOUD_SPREAD = 1.5    # loud iff uncensored spread >= 1.5x or mixed censoring


def basins(values):
    """Count distinct draws: sort, anchor-cluster at BASIN_REL."""
    out = []
    for v in sorted(values):
        if not out or v / out[-1][0] >= 1.0 + BASIN_REL:
            out.append([v])
        else:
            out[-1].append(v)
    return out


def main():
    with open(os.path.join(STATE, "summary.json")) as f:
        data = json.load(f)
    summary = data["summary"]

    cases = {}
    quiet_pairs = []  # (case, lo, hi) cross-salt ratios on quiet cases
    for case, s in sorted(summary.items()):
        unc = s["uncensored_evals"]
        bs = basins(unc)
        hard = case in M_FAMILY or (s["median"] or 0) >= HARD_MIN_MEDIAN
        cases[case] = {
            "per_salt": s["per_salt"],
            "n_censored": s["n_censored"],
            "uncensored_evals": unc,
            "basin_count": len(bs),
            "basins": [[b[0], b[-1], len(b)] for b in bs],
            "median": s["median"],
            "spread": s["spread_max_over_min"],
            "loud": s["loud"],
            "hard_class": hard,
            "salt_invariant": not s["eval_sensitive"],
        }
        if not s["loud"] and len(bs) >= 2:
            for i in range(len(bs)):
                for j in range(len(bs)):
                    if i != j:
                        quiet_pairs.append(
                            (case, bs[i][0] / bs[j][0]))

    band = [min(p[1] for p in quiet_pairs), max(p[1] for p in quiet_pairs)]
    n_invariant = sum(1 for c in cases.values() if c["salt_invariant"])
    hard_cases = [c for c, d in cases.items() if d["hard_class"]]
    loud_cases = [c for c, d in cases.items() if d["loud"]]

    calibration = {
        "pinned": {
            "W_LO": W_LO,
            "W_HI": W_HI,
            "direction_agreement": AGREEMENT,
            "basin_granularity_rel": BASIN_REL,
            "hard_class": {
                "definition": "m20-m23 family or baseline median >= 10M child evals",
                "cases": hard_cases,
            },
            "loud_definition": f"uncensored spread >= {LOUD_SPREAD}x or mixed censoring",
            "loud_cases": loud_cases,
            "under_budget_rule": "> ceil(S/2) censored salts on a case for either arm"
                                 " => under-budgeted; contributes no win/regression",
            "adopt_rule": ">= 1 censored-win or win on a hard-class case AND no"
                          " regression/censored-loss on any pre-registered case",
            "reject_rule": "any regression or censored-loss",
            "else": "defer (extend the salt set on the affected cases first)",
        },
        "noise": {
            "quiet_cross_salt_band": band,
            "quiet_band_note": "min/max ratio over distinct draws within every"
                               " quiet case: how far pure noise moves a"
                               " between-salt comparison; single-draw gates are"
                               " inside this band and are retired",
            "salt_invariant_cases": n_invariant,
            "correlated_salt_pairs": data["correlated_salt_pairs"],
        },
        "cases": cases,
    }
    with open(os.path.join(STATE, "calibration.json"), "w") as f:
        json.dump(calibration, f, indent=1)

    print(f"pinned: W_LO={W_LO} W_HI={W_HI} agreement>={AGREEMENT:.0%}"
          f" basin_rel={BASIN_REL}")
    print(f"hard class ({len(hard_cases)}):", hard_cases)
    print(f"loud ({len(loud_cases)}):", loud_cases)
    print(f"salt-invariant cases: {n_invariant}/{len(cases)}")
    print(f"quiet cross-salt noise band: [{band[0]:.3f}, {band[1]:.3f}]")
    print()
    hdr = f"{'case':<12}{'median':>13}{'spread':>8}{'basins':>7}{'loud':>6}{'hard':>6}"
    print(hdr)
    for case, d in cases.items():
        print(f"{case:<12}{d['median'] if d['median'] else float('nan'):>13,.0f}"
              f"{d['spread'] if d['spread'] else 1.0:>8.3f}"
              f"{d['basin_count']:>7}{str(d['loud']):>6}{str(d['hard_class']):>6}")


if __name__ == "__main__":
    main()
