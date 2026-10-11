#!/usr/bin/env python3
"""plan8 campaign: salt-seeded 4-racer process portfolio vs fresh salt-0 baseline.

Per plan8.md stages 3-4. Case order and racer set are pre-registered; no
post-selection swaps. Per round (rounds outer, cases in fixed order):
  (a) one fresh solo salt-0 baseline run, machine exclusive;
  (b) one race: R0(s0) R1(s2) R2(s3) R3(s5) launched concurrently, each with
      its own budget cap and the race wall cap as --timeout; wall recorded
      per racer; W_first = wall of the first decisive finish. After the first
      decisive finish the remaining racers run until a 240 s grace elapses or
      their own cap, then are SIGKILLed and recorded wall_killed (evals
      excluded from verdicts).

One JSON line per run -> state/races8_raw.jsonl (baseline / racer / race).
The pre-registered extension trigger fires automatically: if a case's
W_first round spread exceeds +-20 % of the median after 3 rounds, 3 more
rounds run for that case (flagged "extended": true).

Usage: campaign8.py pilot          # stage 3: single dec10 race, label pilot
       campaign8.py campaign       # stage 4: full campaign
"""

import json
import os
import signal
import subprocess
import sys
import time

sys.dont_write_bytecode = True

BIN = os.path.abspath("../../../../../target/release/atomic_solver")
HERE = os.path.dirname(os.path.abspath(__file__))
STATE = os.path.join(HERE, "state")
LOGS = os.path.join(HERE, "logs")

# (fen, evals budget, race wall cap s) — plan8.md fixed parameters
CASES = {
    "m20_white": ("4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R5RK w - - 4 20", 2_000_000_000, 500),
    "stress": ("4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21", 2_500_000_000, 450),
    "dec10": ("3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41", 1_000_000_000, 120),
    "m22_white": ("4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22", 1_000_000_000, 60),
    "dec13": ("r1bq1k1r/ppN4p/n1p1p3/3p1n1P/1b1P2P1/2P5/PP6/RNBQKB1R w KQ - 1 14", 1_000_000_000, 60),
    "rem12": ("rnbqkbnr/8/6pp/pppppp1B/3PPP2/N5PN/PPP4P/R1BQ1RK1 w kq - 0 10", 1_000_000_000, 120),
}
CASE_ORDER = ["m20_white", "stress", "dec10", "m22_white", "dec13", "rem12"]
RACERS = [("R0", 0), ("R1", 2), ("R2", 3), ("R3", 5)]
GRACE_S = 240.0
ROUNDS = 3
SPREAD_TRIGGER = 0.20


def parse_outcome(text):
    for line in text.splitlines():
        if line.startswith("outcome:"):
            return line.split()[1]
    return None


def parse_evals(err):
    for line in err.splitlines():
        if line.startswith("evals:"):
            return int(line.split()[1])
    return None


def loadavg():
    with open("/proc/loadavg") as f:
        return f.read().split()[:3]


def append(rec):
    with open(os.path.join(STATE, "races8_raw.jsonl"), "a") as f:
        f.write(json.dumps(rec) + "\n")
    print(json.dumps(rec), flush=True)


def run_solo(case, round_no, label):
    fen, budget, cap = CASES[case]
    tag = f"{label}_{case}_r{round_no}_solo_s0"
    cmd = [BIN, "--fen", fen, "--tt-size", "128", "--first-outcome", "--outcome-only",
           "--salt", "0", "--budget", str(budget), "--timeout", str(cap)]
    t0 = time.monotonic()
    p = subprocess.run(cmd, capture_output=True, text=True)
    wall = time.monotonic() - t0
    out, err = p.stdout, p.stderr
    open(os.path.join(LOGS, tag + ".out"), "w").write(out)
    open(os.path.join(LOGS, tag + ".err"), "w").write(err)
    outcome = parse_outcome(out)
    censored = ("budget" if "budget exhausted" in out
                else "timeout" if outcome is None else None)
    rec = {"kind": "baseline", "case": case, "round": round_no, "label": label,
           "salt": 0, "wall_s": round(wall, 3), "outcome": outcome,
           "evals": parse_evals(err), "censored": censored, "rc": p.returncode,
           "budget": budget, "cap_s": cap}
    append(rec)
    return rec


