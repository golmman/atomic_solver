#!/usr/bin/env python3
"""plan6 campaign: interleaved N=1/2/3/4 wall runs on m22 + rem12 + shuffle-win.

plan5b's campaign protocol, tightened per plan6: >= 8 interleaved rounds per
case, each round runs N in 1,2,3,4 order (N=1 is the in-session reference).
Per run: wall, outcome, and (N>1) the [parallel] stderr counters.
Appends one JSON line per run to state/campaign6_raw.jsonl; solver stderr is
kept under logs/ (gitignored). Outcome from --outcome-only stdout.

Usage: campaign6.py <rounds> [case_filter]
"""
import json
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path("/workspace/atomic_solver")
BIN = ROOT / "target/release/atomic_solver"
HERE = Path(__file__).resolve().parent
STATE = HERE / "state"
LOGS = HERE / "logs"
CASES = [
    ("m22", "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22", 30),
    ("rem12", "rnbqkbnr/8/6pp/pppppp1B/3PPP2/N5PN/PPP4P/R1BQ1RK1 w kq - 0 10", 60),
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
                try:
                    par[k] = int(v)
                except ValueError:
                    par[k] = v
    rec = {"case": case, "round": round_no, "threads": threads,
           "wall_s": round(wall, 3), "outcome": outcome,
           "cap_s": cap, "rc": proc.returncode, **par}
    (LOGS / f"campaign6_{case}_r{round_no}_t{threads}.err").write_text(proc.stderr)
    with open(STATE / "campaign6_raw.jsonl", "a") as f:
        f.write(json.dumps(rec) + "\n")
    print(json.dumps(rec), flush=True)


def main():
    rounds = int(sys.argv[1]) if len(sys.argv) > 1 else 8
    only = sys.argv[2] if len(sys.argv) > 2 else None
    STATE.mkdir(exist_ok=True)
    LOGS.mkdir(exist_ok=True)
    for case, fen, cap in CASES:
        if only and case != only:
            continue
        for r in range(1, rounds + 1):
            for n in THREADS:
                one_run(case, fen, cap, n, r)


if __name__ == "__main__":
    main()
