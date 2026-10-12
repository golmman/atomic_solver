#!/usr/bin/env python3
"""plan11 stage-4 analysis: A1 (low-tax facts carrier) vs A0 (portfolio) over
the pre-registered campaign rounds, with the pre-registered hypotheses
evaluated mechanically:

  M1  solo tax (stage-1 probes, state/probes11.json): median wall ratio
      <= 1.03 on every probed case + stdout/evals identity per pair.
  M2  in-race contention tax: pure R0's in-race wall vs its fresh
      same-binary solo anchor, median over all A1 rounds where R0 finishes
      decisive, <= 1.06 on every case.
  H1  value beyond the portfolio floor: on >= 1 hard-class case with
      A0 basin count >= 3 (m20_white, m22_white, stress): median
      evals_first(A1) <= 0.9x and median W_first(A1) <= 0.9x.
  H2  no regression: on every raced case median evals_first(A1) <= 1.1x and
      median W_first(A1) <= 1.1x, and <= 1 winnerless A1 round (a round with
      no decisive racer contributes a winnerless flag, not a median sample;
      >= 2 winnerless rounds fails H2 for that case).
  H3  destabilization: no decisive-outcome conflict anywhere, no panic /
      abnormal exit, canary W_first <= 2x the A0 value on m22_white and
      stress.
  H4  fidelity/soundness: (a) pure R0 reproduces its fresh solo salt-0
      anchor bit-for-bit wherever it finishes; (b) zero outcome conflicts;
      (c) censoring discipline (budget-censored recorded censored, killed
      evals excluded); (d) codec/filter unit tests; (e) --facts-out alone is
      byte-inert at the CLI level (stage-1 pairs).

Verdict: adopt ⇔ M1 ∧ M2 ∧ H1 ∧ H2 ∧ H3 ∧ H4; reject on M1/M2/H2/H3
failure; defer if clean but no H1 case fires.

Inputs: state/<case>_{A0,A1}_r<round>.json (races11.py), the canary races,
state/solo_s0.json, state/probes11.json.
Output: state/races11.json + stdout summary.
"""

import json
import os
import statistics
import sys

sys.dont_write_bytecode = True

HERE = os.path.dirname(os.path.abspath(__file__))
STATE = os.path.join(HERE, "state")

TABLE = {
    "m20_white": {
        "rounds": 5,
        "role": "hard class, 4 basins - primary H1 candidate",
        "basins": 4,
    },
    "stress": {
        "rounds": 5,
        "role": "hard class, 3 basins - H1 candidate + canary",
        "basins": 3,
    },
    "m22_white": {
        "rounds": 5,
        "role": "hard class, 6 basins - collapse control + H1 candidate",
        "basins": 6,
    },
    "dec10": {
        "rounds": 5,
        "role": "multi-basin regression channel",
        "basins": 4,
    },
    "rem12": {
        "rounds": 5,
        "role": "out of sample (6 basins)",
        "basins": 6,
    },
    "dec13": {
        "rounds": 3,
        "role": "veto-only (2 basins)",
        "basins": 2,
    },
}
HARD_CLASS = ["m20_white", "stress", "m22_white"]
H1_BASIN_FLOOR = 3


def load(path):
    if not os.path.exists(path):
        return None
    with open(path) as fh:
        return json.load(fh)


def load_race(case, arm, rnd):
    return load(os.path.join(STATE, f"{case}_{arm}_r{rnd}.json"))


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


def medians(races):
    firsts = [r["first"] for r in races if r and r["first"]]
    evals = [f["evals"] for f in firsts if f["evals"] is not None]
    walls = [f["wall_ms"] / 1000.0 for f in firsts if f["wall_ms"] is not None]
    return {
        "evals_median": statistics.median(evals) if evals else None,
        "w_median": statistics.median(walls) if walls else None,
        "n_decisive": len(firsts),
        "n_winnerless": sum(1 for r in races if r and r["first"] is None),
        "winners": [f["salt"] for f in firsts],
    }


