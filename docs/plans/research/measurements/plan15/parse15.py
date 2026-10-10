#!/usr/bin/env python3
"""plan15 parse: raw probe JSONs + traces -> state/{anatomy,divergence,arms}.json.

anatomy    : P1 per-run derived metrics (first-sweep shares OR/AND, fresh
             share, children/ply distributions, unique share, repetition
             counters) for anchor_b + p1 runs.
divergence : P2 trace alignment (first divergence index, post-divergence
             share, ply distribution of the first 1M records).
arms       : P3 decision table: per sub-arm x case ratios vs the baseline,
             GO-qualification flags (pre-registered rules).
"""

import glob
import json
import os
import struct
import sys

sys.dont_write_bytecode = True

HERE = os.path.dirname(os.path.abspath(__file__))
RAW = os.environ.get("RAW", "/tmp/plan15/raw")
STATE = os.path.join(HERE, "state")

BASE_SALT0 = {"stress": 249_480_478, "m22": 14_156_269,
              "dec13": 3_822_602, "dec10": 4_262_128}


def load_run(name):
    return json.load(open(os.path.join(RAW, name + ".json")))


def censored(d):
    return d["exit_reason"] == "BudgetExhausted"


def anatomy():
    out = {}
    for path in sorted(glob.glob(os.path.join(RAW, "*.json"))):
        name = os.path.basename(path)[:-5]
        if not (name.startswith("anchor_b_") or name.startswith("p1_")):
            continue
        d = json.load(open(path))
        e = d["child_evals"]
        fsce = d["first_sweep_cut_evals"]
        fsc = d["first_sweep_cut"]
        rec = {
            "child_evals": e,
            "outcome": d["outcome"],
            "exit_reason": d["exit_reason"],
            "censored": censored(d),
            "nodes": d["nodes"],
            "frames_loop": d["frames_loop"],
            "evals_per_frame": round(e / d["frames_loop"], 2),
            "first_sweep_cut_share": round(sum(fsce) / e, 4),
            "first_sweep_cut_share_or": round(fsce[0] / e, 4),
            "first_sweep_cut_share_and": round(fsce[1] / e, 4),
            "first_sweep_cut_fresh_share": round(
                sum(d["first_sweep_cut_fresh"]) / max(sum(fsc), 1), 4),
            "unique_share": round(d["unique_frames"] / d["frames_loop"], 4),
            "unique_frames": d["unique_frames"],
            "unique_sat": d["unique_sat"],
            "children_hist": d["children_hist"],
            "ply_frames": d["ply_frames"],
            # descendant evals overlap across nesting (reexam caveat): the
            # ply table shows where deep recursion happens, not additive work
            "ply_evals": d["ply_evals"],
            "suppress_draw_stores": d["suppress_draw_stores"],
            "suppress_draw_clobbers": d["suppress_draw_clobbers"],
            "stale_after_return": d["stale_after_return"],
            "stale_evals_share": round(d["stale_evals_after"] / e, 4),
            "explored_marks": d["explored_marks"],
            "explored_frames": d["explored_frames"],
            "tt_bound_reuse_inf": d["tt_bound_reuse_inf"],
            "inf_unsolved_stores": d["inf_unsolved_stores"],
        }
        out[name] = rec
    return out


def load_trace(path):
    recs = []
    with open(path, "rb") as f:
        while True:
            b = f.read(9)
            if len(b) < 9:
                break
            recs.append((struct.unpack("<Q", b[:8])[0], b[8]))
    return recs


def divergence():
    out = {}
    for case in ("m22", "dec13"):
        a = load_trace(os.path.join(RAW, f"p2_{case}_base.trace"))
        b = load_trace(os.path.join(RAW, f"p2_{case}_mob.trace"))
        n = min(len(a), len(b))
        div = n
        for i in range(n):
            if a[i] != b[i]:
                div = i
                break
        out[case] = {
            "base_len": len(a),
            "mob_len": len(b),
            "mob_trace_capped": len(b) == 1 << 24,
            "first_divergence": div,
            "share_after_base": round((len(a) - div) / len(a), 4),
            "share_after_mob": round((len(b) - div) / len(b), 4),
            "base_ply_top5_first1M": _ply_top(a),
            "mob_ply_top5_first1M": _ply_top(b),
        }
    return out


def _ply_top(trace):
    from collections import Counter
    c = Counter(p for _, p in trace[:1_000_000])
    return c.most_common(5)


def arms():
    base_s0 = dict(BASE_SALT0)
    for case in ("m22",):
        for s in (1, 2):
            base_s0[f"m22_s{s}"] = load_run(f"p3_m22_base_s{s}")["child_evals"]
    out = {"baseline": base_s0, "arms": {}}
    for arm in ("mob", "mob-or", "mob-and", "mobcap8", "moblog"):
        rec = {}
        # stress/dec13/dec10 at salt 0
        for case in ("stress", "dec13", "dec10"):
            name = f"p1_{case}_{arm}" if arm == "mob" else f"p3_{case}_{arm}"
            d = load_run(name)
            r = {
                "child_evals": d["child_evals"],
                "censored": censored(d),
                "ratio_s0": round(d["child_evals"] / base_s0[case], 4),
            }
            rec[case] = r
        # m22 at salts 0,1,2
        for s in (0, 1, 2):
            name = f"p1_m22_{arm}" if arm == "mob" else f"p3_m22_{arm}_s{s}"
            d = load_run(name)
            key = "m22" if s == 0 else f"m22_s{s}"
            rec[f"m22_s{s}"] = {
                "child_evals": d["child_evals"],
                "censored": censored(d),
                "ratio": round(d["child_evals"] / base_s0[key], 4),
            }
        # pre-registered GO qualification
        direction_win = (not rec["stress"]["censored"]
                         and rec["stress"]["ratio_s0"] <= 0.9) or \
                        (not rec["dec13"]["censored"]
                         and rec["dec13"]["ratio_s0"] <= 0.9)
        m22_ok = sum(1 for s in (0, 1, 2)
                     if not rec[f"m22_s{s}"]["censored"]
                     and rec[f"m22_s{s}"]["ratio"] <= 1.5)
        rec["direction_win"] = direction_win
        rec["m22_salts_ok"] = m22_ok
        rec["qualifies_go"] = bool(direction_win and m22_ok >= 2)
        out["arms"][arm] = rec
    return out


def main():
    os.makedirs(STATE, exist_ok=True)
    a = anatomy()
    json.dump(a, open(os.path.join(STATE, "anatomy.json"), "w"), indent=1)
    v = divergence()
    json.dump(v, open(os.path.join(STATE, "divergence.json"), "w"), indent=1)
    r = arms()
    json.dump(r, open(os.path.join(STATE, "arms.json"), "w"), indent=1)
    for name, obj in (("anatomy", a), ("divergence", v), ("arms", r)):
        print(f"== {name} ==")
        print(json.dumps(obj, indent=1)[:3000])


if __name__ == "__main__":
    main()
