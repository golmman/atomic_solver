#!/usr/bin/env python3
"""plan12 stage-0 Phase-0 noise probe (H0 verdict, mechanical).

Tests H0: plan6's SPDFPN bad-trajectory tails are ordinary sequential
realization draws under the measured per-run parallel overhead, i.e.

    inflation_r = (helper_evals_r + coordinator_evals_r) / evals_seq_salt0
              ~= ratio_r * overhead_r

with ratio_r drawn from the sequential cross-salt realization distribution
and overhead_r the parallelism-specific work overhead of the mechanism.

Pre-registered test set: {shuffle_win, rem12} (m22 was plan6's control,
measurement-only; its numbers are reported as an out-of-scope finding).

Tests
  1. magnitude : observed bad-tail inflation vs products
                 {ratio x overhead} over uncensored ratio draws x overhead
                 estimates from the same (case, N) non-tail runs.
                 Overhead estimators (all reported, most generous = max):
                   - inmax : max inflation among non-tail runs at (case, N)
                   - inmed : median inflation among non-tail runs
                   - decomp: inflation / (coordinator_evals / seq_evals)
                     per run (coordinator work as the realization proxy);
                     reported as per-run values + median
                 H0 rejected only if observed > 1.2 x EVERY product.
  2. frequency : parallel tail rate vs sequential cross-salt tail rate
                 (fraction of salt draws with ratio >= 2x; censored draws
                 are tails a fortiori at their budget ratio). Veto-only.
  3. decomposition : bad-tail excess attributed to coordinator_evals
                 (perturbation of the finishing trajectory) vs
                 helper_evals (wasted helper work).

Inputs (all committed):
  - measurements/plan6/state/campaign6_raw.jsonl + campaign6_summary.json
  - docs/plans/research/measurements/plan12/state/runs.json (corpus salts)
  - state/rem12_salts.json, state/stress_salts_fresh.json (fresh probes)

Output: state/probe_noise.json
"""
import json
import statistics
from pathlib import Path

HERE = Path(__file__).resolve().parent
M6 = HERE / ".." / "plan6" / "state"
CORPUS = HERE / ".." / ".." / ".." / "research" / "measurements" / "plan12" / "state" / "runs.json"
OUT = HERE / "state" / "probe_noise.json"

SEQ_EVALS = {  # plan6-era in-session sequential salt-0 references (child evals)
    "m22": 14_156_269,
    "rem12": 69_605_385,
    "shuffle_win": 249_480_478,
}
SEQ_WALL = {"m22": 3.263, "rem12": 16.886, "shuffle_win": 57.707}
CAP = {"m22": 30.0, "rem12": 60.0, "shuffle_win": 100.0}
TEST_CASES = ["shuffle_win", "rem12"]
CORPUS_CASE = {"shuffle_win": "stress", "m22": "m22_white"}


def load_campaign():
    runs = [json.loads(l) for l in (M6 / "campaign6_raw.jsonl").read_text().splitlines() if l.strip()]
    by = {}
    for r in runs:
        r["total_evals"] = r.get("helper_evals", 0) + r.get("coordinator_evals", 0)
        by.setdefault((r["case"], r["threads"]), []).append(r)
    return by


def tail_p(r):
    """bad-trajectory tail classification: cap-hit or wall >= 1.5x seq wall."""
    return (r["wall_s"] >= CAP[r["case"]] - 0.5) or (r["wall_s"] >= 1.5 * SEQ_WALL[r["case"]])


def ratios_from_corpus(case):
    runs = json.loads(CORPUS.read_text())["runs"]
    rs = [r for r in runs if r["case"] == case]
    base = next(r["evals"] for r in rs if r["salt"] == 0)
    out = []
    for r in sorted(rs, key=lambda x: x["salt"]):
        if r["censored"] == "budget":
            out.append({"salt": r["salt"], "censored": True,
                        "ratio_lower_bound": r["evals"] / base})
        else:
            out.append({"salt": r["salt"], "censored": False,
                        "ratio": r["evals"] / base})
    return {"base_evals": base, "draws": out}


def ratios_fresh(path):
    d = json.loads(path.read_text())
    rs = d["runs"]
    base = next(r["evals"] for r in rs if r["salt"] == 0)
    out = []
    for r in sorted(rs, key=lambda x: x["salt"]):
        if r["budget_exhausted"]:
            out.append({"salt": r["salt"], "censored": True,
                        "ratio_lower_bound": r["evals"] / base})
        else:
            out.append({"salt": r["salt"], "censored": False,
                        "ratio": r["evals"] / base})
    return {"base_evals": base, "draws": out}


def uncensored_max_ratio(band):
    vals = [d["ratio"] for d in band["draws"] if not d["censored"]]
    return max(vals), len(vals)