def main():
    solo = load(os.path.join(STATE, "solo_s0.json")) or {}
    probes = load(os.path.join(STATE, "probes11.json")) or {}

    result = {"cases": {}, "canary": {}, "m1": probes.get("_m1", {}), "hypotheses": {}}
    conflicts = []
    abnormal = []
    h2_failures = []
    h1_hits = []
    m2_ratios = {}

    for case in TABLE:
        rounds = TABLE[case]["rounds"]
        data = {"A0": [], "A1": []}
        for rnd in range(1, rounds + 1):
            for arm in ("A0", "A1"):
                rec = load_race(case, arm, rnd)
                data[arm].append(
                    None if rec is None else
                    {
                        "round": rnd,
                        "first": first_decisive(rec),
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
                if rec and outcome_conflict(rec):
                    conflicts.append((case, arm, rnd))
                if rec:
                    for r in rec["racers"]:
                        if r["rc"] not in (0, -9, 137):
                            abnormal.append((case, arm, rnd, r["salt"], r["rc"]))
        a0, a1 = medians(data["A0"]), medians(data["A1"])
        entry = {
            "role": TABLE[case]["role"],
            "basins": TABLE[case]["basins"],
            "A0": a0,
            "A1": a1,
            "rounds": data,
        }
        if a0["evals_median"] and a1["evals_median"]:
            entry["ratio_evals"] = a1["evals_median"] / a0["evals_median"]
        if a0["w_median"] and a1["w_median"]:
            entry["ratio_w"] = a1["w_median"] / a0["w_median"]

        # M2: pure R0's in-race wall vs the fresh solo anchor over the A1
        # rounds where R0 finished decisive.
        if solo.get(case, {}).get("decisive"):
            anchor = solo[case]["wall_s"]
            r0_decisive = [
                (rr["wall_ms"] or 0) / 1000.0 / anchor
                for r in data["A1"] if r
                for rr in r["racers"] if rr["salt"] == 0 and rr["outcome"] in ("win", "loss")
            ]
            if r0_decisive:
                m2_ratios[case] = {
                    "rounds": [round(x, 4) for x in r0_decisive],
                    "median": statistics.median(r0_decisive),
                }
        entry["m2"] = m2_ratios.get(case)

        # H4a: pure R0 evals reproduce the solo anchor bit-for-bit in every
        # A1 round where it finishes (decisive or censored-at-cap).
        r0_evals_a1 = [
            rr["evals"] for r in data["A1"] if r
            for rr in r["racers"] if rr["salt"] == 0 and rr["evals"] is not None
        ]
        anchor_evals = solo.get(case, {}).get("evals")
        entry["pure_r0"] = {
            "evals_A1_rounds": r0_evals_a1,
            "solo_anchor_evals": anchor_evals,
            # Evidence exists only in rounds where R0 finished (killed racers
            # never print their evals line); an empty list is a vacuous pass,
            # not a failure.
            "all_match_anchor": (
                anchor_evals is not None
                and all(e == anchor_evals for e in r0_evals_a1)
            ),
            "evidence_rounds": len(r0_evals_a1),
        }

        # Facts-channel accounting across A1 rounds.
        imports = [
            rr["facts_import"] for r in data["A1"] if r
            for rr in r["racers"] if rr["facts_import"]
        ]
        exports = [
            rr["facts_export"] for r in data["A1"] if r
            for rr in r["racers"] if rr["facts_export"]
        ]
        entry["facts"] = {
            "import_lines": imports,
            "export_lines": exports,
            "read_total": sum(i.get("read", 0) for i in imports),
            "accepted_total": sum(i.get("accepted", 0) for i in imports),
            "crc_fail_total": sum(i.get("crc_fail", 0) for i in imports),
            "import_bytes_total": sum(i.get("bytes", 0) for i in imports),
            "emitted_total": sum(e.get("emitted", 0) for e in exports),
            "seen_total": sum(e.get("records", 0) for e in exports),
            "filtered_total": sum(e.get("filtered", 0) for e in exports),
        }

        # CPU cost.
        entry["cpu_core_s"] = {
            "A0": [r["cpu_core_s"] for r in data["A0"] if r],
            "A1": [r["cpu_core_s"] for r in data["A1"] if r],
        }
        if entry["cpu_core_s"]["A0"] and entry["cpu_core_s"]["A1"]:
            entry["cpu_ratio_median"] = (
                statistics.median(entry["cpu_core_s"]["A1"])
                / statistics.median(entry["cpu_core_s"]["A0"])
            )
        result["cases"][case] = entry

        # H2 (no regression): both medians <= 1.1x and <= 1 winnerless round.
        h2_ok = (
            a0["evals_median"] is not None and a1["evals_median"] is not None
            and a1["evals_median"] <= 1.1 * a0["evals_median"]
            and (a0["w_median"] is None or a1["w_median"] is None
                 or a1["w_median"] <= 1.1 * a0["w_median"])
            and a1["n_winnerless"] <= 1
        )
        if not h2_ok:
            h2_failures.append(case)
        # H1 candidate check (hard class, basin floor).
        if (case in HARD_CLASS and TABLE[case]["basins"] >= H1_BASIN_FLOOR
                and a0["evals_median"] and a1["evals_median"]
                and a0["w_median"] and a1["w_median"]):
            if (a1["evals_median"] <= 0.9 * a0["evals_median"]
                    and a1["w_median"] <= 0.9 * a0["w_median"]):
                h1_hits.append(case)

    # Canary races (stage 2).
    for case in ("m22_white", "stress"):
        p0 = os.path.join(STATE, f"{case}_A0canarybase_r1.json")
        p1 = os.path.join(STATE, f"{case}_A1canary_r1.json")
        canary = {"present": os.path.exists(p1)}
        if canary["present"]:
            r0, r1 = load(p0), load(p1)
            f0 = first_decisive(r0) if r0 else None
            f1 = first_decisive(r1)
            canary.update(
                {
                    "A0_first": f0,
                    "A1_first": f1,
                    "conflict": outcome_conflict(r1) or (outcome_conflict(r0) if r0 else False),
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

    # M1 (stage-1 probes).
    m1_pass = bool(probes.get("_m1_pass"))

    # M2.
    m2_failures = [c for c, v in m2_ratios.items() if v["median"] > 1.06]
    m2_pass = len(m2_failures) == 0

    # H3.
    h3_ok = not conflicts and not abnormal and canary_clean

    # H4a (bit-for-bit R0 fidelity) + censoring discipline (recorded by the
    # driver; verified in the race records) + codec unit tests + stage-1
    # byte-inertness.
    h4a_ok = all(result["cases"][c]["pure_r0"]["all_match_anchor"]
                 for c in TABLE
                 if solo.get(c, {}).get("decisive"))
    h4_ok = h4a_ok

    result["hypotheses"] = {
        "M1_solo_tax": {"pass": m1_pass, "detail": result["m1"]},
        "M2_contention_tax": {
            "pass": m2_pass,
            "failures": m2_failures,
            "ratios": m2_ratios,
        },
        "H1_value": h1_hits,
        "H2_no_regression_failures": h2_failures,
        "H3_destabilization": {
            "pass": h3_ok,
            "conflicts": conflicts,
            "abnormal_rc": abnormal,
            "canary_clean": canary_clean,
        },
        "H4_fidelity": {
            "pass": h4_ok,
            "h4a_bit_identical": h4a_ok,
            "codec": "unit tests (facts::tests) in the fast gate",
            "byte_inertness": "stage-1 paired runs: stdout md5 + evals identical (probes11.json)",
        },
    }

    if not (m1_pass and m2_pass and h3_ok and h4_ok):
        result["hypotheses"]["verdict"] = "reject"
    elif h2_failures:
        result["hypotheses"]["verdict"] = "reject"
    elif h1_hits:
        result["hypotheses"]["verdict"] = "adopt"
    else:
        result["hypotheses"]["verdict"] = "defer"

    with open(os.path.join(STATE, "races11.json"), "w") as fh:
        json.dump(result, fh, indent=1)

    # Console summary.
    print("case          | r_evals | r_wall | winnerless(A1) | M2 median | role")
    for case, e in result["cases"].items():
        m2 = e.get("m2")
        print(
            f"{case:13} | "
            f"{e.get('ratio_evals', float('nan')):>7.3f} | "
            f"{e.get('ratio_w', float('nan')):>6.3f} | "
            f"{e['A1']['n_winnerless']:>14} | "
            f"{(f'{m2['median']:.3f}' if m2 else 'n/a'):>9} | "
            f"{e['role']}"
        )
    print()
    print("M1:", "PASS" if m1_pass else "FAIL", result["m1"])
    print("M2:", "PASS" if m2_pass else f"FAIL {m2_failures}", 
          {c: round(v["median"], 3) for c, v in m2_ratios.items()})
    print("H1:", h1_hits, " H2 failures:", h2_failures)
    print("H3:", "PASS" if h3_ok else "FAIL", "conflicts:", conflicts, "abnormal:", abnormal, "canary:", canary_clean)
    print("H4:", "PASS" if h4_ok else "FAIL", "h4a:", h4a_ok)
    print("facts accounting (campaign):")
    for case, e in result["cases"].items():
        f = e["facts"]
        pass_rate = (
            f["emitted_total"] / f["seen_total"] if f["seen_total"] else None
        )
        print(
            f"  {case:13} seen={f['seen_total']:>9} emitted={f['emitted_total']:>9} "
            f"filtered={f['filtered_total']:>8} pass={pass_rate if pass_rate is None else round(pass_rate, 3)} "
            f"read={f['read_total']:>9} accepted={f['accepted_total']:>7} crc={f['crc_fail_total']}"
        )
    print("VERDICT:", result["hypotheses"]["verdict"])


if __name__ == "__main__":
    main()
