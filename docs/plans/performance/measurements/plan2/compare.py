#!/usr/bin/env python3
"""Render the plan2 tables and the pre-registered bar checks.

Reads `state/ab_wall.json` (authoritative interleaved A/B, 15 reps/case),
`state/ab_wall_preflight.json` (25-rep follow-up on the closure-decided
cells), `state/{baseline,after}.json` (sequential corpus, 5 runs/case) and
`state/{baseline,after}_sweep.json` (`--tt-size` sweep on mate-in-1), and
prints the markdown tables quoted in `report2.md` plus the bar verdicts.

Usage:
  python3 compare.py [--state state]
"""

import argparse
import json
import os
import sys

sys.dont_write_bytecode = True

# Pre-registered bars (docs/plans/performance/plan2.md).
SHORT_CASES = ("mate1", "easy_kq")
CLOSURE_CASES = ("draw_kr", "draw_cyc", "ladder_root")
DEEP_CASES = ("dec10", "m22", "m22_b", "stress_b")
SHORT_RSS_MB = 40
SHORT_WALL_MS = 5.0
SHORT_MIN_SPEEDUP = 10.0
CLOSURE_MIN_SPEEDUP = 1.0 / 0.9  # ">= 10% faster"
DEEP_CEILING = 1.06
# Deep cells touch the whole reservation in both arms, so their peak RSS must
# agree to within measurement noise (page-table/rusage granularity); a few
# hundred KiB of jitter is observed in both directions, run to run.
DEEP_RSS_NOISE_KB = 1024


def load(path):
    with open(path) as fh:
        return json.load(fh)


def speedup(r):
    return 1.0 / r["wall_ratio"] if r["wall_ratio"] < 0.95 else 0.0


def mb(kb):
    return kb // 1024


def render_ab(ab, title):
    print(f"### {title}\n")
    print(
        "| case | class | base wall (s) | after wall (s) | ratio | speedup "
        "| base evals | after evals | evals identical "
        "| RSS base → after (MB) | minflt base → after |"
    )
    print("|---|---|---:|---:|---:|---:|---:|---:|:---:|---:|---:|")
    for name, r in ab["cases"].items():
        sp = speedup(r)
        sp_str = f"{sp:.1f}×" if sp else "—"
        print(
            f"| `{name}` | {r['class']} | {r['base_wall_median_s']:.4f} "
            f"| {r['after_wall_median_s']:.4f} | {r['wall_ratio']:.3f} | {sp_str} "
            f"| {r['base_evals']:,} | {r['after_evals']:,} "
            f"| {'yes' if r['evals_identical'] else 'NO'} "
            f"| {mb(r['base_rss_max_kb'])} → {mb(r['after_rss_max_kb'])} "
            f"| {r['base_minflt_median']:,} → {r['after_minflt_median']:,} |"
        )
    print()


def render_corpus(base, after):
    print("### Sequential corpus (5 runs/case, same configuration)\n")
    print(
        "| case | class | base wall (s) | after wall (s) | base RSS (MB) | after RSS (MB) "
        "| base minflt | after minflt | evals (both arms) | outcome / pv_status |"
    )
    print("|---|---|---:|---:|---:|---:|---:|---:|---:|---|")
    for name, b in base.items():
        a = after[name]
        assert a["evals"] == b["evals"], f"{name}: eval drift"
        print(
            f"| `{name}` | {b['class']} | {b['wall_median_s']:.4f} | {a['wall_median_s']:.4f} "
            f"| {mb(b['rss_max_kb'])} | {mb(a['rss_max_kb'])} "
            f"| {b['minflt_median']:,} | {a['minflt_median']:,} | {b['evals']:,} "
            f"| {b['outcome']} / {b['pv_status']} |"
        )
    print()


def render_sweep(base, after):
    print("### `--tt-size` sweep on mate-in-1 (5 runs each)\n")
    print("| `--tt-size` | base wall (s) | after wall (s) | base RSS (MB) | after RSS (MB) | base minflt | after minflt |")
    print("|---:|---:|---:|---:|---:|---:|---:|")
    for name, b in base.items():
        a = after[name]
        print(
            f"| {b['tt_size_mb']} | {b['wall_median_s']:.4f} | {a['wall_median_s']:.4f} "
            f"| {mb(b['rss_max_kb'])} | {mb(a['rss_max_kb'])} "
            f"| {b['minflt_median']:,} | {a['minflt_median']:,} |"
        )
    print()


