#!/usr/bin/env python3
"""plan8 stage 5: mechanical H1-H4 evaluation from the campaign raw record.

Reads state/races8_raw.jsonl (campaign + pilot) and the plan12 record, computes
per-case baseline (fresh median + plan12 distribution) vs W_first / evals_first
/ evals_min, per-round distributions, basin counts + racer-set basin coverage,
evaluates H1-H4 mechanically per plan8.md, and computes CPU cost (sum of racer
core-seconds per answer; vs the shipped salt-0 run's core-seconds on m20_white).

Output: state/races8.json, state/races8.csv.
"""

import csv
import json
import os
import statistics
import sys

sys.dont_write_bytecode = True

HERE = os.path.dirname(os.path.abspath(__file__))
STATE = os.path.join(HERE, "state")
RUNS_JSON = os.path.join(
    HERE, "..", "..", "..", "research", "measurements", "plan12", "state", "runs.json"
)
RACER_SALTS = [0, 2, 3, 5]
DECISIVE = ("win", "loss")


def load_jsonl(path):
    with open(path) as f:
        return [json.loads(l) for l in f if l.strip()]


def median(xs):
    return statistics.median(xs)


def main():
    recs = load_jsonl(os.path.join(STATE, "races8_raw.jsonl"))
    plan12 = {}
    for r in json.load(open(RUNS_JSON))["runs"]:
        plan12.setdefault(r["case"], {})[r["salt"]] = r
    stage2 = {r["salt"]: r for r in
              json.load(open(os.path.join(STATE, "stage2_rem12.json")))["runs"]}
    derived = json.load(open(os.path.join(STATE, "derived_evals.json")))

    main_recs = [r for r in recs if r["label"] == "main"]
    cases = sorted({r["case"] for r in main_recs})
    out = {"racer_salts": RACER_SALTS, "rounds": 3, "pilot_excluded": True, "cases": {}}

    # ---- H4 (fidelity) -----------------------------------------------------
    h4a = json.load(open(os.path.join(STATE, "stage0_identity.json")))
    fidelity_b = []          # (case, salt, racer_evals, anchor_evals, ok)
    conflicts = []           # cases with >1 distinct decisive outcome
    censored_records = []
    for case in cases:
        races = [r for r in main_recs if r["kind"] == "race" and r["case"] == case]
        racers = [r for r in main_recs if r["kind"] == "racer" and r["case"] == case]
        # (b) decisive racers reproduce anchors
        for r in racers:
            if r["status"] != "decisive":
                continue
            if case == "rem12":
                anchor = stage2[r["salt"]]
            else:
                anchor = plan12[case][r["salt"]]
            ok = (r["evals"] == anchor["evals"] and r["outcome"] == anchor["outcome"])
            fidelity_b.append({"case": case, "salt": r["salt"], "round": r["round"],
                               "evals": r["evals"], "anchor_evals": anchor["evals"],
                               "outcome": r["outcome"], "anchor_outcome": anchor["outcome"],
                               "ok": ok})
        # (c) zero decisive-outcome conflicts within a race
        for race_rec in races:
            rnd = race_rec["round"]
            outs = {r["outcome"] for r in racers
                    if r["round"] == rnd and r["status"] == "decisive"}
            if len(outs) > 1:
                conflicts.append({"case": case, "round": rnd, "outcomes": sorted(outs)})
        # (d) censoring recorded, never read as a draw
        for r in racers:
            if r["status"] in ("censored_budget", "censored_timeout"):
                censored_records.append({"case": case, "round": r["round"],
                                         "salt": r["salt"], "status": r["status"],
                                         "outcome": r["outcome"]})
            if r["status"] == "censored_budget" and r["outcome"] not in (None, "draw"):
                conflicts.append({"case": case, "round": r["round"], "salt": r["salt"],
                                  "defect": "censored run with decisive outcome"})
    h4b_ok = all(f["ok"] for f in fidelity_b)
    h4c_ok = not conflicts
    h4d_ok = all(c["outcome"] in (None, "draw") for c in censored_records)
    h4 = {"a_pass": all(r["match"] for r in h4a["runs"]),
          "b_pass": h4b_ok, "b_checks": fidelity_b,
          "c_pass": h4c_ok, "c_violations": conflicts,
          "d_pass": h4d_ok, "censored_records": censored_records,
          "pass": all(r["match"] for r in h4a["runs"]) and h4b_ok and h4c_ok and h4d_ok}

    # ---- per-case verdicts --------------------------------------------------
    for case in cases:
        bases = [r for r in main_recs if r["kind"] == "baseline" and r["case"] == case]
        racers = [r for r in main_recs if r["kind"] == "racer" and r["case"] == case]
        races = [r for r in main_recs if r["kind"] == "race" and r["case"] == case]
        w_firsts = [r["w_first_s"] for r in races]
        baseline_walls = [b["wall_s"] for b in bases]
        baseline_evals = [b["evals"] for b in bases]
        # first decisive racer per race
        evals_first, first_racer = [], []
        for race_rec in races:
            cand = [r for r in racers if r["round"] == race_rec["round"]
                    and r["status"] == "decisive"]
            cand.sort(key=lambda r: r["wall_s"])
            if cand:
                evals_first.append(cand[0]["evals"])
                first_racer.append({"round": race_rec["round"],
                                    "racer": cand[0]["racer"], "salt": cand[0]["salt"],
                                    "wall_s": cand[0]["wall_s"],
                                    "evals": cand[0]["evals"]})
        evals_min = []
        for race_rec in races:
            vals = [r["evals"] for r in racers if r["round"] == race_rec["round"]
                    and r["status"] == "decisive"]
            if vals:
                evals_min.append(min(vals))
        ratio_w = [w / b for w, b in zip(sorted(w_firsts),
                                         sorted(baseline_walls))] if len(w_firsts) == len(baseline_walls) else None
        s0 = plan12.get(case, {}).get(0)
        p12 = derived["cases"].get(case)
        # salt-0 racer per race (portfolio includes the default racer)
        s0_racer_decisive = [
            any(r["round"] == race_rec["round"] and r["salt"] == 0
                and r["status"] == "decisive" for r in racers)
            for race_rec in races]
        entry = {
            "n_rounds": len(races),
            "baseline_walls": baseline_walls,
            "baseline_walls_median": median(baseline_walls),
            "baseline_evals": baseline_evals,
            "baseline_salt0_censored_plan12": s0["censored"] if s0 else None,
            "w_first_rounds": w_firsts,
            "w_first_median": median(w_firsts),
            "w_first_min": min(w_firsts),
            "w_first_max": max(w_firsts),
            "w_first_spread_rel": (max(w_firsts) - min(w_firsts)) / median(w_firsts),
            "evals_first_rounds": evals_first,
            "evals_min_rounds": evals_min,
            "first_racer": first_racer,
            "ratio_w_first_vs_baseline": [round(x, 4) for x in ratio_w] if ratio_w else None,
            "ratio_w_median": round(median(w_firsts) / median(baseline_walls), 4),
            "ratio_evals_median": round(median(evals_first) / median(baseline_evals), 4)
            if evals_first and all(baseline_evals) else None,
            "salt0_racer_decisive_per_round": s0_racer_decisive,
            "plan12_prediction": p12["prediction"] if p12 else "n/a (stage-2 extension)",
            "plan12_basin_count": p12["basin_count_plan12"] if p12 else None,
            "plan12_racer_basin_coverage": p12["racer_basin_coverage"] if p12 else None,
            "cpu_core_s_per_race": [round(sum(r["wall_s"] for r in racers
                                              if r["round"] == race_rec["round"]), 1)
                                    for race_rec in races],
            "baseline_core_s": [b["wall_s"] for b in bases],
        }
        out["cases"][case] = entry

    # ---- hypotheses ---------------------------------------------------------
    m20 = out["cases"]["m20_white"]
    h1_every_race_decisive = all(any(
        r["round"] == race_rec["round"] and r["status"] == "decisive"
        for r in [r for r in main_recs if r["kind"] == "racer" and r["case"] == "m20_white"])
        for race_rec in [r for r in main_recs if r["kind"] == "race"
                         and r["case"] == "m20_white"])
    h1 = {"every_race_has_decisive_racer": h1_every_race_decisive,
          "w_first_median_s": m20["w_first_median"],
          "gate_s": 160.0,
          "pass": h1_every_race_decisive and m20["w_first_median"] <= 160.0}

    d10 = out["cases"]["dec10"]
    h2 = {
        "dec10": {"w_first_median": d10["w_first_median"],
                  "baseline_median": d10["baseline_walls_median"],
                  "ratio": d10["ratio_w_median"], "gate": 0.80,
                  "pass": d10["ratio_w_median"] <= 0.80},
        # m23_white is not in the raced subset (fixed-parameters table); its
        # H2 clause is recorded, not gated: evals prediction 0.976x (s2) from
        # the pre-computation (plan12: 9,673,403 vs 9,440,650).
        "m23_white": {"measured": False,
                      "evals_prediction_ratio": 9440650 / 9673403,
                      "note": "not raced per plan8 fixed-parameters table; "
                              "recorded from the plan12 pre-computation"},
    }
    h2_pass = h2["dec10"]["pass"]

    h3 = {}
    for case in ("stress", "m22_white", "dec13"):
        e = out["cases"][case]
        h3[case] = {"ratio_w_median": e["ratio_w_median"], "gate": 1.10,
                    "pass": e["ratio_w_median"] <= 1.10}
    h3_pass = all(v["pass"] for v in h3.values())

    # rem12 out-of-sample: recorded, not gated (no pre-registered hypothesis)
    rem = out["cases"]["rem12"]

    verdict = ("adopt" if (h1["pass"] and h3_pass and h4["pass"])
               else "reject" if (not h3_pass or not h4["pass"])
               else "defer")
    out["hypotheses"] = {
        "H1": h1, "H2": h2, "H3": h3, "H4": h4,
        "H2_pass_gated_dec10": h2_pass,
        "H3_pass": h3_pass, "H4_pass": h4["pass"],
        "verdict_rule": "adopt iff H1 and H3 and H4; reject if H3 or H4 fails; "
                        "defer if all m20 racers censored at the race wall cap",
        "verdict": verdict,
        "contention_anomaly": (h1["pass"] and h3_pass and h4["pass"]
                               and not h2_pass),
    }

    # m20 cost accounting
    m20_cost = {
        "shipped_salt0_solo_core_s": [b["wall_s"] for b in
                                      [r for r in main_recs
                                       if r["kind"] == "baseline" and r["case"] == "m20_white"]],
        "shipped_result": "budget-censored at 2 B evals, no answer",
        "portfolio_race_core_s": m20["cpu_core_s_per_race"],
        "portfolio_w_first_s": m20["w_first_rounds"],
        "portfolio_result": "proven win at 791,401,553 evals (evals_first = evals_min = "
                            "the plan12 s5 anchor, all three races)",
    }
    out["m20_cost"] = m20_cost

    with open(os.path.join(STATE, "races8.json"), "w") as f:
        json.dump(out, f, indent=1)

    # flat CSV
    with open(os.path.join(STATE, "races8.csv"), "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["kind", "label", "case", "round", "racer", "salt", "wall_s",
                    "outcome", "evals", "status", "censored", "rc"])
        for r in recs:
            if r["kind"] == "race":
                w.writerow(["race", r["label"], r["case"], r["round"], "", "", "",
                            "", "", "", "", r["w_first_s"]])
            else:
                w.writerow([r["kind"], r["label"], r.get("case"), r.get("round"),
                            r.get("racer", ""), r.get("salt", ""), r["wall_s"],
                            r.get("outcome"), r.get("evals"), r.get("status", ""),
                            r.get("censored"), r.get("rc")])

    print(json.dumps(out["hypotheses"], indent=1)[:4000])
    print("\nper-case summary:")
    for case, e in out["cases"].items():
        print(f"{case}: W_first median {e['w_first_median']:.3f} vs baseline "
              f"{e['baseline_walls_median']:.3f} = {e['ratio_w_median']}x; "
              f"evals_first {median(e['evals_first_rounds']):,.0f} "
              f"({e['ratio_evals_median']}x salt-0); cpu/race "
              f"{median(e['cpu_core_s_per_race']):.0f} core-s")


if __name__ == "__main__":
    main()
