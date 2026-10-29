#!/usr/bin/env python3
"""plan14 Phase 0 measurement driver (cut-context table spike).

Runs the release CLI (first-outcome, outcome-only, salt 0) over the Phase 0
case list and arm grid, parses the deterministic metrics, and appends each
result to `state/matrix.json`. Raw transcripts are not committed (AGENTS.md);
the parsed JSON plus this driver are the reproducible record.

Usage:
    python3 plan14_matrix.py [--binary PATH] [--only case1,case2]
                             [--arms base,on] [--tt 128,256] [--slots 21,22]

The spike is enabled through the temporary env gate:
    DFPN_CUTCTX=1 DFPN_CUTCTX_SLOTS=<b> DFPN_CUTCTX_STATS=1
"""

import argparse
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

sys.dont_write_bytecode = True

HERE = Path(__file__).resolve().parent
STATE = HERE / "state"
STATE.mkdir(exist_ok=True)
OUT = STATE / "matrix.json"

CASES = {
    # name: (fen, timeout seconds)
    "stress": ("4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21", 300),
    "m22": ("4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22", 300),
    "m20": ("4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R5RK w - - 4 20", 300),
    "m23": ("4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23", 300),
    "dec10": ("3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41", 300),
    "dec06": ("3r2k1/8/p7/2P2P2/8/1P1p3P/P2K2r1/6R1 w - - 1 32", 300),
    "dec13": ("r1bq1k1r/ppN4p/n1p1p3/3p1n1P/1b1P2P1/2P5/PP6/RNBQKB1R w KQ - 1 14", 300),
    "dec02": ("8/1k1p4/1P2p3/p2P1P2/P7/6p1/6K1/8 b - - 0 27", 300),
    "dec01": ("r5r1/5N1k/2p2p2/pp1p3p/3p4/2P1P3/P7/2bQ1R1K w - - 0 30", 120),
}

RE_OUTCOME = re.compile(r"^outcome: (\w+) length: (\d+)$", re.M)
RE_EVALS = re.compile(r"^evals: (\d+)$", re.M)
RE_CUTCTX = re.compile(r"^cutctx: (.*)$", re.M)


def parse_cutctx(line):
    fields = {}
    for part in line.split():
        k, _, v = part.partition("=")
        fields[k] = int(v)
    return fields


def run_arm(binary, case, fen, timeout, tt, slots):
    env = dict(os.environ)
    for k in list(env):
        if k.startswith("DFPN_CUTCTX"):
            del env[k]
    if slots is not None:
        env["DFPN_CUTCTX"] = "1"
        env["DFPN_CUTCTX_SLOTS"] = str(slots)
        env["DFPN_CUTCTX_STATS"] = "1"
    cmd = [
        binary,
        "--fen", fen,
        "--tt-size", str(tt),
        "--timeout", str(timeout),
        "--first-outcome",
        "--outcome-only",
    ]
    t0 = time.time()
    try:
        proc = subprocess.run(
            cmd, env=env, capture_output=True, text=True, timeout=timeout + 120
        )
        stdout, stderr, rc = proc.stdout, proc.stderr, proc.returncode
    except subprocess.TimeoutExpired as exc:
        stdout = exc.stdout or ""
        stderr = exc.stderr or ""
        if isinstance(stdout, bytes):
            stdout = stdout.decode()
        if isinstance(stderr, bytes):
            stderr = stderr.decode()
        rc = -1
    wall = time.time() - t0

    outcome_m = RE_OUTCOME.search(stdout)
    evals_m = RE_EVALS.search(stderr)
    cutctx_m = RE_CUTCTX.search(stderr)
    timed_out = "timeout" in stdout.split()

    result = {
        "case": case,
        "tt_mb": tt,
        "cutctx_slots_bits": slots,
        "timeout_s": timeout,
        "outcome": outcome_m.group(1) if outcome_m else None,
        "pv_len": int(outcome_m.group(2)) if outcome_m else None,
        "child_evals": int(evals_m.group(1)) if evals_m else None,
        "censored": timed_out,
        "exit_code": rc,
        "wall_s": round(wall, 2),
        "cutctx": parse_cutctx(cutctx_m.group(1)) if cutctx_m else None,
    }
    return result


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--binary", default="target/release/atomic_solver")
    ap.add_argument("--only", default=None)
    ap.add_argument("--arms", default="base,on")
    ap.add_argument("--tt", default="128,256")
    ap.add_argument("--slots", default="21,22")
    ap.add_argument("--out", default=None)
    args = ap.parse_args()

    global OUT
    if args.out:
        OUT = Path(args.out)

    cases = list(CASES)
    if args.only:
        cases = [c.strip() for c in args.only.split(",")]
    arms = [a.strip() for a in args.arms.split(",")]
    tts = [int(t) for t in args.tt.split(",")]
    slots = [int(s) for s in args.slots.split(",")]

    existing = []
    if OUT.exists():
        existing = json.loads(OUT.read_text())

    for case in cases:
        fen, timeout = CASES[case]
        for arm in arms:
            tt_list = tts if arm == "base" else tts
            for tt in tt_list:
                slot_list = [None] if arm == "base" else slots
                for sl in slot_list:
                    label = f"tt{tt} slots={'-' if sl is None else '2^' + str(sl)}"
                    print(f"[{case}] {label} ...", flush=True)
                    res = run_arm(args.binary, case, fen, timeout, tt, sl)
                    print(
                        f"    outcome={res['outcome']} evals={res['child_evals']} "
                        f"censored={res['censored']} wall={res['wall_s']}s",
                        flush=True,
                    )
                    if res["cutctx"]:
                        print(f"    {res['cutctx']}", flush=True)
                    existing.append(res)
                    OUT.write_text(json.dumps(existing, indent=2) + "\n")

    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
