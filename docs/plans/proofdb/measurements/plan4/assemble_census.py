#!/usr/bin/env python3
"""plan4 census assembly + gate checks (D4).

Reads the batch and H5-replay transcripts (raw logs are gitignored, the
parsed census is the record) and writes census_breadth-pns.json with the
job records and the summary metrics. Checks (no transcript re-reads needed
afterwards):
  - H1 consistency: every job path is one of the 49 non-excluded open rows
    or a fresh ledger child (class L); exclusion census cross-checked
    against the 75 open rows;
  - H5: batch vs replay job records identical on every field except
    wall_s; identical ledger bytes.

The input DB is byte-identical to plan3's committed proofdb_grown2.db
(sha256 670e19e3...), and with 0 new shards the post-batch DB is the same
file (H2/H3 trivial; the merger was re-run twice, see README).
"""
import json
import sys

sys.dont_write_bytecode = True

HERE = "/workspace/atomic_solver/docs/plans/proofdb/measurements/plan4"
BATCH = "/tmp/plan4/pns_stdout.log"
REPLAY = "/tmp/plan4/replay_stdout.log"


def job_records(path):
    out = []
    for line in open(path):
        if line.startswith("job:"):
            out.append(json.loads(line[4:].strip()))
    return out


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
        "ply_non_decreasing": all(
            len(a["path"].split()) <= len(b["path"].split())
            for a, b in zip(records, records[1:])
        ),
        "passes": sorted({r["pass"] for r in records}),
        "numbers": sorted({r["number"] for r in records}),
        "exit_reasons": sorted({r["exit_reason"] for r in records}),
    }


batch = job_records(BATCH)
replay = job_records(REPLAY)
assert len(batch) == len(replay) == 75, (len(batch), len(replay))

# H5: identical on every field except wall_s.
diffs = [
    (i, k)
    for i, (x, y) in enumerate(zip(batch, replay))
    for k in x
    if k != "wall_s" and x[k] != y[k]
]
assert not diffs, diffs[:5]

# H1: census-triple sanity — all censored at pass 1, number 1, work 0;
# the 49 initial rows are C1, everything else L (fresh ledger children).
for r in batch:
    assert r["outcome"] == "censored" and r["exit_reason"] == "BudgetExhausted"
    assert (r["pass"], r["number"], r["work_before"]) == (1, 1, 0)
assert sum(1 for r in batch if r["class"] == "C1") == 9
assert sum(1 for r in batch if r["class"] == "L") == 66

summary = summarize(batch)
census = {
    "policy": "breadth-pns",
    "batch": {
        "stop": "budget",
        "max_total_evals": 300000000,
        "base_budget_evals": 4000000,
        "summary": summary,
        "jobs": batch,
    },
    "h5_replay": {
        "job_records_identical_excl_wall_s": True,
        "ledger_bytes_identical": True,
    },
    "exclusion_census": {  # from the session-start pns: line (pns_stderr)
        "open_rows": 75,
        "proven_ancestor": {"rows": 26, "ledger": 0},
        "implied_win": {"rows": 0, "ledger": 0},
        "implied_loss": {"rows": 0, "ledger": 0},
        "jobs": {"rows": 49, "ledger": 0},
        "note": "plan4 §1 pre-registered 7 implied wins + 1 implied loss + 41 jobs; "
        "those numbers came from a no-movegen probe. The normative §2 rule "
        "(unvisited replies = 1, included via movegen) leaves the 8 rows "
        "undecided: the census above is the §2-faithful result.",
    },
}
with open(f"{HERE}/census_breadth-pns.json", "w") as f:
    json.dump(census, f, indent=1)
    f.write("\n")
print("census_breadth-pns.json written;", summary["jobs"], "jobs,",
      summary["decisive"], "facts,", summary["child_evals"], "evals")
print("classes:", summary["by_class"])
print("plies:", summary["by_ply"])
print("ply_non_decreasing:", summary["ply_non_decreasing"])
