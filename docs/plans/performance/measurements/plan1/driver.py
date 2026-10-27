#!/usr/bin/env python3
"""plan1 corpus driver: probe stand-in vs production baseline vs after.

Corpus classes: easy-win (bounded DF-PN solves quickly), hard-win (KQvK
ladder root / step 3), draw (KR/KN/KB/cyclic-rook/adjacency-immunity),
terminal (mate/stalemate/extinction roots), detector-miss control.

Modes:
  probe     --tt-size 4 --budget 2000 --no-preflight (the plan's stand-in
            for the cheap probe; Phase 0 calibration)
  baseline  --tt-size 256 --timeout 2 --first-outcome (shipped HEAD: the
            pre-phase runs unconditionally on gated roots)
  after     same configuration as baseline, run on the post-change binary

Each case runs `--runs` times; wall (monotonic) and peak RSS (`ru_maxrss`
from `os.wait4` on the child) are recorded per run, plus the parsed stdout
(outcome / pv length / pv_status / preflight line / budget line) and the
stderr `evals:` counter.

Usage:
  python3 driver.py --binary target/release/atomic_solver --mode probe \
      --runs 1 --out state/probe_phase0.json
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

# name, class, fen
CASES = [
    ("easy_qf1", "easy-win", "4k3/8/8/8/8/8/8/4KQ2 w - - 0 1"),
    ("easy_qd1", "easy-win", "4k3/8/8/8/8/8/8/3QK3 w - - 0 1"),
    ("easy_qa1", "easy-win", "4k3/8/8/8/8/8/8/Q3K3 w - - 0 1"),
    ("easy_qd8", "easy-win", "3Qk3/8/8/8/8/8/8/4K3 w - - 0 1"),
    ("hard_ladder_root", "hard-win", "8/2K5/k7/8/8/8/8/4Q3 w - - 0 1"),
    ("hard_ladder_step3", "hard-win", "8/5K2/8/3k4/8/8/8/4Q3 w - - 6 4"),
    ("draw_kr", "draw", "4k3/8/8/8/8/8/8/4KR2 w - - 0 1"),
    ("draw_kn", "draw", "4k3/8/8/8/8/8/8/4KN2 w - - 0 1"),
    ("draw_kb", "draw", "4k3/8/8/8/8/8/8/4KB2 w - - 0 1"),
    ("draw_cyclic_rook", "draw", "8/8/8/8/2k5/8/8/4KR2 w - - 0 1"),
    ("draw_adjacency", "draw", "8/8/8/8/8/1K6/1Q6/k7 b - - 0 1"),
    ("term_mate", "terminal", "7K/8/8/8/8/8/1Q6/k7 b - - 0 1"),
    ("term_stalemate", "terminal", "7k/8/8/8/8/8/2q5/K7 w - - 0 1"),
    ("term_extinct", "terminal", "8/8/8/8/8/8/8/4k3 b - - 0 1"),
    ("control_4men", "detector-miss", "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"),
]

MODES = {
    "probe": ["--tt-size", "4", "--budget", "2000", "--no-preflight"],
    "baseline": ["--tt-size", "256", "--timeout", "2"],
    "after": ["--tt-size", "256", "--timeout", "2"],
}

RE_OUTCOME = re.compile(r"^outcome: (\w+) length: (\d+)$")
RE_PV_STATUS = re.compile(r"^pv_status: (.+)$")
RE_PREFLIGHT = re.compile(r"^preflight: (.+)$")
RE_BUDGET = re.compile(r"^budget exhausted$")
RE_TIMEOUT = re.compile(r"^timeout$")
RE_EVALS = re.compile(r"^evals: (\d+)$", re.MULTILINE)


def run_once(binary, fen, extra):
    """One solver run: returns (wall_s, peak_rss_kb, stdout, stderr)."""
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
    return wall, rusage.ru_maxrss, stdout, stderr


def run_report_line(binary, fen, extra):
    """One non-`--outcome-only` run (stdin at /dev/null) to capture the
    `preflight:` report line, which outcome-only mode suppresses."""
    cmd = [binary, "--fen", fen, *extra, "--first-outcome"]
    with tempfile.TemporaryFile() as out_fh, tempfile.TemporaryFile() as err_fh:
        proc = subprocess.Popen(
            cmd,
            stdout=out_fh,
            stderr=err_fh,
            stdin=subprocess.DEVNULL,
        )
        proc.wait()
        out_fh.seek(0)
        return out_fh.read().decode()


def parse(stdout, stderr):
    outcome = None
    pv_len = None
    for line in stdout.splitlines():
        m = RE_OUTCOME.match(line)
        if m:
            outcome = m.group(1)
            pv_len = int(m.group(2))
    pv_status = None
    preflight = None
    for line in stdout.splitlines():
        m = RE_PV_STATUS.match(line)
        if m:
            pv_status = m.group(1)
        m = RE_PREFLIGHT.match(line)
        if m:
            preflight = m.group(1)
    evals = RE_EVALS.search(stderr)
    return {
        "outcome": outcome,
        "pv_len": pv_len,
        "pv_status": pv_status,
        "preflight": preflight,
        "budget_exhausted": bool(RE_BUDGET.search(stdout)),
        "timeout": bool(RE_TIMEOUT.search(stdout)),
        "evals": int(evals.group(1)) if evals else None,
    }


def parse_preflight(stdout):
    for line in stdout.splitlines():
        m = RE_PREFLIGHT.match(line)
        if m:
            return m.group(1)
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--binary", default=BINARY_DEFAULT)
    ap.add_argument("--mode", required=True, choices=sorted(MODES))
    ap.add_argument("--runs", type=int, default=1)
    ap.add_argument("--only", default=None, help="comma-separated case names")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    only = set(args.only.split(",")) if args.only else None
    results = {}
    for name, klass, fen in CASES:
        if only and name not in only:
            continue
        walls, rss, parsed = [], [], None
        for _ in range(args.runs):
            wall, rss_kb, stdout, stderr = run_once(args.binary, fen, MODES[args.mode])
            walls.append(round(wall, 4))
            rss.append(rss_kb)
            if parsed is None:
                parsed = parse(stdout, stderr)
        report_line = parse_preflight(run_report_line(args.binary, fen, MODES[args.mode]))
        parsed["preflight"] = report_line
        parsed["wall_s"] = walls
        parsed["wall_median_s"] = round(statistics.median(walls), 4)
        parsed["rss_max_kb"] = max(rss)
        parsed["class"] = klass
        parsed["fen"] = fen
        parsed["mode"] = args.mode
        results[name] = parsed
        print(
            f"{name:20s} {klass:13s} out={str(parsed['outcome']):5s} "
            f"len={parsed['pv_len']} evals={parsed['evals']} "
            f"wall={parsed['wall_median_s']} status={parsed['pv_status']} "
            f"pre={parsed['preflight']}"
            + (" BUDGET" if parsed["budget_exhausted"] else "")
            + (" TIMEOUT" if parsed["timeout"] else ""),
            flush=True,
        )

    os.makedirs(os.path.dirname(args.out) or ".", exist_ok=True)
    with open(args.out, "w") as fh:
        json.dump(
            {
                "mode": args.mode,
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
