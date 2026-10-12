#!/usr/bin/env python3
"""plan10 stage-4 analysis: A1 (cooperative facts) vs A0 (portfolio) over the
pre-registered campaign rounds, with H1-H4 evaluated mechanically.

Inputs: state/<case>_{A0,A1}_r<round>.json race records (races10.py output),
optional solo salt-0 anchors in state/solo_s0.json (export-inertness /
pure-racer fidelity references).

Outputs: state/races10.json (aggregate), stdout summary.
"""

import json
import os
import statistics
import sys

sys.dont_write_bytecode = True

HERE = os.path.dirname(os.path.abspath(__file__))
STATE = os.path.join(HERE, "state")

# Pre-registered table (plan10.md): rounds, role, plan8/plan12 basin count
# and basin coverage of the racer set, plus the plan8 A0 winner references.
TABLE = {
    "m20_white": {
        "rounds": 5,
        "role": "hard class, 4 basins - primary H1 candidate",
        "basins": 4,
        "racer_basin_coverage": [0, 1, 2, 3],
    },
    "stress": {
        "rounds": 5,
        "role": "hard class, 3 basins - H1 candidate + canary",
        "basins": 3,
        "racer_basin_coverage": [0, 2, 5],
    },
    "m22_white": {
        "rounds": 5,
        "role": "hard class, 6 basins - collapse control + H1 candidate",
        "basins": 6,
        "racer_basin_coverage": [0, 2, 3, 5],
    },
    "dec10": {
        "rounds": 5,
        "role": "multi-basin regression channel",
        "basins": 4,
        "racer_basin_coverage": [0, 1, 2, 3],
    },
    "rem12": {
        "rounds": 5,
        "role": "out of sample (6 basins)",
        "basins": 6,
        "racer_basin_coverage": [0, 1, 2, 3, 4, 5],
    },
    "dec13": {
        "rounds": 3,
        "role": "veto-only (2 basins)",
        "basins": 2,
        "racer_basin_coverage": [0, 1, 1, 1],
    },
}
HARD_CLASS = ["m20_white", "stress", "m22_white"]

# plan12/plan8 solo evals per salt (diagnostic reference only, never a verdict
# input: cooperative racers have no anchor by design). Solo reference draws.
SOLO_REFERENCE = {
    "m20_white": {0: 2000000027, 2: 1384231894, 3: 861809196, 5: 791401553},
    "stress": {0: 249480478, 2: 2500000002, 3: 2500000001, 5: 252069909},
    "m22_white": {0: 14156269, 2: 15815489, 3: 17832300, 5: 21236889},
    "dec10": {0: 4262128, 2: 2918636, 3: 4802246, 5: 3901858},
    "rem12": {0: 69605385, 2: 65124029, 3: 59573726, 5: 63082193},
    "dec13": {0: 3822602, 2: 5038383, 3: 5032382, 5: 5032308},
}


def load_race(case, arm, rnd):
    path = os.path.join(STATE, f"{case}_{arm}_r{rnd}.json")
    if not os.path.exists(path):
        return None
    with open(path) as fh:
        return json.load(fh)


def first_decisive(rec):
    """The plan8/plan9 `evals_first`/`W_first` pair: the first decisive
    racer's evals + wall."""
    winner = rec.get("winner_salt")
    if winner is None:
        return None
    r = next(x for x in rec["racers"] if x["salt"] == winner)
    return {"salt": winner, "evals": r["evals"], "wall_ms": r["wall_ms"],
            "outcome": r["outcome"], "censored": r["censored"]}


def core_seconds(rec):
    return sum(r["wall_ms"] for r in rec["racers"]) / 1000.0


def outcome_conflict(rec):
    decisive = {r["outcome"] for r in rec["racers"] if r["outcome"] in ("win", "loss")}
    return len(decisive) > 1


def per_case(case):
    rounds = TABLE[case]["rounds"]
    out = {"rounds_expected": rounds, "A0": [], "A1": []}
    for rnd in range(1, rounds + 1):
        for arm in ("A0", "A1"):
            rec = load_race(case, arm, rnd)
            if rec is None:
                out[arm].append(None)
                continue
            fd = first_decisive(rec)
            out[arm].append(
                {
                    "round": rnd,
                    "first": fd,
                    "cpu_core_s": core_seconds(rec),
                    "conflict": outcome_conflict(rec),
                    "racers": [
                        {k: r[k] for k in
                         ("salt", "outcome", "censored", "killed", "evals",
                          "wall_ms", "decisive_at_ms", "facts_import",
                          "facts_export")}
                        for r in rec["racers"]
                    ],
                }
            )
    return out


