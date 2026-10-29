#!/usr/bin/env python3
"""Reproducible provari vs atomic_solver baseline driver (provari plan1).

The corpus is committed beside this script (corpus.json).  Every run is
executed sequentially, one process at a time, with the same nominal budget on
both sides.  Only parsed rows are written; raw stdout/stderr transcripts are
kept in memory and never persisted (the repo ignores them, and this script
does not create them).

Usage:
    python3 run_baseline.py \
        --provari /tmp/provari-oracle/target/release/provari \
        --solver ./target/release/atomic_solver \
        --provari-commit <sha> --solver-commit <sha>

Use --dry-run to print the exact commands without running anything.

Justification for a single >10 KB file: corpus loading, command construction,
output parsing, provenance capture, proof checking, and run summarisation are
one measurement contract.  Splitting them would separate a command from the
parser that interprets its output, which is exactly the failure mode this
driver exists to prevent.
"""

from __future__ import annotations

import argparse
import json
import platform
import re
import statistics
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

sys.dont_write_bytecode = True

HERE = Path(__file__).resolve().parent
DEFAULT_CORPUS = HERE / "corpus.json"
DEFAULT_OUT = HERE / "results.json"

OUTCOME_RE = re.compile(r"^outcome: (win|loss|draw) length: (\d+)$")
PV_RE = re.compile(r"^pv: (.*)$")
PV_STATUS_RE = re.compile(r"^pv_status: (.*)$")
PRE_EXIT_RE = re.compile(r"^pre_exit: reason=(\S+) outcome=(\S+) nodes=(\d+)$")
EVALS_RE = re.compile(r"^evals: (\d+)$")
CUT_LINES = {"timeout", "quit", "budget exhausted"}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run the committed provari/atomic_solver baseline corpus.",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument("--provari", required=True, help="provari binary")
    parser.add_argument("--solver", required=True, help="atomic_solver binary")
    parser.add_argument("--corpus", default=str(DEFAULT_CORPUS))
    parser.add_argument("--out", default=str(DEFAULT_OUT))
    parser.add_argument("--seconds", type=float, default=10.0)
    parser.add_argument("--memory", type=int, default=192, metavar="MB")
    parser.add_argument("--repeat", type=int, default=3,
                        help="core-case repeats per solver/configuration")
    parser.add_argument("--guide", choices=("model", "hand", "none"),
                        default="model", help="provari guide for every run")
    parser.add_argument("--provari-commit", default="unknown")
    parser.add_argument("--provari-features", default="bundled")
    parser.add_argument("--solver-commit", default="unknown")
    parser.add_argument("--date", default=None, help="ISO date (default: UTC today)")
    parser.add_argument("--host", default=None)
    parser.add_argument("--no-proof-check", action="store_true",
                        help="skip the core provari --proofs/check validation")
    parser.add_argument("--no-hand-control", action="store_true",
                        help="skip the 3-run core provari --guide hand control")
    parser.add_argument("--dry-run", action="store_true",
                        help="print commands instead of running them")
    return parser.parse_args()


def fmt_seconds(value: float) -> str:
    return str(int(value)) if value == int(value) else str(value)


def provari_command(args: argparse.Namespace, fen: str, guide: str | None = None) -> list[str]:
    return [
        args.provari, "prove", fen,
        "--seconds", fmt_seconds(args.seconds),
        "--memory", str(args.memory),
        "--guide", guide or args.guide,
    ]


def solver_command(args: argparse.Namespace, fen: str) -> list[str]:
    return [
        args.solver, "--fen", fen,
        "--timeout", fmt_seconds(args.seconds),
        "--first-outcome", "--outcome-only",
        "--tt-size", str(args.memory),
    ]


