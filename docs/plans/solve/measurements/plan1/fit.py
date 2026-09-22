#!/usr/bin/env python3
"""Plan 1 fit: growth model, d_e, W, pre-registered gate verdict.

Reads state/cost_*_p*.json (128 MB runs), random_playouts.json,
state/ladder_*.json and state/menhist_summary.json; writes fit.json and
a human-readable summary to stdout. All formulas are pre-registered in
README.md ("Pre-registered analysis formulas") and were fixed before
the cost data was fitted.
"""

from __future__ import annotations

import glob
import json
import math
import os
import re
import statistics

ROOT = os.path.dirname(os.path.abspath(__file__))
STATE = os.path.join(ROOT, "state")


def load_cost():
    rows = []
    for p in sorted(glob.glob(os.path.join(STATE, "cost_*_p*.json"))):
        if re.search(r"_\d+mb(_p\d+)?\.json$", os.path.basename(p)):
            continue  # addendum runs
        with open(p) as f:
            rows.append(json.load(f))
    return rows


def ols_log_b(points):
    """Least squares of log10(nodes) vs ply; returns (b_per_ply, intercept, n)."""
    n = len(points)
    if n < 2:
        return None, None, n
    xs = [p for p, _ in points]
    ys = [math.log10(v) for _, v in points]
    mx, my = sum(xs) / n, sum(ys) / n
    sxx = sum((x - mx) ** 2 for x in xs)
    if sxx == 0:
        return None, None, n
    sxy = sum((x - mx) * (y - my) for x, y in zip(xs, ys))
    slope = sxy / sxx
    return 10**slope, my - slope * mx, n


def main():
    rows = load_cost()
    fit_rows = [r for r in rows if not r.get("informational_seed_position")]
    uncens = [r for r in fit_rows if not r["censored"]]

    plies = sorted({r["ply"] for r in fit_rows})
    by_ply = {p: [r for r in fit_rows if r["ply"] == p] for p in plies}
    frac_uncens = {
        p: sum(1 for r in by_ply[p] if not r["censored"]) / len(by_ply[p]) for p in plies
    }
    # Pre-registered pooled window: plies where the pooled uncensored
    # fraction is >= 50%, spanning lowest..highest such ply.
    window = [p for p in plies if frac_uncens[p] >= 0.5]
    fit_pts = [(r["ply"], r["nodes"]) for r in uncens if r["ply"] in window]
    b_pooled, a_pooled, n_pooled = ols_log_b(fit_pts)

    lines = []
    for p in sorted(glob.glob(os.path.join(STATE, "ladder_*.json"))):
        line = os.path.basename(p)[7:-5]
        with open(p) as f:
            lad = json.load(f)
        steered = [e["ply"] for e in lad["positions"] if e.get("steer") == "pv"]
        stop = lad["positions"][-1]
        pts = []
        for e in lad["positions"]:
            if e["ply"] < 2:
                continue
            r = next((x for x in rows if x["line"] == line and x["ply"] == e["ply"]), None)
            if r and not r["censored"] and r["nodes"]:
                pts.append((e["ply"], r["nodes"]))
        bl, al, nl = None, None, 0
        if pts:
            pls = sorted({pp for pp, _ in pts})
            span = [(pp, v) for pp, v in pts if min(pls) <= pp <= max(pls)]
            bl, al, nl = ols_log_b(span)
        lines.append(
            {
                "line": line,
                "b_per_line": bl,
                "uncensored_points": nl,
                "uncensored_plies": sorted(pp for pp, _ in pts),
                "deepest_pv_steered_ply": max(steered) if steered else None,
                "last_recorded_ply": stop["ply"],
                "stop_reason": stop["stop_reason"],
                # the line ends ~1-2 plies past the last recorded position
                "terminal_ply_est": stop["ply"] + 2,
            }
        )

    terminal_plies = sorted(l["terminal_ply_est"] for l in lines)
    d_liq = statistics.median(terminal_plies)

    # d_e (men) from the instrumentation summary
    d_e_men = None
    shares = {}
    mh_path = os.path.join(STATE, "menhist_summary.json")
    if os.path.exists(mh_path):
        with open(mh_path) as f:
            mh = json.load(f)
        shares = mh.get("cumulative_nonterminal_shares", {})
        total = mh.get("nonterminal_total", 0)
        cum = 0.0
        for m in sorted(int(k) for k in mh.get("nonterminal_by_men", {})):
            cum += mh["nonterminal_by_men"][str(m)]
            if total and cum / total >= 0.95 and d_e_men is None:
                d_e_men = m

    # d_e (plies): plies-to-terminal horizon within which >=95% of random
    # trajectories sit at men <= m* (m* = 5 and 6, the generable candidates)
    rp = json.load(open(os.path.join(ROOT, "random_playouts.json")))
    d_e_plies = {}
    for mstar in (5, 6):
        fracs = []
        for g in rp["games"]:
            path = g["men_path"]
            k = 0
            for i in range(len(path) - 1, -1, -1):
                if path[i] <= mstar:
                    k += 1
                else:
                    break
            fracs.append(k)
        d_e_plies[mstar] = sorted(fracs)[int(0.95 * len(fracs))]

    d_eff = None
    W = None
    verdict = None
    if b_pooled:
        # Anchoring credit only if a generable men-count actually covers
        # >=95% of leaf sites; otherwise d_e contributes ~0 plies.
        d_eff = max(d_liq - (d_e_men if d_e_men is not None else 0), 0)
        W = b_pooled**d_eff
        half_reach_below_30 = sum(1 for l in lines if l["last_recorded_ply"] < 30) >= len(lines) / 2
        if W <= 1e15 and not half_reach_below_30:
            verdict = "GO"
        elif W <= 1e17 and not half_reach_below_30:
            verdict = "MARGINAL"
        else:
            verdict = "RETHINK"
    else:
        verdict = "RETHINK"

    out = {
        "n_cost_runs": len(rows),
        "n_censored": len([r for r in fit_rows if r["censored"]]),
        "n_uncensored": len(uncens),
        "uncensored_fraction_by_ply": frac_uncens,
        "pre_registered_window_plies": [window[0], window[-1]] if window else None,
        "b_pooled_per_ply": b_pooled,
        "fit_points_pooled": n_pooled,
        "per_line": lines,
        "d_liq_median_terminal_ply": d_liq,
        "terminal_plies_est": terminal_plies,
        "d_e_men_95pct": d_e_men,
        "nonterminal_cumulative_shares": shares,
        "d_e_plies_horizon_random": d_e_plies,
        "D_eff_used": d_eff,
        "W_projected_nodes": W,
        "gate_verdict": verdict,
    }
    with open(os.path.join(ROOT, "fit.json"), "w") as f:
        json.dump(out, f, indent=1, default=str)
    print(json.dumps(out, indent=1, default=str))


if __name__ == "__main__":
    main()
