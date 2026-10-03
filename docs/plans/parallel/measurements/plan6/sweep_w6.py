#!/usr/bin/env python3
"""plan6 W confirmation on the long class (shuffle-win), stage 2.

Per W in {4000, 10000, 20000}: patch MAX_WORK_PER_JOB, rebuild release,
run 1 sequential (N=1) reference, then 3 reps at --threads 4 (100 s cap).
Appends one JSON line per run to state/sweep6_raw.jsonl (wall, outcome,
[parallel] counters). Caller is responsible for restoring the locked W
afterwards (re-spatch + rebuild + verify).

Usage: sweep_w6.py
"""
import json
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path("/workspace/atomic_solver")
SRC = ROOT / "src/search/dfpn/parallel/mod.rs"
BIN = ROOT / "target/release/atomic_solver"
HERE = Path(__file__).resolve().parent
STATE = HERE / "state"
LOGS = HERE / "logs"
FEN = "4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21"  # shuffle-win
CAP = 100
WS = [4000, 10000, 20000]


def set_w(w: int) -> None:
    text = SRC.read_text()
    new, n = re.subn(r"pub\(super\) const MAX_WORK_PER_JOB: u64 = [\d_]+;",
                     f"pub(super) const MAX_WORK_PER_JOB: u64 = {w};", text)
    assert n == 1, f"constant not found/unique ({n})"
    SRC.write_text(new)


def build() -> None:
    subprocess.run(["cargo", "build", "--release", "--bin", "atomic_solver"],
                   cwd=ROOT, check=True, capture_output=True)


def one_run(w, threads, rep):
    t0 = time.monotonic()
    proc = subprocess.run(
        [str(BIN), "--fen", FEN, "--timeout", str(CAP), "--first-outcome",
         "--outcome-only"] + ([] if threads == 1 else ["--threads", str(threads)]),
        capture_output=True, text=True)
    wall = time.monotonic() - t0
    outcome = None
    for line in proc.stdout.splitlines():
        if line.startswith("outcome:"):
            outcome = line.split()[1]
            break
    par = {}
    for line in proc.stderr.splitlines():
        if line.startswith("[parallel]"):
            for tok in line.split()[1:]:
                k, _, v = tok.partition("=")
                try:
                    par[k] = int(v)
                except ValueError:
                    par[k] = v
    rec = {"stage": "sweep6", "W": w, "rep": rep, "threads": threads,
           "wall_s": round(wall, 3), "outcome": outcome, "cap_s": CAP,
           "rc": proc.returncode, **par}
    (LOGS / f"sweep6_w{w}_t{threads}_rep{rep}.err").write_text(proc.stderr)
    with open(STATE / "sweep6_raw.jsonl", "a") as f:
        f.write(json.dumps(rec) + "\n")
    print(json.dumps(rec), flush=True)


def main() -> None:
    STATE.mkdir(exist_ok=True)
    LOGS.mkdir(exist_ok=True)
    for w in WS:
        set_w(w)
        build()
        one_run(w, 1, 0)          # N = 1 reference (W-independent, interleaved)
        for rep in range(1, 4):
            one_run(w, 4, rep)


if __name__ == "__main__":
    main()
