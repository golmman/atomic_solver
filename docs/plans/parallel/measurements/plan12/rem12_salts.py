#!/usr/bin/env python3
"""plan12 stage-0 fresh probe: rem12 sequential realization distribution.

Runs the plan6 mid case (rem12) at the 6 canonical salts under the research
plan12 rollout protocol (first-outcome, outcome-only, 1 B child-eval budget,
eps 0.125 default, 128 MB TT default, 600 s wall cap). Sequential single
process; deterministic per (FEN, salt) — only wall is environment-dependent.

Complements the committed research plan12 corpus (rem12 was not in it) so the
H0 noise model gets an empirical realization band for all three plan6 cases.

Output: state/rem12_salts.json (raw transcripts stay in /tmp, gitignored).
"""
import json
import re
import subprocess
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
BIN = (HERE / ".." / ".." / ".." / ".." / ".." / "target" / "release" / "atomic_solver").resolve()
OUT = HERE / "state" / "rem12_salts.json"

FEN = "rnbqkbnr/8/6pp/pppppp1B/3PPP2/N5PN/PPP4P/R1BQ1RK1 w kq - 0 10"
SALTS = [0, 1, 2, 3, 4, 5]
BUDGET = 1_000_000_000
TIMEOUT_S = 600


def run_one(salt: int) -> dict:
    t0 = time.monotonic()
    proc = subprocess.run(
        [
            str(BIN),
            "--fen", FEN,
            "--first-outcome",
            "--outcome-only",
            "--salt", str(salt),
            "--budget", str(BUDGET),
            "--timeout", str(TIMEOUT_S),
        ],
        capture_output=True,
        text=True,
        timeout=TIMEOUT_S + 120,
    )
    wall = time.monotonic() - t0
    m = re.search(r"^outcome: (\w+)", proc.stdout, re.M)
    outcome = m.group(1) if m else None
    e = re.search(r"^evals: (\d+)", proc.stderr, re.M)
    evals = int(e.group(1)) if e else None
    budget_exhausted = "budget" in proc.stderr.lower()
    return {
        "salt": salt,
        "rc": proc.returncode,
        "wall_s": round(wall, 3),
        "outcome": outcome,
        "decisive": outcome in ("win", "loss"),
        "budget_exhausted": budget_exhausted,
        "evals": evals,
        "budget": BUDGET,
    }


def main() -> None:
    results = [run_one(s) for s in SALTS]
    OUT.write_text(json.dumps({"fen": FEN, "runs": results}, indent=1))
    for r in results:
        print(r["salt"], r["outcome"], r["evals"], f"{r['wall_s']:.1f}s",
              "censored" if r["budget_exhausted"] else "")


if __name__ == "__main__":
    main()
