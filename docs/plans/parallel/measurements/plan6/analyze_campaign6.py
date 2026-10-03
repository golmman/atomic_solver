#!/usr/bin/env python3
"""plan6 campaign analysis: medians, speedups, inflation, cap-hits, agreement.

Reads state/campaign6_raw.jsonl, writes state/campaign6_summary.json and
state/campaign6.csv. N=1 walls are the in-session denominator (per-round
pairs; medians over all rounds). Inflation = median total child_evals
(helper+coordinator) / sequential child_evals reference. Agreement counts
decisive runs whose outcome equals the sequential outcome; cap-hits
(resource-cut draws at the wall cap) are counted separately.
"""
import json
import statistics
import csv
from pathlib import Path

HERE = Path(__file__).resolve().parent
STATE = HERE / "state"

SEQ_EVALS = {
    "m22": 14_156_269,
    "rem12": 69_605_385,
    "shuffle_win": 249_480_478,
}
SEQ_OUTCOME = {"m22": "win", "rem12": "win", "shuffle_win": "win"}
CASES = ["m22", "rem12", "shuffle_win"]


def main():
    recs = [json.loads(l) for l in open(STATE / "campaign6_raw.jsonl")]
    summary = {"cases": {}}
    csv_rows = []
    for case in CASES:
        rs = [r for r in recs if r["case"] == case]
        n1_walls = {r["round"]: r["wall_s"] for r in rs if r["threads"] == 1}
        seq_ref_wall = statistics.median(n1_walls.values())
        per_n = {}
        for n in (2, 3, 4):
            runs = [r for r in rs if r["threads"] == n]
            walls = [r["wall_s"] for r in runs]
            totals = [r["helper_evals"] + r["coordinator_evals"] for r in runs]
            helper_share = [r["helper_evals"] / t for r, t in zip(runs, totals)]
            decisive = [r for r in runs if r["outcome"] != "draw"]
            agree = sum(1 for r in decisive if r["outcome"] == SEQ_OUTCOME[case])
            cap_hits = sum(1 for r in runs if r["outcome"] == "draw" and r["wall_s"] >= r["cap_s"] - 0.5)
            panics = sum(1 for r in runs if r["rc"] != 0)
            med_wall = statistics.median(walls)
            med_total = statistics.median(totals)
            per_n[n] = {
                "runs": len(runs),
                "walls_s": walls,
                "median_wall_s": round(med_wall, 3),
                "median_speedup_vs_n1": round(seq_ref_wall / med_wall, 3),
                "totals_evals": totals,
                "median_total_evals": round(med_total),
                "inflation_vs_seq": round(med_total / SEQ_EVALS[case], 3),
                "helper_share_median": round(statistics.median(helper_share), 3),
                "decisive": len(decisive),
                "agreement": f"{agree}/{len(decisive)}",
                "cap_hits": cap_hits,
                "cap_hit_rate": round(cap_hits / len(runs), 3),
                "nonzero_rc": panics,
                "median_wall_paired_mean_n1": round(
                    statistics.median(
                        [n1_walls[r["round"]] for r in runs if r["round"] in n1_walls]
                    ),
                    3,
                ),
            }
            csv_rows.append([case, n, len(runs), round(med_wall, 3),
                             round(seq_ref_wall / med_wall, 3),
                             round(med_total / SEQ_EVALS[case], 3),
                             cap_hits, f"{agree}/{len(decisive)}", panics])
        summary["cases"][case] = {
            "seq_ref_wall_s": round(seq_ref_wall, 3),
            "seq_evals": SEQ_EVALS[case],
            "per_threads": per_n,
            "n1_walls": [n1_walls[k] for k in sorted(n1_walls)],
        }

    # N in {8, 16}: log-fit extrapolations on the three real points (S(N) = a + b ln N).
    ext = {}
    for case in CASES:
        pts = [(1.0, summary["cases"][case]["seq_ref_wall_s"])]
        for n in (2, 3, 4):
            pts.append((float(n), summary["cases"][case]["per_threads"][n]["median_wall_s"]))
        xs = [__import__("math").log(x) for x, _ in pts]
        ys = [y for _, y in pts]
        mx, my = statistics.mean(xs), statistics.mean(ys)
        b = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sum((x - mx) ** 2 for x in xs)
        a = my - b * mx
        ext[case] = {
            "model": "S(N) = a + b*ln(N) on medians at N=1,2,3,4 (speedup form)",
            "speedup": {str(n): round(pts[0][1] / (a + b * __import__("math").log(n)), 2) for n in (8, 16)},
            "label": "EXTRAPOLATION ONLY — not a wall measurement, non-gating",
        }
    summary["extrapolations_n8_n16"] = ext

    # H2 trend: Spearman rank correlation between case length (seq wall) and S4.
    lengths = {"m22": summary["cases"]["m22"]["seq_ref_wall_s"],
               "rem12": summary["cases"]["rem12"]["seq_ref_wall_s"],
               "shuffle_win": summary["cases"]["shuffle_win"]["seq_ref_wall_s"]}
    s4 = {c: summary["cases"][c]["per_threads"][4]["median_speedup_vs_n1"] for c in CASES}
    def rank(d):
        order = sorted(d, key=d.get)
        return {k: i for i, k in enumerate(order)}
    rl, rs = rank(lengths), rank(s4)
    n = len(CASES)
    d2 = sum((rl[c] - rs[c]) ** 2 for c in CASES)
    summary["length_trend"] = {
        "seq_walls_s": lengths,
        "s4": s4,
        "spearman_rho": round(1 - 6 * d2 / (n * (n * n - 1)), 3),
    }
    json.dump(summary, open(STATE / "campaign6_summary.json", "w"), indent=2)
    with open(STATE / "campaign6.csv", "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["case", "N", "runs", "median_wall_s", "speedup_vs_n1",
                    "inflation_vs_seq", "cap_hits", "agreement", "nonzero_rc"])
        w.writerows(csv_rows)
    print(json.dumps(summary["cases"], indent=2))
    print("length_trend:", json.dumps(summary["length_trend"]))
    print("extrapolations:", json.dumps(ext))


if __name__ == "__main__":
    main()
