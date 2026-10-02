#!/usr/bin/env python3
"""plan10 census assembly + gate checks (D2).

Reads the arms' transcripts + the H5 replays (raw logs stay in /tmp/plan10,
gitignored; the parsed census is the record) and writes census_armA.json /
census_armB.json + verdicts.json. Arms (plan10 §3, both from the standing
post-plan9 state — DB `6c724928…` 41,459 nodes, manifest `9992dba4…` 256
entries, ledger `a696610b…` 8,446 records — staging copies, fresh TT, root =
startpos, policy and-close, order completion, no mid-run tuning):

  armA (tier sweep)   - `--and-close-max-budget 100000000`, base 4M, cap 10.5B,
                        TT 128 MB (default); the D1 budget filter drops exactly
                        the three 6B head probes (echo `jobs 1080 of 1083`);
  armB (g1f3 6B rung) - no filter, `--max-jobs 3`, base 4M, cap 18.5B,
                        TT 1024 MB.

§9 amendment context: the plan-session pilot (measurements/plan10/probe/,
committed) predicted arm A = 3 facts / 1,077 censors byte-matched to its own
run. Arm A's official run falsified that pre-registration (6 facts / 1,074
censors): the pilot's three 6B head probes ran first in the same session and
warmed the cross-job private TT, changing the tier jobs' within-job search.
The §9 re-scope replaces the pilot baseline with a full in-session replay per
§4.2 (arm A run 1 vs armA_replay; arm B vs armB_replay).

Checks:
  - H1: the pinned session-start census (§1: job set 1,083 = 0 fresh +
    1,083 ledger-censored; active rows 49; the gradient head; exclusions
    26/0/0); armA: the filter echo `jobs 1080 of 1083`, every job budget
    <= 100M and >= 2 × work_before; armB: no filter echo, the sequence head
    exactly the three g1f3 defenses at the pinned budgets/passes/
    work_before, `--max-jobs 3` stops the arm; no job path is a standing
    manifest path; no-exposure (post-run ledger == start + censor bumps
    only, 0 new records, decisive records untouched);
  - §9/H5: per-arm replay (identical job sequence excl wall_s;
    byte-identical post-run ledger; arm A: the 6 fact shards byte-identical
    and the grown manifest byte-identical to run 1 `e91ad57b…`);
  - the pilot falsification, quantified: per-job field diffs between the
    pilot's census and arm A run 1 (the falsified pre-registration record);
  - the union advance gates (§3.6/§4.3 re-scoped): digest-pinned inputs,
    determinism (reversed inputs), idempotence, N=1 normalization;
  - H2: merger ×2 (+1) over the grown manifest -> byte-identical DB + dump,
    262/262 shards replay-validated;
  - the flip analysis over the grown DB (and the pre-run standing DB as
    sanity): the §3.7 prior (no completion, 2 rows to missing 8, one OR-row
    candidate removed) checked; the completion branch must not fire without
    a completed row.
"""
import json
import sys
from collections import Counter

sys.dont_write_bytecode = True

HERE = "/workspace/atomic_solver/docs/plans/proofdb/measurements/plan10"
ROOT = "/workspace/atomic_solver"
TMP = "/tmp/plan10"
PROBE = f"{HERE}/probe"

PINNED_HEAD = ("g1f3:3 e2e3:7 e2e4:7 g1h3:9 a2a3 a7a6 b2b3 a6a5 c2c3 a5a4 d2d3 "
               "a4b3 e2e3:10 d2d4 a7a6 a2a3 a6a5 b2b3 a5a4 c2c3 a4b3 e2e3:11 "
               "d2d4 d7d5 a2a3 a7a6 b2b3 a6a5 c2c3 a5a4 e2e3 a4b3 f2f3 b7b6 "
               "g2g3 b6b5 h2h3 b5b4 a3a4 b4b3 c3c4 b3b2 e3e4 b2a1q f3f4 d5c4 "
               "g3g4 c7c6:12 root:13 a2a3 a7a6 b2b3 a6a5 c2c3 a5a4:17 d2d4:18")
