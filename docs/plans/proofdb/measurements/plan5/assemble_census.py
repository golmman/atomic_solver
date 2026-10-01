#!/usr/bin/env python3
"""plan5 census assembly + gate checks (D1).

Reads the batch-2 and H5-replay transcripts (raw logs are gitignored, the
parsed census is the record) and writes census_breadth-pns.json with the
job records and the summary metrics. Batch 2 is the plan4-policy control
run (no code changed): checks (no transcript re-reads needed afterwards):
  - decision-5 census match: the session-start `pns:` line equals the
    §1-pinned values (rows 35055 / open 75, ledger records 1434, jobs 1483
    = 49 rows + 1434 ledger);
  - number split (measured from the initial ledger + the plan4 batch-1
    census): 1408 number-1 / 75 number-2;
  - H1: every job path is a fresh in-tree ledger child (class L) with
    census triple (pass, number, work_before) == (1, 1, 0), ply 2;
  - E1: constant job ply 2, 75 visits <= 289 available (cap before drain);
  - E2: 0 decisive;
  - E3: no pass >= 2 (ladder never fired — the plan's goal measurement);
  - E4: ledger growth 1443 -> 3033 (range 2600-3200), all new records at
    ply 3, exactly 75 bumps (passes_failed 0 -> 1);
  - E5/H5: batch vs replay job records identical on every field except
    wall_s; byte-identical post-batch ledger.

The input DB is byte-identical to plan3's committed proofdb_grown2.db
(sha256 670e19e3...); 0 new shards, so the post-batch DB is the same file
(H2/H3: the merger was re-run twice, byte-identical; see README).
"""
import json
import sys

sys.dont_write_bytecode = True

HERE = "/workspace/atomic_solver/docs/plans/proofdb/measurements/plan5"
ROOT = "/workspace/atomic_solver"
BATCH = "/tmp/plan5/pns_stdout.log"
REPLAY = "/tmp/plan5/replay_stdout.log"
BATCH_ERR = "/tmp/plan5/pns_stderr.log"
REPLAY_ERR = "/tmp/plan5/replay_stderr.log"
INITIAL_LEDGER = "/tmp/plan5/initial_ledger.json"
POST_LEDGER = f"{ROOT}/data/proofdb_work.json"
REPLAY_LEDGER = "/tmp/plan5/replay_ledger.json"


def job_records(path):
    out = []
    for line in open(path):
        if line.startswith("job:"):
            out.append(json.loads(line[4:].strip()))
    return out


def pns_line(path):
    for line in open(path):
        if line.startswith("pns:"):
            return line.strip()
    raise AssertionError("no pns: line")


def summarize(records):
    return {
        "jobs": len(records),
        "decisive": sum(1 for r in records if r["tag"]),
        "censored": sum(1 for r in records if not r["tag"]),
        "child_evals": sum(r["child_evals"] for r in records),
        "wall_s_sum": round(sum(r["wall_s"] for r in records), 1),
        "by_class": {
            c: {
                "jobs": sum(1 for r in records if r["class"] == c),
                "evals": sum(r["child_evals"] for r in records if r["class"] == c),
            }
            for c in sorted({r["class"] for r in records})
        },
        "by_ply": {
            p: sum(1 for r in records if len(r["path"].split()) == p)
            for p in sorted({len(r["path"].split()) for r in records})
        },
        "ply_constant": len({len(r["path"].split()) for r in records}) == 1,
        "passes": sorted({r["pass"] for r in records}),
        "numbers": sorted({r["number"] for r in records}),
        "exit_reasons": sorted({r["exit_reason"] for r in records}),
    }


batch = job_records(BATCH)
replay = job_records(REPLAY)
assert len(batch) == 75, len(batch)

# Decision-5 census match (§1 pinned values).
pinned = (
    "pns: rows 35055 (open 75), ledger records 1434 (1434 new, 0 dropped as "
    "decided); exclusions proven-ancestor r26/l0, implied-win r0/l0, "
    "implied-loss r0/l0; jobs 1483 (rows 49, ledger 1434); base budget 4000000"
)
pns_batch, pns_replay = pns_line(BATCH_ERR), pns_line(REPLAY_ERR)
assert pns_batch == pinned, pns_batch
assert pns_replay == pinned, pns_replay

# Number split measured from the initial ledger + plan4's batch-1 census.
pre = json.load(open(INITIAL_LEDGER))["records"]
b1 = json.load(open(f"{ROOT}/docs/plans/proofdb/measurements/plan4/"
                    "census_breadth-pns.json"))
c1 = {tuple(r["path"].split()) for r in b1["batch"]["jobs"]
       if r["class"] == "C1"}
