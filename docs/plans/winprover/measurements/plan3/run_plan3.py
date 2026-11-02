#!/usr/bin/env python3
"""Plan 3 driver: canonical bench (opt vs opt+win) plus front-end accounting.

The canonical benchmark is delegated to the initiative's committed driver
(`../bench/run_bench.py`) so the ledger stays comparable: same corpus, same
10 s / 192 MB budget, same 3 repeats, same provari `--guide hand` reference.
This script adds the one thing the CLI cannot report separately: the win
front-end's own child evaluations and frames, measured by `probe.rs` with the
same 1,000,000-eval cap.

Outputs (under this directory, all parsed — no raw transcripts):

- `results.json`  — the bench driver's per-run rows and ledger;
- `front_end.json` — per-cell front-end work, plus the general-search eval
  totals from the `opt` and `optwin` candidate runs;
- `results.csv`   — the ledger cells as a flat table.

Usage:
    python3 run_plan3.py --provari /tmp/winprover-oracle/target/release/provari \\
        --solver ./target/release/atomic_solver
    python3 run_plan3.py --skip-bench   # re-derive front_end.json/results.csv

`--dry-run` prints the commands and writes nothing.
"""

from __future__ import annotations

import argparse
import glob
import json
import subprocess
import sys
import tempfile
from pathlib import Path

sys.dont_write_bytecode = True

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[4]
BENCH = HERE.parent / "bench" / "run_bench.py"
DEFAULT_OUT = HERE / "results.json"
FRONT_OUT = HERE / "front_end.json"
CSV_OUT = HERE / "results.csv"

CELLS = [
    ("core", "core", "r1b1kbnr/ppp3pp/n4q2/3pppN1/1P1PP3/8/P1P2PPP/RNBQKB1R w KQkq - 0 1"),
    ("easy-win", "1eb80b92f92c",
     "rnbq1b1r/pp1p1kpp/2p1pp1n/8/3PP1P1/2N2N2/PPP2P1P/R1BQKB1R w KQ - 0 1"),
    ("deep-win", "db6a741c723f",
     "rnb1kbnr/pppp2pp/5q2/4ppN1/3P4/8/PPP1PPPP/RNBQKB1R w KQkq - 0 1"),
]

OPT = "--move-class-pn --quiet-grouping --eval-pn or"
OPTWIN = OPT + " --win-front-end"
PROVARI_COMMIT = "48686bc6c786d6ea70568f0b2ad4002cf0520206"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--provari", required=True, help="pinned provari oracle")
    parser.add_argument("--solver", required=True, help="atomic_solver release binary")
    parser.add_argument("--out", default=str(DEFAULT_OUT))
    parser.add_argument("--budget", type=int, default=1_000_000,
                        help="front-end child-eval cap (default: 1000000)")
    parser.add_argument("--skip-bench", action="store_true",
                        help="reuse results.json instead of re-running the bench")
    parser.add_argument("--dry-run", action="store_true")
    return parser.parse_args()


def bench_command(args: argparse.Namespace) -> list[str]:
    return [
        sys.executable, str(BENCH),
        "--provari", args.provari,
        "--solver", args.solver,
        "--cases", ",".join(cid for _, cid, _ in CELLS),
        "--repeats", "3",
        "--stack", f"opt={OPT}",
        "--stack", f"optwin={OPTWIN}",
        "--provari-commit", PROVARI_COMMIT,
        "--solver-commit", "d623b21 working tree (winprover plan3)",
        "--out", args.out,
    ]


def build_probe(dry_run: bool) -> Path | None:
    probes = sorted(
        (REPO / "target" / "release" / "deps").glob("libatomic_solver-*.rlib"),
        key=lambda p: p.stat().st_mtime,
    )
    if not probes:
        raise SystemExit("no target/release/deps/libatomic_solver-*.rlib; run cargo build --release")
    out = Path(tempfile.mkdtemp(prefix="plan3-probe-")) / "probe"
    cmd = [
        "rustc", "--edition", "2024", "-O", "-C", "panic=abort", "-C", "lto=thin",
        str(HERE / "probe.rs"), "-o", str(out),
        "-L", str(REPO / "target" / "release" / "deps"),
        "--extern", f"atomic_solver={probes[-1]}",
    ]
    print("BUILD " + " ".join(cmd))
    if not dry_run:
        subprocess.run(cmd, check=True, cwd=REPO)
    return out


def run_probe(probe: Path, fen: str, budget: int) -> dict:
    text = subprocess.run(
        [str(probe), fen, str(budget)],
        capture_output=True, text=True, check=True,
    ).stdout
    return json.loads(text.strip().splitlines()[-1])


def main() -> int:
    args = parse_args()
    probe = build_probe(args.dry_run)

    if not args.skip_bench:
        cmd = bench_command(args)
        print("RUN " + " ".join(cmd))
        if args.dry_run:
            return 0
        subprocess.run(cmd, check=True, cwd=REPO)

    results = json.loads(Path(args.out).read_text())
    front_rows = []
    for short, cid, fen in CELLS:
        measured = run_probe(probe, fen, args.budget)
        cell = results["ledger"]["cells"][cid]
        front_rows.append({
            "cell": short,
            "id": cid,
            "fen": fen,
            "budget": args.budget,
            "front_end": measured,
            "opt_evals": cell["opt"]["evals"],
            "optwin_evals": cell["optwin"]["evals"],
            "opt_wall_s": cell["opt"]["wall_s"],
            "optwin_wall_s": cell["optwin"]["wall_s"],
            "opt_factor": cell["opt"].get("factor"),
            "optwin_factor": cell["optwin"].get("factor"),
            "optwin_censored": not cell["optwin"]["win"],
        })

    payload = {
        "provenance": results["provenance"],
        "front_end_cap": args.budget,
        "cells": front_rows,
        "ledger": results["ledger"]["summary"],
    }
    Path(FRONT_OUT).write_text(json.dumps(payload, indent=1) + "\n")

    lines = ["cell,stack,wall_s,evals,factor,note"]
    for row in front_rows:
        cell = results["ledger"]["cells"][row["id"]]
        for stack in ("opt", "optwin"):
            entry = cell[stack]
            factor = entry.get("factor")
            lines.append(
                f'{row["cell"]},{stack},{entry["wall_s"]},{entry["evals"]},'
                f'{"" if factor is None else factor},{entry["note"]}'
            )
    Path(CSV_OUT).write_text("\n".join(lines) + "\n")

    print(f"wrote {FRONT_OUT} and {CSV_OUT}")
    for name, summary in payload["ledger"].items():
        if name in ("opt", "optwin"):
            print(
                f"{name}: shared={summary['shared_count']} "
                f"geomean={summary['geomean_factor']} "
                f"coverage={summary['coverage_candidate']}/"
                f"{summary['coverage_reference']}"
            )
    return 0


if __name__ == "__main__":
    sys.exit(main())
