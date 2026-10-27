#!/usr/bin/env python3
"""Emit the plan1 before/after corpus table from the parsed state files.

Reads `state/baseline.json`, `state/after.json` (same-config CLI runs) and,
when present, `state/ab_wall.json` (interleaved A/B), and prints the markdown
table quoted in `report1.md` plus the derived bars (extra evals vs the
baseline's pre-phase evals, wall ratios).

Usage:
  python3 compare.py [--state state] > table.md
"""

import argparse
import json
import os
import sys

sys.dont_write_bytecode = True


def load(path):
    with open(path) as fh:
        return json.load(fh)["cases"]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--state", default="state")
    args = ap.parse_args()

    base = load(os.path.join(args.state, "baseline.json"))
    after = load(os.path.join(args.state, "after.json"))
    ab_path = os.path.join(args.state, "ab_wall.json")
    ab = load(ab_path) if os.path.exists(ab_path) else {}

    print("| case | class | base wall | after wall | ratio | base evals | after evals | delta |")
    print("|---|---|---:|---:|---:|---:|---:|---:|")
    for name, b in base.items():
        a = after[name]
        delta = a["evals"] - b["evals"]
        pct = 100.0 * delta / b["evals"] if b["evals"] else 0.0
        bw = ab.get(name, {}).get("base_wall_median_s", b["wall_median_s"])
        aw = ab.get(name, {}).get("after_wall_median_s", a["wall_median_s"])
        print(
            f"| {name} | {b['class']} | {bw:.4f} | {aw:.4f} | "
            f"{aw / bw:.3f} | {b['evals']} | {a['evals']} | "
            f"+{delta} ({pct:.3f}%) |"
        )

    print()
    print("| case | base outcome/len | after outcome/len | base status | after status |")
    print("|---|---|---|---|---|")
    for name, b in base.items():
        a = after[name]
        print(
            f"| {name} | {b['outcome']}/{b['pv_len']} | {a['outcome']}/{a['pv_len']} | "
            f"{b['pv_status']} | {a['pv_status']} |"
        )

    print()
    print("| case | base preflight | after preflight |")
    print("|---|---|---|")
    for name, b in base.items():
        print(f"| {name} | {b['preflight']} | {after[name]['preflight']} |")

    print()
    print("| case | base rss KB | after rss KB |")
    print("|---|---:|---:|")
    for name, b in base.items():
        a = after[name]
        print(f"| {name} | {b['rss_max_kb']} | {a['rss_max_kb']} |")


if __name__ == "__main__":
    main()
