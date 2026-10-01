#!/usr/bin/env python3
"""plan6 census assembly + gate checks (D2).

Reads the four arms' transcripts + the H5 replays (raw logs are gitignored;
the parsed census is the record) and writes census_arm_<X>.json +
verdicts.json. Arms (plan6 §4, all from the identical post-batch-2 state =
data/proofdb.db + the plan5 ledger snapshot, staging copies):

  A   - status-quo control: the plan4 selector (pre-refactor binary), no cfg;
  A'  - equivalence replay: the new selector, degenerate config
        (reserve_share 0, layer_visit_cap 0) — must equal arm A on every job
        field except wall_s and the new `kind` census field, and byte-match
        its post-run ledger (the equivalence contract);
  B   - the §2 mechanism defaults (reserve_share 0.25, layer_visit_cap 24,
        interleave_k 4, no-virgin-child eligibility, geometric growth,
        max_rung_passes 3, fewest-passes rotation);
  C   - ladder deletion: reserve_share 0, layer_visit_cap 24.

Checks (no transcript re-reads needed afterwards):
  - H1: session-start `pns:` census per arm (identical modulo the cfg echo;
    the pinned post-batch-2 numbers: rows 35055 / open 75, ledger records
    3024, 0 dropped, exclusions r26, jobs 3073 = 49 rows + 3024 ledger);
    number split from the initial ledger: 2923 number-1 + 150 number-2;
    job paths in-tree (ledger record or open row; never a proven row);
    census quadruple (kind, pass, number, work_before) consistent with the
    selection state (number == pass on this frontier: structural 1 + the
    censor bumps);
  - H5: per-arm replay (identical job sequence excl wall_s; byte-identical
    post-run ledger) + the A' ≡ A field-wise match;
  - B mechanics: 9 rungs (reserve-bound), all censored, rotation order,
    per-ply-layer expansion caps ≤ 24, reserve accounting;
  - the §4.5 decision-rule data: B vs C on facts / max ply / evals per
    ply layer / rung yield.

The input DB sha256 670e19e3... ; 0 new shards in every arm, so no DB grew
(H2: the merger was re-run twice, byte-identical to the input DB; see
README). The winning arm's post-run ledger becomes the standing
data/proofdb_work.json (the verdict's side effect; recorded in README).
"""
import json
import sys
from collections import Counter

sys.dont_write_bytecode = True

HERE = "/workspace/atomic_solver/docs/plans/proofdb/measurements/plan6"
ROOT = "/workspace/atomic_solver"
TMP = "/tmp/plan6"
INITIAL_LEDGER = f"{ROOT}/docs/plans/proofdb/measurements/plan5/ledger_snapshot.json"

ARMS = {  # arm -> (stdout log, cfg echo expected)
    "A": (f"{TMP}/A_stdout.log", None),
    "A_ap": (f"{TMP}/A_ap_stdout.log", "reserve_share 0, layer_visit_cap 0"),
    "B": (f"{TMP}/B_stdout.log", "reserve_share 0.25, layer_visit_cap 24"),
    "C": (f"{TMP}/C_stdout.log", "reserve_share 0, layer_visit_cap 24"),
}
REPLAYS = {"A_ap": f"{TMP}/A_ap_replay_stdout.log", "B": f"{TMP}/B_replay_stdout.log",
           "C": f"{TMP}/C_replay_stdout.log"}


def jobs(path):
    return [json.loads(l[4:]) for l in open(path) if l.startswith("job:")]


def stderr(path):
    return open(f"{path[:-11]}_stderr.log").read() if path.endswith("_stdout.log") else ""


def pns_line(path):
    lines = [l.strip() for l in stderr(path).split("\n") if l.startswith("pns:")]
    assert len(lines) == 1, f"expected one pns: line in {path}"
    return lines[0]


def summarize(records):
    return {
        "jobs": len(records),
        "decisive": sum(1 for r in records if r["tag"]),
        "censored": sum(1 for r in records if not r["tag"]),
        "child_evals": sum(r["child_evals"] for r in records),
        "wall_s_sum": round(sum(r["wall_s"] for r in records), 1),
        "by_kind": dict(Counter(r.get("kind") for r in records)),
        "by_class": dict(Counter(r["class"] for r in records)),
        "by_ply": {
            p: {"jobs": sum(1 for r in records if len(r["path"].split()) == p),
                "evals": sum(r["child_evals"] for r in records
                             if len(r["path"].split()) == p)}
            for p in sorted({len(r["path"].split()) for r in records})
        },
        "by_pass": dict(Counter(r["pass"] for r in records)),
        "exit_reasons": sorted({r["exit_reason"] for r in records}),
    }