def medians(races):
    firsts = [r["first"] for r in races if r and r["first"]]
    evals = [f["evals"] for f in firsts if f["evals"] is not None]
    walls = [f["wall_ms"] / 1000.0 for f in firsts if f["wall_ms"] is not None]
    censored = [f for f in firsts if f["censored"]]
    return {
        "evals_first": evals,
        "w_first": walls,
        "evals_median": statistics.median(evals) if evals else None,
        "w_median": statistics.median(walls) if walls else None,
        "n_decisive": len(firsts),
        "n_censored_first": len(censored),
        "winners": [f["salt"] for f in firsts],
    }


def main():
    solo_path = os.path.join(STATE, "solo_s0.json")
    solo = json.load(open(solo_path)) if os.path.exists(solo_path) else {}

    result = {"cases": {}, "canary": {}, "hypotheses": {}}
    conflicts = []
    h2_failures = []
    h1_hits = []

    for case in TABLE:
        data = per_case(case)
        a0, a1 = medians(data["A0"]), medians(data["A1"])
        entry = {
            "role": TABLE[case]["role"],
            "basins": TABLE[case]["basins"],
            "A0": a0,
            "A1": a1,
            "rounds": {arm: [
                {k: r[k] for k in ("round", "first", "cpu_core_s", "conflict")}
                if r else None
                for r in data[arm]
            ] for arm in ("A0", "A1")},
        }
        # Per-racer detail vs solo references (diagnostic).
        entry["racer_detail"] = []
        for arm in ("A0", "A1"):
            for r in data[arm]:
                if r is None:
                    continue
                for racer in r["racers"]:
                    ref = SOLO_REFERENCE[case].get(racer["salt"])
                    entry["racer_detail"].append({
                        "arm": arm, "round": r["round"],
                        "salt": racer["salt"],
                        "evals": racer["evals"],
                        "outcome": racer["outcome"],
                        "censored": racer["censored"],
                        "solo_reference": ref,
                        "evals_vs_solo": (
                            racer["evals"] / ref
                            if (racer["evals"] and ref and ref < 10**12)
                            else None
                        ),
                    })
        # Facts-channel accounting across A1 rounds.
        imp = [racer["facts_import"] for r in data["A1"] if r
               for racer in r["racers"] if racer["facts_import"]]
        exp = [racer["facts_export"] for r in data["A1"] if r
               for racer in r["racers"] if racer["facts_export"]]
        entry["facts"] = {
            "imports": imp,
            "exports": exp,
            "crc_fail_total": sum(i.get("crc_fail", 0) for i in imp),
            "accepted_total": sum(i.get("accepted", 0) for i in imp),
            "read_total": sum(i.get("read", 0) for i in imp),
        }
        # Pure-R0 fidelity: evals across A1 rounds (and vs A0 rounds + solo
        # anchor when decisive in both).
        r0_a1 = [rr["evals"] for r in data["A1"] if r
                 for rr in r["racers"] if rr["salt"] == 0 and rr["evals"]]
        r0_a0 = [rr["evals"] for r in data["A0"] if r
                 for rr in r["racers"] if rr["salt"] == 0 and rr["evals"]]
        entry["pure_r0"] = {
            "evals_A1_rounds": r0_a1,
            "evals_A0_rounds": r0_a0,
            "solo_anchor": solo.get(case),
            "constant_A1": len(set(r0_a1)) <= 1,
            "matches_A0": set(r0_a1) <= set(r0_a0) if r0_a0 else None,
        }
        # CPU cost.
        entry["cpu_core_s"] = {
            "A0": [r["cpu_core_s"] for r in data["A0"] if r],
            "A1": [r["cpu_core_s"] for r in data["A1"] if r],
        }
        if a0["evals_median"] and a1["evals_median"]:
            entry["ratio_evals"] = a1["evals_median"] / a0["evals_median"]
        if a0["w_median"] and a1["w_median"]:
            entry["ratio_w"] = a1["w_median"] / a0["w_median"]
        result["cases"][case] = entry

        for r in data["A0"] + data["A1"]:
            if r and r["conflict"]:
                conflicts.append((case, r["round"], r["first"]))

        # H2 (no regression): both medians <= 1.1x.
        if a0["evals_median"] and a1["evals_median"]:
            ok = (
                a1["evals_median"] <= 1.1 * a0["evals_median"]
                and (a0["w_median"] is None or a1["w_median"] is None
                     or a1["w_median"] <= 1.1 * a0["w_median"])
            )
            if not ok:
                h2_failures.append(case)
        # H1 candidate check (hard class only).
        if case in HARD_CLASS and a0["evals_median"] and a1["evals_median"]:
            if (a1["evals_median"] <= 0.9 * a0["evals_median"]
                    and a0["w_median"] and a1["w_median"]
                    and a1["w_median"] <= 0.9 * a0["w_median"]):
                h1_hits.append(case)

    # Canary races (stage 2).
    for case in ("m22_white", "stress"):
        base = load_race(case, "A0", None) or None
        # canary races carry the tag; races10 named them <case>_A0canarybase_r1
        p0 = os.path.join(STATE, f"{case}_A0canarybase_r1.json")
        p1 = os.path.join(STATE, f"{case}_A1canary_r1.json")
        canary = {"present": os.path.exists(p1)}
        if canary["present"]:
            r0 = json.load(open(p0)) if os.path.exists(p0) else None
            r1 = json.load(open(p1))
            f0 = first_decisive(r0) if r0 else None
            f1 = first_decisive(r1)
            canary.update(
                {
                    "A0_first": f0,
                    "A1_first": f1,
                    "conflict": outcome_conflict(r1) or outcome_conflict(r0),
                    "w_ratio": (
                        f1["wall_ms"] / f0["wall_ms"]
                        if f0 and f1 and f0["wall_ms"] and f1["wall_ms"]
                        else None
                    ),
                    "panic_or_abnormal_rc": any(
                        r["rc"] not in (0, -9) for r in r1["racers"]
                    ),
                    "within_wall_caps": all(
                        r["wall_ms"] <= r1["wall_cap_s"] * 1000 + 5000
                        for r in r1["racers"]
                    ),
                }
            )
        result["canary"][case] = canary

    canary_clean = all(
        c.get("present")
        and not c.get("conflict")
        and not c.get("panic_or_abnormal_rc")
        and c.get("within_wall_caps")
        and c.get("w_ratio") is not None and c["w_ratio"] <= 2.0
        for c in result["canary"].values()
    )
    result["hypotheses"] = {
        "H1_value": h1_hits,
        "H2_no_regression_failures": h2_failures,
        "H3_destabilization": {
            "conflicts": conflicts,
            "canary_clean": canary_clean,
        },
        "H4_fidelity": {
            "pure_r0": {c: result["cases"][c]["pure_r0"] for c in TABLE},
            "export_inertness": "stage-1 paired runs: stdout byte-identical, evals identical (see README)",
            "codec": "unit tests (facts::tests) in the fast gate",
        },
        "verdict": None,
    }
    h2_ok = not h2_failures
    h3_ok = not conflicts and canary_clean
    # H4a: pure R0 constant across A1 rounds wherever measured.
    h4a_ok = all(
        result["cases"][c]["pure_r0"]["constant_A1"]
        and (result["cases"][c]["pure_r0"]["matches_A0"] is not False)
        for c in TABLE
    )
    h4_ok = h4a_ok
    if not (h2_ok and h3_ok and h4_ok):
        result["hypotheses"]["verdict"] = "reject"
    elif h1_hits:
        result["hypotheses"]["verdict"] = "adopt"
    else:
        result["hypotheses"]["verdict"] = "defer"

    with open(os.path.join(STATE, "races10.json"), "w") as fh:
        json.dump(result, fh, indent=1)

    # Console summary.
    print("case          | ratio_evals | ratio_w | A0 win | A1 win | verdict-input")
    for case, e in result["cases"].items():
        print(
            f"{case:13} | "
            f"{e.get('ratio_evals', float('nan')):>10.3f} | "
            f"{e.get('ratio_w', float('nan')):>6.3f} | "
            f"{str(e['A0']['evals_median']):>10} | "
            f"{str(e['A1']['evals_median']):>10} | "
            f"{e['role']}"
        )
    print("H1:", h1_hits, " H2 failures:", h2_failures,
          " conflicts:", conflicts, " canary_clean:", canary_clean)
    print("VERDICT:", result["hypotheses"]["verdict"])


if __name__ == "__main__":
    main()
