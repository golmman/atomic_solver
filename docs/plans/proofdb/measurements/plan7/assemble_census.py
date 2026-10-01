#!/usr/bin/env python3
"""plan7 census assembly + gate checks (D2).

Reads the two arms' transcripts + the H5 replays (raw logs stay in
/tmp/plan7, gitignored; the parsed census is the record) and writes
census_arm_{1,2}.json + verdicts.json + union.json. Arms (plan7 §4, both
from the standing post-plan6 state, staging copies, fresh TT, cap 300M,
base 4M, root = startpos, policy breadth-pns = the shipped default, no
--pns-config):

  arm 1 (control) - the standing ledger as-is (data/proofdb_work.json,
                    sha 058a6202... at run time);
  arm 2 (union)   - the ledger-unioned copy (standing + plan6 arm A/B
                    snapshots, 5,950 records, built by
                    proofdb_ledger_union with pinned digests).

Checks:
  - H1: the pinned session-start `pns:` census per arm (arm 1: ledger
    records 4559 / jobs 4608; arm 2: 5938 / 5987; identical otherwise,
    incl. the cfg echo of the compiled defaults); every job path in its
    arm's post-run ledger; census quadruple (pass 1 visits at
    (number 1, work_before 0)); manifest unchanged;
  - the §4.3 re-censor metric (finding 4 made measurable): a job is a
    re-censor iff its path had passes_failed = 0 at session start and the
    path appears censored in any committed plan6 arm snapshot;
  - H5: per-arm replay (identical job sequence excl wall_s;
    byte-identical post-run ledger);
  - the §4.5 decision rule (the union's marginal value) and the §4.6
    closure data for plan8.

Union determinism/idempotence/normalization/coverage are checked by the
driver commands in README.md (byte digests below) plus the §2.2 coverage
assertion over the real artifacts (re-run here).
"""
import json
import sys
from collections import Counter

sys.dont_write_bytecode = True

HERE = "/workspace/atomic_solver/docs/plans/proofdb/measurements/plan7"
ROOT = "/workspace/atomic_solver"
TMP = "/tmp/plan7"
PIN6 = f"{ROOT}/docs/plans/proofdb/measurements/plan6"
STANDING_AT_RUN_TIME = "058a62026e1109ad4573dd85eb958c9b6de590d999564049f4755f74cc554aa4"
PINNED_PREFIX = (
    "pns: rows 35055 (open 75), ledger records {ledger} ({ledger} new, 0 dropped as "
    "decided); exclusions proven-ancestor r26/l0, implied-win r0/l0, "
    "implied-loss r0/l0; jobs {jobs} (rows 49, ledger {ledger}); base budget 4000000"
)
CFG_ECHO = ("; cfg reserve_share 0, layer_visit_cap 24, interleave_k 4, "
            "eligibility no-virgin-child, rung_growth geometric, "
            "max_rung_passes 3, rotation fewest-passes")
ARMS = {  # arm -> (dir, pinned ledger records, pinned jobs, session-start ledger)
    # arm 1's start = the standing ledger at run time = the plan6 arm-C
    # snapshot (sha 058a6202..., verified byte-identical to
    # data/proofdb_work.json before the post-verdict advance).
    "arm1": (f"{TMP}/arm1", 4559, 4608, f"{PIN6}/ledger_snapshot_C.json"),
    "arm2": (f"{TMP}/arm2", 5938, 5987, f"{TMP}/union/ledger.json"),
}


def jobs(path):
    return [json.loads(l[4:]) for l in open(path) if l.startswith("job:")]


def load_ledger(p):
    d = json.load(open(p))["records"]
    return {tuple(r["path"]): r for r in d}


def sha(path):
    import hashlib
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


snap6 = {a: load_ledger(f"{PIN6}/ledger_snapshot_{a}.json") for a in ("A", "B", "C")}
censored6 = {p for m in snap6.values() for p, r in m.items() if r["passes_failed"] > 0}

