#!/usr/bin/env python3
"""plan3 corpus driver: TT pre-probe before make/unmake, one arm per invocation.

Runs the plan3 corpus against one release binary and records, per case: median
wall (monotonic, around `fork`+`exec`+`wait4`), peak RSS (`ru_maxrss`), minor
faults (`ru_minflt`), the child-eval count (stderr `evals:`) and the full
parsed stdout (outcome / pv length / pv_status / budget/timeout markers).

This is the Phase-0 / after single-arm corpus record. The authoritative wall
comparison for plan3's bars 3-4 is `ab_wall.py` (interleaved ABAB); single-arm
ratios are not accepted for those bars.

Usage:
  python3 driver.py --binary /tmp/plan3_base/target/release/atomic_solver \
      --runs 5 --out state/baseline.json
  python3 driver.py --binary target/release/atomic_solver \
      --runs 5 --out state/after.json
"""

import argparse
import json
import os
import re
import statistics
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode = True

BINARY_DEFAULT = "target/release/atomic_solver"

TT_MB = ["--tt-size", "256"]

DEC13_FEN = "r1bq1k1r/ppN4p/n1p1p3/3p1n1P/1b1P2P1/2P5/PP6/RNBQKB1R w KQ - 1 14"
DEC10_FEN = "3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41"
M22_FEN = "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22"
STRESS_FEN = "4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21"
REM12_FEN = "rnbqkbnr/8/6pp/pppppp1B/3PPP2/N5PN/PPP4P/R1BQ1RK1 w kq - 0 10"

# name, class, fen, case-specific args
CASES = [
    ("mate1", "short", "3Qk3/8/8/8/8/8/8/4K3 w - - 0 1", ["--timeout", "2"]),
    ("easy_kq", "short", "4k3/8/8/8/8/8/8/4KQ2 w - - 0 1", ["--timeout", "2"]),
    ("dec13", "deep", DEC13_FEN, ["--timeout", "10"]),
    ("dec10", "deep", DEC10_FEN, ["--timeout", "10"]),
    ("m22", "deep", M22_FEN, ["--timeout", "10"]),
    ("m22_b", "deep-budget", M22_FEN, ["--budget", "10000000"]),
    ("stress_b", "deep-budget", STRESS_FEN, ["--budget", "10000000"]),
    ("rem12", "deep-long", REM12_FEN, ["--timeout", "60"]),
]

RE_OUTCOME = re.compile(r"^outcome: (\w+) length: (\d+)$")
RE_PV_STATUS = re.compile(r"^pv_status: (.+)$")
RE_BUDGET = re.compile(r"^budget exhausted$")
RE_TIMEOUT = re.compile(r"^timeout$")
RE_EVALS = re.compile(r"^evals: (\d+)$", re.MULTILINE)


def run_once(binary, fen, extra):
    """One solver run: (wall_s, peak_rss_kb, minor_faults, stdout, stderr)."""
    cmd = [binary, "--fen", fen, *extra, "--first-outcome", "--outcome-only"]
    with tempfile.TemporaryFile() as out_fh, tempfile.TemporaryFile() as err_fh:
        t0 = time.monotonic()
        proc = subprocess.Popen(cmd, stdout=out_fh, stderr=err_fh)
        _pid, status, rusage = os.wait4(proc.pid, 0)
        wall = time.monotonic() - t0
        proc.returncode = os.waitstatus_to_exitcode(status)
        out_fh.seek(0)
        err_fh.seek(0)
        stdout = out_fh.read().decode()
        stderr = err_fh.read().decode()
    return wall, rusage.ru_maxrss, rusage.ru_minflt, stdout, stderr


def parse(stdout, stderr):
    outcome = None
    pv_len = None
    pv_status = None
    for line in stdout.splitlines():
        m = RE_OUTCOME.match(line)
        if m:
            outcome = m.group(1)
            pv_len = int(m.group(2))
        m = RE_PV_STATUS.match(line)
        if m:
            pv_status = m.group(1)
    evals = RE_EVALS.search(stderr)
    return {
        "outcome": outcome,
        "pv_len": pv_len,
        "pv_status": pv_status,
        "budget_exhausted": bool(RE_BUDGET.search(stdout)),
        "timeout": bool(RE_TIMEOUT.search(stdout)),
        "evals": int(evals.group(1)) if evals else None,
        "stdout": [line for line in stdout.splitlines() if line.strip()],
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--binary", default=BINARY_DEFAULT)
    ap.add_argument("--runs", type=int, default=5)
    ap.add_argument("--only", default=None, help="comma-separated case names")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    only = set(args.only.split(",")) if args.only else None
    results = {}
    for name, klass, fen, extra in CASES:
        if only and name not in only:
            continue
        walls, rss, faults = [], [], []
        parsed = None
        for _ in range(args.runs):
            wall, rss_kb, minflt, stdout, stderr = run_once(
                args.binary, fen, TT_MB + extra
            )
            walls.append(round(wall, 4))
            rss.append(rss_kb)
            faults.append(minflt)
            if parsed is None:
                parsed = parse(stdout, stderr)
        parsed["wall_s"] = walls
        parsed["wall_median_s"] = round(statistics.median(walls), 4)
        parsed["wall_min_s"] = min(walls)
        parsed["rss_max_kb"] = max(rss)
        parsed["rss_median_kb"] = int(statistics.median(rss))
        parsed["minflt_median"] = int(statistics.median(faults))
        parsed["class"] = klass
        parsed["fen"] = fen
        parsed["args"] = TT_MB + extra
        results[name] = parsed
        print(
            f"{name:10s} {klass:12s} out={str(parsed['outcome']):5s} "
            f"len={parsed['pv_len']} evals={parsed['evals']} "
            f"wall={parsed['wall_median_s']:.4f} rss={parsed['rss_max_kb'] // 1024}MB "
            f"minflt={parsed['minflt_median']} status={parsed['pv_status']}"
            + (" BUDGET" if parsed["budget_exhausted"] else "")
            + (" TIMEOUT" if parsed["timeout"] else ""),
            flush=True,
        )

    os.makedirs(os.path.dirname(args.out) or ".", exist_ok=True)
    with open(args.out, "w") as fh:
        json.dump(
            {
                "runs": args.runs,
                "binary": args.binary,
                "cases": results,
            },
            fh,
            indent=2,
            sort_keys=True,
        )
        fh.write("\n")
    print(f"wrote {args.out}")


if __name__ == "__main__":
    main()
