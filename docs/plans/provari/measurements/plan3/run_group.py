#!/usr/bin/env python3
"""Quiet-move grouping: four-configuration driver (provari plan3).

Reuses the committed plan1 corpus by path (never re-picks `sample20`). Every
run is a release build, sequential, one process at a time, no tablebases,
192 MB TT label. Only parsed rows are written; raw stdout/stderr stay in
memory (the repo ignores transcripts).

Usage:
    python3 run_group.py \
        --solver ./target/release/atomic_solver \
        --verify ./target/release/examples/verify_ppv \
        --reconstruct ./target/release/examples/reconstruct_pt

Configurations (the interaction is the point):
  off   = shipped baseline
  group = baseline + --quiet-grouping
  mc    = baseline + --move-class-pn
  both  = baseline + --quiet-grouping --move-class-pn

Runs (plan3 measurement protocol):
  * core FEN: --timeout 90 --budget 600000000, 3 repeats/config
  * shared case 1eb80b92f92c: --timeout 10 --budget 300000000, 3 repeats/config
  * sample20: --timeout 10 (no explicit budget), 1 repeat/config

Decisive rows are validated twice, deduplicated by (case, config, PV):

  * `verify_ppv` on the measured first-outcome PV (a PV property; an
    informational PV may legitimately fail this);
  * the proof-tree path: re-run the solver with `--tt-dump-path`, rebuild
    the tree with `reconstruct_pt`, and read its replay-based `validate`
    verdict (this checks the published outcome, not the PV).

`--dry-run` prints the exact measurement commands. `--skip-validation`
measures only.
"""

from __future__ import annotations

import argparse
import json
import platform
import re
import statistics
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path

sys.dont_write_bytecode = True

HERE = Path(__file__).resolve().parent
PLAN1_CORPUS = HERE.parent / "plan1" / "corpus.json"
DEFAULT_OUT = HERE / "results.json"

SHARED_ID = "1eb80b92f92c"
CONFIGS = ("off", "group", "mc", "both")
CONFIG_FLAGS = {
    "off": [],
    "group": ["--quiet-grouping"],
    "mc": ["--move-class-pn"],
    "both": ["--quiet-grouping", "--move-class-pn"],
}
OUTCOME_RE = re.compile(r"^outcome: (win|loss|draw) length: (\d+)$", re.MULTILINE)
EVALS_RE = re.compile(r"^evals: (\d+)$", re.MULTILINE)
CUT_LINES = {"timeout", "quit", "budget exhausted"}


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="Run the plan3 four-configuration quiet-grouping measurement.",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    p.add_argument("--solver", required=True, help="atomic_solver binary")
    p.add_argument("--verify", default="./target/release/examples/verify_ppv")
    p.add_argument("--reconstruct", default="./target/release/examples/reconstruct_pt")
    p.add_argument("--corpus", default=str(PLAN1_CORPUS))
    p.add_argument("--out", default=str(DEFAULT_OUT))
    p.add_argument("--solver-commit", default="unknown")
    p.add_argument("--date", default=None, help="ISO date (default: UTC today)")
    p.add_argument("--host", default=None)
    p.add_argument("--verify-timeout", type=int, default=120,
                   help="verify_ppv wall limit per decisive PV")
    p.add_argument("--reconstruct-timeout", type=int, default=1500,
                   help="reconstruct_pt wall limit per decisive position")
    p.add_argument("--skip-validation", action="store_true")
    p.add_argument("--dry-run", action="store_true")
    return p.parse_args()


def solver_command(args, fen: str, config: str, timeout: int,
                   budget: int | None) -> list[str]:
    cmd = [args.solver, "--fen", fen, "--tt-size", "192",
           "--first-outcome", "--outcome-only", "--timeout", str(timeout)]
    if budget is not None:
        cmd += ["--budget", str(budget)]
    cmd += CONFIG_FLAGS[config]
    return cmd


def run_process(cmd: list[str], timeout: float) -> dict:
    start = time.perf_counter()
    try:
        proc = subprocess.run(
            cmd, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, text=True, errors="replace",
            timeout=timeout, check=False,
        )
        rc, out, err = proc.returncode, proc.stdout, proc.stderr
        error = None
    except subprocess.TimeoutExpired as exc:
        out = exc.stdout.decode(errors="replace") if isinstance(exc.stdout, bytes) else (exc.stdout or "")
        err = exc.stderr.decode(errors="replace") if isinstance(exc.stderr, bytes) else (exc.stderr or "")
        rc, error = None, "driver-timeout"
    return {"rc": rc, "stdout": out, "stderr": err,
            "wall_s": time.perf_counter() - start, "error": error}