arm_data = {}
for arm, (d, pin_ledger, pin_jobs, start_path) in ARMS.items():
    recs = jobs(f"{d}/{arm}_stdout.log")
    lines = [l.strip() for l in open(f"{d}/{arm}_stderr.log") if l.startswith("pns:")]
    assert len(lines) == 1, arm
    line = lines[0]
    pinned = PINNED_PREFIX.format(ledger=pin_ledger, jobs=pin_jobs)
    assert line.startswith(pinned), line
    assert line[len(pinned):] == CFG_ECHO, f"arm {arm} cfg echo: {line[len(pinned):]!r}"
    start = load_ledger(start_path)
    post = load_ledger(f"{d}/ledger.json")
    # H1: every job path is a record in the post-run ledger; quadruple.
    for j in recs:
        p = tuple(j["path"].split())
        assert p in post, (arm, j["path"])
        if j["pass"] == 1:
            assert (j["number"], j["work_before"]) == (1, 0), j
        else:
            assert j["number"] == j["pass"], j
        assert j["policy"] == "breadth-pns" and j["outcome"] == "censored"
    manifest_line = [l for l in open(f"{d}/{arm}_stdout.log")
                     if l.startswith("manifest:")][0].strip()
    assert "unchanged (197 entries" in manifest_line, manifest_line
    # §4.3 re-censor metric.
    recensor = [j for j in recs
                if start[tuple(j["path"].split())]["passes_failed"] == 0
                and tuple(j["path"].split()) in censored6]
    ply = Counter(len(j["path"].split()) for j in recs)
    evalply = {p: sum(j["child_evals"] for j in recs if len(j["path"].split()) == p)
               for p in sorted(ply)}
    newpaths = sum(1 for p in post if p not in start)
    newply = Counter(len(p) for p in post if p not in start)
    upgrades = sum(1 for p, r in start.items()
                   if p in post and post[p]["passes_failed"] > r["passes_failed"])
    m = {
        "jobs": len(recs),
        "facts": sum(1 for j in recs if j["tag"]),
        "kinds": dict(Counter(j["kind"] for j in recs)),
        "ply_distribution": {str(k): v for k, v in sorted(ply.items())},
        "evals_per_ply": {str(k): v for k, v in evalply.items()},
        "total_evals": sum(j["child_evals"] for j in recs),
        "max_ply": max(ply),
        "n_recensor": len(recensor),
        "recensor_waste_evals": sum(j["child_evals"] for j in recensor),
        "recensor_paths": [j["path"] for j in recensor],
        "ledger_start_records": len(start),
        "ledger_post_records": len(post),
        "ledger_growth": len(post) - len(start),
        "new_paths_vs_start": newpaths,
        "new_paths_by_ply": {str(k): v for k, v in sorted(newply.items())},
        "pass_upgrades_of_known_records": upgrades,
        "manifest_line": manifest_line,
    }
    # H5: replay identity (job sequence excl wall_s, byte-identical ledger).
    rrecs = jobs(f"{TMP}/{arm}_replay/{arm}_replay_stdout.log")
    strip = lambda js: [{k: v for k, v in j.items() if k != "wall_s"} for j in js]
    l1, l2 = sha(f"{d}/ledger.json"), sha(f"{TMP}/{arm}_replay/ledger.json")
    h5 = {"job_sequence_identical": strip(recs) == strip(rrecs),
          "ledger_bytes_identical": l1 == l2, "ledger_sha256": l1}
    assert h5["job_sequence_identical"] and h5["ledger_bytes_identical"], arm
    arm_data[arm] = {"metrics": m, "jobs": recs, "pns_line": line, "h5": h5}

# §2.2 coverage over the real union artifact (re-run of the driver check;
# arm-1's standing input is covered by the --expect digest verification).
u = load_ledger(f"{TMP}/union/ledger.json")
for name, inp in (("A", snap6["A"]), ("B", snap6["B"])):
    for p, r in inp.items():
        x = u.get(p)
        assert x is not None and (
            x["passes_failed"] > r["passes_failed"]
            or (x["passes_failed"] == r["passes_failed"]
                and x["work_done"] >= r["work_done"])), (name, p)

a1, a2 = arm_data["arm1"]["metrics"], arm_data["arm2"]["metrics"]
decision = {
    "condition_1_fewer_recensors": {
        "arm1": a1["n_recensor"], "arm2": a2["n_recensor"],
        "holds": a2["n_recensor"] < a1["n_recensor"]},
    "condition_2_facts": {
        "arm1": a1["facts"], "arm2": a2["facts"], "holds": a2["facts"] >= a1["facts"]},
    "condition_3_max_ply": {
        "arm1": a1["max_ply"], "arm2": a2["max_ply"],
        "holds": a2["max_ply"] >= a1["max_ply"]},
}
decision["verdict"] = ("union ships: standing ledger advances to arm 2's "
                       "post-run ledger; proofdb_ledger_union is the "
                       "standing-state merge tool (§4.5)") if all(
    decision[k]["holds"] for k in
    ("condition_1_fewer_recensors", "condition_2_facts", "condition_3_max_ply")
) else "keep the current advance rule; investigate the union result (§4.5)"

