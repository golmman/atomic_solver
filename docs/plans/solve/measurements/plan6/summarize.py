#!/usr/bin/env python3
"""Aggregate plan6 arm results: medians, speedup, inflation, gates."""
import glob
import json
import os
import statistics

HERE = os.path.dirname(os.path.abspath(__file__))
STATE = os.path.join(HERE, "state")


def load(pattern):
    rows = []
    for f in sorted(glob.glob(os.path.join(STATE, pattern))):
        d = json.load(open(f))
        if "rep" in d:
            rows.append(d)
    return rows


def med(xs):
    return statistics.median(xs) if xs else None


def main():
    out = {}
    for pos in ["m22", "shuffle"]:
        seq = load(f"seq_{pos}_rep*.json")
        if not seq:
            continue
        s = {
            "wall_med": med([r["wall_s"] for r in seq]),
            "nodes_med": med([r.get("nodes") or r.get("worker_nodes") or 0 for r in seq]),
            "child_evals_med": med(
                [r.get("child_evals") or r.get("worker_child_evals") or 0 for r in seq]
            ),
            "outcome": seq[0]["outcome"],
            "reps": len(seq),
            "censored": sum(1 for r in seq if r.get("exit") == "timeout"),
        }
        out[f"{pos}/S"] = s
        # arm token: 2-worker arms use <ARM>, 4-worker C4' arms use <ARM>w4
        arm_files = sorted(
            f for f in glob.glob(os.path.join(STATE, f"{pos}_*_rep*.json"))
            if "summary" not in f
        )
        arms = {}
        for f in arm_files:
            base = os.path.basename(f)[: -len("_rep0.json")]
            name, arm = base.split("_", 1)
            if name != pos:
                continue
            arms.setdefault(arm, []).append(f)
        for arm, files in sorted(arms.items()):
            rows = [json.load(open(f)) for f in files]
            proven = [r for r in rows if r.get("exit") == "proven"]
            a = {
                "workers": rows[0].get("workers"),
                "reps": len(rows),
                "proven": len(proven),
                "censored": len(rows) - len(proven),
                "wall_med_all": med([r["wall_s"] for r in rows]),
                "wall_med_proven": med([r["wall_s"] for r in proven]) if proven else None,
                "child_evals_med_all": med([r["worker_child_evals"] for r in rows]),
                "child_evals_med_proven": med(
                    [r["worker_child_evals"] for r in proven]
                )
                if proven
                else None,
                "jobs_abandoned_total": sum(r.get("jobs_abandoned", 0) for r in rows),
                "verify_failures_total": sum(r.get("verify_failures", 0) for r in rows),
                "job_errors_total": sum(r.get("job_errors", 0) for r in rows),
                "validate_ok_proven": sum(1 for r in proven if r.get("validate") == "ok"),
            }
            if s["wall_med"] and a["wall_med_proven"]:
                a["wall_speedup_proven"] = s["wall_med"] / a["wall_med_proven"]
            if s["child_evals_med"] and a["child_evals_med_proven"]:
                a["work_inflation_proven"] = (
                    a["child_evals_med_proven"] / s["child_evals_med"]
                )
            out[f"{pos}/{arm}"] = a
    with open(os.path.join(STATE, "summary.json"), "w") as f:
        json.dump(out, f, indent=2)
    print(json.dumps(out, indent=2))


if __name__ == "__main__":
    main()
