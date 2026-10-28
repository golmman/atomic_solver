#!/usr/bin/env python3
"""Counterbalanced A/B for the repetition-path bookkeeping probe (plan4).

One repetition = one run of each arm back to back; the arm order alternates by
repetition (base->after on even, after->base on odd) so within-pair drift does
not systematically penalize one arm. Records wall, process CPU time
(`ru_utime + ru_stime`), peak RSS, minor faults, the child-eval count and stdout.
`child_evals` and stdout equality across arms *and* repetitions is a hard
abort: the bookkeeping change must be behaviourally invisible.

The plan-session probe used this driver against three `/tmp` builds:
`incremental_only` (item 3 alone), `bloom_only` (item 4 alone), and `combined`
(both, the plan's design). The committed JSON files in this directory are the
parsed outputs; raw transcripts are not committed.

Usage:
  python3 ab_path_bookkeeping.py --base /tmp/plan4_base/target/release/atomic_solver \
      --after /tmp/plan4_combo/target/release/atomic_solver \
      --only dec10,m22_b,stress_b --out combined.json
"""

import argparse
import json
import os
import statistics
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode = True

TT_MB = ["--tt-size", "256"]

CASES = {
    "mate1": ("3Qk3/8/8/8/8/8/8/4K3 w - - 0 1", ["--timeout", "2"]),
    "easy_kq": ("4k3/8/8/8/8/8/8/4KQ2 w - - 0 1", ["--timeout", "2"]),
    "dec13": (
        "r1bq1k1r/ppN4p/n1p1p3/3p1n1P/1b1P2P1/2P5/PP6/RNBQKB1R w KQ - 1 14",
        ["--timeout", "10"],
    ),
    "dec10": (
        "3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41",
        ["--timeout", "10"],
    ),
    "m20_b": (
        "4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R5RK w - - 4 20",
        ["--budget", "30000000"],
    ),
    "m22_b": (
        "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22",
        ["--budget", "10000000"],
    ),
    "stress_b": (
        "4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21",
        ["--budget", "10000000"],
    ),
    "rem12": (
        "rnbqkbnr/8/6pp/pppppp1B/3PPP2/N5PN/PPP4P/R1BQ1RK1 w kq - 0 10",
        ["--timeout", "60"],
    ),
}

REPS = {
    "mate1": 15,
    "easy_kq": 15,
    "dec13": 21,
    "dec10": 31,
    "m20_b": 21,
    "m22_b": 21,
    "stress_b": 21,
    "rem12": 11,
}


def run_arm(binary, fen, extra):
    with tempfile.TemporaryFile() as out_fh, tempfile.TemporaryFile() as err_fh:
        t0 = time.monotonic()
        proc = subprocess.Popen(
            [binary, "--fen", fen, *TT_MB, *extra, "--first-outcome", "--outcome-only"],
            stdout=out_fh,
            stderr=err_fh,
        )
        _pid, _status, rusage = os.wait4(proc.pid, 0)
        wall = time.monotonic() - t0
        out_fh.seek(0)
        err_fh.seek(0)
        stdout = out_fh.read().decode()
        stderr = err_fh.read().decode()

    evals = None
    for line in stderr.splitlines():
        if line.startswith("evals: "):
            evals = int(line.split()[1])
            break
    return {
        "wall_s": wall,
        "cpu_s": rusage.ru_utime + rusage.ru_stime,
        "rss_max_kb": rusage.ru_maxrss,
        "minflt": rusage.ru_minflt,
        "evals": evals,
        "stdout": stdout,
    }


def median(values):
    return statistics.median(values)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", required=True)
    ap.add_argument("--after", default="target/release/atomic_solver")
    ap.add_argument("--only", default=None)
    ap.add_argument("--reps", default=None, help="per-case override, e.g. dec10:51")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    only = set(args.only.split(",")) if args.only else None
    overrides = {}
    for item in (args.reps or "").split(","):
        if item:
            key, value = item.split(":")
            overrides[key] = int(value)

    results = {}
    for name, (fen, extra) in CASES.items():
        if only and name not in only:
            continue
        reps = overrides.get(name, REPS[name])
        base, after = [], []
        for i in range(reps):
            if i % 2 == 0:
                rb = run_arm(args.base, fen, extra)
                ra = run_arm(args.after, fen, extra)
            else:
                ra = run_arm(args.after, fen, extra)
                rb = run_arm(args.base, fen, extra)
            base.append(rb)
            after.append(ra)

        reference = base[0]
        for arm, runs in (("base", base), ("after", after)):
            for i, run in enumerate(runs):
                if run["evals"] != reference["evals"]:
                    sys.exit(
                        f"IDENTITY FAILURE: {name} {arm} rep {i}: "
                        f"evals {run['evals']} != {reference['evals']}"
                    )
                if run["stdout"] != reference["stdout"]:
                    sys.exit(f"IDENTITY FAILURE: {name} {arm} rep {i}: stdout differs")

        bw = median([r["wall_s"] for r in base])
        aw = median([r["wall_s"] for r in after])
        bc = median([r["cpu_s"] for r in base])
        ac = median([r["cpu_s"] for r in after])
        paired = sorted(a["wall_s"] / b["wall_s"] for b, a in zip(base, after))
        quartiles = [
            round(paired[len(paired) // 4], 4),
            round(median(paired), 4),
            round(paired[(3 * len(paired)) // 4], 4),
        ]
        rec = {
            "fen": fen,
            "args": TT_MB + extra,
            "reps": reps,
            "order": "counterbalanced per repetition",
            "base_wall_median_s": round(bw, 4),
            "after_wall_median_s": round(aw, 4),
            "wall_ratio": round(aw / bw, 4),
            "base_cpu_median_s": round(bc, 4),
            "after_cpu_median_s": round(ac, 4),
            "cpu_ratio": round(ac / bc, 4),
            "paired_wall_ratio_quartiles": quartiles,
            "evals": reference["evals"],
            "base_rss_max_kb": max(r["rss_max_kb"] for r in base),
            "after_rss_max_kb": max(r["rss_max_kb"] for r in after),
            "base_minflt_median": int(median([r["minflt"] for r in base])),
            "after_minflt_median": int(median([r["minflt"] for r in after])),
            "stdout": reference["stdout"],
        }
        results[name] = rec
        print(
            f"{name:10s} reps={reps:2d} "
            f"wall {rec['base_wall_median_s']:.4f}->{rec['after_wall_median_s']:.4f} "
            f"ratio={rec['wall_ratio']:.4f} "
            f"cpu {rec['base_cpu_median_s']:.4f}->{rec['after_cpu_median_s']:.4f} "
            f"ratio={rec['cpu_ratio']:.4f} evals={rec['evals']}",
            flush=True,
        )

    os.makedirs(os.path.dirname(args.out) or ".", exist_ok=True)
    with open(args.out, "w") as fh:
        json.dump({"cases": results}, fh, indent=2, sort_keys=True)
        fh.write("\n")
    print(f"wrote {args.out}")


if __name__ == "__main__":
    main()
