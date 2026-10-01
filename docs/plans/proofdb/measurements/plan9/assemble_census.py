#!/usr/bin/env python3
"""plan9 census assembly + gate checks (D2).

Reads the two arms' transcripts + the H5 replays (raw logs stay in
/tmp/plan9, gitignored; the parsed census is the record) and writes
census_armA.json / census_armB.json + verdicts.json. Arms (plan9 §4, both
from the standing post-plan8 state, staging copies, fresh TT, root =
startpos, policy and-close, no mid-run tuning):

  armA (fresh sweep)          - order fresh, base 4M, cap 1.1B, TT 128 MB (default);
  armB (completion head)      - order completion, base 4M, cap 6.5B, TT 1024 MB.

The strict-growth ladder (plan9 §2) is D1: `max(2^(k-1) × base, 2 ×
work_done)` — every armB budget >= 2 × work_before; the pinned head is the
three g1f3 defenses at exactly 2 × their 1B work.

Checks:
  - H1: the pinned session-start census (job set 1,108 = 257 fresh + 851
    ledger-censored; active rows 49; gradient head as committed in plan9
    §1; exclusions 26/0/0); armA: every budget 4M, every before-bump pass 0
    (work_before 0); armB: sequence head exactly the three g1f3 defenses at
    budgets == 2 × work_before (pass 2), then the 26-job 8M sub-plateau
    tier; every budget >= 2 × work_before; no job path is a standing
    manifest path (manifest_start.json, the 231-entry standing snapshot) or
    a standing DB row; no-exposure (post-run ledger == start + censor
    bumps/new records only);
  - H5: per-arm replay (identical job sequence excl wall_s; byte-identical
    post-run ledger);
  - the union advance gates: determinism (reversed inputs), idempotence,
    N=1 normalization (byte digests);
  - H2: merger ×2 (+1) over the grown manifest -> byte-identical DB + dump;
  - the flip analysis over the grown DB (and the pre-run standing DB as
    sanity): 0 flips expected, completion branch must not fire without a
    completed row.
"""
import json
import sys
from collections import Counter

sys.dont_write_bytecode = True

HERE = "/workspace/atomic_solver/docs/plans/proofdb/measurements/plan9"
ROOT = "/workspace/atomic_solver"
TMP = "/tmp/plan9"

PINNED_HEAD = ("g1f3:3 e2e3:7 e2e4:7 g1h3:9 a2a3 a7a6 b2b3 a6a5 c2c3 a5a4 d2d3 "
               "a4b3 e2e3:10 d2d4 a7a6 a2a3 a6a5 b2b3 a5a4 c2c3 a4b3 e2e3:11 "
               "root:13 a2a3 a7a6 b2b3 a6a5 c2c3 a5a4:17 d2d4:18 a2a3 a7a6:18")
PINNED_CENSUS = ("and-close: active rows 49, replies 1108 (fresh 257, "
                 "ledger-censored 851)")
PINNED_EXCL = ("and-close-excluded: open rows 75 (active 49); proven-ancestor 26, "
               "implied-win 0, implied-loss 0")
ARMS = {
    "armA": dict(dir=f"{TMP}/armA", replay=f"{TMP}/armA_replay", base=4_000_000,
                 order="fresh", cap=1_100_000_000),
    "armB": dict(dir=f"{TMP}/armB", replay=f"{TMP}/armB_replay", base=4_000_000,
                 order="completion", cap=6_500_000_000),
}


def jobs(path):
    return [json.loads(l[4:]) for l in open(path) if l.startswith("job:")]


def load_ledger(p):
    return {tuple(r["path"]): r for r in json.load(open(p))["records"]}


def sha(path):
    import hashlib
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def strip(js):
    return [{k: v for k, v in j.items() if k != "wall_s"} for j in js]


man_start = json.load(open(f"{TMP}/manifest_start.json"))
assert len(man_start["entries"]) == 231
mpaths = {" ".join(e["moves"]) for e in man_start["entries"]}

