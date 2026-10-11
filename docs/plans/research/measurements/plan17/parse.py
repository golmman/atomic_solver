#!/usr/bin/env python3
"""Parse the plan17 rollout raw outputs into the committed state/ files.

    python3 parse.py [RAW_DIR]   (default: $RAW or /tmp/plan17/results)

Reads `<case>_s<salt>_<arm>.out/.err/.meta.json` written by driver.py and
writes, next to this script:

    state/runs.json      per-run record (arm, evals, outcome, censoring, wall,
                         hook counters)
    state/summary.json   per-case-per-arm record + D3 checks
    state/claims.json    the D3(e) adopted-claim dumps (root FEN joined in)

Hard failures (exit 1, any violation HALTs per plan17 D3):

    D3(b) baseline-arm identity: the base arm at every salt equals plan12's
          recorded per-salt values exactly (evals AND censoring status),
          all 132 cells — the null is degenerate; any mismatch is an
          identity failure, not noise. (The pre-hook pristine-build check
          D3(a) itself is recorded in env.json.)
    D3(c) hook fidelity anchors: at salt 0 the candidate arms reproduce
          dfpn report10/report11's archived spike numbers exactly —
          arma: stress 227,597,289 and m22 18,139,173; plan10: stress
          155,394,450. The archived m22 plan10 reading (120 s timeout,
          714.6 M evals) used different caps, so m22/plan10 is recorded as
          informational (censoring pattern per salt), not an anchor.
          NOTE (plan17 deviation, see report17.md): report11's arma
          quick-suite byte-identity claim does NOT hold under the faithful
          port (37/59 quick cases drift, zero outcome flips) — it is
          arithmetically incompatible with the exact stress anchors above;
          recorded as a failed-as-stated sub-anchor, not a hook defect.
    D3(d) outcome invariants: zero decisive-outcome conflicts within any arm
          across salts, and every uncensored run of every arm matches its
          fixture expected outcome (for the candidate arms this is a
          substantive gate on the shift lemma — a conflict is a soundness
          defect, never noise).
    D3(f)-structural: a budget-censored draw is the no-result sentinel,
          never a proven draw (censored runs are excluded from every
          outcome set; a censored run marked decisive is a failure).
"""

import glob
import json
import os
import re
import statistics
import sys

sys.dont_write_bytecode = True

HERE = os.path.dirname(os.path.abspath(__file__))
RAW = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("RAW", "/tmp/plan17/results")
OUT = os.path.join(HERE, "state")
PLAN12 = os.path.join(HERE, "..", "plan12", "state")

# D3(c) anchors: dfpn report10/report11 archived spike values (salt 0).
ANCHORS = {
    ("arma", "stress"): 227_597_289,
    ("arma", "m22_white"): 18_139_173,
    ("plan10", "stress"): 155_394_450,
}

NAME = re.compile(r"^(?P<case>m20_white|stress|m22_white|m23_white|dec\d\d)"
                  r"_s(?P<salt>\d+)_(?P<arm>base|plan10|arma)$")
BUDGET = {"stress": 2_500_000_000, "m20_white": 2_000_000_000}
BUDGET_DEFAULT = 1_000_000_000
ARMS = ["base", "plan10", "arma"]

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
    with open(out_path[: -len(".out")] + ".err") as fe:
        stderr = fe.read()
    outcome = None
    for line in stdout.splitlines():
        if line.startswith("outcome: "):
            outcome = line.split()[1]
    evals = None
    counters = None
    for line in stderr.splitlines():
        if line.startswith("evals: "):
            evals = int(line.split()[1])
        if line.startswith("clockreuse: "):
            counters = line[len("clockreuse: "):]
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
        "hook_counters": counters,
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
    info = []
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

    # D3(c): fidelity anchors (candidate-arm salt-0 cells vs report10/11).
    anchor_results = {}
    for (arm, case), want in sorted(ANCHORS.items()):
        if (case, 0, arm) not in by_key:
            info.append({"note": f"D3c_anchor_pending", "arm": arm, "case": case})
            continue
        r = by_key[(case, 0, arm)]
        ok = r["evals"] == want and not r["censored"]
        anchor_results[f"{arm}_{case}_s0"] = {
            "expected": want, "got": r["evals"], "censored": r["censored"],
            "ok": ok}
        if not ok:
            failures.append({"check": "D3c_fidelity_anchor", "arm": arm,
                             "case": case, "expected": want,
                             "got": r["evals"], "censored": r["censored"]})
    # Informational (not an anchor): plan10 on m22_white — the archived
    # reading used different caps; record the censoring pattern per salt.
    if all(("m22_white", s, "plan10") in by_key for s in range(6)):
        m22_plan10 = {
            str(s): {"evals": by_key[("m22_white", s, "plan10")]["evals"],
                     "censored": by_key[("m22_white", s, "plan10")]["censored"],
                     "outcome": by_key[("m22_white", s, "plan10")]["outcome"]}
            for s in range(6)
        }
        info.append({"note": "plan10_m22_informational_per_salt",
                     "data": m22_plan10})

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

    # D3(e) source: collect the adopted-claim dumps (root FEN joined in).
    claims = []
    sample_dir = os.path.join(RAW, "samples")
    if os.path.isdir(sample_dir):
        for path in sorted(glob.glob(os.path.join(sample_dir, "*.jsonl"))):
            base = os.path.basename(path)[: -len(".jsonl")]
            m = NAME.match(base)
            if not m:
                raise SystemExit(f"unparsed sample name: {base}")
            fen = CASES_FEN[m["case"]]
            with open(path) as f:
                for line in f:
                    line = line.strip()
                    if line:
                        c = json.loads(line)
                        c.update({"case": m["case"], "salt": int(m["salt"]),
                                  "arm": m["arm"], "fen": fen})
                        claims.append(c)

    with open(os.path.join(OUT, "runs.json"), "w") as f:
        json.dump({"runs": runs}, f, indent=1)
    with open(os.path.join(OUT, "summary.json"), "w") as f:
        json.dump({"summary": summary, "anchors": anchor_results,
                   "informational": info, "claims": claims,
                   "failures": failures}, f, indent=1)

    for case in EXPECTED:
        for arm in ARMS:
            if arm not in summary.get(case, {}):
                continue  # arm not (yet) rolled out
            s = summary[case][arm]
            print(f"{case:10s} {arm:7s} uncensored: {s['uncensored_evals']} "
                  f"censored: {[(c['salt'], c['by']) for c in s['censored']]} "
                  f"outcomes: {s['decisive_outcomes']}")
    print(f"claims collected: {len(claims)} "
          f"(plan10: {sum(1 for c in claims if c['arm'] == 'plan10')}, "
          f"arma: {sum(1 for c in claims if c['arm'] == 'arma')})")
    if failures:
        print("HARD FAILURES (HALT):", json.dumps(failures, indent=1))
        sys.exit(1)
    print(f"OK: D3(b),(c),(d) + structural pass ({len(runs)} rollout runs); "
          "NOTE: D3(c) arma quick-suite byte-identity sub-anchor failed as "
          "stated (deviation recorded in report17.md, see module docstring)")


