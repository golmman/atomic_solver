#!/usr/bin/env python3
"""plan11: fresh same-binary salt-0 solo anchors (the pure racer R0's
deterministic reference for the M2/H4a fidelity checks; report10
deviation-1 protocol — cross-binary anchors are never used).
Machine-exclusive: run only when no race is active. m20_white s0 is skipped
(budget-censored solo, never decisive; R0 is killed before its evals line in
every m20 race).

Writes state/solo_s0.json.
"""

import json
import os
import subprocess
import sys
import time

sys.dont_write_bytecode = True

HERE = os.path.dirname(os.path.abspath(__file__))
BIN = os.environ.get(
    "BIN", "/workspace/atomic_solver/target/release/atomic_solver"
)
STATE = os.path.join(HERE, "state")
LOGS = os.path.join(HERE, "logs")

CASES = {
    "m22_white": ("4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22", 1_000_000_000, 600),
    "dec10": ("3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41", 1_000_000_000, 600),
    "dec13": ("r1bq1k1r/ppN4p/n1p1p3/3p1n1P/1b1P2P1/2P5/PP6/RNBQKB1R w KQ - 1 14", 1_000_000_000, 600),
    "rem12": ("rnbqkbnr/8/6pp/pppppp1B/3PPP2/N5PN/PPP4P/R1BQ1RK1 w kq - 0 10", 1_000_000_000, 600),
    "stress": ("4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21", 2_500_000_000, 600),
}


def main():
    os.makedirs(STATE, exist_ok=True)
    out = {}
    path = os.path.join(STATE, "solo_s0.json")
    if os.path.exists(path):
        out = json.load(open(path))
    for case, (fen, budget, timeout) in CASES.items():
        if case in out:
            continue
        cmd = [
            BIN, "--fen", fen, "--tt-size", "128", "--first-outcome",
            "--outcome-only", "--salt", "0", "--budget", str(budget),
            "--timeout", str(timeout),
        ]
        t0 = time.time()
        logdir = os.path.join(LOGS, "solo_s0")
        os.makedirs(logdir, exist_ok=True)
        with open(os.path.join(logdir, f"{case}.out"), "w") as fo, \
                open(os.path.join(logdir, f"{case}.err"), "w") as fe:
            p = subprocess.run(cmd, stdout=fo, stderr=fe, stdin=subprocess.DEVNULL)
        wall = time.time() - t0
        err = open(os.path.join(logdir, f"{case}.err")).read()
        stdout = open(os.path.join(logdir, f"{case}.out")).read()
        evals = None
        for line in err.splitlines():
            if line.startswith("evals: "):
                evals = int(line.split()[1])
        decisive = "budget exhausted" not in stdout and any(
            line.startswith("outcome: ") and line.split()[1] in ("win", "loss")
            for line in stdout.splitlines()
        )
        out[case] = {
            "evals": evals,
            "wall_s": wall,
            "rc": p.returncode,
            "decisive": decisive,
            "budget_exhausted": "budget exhausted" in stdout,
        }
        print(case, out[case], flush=True)
    with open(path, "w") as fh:
        json.dump(out, fh, indent=1)
    print("SOLO ANCHORS DONE", flush=True)


if __name__ == "__main__":
    main()