def parse_run(result: dict) -> dict:
    outcome = first_win = pv = pv_status = None
    for line in result["stdout"].splitlines():
        line = line.strip()
        m = OUTCOME_RE.match(line)
        if m:
            outcome, first_win = m.group(1), int(m.group(2))
        elif line.startswith("pv: "):
            pv = line[4:]
        elif line.startswith("pv_status: "):
            pv_status = line[len("pv_status: "):]
    evals = None
    m = EVALS_RE.search(result["stderr"])
    if m:
        evals = int(m.group(1))
    cut = [l.strip() for l in result["stdout"].splitlines() if l.strip() in CUT_LINES]
    if cut:
        exit_reason = {"timeout": "Timeout", "quit": "Quit",
                       "budget exhausted": "BudgetExhausted"}[cut[-1]]
    else:
        exit_reason = "Complete" if outcome is not None else "Unknown"
    censored = outcome == "draw" and exit_reason in ("Timeout", "BudgetExhausted")
    return {"wall_s": round(result["wall_s"], 3), "outcome": outcome,
            "first_win_plies": first_win, "evals": evals,
            "pv_status": pv_status, "exit": exit_reason,
            "censored": censored, "pv": pv,
            "rc": result["rc"], "error": result["error"]}


def verify_pv(args, fen: str, moves: str) -> dict:
    cmd = [args.verify, "--fen", fen, "--moves", moves,
           "--timeout", str(args.verify_timeout)]
    result = run_process(cmd, timeout=args.verify_timeout + 30)
    ok = result["stdout"].strip().endswith("is_ppv: true")
    reason = None
    for line in reversed(result["stderr"].splitlines()):
        line = line.strip()
        if line and not line.startswith("[bounded_search]"):
            reason = line
            break
    if ok:
        verdict, reason = "valid", None
    elif "not proven lost within" in (reason or "") and result["wall_s"] >= args.verify_timeout - 5:
        verdict = "inconclusive"
    elif result["error"] is not None:
        verdict = "inconclusive"
    elif reason is not None and ("not a longest defense" in reason
                                 or "not proven lost within" in reason):
        verdict = "refuted-pv"
    else:
        verdict = "inconclusive"
    return {"kind": "pv", "verdict": verdict, "is_ppv": ok,
            "reason": reason, "wall_s": round(result["wall_s"], 3)}


def validate_proof(args, fen: str, config: str, timeout: int,
                   budget: int | None) -> dict:
    """Dump the TT from a validation re-run, rebuild, and validate."""
    with tempfile.TemporaryDirectory(prefix="plan3-validate-") as tmp:
        snap = str(Path(tmp) / "tt.bin")
        out = str(Path(tmp) / "pt.bin")
        cmd = solver_command(args, fen, config, timeout, budget) + \
            ["--tt-dump-path", snap]
        solver = run_process(cmd, timeout=timeout + 30)
        measured = parse_run(solver)
        if measured["outcome"] not in ("win", "loss") or not Path(snap).is_file():
            return {"kind": "proof", "verdict": "no-tree",
                    "measured_outcome": measured["outcome"]}
        recon = run_process(
            [args.reconstruct, "--snapshot", snap, "--out", out,
             "--pt-size", "512", "--fill-attempt-budget", "100000000",
             "--fill-total-budget", "0", "--json"],
            timeout=args.reconstruct_timeout + 30,
        )
        try:
            report = json.loads(recon["stdout"])
        except json.JSONDecodeError:
            return {"kind": "proof", "verdict": "reconstruct-error",
                    "measured_outcome": measured["outcome"],
                    "error": recon["error"], "wall_s": round(recon["wall_s"], 3)}
        valid = report.get("validate") == "ok" and \
            report.get("root_outcome") == measured["outcome"]
        return {"kind": "proof", "verdict": "valid" if valid else "invalid",
                "measured_outcome": measured["outcome"],
                "root_outcome": report.get("root_outcome"),
                "nodes": report.get("nodes"),
                "fill_evals": report.get("fill_evals"),
                "validate": report.get("validate"),
                "wall_s": round(recon["wall_s"], 3)}


def run_case(args, case_id: str, fen: str, timeout: int, budget: int | None,
             repeats: int, cache: dict) -> list[dict]:
    rows = []
    for config in CONFIGS:
        for run in range(1, repeats + 1):
            cmd = solver_command(args, fen, config, timeout, budget)
            if args.dry_run:
                print(" ".join(cmd))
                continue
            row = parse_run(run_process(cmd, timeout=timeout + 30))
            row.update({"case": case_id, "config": config, "run": run,
                        "timeout_s": timeout, "budget": budget})
            if not args.skip_validation and row["outcome"] in ("win", "loss"):
                key = (case_id, config, row["pv"])
                if key not in cache:
                    cache[key] = {
                        "fen": fen,
                        "pv_verify": verify_pv(args, fen, row["pv"])
                        if row["pv"] else None,
                        "proof": validate_proof(args, fen, config, timeout, budget),
                    }
                row["validation"] = cache[key]
            rows.append(row)
    return rows


