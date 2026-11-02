#!/usr/bin/env python3
"""Plan 5 driver: default-flag decision (quick suite) plus the canonical
head-to-head bench.

Two measurements, one record:

1. **Quick-suite comparison** (the default-flag decision rule). The 59-case
   quick suite is the shipped fast benchmark set: every `decisive_positions.txt`
   case plus the `move_order_positions.txt` cases named `mNN` with `NN >= 23`
   (the same selection `examples/benchmark.rs::quick_suite` uses). Each case is
   run once per stack through the product CLI at the suite's canonical
   settings (`--first-outcome --timeout 3`, release build, default epsilon and
   TT label), for the old shipped default and for the opt stack
   (`--move-class-pn --quiet-grouping --eval-pn or`). Outcomes are compared
   case-by-case against the fixture's expected outcome and against each other.

2. **Canonical bench** (the `bench.md` ledger row). Delegated to the committed
   initiative driver (`../bench/run_bench.py`) so the row stays comparable:
   same frozen corpus, same 10 s / 192 MB budget, same three repeats, same
   provari `--guide hand` reference.

Outputs (all parsed -- no raw transcripts, no `__pycache__`):

- `results.json` -- provenance, the quick-suite table and its summary, and the
  bench driver's per-run rows plus derived ledger;
- `results.csv`  -- the bench ledger cells as a flat table.

Usage:
    python3 run_plan5.py --provari /tmp/winprover-oracle/target/release/provari \\
        --solver ./target/release/atomic_solver
    python3 run_plan5.py --skip-bench   # re-derive results.json/results.csv

`--dry-run` prints the commands and writes nothing.
"""

from __future__ import annotations

import argparse
import csv
import json
import datetime
import platform
import subprocess
import sys
import time
from pathlib import Path

sys.dont_write_bytecode = True

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[4]
BENCH = HERE.parent / "bench" / "run_bench.py"
DEFAULT_OUT = HERE / "results.json"
CSV_OUT = HERE / "results.csv"
FIXTURES = REPO / "tests" / "fixtures"

CELLS = [
    ("core", "core", "r1b1kbnr/ppp3pp/n4q2/3pppN1/1P1PP3/8/P1P2PPP/RNBQKB1R w KQkq - 0 1"),
    ("easy-win", "1eb80b92f92c",
     "rnbq1b1r/pp1p1kpp/2p1pp1n/8/3PP1P1/2N2N2/PPP2P1P/R1BQKB1R w KQ - 0 1"),
    ("deep-win", "db6a741c723f",
     "rnb1kbnr/pppp2pp/5q2/4ppN1/3P4/8/PPP1PPPP/RNBQKB1R w KQkq - 0 1"),
]

# The two stacks the default decision compares. `default` is the shipped flag
# set (no experimental flags); `opt` is the best measured opt-in stack.
DEFAULT_STACK = ""
OPT_STACK = "--move-class-pn --quiet-grouping --eval-pn or"
PROVARI_COMMIT = "48686bc6c786d6ea70568f0b2ad4002cf0520206"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--provari", required=True, help="pinned provari oracle")
    parser.add_argument("--solver", required=True, help="atomic_solver release binary")
    parser.add_argument("--out", default=str(DEFAULT_OUT))
    parser.add_argument("--timeout", type=float, default=3.0,
                        help="quick-suite per-case timeout (default: 3 s)")
    parser.add_argument("--quick-repeats", type=int, default=1,
                        help="quick-suite runs per case per stack (default: 1)")
    parser.add_argument("--skip-quick", action="store_true",
                        help="reuse the quick-suite section instead of re-running it")
    parser.add_argument("--skip-bench", action="store_true",
                        help="reuse the bench section instead of re-running it")
    parser.add_argument("--dry-run", action="store_true")
    return parser.parse_args()


def parse_fixture(path: Path) -> list[dict]:
    cases = []
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split(";")
        name = parts[0].strip()
        fen = parts[1].strip() if len(parts) > 1 else ""
        expected = parts[2].strip() if len(parts) > 2 and parts[2].strip() else None
        if name and fen:
            cases.append({"name": name, "fen": fen, "expected": expected})
    return cases


def move_order_number(name: str) -> int | None:
    prefix = name.split("_", 1)[0]
    if not prefix.startswith("m"):
        return None
    try:
        return int(prefix[1:])
    except ValueError:
        return None