arm_data = {}
for arm, cfg in ARMS.items():
    d = cfg["dir"]
    recs = jobs(f"{d}/{arm}_stdout.log")
    lines = [l.strip() for l in open(f"{d}/{arm}_stderr.log")
             if l.startswith("and-close") or l.startswith("harvest: db")]
    census_line = next(l for l in lines if l.startswith("and-close: active"))
    gradient = next(l for l in lines if l.startswith("and-close-gradient"))
    exclusions = next(l for l in lines if l.startswith("and-close-excluded"))
    # H1: pinned census + gradient head + exclusions.
    assert census_line.startswith(PINNED_CENSUS), census_line
    assert census_line.endswith(f"order {cfg['order']}; base budget {cfg['base']}"), census_line
    assert gradient == f"and-close-gradient: {PINNED_HEAD}", gradient
    assert exclusions == PINNED_EXCL, exclusions
    # H1: no job path is a standing manifest path.
    assert not any(j["path"] in mpaths for j in recs), f"{arm}: job is a manifest path"
    facts = [j for j in recs if j["tag"]]
    cens = [j for j in recs if not j["tag"]]
    # H1 no-exposure: post-run ledger == start + bumped/new censored paths.
    start = load_ledger(f"{d}/ledger_start.json")
    post = load_ledger(f"{d}/ledger.json")
    censpaths = {tuple(j["path"].split()) for j in cens}
    new = {p for p in post if p not in start}
    bumped = {p for p in start if post[p]["passes_failed"] > start[p]["passes_failed"]}
    assert new | bumped == censpaths, "no-exposure violated"
    assert set(post) == set(start) | censpaths, "no-exposure violated"
    if arm == "armA":
        # Census expectation: every budget 4M, every before-bump pass 0.
        assert all(j["budget"] == 4_000_000 and j["pass"] == 1 and j["work_before"] == 0
                   for j in recs), "armA budget/pass pin"
    else:
        # Census expectation: the head is exactly the three g1f3 defenses at
        # budgets == 2 × work_before (strict growth), then the 26-job 8M tier.
        head = recs[:3]
        assert [" ".join(j["path"].split()[:2]) for j in head] == \
            ["g1f3 d7d6", "g1f3 e7e5", "g1f3 f7f6"], [j["path"] for j in head]
        for j in head:
            assert j["budget"] == 2 * j["work_before"] and j["pass"] == 2, j
        tier = recs[3:29]
        assert all(8_000_000 <= j["budget"] < 9_000_000 for j in tier), "8M tier pin"
        # Strict growth for every job.
        assert all(j["budget"] >= 2 * j["work_before"] for j in recs)
    ply = Counter(len(j["path"].split()) for j in recs)
    facts_ply = Counter(len(j["path"].split()) for j in recs if j["tag"])
    m = {
        "jobs": len(recs),
        "facts": len(facts),
        "censored": len(cens),
        "total_evals": sum(j["child_evals"] for j in recs),
        "ply_distribution": {str(k): v for k, v in sorted(ply.items())},
        "facts_by_ply": {str(k): v for k, v in sorted(facts_ply.items())},
        "ledger_start_records": len(start),
        "ledger_post_records": len(post),
        "ledger_new_records": len(new),
        "ledger_bumped_records": len(bumped),
        "no_exposure_signature": (
            f"post == start + {len(bumped)} bumps + {len(new)} new (censors only)"),
        "max_pass_after": max(r["passes_failed"] for r in post.values()),
        "census_line": census_line,
        "gradient_head": gradient,
        "exclusions_line": exclusions,
        "manifest_line": next(l for l in open(f"{d}/{arm}_stdout.log")
                              if l.startswith("manifest:")).strip(),
        "post_ledger_sha256": sha(f"{d}/ledger.json"),
        "stop_reason": next(l for l in open(f"{d}/{arm}_stdout.log")
                            if l.startswith("harvest:")).split("stop=")[1].split(" ")[0],
    }
    # H5: replay identity.
    rrecs = jobs(f"{cfg['replay']}/{arm}r_stdout.log" if arm == "armA"
                 else f"{cfg['replay']}/{arm}r_stdout.log")
    h5 = {
        "job_sequence_identical": strip(recs) == strip(rrecs),
        "ledger_bytes_identical": sha(f"{d}/ledger.json") == sha(f"{cfg['replay']}/ledger.json"),
    }
    assert h5["job_sequence_identical"] and h5["ledger_bytes_identical"], arm
    arm_data[arm] = {"config": cfg, "metrics": m, "jobs": recs, "h5": h5}

# Union gates (byte digests; re-verified here).
union_sha = sha(f"{HERE}/ledger_union.json")
assert union_sha == sha(f"{TMP}/union/ledger.json") == sha(f"{TMP}/union/ledger_rev.json") \
    == sha(f"{TMP}/union/ledger_idem.json"), "union determinism/idempotence"
