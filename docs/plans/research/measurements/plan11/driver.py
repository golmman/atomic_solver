#!/usr/bin/env python3
"""plan11 pilot: per-case salt distribution on the product CLI.

Runs the D4 grid: 6 cases x 5 salts (0..4) of

    atomic_solver --fen <FEN> --tt-size 128 --first-outcome --outcome-only \
        --salt <s> --budget <cap> --timeout 600

First-outcome `child_evals` is parsed from the stderr `evals:` line; a run
that exits via `budget exhausted` or `timeout` without a decisive outcome is
right-censored at the cap. Salt 0 doubles as the trajectory-identity check
(stress must reproduce 249,480,478 exactly). One duplicate run (stress salt 1)
covers the PIVOT check (within-salt rerun variance).

Usage:
    python3 driver.py            # runs the grid (resumable: existing raw
                                 # outputs are skipped)
    BIN=... RAW=... python3 driver.py

Env:
    BIN  solver binary (default: <repo>/target/release/atomic_solver)
    RAW  raw output directory (default: /tmp/plan11/results; transcripts are
         not committed per the AGENTS.md measurement layout)
"""

import concurrent.futures
import json
import os
import subprocess
import sys
import time

sys.dont_write_bytecode = True

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../../../.."))
BIN = os.environ.get("BIN", os.path.join(REPO, "target/release/atomic_solver"))
RAW = os.environ.get("RAW", "/tmp/plan11/results")

# D3 corpus: named fixtures from tests/fixtures/. All are won for the side to
# move (expected outcome `win` in both fixture files).
CASES = {
    "m20_white": "4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R5RK w - - 4 20",
    "stress": "4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21",
    "m22_white": "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22",
    "m23_white": "4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23",
    "dec13": "r1bq1k1r/ppN4p/n1p1p3/3p1n1P/1b1P2P1/2P5/PP6/RNBQKB1R w KQ - 1 14",
    "dec10": "3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41",
}

# D4: per-run child-eval caps (stress 2.5 B as in reexamination.md, others 1 B),
# wall-clock secondary cap 600 s/run.
BUDGET = {c: (2_500_000_000 if c == "stress" else 1_000_000_000) for c in CASES}
TIMEOUT_S = 600
SALTS = [0, 1, 2, 3, 4]

# PIVOT check: one duplicate run on the worst case (stress, the heavy-tailed
# shape). Same salt: a deterministic, budget-bounded search must reproduce the
# first draw bit-for-bit.
DUPLICATES = [("stress", 1, "rerun")]


def jobs():
    for case in CASES:
        for salt in SALTS:
            yield case, salt, ""
    for case, salt, tag in DUPLICATES:
        yield case, salt, tag


def run(job):
    case, salt, tag = job
    name = f"{case}_s{salt}{tag}"
    out_path = os.path.join(RAW, name + ".out")
    err_path = os.path.join(RAW, name + ".err")
    meta_path = os.path.join(RAW, name + ".meta.json")
    if os.path.exists(out_path) and os.path.getsize(out_path) > 0:
        return name, "skip"
    cmd = [
        BIN,
        "--fen",
        CASES[case],
        "--tt-size",
        "128",
        "--first-outcome",
        "--outcome-only",
        "--salt",
        str(salt),
        "--budget",
        str(BUDGET[case]),
        "--timeout",
        str(TIMEOUT_S),
    ]
    t0 = time.time()
    with open(out_path, "w") as fo, open(err_path, "w") as fe:
        p = subprocess.run(cmd, stdout=fo, stderr=fe, stdin=subprocess.DEVNULL)
    wall = time.time() - t0
    with open(meta_path, "w") as fm:
        json.dump({"cmd": cmd, "rc": p.returncode, "wall_s": wall}, fm)
    return name, f"rc={p.returncode} wall={wall:.1f}s"


if __name__ == "__main__":
    os.makedirs(RAW, exist_ok=True)
    if not os.path.exists(BIN):
        raise SystemExit(f"solver binary not found: {BIN}")
    with concurrent.futures.ThreadPoolExecutor(3) as ex:
        for name, status in ex.map(run, list(jobs())):
            print(name, status, flush=True)
    print("ALL DONE")
