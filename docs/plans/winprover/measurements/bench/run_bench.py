#!/usr/bin/env python3
"""Bench driver for `docs/plans/winprover/bench.md`.

Measures the two sides of the bench ledger on one host, sequentially, one
process at a time:

- **reference:** the pinned provari oracle with the hand guide
  (`--guide hand`), the initiative's primary column;
- **candidates:** `atomic_solver` under one or more named flag stacks
  (`--stack NAME=FLAGS`, repeatable).

Both sides get the same nominal budget (10 s, 192 MB label) and the same
repeat count. Only parsed rows are written; raw stdout/stderr stay in memory
(no transcripts, no `__pycache__`).

The driver writes `results.json` with per-run rows plus the derived ledger:

    factor(cell) = median_candidate_wall / median_reference_wall

`bench.md` defines the bench set, the censoring rules and how the ledger is
read. The corpus file is the pinned `provari` `sample20` definition (path and
digest recorded by `bench.md`); this driver never edits it.

Usage:
    python3 run_bench.py --provari /tmp/provari-bench/target/release/provari \\
        --solver ./target/release/atomic_solver \\
        --stack default= --stack opt="--move-class-pn --quiet-grouping --eval-pn or"

`--dry-run` prints the exact commands without running anything.

Justification for a single >10 KB file: corpus loading, command construction,
output parsing, provenance capture, and the derived per-cell ledger are one
measurement contract. Splitting them would separate a command from the parser
that interprets its output, which is the failure mode this driver exists to
prevent (same reasoning as `provari/measurements/plan1/run_baseline.py`).
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import statistics
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

sys.dont_write_bytecode = True

HERE = Path(__file__).resolve().parent
DEFAULT_CORPUS = (
    HERE.parents[2] / "provari" / "measurements" / "plan1" / "corpus.json"
)
DEFAULT_OUT = HERE / "results.json"
CORE_ID = "core"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--provari", required=True, help="provari binary")
    parser.add_argument("--solver", required=True, help="atomic_solver binary")
    parser.add_argument("--corpus", default=str(DEFAULT_CORPUS))
    parser.add_argument("--out", default=str(DEFAULT_OUT))
    parser.add_argument("--seconds", type=float, default=10.0)
    parser.add_argument("--memory", type=int, default=192, metavar="MB")
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument(
        "--cases",
        default="",
        help="comma-separated case ids (default: the whole corpus)",
    )
    parser.add_argument(
        "--stack",
        action="append",
        default=[],
        metavar="NAME=FLAGS",
        help="named solver flag stack (repeatable; default: no flags)",
    )
    parser.add_argument("--provari-commit", default="unknown")
    parser.add_argument("--solver-commit", default="unknown")
    parser.add_argument("--date", default=None)
    parser.add_argument("--dry-run", action="store_true")
    return parser.parse_args()


def load_cases(path: Path) -> list[dict]:
    data = json.loads(path.read_text())
    return [data["core"], *data["sample20"]]


def provari_command(args: argparse.Namespace, fen: str) -> list[str]:
    return [
        args.provari, "prove", fen,
        "--seconds", str(int(args.seconds)),
        "--memory", str(args.memory),
        "--guide", "hand",
    ]


def solver_command(args: argparse.Namespace, fen: str, flags: list[str]) -> list[str]:
    return [
        args.solver,
        "--fen", fen,
        "--timeout", str(int(args.seconds)),
        "--tt-size", str(args.memory),
        "--first-outcome",
        "--outcome-only",
        *flags,
    ]


def run_process(cmd: list[str], timeout: float) -> tuple[int, str, float]:
    start = time.perf_counter()
    try:
        proc = subprocess.run(
            cmd, capture_output=True, text=True, timeout=timeout + 60, check=False
        )
        wall = time.perf_counter() - start
        return proc.returncode, proc.stdout + proc.stderr, wall
    except subprocess.TimeoutExpired:
        return 124, "", time.perf_counter() - start


def last_json(text: str) -> dict | None:
    for line in reversed(text.splitlines()):
        line = line.strip()
        if line.startswith("{"):
            try:
                return json.loads(line)
            except json.JSONDecodeError:
                continue
    return None


def parse_solver(text: str) -> dict:
    row = {"outcome": "none", "evals": None, "exit": None, "pv_status": None}
    for line in text.splitlines():
        line = line.strip()
        if line.startswith("outcome:"):
            # `outcome: win length: 29` / `outcome: draw length: 0`
            words = line.split(":", 1)[1].split()
            row["outcome"] = words[0] if words else "none"
            row["pv_length"] = int(words[-1]) if words[-1].isdigit() else None
        elif line.startswith("evals:"):
            row["evals"] = int(line.split(":", 1)[1].strip())
        elif line.startswith("pv_status:"):
            row["pv_status"] = line.split(":", 1)[1].strip()
        elif line.startswith("pre_exit:"):
            for part in line.split(":", 1)[1].split():
                if "=" in part:
                    key, value = part.split("=", 1)
                    if key == "exit_reason":
                        row["exit"] = value
    return row


def reference_row(args: argparse.Namespace, case: dict) -> list[dict]:
    rows = []
    for index in range(args.repeats):
        cmd = provari_command(args, case["fen"])
        if args.dry_run:
            print("REF  " + " ".join(cmd))
            continue
        code, text, wall = run_process(cmd, args.seconds)
        payload = last_json(text) or {}
        rows.append({
            "case": case["id"],
            "side": "provari-hand",
            "run": index + 1,
            "wall_s": round(wall, 3),
            "returncode": code,
            "win": bool(payload.get("found")),
            "mate": payload.get("mate"),
            "nodes": payload.get("nodes"),
            "ms": payload.get("ms"),
        })
    return rows


def candidate_rows(args: argparse.Namespace, case: dict, name: str,
                   flags: list[str]) -> list[dict]:
    rows = []
    for index in range(args.repeats):
        cmd = solver_command(args, case["fen"], flags)
        if args.dry_run:
            print(f"CAND " + " ".join(cmd))
            continue
        code, text, wall = run_process(cmd, args.seconds)
        parsed = parse_solver(text)
        rows.append({
            "case": case["id"],
            "side": f"atomic_solver:{name}",
            "run": index + 1,
            "wall_s": round(wall, 3),
            "returncode": code,
            "win": parsed["outcome"] == "win",
            "outcome": parsed["outcome"],
            "pv_length": parsed.get("pv_length"),
            "evals": parsed["evals"],
            "exit": parsed["exit"],
            "pv_status": parsed["pv_status"],
            "flags": flags,
        })
    return rows


def medians(rows: list[dict], side: str) -> dict[str, dict]:
    by_case: dict[str, list[dict]] = {}
    for row in rows:
        if row["side"] == side:
            by_case.setdefault(row["case"], []).append(row)
    return {
        case: {
            "runs": len(group),
            "wall_s": statistics.median(r["wall_s"] for r in group),
            # A censored run must not be read as a latency: a side's cell is
            # a win only if every repeat won.
            "win": all(r["win"] for r in group),
            "nodes": next((r["nodes"] for r in group if r.get("nodes")), None),
            "mate": next((r["mate"] for r in group if r.get("mate")), None),
            "evals": next((r["evals"] for r in group if r.get("evals")), None),
            "exit": next((r["exit"] for r in group if r.get("exit")), None),
            "returncodes": sorted({r["returncode"] for r in group}),
        }
        for case, group in by_case.items()
    }


def ledger(case_ids: list[str], ref: dict[str, dict],
           cands: dict[str, dict[str, dict]], seconds: float) -> dict:
    """Per-case factors and the shared-win geometric mean per stack."""
    cells: dict[str, dict] = {}
    for case in case_ids:
        cell: dict = {"reference": ref[case]}
        for name, table in cands.items():
            entry = dict(table[case])
            if not ref[case]["win"]:
                entry["factor"] = None
                entry["note"] = "no reference win (provari hand censored)"
            elif not entry["win"]:
                # Lower bound only: we needed more than the budget.
                entry["factor"] = None
                entry["factor_lower_bound"] = round(seconds / ref[case]["wall_s"], 2)
                entry["note"] = "candidate censored"
            else:
                entry["factor"] = round(entry["wall_s"] / ref[case]["wall_s"], 3)
                entry["note"] = "shared win"
            cell[name] = entry
        cells[case] = cell

    summary = {}
    for name in cands:
        shared = [c for c in case_ids
                  if cells[c][name]["note"] == "shared win"]
        factors = [cells[c][name]["factor"] for c in shared]
        geomean = None
        if factors:
            geomean = round(
                statistics.geometric_mean(factors), 3) if len(factors) > 1 \
                else factors[0]
        summary[name] = {
            "shared_cells": shared,
            "shared_count": len(shared),
            "geomean_factor": geomean,
            "coverage_reference": sum(1 for c in case_ids if ref[c]["win"]),
            "coverage_candidate": sum(1 for c in case_ids
                                      if cells[c][name]["win"]),
            "censored_cells": [c for c in case_ids
                               if not cells[c][name]["win"]],
        }
    return {"cells": cells, "summary": summary}


def main() -> int:
    args = parse_args()
    stacks = [("default", [])]
    for spec in args.stack:
        name, _, flags = spec.partition("=")
        stacks.append((name, flags.split()))
    flag_of = dict(stacks)

    committed = str(subprocess.run(
        ["git", "-C", str(HERE), "rev-parse", "HEAD"],
        capture_output=True, text=True, check=False).stdout.strip() or "unknown")
    cases = load_cases(Path(args.corpus))
    if args.cases:
        wanted = [name.strip() for name in args.cases.split(",") if name.strip()]
        known = {case["id"] for case in cases}
        missing = [name for name in wanted if name not in known]
        if missing:
            raise SystemExit(f"unknown case ids: {missing}")
        cases = [case for case in cases if case["id"] in wanted]
    case_ids = [case["id"] for case in cases]

    rows: list[dict] = []
    for case in cases:
        rows += reference_row(args, case)
        for name, flags in stacks:
            rows += candidate_rows(args, case, name, flags)
    if args.dry_run:
        return 0

    ref = medians(rows, "provari-hand")
    cands = {name: medians(rows, f"atomic_solver:{name}") for name, _ in stacks}
    result = {
        "provenance": {
            "date": args.date or datetime.now(timezone.utc).strftime("%Y-%m-%d"),
            "host": platform.node(),
            "platform": platform.platform(),
            "cpu_count": os.cpu_count(),
            "provari": {
                "commit": args.provari_commit,
                "guide": "hand",
                "binary": args.provari,
            },
            "solver": {"commit": args.solver_commit, "binary": args.solver},
            "stacks": flag_of,
            "budgets": {"seconds": args.seconds, "memory_mb": args.memory,
                        "repeats": args.repeats},
            "corpus": args.corpus,
            "bench_commit": committed,
        },
        "runs": rows,
        "ledger": ledger(case_ids, ref, cands, args.seconds),
    }
    Path(args.out).write_text(json.dumps(result, indent=1) + "\n")
    print(f"wrote {args.out}")
    for name, summary in result["ledger"]["summary"].items():
        print(f"{name}: shared={summary['shared_count']} "
              f"geomean={summary['geomean_factor']} "
              f"coverage={summary['coverage_candidate']}"
              f"/{summary['coverage_reference']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
