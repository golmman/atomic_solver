#!/usr/bin/env python3
"""plan12 rollout: per-case salt distribution over the 22-case corpus.

Runs the pre-registered grid (plan12.md D1–D3): 22 cases x 5 salts (0..4) of

    atomic_solver --fen <FEN> --tt-size 128 --first-outcome --outcome-only \
        --salt <s> --budget <cap> --timeout 600

First-outcome `child_evals` is parsed from the stderr `evals:` line; a run
that exits via `budget exhausted` or `timeout` without a decisive outcome is
right-censored at the cap. Salt 0 doubles as the trajectory-identity anchor
(the six plan11 pilot cases must reproduce their known salt-0 counts).

PIVOT extension (D7): if parse.py reports a correlated salt pair, add the
replacement salt(s) to EXTRA_SALTS and re-run; existing raw outputs are
skipped (resumable).

Usage:
    python3 driver.py            # runs the grid
    BIN=... RAW=... python3 driver.py

Env:
    BIN  solver binary (default: <repo>/target/release/atomic_solver)
    RAW  raw output directory (default: /tmp/plan12/results; transcripts are
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
RAW = os.environ.get("RAW", "/tmp/plan12/results")

# D1 corpus: whole m20–m23 family + decisive controls dec01–dec18.
# expected: fixture outcome from the side-to-move perspective (D5(c) check).
CASES = {
    "m20_white": ("4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R5RK w - - 4 20", "win"),
    "stress": ("4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21", "win"),
    "m22_white": ("4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22", "win"),
    "m23_white": ("4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23", "win"),
    "dec01": ("r5r1/5N1k/2p2p2/pp1p3p/3Pp3/2P1P3/P7/2bQ1R1K w - - 0 30", "win"),
    "dec02": ("8/1k1p4/1P2p3/p2P1P2/P7/6p1/6K1/8 b - - 0 27", "loss"),
    "dec03": ("r4r2/pp2p1Bk/2p3p1/1B1pP1bp/3P3P/1PN5/2P5/5R1K w - - 1 21", "win"),
    "dec04": ("3k3r/1p1P4/4p2P/5p2/P7/2n1P3/6K1/7R b - - 0 25", "win"),
    "dec05": ("7k/7p/1pp3p1/3B4/2P5/5pPP/P7/R5K1 w - - 0 26", "win"),
    "dec06": ("3r2k1/8/p7/2P2P2/8/1P1p3P/P2K2r1/6R1 w - - 1 32", "loss"),
    "dec07": ("r2qk2r/p2n4/6pp/1ppppp2/3P1B2/5PPP/PPP1Q1B1/R4RK1 w kq - 0 18", "win"),
    "dec08": ("r4r1k/3q1P2/pp4pp/2pp4/N4Q2/2P3PP/PP2p3/R4R1K w - - 0 25", "win"),
    "dec09": ("5k2/p1Rb1P2/6r1/1P2p3/P2pP1np/3P3B/3b4/3K2R1 b - - 0 28", "win"),
    "dec10": ("3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41", "win"),
    "dec11": ("rnb1k2r/p2p3p/4ppp1/qp6/3PPQ2/8/PPP2PPP/R3KB1R w KQkq - 2 11", "win"),
    "dec12": ("2b2k1r/pp5p/2ppp3/8/4P2P/qPPP4/P1Q2b2/5R1K w - - 0 23", "win"),
    "dec13": ("r1bq1k1r/ppN4p/n1p1p3/3p1n1P/1b1P2P1/2P5/PP6/RNBQKB1R w KQ - 1 14", "win"),
    "dec14": ("r1bq1k1r/ppN5/n1p1p1pp/3p1p1P/3P4/P1PB4/1P3PP1/R1BQK2R w KQ - 0 15", "win"),
    "dec15": ("r3k1r1/1b1n3p/3p1pPb/pp1Pp3/P3P3/4B3/1PP4P/R3KBR1 w Qq - 1 20", "win"),
    "dec16": ("2kr1b1r/ppp3pp/4bp2/4p3/2B1P3/2PP2PP/PP1n1P2/RN1Q1R1K w - - 3 14", "win"),
    "dec17": ("r1bqk2r/pp4pp/2p2p1n/1P4B1/4P3/2Pp1PP1/7P/R2QKBNR w KQkq - 0 14", "win"),
    "dec18": ("r1b2k1r/p1p4p/1pnNppp1/1B1p4/1b1PP3/2P5/PP4PP/RNB1K2R w KQ - 0 13", "win"),
}

# D3 budgets: stress 2.5 B (reexamination cap); m20 2 B (pre-registered
# extension for the censored-heavy pilot case); all others 1 B.
BUDGET = {"stress": 2_500_000_000, "m20_white": 2_000_000_000}
BUDGET_DEFAULT = 1_000_000_000
TIMEOUT_S = 600
SALTS = [0, 1, 2, 3, 4]
# D7 PIVOT extension (fired 2026-10-10): parse.py found salt pair (1, 4)
# near-identical (< 1e-4 rel) on two eval-sensitive cases (dec10, dec13).
# Per D7, salt 5 was added and analysis uses all available draws: the
# canonical salt set is {0, 1, 2, 3, 4, 5}, with per-case basin counts
# reported (near-twin structure documented in gate_methodology.md).
EXTRA_SALTS = [5]


def jobs():
    for case in CASES:
        for salt in SALTS + EXTRA_SALTS:
            yield case, salt


def run(job):
    case, salt = job
    name = f"{case}_s{salt}"
    out_path = os.path.join(RAW, name + ".out")
    err_path = os.path.join(RAW, name + ".err")
    meta_path = os.path.join(RAW, name + ".meta.json")
    if os.path.exists(out_path) and os.path.getsize(out_path) > 0:
        return name, "skip"
    fen, _expected = CASES[case]
    cmd = [
        BIN,
        "--fen",
        fen,
        "--tt-size",
        "128",
        "--first-outcome",
        "--outcome-only",
        "--salt",
        str(salt),
        "--budget",
        str(BUDGET.get(case, BUDGET_DEFAULT)),
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
