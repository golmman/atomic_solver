#!/usr/bin/env python3
"""plan3 gate checks + census assembly (D3).

Reads the three harvest run transcripts (p{0,1,2}_stdout.log, gitignored),
the input/grown dumps, and the standing manifest; writes:
  - census_open-deepest.json / census_sharp-siblings.json /
    census_sharp-heavy-tail.json   (per-policy job records + summary)
  - policy_comparison.json          (the pre-registered metric table)
  - sample_lines.txt                (the merger's --sample-lines 10 output)
  - gate results printed to stdout (H3 diff, H4 spot-checks)

Raw transcripts are NOT committed (measurement conventions); these parsed
results are the record.
"""
import json
import sqlite3
import statistics
import sys

sys.dont_write_bytecode = True

HERE = "/workspace/atomic_solver/docs/plans/proofdb/measurements/plan3"
MANIFEST = "/workspace/atomic_solver/docs/plans/proofdb/shards/manifest.json"

POLICIES = {
    "open-deepest": "p0_stdout.log",
    "sharp-siblings": "p1_stdout.log",
    "sharp-heavy-tail": "p2_stdout.log",
}


def job_records(transcript):
    out = []
    for line in open(f"{HERE}/{transcript}"):
        if line.startswith("job:"):
            out.append(json.loads(line[5:].strip()))
    return out


def summarize(policy, records):
    screen = [r for r in records if r["tier"] == "screen"]
    heavy = [r for r in records if r["tier"] == "heavy"]
    facts = [r for r in records if r["tag"]]
    evals = sum(r["child_evals"] for r in records)
    classes = {}
    for r in screen:
        d = classes.setdefault(r["class"], {"jobs": 0, "decisive": 0, "evals": 0})
        d["jobs"] += 1
        d["evals"] += r["child_evals"]
        if r["tag"]:
            d["decisive"] += 1
    fact_costs = [r["child_evals"] for r in facts]
    by_class = {}
    for r in facts:
        by_class.setdefault(r["class"], []).append(r["child_evals"])
    return {
        "screen_jobs": len(screen),
        "screen_evals": sum(r["child_evals"] for r in screen),
        "heavy_jobs": len(heavy),
        "heavy_evals": sum(r["child_evals"] for r in heavy),
        "heavy_decisive": sum(1 for r in heavy if r["tag"]),
        "jobs_by_class": classes,
        "new_facts": len(facts),
        "fact_evals_total": sum(fact_costs),
        "cost_per_fact_median": statistics.median(fact_costs) if fact_costs else None,
        "cost_per_fact_mean": round(statistics.fmean(fact_costs), 1) if fact_costs else None,
        "facts_per_100m": round(100e6 * len(facts) / evals, 2) if evals else 0.0,
        "fact_cost_by_class": {
            k: {"n": len(v), "median": statistics.median(v)} for k, v in by_class.items()
        },
        "fact_ply_distribution": {
            str(p): sum(1 for r in facts if r["path"].count(" ") + 1 == p)
            for p in sorted({r["path"].count(" ") + 1 for r in facts})
        },
        "fact_outcomes": {
            o: sum(1 for r in facts if r["outcome"] == o)
            for o in {r["outcome"] for r in facts}
        },
        "records": records,
    }


def wall_of(transcript):
    for line in open(f"{HERE}/{transcript}"):
        if line.startswith("harvest: policy "):
            return float(line.rsplit("wall ", 1)[1].rstrip("s\n"))
    return None


def stop_of(transcript):
    for line in open(f"{HERE}/{transcript}"):
        if line.startswith("harvest: policy "):
            return line.split("stop=", 1)[1].split(" ", 1)[0]
    return None


def load_dump(path):
    """full UCI path -> (outcome, depth_bound) from a canonical dump (TSV).

    The dump stores one move per node plus parent_id links; paths are
    rebuilt by walking the parent chain (root has parent_id '').
    """
    parent = {}
    move = {}
    rows = {}
    with open(path) as f:
        for line in f:
            if line.startswith("#") or line.startswith("id\t"):
                continue
            f_ = line.rstrip("\n").split("\t")
            rid = f_[0]
            parent[rid] = f_[1]
            move[rid] = f_[3]
            rows[rid] = (f_[5], f_[6])

    def full_path(rid):
        chain = []
        while move[rid]:
            chain.append(move[rid])
            rid = parent[rid]
        return " ".join(reversed(chain))

    return {full_path(rid): rows[rid] for rid in rows}


