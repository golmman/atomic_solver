#!/usr/bin/env python3
"""plan15 driver: m22-collapse diagnosis under leaf-initialization arms.

Runs the pre-registered grid (plan15.md P1-P3) via the temporary
`probe_solve` example (instrumented build; counters always on, arms behind
`PROBE_*` env vars). Resumable: a job whose JSON output exists is skipped.

  anchor_b : counter-only identity anchors (run manually after anchors_a)
  p1       : P1 anatomy, base + mob arms x {stress, m22, dec13, dec10}, salt 0
  p2       : P2 selection traces, base + mob x {m22, dec13}, salt 0
  p3       : P3 micro-arms {mob-or, mob-and, mobcap8, moblog}
             m22 x salts {0,1,2} @ 1B; stress @ 300M; dec13/dec10 @ 1B (salt 0)
             plus base-m22 at salts {1,2} for per-salt ratio pairing
  all      : p1 + p2 + p3

Usage:
    python3 probe15.py [phase]
    BIN=... RAW=... python3 probe15.py
"""

import concurrent.futures
import json
import os
import subprocess
import sys
import time

sys.dont_write_bytecode = True

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "../../../.."))
BIN = os.environ.get(
    "BIN", "/tmp/plan15/target/release/examples/probe_solve")
RAW = os.environ.get("RAW", "/tmp/plan15/raw")
WORKERS = 3

FENS = {
    "stress": "4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21",
    "m22": "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22",
    "dec13": "r1bq1k1r/ppN4p/n1p1p3/3p1n1P/1b1P2P1/2P5/PP6/RNBQKB1R w KQ - 1 14",
    "dec10": "3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41",
    "dec15": "r3k1r1/1b1n3p/3p1pPb/pp1Pp3/P3P3/4B3/1PP4P/R3KBR1 w Qq - 1 20",
}
BUDGET = 1_000_000_000
BUDGET_STRESS = 2_500_000_000
BUDGET_STRESS_P3 = 300_000_000
TIMEOUT = 600


def jobs_for(phase):
    """Yield (name, env, argv) triples."""
    def base_cmd(case, budget):
        return [BIN, "--fen", FENS[case], "--tt", "128",
                "--budget", str(budget), "--timeout", str(TIMEOUT),
                "--salt", "0"]

    def salted_cmd(case, salt, budget):
        return [BIN, "--fen", FENS[case], "--tt", "128",
                "--budget", str(budget), "--timeout", str(TIMEOUT),
                "--salt", str(salt)]

    out = []
    if phase in ("anchor_b", "all"):
        for case in ("m22", "dec13", "dec10", "dec15"):
            out.append((f"anchor_b_{case}", {}, base_cmd(case, BUDGET)))
    if phase in ("p1", "all"):
        for arm, init in (("base", None), ("mob", "mob")):
            for case in ("stress", "m22", "dec13", "dec10"):
                budget = BUDGET_STRESS if case == "stress" else BUDGET
                env = {"PROBE_INIT": init} if init else {}
                out.append((f"p1_{case}_{arm}", env, base_cmd(case, budget)))
    if phase in ("p2", "all"):
        for arm, init in (("base", None), ("mob", "mob")):
            for case in ("m22", "dec13"):
                env = {"PROBE_INIT": init} if init else {}
                env = dict(env, PROBE_TRACE=f"{RAW}/p2_{case}_{arm}.trace")
                out.append((f"p2_{case}_{arm}", env, base_cmd(case, BUDGET)))
    if phase in ("p3", "all"):
        # baseline m22 at salts 1,2 for per-salt ratio pairing (salt 0 from P1)
        for salt in (1, 2):
            out.append((f"p3_m22_base_s{salt}", {},
                        salted_cmd("m22", salt, BUDGET)))
        for arm in ("mob-or", "mob-and", "mobcap8", "moblog"):
            for salt in (0, 1, 2):
                out.append((f"p3_m22_{arm}_s{salt}",
                            {"PROBE_INIT": arm},
                            salted_cmd("m22", salt, BUDGET)))
            out.append((f"p3_stress_{arm}", {"PROBE_INIT": arm},
                        base_cmd("stress", BUDGET_STRESS_P3)))
            out.append((f"p3_dec13_{arm}", {"PROBE_INIT": arm},
                        base_cmd("dec13", BUDGET)))
            out.append((f"p3_dec10_{arm}", {"PROBE_INIT": arm},
                        base_cmd("dec10", BUDGET)))
    return out


def run(job):
    name, env, cmd = job
    json_path = os.path.join(RAW, name + ".json")
    if os.path.exists(json_path) and os.path.getsize(json_path) > 0:
        return name, "skip"
    t0 = time.time()
    full_env = dict(os.environ, **env)
    with open(os.path.join(RAW, name + ".out"), "w") as fo, \
            open(os.path.join(RAW, name + ".err"), "w") as fe:
        p = subprocess.run(cmd, stdout=fo, stderr=fe,
                           stdin=subprocess.DEVNULL, env=full_env)
    wall = time.time() - t0
    if p.returncode != 0:
        return name, f"FAILED rc={p.returncode}"
    # probe_solve prints exactly one JSON line on stdout
    with open(json_path, "w") as f:
        for line in open(os.path.join(RAW, name + ".out")):
            if line.startswith("{"):
                f.write(line)
                break
    return name, f"ok wall={wall:.1f}s"


def main():
    phase = sys.argv[1] if len(sys.argv) > 1 else "all"
    os.makedirs(RAW, exist_ok=True)
    if not os.path.exists(BIN):
        raise SystemExit(f"probe binary not found: {BIN}")
    jobs = jobs_for(phase)
    print(f"{len(jobs)} jobs, phase={phase}, workers={WORKERS}", flush=True)
    with concurrent.futures.ThreadPoolExecutor(WORKERS) as ex:
        for name, status in ex.map(run, jobs):
            print(name, status, flush=True)
    print("ALL DONE")


if __name__ == "__main__":
    main()
