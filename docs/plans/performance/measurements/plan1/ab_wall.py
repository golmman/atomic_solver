#!/usr/bin/env python3
"""Interleaved wall-clock A/B for the plan1 probe gate.

Baseline and after runs are interleaved (ABAB…, one repetition = one run of
each) so that container drift hits both arms equally. Configuration per arm
is the production one: `--tt-size 256 --timeout 2 --first-outcome
--outcome-only` (the probe gate is automatic in the after arm).

The baseline binary must be built from pristine `HEAD`, e.g.

  git archive HEAD | tar -x -C /tmp/plan1_base
  (cd /tmp/plan1_base && cargo build --release --bins)

Usage:
  python3 ab_wall.py --base /tmp/plan1_base/target/release/atomic_solver \
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

CONFIG = ["--tt-size", "256", "--timeout", "2", "--first-outcome", "--outcome-only"]

# name, class, fen
CASES = [
    ("easy_qf1", "easy-win", "4k3/8/8/8/8/8/8/4KQ2 w - - 0 1"),
    ("draw_kr", "draw", "4k3/8/8/8/8/8/8/4KR2 w - - 0 1"),
    ("draw_kn", "draw", "4k3/8/8/8/8/8/8/4KN2 w - - 0 1"),
    ("draw_kb", "draw", "4k3/8/8/8/8/8/8/4KB2 w - - 0 1"),
    ("draw_cyclic_rook", "draw", "8/8/8/8/2k5/8/8/4KR2 w - - 0 1"),
    ("draw_adjacency", "draw", "8/8/8/8/8/1K6/1Q6/k7 b - - 0 1"),
    ("hard_ladder_root", "hard-win", "8/2K5/k7/8/8/8/8/4Q3 w - - 0 1"),
    ("hard_ladder_step3", "hard-win", "8/5K2/8/3k4/8/8/8/4Q3 w - - 6 4"),
]


def evals_of(stderr_bytes):
    for line in stderr_bytes.decode().splitlines():
        if line.startswith("evals: "):
            return int(line.split()[1])
    return None


def run_arm(binary, fen):
    with tempfile.TemporaryFile() as out_fh, tempfile.TemporaryFile() as err_fh:
        t0 = time.monotonic()
        proc = subprocess.Popen(
            [binary, "--fen", fen, *CONFIG], stdout=out_fh, stderr=err_fh
        )
        _pid, _status, rusage = os.wait4(proc.pid, 0)
        wall = time.monotonic() - t0
        out_fh.seek(0)
        err_fh.seek(0)
        return {
            "wall_s": round(wall, 4),
            "rss_max_kb": rusage.ru_maxrss,
            "stdout": out_fh.read().decode(),
            "evals": evals_of(err_fh.read()),
        }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", required=True)
    ap.add_argument("--after", default="target/release/atomic_solver")
    ap.add_argument("--reps", type=int, default=15)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    results = {}
    for name, klass, fen in CASES:
        base, after = [], []
        for _ in range(args.reps):
            base.append(run_arm(args.base, fen))
            after.append(run_arm(args.after, fen))
        bw = statistics.median([r["wall_s"] for r in base])
        aw = statistics.median([r["wall_s"] for r in after])
        rec = {
            "class": klass,
            "fen": fen,
            "reps": args.reps,
            "base_wall_s": [r["wall_s"] for r in base],
            "after_wall_s": [r["wall_s"] for r in after],
            "base_wall_median_s": round(bw, 4),
            "after_wall_median_s": round(aw, 4),
            "wall_ratio": round(aw / bw, 3),
            "base_evals": base[0]["evals"],
            "after_evals": after[0]["evals"],
            "base_rss_max_kb": max(r["rss_max_kb"] for r in base),
            "after_rss_max_kb": max(r["rss_max_kb"] for r in after),
            "base_stdout": base[0]["stdout"],
            "after_stdout": after[0]["stdout"],
        }
        results[name] = rec
        print(
            f"{name:20s} {klass:9s} base={rec['base_wall_median_s']:.4f}s "
            f"after={rec['after_wall_median_s']:.4f}s ratio={rec['wall_ratio']:.3f} "
            f"evals {rec['base_evals']} -> {rec['after_evals']}",
            flush=True,
        )

    os.makedirs(os.path.dirname(args.out) or ".", exist_ok=True)
    with open(args.out, "w") as fh:
        json.dump(
            {"config": CONFIG, "reps": args.reps, "cases": results},
            fh,
            indent=2,
            sort_keys=True,
        )
        fh.write("\n")
    print(f"wrote {args.out}")


if __name__ == "__main__":
    main()
