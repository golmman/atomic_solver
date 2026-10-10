#!/usr/bin/env python3
"""Parse the plan16 rollout raw outputs into the committed state/ files.

    python3 parse.py [RAW_DIR]   (default: $RAW or /tmp/plan16/results)

Reads `<case>_s<salt>_<arm>.out/.err/.meta.json` written by driver.py plus
the Phase-0 anchor files `<arm>_<case>_s<salt>.out/.err` (D2(c), from
ANCHOR_DIR) and writes, next to this script:

    state/runs.json      per-run record (arm, evals, outcome, censoring, wall)
    state/summary.json   per-case-per-arm record + D2 checks

Hard failures (exit 1, any violation HALTs per plan16 D2):

    D2(a) pristine identity: the baseline-arm salt-0 evals equal the plan12
          recorded salt-0 values exactly (the pre-hook pristine-build check
          itself is recorded in env.json; here the hook-unset build must
          still reproduce them).
    D2(b) hook non-perturbation: the baseline arm at every salt equals
          plan12's recorded per-salt values exactly (evals AND censoring
          status), all 132 cells.
    D2(c) plan15 anchor reproduction: the 12 init-on cells measured in
          plan15's P3 (stress @ 300 M cap, dec13/dec10/m22 @ 1 B) reproduce
          plan15's archived `state/arms.json` values exactly.
    D2(e) zero decisive-outcome conflicts within any arm across salts, and
          every uncensored run matches its fixture expected outcome (the
          init steers only — solved values are computed exactly as before —
          so an arm-level outcome flip is a soundness defect, not noise).
    D2(f) a budget-censored draw is the no-result sentinel, never a proven
          draw (censored runs are excluded from every outcome set; a
          censored run marked decisive is a failure).
"""

import glob
import json
import os
import re
import statistics
import sys

sys.dont_write_bytecode = True

HERE = os.path.dirname(os.path.abspath(__file__))
RAW = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("RAW", "/tmp/plan16/results")
ANCHOR_DIR = os.environ.get("ANCHOR_DIR", "/tmp/plan16/raw")
OUT = os.path.join(HERE, "state")
PLAN12 = os.path.join(HERE, "..", "plan12", "state")
PLAN15 = os.path.join(HERE, "..", "plan15", "state")

# D2(a) baselines: plan12 salt-0 child evals.
SALT0_BASELINE = {
    "stress": 249_480_478,
    "m22_white": 14_156_269,
    "m23_white": 9_673_403,
    "m20_white": None,  # censored at the 2 B cap in plan12 (identity = censored)
    "dec13": 3_822_602,
    "dec10": 4_262_128,
}

# D2(c) anchors: plan15 P3 archived values (arms.json) + their run budgets.
# Anchor files: <arm>_<case>_s<salt>.out/.err in ANCHOR_DIR.
ANCHOR_BUDGET = {"stress": 300_000_000}  # others 1 B (plan15 P3 budgets)
with open(os.path.join(PLAN15, "arms.json")) as f:
    _arms15 = json.load(f)["arms"]
ANCHORS = {}
for _arm in ("mob-or", "mobcap8"):
    ANCHORS[(_arm, "stress", 0)] = _arms15[_arm]["stress"]["child_evals"]
    ANCHORS[(_arm, "dec13", 0)] = _arms15[_arm]["dec13"]["child_evals"]
    ANCHORS[(_arm, "dec10", 0)] = _arms15[_arm]["dec10"]["child_evals"]
    for _s, _key in ((0, "m22_s0"), (1, "m22_s1"), (2, "m22_s2")):
        ANCHORS[(_arm, "m22", _s)] = _arms15[_arm][_key]["child_evals"]

NAME = re.compile(r"^(?P<case>m20_white|stress|m22_white|m23_white|dec\d\d)"
                  r"_s(?P<salt>\d+)_(?P<arm>base|mob-or|mobcap8)$")
BUDGET = {"stress": 2_500_000_000, "m20_white": 2_000_000_000}
BUDGET_DEFAULT = 1_000_000_000
ARMS = ["base", "mob-or", "mobcap8"]

EXPECTED = {
    "m20_white": "win", "stress": "win", "m22_white": "win", "m23_white": "win",
    "dec01": "win", "dec02": "loss", "dec03": "win", "dec04": "win",
    "dec05": "win", "dec06": "loss", "dec07": "win", "dec08": "win",
    "dec09": "win", "dec10": "win", "dec11": "win", "dec12": "win",
    "dec13": "win", "dec14": "win", "dec15": "win", "dec16": "win",
    "dec17": "win", "dec18": "win",
}


