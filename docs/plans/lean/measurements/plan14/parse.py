#!/usr/bin/env python3
"""Parse the plan14 rollout raw outputs into the committed state/ files.

    python3 parse.py [RAW_DIR]   (default: $RAW or <measurements>/raw)

Reads `<case>_s<salt>.out/.err/.meta.json` written by driver.py (plus the
`<case>_s0r1` within-build determinism reruns) and writes, next to this
script:

    state/runs.json      per-run record (evals, outcome, censoring, wall)
    state/summary.json   per-case record + soundness checks

Hard failures (exit 1):
    S1  within-build determinism: every `<case>_s0r1` rerun must reproduce
        the salt-0 stdout byte-for-byte and child_evals exactly (plan14
        renegotiated soundness invariant 1; shipped-trajectory identity is
        N/A — arm A intentionally changes search behavior).
    S2  zero decisive-outcome conflicts across salts on any case
        (unchanged gate v1.0 invariant 2; a conflict is a soundness
        defect — halt and diagnose).
    S3  every uncensored decisive run matches its fixture expectation.
"""

import glob
import json
import os
import re
import statistics
import sys

sys.dont_write_bytecode = True

HERE = os.path.dirname(os.path.abspath(__file__))
RAW = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("RAW", os.path.join(HERE, "raw"))
OUT = os.path.join(HERE, "state")

NAME = re.compile(r"^(?P<case>m20_white|stress|m22_white|m23_white|dec\d\d)"
                  r"_s(?P<salt>\d)(?:r1)?$")
BUDGET = {"stress": 2_500_000_000, "m20_white": 2_000_000_000}
BUDGET_DEFAULT = 1_000_000_000

EXPECTED = {
    "m20_white": "win", "stress": "win", "m22_white": "win", "m23_white": "win",
    "dec01": "win", "dec02": "loss", "dec03": "win", "dec04": "win",
    "dec05": "win", "dec06": "loss", "dec07": "win", "dec08": "win",
    "dec09": "win", "dec10": "win", "dec11": "win", "dec12": "win",
    "dec13": "win", "dec14": "win", "dec15": "win", "dec16": "win",
    "dec17": "win", "dec18": "win",
}


def parse_run(base, case):
    with open(os.path.join(RAW, base + ".out")) as fo:
        stdout = fo.read()
    with open(os.path.join(RAW, base + ".err")) as fe:
        stderr = fe.read()
    outcome = None
    for line in stdout.splitlines():
        if line.startswith("outcome: "):
            outcome = line.split()[1]
    evals = None
    for line in stderr.splitlines():
        if line.startswith("evals: "):
            evals = int(line.split()[1])
    censored_by = None
    if "budget exhausted" in stdout:
        censored_by = "budget"
    elif stdout.rstrip().endswith("timeout"):
        censored_by = "wall"
    decisive = outcome is not None and outcome != "draw"
    if decisive:
        censored_by = None
    wall = None
    meta_path = os.path.join(RAW, base + ".meta.json")
    if os.path.exists(meta_path):
        with open(meta_path) as fm:
            wall = json.load(fm)["wall_s"]
    return {
        "name": base,
        "outcome": outcome,
        "decisive": decisive,
        "censored": censored_by,
        "evals": evals,
        "budget": BUDGET.get(case, BUDGET_DEFAULT),
        "wall_s": wall,
        "stdout_bytes": len(stdout),
    }, stdout


def main():
    runs = []
    stdouts = {}
    for out_path in sorted(glob.glob(os.path.join(RAW, "*.out"))):
        base = os.path.basename(out_path)[: -len(".out")]
        m = NAME.match(base)
        if not m:
            raise SystemExit(f"unparsed run name: {base}")
        r, stdout = parse_run(base, m["case"])
        r["case"] = m["case"]
        r["salt"] = int(m["salt"])
        r["rerun"] = base.endswith("r1")
        runs.append(r)
        stdouts[base] = stdout
    if not runs:
        raise SystemExit("no runs found")

    failures = []

    # S1: within-build determinism (salt-0 rerun identity).
    for case in EXPECTED:
        r0 = next(r for r in runs if r["case"] == case and r["name"] == f"{case}_s0")
        r1 = next(r for r in runs if r["case"] == case and r["name"] == f"{case}_s0r1")
        same = (stdouts[f"{case}_s0"] == stdouts[f"{case}_s0r1"]
                and r0["evals"] == r1["evals"])
        if not same:
            failures.append({
                "check": "S1_within_build_determinism",
                "case": case,
                "stdout_equal": stdouts[f"{case}_s0"] == stdouts[f"{case}_s0r1"],
                "evals0": r0["evals"], "evals1": r1["evals"],
            })

    summary = {}
    for case in {r["case"] for r in runs}:
        cruns = sorted((r for r in runs if r["case"] == case and not r["rerun"]),
                       key=lambda r: r["salt"])
        # S2: conflicts among decisive outcomes across salts.
        decisive_outcomes = {r["outcome"] for r in cruns if r["decisive"]}
        if len(decisive_outcomes) > 1:
            failures.append(
                {"check": "S2_outcome_flip", "case": case,
                 "outcomes": sorted(decisive_outcomes)})
        # S3: fixture expectation on every uncensored run.
        for r in cruns:
            if r["decisive"] and r["outcome"] != EXPECTED[case]:
                failures.append(
                    {"check": "S3_fixture_mismatch", "case": case,
                     "salt": r["salt"], "expected": EXPECTED[case],
                     "got": r["outcome"]})
        uncensored = [r["evals"] for r in cruns if not r["censored"]]
        censored = [r for r in cruns if r["censored"]]
        spread = (max(uncensored) / min(uncensored)) if len(uncensored) >= 2 else None
        summary[case] = {
            "expected": EXPECTED[case],
            "decisive_outcomes": sorted(decisive_outcomes),
            "n_censored": len(censored),
            "n_runs": len(cruns),
            "uncensored_evals": sorted(uncensored),
            "censored": [
                {"salt": r["salt"], "by": r["censored"], "evals": r["evals"]}
                for r in censored
            ],
            "min": min(uncensored) if uncensored else None,
            "median": statistics.median(uncensored) if uncensored else None,
            "max": max(uncensored) if uncensored else None,
            "spread_max_over_min": spread,
        }

    with open(os.path.join(OUT, "runs.json"), "w") as f:
        json.dump({"runs": runs}, f, indent=1)
    with open(os.path.join(OUT, "summary.json"), "w") as f:
        json.dump({"summary": summary, "failures": failures}, f, indent=1)

    for case, s in sorted(summary.items()):
        print(case, "decisive:", s["decisive_outcomes"],
              "uncensored:", s["uncensored_evals"],
              "censored:", [(c["salt"], c["by"]) for c in s["censored"]])
    if failures:
        print("HARD FAILURES:", json.dumps(failures, indent=1))
        sys.exit(1)
    print("OK: S1 (within-build determinism) / S2 (no outcome flips) / "
          "S3 (fixture expectations) all pass")


if __name__ == "__main__":
    main()
