#!/usr/bin/env python3
"""Plan 12 Phase 1 driver: restart-schedule grid over the spike-gated copy.

Runs the S0-S5 arm matrix (see plan12.md) against a *separate* build tree
whose `bounded_search` carries the temporary SPIKE_CHUNK_* env gate. The
product tree is never touched by this script.

Usage:
    python3 driver.py --root /tmp/p12/worktree --out results

Environment per arm (all unset = shipped schedule S0):
    SPIKE_CHUNK_BASE / SPIKE_CHUNK_FACTOR / SPIKE_CHUNK_CAP

Raw solver transcripts are written under a scratch `logs/` directory
(gitignored); only the parsed `results/*.json` files are committed.
"""

import json
import os
import re
import subprocess
import sys
import time

sys.dont_write_bytecode = True

STRESS = "4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21"
M22 = "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22"
M23_WHITE = "4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23"
DEC31 = "r1bq1k2/ppN3p1/2p1p2P/6P1/1b1P1p2/8/PP3P2/R2K1B1R b - - 0 15"

ARMS = {
    "S0": {},
    "S1": {"SPIKE_CHUNK_BASE": "18446744073709551615"},
    "S2": {"SPIKE_CHUNK_BASE": "500000", "SPIKE_CHUNK_FACTOR": "1000"},
    "S3": {"SPIKE_CHUNK_BASE": "500000", "SPIKE_CHUNK_CAP": "5000000"},
    "S4": {"SPIKE_CHUNK_BASE": "500000", "SPIKE_CHUNK_FACTOR": "1.25"},
    "S5": {"SPIKE_CHUNK_BASE": "500000", "SPIKE_CHUNK_FACTOR": "4"},
}

# (name, fen, extra args, timeout seconds)
CASES = [
    ("stress_fo", STRESS, ["--first-outcome"], 200),
    ("stress_default", STRESS, [], 60),
    ("m22_fo", M22, ["--first-outcome"], 60),
    ("m23_white_fo", M23_WHITE, ["--first-outcome"], 60),
    ("dec31_fo", DEC31, ["--first-outcome"], 20),
]

TTS = [256, 128]


def parse_solver(stdout, stderr):
    rec = {
        "outcome": None,
        "pv_len": None,
        "pv_status": None,
        "pre_exit": None,
        "evals": None,
        "nodes": None,
        "chunks": stderr.count("chunk done:"),
    }
    for line in stdout.splitlines():
        m = re.match(r"^outcome: (\w+) length: (\d+)", line)
        if m:
            rec["outcome"] = m.group(1)
            rec["pv_len"] = int(m.group(2))
        elif line.startswith("pv_status: "):
            rec["pv_status"] = line.split(": ", 1)[1]
        elif line.startswith("pre_exit: "):
            rec["pre_exit"] = line.split(": ", 1)[1]
    for line in stderr.splitlines():
        if line.startswith("evals: "):
            rec["evals"] = int(line.split(": ", 1)[1])
    m = re.search(r"nodes=(\d+)", stderr.splitlines()[-1]) if stderr.splitlines() else None
    if rec["pre_exit"]:
        m2 = re.search(r"nodes=(\d+)", rec["pre_exit"])
        if m2:
            rec["nodes"] = int(m2.group(1))
    return rec


def parse_quick(stdout):
    total = 0
    ok = 0
    wrong = 0
    rows = 0
    for line in stdout.splitlines():
        if not line.startswith("| "):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) < 7 or cells[0] in ("name", ""):
            continue
        if not re.match(r"^[a-z0-9_]+$", cells[0]):
            continue
        rows += 1
        if cells[1] == "ok":
            ok += 1
        if cells[1] == "wrong":
            wrong += 1
        try:
            total += int(cells[5])
        except ValueError:
            pass
    return {"evals": total, "cases": rows, "ok": ok, "wrong": wrong}


def main():
    root = None
    out = "results"
    args = sys.argv[1:]
    i = 0
    while i < len(args):
        if args[i] == "--root":
            root = args[i + 1]
            i += 2
        elif args[i] == "--out":
            out = args[i + 1]
            i += 2
        else:
            raise SystemExit(f"unknown option {args[i]}")
    if root is None:
        raise SystemExit("--root is required")

    solver = os.path.join(root, "target/release/atomic_solver")
    benchmark = os.path.join(root, "target/release/examples/benchmark")
    os.makedirs(out, exist_ok=True)
    logdir = os.path.join(os.path.dirname(os.path.abspath(out)), "logs")
    os.makedirs(logdir, exist_ok=True)

    all_results = {}
    for arm, overlay in ARMS.items():
        arm_results = {}
        env = dict(os.environ)
        for k in ("SPIKE_CHUNK_BASE", "SPIKE_CHUNK_FACTOR", "SPIKE_CHUNK_CAP"):
            env.pop(k, None)
        env.update(overlay)

        for tt in TTS:
            for name, fen, extra, timeout in CASES:
                key = f"{name}_tt{tt}"
                cmd = [solver, "--fen", fen, "--timeout", str(timeout), "--tt-size", str(tt)] + extra
                t0 = time.monotonic()
                proc = subprocess.run(cmd, env=env, capture_output=True, text=True)
                wall = time.monotonic() - t0
                with open(os.path.join(logdir, f"{arm}_{key}.out"), "w") as f:
                    f.write(proc.stdout)
                with open(os.path.join(logdir, f"{arm}_{key}.err"), "w") as f:
                    f.write(proc.stderr)
                rec = parse_solver(proc.stdout, proc.stderr)
                rec["wall_s"] = round(wall, 3)
                arm_results[key] = rec
                print(f"{arm} {key}: {rec['outcome']} evals={rec['evals']} wall={wall:.1f}s", flush=True)

            key = f"quick_tt{tt}"
            cmd = [benchmark, "--suite", "quick", "--first-outcome", "--timeout", "3", "--tt-size", str(tt)]
            t0 = time.monotonic()
            proc = subprocess.run(cmd, env=env, capture_output=True, text=True)
            wall = time.monotonic() - t0
            with open(os.path.join(logdir, f"{arm}_{key}.out"), "w") as f:
                f.write(proc.stdout)
            with open(os.path.join(logdir, f"{arm}_{key}.err"), "w") as f:
                f.write(proc.stderr)
            rec = parse_quick(proc.stdout)
            rec["wall_s"] = round(wall, 3)
            arm_results[key] = rec
            print(f"{arm} {key}: evals={rec['evals']} ok={rec['ok']}/{rec['cases']} wrong={rec['wrong']}", flush=True)

        all_results[arm] = arm_results
        with open(os.path.join(out, f"{arm}.json"), "w") as f:
            json.dump(arm_results, f, indent=2, sort_keys=True)

    with open(os.path.join(out, "grid.json"), "w") as f:
        json.dump(all_results, f, indent=2, sort_keys=True)


if __name__ == "__main__":
    main()
