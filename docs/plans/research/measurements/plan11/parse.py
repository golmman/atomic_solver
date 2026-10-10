#!/usr/bin/env python3
"""Parse the plan11 pilot raw outputs into the committed state/ files.

    python3 parse.py [RAW_DIR]   (default: $RAW or /tmp/plan11/results)

Reads `<case>_s<salt>[rerun].out/.err/.meta.json` written by driver.py and
writes, next to this script:

    state/runs.json      per-run record (evals, outcome, censoring, wall)
    state/summary.json   per-case D6 analysis (min/median/max over uncensored,
                         censored count, spread, sensitivity classification)

D6 classification: a case is salt-sensitive iff (max/min >= 1.5x among
uncensored draws) or (any censored draw while another salt solves).
"""

import glob
import json
import os
import re
import statistics
import sys

sys.dont_write_bytecode = True

HERE = os.path.dirname(os.path.abspath(__file__))
RAW = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("RAW", "/tmp/plan11/results")
OUT = os.path.join(HERE, "state")

NAME = re.compile(r"^(?P<case>m20_white|stress|m22_white|m23_white|dec13|dec10)"
                  r"_s(?P<salt>\d+)(?P<tag>rerun)?$")
BUDGET = {"stress": 2_500_000_000}
BUDGET_DEFAULT = 1_000_000_000


def parse_run(case, salt, tag):
    name = f"{case}_s{salt}{tag}"
    out_path = os.path.join(RAW, name + ".out")
    err_path = os.path.join(RAW, name + ".err")
    with open(out_path) as fo, open(err_path) as fe:
        stdout = fo.read()
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
    meta_path = os.path.join(RAW, name + ".meta.json")
    if os.path.exists(meta_path):
        with open(meta_path) as fm:
            wall = json.load(fm)["wall_s"]
    return {
        "case": case,
        "salt": salt,
        "tag": tag or "primary",
        "outcome": outcome,
        "decisive": decisive,
        "censored": censored_by,
        "evals": evals,
        "budget": BUDGET.get(case, BUDGET_DEFAULT),
        "wall_s": wall,
    }


def main():
    runs = []
    for out_path in sorted(glob.glob(os.path.join(RAW, "*.out"))):
        base = os.path.basename(out_path)[: -len(".out")]
        m = NAME.match(base)
        if not m:
            raise SystemExit(f"unparsed run name: {base}")
        if m["tag"]:
            continue  # duplicates handled after their primary
        tag = "rerun" if base.endswith("rerun") else ""
        runs.append(parse_run(m["case"], int(m["salt"]), tag))
    duplicates = [parse_run(m.group(1), int(m["salt"]), "rerun")
                  for m in (NAME.match(os.path.basename(p)[: -len(".out")])
                            for p in glob.glob(os.path.join(RAW, "*rerun.out")))
                  if m]
    with open(os.path.join(OUT, "runs.json"), "w") as f:
        json.dump({"runs": runs, "duplicates": duplicates}, f, indent=1)

    summary = {}
    flips = []
    for case in {r["case"] for r in runs}:
        cruns = [r for r in runs if r["case"] == case and r["tag"] == "primary"]
        # D5(c): a soundness defect is conflicting *decisive* outcomes (win
        # vs loss). The `draw` of a budget-censored run is the documented
        # no-result sentinel, not a proven draw — it is censoring, not a
        # flip (matches reexamination.md's protocol).
        decisive_outcomes = {r["outcome"] for r in cruns if r["decisive"]}
        if len(decisive_outcomes) > 1:
            flips.append({"case": case, "outcomes": sorted(decisive_outcomes)})
        uncensored = [r["evals"] for r in cruns if not r["censored"]]
        censored = [r for r in cruns if r["censored"]]
        spread = (max(uncensored) / min(uncensored)) if len(uncensored) >= 2 else None
        summary[case] = {
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
            # D6: sensitive iff spread >= 1.5x among uncensored, or mixed
            # censored/uncensored.
            "salt_sensitive": bool(
                (spread is not None and spread >= 1.5)
                or (censored and uncensored)
            ),
        }
    with open(os.path.join(OUT, "summary.json"), "w") as f:
        json.dump({"summary": summary, "outcome_flips": flips, "duplicates": duplicates},
                  f, indent=1)

    for case, s in summary.items():
        print(case, "decisive:", s["decisive_outcomes"],
              "uncensored:", s["uncensored_evals"],
              "censored:", [(c["salt"], c["by"]) for c in s["censored"]],
              "spread:", s["spread_max_over_min"], "sensitive:", s["salt_sensitive"])
    if flips:
        print("OUTCOME FLIPS (D5(c) violation):", flips)
        sys.exit(1)
    print("OK: no outcome flips across salts")


if __name__ == "__main__":
    main()
