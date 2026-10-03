#!/usr/bin/env python3
"""plan7 campaign: process-level portfolio racing vs sequential baseline.

Per case, >= 8 interleaved rounds; each round = (a) one baseline sequential
default-config run alone on the machine, then (b) one race: P0-P3 launched
concurrently (4 diverse --refine-cap/--epsilon configurations), wall
recorded per racer, W_first = first decisive finish. Per run: wall, outcome,
cap-hit flag, rc. One JSON line per run -> state/portfolio7_raw.jsonl;
solver stderr + stdout kept under logs/ (gitignored).

Usage: portfolio7.py <rounds> [case_filter] [label]   # label e.g. "pilot"
"""
import json
import subprocess
import sys
import time
from pathlib import Path

sys.dont_write_bytecode = True

ROOT = Path("/workspace/atomic_solver")
BIN = ROOT / "target/release/atomic_solver"
HERE = Path(__file__).resolve().parent
STATE = HERE / "state"
LOGS = HERE / "logs"

CASES = [
    ("m22", "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22", 30),
    ("rem12", "rnbqkbnr/8/6pp/pppppp1B/3PPP2/N5PN/PPP4P/R1BQ1RK1 w kq - 0 10", 60),
    ("shuffle_win", "4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21", 100),
]

# Racer portfolio (frozen in plan7.md; no post-selection swaps).
RACERS = [
    ("P0", ["--refine-cap", "0.25", "--epsilon", "0.125"]),  # product defaults
    ("P1", ["--refine-cap", "0",    "--epsilon", "0.125"]),  # uncapped refinement
    ("P2", ["--refine-cap", "1.0",  "--epsilon", "0.125"]),  # loose cap
    ("P3", ["--refine-cap", "0.25", "--epsilon", "0.25"]),   # shifted threshold
]


def parse_outcome(stdout):
    for line in stdout.splitlines():
        if line.startswith("outcome:"):
            return line.split()[1]
    return None


def parse_child_evals(stdout):
    for line in stdout.splitlines():
        if line.startswith("child_evals:"):
            return int(line.split()[1])
    return None


def append(rec):
    with open(STATE / "portfolio7_raw.jsonl", "a") as f:
        f.write(json.dumps(rec) + "\n")
    print(json.dumps(rec), flush=True)


def baseline_run(case, fen, cap, round_no, label):
    t0 = time.monotonic()
    proc = subprocess.run(
        [str(BIN), "--fen", fen, "--timeout", str(cap),
         "--first-outcome", "--outcome-only"],
        capture_output=True, text=True)
    wall = time.monotonic() - t0
    outcome = parse_outcome(proc.stdout)
    (LOGS / f"portfolio7_{case}_r{round_no}_{label}_base.out").write_text(proc.stdout)
    (LOGS / f"portfolio7_{case}_r{round_no}_{label}_base.err").write_text(proc.stderr)
    append({"kind": "baseline", "case": case, "round": round_no, "label": label,
            "wall_s": round(wall, 3), "outcome": outcome, "cap_s": cap,
            "cap_hit": outcome != "win", "rc": proc.returncode,
            "child_evals": parse_child_evals(proc.stdout)})


def race(case, fen, cap, round_no, label):
    procs = []
    t0 = time.monotonic()
    for name, cfg in RACERS:
        with open(LOGS / f"portfolio7_{case}_r{round_no}_{label}_race_{name}.out", "w") as out_f, \
                open(LOGS / f"portfolio7_{case}_r{round_no}_{label}_race_{name}.err", "w") as err_f:
            p = subprocess.Popen(
            [str(BIN), "--fen", fen, "--timeout", str(cap), "--first-outcome",
             "--outcome-only"] + cfg,
            stdout=out_f, stderr=err_f, text=True)
        procs.append((name, cfg, p))
    wall = {}
    pending = list(range(len(procs)))
    while pending:
        for i in pending[:]:
            if procs[i][2].poll() is not None:
                wall[i] = time.monotonic() - t0
                pending.remove(i)
        if pending:
            time.sleep(0.005)
    for i, (name, cfg, p) in enumerate(procs):
        stdout = (LOGS / f"portfolio7_{case}_r{round_no}_{label}_race_{name}.out").read_text()
        outcome = parse_outcome(stdout)
        append({"kind": "racer", "case": case, "round": round_no, "label": label,
                "racer": name, "cfg": " ".join(cfg),
                "wall_s": round(wall[i], 3), "outcome": outcome, "cap_s": cap,
                "cap_hit": outcome != "win", "rc": p.returncode})
    # emit the race-level first-decisive-finish line
    decisive = [wall[i] for i, (name, _, _) in enumerate(procs)
                if parse_outcome((LOGS / f"portfolio7_{case}_r{round_no}_{label}_race_{name}.out").read_text()) == "win"]
    append({"kind": "race", "case": case, "round": round_no, "label": label,
            "w_first_s": round(min(decisive), 3) if decisive else None,
            "n_racers": len(procs), "cap_s": cap})


def main():
    rounds = int(sys.argv[1]) if len(sys.argv) > 1 else 8
    only = sys.argv[2] if len(sys.argv) > 2 and sys.argv[2] != "-" else None
    label = sys.argv[3] if len(sys.argv) > 3 else "main"
    STATE.mkdir(exist_ok=True)
    LOGS.mkdir(exist_ok=True)
    for r in range(1, rounds + 1):        # rounds outer: cases round-robin per round
        for case, fen, cap in CASES:
            if only and case != only:
                continue
            baseline_run(case, fen, cap, r, label)
            race(case, fen, cap, r, label)


if __name__ == "__main__":
    main()