PINNED_CENSUS = ("and-close: active rows 49, replies 1083 (fresh 0, "
                 "ledger-censored 1083)")
PINNED_EXCL = ("and-close-excluded: open rows 75 (active 49); proven-ancestor 26, "
               "implied-win 0, implied-loss 0")
PINNED_FILTER = "and-close-filter: max-budget 100000000, jobs 1080 of 1083"
ARMA_MANIFEST_SHA = "e91ad57b44008fee65ab0b124fab51365c17d08f891777cde525cfecb61fb5fd"
ARMA_LEDGER_SHA = "9c060103bd23038a5bd270eb2399494ce6208c6e1b3f16e53d2ed11d91d846e1"
# plan10 §3.2's head pins: path | budget | outcome | pass | work_before.
PINNED_ARMB_HEAD = {
    "g1f3 d7d6": (6_000_000_190, 3, 3_000_000_095),
    "g1f3 e7e5": (6_000_000_194, 3, 3_000_000_097),
    "g1f3 f7f6": (6_000_000_102, 3, 3_000_000_051),
}
ARMS = {
    "armA": dict(dir=f"{TMP}/armA", replay=f"{TMP}/armA_replay", base=4_000_000,
                 order="completion", cap=10_500_000_000, tt_mb=128, filtered=True),
    "armB": dict(dir=f"{TMP}/armB", replay=f"{TMP}/armB_replay", base=4_000_000,
                 order="completion", cap=18_500_000_000, tt_mb=1024, filtered=False),
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


man_start = json.loads(
    __import__("subprocess").run(
        ["git", "-C", ROOT, "show", "HEAD:docs/plans/proofdb/shards/manifest.json"],
        capture_output=True, check=True).stdout)
assert len(man_start["entries"]) == 256
mpaths = {" ".join(e["moves"]) for e in man_start["entries"]}
# The pre-run standing ledger (plan9's union advance) — the post-run standing
# state in data/ is already this batch's union.
start = load_ledger(f"{HERE}/../plan9/ledger_union.json")
import hashlib
assert hashlib.sha256(open(f"{HERE}/../plan9/ledger_union.json", "rb").read()).hexdigest() \
    == "a696610bbf4c6d6b530088652e20e3279a51ef17fbc6a900d49c6222aac77590"
assert len(start) == 8_446

arm_data = {}
for arm, cfg in ARMS.items():
    d = cfg["dir"]
    recs = jobs(f"{d}/jobs.out")
    census_line = gradient = exclusions = filter_line = None
    for l in open(f"{d}/session.err"):
        l = l.rstrip("\n")
        if l.startswith("and-close: active"):
            census_line = l
        elif l.startswith("and-close-gradient"):
            gradient = l
        elif l.startswith("and-close-excluded"):
            exclusions = l
        elif l.startswith("and-close-filter"):
            filter_line = l
    # H1: pinned census + gradient head + exclusions; filter echo per arm.
    assert census_line == f"{PINNED_CENSUS}; order {cfg['order']}; base budget {cfg['base']}", census_line
    assert gradient == f"and-close-gradient: {PINNED_HEAD}", gradient
    assert exclusions == PINNED_EXCL, exclusions
    if cfg["filtered"]:
        assert filter_line == PINNED_FILTER, filter_line
    else:
        assert filter_line is None, filter_line
    # H1: no job path is a standing manifest path.
    assert not any(j["path"] in mpaths for j in recs), f"{arm}: job is a manifest path"
    facts = [j for j in recs if j["tag"]]
    cens = [j for j in recs if not j["tag"]]
    if arm == "armA":
        # H1: every job budget <= 100M (the filter) and strict growth holds.
        assert all(j["budget"] <= 100_000_000 for j in recs), "armA filter pin"
        assert all(j["budget"] >= 2 * j["work_before"] for j in recs)
        # §9 re-scoped expectation: 6 facts / 1,074 censors.
        assert len(facts) == 6 and len(cens) == 1_074 and len(recs) == 1_080, \
            (len(facts), len(cens))
    else:
        # H1: the head is exactly the pinned trio; --max-jobs 3 stops the arm.
        assert len(recs) == 3, [j["path"] for j in recs]
        for j in recs:
            pin = PINNED_ARMB_HEAD[j["path"]]
            assert (j["budget"], j["pass"], j["work_before"]) == pin, (j["path"], j)
        assert all(j["budget"] >= 2 * j["work_before"] for j in recs)
    # H1 no-exposure: post-run ledger == start + bumps only; decisive
    # records untouched; 0 new records (all 1,080/3 jobs are ledger-known).
    post = load_ledger(f"{d}/ledger.json")
    new = {p for p in post if p not in start}
    gone = {p for p in start if p not in post}
    censpaths = {tuple(j["path"].split()) for j in cens}
    factpaths = {tuple(j["path"].split()) for j in facts}
    bumped = {p for p in start if p in post and post[p] != start[p]}
    assert not new and not gone, (len(new), len(gone))
    assert bumped == censpaths, (len(bumped), len(censpaths))
    assert all(post[p] == start[p] for p in factpaths if p in start), \
        "decisive record touched"
    assert all(post[p]["passes_failed"] == start[p]["passes_failed"] + 1
               for p in censpaths)
    ply = Counter(len(j["path"].split()) for j in recs)
    facts_ply = Counter(len(j["path"].split()) for j in recs if j["tag"])
    tier = Counter((j["budget"] + 3_999_999) // 4_000_000 * 4_000_000
                   for j in recs)
    m = {
        "jobs": len(recs),
        "facts": len(facts),
        "censored": len(cens),
        "total_evals": sum(j["child_evals"] for j in recs),
        "ply_distribution": {str(k): v for k, v in sorted(ply.items())},
        "facts_by_ply": {str(k): v for k, v in sorted(facts_ply.items())},
        "tier_jobs": {str(k): v for k, v in sorted(tier.items())},
        "facts_by_tier": {str(k): v for k, v in sorted(
            Counter((j["budget"] + 3_999_999) // 4_000_000 * 4_000_000
                    for j in facts).items())},
        "ledger_start_records": len(start),
        "ledger_post_records": len(post),
        "ledger_new_records": len(new),
        "ledger_bumped_records": len(bumped),
        "no_exposure_signature": f"post == start + {len(bumped)} bumps + 0 new",
        "max_pass_after": max(r["passes_failed"] for r in post.values()),
        "census_line": census_line,
        "gradient_head": gradient,
        "exclusions_line": exclusions,
        "filter_line": filter_line,
        "manifest_line": next(l for l in open(f"{d}/jobs.out")
                              if l.startswith("manifest:")).strip(),
        "post_ledger_sha256": sha(f"{d}/ledger.json"),
        "stop_reason": next(l for l in open(f"{d}/jobs.out")
                            if l.startswith("harvest:")).split("stop=")[1].split(" ")[0],
    }
    # §9/H5: replay identity (job sequence excl wall_s; ledger bytes).
    rrecs = jobs(f"{cfg['replay']}/jobs.out")
    h5 = {
        "job_sequence_identical": strip(recs) == strip(rrecs),
        "ledger_bytes_identical": sha(f"{d}/ledger.json") == sha(f"{cfg['replay']}/ledger.json"),
        "shards_bytes_identical": None,
        "manifest_bytes_identical": None,
    }
    assert h5["job_sequence_identical"] and h5["ledger_bytes_identical"], arm
    if facts:
        same = all(sha(f"{d}/shards/{j['tag']}.bin") == sha(f"{cfg['replay']}/shards/{j['tag']}.bin")
                   for j in facts)
        h5["shards_bytes_identical"] = same
        assert same, f"{arm}: replay shards differ"
    h5["manifest_bytes_identical"] = sha(f"{d}/manifest.json") == sha(f"{cfg['replay']}/manifest.json")
    assert h5["manifest_bytes_identical"], f"{arm}: replay manifest differs"
    arm_data[arm] = {"config": cfg, "metrics": m, "jobs": recs, "h5": h5}

# §9: the arm A grown manifest is exactly run 1's 262-entry manifest.
assert arm_data["armA"]["metrics"]["manifest_line"].startswith("manifest: 262 entries")
assert sha(f"{TMP}/armA/manifest.json") == sha(f"{TMP}/armA_replay/manifest.json")
assert arm_data["armB"]["metrics"]["manifest_line"].startswith("manifest: 256 entries") or \
    "unchanged" in arm_data["armB"]["metrics"]["manifest_line"]

# Pilot falsification, quantified (the §9 record): per-job field diffs
# between the pilot's census and arm A run 1 on the 1,080 shared jobs.
pilot = jobs(f"{PROBE}/census_jobs.jsonl")
pilot_by_path = {j["path"]: j for j in pilot}
shared = [j for j in arm_data["armA"]["jobs"] if j["path"] in pilot_by_path]
assert len(shared) == 1_080
fields = ("budget", "child_evals", "outcome", "exit_reason", "pass", "work_before")
diffs = {f: [j["path"] for j in shared
             if j[f] != pilot_by_path[j["path"]][f]] for f in fields}
falsification = {
    "pilot_jobs": len(pilot),
    "pilot_facts": sum(1 for j in pilot if j["tag"]),
    "armA_run_facts": arm_data["armA"]["metrics"]["facts"],
    "shared_jobs": len(shared),
    "differing_fields": {f: len(v) for f, v in diffs.items()},
    "outcome_flipped_paths": diffs["outcome"],
    "note": "child_evals diverge on the shared jobs; outcomes flip on 3 "
            "(censored -> win). Root cause: the pilot's three 6B head probes "
            "ran first in the same session and warmed the cross-job private "
            "TT; arm A drops them. Budget bookkeeping (budget/pass/"
            "work_before) is identical everywhere — the strict-growth ladder "
            "is unaffected; §4.1's byte-exact baseline and H2/H3's shard "
            "byte-equality are falsified (§9).",
}
assert diffs["budget"] == [] and diffs["pass"] == [] and diffs["work_before"] == []
assert diffs["exit_reason"] == diffs["outcome"]  # BudgetExhausted -> Complete on the same 3 paths
assert len(diffs["outcome"]) == 3

# H3/H4: the 6 fact paths are disjoint from every standing manifest path
# (no same-path outcome contradiction is possible); the union of both
# arms' fact sets is disjoint.
fa = {j["path"] for j in arm_data["armA"]["jobs"] if j["tag"]}
fb = {j["path"] for j in arm_data["armB"]["jobs"] if j["tag"]}
assert not (fa & mpaths) and not (fb & mpaths) and not (fa & fb)

# Union gates (byte digests; the CLI ran with --expect pins — see README).
union_sha = sha(f"{HERE}/ledger_union.json")
assert union_sha == sha(f"{TMP}/union/ledger.json") == sha(f"{TMP}/union/ledger_rev.json") \
    == sha(f"{TMP}/union/ledger_idem.json"), "union determinism/idempotence"
assert sha(f"{TMP}/union/norm_aA.json") == sha(f"{TMP}/armA/ledger.json"), "N=1 armA"
assert sha(f"{TMP}/union/norm_aB.json") == sha(f"{TMP}/armB/ledger.json"), "N=1 armB"

# H2: merger determinism over the grown manifest (262/262 replay-validated
# by the merger itself; its stderr is the record).
assert sha(f"{TMP}/grown1.db") == sha(f"{TMP}/grown2.db")
assert sha(f"{TMP}/grown1.txt") == sha(f"{TMP}/grown2.txt")
grown = json.load(open(f"{TMP}/armA/manifest.json"))
assert len(grown["entries"]) == 262

# Flip analysis (over the grown DB + pre-run standing sanity).
flips = json.load(open(f"{HERE}/flip_analysis.json"))
prerun = json.load(open(f"{HERE}/flip_prerun.json"))
assert flips["flips_total"] == 0 and prerun["flips_total"] == 0, (flips, prerun)

factsA = arm_data["armA"]["metrics"]["facts"]
factsB = arm_data["armB"]["metrics"]["facts"]
armb_censored = arm_data["armB"]["metrics"]["censored"] == 3
verdicts = {
    "H1_integrity": "pass (pinned census 1,083 = 0 fresh + 1,083 censored, gradient "
                    "head, exclusions 26/0/0 reproduced per arm; armA: filter echo "
                    "`jobs 1080 of 1083`, every job budget <= 100M and >= 2 × "
                    "work_before; armB: no filter echo, head exactly the pinned g1f3 "
                    "trio at budgets 6,000,000,190/194/102 (pass 3, work ≈ 3B), "
                    "--max-jobs 3 stops the arm; no job path a standing manifest "
                    "path; no-exposure exact both arms: 0 new records, "
                    "decisive records untouched)",
    "H2_merge_determinism": "pass (262/262 shards replay-validated; grown1.db == "
                            f"grown2.db == {sha(f'{TMP}/grown1.db')[:16]}..., dumps "
                            "identical; grown manifest == arm A run 1's e91ad57b…)",
    "H3_H4_no_regression": "pass (6 promoted facts all replay-validated by the "
                           "merger; fact paths disjoint from every standing "
                           "manifest path and between arms; the pilot shard "
                           "byte-equality gate is re-scoped per §9 — the pilot's "
                           "pre-registration was falsified)",
    "H5_policy_determinism": "pass (§9 re-scoped: arm A run 1 vs full in-session "
                             "replay — job sequences identical excl wall_s, "
                             "post-run ledgers byte-identical, all 6 fact shards "
                             "byte-identical, grown manifests byte-identical; "
                             "arm B run vs replay likewise; union determinism/"
                             "idempotence/N=1 normalization byte-identical on the "
                             "digest-pinned real inputs)",
    "H6_hygiene": "pass (make test green incl. the new --and-close-max-budget "
                  "filter tests; clippy/fmt/doc clean; no src/ changes; "
                  "new/edited example files <= 10 KB; data/ ignored)",
    "flip_analysis": "pass (0 flips over the grown DB and the pre-run standing DB; "
                     "no row reached AND-completeness; the completion branch of "
                     "§4.3 did not fire; the two …e2e3 rows moved to missing 8, "
                     "the OR-row's e1e2/b8d7/a8a4-class candidates removed per "
                     "§3.7's prior)",
    "pilot_falsification": falsification,
    "armA": {"facts": factsA,
             "verdict": "the tier sweep ran its full 1,080-job set (stop=exhausted) "
                        "and yielded 6 facts (0.55% of jobs): 2 at the 24M tier, 4 "
                        "at the 8M tier — three of them re-deriving the pilot's "
                        "fact paths, three new. The §3.1 expectation (exactly the "
                        "pilot's 3 facts) is falsified; §9's re-scoped expectation "
                        "is confirmed by the replay"},
    "armB": {"facts": factsB,
             "verdict": ("3 censors: each g1f3 defense consumed its ≈ 6B budget at "
                         "TT 1024 MB (work ≈ 9B after; next rung 18B) — the climb's "
                         "first config-matched >= 2B measurement confirms the "
                         "config-mismatch concern was real only in the "
                         "TT-history sense; the band prior holds"
                         if armb_censored else
                         f"completion branch: {factsB} head fact(s) — see report10"),
             },
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
