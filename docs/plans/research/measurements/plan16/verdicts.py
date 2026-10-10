#!/usr/bin/env python3
"""Compute plan16 verdicts per the pinned gate (gate_methodology.md v1.0).

    python3 verdicts.py

Reads state/summary.json (written by parse.py) and plan12's committed
state/calibration.json (baseline basin counts) and writes:

    state/verdicts.json   per-case-per-arm verdict tables (D3) plus the
                          lever-level verdict per arm, the D4/D6 notes, and
                          the paired-ratio dispersion summary (the
                          comparison rule's second real-arm calibration
                          data point)

Verdict rules (pinned v1.0, applied verbatim per plan16 D3):
    censored-win   candidate solves where baseline is censored on >= 1 salt,
                   and is never censored where baseline solves
    censored-loss  mirror image
    win            median paired ratio <= 0.9 AND >= 75% of ratios < 1
    regression     median paired ratio >= 1.1 AND >= 75% of ratios > 1
    unclear        none of the above
    under-budget   > 3 of 6 salts censored on either arm -> case
                   unresolved, contributes nothing (plan16 D3 under-budget
                   rule)

Lever verdict per arm (D3):
    adopt   >= 1 censored-win or win on a hard-class case (stress, m20_white,
            m22_white, m23_white) AND no regression / censored-loss anywhere;
            a low-diversity baseline (basin count < 3) cannot solely support
            an adopt
    reject  any regression or censored-loss (D6: applied verbatim, including
            at dec10 — a gate-grade dec10 regression rejects the arm; an
            unclear dec10 does not)
    defer   otherwise (D4: one salt-set extension {6, 7} on the affected
            cases, both arms, before a second defer stands)

D6 calibration question (recorded as data, never applied as a rule): when
an arm's *sole* negative is a dec10 regression whose paired ratios sit
inside/at the edge of the quiet band [0.760, 1.316], the arm output carries
a `d6_calibration_question` note for the report.
"""

import json
import os
import sys

sys.dont_write_bytecode = True

HERE = os.path.dirname(os.path.abspath(__file__))
PLAN12 = os.path.join(HERE, "..", "plan12", "state")

W_LO = 0.9
W_HI = 1.1
AGREE = 0.75
UNDER_BUDGET_MAX_CENSORED = 3  # > 3 of 6 -> under-budgeted
QUIET_LO, QUIET_HI = 0.760, 1.316  # pinned noise band (methodology premise)
HARD_CLASS = ["stress", "m20_white", "m22_white", "m23_white"]
CAND_ARMS = ["mob-or", "mobcap8"]
CASES = ["m20_white", "stress", "m22_white", "m23_white"] + [f"dec{i:02d}" for i in range(1, 19)]


