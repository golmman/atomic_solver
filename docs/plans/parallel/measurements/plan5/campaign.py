#!/usr/bin/env python3
"""plan5b campaign: interleaved N=1/2/3/4 wall runs on m22 + shuffle-win.

Per case, `rounds` rounds; each round runs N in 1,2,3,4 order (N=1 is the
in-session reference; interleaving defeats session drift, plan4 protocol).
Per run: wall, outcome, and (N>1) the [parallel] stderr summary.
Appends one JSON line per run to state/campaign_raw.jsonl; solver stderr
is kept under logs/ (gitignored). Outcome from --outcome-only stdout.
"""
import json
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path("/workspace/atomic_solver")
BIN = ROOT / "target/release/atomic_solver"
STATE = ROOT / "docs/plans/parallel/measurements/plan5/state"
LOGS = ROOT / "docs/plans/parallel/measurements/plan5/logs"
CASES = [
    ("m22", "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22", 30),
    ("shuffle_win", "4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21", 100),
]
THREADS = [1, 2, 3, 4]


def one_run(case, fen, cap, threads, round_no):
    t0 = time.monotonic()
    proc = subprocess.run(
        [str(BIN), "--fen", fen, "--timeout", str(cap), "--first-outcome",
         "--outcome-only"] + ([] if threads == 1 else ["--threads", str(threads)]),
        capture_output=True, text=True)
    wall = time.monotonic() - t0
    outcome = None
    for line in proc.stdout.splitlines():
        if line.startswith("outcome:"):
            outcome = line.split()[1]
            break
    par = {}
    for line in proc.stderr.splitlines():
        if line.startswith("[parallel]"):
            for tok in line.split()[1:]:
                k, _, v = tok.partition("=")
                par[k] = int(v)
    rec = {"case": case, "round": round_no, "threads": threads,
           "wall_s": round(wall, 3), "outcome": outcome,
           "cap_s": cap, "rc": proc.returncode, **par}
    (LOGS / f"campaign_{case}_r{round_no}_t{threads}.err").write_text(proc.stderr)
    with open(STATE / "campaign_raw.jsonl", "a") as f:
        f.write(json.dumps(rec) + "\n")
    print(json.dumps(rec), flush=True)


def main():
    rounds = int(sys.argv[1]) if len(sys.argv) > 1 else 5
    only = sys.argv[2] if len(sys.argv) > 2 else None
    for case, fen, cap in CASES:
        if only and case != only:
            continue
        for r in range(1, rounds + 1):
            for n in THREADS:
                one_run(case, fen, cap, n, r)


if __name__ == "__main__":
    main()
