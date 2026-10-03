#!/usr/bin/env python3
"""plan5b W sweep: build with a given MAX_WORK_PER_JOB, run m22 t4 reps.

Usage: sweep_w.py <W> <reps>
Patches src/search/dfpn/parallel/mod.rs, rebuilds release, runs `reps`
m22 first-outcome runs at --threads 4 (30 s cap), appends one JSON line
per rep to state/sweep_raw.jsonl (wall, outcome, coordinator/helper evals).
Caller is responsible for restoring the locked W afterwards.
"""
import json
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[:5][0] if False else Path("/workspace/atomic_solver")
SRC = ROOT / "src/search/dfpn/parallel/mod.rs"
BIN = ROOT / "target/release/atomic_solver"
STATE = ROOT / "docs/plans/parallel/measurements/plan5/state"
LOGS = ROOT / "docs/plans/parallel/measurements/plan5/logs"
FEN = "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22"


def set_w(w: int) -> None:
    text = SRC.read_text()
    new, n = re.subn(r"pub\(super\) const MAX_WORK_PER_JOB: u64 = [\d_]+;",
                     f"pub(super) const MAX_WORK_PER_JOB: u64 = {w};", text)
    assert n == 1, f"constant not found/unique ({n})"
    SRC.write_text(new)


def build() -> None:
    subprocess.run(["cargo", "build", "--release", "--bin", "atomic_solver"],
                   cwd=ROOT, check=True, capture_output=True)


def one_run(w: int, rep: int, threads: int = 4) -> dict:
    t0 = time.monotonic()
    proc = subprocess.run(
        [str(BIN), "--fen", FEN, "--timeout", "30", "--first-outcome",
         "--outcome-only", "--threads", str(threads)],
        capture_output=True, text=True)
    wall = time.monotonic() - t0
    out = proc.stdout
    err = proc.stderr
    outcome = None
    for line in out.splitlines():
        if line.startswith("outcome:"):
            outcome = line.split()[1]
            break
    par = {}
    for line in err.splitlines():
        if line.startswith("[parallel]"):
            for tok in line.split()[1:]:
                k, _, v = tok.partition("=")
                par[k] = int(v)
    rec = {"W": w, "rep": rep, "threads": threads, "wall_s": round(wall, 3),
           "outcome": outcome, "rc": proc.returncode, **par}
    (LOGS / f"sweep_w{w}_rep{rep}.err").write_text(err)
    return rec


def main() -> None:
    w, reps = int(sys.argv[1]), int(sys.argv[2])
    set_w(w)
    build()
    for rep in range(1, reps + 1):
        rec = one_run(w, rep)
        with open(STATE / "sweep_raw.jsonl", "a") as f:
            f.write(json.dumps(rec) + "\n")
        print(json.dumps(rec), flush=True)


if __name__ == "__main__":
    main()