def parse_run(out_path, eval_budget, arm=None, case=None, salt=None):
    base = os.path.basename(out_path)[: -len(".out")]
    with open(out_path) as fo:
        stdout = fo.read()
    with open(out_path[:-len(".out")] + ".err") as fe:
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
    meta_path = out_path[:-len(".out")] + ".meta.json"
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

    # D2(a): baseline salt-0 identity on the cases with a known count.
    for case, want in SALT0_BASELINE.items():
        r0 = by_key[(case, 0, "base")]
        if want is None:
            if not (r0["censored"] or (r0["decisive"]
                                       and r0["outcome"] == EXPECTED[case])):
                failures.append({"check": "D2a_salt0_identity_m20",
                                 "got": {"outcome": r0["outcome"],
                                         "censored": r0["censored"],
                                         "evals": r0["evals"]}})
            continue
        if r0["evals"] != want or r0["censored"]:
            failures.append({"check": "D2a_salt0_identity", "case": case,
                             "expected_evals": want, "got": r0["evals"],
                             "censored": r0["censored"]})

    # D2(b): baseline arm at every salt vs plan12's recorded values.
    with open(os.path.join(PLAN12, "summary.json")) as f:
        plan12 = json.load(f)["summary"]
    for case in EXPECTED:
        ref = plan12[case]["per_salt"]
        for salt in range(6):
            r = by_key[(case, salt, "base")]
            want = ref[str(salt)]
            if r["evals"] != want["evals"] or r["censored"] != want["censored"]:
                failures.append({"check": "D2b_baseline_persalt", "case": case,
                                 "salt": salt, "expected": want,
                                 "got": {"evals": r["evals"],
                                         "censored": r["censored"]}})

    # D2(c): the 12 plan15 anchor cells (anchor files, plan15 budgets).
    anchor_results = {}
    for (arm, case, salt), want in sorted(ANCHORS.items()):
        out_path = os.path.join(ANCHOR_DIR, f"anchor_{arm}_{case}_s{salt}.out")
        if not os.path.exists(out_path):
            failures.append({"check": "D2c_anchor_missing", "arm": arm,
                             "case": case, "salt": salt, "path": out_path})
            continue
        r = parse_run(out_path, ANCHOR_BUDGET.get(case, BUDGET_DEFAULT), arm, case, salt)
        ok = r["evals"] == want and not r["censored"]
        anchor_results[f"{arm}_{case}_s{salt}"] = {
            "expected": want, "got": r["evals"],
            "censored": r["censored"], "ok": ok}
        if not ok:
            failures.append({"check": "D2c_anchor_reproduction", "arm": arm,
                             "case": case, "salt": salt, "expected": want,
                             "got": r["evals"], "censored": r["censored"]})

    # D2(e)/(f): outcome checks per case per arm.
    summary = {}
    for case in EXPECTED:
        for arm in ARMS:
            cruns = sorted((r for r in runs if r["case"] == case and r["arm"] == arm),
                           key=lambda r: r["salt"])
            for r in cruns:
                if r["censored"] and r["decisive"]:
                    failures.append({"check": "D2f_censored_decisive",
                                     "case": case, "salt": r["salt"], "arm": arm})
            decisive_outcomes = {r["outcome"] for r in cruns if r["decisive"]}
            if len(decisive_outcomes) > 1:  # D2(e): conflicts across salts
                failures.append({"check": "D2e_outcome_flip", "case": case,
                                 "arm": arm, "outcomes": sorted(decisive_outcomes)})
            for r in cruns:
                if r["decisive"] and r["outcome"] != EXPECTED[case]:
                    failures.append({"check": "D2e_fixture_mismatch", "case": case,
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
        json.dump({"summary": summary, "anchors": anchor_results,
                   "failures": failures}, f, indent=1)

    for case in EXPECTED:
        for arm in ARMS:
            s = summary[case][arm]
            print(f"{case:10s} {arm:8s} uncensored: {s['uncensored_evals']} "
                  f"censored: {[(c['salt'], c['by']) for c in s['censored']]} "
                  f"outcomes: {s['decisive_outcomes']}")
    if failures:
        print("HARD FAILURES (HALT):", json.dumps(failures, indent=1))
        sys.exit(1)
    print(f"OK: D2(a),(b),(c),(e),(f) pass ({len(runs)} rollout runs, "
          f"{len(anchor_results)} anchor cells)")


if __name__ == "__main__":
    main()
