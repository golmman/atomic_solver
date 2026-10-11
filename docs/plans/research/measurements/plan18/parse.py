#!/usr/bin/env python3
"""Parse the plan18 rollout raw outputs into the committed state/ files.

    python3 parse.py [RAW_DIR]   (default: $RAW or /tmp/plan18/results)

Reads `<case>_s<salt>_<arm>.out/.err/.meta.json` written by driver.py and
writes, next to this script:

    state/runs.json      per-run record (arm, evals, outcome, censoring, wall)
    state/summary.json   per-case-per-arm record + D3 checks

Hard failures (exit 1, any violation HALTs per plan18 D3):

    D3(b) baseline-arm identity: the base arm at every salt equals plan12's
          recorded per-salt values exactly (evals AND censoring status),
          all 132 cells — the null is degenerate; any mismatch is an
          identity failure, not noise. (The pre-hook pristine-build check
          D3(a) itself is recorded in env.json.)
    D3(d) outcome invariants: zero decisive-outcome conflicts within any arm
          across salts, and every uncensored run of every arm matches its
          fixture expected outcome. A constant re-tune reorders children
          only — solved values are computed exactly as before — so any
          outcome flip is a soundness defect, never noise (the family's
          whole soundness surface; plan18 D3(d)).
    D3(f)-structural: a budget-censored draw is the no-result sentinel,
          never a proven draw (censored runs are excluded from every
          outcome set; a censored run marked decisive is a failure).

The D3(c) hook fidelity anchors ((c0) liveness probes, (c1) env-unset quick
byte-identity, (c2) LEAN10_STATS counter anchor, (c3) candidate-arm quick
per-case fidelity) run before the rollout on the hook build and are recorded
in env.json, not here.
"""

import glob
import json
import os
import re
import statistics
import sys

sys.dont_write_bytecode = True

HERE = os.path.dirname(os.path.abspath(__file__))
RAW = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("RAW", "/tmp/plan18/results")
OUT = os.path.join(HERE, "state")
PLAN12 = os.path.join(HERE, "..", "plan12", "state")

NAME = re.compile(r"^(?P<case>m20_white|stress|m22_white|m23_white|dec\d\d)"
                  r"_s(?P<salt>\d+)_(?P<arm>base|hs2ag1|hs05ag1)$")
BUDGET = {"stress": 2_500_000_000, "m20_white": 2_000_000_000}
BUDGET_DEFAULT = 1_000_000_000
ARMS = ["base", "hs2ag1", "hs05ag1"]

EXPECTED = {
    "m20_white": "win", "stress": "win", "m22_white": "win", "m23_white": "win",
    "dec01": "win", "dec02": "loss", "dec03": "win", "dec04": "win",
    "dec05": "win", "dec06": "loss", "dec07": "win", "dec08": "win",
    "dec09": "win", "dec10": "win", "dec11": "win", "dec12": "win",
    "dec13": "win", "dec14": "win", "dec15": "win", "dec16": "win",
    "dec17": "win", "dec18": "win",
}


def parse_run(out_path, eval_budget, arm=None, case=None, salt=None):
    with open(out_path) as fo:
        stdout = fo.read()
    with open(out_path[: -len(".out")] + ".err") as fe:
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
    meta_path = out_path[: -len(".out")] + ".meta.json"
    if os.path.exists(meta_path):
        with open(meta_path) as fm:
            wall = json.load(fm)["wall_s"]
    return {
        "case": case, "salt": salt, "arm": arm,
        "outcome": outcome, "decisive": decisive, "censored": censored_by,
        "evals": evals, "budget": eval_budget, "wall_s": wall,
    }


def main():
    runs = []
    for out_path in sorted(glob.glob(os.path.join(RAW, "*.out"))):
        base = os.path.basename(out_path)[: -len(".out")]
        m = NAME.match(base)
        if not m:
            raise SystemExit(f"unparsed run name: {base}")
        case, salt, arm = m["case"], int(m["salt"]), m["arm"]
        r = parse_run(out_path, BUDGET.get(case, BUDGET_DEFAULT), arm, case, salt)
        runs.append(r)
    if not runs:
        raise SystemExit("no runs found")

    failures = []
    by_key = {(r["case"], r["salt"], r["arm"]): r for r in runs}

    # D3(b): baseline arm at every salt vs plan12's recorded values.
    with open(os.path.join(PLAN12, "summary.json")) as f:
        plan12 = json.load(f)["summary"]
    for case in EXPECTED:
        ref = plan12[case]["per_salt"]
        for salt in range(6):
            r = by_key[(case, salt, "base")]
            want = ref[str(salt)]
            if r["evals"] != want["evals"] or r["censored"] != want["censored"]:
                failures.append({"check": "D3b_baseline_persalt", "case": case,
                                 "salt": salt, "expected": want,
                                 "got": {"evals": r["evals"],
                                         "censored": r["censored"]}})

    # D3(d)/structural: outcome checks per case per arm.
    summary = {}
    for case in EXPECTED:
        for arm in ARMS:
            cruns = sorted((r for r in runs if r["case"] == case and r["arm"] == arm),
                           key=lambda r: r["salt"])
            if not cruns:
                continue  # arm not (yet) rolled out
            for r in cruns:
                if r["censored"] and r["decisive"]:
                    failures.append({"check": "D3d_censored_decisive",
                                     "case": case, "salt": r["salt"], "arm": arm})
            decisive_outcomes = {r["outcome"] for r in cruns if r["decisive"]}
            if len(decisive_outcomes) > 1:  # conflicts across salts
                failures.append({"check": "D3d_outcome_flip", "case": case,
                                 "arm": arm, "outcomes": sorted(decisive_outcomes)})
            for r in cruns:
                if r["decisive"] and r["outcome"] != EXPECTED[case]:
                    failures.append({"check": "D3d_fixture_mismatch", "case": case,
                                     "arm": arm, "salt": r["salt"],
                                     "expected": EXPECTED[case], "got": r["outcome"]})
            uncensored = [r["evals"] for r in cruns if not r["censored"]]
            summary.setdefault(case, {})[arm] = {
                "decisive_outcomes": sorted(decisive_outcomes),
                "n_censored": sum(1 for r in cruns if r["censored"]),
                "uncensored_evals": sorted(uncensored),
                "median": statistics.median(uncensored) if uncensored else None,
                "censored": [
                    {"salt": r["salt"], "by": r["censored"], "evals": r["evals"],
                     "outcome": r["outcome"]}
                    for r in cruns if r["censored"]
                ],
                "per_salt": {
                    str(r["salt"]): {"evals": r["evals"], "censored": r["censored"],
                                     "outcome": r["outcome"]}
                    for r in cruns
                },
            }

    with open(os.path.join(OUT, "runs.json"), "w") as f:
        json.dump({"runs": runs}, f, indent=1)
    with open(os.path.join(OUT, "summary.json"), "w") as f:
        json.dump({"summary": summary, "failures": failures}, f, indent=1)

    for case in EXPECTED:
        for arm in ARMS:
            if arm not in summary.get(case, {}):
                continue  # arm not (yet) rolled out
            s = summary[case][arm]
            print(f"{case:10s} {arm:8s} uncensored: {s['uncensored_evals']} "
                  f"censored: {[(c['salt'], c['by']) for c in s['censored']]} "
                  f"outcomes: {s['decisive_outcomes']}")
    if failures:
        print("HARD FAILURES (HALT):", json.dumps(failures, indent=1))
        sys.exit(1)
    print(f"OK: D3(b),(d) + structural pass ({len(runs)} rollout runs)")


if __name__ == "__main__":
    main()