def quick_suite() -> list[dict]:
    """The `examples/benchmark.rs::quick_suite` selection, from the fixtures."""
    cases = parse_fixture(FIXTURES / "decisive_positions.txt")
    cases += [
        case
        for case in parse_fixture(FIXTURES / "move_order_positions.txt")
        if (n := move_order_number(case["name"])) is not None and n >= 23
    ]
    return cases


def quick_command(args: argparse.Namespace, case: dict, flags: list[str]) -> list[str]:
    return [
        args.solver,
        "--fen", case["fen"],
        "--timeout", str(int(args.timeout)),
        "--first-outcome",
        "--outcome-only",
        *flags,
    ]


def parse_quick_output(text: str) -> dict:
    """Parse the CLI's stdout+stderr.

    `outcome:`/`pv_status:`/`pre_exit:` are on stdout, `evals:` on stderr
    (the CLI keeps the outcome-only stdout contract byte-exact), and the
    timeout / budget marker is a bare stdout line after the outcome.
    """
    row = {"outcome": "none", "evals": None, "pv_status": None, "exit": None}
    for line in text.splitlines():
        line = line.strip()
        if line.startswith("outcome:"):
            words = line.split(":", 1)[1].split()
            row["outcome"] = words[0] if words else "none"
            row["pv_length"] = int(words[-1]) if words and words[-1].isdigit() else None
        elif line.startswith("evals:"):
            row["evals"] = int(line.split(":", 1)[1].strip())
        elif line.startswith("pv_status:"):
            row["pv_status"] = line.split(":", 1)[1].strip()
        elif line.startswith("pre_exit:"):
            for part in line.split(":", 1)[1].split():
                if part.startswith("reason="):
                    row["exit"] = part.split("=", 1)[1]
        elif line in ("timeout", "budget exhausted", "quit"):
            row["exit"] = line.replace(" ", "-")
    return row


def run_quick_case(args: argparse.Namespace, case: dict,
                   stack: str, flags: list[str]) -> dict:
    cmd = quick_command(args, case, flags)
    if args.dry_run:
        print(f"QUICK {stack} " + " ".join(cmd))
        return {"dry_run": True}
    runs = []
    for _ in range(args.quick_repeats):
        start = time.perf_counter()
        proc = subprocess.run(cmd, capture_output=True, text=True,
                              timeout=args.timeout + 60, check=False)
        wall = time.perf_counter() - start
        parsed = parse_quick_output(proc.stdout + "\n" + proc.stderr)
        parsed["wall_s"] = round(wall, 3)
        parsed["returncode"] = proc.returncode
        runs.append(parsed)
    first = dict(runs[0])
    first["repeats"] = len(runs)
    first["outcome_every_repeat"] = all(r["outcome"] == first["outcome"]
                                        for r in runs)
    first["evals_every_repeat"] = all(r["evals"] == first["evals"]
                                      for r in runs)
    first["wall_s_max"] = max(r["wall_s"] for r in runs)
    return first


def quick_table(args: argparse.Namespace) -> dict:
    cases = quick_suite()
    rows = []
    for case in cases:
        default = run_quick_case(args, case, "default", DEFAULT_STACK.split())
        opt = run_quick_case(args, case, "opt", OPT_STACK.split())
        if args.dry_run:
            continue
        expected = case["expected"]
        expected_outcome = expected.lower() if expected else None
        # `wrong` is the fixture's decisive-mismatch rule (a Draw means the
        # case was censored, not wrong; `examples/benchmark.rs::json_status`).
        # A *regression* is stricter: the opt stack must not lose an outcome
        # the default stack produced, and must not become wrong where the
        # default is right.
        wrong = {name: (expected_outcome is not None
                        and run["outcome"] not in ("draw", expected_outcome))
                 for name, run in (("default", default), ("opt", opt))}
        regression = (
            (default["outcome"] != opt["outcome"] and default["outcome"] != "draw")
            or (wrong["opt"] and not wrong["default"])
        )
        rows.append({
            "name": case["name"],
            "fen": case["fen"],
            "expected": expected,
            "default": default,
            "opt": opt,
            "wrong": wrong,
            "outcome_changed": default["outcome"] != opt["outcome"],
            "regression": regression,
        })

    regressions = [r["name"] for r in rows if r["regression"]]
    changed = [r["name"] for r in rows if r["outcome_changed"]]
    return {
        "cases": rows,
        "summary": {
            "case_count": len(rows),
            "default_total_evals": sum(r["default"]["evals"] or 0 for r in rows),
            "opt_total_evals": sum(r["opt"]["evals"] or 0 for r in rows),
            "eval_identical": sum(1 for r in rows
                                  if r["default"]["evals"] == r["opt"]["evals"]),
            "outcome_changed": changed,
            "regressions": regressions,
            "wrong_default": [r["name"] for r in rows if r["wrong"]["default"]],
            "wrong_opt": [r["name"] for r in rows if r["wrong"]["opt"]],
            "promote": not regressions,
        },
    }


