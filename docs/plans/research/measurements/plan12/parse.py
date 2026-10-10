#!/usr/bin/env python3
"""Parse the plan12 rollout raw outputs into the committed state/ files.

    python3 parse.py [RAW_DIR]   (default: $RAW or /tmp/plan12/results)

Reads `<case>_s<salt>.out/.err/.meta.json` written by driver.py and writes,
next to this script:

    state/runs.json      per-run record (evals, outcome, censoring, wall)
    state/summary.json   per-case record + D5 checks + D6 correlation probe

Hard failures (exit 1):
    D5(a) salt-0 identity: pilot-case salt-0 evals must equal the known
          plan11/reexamination baselines.
    D5(b) decisive-outcome conflicts across salts on any case.
    D5(c) uncensored outcome != fixture expectation.

D6 probe: salt pair (a, b) is correlated iff |e_a − e_b| / min < 1e-4 on
>= 2 EVAL-SENSITIVE cases (both uncensored). A case is eval-sensitive iff
its max pairwise relative eval difference across salts is >= 1e-3 — on
salt-invariant cases (many dec controls: identical evals on every salt)
every pair is trivially near-identical and carries no pair-specific
information. The known dec10 salt1/salt4 near-identity alone does NOT
fire the PIVOT.
"""

import glob
import json
import os
import re
import statistics
import sys

sys.dont_write_bytecode = True

HERE = os.path.dirname(os.path.abspath(__file__))
RAW = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("RAW", "/tmp/plan12/results")
OUT = os.path.join(HERE, "state")

# D5(a) baselines: plan11 pilot salt-0 child evals (== reexamination probe
# values for stress/m22/dec13/dec10).
# NB: plan11's README headline table lists the per-case MIN in its "salt 0"
# column; the authoritative per-salt values are plan11 state/runs.json
# (m23 salt 0 = 9,673,403 there — this rollout reproduces it and salt 1
# exactly, so cross-session trajectory identity holds).
SALT0_BASELINE = {
    "stress": 249_480_478,
    "m22_white": 14_156_269,
    "m23_white": 9_673_403,
    "m20_white": None,  # censored at the 1 B pilot cap; salt-0 rerun at 2 B
    "dec13": 3_822_602,
    "dec10": 4_262_128,
}
# m20 salt 0 was censored at 1 B in the pilot; the rollout cap is 2 B, so the
# identity expectation for m20 salt 0 is "censored again" only if it passes
# 1 B — recorded as a special check: either censored, or (2 B now allows a
# solve) a decisive outcome, but never a *decisive outcome different from*
# the fixture expectation.

NAME = re.compile(r"^(?P<case>m20_white|stress|m22_white|m23_white|dec\d\d)"
                  r"_s(?P<salt>\d+)$")
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


def parse_run(case, salt):
    name = f"{case}_s{salt}"
    with open(os.path.join(RAW, name + ".out")) as fo:
        stdout = fo.read()
    with open(os.path.join(RAW, name + ".err")) as fe:
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
        runs.append(parse_run(m["case"], int(m["salt"])))
    if not runs:
        raise SystemExit("no runs found")

    failures = []

    # D5(a): salt-0 identity on the pilot cases with a known uncensored count.
    for case, want in SALT0_BASELINE.items():
        if want is None:
            continue
        r0 = next(r for r in runs if r["case"] == case and r["salt"] == 0)
        if r0["evals"] != want or r0["censored"]:
            failures.append(
                {"check": "D5a_salt0_identity", "case": case,
                 "expected_evals": want, "got": r0["evals"],
                 "censored": r0["censored"]})

    summary = {}
    for case in {r["case"] for r in runs}:
        cruns = sorted((r for r in runs if r["case"] == case),
                       key=lambda r: r["salt"])
        # D5(b): conflicts among decisive outcomes.
        decisive_outcomes = {r["outcome"] for r in cruns if r["decisive"]}
        if len(decisive_outcomes) > 1:
            failures.append(
                {"check": "D5b_outcome_flip", "case": case,
                 "outcomes": sorted(decisive_outcomes)})
        # D5(c): fixture expectation on every uncensored run.
        for r in cruns:
            if r["decisive"] and r["outcome"] != EXPECTED[case]:
                failures.append(
                    {"check": "D5c_fixture_mismatch", "case": case,
                     "salt": r["salt"], "expected": EXPECTED[case],
                     "got": r["outcome"]})
        uncensored = [r["evals"] for r in cruns if not r["censored"]]
        censored = [r for r in cruns if r["censored"]]
        spread = (max(uncensored) / min(uncensored)) if len(uncensored) >= 2 else None
        # D8(1) loud/quiet: spread >= 1.5x among uncensored, or mixed censoring.
        loud = bool((spread is not None and spread >= 1.5) or (censored and uncensored))
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
            "loud": loud,
        }

    # D6 salt-correlation probe (uncensored pairs, eval-sensitive cases only).
    CORRELATION_REL = 1e-4
    SENSITIVE_REL = 1e-3
    eval_by = {(r["case"], r["salt"]): r for r in runs}
    # per-case salt->evals map + sensitivity for the record
    for case, s in summary.items():
        s["per_salt"] = {
            str(r["salt"]): {"evals": r["evals"], "censored": r["censored"]}
            for r in runs if r["case"] == case
        }
        vals = [r["evals"] for r in runs
                if r["case"] == case and not r["censored"]]
        s["max_pair_rel_diff"] = (
            max(abs(a - b) / min(a, b) for a in vals for b in vals)
            if len(vals) >= 2 else None)
        s["eval_sensitive"] = bool(
            s["max_pair_rel_diff"] is not None
            and s["max_pair_rel_diff"] >= SENSITIVE_REL)
    near = []
    salts = sorted({r["salt"] for r in runs})
    for i, sa in enumerate(salts):
        for sb in salts[i + 1:]:
            hits = []
            for case, s in summary.items():
                if not s["eval_sensitive"]:
                    continue
                ra = eval_by[(case, sa)]
                rb = eval_by[(case, sb)]
                if ra["censored"] or rb["censored"]:
                    continue
                rel = abs(ra["evals"] - rb["evals"]) / min(ra["evals"], rb["evals"])
                if rel < CORRELATION_REL:
                    hits.append({"case": case, "rel_diff": rel})
            if len(hits) >= 2:
                near.append({"salt_pair": [sa, sb], "cases": hits})

    with open(os.path.join(OUT, "runs.json"), "w") as f:
        json.dump({"runs": runs}, f, indent=1)
    with open(os.path.join(OUT, "summary.json"), "w") as f:
        json.dump({"summary": summary, "correlated_salt_pairs": near,
                   "failures": failures}, f, indent=1)

    for case, s in summary.items():
        print(case, "decisive:", s["decisive_outcomes"],
              "uncensored:", s["uncensored_evals"],
              "censored:", [(c["salt"], c["by"]) for c in s["censored"]],
              "spread:", None if s["spread_max_over_min"] is None
              else round(s["spread_max_over_min"], 3),
              "loud:", s["loud"])
    sens = [c for c, s in summary.items() if s["eval_sensitive"]]
    print("eval-sensitive cases (D6, >=0.1% max pairwise rel diff):", sens)
    print("correlated salt pairs (D6, >=2 sensitive cases):", near or "none")
    if failures:
        print("HARD FAILURES:", json.dumps(failures, indent=1))
        sys.exit(1)
    print("OK: D5(a)/(b)/(c) pass; no correlated salt pair fires the PIVOT")


if __name__ == "__main__":
    main()