def run_process(cmd: list[str], timeout: float) -> tuple[int | None, str, str, float, str | None]:
    """Run one command and return (rc, stdout, stderr, wall_s, error)."""
    start = time.perf_counter()
    try:
        proc = subprocess.run(
            cmd,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            errors="replace",
            timeout=timeout,
        )
        return proc.returncode, proc.stdout, proc.stderr, time.perf_counter() - start, None
    except subprocess.TimeoutExpired as exc:
        wall = time.perf_counter() - start
        stdout = exc.stdout.decode(errors="replace") if isinstance(exc.stdout, bytes) else (exc.stdout or "")
        stderr = exc.stderr.decode(errors="replace") if isinstance(exc.stderr, bytes) else (exc.stderr or "")
        return None, stdout, stderr, wall, f"driver timeout after {timeout:.0f}s"
    except OSError as exc:
        return None, "", "", time.perf_counter() - start, f"could not execute: {exc}"


def last_json_object(text: str) -> dict | None:
    for line in reversed(text.splitlines()):
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict):
            return value
    return None


def parse_solver(stdout: str, stderr: str) -> dict:
    parsed: dict = {
        "outcome": None, "pv": None, "pv_status": None,
        "exit": None, "child_evals": None, "solver_nodes": None,
    }
    cut = None
    for line in stdout.splitlines():
        line = line.strip()
        match = OUTCOME_RE.match(line)
        if match:
            parsed["outcome"] = match.group(1)
            parsed["first_win_plies"] = int(match.group(2))
        elif PV_RE.match(line):
            parsed["pv"] = PV_RE.match(line).group(1).strip()
        elif PV_STATUS_RE.match(line):
            parsed["pv_status"] = PV_STATUS_RE.match(line).group(1).strip()
        elif line in CUT_LINES:
            cut = line
    for line in stderr.splitlines():
        line = line.strip()
        match = EVALS_RE.match(line)
        if match:
            parsed["child_evals"] = int(match.group(1))
        match = PRE_EXIT_RE.match(line)
        if match:
            parsed["solver_nodes"] = int(match.group(3))
            parsed["exit"] = match.group(1)
    if parsed["exit"] is None:
        parsed["exit"] = {
            "timeout": "Timeout", "quit": "Quit",
            "budget exhausted": "BudgetExhausted",
        }.get(cut, "Complete" if parsed["outcome"] is not None else None)
    parsed["_cut"] = cut
    return parsed


def provari_row(args: argparse.Namespace, case: dict, repeat_index: int,
                guide: str | None = None) -> dict:
    fen = case["fen"]
    guide = guide or args.guide
    cmd = provari_command(args, fen, guide)
    if args.dry_run:
        return {"case": case["id"], "solver": "provari", "repeat_index": repeat_index,
                "config": guide, "command": " ".join(cmd)}
    rc, stdout, _stderr, wall, error = run_process(cmd, args.seconds + 60)
    data = last_json_object(stdout)
    row = {
        "case": case["id"], "solver": "provari", "repeat_index": repeat_index,
        "config": {"model": "bundled-model", "hand": "hand", "none": "none"}[guide],
        "wall_s": round(wall, 3),
        "outcome": None, "first_win_plies": None,
        "provari_nodes": None, "provari_first_mate_ms": None,
        "provari_ms": None, "provari_mate": None,
        "exit": None, "censored": True, "command": " ".join(cmd),
        "returncode": rc,
    }
    if error:
        row["error"] = error
        return row
    if data is None:
        row["error"] = "no JSON object found on provari stdout"
        return row
    found = bool(data.get("found"))
    mate = data.get("mate")
    row.update({
        "outcome": "win" if found else "none",
        "first_win_plies": 2 * mate if isinstance(mate, int) else None,
        "provari_nodes": data.get("nodes"),
        "provari_first_mate_ms": data.get("first_mate_ms"),
        "provari_ms": data.get("ms"),
        "provari_mate": mate,
        "exit": "Complete" if found else "Budget",
        "censored": not found,
    })
    return row


