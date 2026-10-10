#!/usr/bin/env python3
"""plan14 Phase-0 probe (item #23): in-search move-order tie prevalence.

Runs the pre-registered probe (plan14.md D4): the quick benchmark suite
(dec01-dec46 + m23-m29) plus m22_white and m20_white, each as a short
bounded solve

    ATOMIC_TIE_PROBE=1 atomic_solver --fen <FEN> --tt-size 128 \
        --first-outcome --outcome-only --budget 20000000 --timeout 120

with the temporary `tie_probe` instrumentation recording, per `sort_moves`
call: move count, top tie-group size, moves in tie-groups, top-2 tie.
The `tie_probe:` stderr line is parsed into state/probe.json.

The kill gate (pre-registered): proceed to productization only if the
fraction of `sort_moves` calls with a top-2 tie is >= 10% on >= 2 of the
four gate cases (m22_white, m20_white, dec13, dec10).

Hygiene arm: two quick-suite cases (dec15, dec10) at the plan12 1 B budget,
with the probe disabled and enabled, must reproduce the plan12 salt-0
child-eval counts exactly (the probe must not perturb the trajectory).

Usage:
    python3 probe_driver.py            # runs the probe
    BIN=... RAW=... python3 probe_driver.py
"""

import concurrent.futures
import json
import os
import re
import subprocess
import sys
import time

sys.dont_write_bytecode = True

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../../../.."))
BIN = os.environ.get("BIN", os.path.join(REPO, "target/release/atomic_solver"))
RAW = os.environ.get("RAW", "/tmp/plan14/probe")

BUDGET = 20_000_000
TIMEOUT_S = 120

GATE_CASES = ["m22_white", "m20_white", "dec13", "dec10"]
# plan12 salt-0 child evals (D5(a) baseline) for the hygiene arm.
HYGIENE = {"dec15": 1_077_420, "dec10": 4_262_128}


def load_quick_suite():
    """dec01-dec46 + move-order cases m23..m29 (the benchmark quick suite)."""
    cases = {}
    dec_path = os.path.join(REPO, "tests/fixtures/decisive_positions.txt")
    mo_path = os.path.join(REPO, "tests/fixtures/move_order_positions.txt")
    for path in (dec_path, mo_path):
        with open(path) as f:
            for line in f:
                line = line.strip()
                if not line or line.startswith("#"):
                    continue
                name, fen, expected = line.split(";")[:3]
                name = name.strip()
                mnum = name.split("_")[0][1:]
                if path == mo_path and mnum.isdigit() and int(mnum) < 23:
                    continue
                cases[name] = (fen.strip(), expected.strip() or None)
    return cases


CASES = load_quick_suite()
CASES["m22_white"] = (
    "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22", "win")
CASES["m20_white"] = (
    "4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R5RK w - - 4 20", "win")

TIE_RE = re.compile(r"tie_probe: calls=(\d+) moves=(\d+) tied_moves=(\d+) "
                    r"top2_tie_calls=(\d+) pct_top2=([\d.]+) max_top_tie=(\d+) "
                    r"hist=([\d,]+)")


def run_probe(name):
    fen, _ = CASES[name]
    out_path = os.path.join(RAW, name + ".out")
    err_path = os.path.join(RAW, name + ".err")
    cmd = [
        BIN, "--fen", fen, "--tt-size", "128",
        "--first-outcome", "--outcome-only",
        "--budget", str(BUDGET), "--timeout", str(TIMEOUT_S),
    ]
    env = dict(os.environ, ATOMIC_TIE_PROBE="1")
    t0 = time.time()
    with open(out_path, "w") as fo, open(err_path, "w") as fe:
        p = subprocess.run(cmd, stdout=fo, stderr=fe,
                           stdin=subprocess.DEVNULL, env=env)
    wall = time.time() - t0
    err = open(err_path).read()
    m = TIE_RE.search(err)
    evals = None
    for line in err.splitlines():
        if line.startswith("evals: "):
            evals = int(line.split()[1])
    censored = "budget exhausted" in open(out_path).read()
    rec = {
        "case": name,
        "evals": evals,
        "censored": censored,
        "wall_s": round(wall, 1),
        "rc": p.returncode,
    }
    if m:
        calls, moves, tied, top2, pct, maxtie, hist = m.groups()
        rec["probe"] = {
            "calls": int(calls), "moves": int(moves), "tied_moves": int(tied),
            "top2_tie_calls": int(top2), "pct_top2": float(pct),
            "max_top_tie": int(maxtie),
            "hist": [int(x) for x in hist.split(",")],
        }
    return rec


def run_hygiene(name, probe_on):
    fen, _ = CASES[name]
    tag = f"hyg_{name}_{'on' if probe_on else 'off'}"
    out_path = os.path.join(RAW, tag + ".out")
    err_path = os.path.join(RAW, tag + ".err")
    cmd = [
        BIN, "--fen", fen, "--tt-size", "128",
        "--first-outcome", "--outcome-only",
        "--budget", "1000000000", "--timeout", "600",
    ]
    env = dict(os.environ)
    if probe_on:
        env["ATOMIC_TIE_PROBE"] = "1"
    with open(out_path, "w") as fo, open(err_path, "w") as fe:
        subprocess.run(cmd, stdout=fo, stderr=fe,
                       stdin=subprocess.DEVNULL, env=env)
    evals = None
    for line in open(err_path):
        if line.startswith("evals: "):
            evals = int(line.split()[1])
    return {"tag": tag, "evals": evals, "want": HYGIENE[name]}


def main():
    os.makedirs(RAW, exist_ok=True)
    hyg = []
    for name in HYGIENE:
        for on in (False, True):
            hyg.append(run_hygiene(name, on))

    # gate cases first (small per-run bound), then the rest of the suite
    order = GATE_CASES + [c for c in CASES if c not in GATE_CASES]
    records = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=3) as ex:
        futures = [ex.submit(run_probe, name) for name in order]
        for fut in futures:
            records.append(fut.result())

    failures = [h for h in hyg if h["evals"] != h["want"]]
    out = {"hygiene": hyg, "probe": records,
           "hygiene_ok": not failures, "failures": failures}
    with open(os.path.join(os.path.dirname(os.path.abspath(__file__)),
                           "state", "probe.json"), "w") as f:
        json.dump(out, f, indent=1)

    print(json.dumps(out, indent=1)[:2000])


if __name__ == "__main__":
    main()