closure = {
    "sessions_at_0_facts": 6,
    "both_arms_0_facts": a1["facts"] == 0 and a2["facts"] == 0,
    "rule": "plan8 must change the yield outlook (depth-rationing lever or "
            "fact-yield-oriented selection change); another identical "
            "default batch is not a valid plan8 (§4.6)",
    "fires": a1["facts"] == 0 and a2["facts"] == 0,
}

for arm in ARMS:
    d = arm_data[arm]
    out = {
        "arm": arm,
        "policy": "breadth-pns (shipped default, no --pns-config)",
        "session_start_census": {"pns_line": d["pns_line"]},
        "metrics": d["metrics"],
        "h5_replay": d["h5"],
        "jobs": d["jobs"],
    }
    with open(f"{HERE}/census_{arm}.json", "w") as f:
        json.dump(out, f, indent=1)
        f.write("\n")

union = {
    "tool": "proofdb_ledger_union (examples/)",
    "inputs": [
        {"file": "data/proofdb_work.json", "sha256": STANDING_AT_RUN_TIME,
         "records": 4569},
        {"file": "docs/plans/proofdb/measurements/plan6/ledger_snapshot_A.json",
         "sha256": sha(f"{PIN6}/ledger_snapshot_A.json"), "records": 4933},
        {"file": "docs/plans/proofdb/measurements/plan6/ledger_snapshot_B.json",
         "sha256": sha(f"{PIN6}/ledger_snapshot_B.json"), "records": 4178},
    ],
    "union_records": 5950,
    "per_input": [
        {"input": "standing", "new_paths": 391, "sole_pass_upgrades": 20},
        {"input": "plan6 arm A", "new_paths": 1381, "sole_pass_upgrades": 49},
        {"input": "plan6 arm B", "new_paths": 0, "sole_pass_upgrades": 8},
    ],
    "gates": {
        "digest_verification_before_merge": "pass (--expect, all 3 pinned)",
        "determinism_any_input_order": "pass",
        "idempotence_union_of_u_and_inputs": "pass",
        "n1_normalization_standing": "pass (byte-identical rewrite)",
        "coverage_property_over_real_artifacts": "pass",
    },
    "union_ledger_sha256": sha(f"{TMP}/union/ledger.json"),
    "ply2_split": {"standing": "fresh 190 / censored 153",
                   "union": "fresh 141 / censored 204 (51 A-known recensors recovered)"},
}
with open(f"{HERE}/union.json", "w") as f:
    json.dump(union, f, indent=1)
    f.write("\n")

with open(f"{HERE}/verdicts.json", "w") as f:
    json.dump({
        "h1_pinned_census": "pass (both arms, incl. cfg echo)",
        "h2_merge_determinism": "pass (DB 670e19e3... twice, = input DB at 0 facts)",
        "h3_h4": "pass (vacuous: 0 promoted facts)",
        "h5_replays": {arm: arm_data[arm]["h5"] for arm in ARMS},
        "recensor_metric": {
            "arm1": {"n": a1["n_recensor"], "waste_evals": a1["recensor_waste_evals"]},
            "arm2": {"n": a2["n_recensor"], "waste_evals": a2["recensor_waste_evals"]},
        },
        "decision_rule_union_marginal_value": decision,
        "closure_rule_plan8": closure,
        "post_run_ledger_sha256": {arm: arm_data[arm]["h5"]["ledger_sha256"]
                                   for arm in ARMS},
    }, f, indent=1)
    f.write("\n")

print("per-arm census JSONs + union.json + verdicts.json written")
for arm in ARMS:
    m = arm_data[arm]["metrics"]
    print(f"  {arm}: {m['jobs']} jobs, facts {m['facts']}, plies {m['ply_distribution']}, "
          f"recensors {m['n_recensor']} (waste {m['recensor_waste_evals']:,}), "
          f"ledger {m['ledger_start_records']}->{m['ledger_post_records']}")
print("decision:", decision["verdict"])
print("closure:", "plan8 must change the yield outlook" if closure["fires"] else "re-open")