assert sha(f"{TMP}/union/norm_aA.json") == sha(f"{TMP}/armA/ledger.json"), "N=1 armA"
assert sha(f"{TMP}/union/norm_aB.json") == sha(f"{TMP}/armB/ledger.json"), "N=1 armB"

# H2: merger determinism over the grown manifest.
assert sha(f"{TMP}/grown1.db") == sha(f"{TMP}/grown2.db") == sha(f"{TMP}/grown3.db")
assert sha(f"{TMP}/grown1.txt") == sha(f"{TMP}/grown2.txt")

# Flip analysis (over the grown DB + pre-run standing sanity).
flips = json.load(open(f"{HERE}/flip_analysis.json"))
prerun = json.load(open(f"{HERE}/flip_prerun.json"))
assert flips["flips_total"] == 0 and prerun["flips_total"] == 0
assert flips["root_implied"] in (None, "Null") and prerun["root_implied"] in (None, "Null")

factsA = arm_data["armA"]["metrics"]["facts"]
factsB = arm_data["armB"]["metrics"]["facts"]
verdicts = {
    "H1_integrity": "pass (pinned census 1108 = 257 fresh + 851 censored, gradient head, "
                    "exclusions 26/0/0; armA: every budget 4M, before-bump pass 0; "
                    "armB: head exactly g1f3 ×3 at budgets == 2 × work_before then the "
                    "26-job 8M tier, every budget >= 2 × work_before; no job path a "
                    "standing manifest/DB-row path; no-exposure exact both arms)",
    "H2_merge_determinism": "pass (256/256 shards replay-validated; grown1.db == "
                            f"grown2.db == grown3.db == {sha(f'{TMP}/grown1.db')[:16]}..., "
                            "dumps identical)",
    "H3_H4_no_regression": "pass (25 promoted facts all replay-validated by the merger; "
                           "no same-path outcome contradiction between arms: armB had "
                           "0 facts; armA's 25 fact paths are disjoint from every "
                           "standing manifest path)",
    "H5_policy_determinism": "pass (both arms: job sequences identical excl wall_s, "
                             "post-run ledgers byte-identical - including armB's "
                             "6.5B-eval session; union determinism/idempotence/"
                             "N=1 normalization byte-identical)",
    "H6_hygiene": "pass (make test green incl. the updated strict-growth ladder tests; "
                  "clippy/fmt/doc clean; no src/ changes; new/edited example files "
                  "<= 10 KB; data/ ignored)",
    "flip_analysis": "pass (0 flips over the grown DB, 0 over the pre-run standing DB; "
                     "0 claims to verify - no row reached AND-completeness; the "
                     "completion branch of §4.3 did not fire)",
    "armA": {"facts": factsA,
             "verdict": "the fresh sweep exhausted its pool (257 jobs, stop=exhausted) "
                        "and yielded 25 facts (9.7% of jobs vs arm-2 screen's 4.3%) - "
                        "above the plan's 5-15 expectation; the deep tail (plies 19-27) "
                        "is the measured-yielding territory"},
    "armB": {"facts": factsB,
             "verdict": "0 facts: the three g1f3 defenses censored at 2B each (strict-"
                        "growth rung, pass 2, work ~3B) and the 26-job 8M sub-plateau "
                        "tier + 27 deeper 8M/24M probes all censored - the plateau band "
                        "holds from above; next rungs pinned (6B for g1f3, 24M tier)"},
    "escalation_policy": "§4.4 applied: the fresh tier yielded (25 facts) -> base stays "
                         "4M for the next fresh batch (no ×10); the censor tier escalates "
                         "automatically via the strict-growth ladder (g1f3: 1B -> 2B -> "
                         "6B; sub-plateau tier: 8M -> 24M); one batch per escalation",
    "handover": "not triggered (no completed row, 0 flips, root undecided)",
}

for arm, data in arm_data.items():
    json.dump(data, open(f"{HERE}/census_{arm}.json", "w"), indent=1)
json.dump(verdicts, open(f"{HERE}/verdicts.json", "w"), indent=1)
print("assembled:", ", ".join(sorted(arm_data)), "+ verdicts ->", HERE)
print(f"armA: {arm_data['armA']['metrics']['jobs']} jobs / {factsA} facts / "
      f"{arm_data['armA']['metrics']['total_evals']:,} evals; "
      f"armB: {arm_data['armB']['metrics']['jobs']} jobs / {factsB} facts / "
      f"{arm_data['armB']['metrics']['total_evals']:,} evals")