def solver_row(args: argparse.Namespace, case: dict, repeat_index: int) -> dict:
    fen = case["fen"]
    cmd = solver_command(args, fen)
    if args.dry_run:
        return {"case": case["id"], "solver": "atomic_solver", "repeat_index": repeat_index,
                "config": "first-outcome", "command": " ".join(cmd)}
    rc, stdout, stderr, wall, error = run_process(cmd, args.seconds + 60)
    parsed = parse_solver(stdout, stderr)
    outcome = parsed["outcome"]
    row = {
        "case": case["id"], "solver": "atomic_solver", "repeat_index": repeat_index,
        "config": "first-outcome",
        "wall_s": round(wall, 3),
        "outcome": outcome,
        "first_win_plies": parsed.get("first_win_plies") if outcome in ("win", "loss") else None,
        "child_evals": parsed["child_evals"],
        "solver_nodes": parsed["solver_nodes"],
        "pv": parsed["pv"],
        "pv_status": parsed["pv_status"],
        "exit": parsed["exit"],
        "censored": outcome not in ("win", "loss"),
        "command": " ".join(cmd),
        "returncode": rc,
    }
    if error:
        row["error"] = error
        row["censored"] = True
    elif outcome is None:
        row["error"] = "no outcome line found on atomic_solver stdout"
        row["censored"] = True
    return row


def proof_check(args: argparse.Namespace, core: dict) -> dict:
    if args.dry_run:
        return {"skipped": "dry-run"}
    if args.no_proof_check:
        return {"skipped": "--no-proof-check"}
    proof_path = "/tmp/provari-core-proof.pvr"
    try:
        Path(proof_path).unlink()
    except FileNotFoundError:
        pass
    try:
        cmd = provari_command(args, core["fen"]) + ["--proofs", proof_path]
        rc, stdout, _stderr, wall, error = run_process(cmd, args.seconds + 60)
        prove_data = last_json_object(stdout)
        result = {
            "prove_command": " ".join(cmd),
            "prove_returncode": rc,
            "prove_wall_s": round(wall, 3),
            "proof_file_bytes": Path(proof_path).stat().st_size if Path(proof_path).exists() else None,
            "valid": False,
        }
        if error:
            result["error"] = error
            return result
        if prove_data is None:
            result["error"] = "no JSON object found on provari prove stdout"
            return result
        check_cmd = [args.provari, "check", proof_path]
        check_rc, check_out, _check_err, check_wall, check_error = run_process(
            check_cmd, max(60.0, args.seconds + 60)
        )
        check_data = last_json_object(check_out)
        result.update({
            "check_command": " ".join(check_cmd),
            "check_returncode": check_rc,
            "check_wall_s": round(check_wall, 3),
            "valid": bool(check_data.get("valid")) if check_data else False,
        })
        if check_error:
            result["check_error"] = check_error
        if check_data:
            result["check"] = {
                key: check_data.get(key)
                for key in ("valid", "moves", "won_positions", "highest_counter")
                if key in check_data
            }
        return result
    finally:
        try:
            Path(proof_path).unlink()
        except FileNotFoundError:
            pass


def median(values: list[float]) -> float | None:
    return round(statistics.median(values), 3) if values else None


def summarize(runs: list[dict]) -> dict:
    def wins(solver: str, case_ids: set[str]) -> set[str]:
        return {
            row["case"] for row in runs
            if row["solver"] == solver and row["case"] in case_ids
            and row.get("outcome") == "win"
        }

    sample_ids = {row["case"] for row in runs if row["case"] != "core"}
    provari_wins = wins("provari", sample_ids)
    solver_wins = wins("atomic_solver", sample_ids)
    core = [row for row in runs if row["case"] == "core"]

    def core_stats(solver: str, config: str | None = None) -> dict:
        rows = [row for row in core if row["solver"] == solver
                and (config is None or row["config"] == config)]
        walls = [row["wall_s"] for row in rows
                 if isinstance(row.get("wall_s"), (int, float))]
        evals = [row["child_evals"] for row in rows
                 if isinstance(row.get("child_evals"), int)]
        nodes = [row["provari_nodes"] for row in rows
                 if isinstance(row.get("provari_nodes"), int)]
        return {
            "wall_s": median(walls),
            "child_evals": median([float(value) for value in evals]),
            "provari_nodes": median([float(value) for value in nodes]),
        }

    provari_configs = sorted({row["config"] for row in core if row["solver"] == "provari"})
    core_medians = {
        "provari": {config: core_stats("provari", config) for config in provari_configs},
        "atomic_solver": {"first-outcome": core_stats("atomic_solver", "first-outcome")},
    }
    return {
        "core_medians": core_medians,
        "sample20": {
            "cases": len(sample_ids),
            "provari_wins": len(provari_wins),
            "solver_wins": len(solver_wins),
            "shared_wins": sorted(provari_wins & solver_wins),
            "provari_coverage": len(provari_wins) / len(sample_ids) if sample_ids else None,
            "solver_coverage": len(solver_wins) / len(sample_ids) if sample_ids else None,
        },
    }


