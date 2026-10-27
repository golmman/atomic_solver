#!/usr/bin/env python3
"""Interleaved wall-clock A/B for the lazy (zero-init) TT allocation (plan2).

One repetition = one run of each arm (baseline, after), so container drift
hits both arms equally; the interleaved medians are the authoritative wall
comparison for the plan's bars 2-5. Per run it records wall, peak RSS
(`ru_maxrss`), minor faults (`ru_minflt`), the child-eval count and the
parsed stdout, so eval identity is checked inside the same interleaved loop.

The baseline binary must be built from pristine `HEAD`, e.g.

  rm -rf /tmp/plan2_base && mkdir -p /tmp/plan2_base
  git archive HEAD | tar -x -C /tmp/plan2_base
  (cd /tmp/plan2_base && cargo build --release --bins)

Usage:
  python3 ab_wall.py --base /tmp/plan2_base/target/release/atomic_solver \
      --after target/release/atomic_solver --reps 15 --out state/ab_wall.json
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

DEC10_FEN = "3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41"
M22_FEN = "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22"
STRESS_FEN = "4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21"

# name, class, deep (bar-5 deep cell), fen, case-specific args
CASES = [
    ("mate1", "short", False, "3Qk3/8/8/8/8/8/8/4K3 w - - 0 1", ["--timeout", "2"]),
    ("easy_kq", "short", False, "4k3/8/8/8/8/8/8/4KQ2 w - - 0 1", ["--timeout", "2"]),
    ("draw_kr", "draw", False, "4k3/8/8/8/8/8/8/4KR2 w - - 0 1", ["--timeout", "2"]),
    ("draw_cyc", "draw", False, "8/8/8/8/2k5/8/8/4KR2 w - - 0 1", ["--timeout", "2"]),
    ("ladder_root", "hard-win", False, "8/2K5/k7/8/8/8/8/4Q3 w - - 0 1", ["--timeout", "2"]),
    ("dec10", "deep", True, DEC10_FEN, ["--timeout", "10"]),
    ("m22", "deep", True, M22_FEN, ["--timeout", "10"]),
    ("m22_b", "deep-budget", True, M22_FEN, ["--budget", "10000000"]),
    ("stress_b", "deep-budget", True, STRESS_FEN, ["--budget", "10000000"]),
]


def evals_of(stderr_bytes):
    for line in stderr_bytes.decode().splitlines():
        if line.startswith("evals: "):
            return int(line.split()[1])
    return None


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
        return {
            "wall_s": round(wall, 4),
            "rss_max_kb": rusage.ru_maxrss,
            "minflt": rusage.ru_minflt,
            "stdout": out_fh.read().decode(),
            "evals": evals_of(err_fh.read()),
        }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", required=True)
    ap.add_argument("--after", default="target/release/atomic_solver")
    ap.add_argument("--reps", type=int, default=15)
    ap.add_argument("--only", default=None, help="comma-separated case names")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    only = set(args.only.split(",")) if args.only else None
    results = {}
    for name, klass, deep, fen, extra in CASES:
        if only and name not in only:
            continue
        base, after = [], []
        for _ in range(args.reps):
            base.append(run_arm(args.base, fen, extra))
            after.append(run_arm(args.after, fen, extra))
        bw = statistics.median([r["wall_s"] for r in base])
        aw = statistics.median([r["wall_s"] for r in after])
        rec = {
            "class": klass,
            "deep": deep,
            "fen": fen,
            "args": TT_MB + extra,
            "reps": args.reps,
            "base_wall_s": [r["wall_s"] for r in base],
            "after_wall_s": [r["wall_s"] for r in after],
            "base_wall_median_s": round(bw, 4),
            "after_wall_median_s": round(aw, 4),
            "wall_ratio": round(aw / bw, 4),
            "base_evals": base[0]["evals"],
            "after_evals": after[0]["evals"],
            "evals_identical": all(
                r["evals"] == base[0]["evals"] for r in base
            )
            and all(r["evals"] == base[0]["evals"] for r in after),
            "base_stdout_identical": all(r["stdout"] == base[0]["stdout"] for r in base),
            "after_stdout_identical": all(r["stdout"] == base[0]["stdout"] for r in after),
            "base_rss_max_kb": max(r["rss_max_kb"] for r in base),
            "after_rss_max_kb": max(r["rss_max_kb"] for r in after),
            "base_minflt_median": int(statistics.median([r["minflt"] for r in base])),
            "after_minflt_median": int(statistics.median([r["minflt"] for r in after])),
            "base_stdout": base[0]["stdout"],
            "after_stdout": after[0]["stdout"],
        }
        results[name] = rec
        print(
            f"{name:12s} {klass:12s} base={rec['base_wall_median_s']:.4f}s "
            f"after={rec['after_wall_median_s']:.4f}s ratio={rec['wall_ratio']:.3f}"
            + (" DEEP" if deep else "")
            + (" EVAL-DIFF" if not rec["evals_identical"] else ""),
            f"evals {rec['base_evals']} -> {rec['after_evals']}",
            f"rss {rec['base_rss_max_kb'] // 1024}->{rec['after_rss_max_kb'] // 1024}MB",
            f"minflt {rec['base_minflt_median']}->{rec['after_minflt_median']}",
            flush=True,
        )

    os.makedirs(os.path.dirname(args.out) or ".", exist_ok=True)
    with open(args.out, "w") as fh:
        json.dump({"reps": args.reps, "cases": results}, fh, indent=2, sort_keys=True)
        fh.write("\n")
    print(f"wrote {args.out}")


if __name__ == "__main__":
    main()