def medians(rows: list[dict]) -> dict:
    out = {}
    for config in CONFIGS:
        group = [r for r in rows if r["config"] == config]
        walls = [r["wall_s"] for r in group]
        evals = [r["evals"] for r in group if r["evals"] is not None]
        out[config] = {
            "runs": len(group),
            "wall_median_s": round(statistics.median(walls), 3) if walls else None,
            "evals_median": int(statistics.median(evals)) if evals else None,
            "wins": sum(1 for r in group if r["outcome"] == "win"),
            "censored": sum(1 for r in group if r["censored"]),
            "proof_validated": sum(
                1 for r in group
                if r.get("validation", {}).get("proof", {}).get("verdict") == "valid"),
        }
    return out


def summarize(rows: list[dict]) -> dict:
    by_config = {}
    for config in CONFIGS:
        group = [r for r in rows if r["config"] == config]
        by_config[config] = {
            "cases": len(group),
            "wins": sum(1 for r in group if r["outcome"] == "win"),
            "losses": sum(1 for r in group if r["outcome"] == "loss"),
            "draws": sum(1 for r in group if r["outcome"] == "draw"),
            "censored": sum(1 for r in group if r["censored"]),
            "evals_sum": sum(r["evals"] for r in group if r["evals"] is not None),
        }
    return by_config


def main() -> int:
    args = parse_args()
    corpus = json.loads(Path(args.corpus).read_text())
    core = corpus["core"]
    sample20 = corpus["sample20"]
    shared = next(c for c in sample20 if c["id"] == SHARED_ID)

    if not args.dry_run and not args.skip_validation:
        for b in (args.solver, args.verify, args.reconstruct):
            if not Path(b).is_file():
                print(f"error: missing binary {b}", file=sys.stderr)
                return 2

    cache: dict = {}
    core_rows = run_case(args, "core", core["fen"], timeout=90,
                         budget=600_000_000, repeats=3, cache=cache)
    shared_rows = run_case(args, SHARED_ID, shared["fen"], timeout=10,
                           budget=300_000_000, repeats=3, cache=cache)
    sample_rows = []
    for case in sample20:
        sample_rows += run_case(args, case["id"], case["fen"], timeout=10,
                                budget=None, repeats=1, cache=cache)

    if args.dry_run:
        return 0

    results = {
        "provenance": {
            "plan": "provari plan3 (OR-node quiet-move grouping)",
            "date": args.date or datetime.now(timezone.utc).date().isoformat(),
            "host": args.host or f"{platform.node()} ({platform.system()} {platform.release()})",
            "solver": args.solver,
            "solver_commit": args.solver_commit,
            "verify": None if args.skip_validation else args.verify,
            "reconstruct": None if args.skip_validation else args.reconstruct,
            "verify_timeout_s": None if args.skip_validation else args.verify_timeout,
            "corpus": str(Path(args.corpus)),
            "tt_label_mb": 192,
            "tablebases": "none",
            "configs": {"off": "shipped baseline",
                        "group": "baseline + --quiet-grouping",
                        "mc": "baseline + --move-class-pn",
                        "both": "baseline + --quiet-grouping --move-class-pn"},
            "protocol": {
                "core": {"timeout_s": 90, "budget": 600_000_000, "repeats": 3},
                "shared": {"case": SHARED_ID, "timeout_s": 10,
                           "budget": 300_000_000, "repeats": 3},
                "sample20": {"timeout_s": 10, "budget": None, "repeats": 1},
            },
            "validation": (
                "decisive rows: verify_ppv on the measured PV (PV property, may "
                "legitimately fail for an informational PV) plus a proof-tree "
                "re-run via --tt-dump-path -> reconstruct_pt (replay validator; "
                "checks the published outcome). Deduplicated by (case, config, PV)."
                if not args.skip_validation else "skipped"),
        },
        "core": core_rows,
        "core_medians": medians(core_rows),
        "shared": shared_rows,
        "shared_medians": medians(shared_rows),
        "sample20": sample_rows,
        "sample20_summary": summarize(sample_rows),
        "validation": {f"{k[0]}|{k[1]}": v for k, v in cache.items()},
    }
    Path(args.out).write_text(json.dumps(results, indent=2) + "\n")
    print(f"wrote {args.out}")
    print(json.dumps({"core": results["core_medians"],
                      "shared": results["shared_medians"],
                      "sample20": results["sample20_summary"]}, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