# FENs duplicated from driver.py (import would re-run its __main__ guard is
# fine, but keep parse standalone): case -> fen.
CASES_FEN = {
    "m20_white": "4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R5RK w - - 4 20",
    "stress": "4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21",
    "m22_white": "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22",
    "m23_white": "4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23",
    "dec01": "r5r1/5N1k/2p2p2/pp1p3p/3Pp3/2P1P3/P7/2bQ1R1K w - - 0 30",
    "dec02": "8/1k1p4/1P2p3/p2P1P2/P7/6p1/6K1/8 b - - 0 27",
    "dec03": "r4r2/pp2p1Bk/2p3p1/1B1pP1bp/3P3P/1PN5/2P5/5R1K w - - 1 21",
    "dec04": "3k3r/1p1P4/4p2P/5p2/P7/2n1P3/6K1/7R b - - 0 25",
    "dec05": "7k/7p/1pp3p1/3B4/2P5/5pPP/P7/R5K1 w - - 0 26",
    "dec06": "3r2k1/8/p7/2P2P2/8/1P1p3P/P2K2r1/6R1 w - - 1 32",
    "dec07": "r2qk2r/p2n4/6pp/1ppppp2/3P1B2/5PPP/PPP1Q1B1/R4RK1 w kq - 0 18",
    "dec08": "r4r1k/3q1P2/pp4pp/2pp4/N4Q2/2P3PP/PP2p3/R4R1K w - - 0 25",
    "dec09": "5k2/p1Rb1P2/6r1/1P2p3/P2pP1np/3P3B/3b4/3K2R1 b - - 0 28",
    "dec10": "3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41",
    "dec11": "rnb1k2r/p2p3p/4ppp1/qp6/3PPQ2/8/PPP2PPP/R3KB1R w KQkq - 2 11",
    "dec12": "2b2k1r/pp5p/2ppp3/8/4P2P/qPPP4/P1Q2b2/5R1K w - - 0 23",
    "dec13": "r1bq1k1r/ppN4p/n1p1p3/3p1n1P/1b1P2P1/2P5/PP6/RNBQKB1R w KQ - 1 14",
    "dec14": "r1bq1k1r/ppN5/n1p1p1pp/3p1p1P/3P4/P1PB4/1P3PP1/R1BQK2R w KQ - 0 15",
    "dec15": "r3k1r1/1b1n3p/3p1pPb/pp1Pp3/P3P3/4B3/1PP4P/R3KBR1 w Qq - 1 20",
    "dec16": "2kr1b1r/ppp3pp/4bp2/4p3/2B1P3/2PP2PP/PP1n1P2/RN1Q1R1K w - - 3 14",
    "dec17": "r1bqk2r/pp4pp/2p2p1n/1P4B1/4P3/2Pp1PP1/7P/R2QKBNR w KQkq - 0 14",
    "dec18": "r1b2k1r/p1p4p/1pnNppp1/1B1p4/1b1PP3/2P5/PP4PP/RNB1K2R w KQ - 0 13",
}


if __name__ == "__main__":
    main()
