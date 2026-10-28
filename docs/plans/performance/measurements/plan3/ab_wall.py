#!/usr/bin/env python3
"""Interleaved wall-clock A/B for the TT pre-probe (plan3).

One repetition = one run of each arm back to back, so container drift hits
both arms equally. The order **alternates** by repetition (base→after, then
after→base, …): a fixed base-then-after order systematically makes the after
arm the second run of every pair, which biases the ratio against it under
within-pair drift (measured on the dec10 bar-3 cell). The interleaved medians
are the authoritative wall comparison for plan3's bars 3-4 (single-arm runs
are not accepted for them). For the sub-2 ms latency-floor cells, copy the
after binary onto the baseline's filesystem first: process start from `/tmp`
versus the workspace overlay is a ~1 ms confound at that scale. Per run it
records wall, peak RSS (`ru_maxrss`), minor faults (`ru_minflt`), the
child-eval count and the parsed stdout.

Child-eval identity is a **hard error** here, not a data row: any difference
between the arms (or between repetitions) aborts the run, because plan3's
bar 1 makes a trajectory difference a falsification of the equivalence
argument, never a tolerance question.

The baseline binary must be built from pristine `HEAD`:

  rm -rf /tmp/plan3_base && mkdir -p /tmp/plan3_base
  git archive HEAD | tar -x -C /tmp/plan3_base
  (cd /tmp/plan3_base && cargo build --release --bins)

Usage:
  python3 ab_wall.py --base /tmp/plan3_base/target/release/atomic_solver \
      --after target/release/atomic_solver --out state/ab_wall.json
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

from driver import CASES, TT_MB  # noqa: E402  (same corpus, same per-case args)

# Reps per case: >= 11 everywhere (the plan's bar-3/4 floor). The
# transposition-dense cell gets the most repetitions because its expected gain
# is the largest; `rem12` (~7 s per run) gets the floor, which is still 2.5 min
# per arm.
REPS = {
    "mate1": 15,
    "easy_kq": 15,
    "dec13": 15,
    "dec10": 21,
    "m22": 15,
    "m22_b": 15,
    "stress_b": 15,
    "rem12": 11,
}


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
    ap.add_argument("--scale", type=float, default=1.0, help="scale every rep count")
    ap.add_argument(
        "--reps",
        default=None,
        help="per-case rep override, e.g. 'dec10:151,rem12:31' (bar-3 borderline cells)",
    )
    ap.add_argument("--only", default=None, help="comma-separated case names")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    only = set(args.only.split(",")) if args.only else None
    overrides = {}
    for item in (args.reps or "").split(","):
        if item:
            key, value = item.split(":")
            overrides[key] = int(value)
    results = {}
    for name, klass, fen, extra in CASES:
        if only and name not in only:
            continue
        reps = overrides.get(name, max(1, round(REPS[name] * args.scale)))
        base, after = [], []
        for i in range(reps):
            # Counterbalanced order: no arm is systematically second (see the
            # module docstring).
            if i % 2 == 0:
                rb = run_arm(args.base, fen, extra)
                ra = run_arm(args.after, fen, extra)
            else:
                ra = run_arm(args.after, fen, extra)
                rb = run_arm(args.base, fen, extra)
            base.append(rb)
            after.append(ra)

        # Hard identity gate: the pre-probe must be behaviourally invisible.
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

        bw = statistics.median([r["wall_s"] for r in base])
        aw = statistics.median([r["wall_s"] for r in after])
        # Paired estimator (drift-free but noisier per pair): the median of the
        # per-repetition ratios, with quartiles, so the report can state the
        # measurement's spread next to the median-of-medians the bars use.
        paired = sorted(a["wall_s"] / b["wall_s"] for b, a in zip(base, after))
        quartiles = [
            round(paired[len(paired) // 4], 4),
            round(statistics.median(paired), 4),
            round(paired[(3 * len(paired)) // 4], 4),
        ]
        rec = {
            "class": klass,
            "fen": fen,
            "args": TT_MB + extra,
            "reps": reps,
            "order": "counterbalanced per repetition",
            "base_wall_s": [r["wall_s"] for r in base],
            "after_wall_s": [r["wall_s"] for r in after],
            "base_wall_median_s": round(bw, 4),
            "after_wall_median_s": round(aw, 4),
            "wall_ratio": round(aw / bw, 4),
            "paired_ratio_quartiles": quartiles,
            "evals": reference["evals"],
            "base_rss_max_kb": max(r["rss_max_kb"] for r in base),
            "after_rss_max_kb": max(r["rss_max_kb"] for r in after),
            "base_minflt_median": int(statistics.median([r["minflt"] for r in base])),
            "after_minflt_median": int(statistics.median([r["minflt"] for r in after])),
            "stdout": reference["stdout"],
        }
        results[name] = rec
        print(
            f"{name:10s} {klass:12s} reps={reps:2d} "
            f"base={rec['base_wall_median_s']:.4f}s after={rec['after_wall_median_s']:.4f}s "
            f"ratio={rec['wall_ratio']:.3f} evals={rec['evals']} "
            f"rss {rec['base_rss_max_kb'] // 1024}->{rec['after_rss_max_kb'] // 1024}MB "
            f"minflt {rec['base_minflt_median']}->{rec['after_minflt_median']}",
            flush=True,
        )

    os.makedirs(os.path.dirname(args.out) or ".", exist_ok=True)
    with open(args.out, "w") as fh:
        json.dump({"cases": results}, fh, indent=2, sort_keys=True)
        fh.write("\n")
    print(f"wrote {args.out}")


if __name__ == "__main__":
    main()
