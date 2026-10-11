#!/usr/bin/env python3
"""Run the plan17 D3(e) adopted-claim re-verification sub-study.

    python3 verify.py [RAW_DIR]

Collects the sampled adopted claims from $RAW/samples/*.jsonl (written by
the hook during the rollout), joins the root FEN per case, and invokes the
temporary verifier binary (`examples/plan17_verify`, which replays each
claim's UCI path from the root FEN and re-solves at `max_depth = d` with the
hook OFF — the verifier process runs with ATOMIC_CLOCKREUSE unset) for each
candidate arm separately.

Writes state/verification.json and exits 1 on any disagreement (soundness
defect -> HALT per plan17 D3(e)) or on any failed claim verification.
"""

import json
import os
import subprocess
import sys

sys.dont_write_bytecode = True

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "../../../../.."))
RAW = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("RAW", "/tmp/plan17/results")
BIN = os.environ.get("VERIFY_BIN", os.path.join(REPO, "target/release/examples/plan17_verify"))
OUT = os.path.join(HERE, "state")

# Minimum verified claims per arm (plan17 D3(e): >= 100).
MIN_CLAIMS = 100


def main():
    with open(os.path.join(OUT, "summary.json")) as f:
        claims = json.load(f)["claims"]
    if not claims:
        raise SystemExit("no claims collected; cannot run the sub-study")

    results = {}
    failures = 0
    for arm in ("plan10", "arma"):
        arm_claims = [c for c in claims if c["arm"] == arm]
        if not arm_claims:
            results[arm] = {"n_claims": 0, "verified": 0, "failed": 0, "ok": False}
            failures += 1
            continue
        payload = json.dumps({"claims": arm_claims})
        path = os.path.join(RAW, f"verify_{arm}.json")
        with open(path, "w") as f:
            f.write(payload)
        env = dict(os.environ)
        env.pop("ATOMIC_CLOCKREUSE", None)  # verifier runs with the hook OFF
        env.pop("ATOMIC_CLOCKREUSE_SAMPLE", None)
        p = subprocess.run([BIN, path], capture_output=True, text=True, env=env)
        try:
            summary = json.loads(p.stdout.strip().splitlines()[-1])
        except (ValueError, IndexError):
            summary = {"error": "no verifier output", "rc": p.returncode}
        verified = summary.get("verified", 0)
        failed = summary.get("failed", summary.get("claims", 0) - verified)
        ok = p.returncode == 0 and failed == 0 and verified >= MIN_CLAIMS
        results[arm] = {
            "n_claims": len(arm_claims), "verified": verified, "failed": failed,
            "ok": ok, "verifier": summary,
            "stderr_tail": p.stderr.strip().splitlines()[-5:],
        }
        if not ok:
            failures += 1
        print(f"arm {arm}: claims={len(arm_claims)} verified={verified} "
              f"failed={failed} ok={ok}")

    with open(os.path.join(OUT, "verification.json"), "w") as f:
        json.dump(results, f, indent=1)
    if failures:
        print(f"D3(e) FAILED for {failures} arm(s) — HALT")
        sys.exit(1)
    print("OK: D3(e) adopted-claim re-verification passed "
          f"(>={MIN_CLAIMS} claims per arm, zero disagreements)")


if __name__ == "__main__":
    main()
