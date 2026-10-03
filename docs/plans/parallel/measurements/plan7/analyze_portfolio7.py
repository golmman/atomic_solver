#!/usr/bin/env python3
"""plan7 analysis: baseline vs portfolio-race W_first distributions, H1-H3.

Reads state/portfolio7_raw.jsonl (label="main" rounds only; pilot excluded),
emits state/portfolio7_results.json + state/portfolio7.csv. H1-H3 evaluated
mechanically per plan7.md's pre-registered bands.
"""
import json
import statistics
import sys
from pathlib import Path

sys.dont_write_bytecode = True

HERE = Path(__file__).resolve().parent
STATE = HERE / "state"

CASES = ["m22", "rem12", "shuffle_win"]
RACERS = ["P0", "P1", "P2", "P3"]


def load():
    runs = {"baseline": {}, "racer": {}, "race": {}}
    with open(STATE / "portfolio7_raw.jsonl") as f:
        for line in f:
            rec = json.loads(line)
            if rec.get("label") != "main":
                continue
            if rec["kind"] == "baseline":
                runs["baseline"].setdefault(rec["case"], []).append(rec)
            elif rec["kind"] == "racer":
                runs["racer"].setdefault((rec["case"], rec["racer"]), []).append(rec)
            elif rec["kind"] == "race":
                runs["race"].setdefault(rec["case"], []).append(rec)
    return runs


def med(xs):
    return round(statistics.median(xs), 3)


def main():
    runs = load()
    results = {"cases": {}, "hypotheses": {}}
    csv_lines = ["case,kind,wall_s,outcome,cap_hit,rc"]
    for case in CASES:
        base = runs["baseline"].get(case, [])
        races = runs["race"].get(case, [])
        w_first = [r["w_first_s"] for r in races if r["w_first_s"] is not None]
        racer_recs = []
        for rp in RACERS:
            racer_recs.extend(runs["racer"].get((case, rp), []))
        # H3 soundness: every decisive racer == win; zero panics; cap-hits counted separately
        cap_hits = [r for r in racer_recs if r["cap_hit"]]
        panics = [r for r in racer_recs if r["rc"] != 0]
        violations = [r for r in racer_recs
                      if r["outcome"] is not None and r["outcome"] != "win" and not r["cap_hit"]]
        # CPU cost: sum of all racer wall-seconds per race, averaged per decisive answer
        per_race_cpu = []
        for r in range(1, 9):
            walls = [x["wall_s"] for x in racer_recs if x["round"] == r]
            if walls:
                per_race_cpu.append(round(sum(walls), 3))
        b_med, w_med = med([r["wall_s"] for r in base]), med(w_first)
        b_max, w_max = round(max(r["wall_s"] for r in base), 3), round(max(w_first), 3)
        results["cases"][case] = {
            "n_baseline": len(base), "n_races": len(races),
            "baseline_walls": [r["wall_s"] for r in base],
            "baseline_median_s": b_med, "baseline_max_s": b_max,
            "w_first_values": w_first, "w_first_median_s": w_med, "w_first_max_s": w_max,
            "ratio_median": round(w_med / b_med, 3) if b_med else None,
            "racer_medians_s": {rp: med([r["wall_s"] for r in runs["racer"].get((case, rp), [])])
                                for rp in RACERS},
            "racer_cap_hits": len(cap_hits), "racer_panics": len(panics),
            "soundness_violations": len(violations),
            "cpu_core_s_per_race_median": med(per_race_cpu),
            "cpu_core_s_per_decisive_answer": round(med(per_race_cpu) / len(w_first), 3) if w_first else None,
        }
        for r in base:
            csv_lines.append(f"{case},baseline,{r['wall_s']},{r['outcome']},{r['cap_hit']},{r['rc']}")
        for r in races:
            csv_lines.append(f"{case},w_first,{r['w_first_s']},{r['w_first_s'] is not None},False,0")

    # H1 (shuffle-win): median W_first <= 0.75x baseline median AND race max <= baseline max
    sw = results["cases"]["shuffle_win"]
    h1 = (sw["w_first_median_s"] <= 0.75 * sw["baseline_median_s"]
          and sw["w_first_max_s"] <= sw["baseline_max_s"])
    # H2 (m22, rem12): median W_first <= 1.05x baseline median
    h2 = all(results["cases"][c]["w_first_median_s"] <= 1.05 * results["cases"][c]["baseline_median_s"]
             for c in ("m22", "rem12"))
    # H3 (hard gate): zero violations, zero panics, zero cap-hits among racers
    h3 = (all(results["cases"][c]["soundness_violations"] == 0
              and results["cases"][c]["racer_panics"] == 0
              and results["cases"][c]["racer_cap_hits"] == 0 for c in CASES))
    results["hypotheses"] = {
        "H1_bimodal_benefits": {"pass": h1,
                                "detail": f"shuffle-win: W_first median {sw['w_first_median_s']}s vs baseline median {sw['baseline_median_s']}s (ratio {sw['ratio_median']}), max {sw['w_first_max_s']} vs {sw['baseline_max_s']}"},
        "H2_unimodal_no_regression": {"pass": h2,
                                      "detail": {c: results["cases"][c]["ratio_median"] for c in ("m22", "rem12")}},
        "H3_soundness": {"pass": h3,
                         "detail": {c: {"violations": results["cases"][c]["soundness_violations"],
                                        "panics": results["cases"][c]["racer_panics"],
                                        "cap_hits": results["cases"][c]["racer_cap_hits"]} for c in CASES}},
    }
    with open(STATE / "portfolio7_results.json", "w") as f:
        json.dump(results, f, indent=2)
    with open(STATE / "portfolio7.csv", "w") as f:
        f.write("\n".join(csv_lines) + "\n")
    print(json.dumps(results["hypotheses"], indent=2))
    for c in CASES:
        r = results["cases"][c]
        print(f"{c}: base med {r['baseline_median_s']} (max {r['baseline_max_s']}) | "
              f"W_first med {r['w_first_median_s']} (max {r['w_first_max_s']}) | "
              f"ratio {r['ratio_median']} | racers {r['racer_medians_s']} | "
              f"CPU/race {r['cpu_core_s_per_race_median']}s")


if __name__ == "__main__":
    main()