def field_diffs(a, b, skip):
    return [(i, k) for i, (x, y) in enumerate(zip(a, b)) for k in x
            if k not in skip and x[k] != y[k]]


import hashlib


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


initial = json.load(open(INITIAL_LEDGER))["records"]
init_map = {tuple(r["path"]): r for r in initial}
cens = [r for r in initial if r["passes_failed"] > 0]
fresh = [r for r in initial if r["passes_failed"] == 0]
number_split = {
    "number_1": len(fresh) + 40,  # fresh ledger + unvisited open rows
    "number_2": len(cens),        # 9 on open rows + 141 ledger nodes
    "total_jobs": len(fresh) + 40 + len(cens),
}

PINNED_PREFIX = (
    "pns: rows 35055 (open 75), ledger records 3024 (3024 new, 0 dropped as "
    "decided); exclusions proven-ancestor r26/l0, implied-win r0/l0, "
    "implied-loss r0/l0; jobs 3073 (rows 49, ledger 3024); base budget 4000000"
)

arm_data = {}
for arm, (log, cfg_echo) in ARMS.items():
    recs = jobs(log)
    line = pns_line(log)
    assert line.startswith(PINNED_PREFIX), line
    cfg_in_line = line[len(PINNED_PREFIX):]
    if cfg_echo is None:
        assert cfg_in_line == "", f"arm {arm} must have no cfg echo: {cfg_in_line!r}"
    else:
        assert cfg_in_line.startswith(f"; cfg {cfg_echo},"), \
            f"arm {arm} cfg echo: {cfg_in_line!r}"
    pre_paths = {" ".join(p) for p in init_map}
    post = json.load(open(f"{TMP}/{arm}/ledger.json"))["records"]
    post_paths = {" ".join(r["path"]) for r in post}
    for r in recs:
        p = r["path"]
        # H1: in-tree at visit time — every censored job's path is a ledger
        # record in its arm's post-run ledger (bump or exposure guarantee);
        # the tool's extraction asserts non-row/non-manifest disjointness.
        assert p in post_paths, f"job path {p!r} not in the post-run ledger"
        ply = len(p.split())
        if r["pass"] == 1:
            assert (r["number"], r["work_before"]) == (1, 0), r
        else:
            # revisit: number = 1 + passes_failed; work_before ≈ cumulative
            # 4M per censored visit (standing work + this session's bumps;
            # ± the small per-job eval-counter overhang)
            assert r["number"] == r["pass"], r
            assert 4_000_000 * (r["pass"] - 1) <= r["work_before"] \
                <= 4_000_000 * r["pass"] + 100 * r["pass"], r
        assert r["policy"] == "breadth-pns"
        _ = ply
    arm_data[arm] = {"jobs": recs, "line": line, "summary": summarize(recs),
                     "post_ledger": post}

# --- A' ≡ A: the equivalence contract (fields except wall_s and kind) ---
a, ap = arm_data["A"]["jobs"], arm_data["A_ap"]["jobs"]
eq_diffs = field_diffs(a, ap, skip={"wall_s", "kind"})
assert len(a) == len(ap) and not eq_diffs, eq_diffs[:5]
assert all(r.get("kind") == "expand" for r in ap), "A' emits kind=expand"
assert all("kind" not in r for r in a), "arm A (old binary) emits no kind"
ledger_a, ledger_ap = sha(f"{TMP}/A/ledger.json"), sha(f"{TMP}/A_ap/ledger.json")
ap_eq = {"job_records_identical_mod_wall_s_kind": True,
         "post_run_ledger_bytes_identical": ledger_a == ledger_ap,
         "ledger_sha256": ledger_a}
assert ap_eq["post_run_ledger_bytes_identical"]

# --- H5 per-arm replays ---
h5 = {}
for arm, rlog in REPLAYS.items():
    rr = jobs(rlog)
    diffs = field_diffs(arm_data[arm]["jobs"], rr, skip={"wall_s"})
    assert len(rr) == len(arm_data[arm]["jobs"]) and not diffs, (arm, diffs[:3])
    l1, l2 = sha(f"{TMP}/{arm}/ledger.json"), sha(f"{TMP}/{arm}_replay/ledger.json")
    h5[arm] = {"job_sequence_identical": True, "ledger_bytes_identical": l1 == l2,
               "ledger_sha256": l1}
    assert l1 == l2, arm

