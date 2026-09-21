#!/usr/bin/env python3
"""plan8 Phase 0 separation-table analysis (kill-gate + mix-shift checks).

Reads the `RESEARCH8_STATS` dump CSVs (feature-combination rows) and prints:
- the mass-weighted base rescue rate and the pre-registered kill-gate
  evaluation for every single feature (coverage >= 5% of cut-frame eval
  mass, rescue-rate lift >= 2x);
- all passing feature pairs (composite candidates);
- the key features of record for the epsilon mix-shift comparison.

Usage: analyze.py <stats.csv> [<stats.csv> ...]
"""

import csv
import sys
from collections import defaultdict
from itertools import combinations

FEATS = ["side", "clamp", "cut_primary", "ratio_bucket", "gap_bucket",
         "exit_gap_bucket", "depth_bucket", "rank_bucket"]
INTS = ("keys eval_mass rescued_keys rescued_mass drawres_keys drawres_mass "
        "dead_keys dead_mass resbefore_keys resbefore_mass").split()


def load(path):
    rows = list(csv.DictReader(open(path)))
    for r in rows:
        for k in INTS:
            r[k] = int(r[k])
    return rows


def base_rate(rows):
    rm = sum(r["rescued_mass"] for r in rows)
    dm = sum(r["dead_mass"] for r in rows)
    return rm / (rm + dm)


def agg(rows, fs, pred=lambda r: True):
    a = defaultdict(lambda: defaultdict(int))
    for r in rows:
        if not pred(r):
            continue
        for k in INTS:
            a[tuple(r[f] for f in fs)][k] += r[k]
    return a


def gate(rows, label):
    base = base_rate(rows)
    print(f"== {label} ==")
    print(f"base rescue rate (mass) = {base:.4f}; "
          f"gate: coverage >= 5% AND lift >= 2x (rate >= {2 * base:.4f})")
    any_pass = False
    for f in FEATS:
        a = agg(rows, [f])
        for b, r in sorted(a.items()):
            cov = r["rescued_mass"] + r["dead_mass"]
            cov_frac = cov / sum(x["rescued_mass"] + x["dead_mass"] for x in rows)
            if cov_frac < 0.05 or cov == 0:
                continue
            rr = r["rescued_mass"] / cov
            lift = rr / base
            mark = "  <-- PASS" if lift >= 2.0 else ""
            any_pass |= lift >= 2.0
            print(f"  {f}={b}: cov={cov_frac:.4f} rate={rr:.4f} "
                  f"lift={lift:.2f}{mark}")
    print(f"single-feature gate: {'FAILED' if any_pass else 'PASSED (no signal)'}")
    pairs = []
    for f1, f2 in combinations(FEATS, 2):
        a = agg(rows, [f1, f2])
        for b, r in a.items():
            cov = r["rescued_mass"] + r["dead_mass"]
            cov_frac = cov / sum(x["rescued_mass"] + x["dead_mass"] for x in rows)
            if cov_frac < 0.05 or cov == 0:
                continue
            rr = r["rescued_mass"] / cov
            if rr / base >= 2.0:
                pairs.append((cov_frac, rr / base, dict(zip((f1, f2), b))))
    for cov, lift, key in sorted(pairs, reverse=True):
        print(f"  PASS pair {key}: cov={cov:.4f} lift={lift:.2f}")
    print(f"composite gate: {'FAILED (separable buckets exist)' if pairs else 'PASSED'}")
    print()
    return any_pass or bool(pairs)


def mix_shift(rows, label):
    base = base_rate(rows)
    checks = [
        ("OR & depth<=4 & non-clamped",
         lambda r: r["side"] == "OR" and r["depth_bucket"] == "0" and r["clamp"] == "0"),
        ("AND & tie (ratio<1.25)",
         lambda r: r["side"] == "AND" and r["ratio_bucket"] == "0"),
        ("AND & non-clamped & tie",
         lambda r: r["side"] == "AND" and r["ratio_bucket"] == "0" and r["clamp"] == "0"),
    ]
    print(f"== {label} conditioning buckets ==")
    for name, pred in checks:
        a = agg(rows, ["side"], pred)
        for r in a.values():
            cov = r["rescued_mass"] + r["dead_mass"]
            rr = r["rescued_mass"] / cov if cov else 0.0
            print(f"  {name}: cov={cov / 1:.0f} mass, "
                  f"cov_frac={cov / sum(x['rescued_mass'] + x['dead_mass'] for x in rows):.4f} "
                  f"rate={rr:.4f} lift={rr / base:.2f}")
    print()


if __name__ == "__main__":
    for path in sys.argv[1:]:
        rows = load(path)
        gate(rows, path)
        mix_shift(rows, path)