def render_bars(ab, pre, base, after):
    print("### Bar checks\n")
    checks = []

    # bar 1 (identity): eval identity on every cell + quick-suite totals.
    eval_ok = all(r["evals_identical"] for r in ab["cases"].values())
    checks.append(("1 identity: corpus child_evals identical", eval_ok, ""))

    # bar 2: short latency.
    for case in SHORT_CASES:
        r = ab["cases"][case]
        ok = r["after_wall_median_s"] * 1000 <= SHORT_WALL_MS and speedup(r) >= SHORT_MIN_SPEEDUP
        checks.append(
            (
                f"2 short latency `{case}`",
                ok,
                f"{r['after_wall_median_s'] * 1000:.2f} ms, {speedup(r):.1f}×",
            )
        )

    # bar 3: closure-class latency (>= 10% faster), 25-rep follow-up.
    for case in CLOSURE_CASES:
        r = pre["cases"][case]
        ok = speedup(r) >= CLOSURE_MIN_SPEEDUP
        saved_ms = (r["base_wall_median_s"] - r["after_wall_median_s"]) * 1000
        checks.append(
            (
                f"3 closure-class `{case}` (25 reps)",
                ok,
                f"{speedup(r) * 100 - 100:+.1f}% wall, {saved_ms:.0f} ms saved, "
                f"ratio {r['wall_ratio']:.3f}",
            )
        )

    # bar 4: memory.
    for case in SHORT_CASES + ("draw_kr", "draw_cyc", "ladder_root"):
        r = ab["cases"][case]
        ok = mb(r["after_rss_max_kb"]) <= SHORT_RSS_MB
        checks.append((f"4 RSS <= {SHORT_RSS_MB} MB `{case}`", ok, f"{mb(r['after_rss_max_kb'])} MB"))
    for case in DEEP_CASES:
        b = base[case]
        a = after[case]
        ok = a["rss_max_kb"] <= b["rss_max_kb"] + DEEP_RSS_NOISE_KB
        delta_kb = a["rss_max_kb"] - b["rss_max_kb"]
        checks.append(
            (
                f"4 deep RSS <= baseline `{case}` (+1 MB noise)",
                ok,
                f"{b['rss_max_kb']} → {a['rss_max_kb']} KB ({delta_kb:+} KB)",
            )
        )

    # bar 5: deep throughput ceiling over the interleaved medians.
    worst = max(DEEP_CASES, key=lambda c: ab["cases"][c]["wall_ratio"])
    worst_ratio = ab["cases"][worst]["wall_ratio"]
    checks.append(
        (
            f"5 deep ceiling max ratio <= {DEEP_CEILING}",
            worst_ratio <= DEEP_CEILING,
            f"max = {worst_ratio:.3f} at `{worst}`",
        )
    )

    print("| bar | verdict | measured |")
    print("|---|---|---|")
    for label, ok, detail in checks:
        print(f"| {label} | {'PASS' if ok else '**MISS**'} | {detail} |")
    print()
    return all(ok for _, ok, _ in checks)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--state", default="state")
    args = ap.parse_args()
    s = args.state

    ab = load(os.path.join(s, "ab_wall.json"))
    pre = load(os.path.join(s, "ab_wall_preflight.json"))
    base = load(os.path.join(s, "baseline.json"))["cases"]
    after = load(os.path.join(s, "after.json"))["cases"]
    base_sweep = load(os.path.join(s, "baseline_sweep.json"))["cases"]
    after_sweep = load(os.path.join(s, "after_sweep.json"))["cases"]

    render_ab(ab, f"Interleaved A/B, {ab['reps']} reps/case (authoritative wall)")
    render_ab(pre, f"Interleaved A/B follow-up on the closure-decided cells, {pre['reps']} reps/case")
    render_corpus(base, after)
    render_sweep(base_sweep, after_sweep)
    ok = render_bars(ab, pre, base, after)
    print(f"ALL BARS PASS: {ok}")


if __name__ == "__main__":
    main()