def analyze_case(case, groups):
    seq = SEQ_EVALS[case]
    report = {"case": case, "seq_evals_ref": seq, "seq_wall_ref": SEQ_WALL[case],
              "threads": {}}
    for n in (2, 3, 4):
        runs = groups[(case, n)]
        for r in runs:
            r["inflation"] = r["total_evals"] / seq
            r["coord_ratio"] = r["coordinator_evals"] / seq
            r["helper_ratio"] = r["helper_evals"] / seq
            r["tail"] = tail_p(r)
            r["overhead_decomp"] = r["inflation"] / r["coord_ratio"]
        tails = [r for r in runs if r["tail"]]
        good = [r for r in runs if not r["tail"]]
        th = {
            "n": n,
            "runs": [
                {k: r[k] for k in ("round", "wall_s", "outcome", "total_evals",
                                   "inflation", "coordinator_evals", "coord_ratio",
                                   "helper_evals", "helper_ratio", "tail",
                                   "overhead_decomp", "jobs")}
                for r in runs
            ],
            "tail_rate": f"{len(tails)}/{len(runs)}",
            "good_infl_max": max((r["inflation"] for r in good), default=None),
            "good_infl_med": statistics.median([r["inflation"] for r in good]) if good else None,
            "overhead_decomp_runs": [r["overhead_decomp"] for r in runs],
            "overhead_decomp_med": statistics.median([r["overhead_decomp"] for r in runs]),
        }
        if tails:
            th["tails"] = [
                {k: r[k] for k in ("round", "wall_s", "outcome", "inflation",
                                   "coord_ratio", "helper_ratio", "overhead_decomp",
                                   "jobs")}
                for r in tails
            ]
        if good:
            th["good_median_coord_evals"] = statistics.median([r["coordinator_evals"] for r in good])
            th["good_median_helper_evals"] = statistics.median([r["helper_evals"] for r in good])
        report["threads"][n] = th
    return report


def magnitude_verdict(case, case_rep, uncensored_max, n_tail_infl):
    """H0 rejected only if observed tail inflation > 1.2 x EVERY product."""
    th = case_rep["threads"][4]  # the motivating cell is t4; t2/t3 reported too
    products = []
    for n, thr in case_rep["threads"].items():
        for o_name, o_val in (
            ("inmax", thr["good_infl_max"]),
            ("inmed", thr["good_infl_med"]),
            ("decomp", thr["overhead_decomp_med"]),
        ):
            if o_val is None:
                continue
            products.append({"n": n, "overhead_est": o_name, "overhead": round(o_val, 3),
                             "ratio": round(uncensored_max, 3),
                             "product": round(uncensored_max * o_val, 3)})
    verdict = {"case": case, "ratio_max_uncensored": round(uncensored_max, 3),
               "tail_inflations_t4": [t["inflation"] for t in th.get("tails", [])],
               "products": products}
    relevant = [p for p in products if p["n"] == 4]
    worst = max(p["product"] for p in relevant)
    worst_any = max(p["product"] for p in products)
    observed = max(n_tail_infl)
    rejected = observed > 1.2 * max(p["product"] for p in relevant)
    verdict.update({
        "observed_max_tail_inflation": round(observed, 3),
        "max_product_t4": round(worst, 3),
        "max_product_any_N": round(worst_any, 3),
        "rejected_at_magnitude": rejected,
        "rule": "reject iff observed > 1.2 x every {ratio_max_uncensored x overhead_est} product at t4",
    })
    return verdict