def bench_command(args: argparse.Namespace, bench_out: Path,
                  solver_commit: str) -> list[str]:
    return [
        sys.executable, str(BENCH),
        "--provari", args.provari,
        "--solver", args.solver,
        "--cases", ",".join(cid for _, cid, _ in CELLS),
        "--repeats", "3",
        # `run_bench.py` always adds its `default` stack (no flags); only the
        # opt stack is appended here so the default is not measured twice.
        "--stack", f"opt={OPT_STACK}",
        "--provari-commit", PROVARI_COMMIT,
        "--solver-commit", solver_commit,
        "--out", str(bench_out),
    ]


def flat_ledger(bench: dict) -> list[dict]:
    rows = []
    cells = bench["ledger"]["cells"]
    for cell_name, cell_id, _ in CELLS:
        cell = cells[cell_id]
        row = {
            "cell": cell_name,
            "reference_wall_s": cell["reference"]["wall_s"],
            "reference_nodes": cell["reference"]["nodes"],
            "reference_win": cell["reference"]["win"],
        }
        for stack in ("default", "opt"):
            entry = cell[stack]
            row[f"{stack}_wall_s"] = entry["wall_s"]
            row[f"{stack}_evals"] = entry["evals"]
            row[f"{stack}_factor"] = entry["factor"]
            row[f"{stack}_lower_bound"] = entry.get("factor_lower_bound")
            row[f"{stack}_note"] = entry["note"]
        rows.append(row)
    return rows


def write_csv(path: Path, rows: list[dict]) -> None:
    if not rows:
        path.write_text("")
        return
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)


def main() -> int:
    args = parse_args()
    out = Path(args.out)
    previous: dict = {}
    if out.exists():
        previous = json.loads(out.read_text())

    quick = previous.get("quick_suite")
    if not args.skip_quick:
        quick = quick_table(args)

    bench = previous.get("bench")
    if not args.skip_bench:
        solver_commit = subprocess.run(
            ["git", "-C", str(REPO), "rev-parse", "HEAD"],
            capture_output=True, text=True, check=False,
        ).stdout.strip() or "unknown"
        bench_out = HERE / ".bench-results.json"
        cmd = bench_command(args, bench_out, solver_commit)
        print("RUN " + " ".join(cmd))
        if args.dry_run:
            return 0
        subprocess.run(cmd, check=True, cwd=REPO)
        bench = json.loads(bench_out.read_text())
        bench_out.unlink()

    if args.dry_run:
        return 0

    result = {
        "provenance": {
            "date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d"),
            "host": platform.node(),
            "platform": platform.platform(),
            "solver_commit": bench["provenance"]["solver"]["commit"] if bench else None,
            "solver": args.solver,
            "provari": {
                "commit": PROVARI_COMMIT,
                "binary": args.provari,
                "guide": "hand",
            },
            "stacks": {"default": DEFAULT_STACK, "opt": OPT_STACK},
            "quick_suite": {
                "fixtures": [
                    "tests/fixtures/decisive_positions.txt",
                    "tests/fixtures/move_order_positions.txt (mNN, NN >= 23)",
                ],
                "timeout_s": args.timeout,
                "first_outcome": True,
                "repeats": args.quick_repeats,
            },
            "bench": {
                "budget_seconds": 10.0,
                "memory_mb": 192,
                "repeats": 3,
                "cells": [name for name, _, _ in CELLS],
            },
        },
        "quick_suite": quick,
        "bench": bench,
    }
    out.write_text(json.dumps(result, indent=1) + "\n")
    if bench:
        write_csv(CSV_OUT, flat_ledger(bench))
    if quick:
        summary = quick["summary"]
        print(f"quick: cases={summary['case_count']} "
              f"changed={len(summary['outcome_changed'])} "
              f"regressions={len(summary['regressions'])} "
              f"promote={summary['promote']}")
    if bench:
        for name, summary in bench["ledger"]["summary"].items():
            print(f"{name}: shared={summary['shared_count']} "
                  f"geomean={summary['geomean_factor']} "
                  f"coverage={summary['coverage_candidate']}"
                  f"/{summary['coverage_reference']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
