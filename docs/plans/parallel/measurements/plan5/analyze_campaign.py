#!/usr/bin/env python3
"""plan5b campaign analysis: medians, speedups, inflation, verdict inputs."""
import json
import statistics as st
from pathlib import Path

HERE = Path(__file__).resolve().parent
recs = [json.loads(l) for l in open(HERE / "state/campaign_raw.jsonl")]
SEQ = {"m22": 14_156_269, "shuffle_win": 249_480_478}
CAPS = {"m22": 30, "shuffle_win": 100}

out = {}
for case in ("m22", "shuffle_win"):
    rs = [r for r in recs if r["case"] == case]
    by = {}
    for r in rs:
        by.setdefault(r["threads"], []).append(r)
    seq_wall = st.median(r["wall_s"] for r in by[1])
    case_out = {"seq_wall_median_s": round(seq_wall, 3),
                "seq_walls": [r["wall_s"] for r in by[1]],
                "seq_child_evals": SEQ[case], "per_N": {}}
    for n in (2, 3, 4):
        g = by[n]
        walls = [r["wall_s"] for r in g]
        med = st.median(walls)
        totals = sorted(r["coordinator_evals"] + r["helper_evals"] for r in g)
        med_tot = st.median(totals)
        cap_hits = sum(1 for r in g if r["wall_s"] >= CAPS[case] - 0.5)
        decisive = [r["outcome"] for r in g if r["wall_s"] < CAPS[case] - 0.5]
        entry = {
            "walls": walls, "wall_median_s": round(med, 3),
            "speedup": round(seq_wall / med, 3),
            "outcomes": [r["outcome"] for r in g],
            "decisive_outcomes": decisive,
            "cap_hits": cap_hits,
            "total_evals": totals,
            "inflation_median": round(med_tot / SEQ[case], 3),
            "coordinator_evals": [r["coordinator_evals"] for r in g],
            "helper_evals": [r["helper_evals"] for r in g],
            "helper_share": [round(r["helper_evals"] /
                                   (r["helper_evals"] + r["coordinator_evals"]), 2)
                             for r in g],
        }
        case_out["per_N"][n] = entry
    out[case] = case_out

json.dump(out, open(HERE / "state/campaign_summary.json", "w"), indent=1)
for case, d in out.items():
    print(f"== {case}  seq {d['seq_wall_median_s']}s  seq_evals {d['seq_child_evals']}")
    for n, e in d["per_N"].items():
        print(f"  N={n}: speedup {e['speedup']}x  inflation {e['inflation_median']}x  "
              f"cap_hits {e['cap_hits']}  decisive {set(e['decisive_outcomes'])}  "
              f"helper_share {e['helper_share']}")