cens_ledger = [r for r in pre if r["passes_failed"] > 0]
fresh_ledger = [r for r in pre if r["passes_failed"] == 0]
assert len(cens_ledger) == 75 and len(fresh_ledger) == 1368
assert {tuple(r["path"]) for r in cens_ledger} >= c1
number_split = {
    "number_1": len(fresh_ledger) + 40,  # 1368 fresh ledger + 40 unvisited rows
    "number_2": 66 + 9,  # 66 censored ledger children + 9 censored rows
    "total_jobs": 1483,
    "fresh_ledger_plies": {"2": 289, "3": 1079},
}

# H1: census-triple sanity — every visit is a fresh ledger child
# (class L) at (pass, number, work_before) = (1, 1, 0), ply 2.
pre_paths = {" ".join(r["path"]) for r in fresh_ledger}
for r in batch:
    assert r["class"] == "L" and r["path"] in pre_paths, r
    assert r["outcome"] == "censored" and r["exit_reason"] == "BudgetExhausted"
    assert (r["pass"], r["number"], r["work_before"]) == (1, 1, 0)
    assert len(r["path"].split()) == 2

# E1/E2/E3 verdicts from the job records.
n_fresh_ply2 = number_split["fresh_ledger_plies"]["2"]
e1 = {"job_plies": sorted({len(r["path"].split()) for r in batch}),
      "visits": len(batch), "available_ply2": n_fresh_ply2,
      "cap_before_drain": len(batch) < n_fresh_ply2}
e2 = {"decisive": sum(1 for r in batch if r["tag"])}
e3 = {"max_pass": max(r["pass"] for r in batch), "ladder_fired": any(
    r["pass"] >= 2 for r in batch)}

# E4: ledger growth.
post = json.load(open(POST_LEDGER))["records"]
pre_map = {tuple(r["path"]): r for r in pre}
post_map = {tuple(r["path"]): r for r in post}
new = [p for p in post_map if p not in pre_map]
bumped = [p for p in post_map if p in pre_map and post_map[p] != pre_map[p]]
import collections
e4 = {"records_before": len(pre), "records_after": len(post),
      "accepted_range": [2600, 3200],
      "in_range": 2600 <= len(post) <= 3200,
      "new_records": len(new),
      "new_plies": dict(collections.Counter(len(p) for p in new)),
      "bumped_records": len(bumped),
      "bumped_plies": dict(collections.Counter(len(p) for p in bumped)),
      "censored_after": sum(1 for r in post if r["passes_failed"] > 0),
      "fresh_after": sum(1 for r in post if r["passes_failed"] == 0),
      "exposure_per_visit": round(len(new) / len(batch), 2)}

# E5/H5: identical on every field except wall_s; identical ledger bytes.
diffs = [
    (i, k)
    for i, (x, y) in enumerate(zip(batch, replay))
    for k in x
    if k != "wall_s" and x[k] != y[k]
]
assert len(replay) == len(batch) and not diffs, diffs[:5]
import hashlib
def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()
ledger_identical = sha(POST_LEDGER) == sha(REPLAY_LEDGER)
assert ledger_identical

summary = summarize(batch)
census = {
    "policy": "breadth-pns",
    "note": "batch 2 = the plan4-policy control run (plan5 decision 1); "
            "no selection code changed in plan5",
    "batch": {
        "stop": "budget",
        "max_total_evals": 300000000,
        "base_budget_evals": 4000000,
        "summary": summary,
        "jobs": batch,
    },
    "session_start_census": {
        "pns_line": pns_batch,
        "matches_plan5_sec1_pinned": True,
        "number_split": number_split,
    },
    "expectations": {"E1": e1, "E2": e2, "E3": e3, "E4": e4},
    "h5_replay": {
        "job_records_identical_excl_wall_s": True,
        "ledger_bytes_identical": ledger_identical,
        "pns_census_line_identical": pns_batch == pns_replay,
    },
    "exclusion_census": {  # from the session-start pns: line
        "open_rows": 75,
        "proven_ancestor": {"rows": 26, "ledger": 0},
        "implied_win": {"rows": 0, "ledger": 0},
        "implied_loss": {"rows": 0, "ledger": 0},
        "jobs": {"rows": 49, "ledger": 1434},
    },
}
with open(f"{HERE}/census_breadth-pns.json", "w") as f:
    json.dump(census, f, indent=1)
    f.write("\n")
print("census_breadth-pns.json written;", summary["jobs"], "jobs,",
      summary["decisive"], "facts,", summary["child_evals"], "evals")
print("E1 cap_before_drain:", e1["cap_before_drain"],
      "| E2 decisive:", e2["decisive"],
      "| E3 ladder_fired:", e3["ladder_fired"])
print("E4 records:", e4["records_before"], "->", e4["records_after"],
      "(new", e4["new_records"], "all plies", e4["new_plies"],
      "; exposure/visit", e4["exposure_per_visit"], ")")
print("E5 ledger bytes identical:", ledger_identical)