def classify_verdict(base, cand):
    """D3 verdict for one (case, candidate arm) from per-salt records."""
    ratios = []
    pattern = {}
    base_only = cand_only = False
    for salt in range(6):
        b = base["per_salt"][str(salt)]
        c = cand["per_salt"][str(salt)]
        bc, cc = b["censored"], c["censored"]
        if bc and cc:
            pattern[salt] = "both"
        elif bc:
            pattern[salt] = "base_only"
            base_only = True
        elif cc:
            pattern[salt] = "cand_only"
            cand_only = True
        else:
            pattern[salt] = "neither"
            ratios.append(c["evals"] / b["evals"])
    n_cens_b = sum(1 for v in pattern.values() if v in ("base_only", "both"))
    n_cens_c = sum(1 for v in pattern.values() if v in ("cand_only", "both"))
    under_budget = n_cens_b > UNDER_BUDGET_MAX_CENSORED or n_cens_c > UNDER_BUDGET_MAX_CENSORED

    if under_budget:
        verdict = "unresolved_under_budget"
    elif base_only and not cand_only:
        verdict = "censored-win"
    elif cand_only and not base_only:
        verdict = "censored-loss"
    elif ratios:
        ratios.sort()

        def median(xs):
            n = len(xs)
            return xs[n // 2] if n % 2 else (xs[n // 2 - 1] + xs[n // 2]) / 2

        med = median(ratios)
        frac_below = sum(1 for r in ratios if r < 1) / len(ratios)
        frac_above = sum(1 for r in ratios if r > 1) / len(ratios)
        if med <= W_LO and frac_below >= AGREE:
            verdict = "win"
        elif med >= W_HI and frac_above >= AGREE:
            verdict = "regression"
        else:
            verdict = "unclear"
    else:
        verdict = "unclear"  # symmetric censoring, no paired ratios
    return {
        "verdict": verdict,
        "pattern": {str(k): v for k, v in pattern.items()},
        "n_censored_base": n_cens_b,
        "n_censored_cand": n_cens_c,
        "ratios": ratios,
        "under_budget": under_budget,
    }


def main():
    with open(os.path.join(HERE, "state", "summary.json")) as f:
        summary = json.load(f)["summary"]
    with open(os.path.join(PLAN12, "calibration.json")) as f:
        calib = json.load(f)["cases"]
    basin = {c: calib[c]["basin_count"] for c in CASES}

    verdicts = {}
    all_ratios = {}
    for arm in CAND_ARMS:
        arm_out = {"cases": {}, "lever": None}
        hard_support = []   # cases supporting adopt (censored-win/win, hard class)
        negatives = {}      # case -> (verdict) for regression / censored-loss
        for case in CASES:
            v = classify_verdict(summary[case]["base"], summary[case][arm])
            v["baseline_basin_count"] = basin[case]
            v["low_diversity"] = basin[case] < 3
            v["hard_class"] = case in HARD_CLASS
            arm_out["cases"][case] = v
            all_ratios.setdefault(arm, []).extend(v["ratios"])
            if v["verdict"] in ("regression", "censored-loss"):
                negatives[case] = v["verdict"]
            if (v["verdict"] in ("win", "censored-win") and v["hard_class"]
                    and not v["under_budget"]):
                hard_support.append(case)
        # D6 note: the sole negative is a dec10 regression whose paired
        # ratios sit inside/at the edge of the pinned quiet band.
        d6_note = None
        if set(negatives) == {"dec10"} and negatives["dec10"] == "regression":
            dr = sorted(arm_out["cases"]["dec10"]["ratios"])
            if all(QUIET_LO <= r <= QUIET_HI * 1.05 for r in dr):
                d6_note = ("sole negative is a dec10 regression with all paired "
                           f"ratios in/at the quiet band ({[round(r, 3) for r in dr]}); "
                           "methodology-calibration question for the owner, not "
                           "an applied rule")
        # Lever verdict (D3). Low-diversity basins cannot *solely* support an
        # adopt: at least one supporting case must have basin count >= 3.
        diverse_support = [c for c in hard_support if basin[c] >= 3]
        if negatives:
            arm_out["lever"] = "reject"
        elif hard_support and diverse_support:
            arm_out["lever"] = "adopt"
        else:
            arm_out["lever"] = "defer"
        arm_out["hard_class_support"] = hard_support
        arm_out["diverse_hard_class_support"] = diverse_support
        arm_out["negatives"] = negatives
        arm_out["d6_calibration_question"] = d6_note
        verdicts[arm] = arm_out

    # Paired-ratio dispersion: the comparison rule's second real-arm
    # calibration data point (the first was plan13's e375/e5 arms).
    dispersion = {}
    for arm, ratios in all_ratios.items():
        rs = sorted(ratios)
        n = len(rs)
        dispersion[arm] = {
            "n_pairs": n,
            "min": rs[0] if n else None,
            "max": rs[-1] if n else None,
            "p10": rs[int(0.10 * (n - 1))] if n else None,
            "p25": rs[int(0.25 * (n - 1))] if n else None,
            "median": rs[n // 2] if n else None,
            "p75": rs[int(0.75 * (n - 1))] if n else None,
            "p90": rs[int(0.90 * (n - 1))] if n else None,
            "frac_outside_[0.9,1.1]": (
                sum(1 for r in rs if not (W_LO <= r <= W_HI)) / n) if n else None,
        }

    out = {"verdicts": verdicts, "dispersion": dispersion,
           "pinned": {"W_LO": W_LO, "W_HI": W_HI, "agreement": AGREE,
                      "hard_class": HARD_CLASS, "quiet_band": [QUIET_LO, QUIET_HI]}}
    with open(os.path.join(HERE, "state", "verdicts.json"), "w") as f:
        json.dump(out, f, indent=1)

    for arm in CAND_ARMS:
        print(f"== arm {arm}: lever verdict = {verdicts[arm]['lever']}")
        for case in CASES:
            v = verdicts[arm]["cases"][case]
            print(f"  {case:10s} {v['verdict']:24s} basins={v['baseline_basin_count']} "
                  f"pattern={v['pattern']} ratios="
                  f"{[round(r, 3) for r in v['ratios']]}")
        if verdicts[arm]["d6_calibration_question"]:
            print(f"  D6 note: {verdicts[arm]['d6_calibration_question']}")
    print("paired-ratio dispersion:", json.dumps(dispersion, indent=1))


if __name__ == "__main__":
    main()
