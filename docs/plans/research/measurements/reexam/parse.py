#!/usr/bin/env python3
"""Parse the raw probe outputs into the committed state/ files.

    python3 parse.py [RAW_DIR]   (default: $PROBE_RESULTS or /tmp/probe/results)

Inputs: r{1..5,7}_*.json (one probe_solve JSON line each) and r6_*.json
(benchmark --json suite outputs). Outputs: state/runs.json,
state/suites.json, state/derived.json next to this script.
"""

import glob
import json
import os
import re
import sys

sys.dont_write_bytecode = True

RAW = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("PROBE_RESULTS", "/tmp/probe/results")
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "state")
CASES = ("stress", "m22", "dec13", "dec10")
NAME = re.compile(
    r"^r(?P<group>[1-57])_(?P<case>stress|m22|dec13|dec10)"
    r"(?:_tt(?P<tt>\d+))?(?:_d(?P<depth>\d+))?(?:_(?P<arm>mob|wpns))?"
    r"(?:_s(?P<salt>\d+))?(?:_c(?P<chunk0>\d+))?$"
)


def load_runs():
    runs = {}
    for path in sorted(glob.glob(os.path.join(RAW, "r[1-57]_*.json"))):
        name = os.path.basename(path)[: -len(".json")]
        m = NAME.match(name)
        if not m:
            raise SystemExit(f"unparsed run name: {name}")
        with open(path) as f:
            line = f.read().strip().splitlines()[-1]
        d = json.loads(line)
        d.pop("fen", None)
        d.update(
            group=f"R{m['group']}",
            case=m["case"],
            arm=m["arm"] or "baseline",
            salt=int(m["salt"] or 0),
            chunk0=int(m["chunk0"] or 500_000),
        )
        runs[name] = d
    return runs


def load_suites():
    suites = {}
    for path in sorted(glob.glob(os.path.join(RAW, "r6_*.json"))):
        name = os.path.basename(path)[: -len(".json")]
        with open(path) as f:
            d = json.load(f)
        suites[name] = {
            "suite": d["suite"],
            "mode": d["mode"],
            "timeout_s": d["timeout"],
            "tt_size": d["tt_size"],
            "aggregates": d["aggregates"],
            "cases": [
                {k: c[k] for k in ("name", "status", "outcome", "expected", "child_evals", "pv_len", "timeout", "wrong")}
                for c in d["results"]
            ],
        }
    return suites


def evals(run):
    """child_evals, or None when the run was cut by its budget (censored)."""
    return run["child_evals"] if run["outcome"] != "draw" else None


def derive(runs, suites):
    base = {c: runs[f"r1_{c}"] for c in CASES}
    d = {}
    # First-sweep anatomy (own evals of frames cut on their first sweep; such
    # frames never recurse, so own == descendant and the share is additive).
    d["first_sweep_share"] = {
        c: {
            "total_child_evals": r["child_evals"],
            "first_sweep_evals_share": sum(r["first_sweep_cut_evals"]) / r["child_evals"],
            "or_share": r["first_sweep_cut_evals"][0] / r["child_evals"],
            "and_share": r["first_sweep_cut_evals"][1] / r["child_evals"],
            "fresh_frame_share": sum(r["first_sweep_cut_fresh"]) / sum(r["first_sweep_cut"]),
            "stale_after_return_evals_share": r["stale_evals_after"] / r["child_evals"],
        }
        for c, r in base.items()
    }
    # Salt noise: salt 0 = shipped baseline; None = censored at the budget.
    d["salt_noise"] = {
        c: {str(s): evals(base[c] if s == 0 else runs[f"r4_{c}_s{s}"]) for s in range(5)} for c in CASES
    }
    # Arms vs baseline (salt 0), ratio None when either side is censored.
    arms = {}
    for arm in ("mob", "wpns"):
        arms[arm] = {}
        for c in CASES:
            per_salt = {}
            for s in range(5):
                key = f"r5_{c}_{arm}" + (f"_s{s}" if s else "")
                if key in runs:
                    per_salt[str(s)] = evals(runs[key])
            b, a = evals(base[c]), per_salt.get("0")
            arms[arm][c] = {"evals_by_salt": per_salt, "ratio_salt0": (a / b) if a and b else None}
    d["arms"] = arms
    d["tt_size"] = {k: {"case": r["case"], "tt": r["tt"], "evals": evals(r), "pv_len": r["pv_len"]}
                    for k, r in runs.items() if r["group"] == "R2"}
    d["depth_cap"] = {k: {"case": r["case"], "depth": r["depth"], "evals": evals(r), "pv_len": r["pv_len"]}
                      for k, r in runs.items() if r["group"] == "R3"}
    d["chunk0"] = {k: {"case": r["case"], "chunk0": r["chunk0"], "evals": evals(r), "chunks": r["chunks"],
                       "pv_len": r["pv_len"]}
                   for k, r in runs.items() if r["group"] == "R7"}
    d["suites"] = {k: v["aggregates"] for k, v in suites.items()}
    d["wrong_outcomes"] = sum(v["aggregates"]["wrong"] for v in suites.values()) + sum(
        1 for r in runs.values() if r["outcome"] not in ("win", "draw")
    )
    return d


def main():
    runs = load_runs()
    suites = load_suites()
    os.makedirs(OUT, exist_ok=True)
    for name, obj in (("runs", runs), ("suites", suites), ("derived", derive(runs, suites))):
        with open(os.path.join(OUT, f"{name}.json"), "w") as f:
            json.dump(obj, f, indent=1, sort_keys=True)
            f.write("\n")
    print(f"{len(runs)} runs, {len(suites)} suites -> {OUT}")


if __name__ == "__main__":
    main()