def main():
    # ---------- per-policy census ----------
    summaries = {}
    for policy, transcript in POLICIES.items():
        recs = job_records(transcript)
        s = summarize(policy, recs)
        s["stop"] = stop_of(transcript)
        s["wall_s"] = wall_of(transcript)
        summaries[policy] = s
        with open(f"{HERE}/census_{policy}.json", "w") as f:
            json.dump(s, f, indent=2)
        print(f"census_{policy}.json: {s['screen_jobs']} screen jobs, "
              f"{s['new_facts']} facts, {s['screen_evals']/1e6:.1f}M screen evals, "
              f"{s['heavy_jobs']} heavy jobs, wall {s['wall_s']}s, stop={s['stop']}")

    # ---------- H3: no-regression vs the input DB ----------
    before = load_dump(f"{HERE}/nodes_input.txt")
    after = load_dump(f"{HERE}/nodes_grown2.txt")
    reg = []
    for p, (o, b) in before.items():
        if o not in ("win", "loss"):
            continue
        if p not in after:
            reg.append(("vanished", p))
            continue
        ao, ab = after[p]
        if ao != o:
            reg.append(("outcome", p, o, ao))
        elif b != "" and ab != "" and int(ab) > int(b):
            reg.append(("bound", p, b, ab))
    n_proven = sum(1 for v in before.values() if v[0] in ("win", "loss"))
    print(f"H3: {len(before)} input nodes checked ({n_proven} proven), "
          f"{len(after)} grown; regressions: {len(reg)}")
    for r in reg[:5]:
        print("  REGRESSION", r)
    h3_ok = not reg

    # ---------- H4: new shard roots: DB row outcome == manifest outcome ----
    manifest = json.load(open(MANIFEST))["entries"]
    new_tags = set()
    for transcript in POLICIES.values():
        for r in job_records(transcript):
            if r.get("tag"):
                new_tags.add(r["tag"])
    mism = 0
    checked = 0
    for e in manifest:
        if e["tag"] not in new_tags:
            continue
        o, _b = after[" ".join(e["moves"])]
        if o != e["outcome"]:
            mism += 1
            print(f"  H4 MISMATCH {e['tag']}: db {o} vs manifest {e['outcome']}")
        checked += 1
    print(f"H4: {checked} new shard roots cross-checked against DB rows, "
          f"{mism} mismatches ({len(new_tags)} new tags seen)")
    h4_ok = mism == 0 and checked == len(new_tags)

    # ---------- policy comparison (pre-registered metric) ----------
    comp = {
        "input_db_nodes": len(before),
        "grown2_db_nodes": len(after),
        "screen_budget_evals_each": 300_000_000,
        "winner_rule": "highest new-facts-per-100M-evals; ties: lower median cost/fact; "
        "ties: keep open-deepest (status quo bias)",
        "policies": {
            k: {kk: vv for kk, vv in v.items() if kk != "records"}
            for k, v in summaries.items()
        },
    }
    scores = {k: v["facts_per_100m"] for k, v in summaries.items()}
    comp["winner"] = max(scores, key=lambda k: scores[k]) if max(scores.values()) > 0 \
        else "open-deepest"
    with open(f"{HERE}/policy_comparison.json", "w") as f:
        json.dump(comp, f, indent=2)
    print(f"comparison: {json.dumps(scores)}; winner={comp['winner']}")

    # sample lines from the grown2 merge log
    with open(f"{HERE}/sample_lines.txt", "w") as f:
        for line in open(f"{HERE}/merge_grown2_a.log"):
            if line.startswith("sample "):
                f.write(line)
    print("sample_lines.txt written")

    if not (h3_ok and h4_ok):
        sys.exit(1)


if __name__ == "__main__":
    main()
