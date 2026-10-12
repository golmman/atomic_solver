#!/usr/bin/env python3
"""plan11 races: A0 (portfolio, facts off) vs A1 (low-tax facts carrier).

One race = 4 unmodified solver processes on one FEN, the plan8/plan9 racer
set {R0=0, R1=2, R2=3, R3=5}, the measured configuration
(--tt-size 128 --first-outcome --outcome-only --salt S --budget CAP
 --timeout WALLCAP), and the report8 stop-when-proven kill policy: the
first decisive outcome kills every remaining racer immediately (the winner
runs to natural completion).

Arms (plan11; no escalation arm — A2 is deleted):
  A0  plain portfolio (facts flags absent) — deterministic per racer.
  A1  cooperative (plan11 carrier): R0 pure (--facts-out only), R1..R3
      cooperative (--facts-out <race>/facts-s<S>.bin --facts-in <race>).
      The carrier is key-only emission (FactExporter hook, PATH_LEN_CAP=24
      admission filter) + chunk-boundary import (no threads).

Usage:
    python3 races11.py --case m22_white --arm A1 --tag canary
    python3 races11.py --case dec10 --arm A0 --round 2

Per race the driver writes:
  logs/<case>_<arm><tag>_r<round>/racer_s<S>.{out,err}   raw transcripts
      (never committed)
  state/<case>_<arm><tag>_r<round>.json                  parsed race record
Existing state files are skipped (resumable).
"""

import argparse
import json
import os
import queue
import subprocess
import sys
import threading
import time

sys.dont_write_bytecode = True

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../../../.."))
BIN = os.environ.get(
    "BIN", os.path.join(REPO, "target/release/atomic_solver")
)
HERE = os.path.dirname(os.path.abspath(__file__))
RAW = os.environ.get("RAW", os.path.join(HERE, "logs"))
STATE = os.environ.get("STATE", os.path.join(HERE, "state"))

# Frozen plan12 corpus (FENs identical to
# ../../research/measurements/plan12/driver.py).
FENS = {
    "m20_white": "4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R5RK w - - 4 20",
    "stress": "4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21",
    "m22_white": "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22",
    "dec10": "3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41",
    "rem12": "rnbqkbnr/8/6pp/pppppp1B/3PPP2/N5PN/PPP4P/R1BQ1RK1 w kq - 0 10",
    "dec13": "r1bq1k1r/ppN4p/n1p1p3/3p1n1P/1b1P2P1/2P5/PP6/RNBQKB1R w KQ - 1 14",
}

# plan11 fixed parameters (identical to plan10): evals cap, race wall cap
# (seconds).
CAPS = {
    "m20_white": (2_000_000_000, 500),
    "stress": (2_500_000_000, 450),
    "m22_white": (1_000_000_000, 60),
    "dec10": (1_000_000_000, 120),
    "rem12": (1_000_000_000, 120),
    "dec13": (1_000_000_000, 60),
}

RACER_SALTS = [0, 2, 3, 5]
PURE_SALT = 0


def spawn_racer(solver, fen, salt, budget, wall, facts_out, facts_in):
    cmd = [
        solver,
        "--fen",
        fen,
        "--tt-size",
        "128",
        "--first-outcome",
        "--outcome-only",
        "--salt",
        str(salt),
        "--budget",
        str(budget),
        "--timeout",
        str(wall),
    ]
    if facts_out:
        cmd += ["--facts-out", facts_out]
    if facts_in:
        cmd += ["--facts-in", facts_in]
    env = dict(os.environ)
    env.pop("ATOMIC_SOLVER_FACTS_LOSS_ONLY", None)
    p = subprocess.Popen(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        stdin=subprocess.DEVNULL,
        env=env,
    )
    return p, cmd


def pump(pipe, salt, stream, events):
    for line in iter(pipe.readline, b""):
        events.put((salt, stream, line.decode("utf-8", "replace")))
    pipe.close()


def parse_kv(text):
    out = {}
    for token in text.split():
        k, _, v = token.partition("=")
        try:
            out[k] = int(v)
        except ValueError:
            out[k] = v
    return out