def load_cases(path: Path) -> list[dict]:
    corpus = json.loads(path.read_text())
    core = corpus["core"]
    sample = corpus["sample20"]
    if len(sample) != 20:
        raise SystemExit(f"corpus error: sample20 has {len(sample)} rows, expected 20")
    if not all(isinstance(row.get("id"), str) and isinstance(row.get("fen"), str) for row in sample):
        raise SystemExit("corpus error: every sample20 row needs string id and fen")
    return [{"id": core["id"], "fen": core["fen"]}] + sample


def version_of(binary: str) -> str:
    rc, stdout, stderr, _wall, error = run_process([binary, "--version"], 30)
    text = (stdout or stderr).strip().splitlines()
    if error or rc != 0 or not text:
        return f"unavailable ({error or f'rc={rc}'})"
    return text[0].strip()


def main() -> int:
    args = parse_args()
    corpus_path = Path(args.corpus)
    cases = load_cases(corpus_path)
    if args.repeat < 1:
        raise SystemExit("--repeat must be at least 1")

    if args.dry_run:
        for case in cases:
            print("provari:      " + " ".join(provari_command(args, case["fen"])))
            print("atomic_solver: " + " ".join(solver_command(args, case["fen"])))
        if not args.no_hand_control and args.guide != "hand":
            print("hand control: " + " ".join(provari_command(args, cases[0]["fen"], "hand")))
        if not args.no_proof_check:
            print("proof check:  provari prove <core> ... --proofs /tmp/provari-core-proof.pvr && provari check /tmp/provari-core-proof.pvr")
        return 0

    runs: list[dict] = []
    for case in cases:
        repeats = args.repeat if case["id"] == "core" else 1
        for repeat_index in range(1, repeats + 1):
            row = provari_row(args, case, repeat_index)
            runs.append(row)
            print(f"[{len(runs):3d}] provari       {case['id']:>12} "
                  f"{row.get('outcome')} {row.get('wall_s')}s", file=sys.stderr)
            row = solver_row(args, case, repeat_index)
            runs.append(row)
            print(f"[{len(runs):3d}] atomic_solver {case['id']:>12} "
                  f"{row.get('outcome')} {row.get('wall_s')}s", file=sys.stderr)

    core = next(case for case in cases if case["id"] == "core")
    if not args.no_hand_control and args.guide != "hand":
        for repeat_index in range(1, args.repeat + 1):
            row = provari_row(args, core, repeat_index, guide="hand")
            runs.append(row)
            print(f"[{len(runs):3d}] provari       {core['id']:>12} "
                  f"{row.get('outcome')} {row.get('wall_s')}s (hand)", file=sys.stderr)

    checks = {"core_proof": proof_check(args, core)}

    results = {
        "provenance": {
            "date": args.date or datetime.now(timezone.utc).date().isoformat(),
            "host": args.host or platform.node() or platform.platform(),
            "platform": platform.platform(),
            "provari": {
                "commit": args.provari_commit,
                "version": version_of(args.provari),
                "features": [item for item in args.provari_features.split(",") if item],
                "guide": args.guide,
                "binary": args.provari,
            },
            "solver": {
                "commit": args.solver_commit,
                "version": version_of(args.solver),
                "binary": args.solver,
                "command": " ".join(solver_command(args, "<FEN>")),
            },
            "corpus": {
                "path": str(corpus_path),
                "source": json.loads(corpus_path.read_text()).get("source"),
            },
            "budgets": {
                "seconds": args.seconds,
                "memory_mb": args.memory,
                "provari_memory": "table + proofs",
                "solver_tt_size": "reservation label; pages populated on use",
            },
        },
        "checks": checks,
        "summary": summarize(runs),
        "runs": runs,
    }
    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(results, indent=2) + "\n")
    print(f"wrote {out_path} ({len(runs)} rows)", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