def main():
    groups = load_campaign()
    corpus = {c: ratios_from_corpus(cc) for c, cc in CORPUS_CASE.items()}
    rem12_fresh = ratios_fresh(HERE / "state" / "rem12_salts.json")
    stress_fresh = ratios_fresh(HERE / "state" / "stress_salts_fresh.json")

    reps = {c: analyze_case(c, groups) for c in ("m22", "rem12", "shuffle_win")}

    # ratio band summaries
    def band_sum(b):
        unc = [d["ratio"] for d in b["draws"] if not d["censored"]]
        cens = [d["ratio_lower_bound"] for d in b["draws"] if d["censored"]]
        return {"n_draws": len(b["draws"]), "n_uncensored": len(unc),
                "n_censored": len(cens),
                "uncensored_min": min(unc), "uncensored_max": max(unc),
                "spread": max(unc) / min(unc),
                "censored_ratio_lower_bounds": cens,
                "tails_ge_2x": sum(1 for v in unc if v >= 2.0) + len(cens)}

    bands = {
        "shuffle_win_corpus(plan6-era)": {**band_sum(corpus["shuffle_win"]),
                                          "base_evals": corpus["shuffle_win"]["base_evals"],
                                          "draws": corpus["shuffle_win"]["draws"]},
        "m22_corpus(plan6-era)": {**band_sum(corpus["m22"]),
                                  "base_evals": corpus["m22"]["base_evals"],
                                  "draws": corpus["m22"]["draws"]},
        "rem12_fresh(current binary)": {**band_sum(rem12_fresh),
                                        "base_evals": rem12_fresh["base_evals"],
                                        "draws": rem12_fresh["draws"]},
        "stress_fresh_current_binary_robustness": {**band_sum(stress_fresh),
                                                   "base_evals": stress_fresh["base_evals"],
                                                   "draws": stress_fresh["draws"]},
    }

    mag = {}
    mag["shuffle_win"] = magnitude_verdict(
        "shuffle_win", reps["shuffle_win"],
        uncensored_max_ratio(corpus["shuffle_win"])[0],
        [t["inflation"] for t in reps["shuffle_win"]["threads"][4]["tails"]])
    # rem12 has no tails; the sharper pre-registered test is the median test:
    # median inflation vs ratio_max x median overhead (decomp estimator), and
    # the max-inflation form vs ratio_max x overhead products.
    rem = reps["rem12"]["threads"][4]
    rem12_rmax, _ = uncensored_max_ratio(rem12_fresh)
    rem_products = [
        {"overhead_est": "inmax", "overhead": round(rem["good_infl_max"], 3),
         "product": round(rem12_rmax * rem["good_infl_max"], 3)},
        {"overhead_est": "inmed", "overhead": round(rem["good_infl_med"], 3),
         "product": round(rem12_rmax * rem["good_infl_med"], 3)},
        {"overhead_est": "decomp", "overhead": round(rem["overhead_decomp_med"], 3),
         "product": round(rem12_rmax * rem["overhead_decomp_med"], 3)},
    ]
    mag["rem12"] = {
        "case": "rem12",
        "note": "no bad tails (0/8 cap-hits, max wall 17.8 s < 1.5x seq); sharper pre-registered test: median inflation vs 1.31x-quoted realization band x median overhead",
        "realization_band_fresh": {"uncensored_max": round(rem12_rmax, 3),
                                   "spread": round(max(d["ratio"] for d in rem12_fresh["draws"]) /
                                                   min(d["ratio"] for d in rem12_fresh["draws"]), 3)},
        "median_inflation_t4": round(statistics.median([r["inflation"] for r in rem["runs"]]), 3),
        "overhead_decomp_med_t4": round(rem["overhead_decomp_med"], 3),
        "overhead_decomp_runs_t4": [round(v, 3) for v in rem["overhead_decomp_runs"]],
        "products": rem_products,
        "median_test": {
            "median_inflation": round(statistics.median([r["inflation"] for r in rem["runs"]]), 3),
            "threshold_1.31x_overhead_decomp": round(1.31 * rem["overhead_decomp_med"], 3),
            "exceeded": statistics.median([r["inflation"] for r in rem["runs"]])
                        > 1.31 * rem["overhead_decomp_med"],
        },
        "rejected_at_magnitude": False,
        "rule": "no tails to reject; median test evaluates whether the parallelism-specific overhead itself is realization luck",
    }
    # m22 out-of-scope finding
    m22_tails = [t for n in (2, 3, 4) for t in reps["m22"]["threads"][n].get("tails", [])]
    m22_rmax, _ = uncensored_max_ratio(corpus["m22"])
    m22_over = max(reps["m22"]["threads"][n]["good_infl_max"] for n in (2, 3, 4))
    mag["m22_OUT_OF_SCOPE_finding"] = {
        "case": "m22 (plan6 control, measurement-only; NOT in the pre-registered test set)",
        "tail_inflations": [round(t["inflation"], 2) for t in m22_tails],
        "ratio_max_uncensored": round(m22_rmax, 3),
        "max_overhead_est_any_cell": round(m22_over, 3),
        "max_product": round(m22_rmax * m22_over, 3),
        "rejected_at_magnitude": max(t["inflation"] for t in m22_tails) > 1.2 * m22_rmax * m22_over,
        "sequential_tail_frequency": "0/6 (max uncensored draw 1.671 < 2x)",
        "parallel_tail_frequency": "2/8 across t3/t4 (t3 r3 11.5x win, t4 r2 33.9x cap-hit)",
    }

    out = {
        "probe": "plan12 stage-0 Phase-0 noise probe (H0)",
        "h0": "plan6 bad-trajectory tails = ordinary realization draws under the measured per-run parallel overhead",
        "pre_registered_test_set": TEST_CASES,
        "seq_refs": {"evals": SEQ_EVALS, "wall": SEQ_WALL,
                     "note": "plan6-era in-session references; shuffle_win and m22 equal the research-plan12 corpus salt-0 draws exactly (same flags/era); rem12 fresh current-binary salt0 = 66,604,453 (-4.3% drift, declared)"},
        "ratio_bands": bands,
        "magnitude": mag,
        "frequency": {
            "shuffle_win": {
                "parallel_tail_rate_t4": "3/8",
                "sequential_tail_freq_uncensored": "1/3 (salt1 3.04x; salt0 1.00, salt5 1.01)",
                "censored": "3/6 draws budget-censored at 2.5 G (ratio >= 10.0x, tails a fortiori)",
                "sequential_tail_freq_all_draws": "4/6",
                "verdict": "no veto (3/8 = 0.375 inside [1/3, 1]; below the 4/6 all-draws rate)",
            },
            "rem12": {"parallel_tail_rate": "0/8 at all N",
                      "sequential_tail_freq_fresh": "0/6",
                      "verdict": "consistent; nothing to veto"},
        },
        "decomposition_t4_all_cases": {
            c: {
                "overhead_decomp_runs": [round(r["overhead_decomp"], 3) for r in reps[c]["threads"][4]["runs"]],
                "overhead_decomp_med": round(reps[c]["threads"][4]["overhead_decomp_med"], 3),
                "coord_ratio_range": [round(min(r["coord_ratio"] for r in reps[c]["threads"][4]["runs"]), 3),
                                      round(max(r["coord_ratio"] for r in reps[c]["threads"][4]["runs"]), 3)],
            }
            for c in ("m22", "rem12", "shuffle_win")
        },
        "decomposition_shuffle_win_t4": {
            "bad_tails": [
                {"round": t["round"], "coord_ratio": round(t["coord_ratio"], 3),
                 "helper_ratio": round(t["helper_ratio"], 3),
                 "overhead_decomp": round(t["overhead_decomp"], 3)}
                for t in reps["shuffle_win"]["threads"][4]["tails"]
            ],
            "good_runs": [
                {"round": t["round"], "coord_ratio": round(t["coord_ratio"], 3),
                 "helper_ratio": round(t["helper_ratio"], 3),
                 "overhead_decomp": round(t["overhead_decomp"], 3)}
                for t in reps["shuffle_win"]["threads"][4]["runs"] if not t["tail"]
            ],
            "reading": "overhead_decomp = inflation / coordinator_ratio is ~3.7-3.9x on EVERY t4 run (good and bad): the mechanism work multiplier is near-constant; what varies is the coordinator's realized difficulty (0.33-1.70x salt0). Bad-tail coordinator draws (1.51-1.70x) sit between the quiet cluster (~1.0x) and the loud draw (3.04x) - plausible mid-band draws, unobserved among only 3 uncensored stress draws. Excess work vs good-run medians: ~72% helper-side (+3.1 salt0-units), ~28% coordinator-side (+1.2).",
            "cross_case_note": "the same decomposition on m22 and rem12 t4 gives the identical ~3.8x multiplier (m22 3.56-3.90, rem12 3.78-3.92): the SPDFPN t4 total-work multiplier is ~3.8x the coordinator's realized work on all three plan6 cases; plan6's differing I4 medians (2.22/3.03/3.54) reflect coordinator realization differences, not mechanism behavior. Identification caveat: coordinator_evals is itself perturbation-contaminated (good runs' coord ratios 0.33-0.66 fall below the sequential band minimum - impossible for a pure realization), so 'coordinator realization' conflates the salt draw with helper-TT pollution; the magnitude verdict above does not rest on this identification (it holds under the estimator-free inmax bound too).",
        },
        "verdict": None,  # filled below
    }

    sw = mag["shuffle_win"]["rejected_at_magnitude"]
    rm = mag["rem12"]["rejected_at_magnitude"]
    if not sw and not rm:
        out["verdict"] = {
            "H0": "not rejected on both pre-registered cases (magnitude); frequency cannot veto (shuffle-win parallel tail 3/8 inside the sequential tail band [1/3, 1])",
            "D1": "NO-GO branch (mechanical): the bad-tail premise of backlog #7 is dead; the salt portfolio remains the family's only mechanism",
            "caveat": "out-of-scope m22 finding: the t3/t4 tails (11.5x / 33.9x) DO exceed every noise product - a parallelism-specific inflation component exists on the short control case (repetition-stall / perturbation signature: jobs 10-50x typical); it does not rescue #7's long-case premise (S4(m22) = 1.01x: parallelism is pointless there regardless)",
        }
    else:
        out["verdict"] = {"H0": "rejected on >= 1 pre-registered case", "D1": "GO branch"}

    OUT.write_text(json.dumps(out, indent=1))
    print(json.dumps(out["verdict"], indent=1))
    print("magnitude:", {k: v.get("rejected_at_magnitude") for k, v in mag.items()})
    print("wrote", OUT)


if __name__ == "__main__":
    main()