def run_race(case, arm, tag, rnd):
    assert arm in ("A0", "A1"), "plan11 has no A2 arm"
    fen = FENS[case]
    budget, wall = CAPS[case]
    name = f"{case}_{arm}{tag}_r{rnd}"
    race_logs = os.path.join(RAW, name)
    state_path = os.path.join(STATE, name + ".json")
    if os.path.exists(state_path):
        with open(state_path) as fh:
            return json.load(fh), "skip"
    os.makedirs(race_logs, exist_ok=True)
    os.makedirs(STATE, exist_ok=True)
    # Facts dir on tmpfs: facts dirs are per-race and ephemeral, and a
    # backing filesystem's writeback blocking is an environmental confound
    # that swamps the carrier tax under measurement (the same confound
    # contaminated plan10's stage-1 solo numbers — see report11's findings).
    facts_root = os.environ.get("FACTS_TMP", "/tmp/plan11_facts")
    facts_dir = os.path.join(facts_root, name)

    events = queue.Queue()
    children = {}
    cmds = {}
    for salt in RACER_SALTS:
        facts_out = None
        facts_in = None
        if arm == "A1":
            os.makedirs(facts_dir, exist_ok=True)
            facts_out = os.path.join(facts_dir, f"facts-s{salt}.bin")
            if salt != PURE_SALT:
                facts_in = facts_dir
        p, cmd = spawn_racer(BIN, fen, salt, budget, wall, facts_out, facts_in)
        children[salt] = p
        cmds[salt] = cmd
        for stream, pipe in (("stdout", p.stdout), ("stderr", p.stderr)):
            threading.Thread(
                target=pump, args=(pipe, salt, stream, events), daemon=True
            ).start()

    # Per-racer observation state.
    obs = {
        salt: {
            "decisive": None,        # first decisive outcome word
            "decisive_at_ms": None,
            "final_outcome": None,   # stdout outcome: draw
            "budget_exhausted": False,
            "killed": False,
            "exited": False,
            "wall_ms": None,
            "rc": None,
            "evals": None,
            "facts_import": None,
            "facts_export": None,
        }
        for salt in RACER_SALTS
    }
    winner = None
    start = time.time()

    def kill(salt):
        p = children[salt]
        if p.poll() is None:
            p.kill()
        obs[salt]["killed"] = True
        obs[salt]["wall_ms"] = int((time.time() - start) * 1000)

    def live():
        return [
            s
            for s in RACER_SALTS
            if not obs[s]["exited"] and not obs[s]["killed"]
        ]

    def mark_exits():
        for salt, p in children.items():
            if not obs[salt]["exited"] and p.poll() is not None:
                obs[salt]["exited"] = True
                obs[salt]["rc"] = p.returncode
                if not obs[salt]["killed"]:
                    obs[salt]["wall_ms"] = int((time.time() - start) * 1000)

    def handle(salt, stream, line):
        line = line.strip()
        o = obs[salt]
        with open(
            os.path.join(race_logs, f"racer_s{salt}.{stream}"), "a"
        ) as fh:
            fh.write(line + "\n")
        if line.startswith("outcome: "):
            word = line.split()[1]
            if word in ("win", "loss"):
                if o["decisive"] is None:
                    o["decisive"] = word
            elif word == "draw" and stream == "stdout":
                o["final_outcome"] = "draw"
        elif line == "budget exhausted":
            o["budget_exhausted"] = True
        # Winner bookkeeping: the first decisive line anywhere wins the race
        # (stop-when-proven: kill every live racer but the winner).
        nonlocal winner
        if winner is None:
            for s in RACER_SALTS:
                if obs[s]["decisive"] is not None:
                    winner = s
                    obs[s]["decisive_at_ms"] = int((time.time() - start) * 1000)
                    for other in live():
                        if other != s:
                            kill(other)
                    break

    while True:
        mark_exits()
        now = time.time()
        if now - start >= wall:
            for salt in live():
                kill(salt)
        try:
            salt, stream, line = events.get(timeout=0.02)
        except queue.Empty:
            if all(obs[s]["exited"] or obs[s]["killed"] for s in RACER_SALTS):
                # Everyone settled; drain trailing lines briefly.
                while True:
                    try:
                        salt, stream, line = events.get(timeout=0.05)
                    except queue.Empty:
                        break
                    else:
                        handle(salt, stream, line)
                break
            continue
        handle(salt, stream, line)

    # Settle: reap exit codes and final walls.
    for salt, p in children.items():
        try:
            p.wait(timeout=10)
        except subprocess.TimeoutExpired:
            p.kill()
            p.wait()
        obs[salt]["rc"] = p.returncode
        obs[salt]["exited"] = True
        if not obs[salt]["killed"]:
            obs[salt]["wall_ms"] = int((time.time() - start) * 1000)

    # Parse the stderr facts summaries from the raw transcripts (robust
    # against pump-thread scheduling).
    for salt in RACER_SALTS:
        err_path = os.path.join(race_logs, f"racer_s{salt}.stderr")
        if os.path.exists(err_path):
            text = open(err_path).read()
            for line in text.splitlines():
                if line.startswith("facts_import: "):
                    obs[salt]["facts_import"] = parse_kv(
                        line[len("facts_import: ") :]
                    )
                elif line.startswith("facts_export: "):
                    obs[salt]["facts_export"] = parse_kv(
                        line[len("facts_export: ") :]
                    )
                elif line.startswith("evals: "):
                    obs[salt]["evals"] = int(line[len("evals: ") :])

    record = build_record(name, case, arm, tag, rnd, obs, winner, budget, wall, cmds)
    with open(state_path, "w") as fh:
        json.dump(record, fh, indent=1)
    return record, "ok"


