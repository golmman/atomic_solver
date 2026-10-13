#!/usr/bin/env python3
"""plan12 stage-0 robustness probe: stress (shuffle-win) salt draws on the
current binary.

The corpus stress realization band (3.04x loud draw, research plan12) was
measured on the 2026-10-10 binary; the current binary's eval counts have
drifted (salt0 287.9 M vs corpus 249.5 M). This re-runs the three uncensored
corpus salts {0, 1, 5} to check the loud-draw shape survives the drift
(robustness input for the H0 noise model caveat; the plan6-era magnitude test
itself uses the plan6-era corpus band, which is self-consistent with the
plan6 campaign data).

Output: state/stress_salts_fresh.json.
"""
import json
import re
import subprocess
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
BIN = (HERE / ".." / ".." / ".." / ".." / ".." / "target" / "release" / "atomic_solver").resolve()
OUT = HERE / "state" / "stress_salts_fresh.json"

FEN = "4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21"
SALTS = [0, 1, 5]
BUDGET = 2_500_000_000
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
    e = re.search(r"^evals: (\d+)", proc.stderr, re.M)
    return {
        "salt": salt,
        "rc": proc.returncode,
        "wall_s": round(wall, 3),
        "outcome": m.group(1) if m else None,
        "decisive": bool(m) and m.group(1) in ("win", "loss"),
        "budget_exhausted": "budget" in proc.stderr.lower(),
        "evals": int(e.group(1)) if e else None,
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