def race(case, round_no, label):
    fen, budget, cap = CASES[case]
    tag = f"{label}_{case}_r{round_no}_race"
    procs = {}
    files = {}
    for name, salt in RACERS:
        o = open(os.path.join(LOGS, f"{tag}_{name}.out"), "w")
        e = open(os.path.join(LOGS, f"{tag}_{name}.err"), "w")
        cmd = [BIN, "--fen", fen, "--tt-size", "128", "--first-outcome", "--outcome-only",
               "--salt", str(salt), "--budget", str(budget), "--timeout", str(cap)]
        p = subprocess.Popen(cmd, stdout=o, stderr=e, text=True)
        procs[name] = {"p": p, "salt": salt, "start": time.monotonic()}
        files[name] = (o, e)
    t0 = time.monotonic()
    load0 = loadavg()
    finished = {}   # name -> wall
    killed = set()
    w_first = None
    grace_deadline = None
    while len(finished) + len(killed) < len(RACERS):
        for name, info in procs.items():
            if name in finished or name in killed:
                continue
            if info["p"].poll() is not None:
                w = time.monotonic() - t0
                finished[name] = w
                out = open(os.path.join(LOGS, f"{tag}_{name}.out")).read()
                outcome = parse_outcome(out)
                if w_first is None and outcome in ("win", "loss"):
                    w_first = w
                    grace_deadline = time.monotonic() + GRACE_S
        now = time.monotonic()
        if grace_deadline is not None and now >= grace_deadline:
            for name, info in procs.items():
                if name not in finished and name not in killed:
                    info["p"].send_signal(signal.SIGKILL)
                    killed.add(name)
            continue
        time.sleep(0.005)
    load1 = loadavg()
    for name, info in procs.items():
        p = info["p"]
        try:
            p.wait(timeout=5)
        except subprocess.TimeoutExpired:
            p.kill()
            p.wait()
        files[name][0].close()
        files[name][1].close()
        out = open(os.path.join(LOGS, f"{tag}_{name}.out")).read()
        err = open(os.path.join(LOGS, f"{tag}_{name}.err")).read()
        outcome = parse_outcome(out)
        wall = finished.get(name, time.monotonic() - t0)
        if name in killed:
            status = "wall_killed"
        elif outcome in ("win", "loss"):
            status = "decisive"
        elif "budget exhausted" in out:
            status = "censored_budget"
        elif outcome is None:
            status = "censored_timeout"
        else:
            status = f"other_rc{p.returncode}"
        append({"kind": "racer", "case": case, "round": round_no, "label": label,
                "racer": name, "salt": info["salt"], "wall_s": round(wall, 3),
                "outcome": outcome, "evals": parse_evals(err), "status": status,
                "rc": p.returncode, "budget": budget, "cap_s": cap})
    append({"kind": "race", "case": case, "round": round_no, "label": label,
            "w_first_s": round(w_first, 3) if w_first is not None else None,
            "n_racers": len(RACERS), "cap_s": cap,
            "load_before": load0, "load_after": load1})


def w_first_rounds(case, label):
    vals = []
    with open(os.path.join(STATE, "races8_raw.jsonl")) as f:
        for line in f:
            r = json.loads(line)
            if r["kind"] == "race" and r["case"] == case and r["label"] == label \
                    and r["w_first_s"] is not None:
                vals.append(r["w_first_s"])
    return vals


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else "campaign"
    os.makedirs(STATE, exist_ok=True)
    os.makedirs(LOGS, exist_ok=True)
    if mode == "pilot":
        race("dec10", 0, "pilot")
        return
    label = "main"
    done = set()
    for r in range(1, ROUNDS + 1):
        for case in CASE_ORDER:
            run_solo(case, r, label)
            race(case, r, label)
            done.add(case)
    # Pre-registered extension trigger: W_first spread > +-20 % of the median.
    for case in CASE_ORDER:
        vals = w_first_rounds(case, label)
        if len(vals) >= 2 and vals and (max(vals) - min(vals)) / (sum(vals) / len(vals)) > SPREAD_TRIGGER:
            print(f"EXTENSION TRIGGER {case}: W_first spread {vals}", flush=True)
            for r in range(ROUNDS + 1, 2 * ROUNDS + 1):
                run_solo(case, r, label)
                race(case, r, label)
    print("CAMPAIGN DONE")


if __name__ == "__main__":
    main()