def build_record(name, case, arm, tag, rnd, obs, winner, budget, wall, cmds):
    racers = []
    for salt in RACER_SALTS:
        o = obs[salt]
        if o["decisive"]:
            outcome = o["decisive"]
            censored = False
        elif o["budget_exhausted"]:
            # plan8 censoring discipline: never a proven draw.
            outcome = "draw"
            censored = True
        elif o["killed"]:
            outcome = "killed"
            censored = False
        elif o["final_outcome"] == "draw":
            outcome = "draw"
            censored = False
        else:
            outcome = "error"
            censored = False
        # Killed racers have their evals excluded (plan8 discipline).
        evals = None if (o["killed"] and not o["decisive"]) else o["evals"]
        racers.append(
            {
                "salt": salt,
                "pure": salt == PURE_SALT,
                "outcome": outcome,
                "censored": censored,
                "killed": o["killed"],
                "exited": o["exited"],
                "rc": o["rc"],
                "wall_ms": o.get("wall_ms"),
                "evals": evals,
                "decisive_at_ms": o.get("decisive_at_ms"),
                "facts_import": o["facts_import"],
                "facts_export": o["facts_export"],
            }
        )
    return {
        "race": name,
        "case": case,
        "arm": arm,
        "tag": tag,
        "round": rnd,
        "budget": budget,
        "wall_cap_s": wall,
        "winner_salt": winner,
        "racers": racers,
        "cmds": {str(s): c for s, c in cmds.items()},
        "host_nproc": os.cpu_count(),
        "finished_s": time.strftime("%Y-%m-%dT%H:%M:%S"),
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--case", required=True, choices=sorted(FENS))
    ap.add_argument("--arm", default="A0", choices=["A0", "A1"])
    ap.add_argument("--tag", default="", help="e.g. canary")
    ap.add_argument("--round", type=int, default=1)
    args = ap.parse_args()
    if not os.path.exists(BIN):
        raise SystemExit(f"solver binary not found: {BIN}")
    rec, status = run_race(args.case, args.arm, args.tag, args.round)
    winner = rec["winner_salt"]
    w = (
        next((r for r in rec["racers"] if r["salt"] == winner), None)
        if winner is not None
        else None
    )
    print(
        f"{rec['race']}: {status} winner={winner} "
        + (
            f"evals={w['evals']} wall={w['wall_ms']}ms outcome={w['outcome']}"
            if w
            else "none"
        ),
        flush=True,
    )


if __name__ == "__main__":
    main()