# --- arm B mechanics ---
b = arm_data["B"]["jobs"]
rungs = [r for r in b if r["kind"] == "rung"]
expands = [r for r in b if r["kind"] == "expand"]
reserve = 0.25 * 300_000_000
rung_evals = sum(r["child_evals"] for r in rungs)
layer_exp = Counter(len(r["path"].split()) for r in expands)
b_mech = {
    "rung_visits": len(rungs),
    "rung_evals": rung_evals,
    "reserve_budget": int(reserve),
    "reserve_bound": rung_evals >= reserve,  # the reserve was the binding limit
    "all_rungs_censored": all(r["outcome"] == "censored" for r in rungs),
    "rung_targets_in_order": [r["path"] for r in rungs],
    "rung_targets_all_at_pass_2": sorted({r["pass"] for r in rungs}) == [2, 3],
    "max_expansion_visits_per_layer": dict(layer_exp),
    "layer_caps_respected": all(v <= 24 for v in layer_exp.values()),
    "facts": sum(1 for r in b if r["tag"]),
    "max_ply": max(len(r["path"].split()) for r in b),
}
# Rotation order: the eligible-at-start set, fewest-passes (all pass 1) ties
# broken by (ply, path) → the root, then ply-1 in path order.
eligible_at_start = ["", "a2a3", "a2a4", "b1a3", "b1c3", "b2b3", "b2b4"]
observed = [r["path"] for r in rungs]
assert observed[:len(eligible_at_start)] == eligible_at_start, observed
assert observed[len(eligible_at_start):] == ["c2c3", ""], \
    "c2c3 flipped eligible mid-session; the root's pass-3 rung closes the reserve"
b_mech["rotation_order_verified"] = True

# --- the §4.5 decision rule: B vs C ---
c = arm_data["C"]["jobs"]
decision = {
    "facts": {"B": b_mech["facts"], "C": sum(1 for r in c if r["tag"])},
    "max_ply_reached": {"B": b_mech["max_ply"], "C": max(len(r["path"].split()) for r in c)},
    "evals_per_ply_layer": {
        "B": {p: v["evals"] for p, v in arm_data["B"]["summary"]["by_ply"].items()},
        "C": {p: v["evals"] for p, v in arm_data["C"]["summary"]["by_ply"].items()},
    },
    "rung_yield": {"B": {"rungs": len(rungs), "facts": b_mech["facts"],
                          "all_censored": b_mech["all_rungs_censored"]}, "C": {"rungs": 0}},
}
ladder_no_go = (
    decision["facts"]["B"] <= decision["facts"]["C"]
    and b_mech["all_rungs_censored"]
    and decision["max_ply_reached"]["B"] <= decision["max_ply_reached"]["C"]
)
decision["verdict"] = "ladder no-go: default config becomes arm C's" if ladder_no_go \
    else "ladder stays at the measured share"

# --- write the per-arm census JSONs + verdicts ---
for arm in ARMS:
    d = arm_data[arm]
    out = {
        "arm": arm,
        "policy": "breadth-pns",
        "session_start_census": {
            "pns_line": d["line"],
            "number_split": number_split,
        },
        "batch": {
            "stop": "budget",
            "max_total_evals": 300_000_000,
            "base_budget_evals": 4_000_000,
            "summary": d["summary"],
            "jobs": d["jobs"],
        },
    }
    if arm == "A_ap":
        out["equivalence_to_arm_A"] = ap_eq
    if arm in h5:
        out["h5_replay"] = h5[arm]
    if arm == "B":
        out["mechanics"] = b_mech
    with open(f"{HERE}/census_arm_{arm}.json", "w") as f:
        json.dump(out, f, indent=1)
        f.write("\n")

with open(f"{HERE}/verdicts.json", "w") as f:
    json.dump({
        "ap_equiv_A": ap_eq,
        "h5_replays": h5,
        "arm_B_mechanics": b_mech,
        "decision_rule_B_vs_C": decision,
        "input_ledger_sha256": sha(f"{TMP}/pristine/ledger_init.json"),
        "post_run_ledger_sha256": {arm: sha(f"{TMP}/{arm}/ledger.json") for arm in ARMS},
    }, f, indent=1)
    f.write("\n")

print("per-arm census JSONs + verdicts.json written")
for arm in ARMS:
    s = arm_data[arm]["summary"]
    print(f"  {arm}: {s['jobs']} jobs ({s['by_kind']}), facts {s['decisive']}, "
          f"plies {sorted(s['by_ply'])}, evals {s['child_evals']}")
print("A'≡A:", ap_eq["job_records_identical_mod_wall_s_kind"],
      "| ledgers equal:", ap_eq["post_run_ledger_bytes_identical"])
print("decision:", json.dumps(decision["verdict"]))
