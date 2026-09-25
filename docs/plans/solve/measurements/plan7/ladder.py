#!/usr/bin/env python3
"""Plan 7 harness (`solve` initiative): the sequential d4d5-p2 sizing
ladder (item 8, stage 1 of 3), driving the unmodified release binaries
as a black box (Python 3 stdlib only).  No product changes.

Subcommands:
  env     -- record environment metadata to env.json
  ladder  -- the sizing ladder, strictly sequential arms (both
             pre-registered in ../../plan7.md before any run;
             resumable per arm):
               L1: --timeout 3600  (fresh, this plan)
               L4: --timeout 14400 (fresh, this plan)
             (M2 = --timeout 7200 is on record as plan3 A1 and is
             reused, not re-run; comparability is checked in `analyze`.)
             Reference config: default 128 MB TT, --first-outcome,
             TT snapshot dump; NO --outcome-only (the `pre_exit: nodes=`
             line is the primary metric).  On a decisive completion,
             reconstruct_pt must print `validate: ok` for the result to
             count as an artifact.
  analyze -- apply the pre-registered stage-1 gate (../../plan7.md §3):
             rate law + eta fit (two-point L1/L4; three-point L1/M2/L4
             iff the comparability check passes), censoring floors,
             RSS profile, TT occupancy / fill point.  Writes
             analysis.json.
  status  -- print progress summary

Layout and conventions follow ../plan3/probe.py (which follows
../plan1/spike.py and ../plan2/ssfp.py): raw stdout/stderr captures
under logs/ (`<stem>.out` / `<stem>.err`), state JSONs under state/,
snapshots under snaps/ (kept until the report, then pruned to counts +
`.keep` markers).  The snapshot reader and binary paths are reused
from ../plan2/ssfp.py verbatim (`import ssfp`).  Max-RSS is recorded
per child process via os.wait4 ru_maxrss (no /usr/bin/time in this
container).

Protocol (../../plan7.md §2): strictly sequential (L1 first, then
L4), nothing else running concurrently, stdin from DEVNULL.  An arm
interrupted > 30 min in is NOT re-run (compute cap); the stage reports
with what completed.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import subprocess
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = HERE
REPO = os.path.abspath(os.path.join(ROOT, "..", "..", "..", "..", ".."))
PLAN2 = os.path.join(ROOT, "..", "plan2")
PLAN3 = os.path.join(ROOT, "..", "plan3")
PLAN3_STATE = os.path.join(PLAN3, "state")
sys.path.insert(0, PLAN2)
sys.dont_write_bytecode = True  # no __pycache__ litter in ../plan2

import ssfp  # noqa: E402  (verbatim snapshot reader + binary paths)

LOGS = os.path.join(ROOT, "logs")
STATE = os.path.join(ROOT, "state")
SNAPS = os.path.join(ROOT, "snaps")

# Pre-registered in plan7.md §1/§2 (fixed before any run).
ROOT_FEN = "rnbqkbnr/ppp1pppp/8/3p4/3P4/8/PPP1PPPP/RNBQKBNR w KQkq d6 0 2"
ARMS = (("ladder1", 3600), ("ladder4", 14400))
# 128 MB TT capacity in keys (plan3 §1 fact: 4,194,304 keys = full table).
TT128_CAPACITY_KEYS = 4_194_304
WATCHDOG_MARGIN = 600  # solver self-limits at --timeout; kill margin

# Pre-registered gate constants (plan7.md §3, fixed before any run).
COMPARABILITY_BAND = 0.10  # fresh-arm rates within ±10% of A1's
ETA_LINEAR_LO, ETA_LINEAR_HI = 0.9, 1.1


def ensure_dirs():
    for d in (LOGS, STATE, SNAPS):
        os.makedirs(d, exist_ok=True)


def run_raw(cmd, timeout, out_path, err_path):
    """Run a child with stdin=DEVNULL, raw stdout/stderr to files, and
    max-RSS via os.wait4. Returns (wall, rc, rss_kb)."""
    t0 = time.monotonic()
    with open(out_path, "w") as fo, open(err_path, "w") as fe:
        p = subprocess.Popen(
            cmd, stdin=subprocess.DEVNULL, stdout=fo, stderr=fe
        )
        killer = threading.Timer(timeout + WATCHDOG_MARGIN, p.kill)
        killer.start()
        try:
            _pid, status, ru = os.wait4(p.pid, 0)
        finally:
            killer.cancel()
        p.returncode = os.waitstatus_to_exitcode(status)
    return round(time.monotonic() - t0, 3), p.returncode, ru.ru_maxrss


def parse_stdout(text):
    """Parse solver stdout; same field semantics as plan2 ssfp.solve and
    plan3 probe.parse_stdout so state files stay comparable."""
    parsed = {
        "outcome": None,
        "pv": [],
        "pv_status": None,
        "exit_reason": None,
        "exit_outcome": None,
        "nodes": None,
        "tt_bytes": None,
        "tt_solved": None,
        "tt_unsolved": None,
        "timeout_flag": False,
    }
    for line in text.splitlines():
        if line == "timeout":
            parsed["timeout_flag"] = True
        elif line.startswith("outcome: "):
            parsed["outcome"] = line[9:].split()[0]
        elif line.startswith("pv: "):
            parsed["pv"] = line[4:].split()
        elif line.startswith("pv_status: "):
            parsed["pv_status"] = line[11:].strip()
        elif line.startswith("pre_exit: "):
            kv = dict(p.split("=", 1) for p in line[10:].split())
            parsed["exit_reason"] = kv.get("reason")
            parsed["exit_outcome"] = kv.get("outcome")
            parsed["nodes"] = int(kv["nodes"]) if "nodes" in kv else None
        elif line.startswith("tt_snapshot: "):
            kv = dict(p.split("=", 1) for p in line[13:].split()[1:])
            parsed["tt_bytes"] = int(kv["bytes"]) if "bytes" in kv else None
            parsed["tt_solved"] = int(kv["solved"]) if "solved" in kv else None
            parsed["tt_unsolved"] = (
                int(kv["unsolved"]) if "unsolved" in kv else None
            )
    parsed["censored"] = (
        parsed["exit_reason"] == "Timeout" or parsed["timeout_flag"]
    )
    return parsed


def solve_raw(fen, timeout, snap, log_stem):
    """One solver run (reference config, no --outcome-only) with RSS
    accounting; log files <stem>.out/.err hold the raw captures."""
    cmd = [ssfp.SOLVER, "--fen", fen, "--timeout", str(timeout),
           "--first-outcome", "--tt-dump-path", snap]
    out_path = os.path.join(LOGS, f"{log_stem}.out")
    err_path = os.path.join(LOGS, f"{log_stem}.err")
    wall, rc, rss_kb = run_raw(cmd, timeout, out_path, err_path)
    with open(out_path) as f:
        rec = parse_stdout(f.read())
    rec.update(
        fen=fen,
        wall=wall,
        rc=rc,
        rss_kb=rss_kb,
        cmd=" ".join(cmd),
    )
    return rec


def reconstruct(snap, out_pt, log_stem):
    """Offline reconstruction; returns (rc, validate, outcome, wall, rss_kb)."""
    cmd = [ssfp.RECONSTRUCT, "--snapshot", snap, "--out", out_pt]
    out_path = os.path.join(LOGS, f"{log_stem}.out")
    err_path = os.path.join(LOGS, f"{log_stem}.err")
    wall, rc, rss_kb = run_raw(cmd, 3600, out_path, err_path)
    with open(out_path) as f:
        text = f.read()
    validate = outcome = None
    for line in text.splitlines():
        if line.startswith("validate: "):
            validate = line[10:].strip()
        elif line.startswith("outcome: "):
            outcome = line[9:].strip()
    return rc, validate, outcome, wall, rss_kb


def tt_occupancy(snap):
    """Secondary metric (plan7.md §2): solved/unsolved key counts from
    the snapshot via the verbatim plan2 reader.  Registered as NOT a
    progress metric (capacity-capped, plan7.md §1)."""
    _h, solved, unsolved = ssfp.read_tt_snapshot(snap)
    return len(solved), len(unsolved)


# --------------------------------- env ---------------------------------------


def cmd_env(_args):
    meta = {
        "git_rev": subprocess.run(
            ["git", "rev-parse", "HEAD"], capture_output=True, text=True,
            cwd=REPO,
        ).stdout.strip(),
        "git_dirty": subprocess.run(
            ["git", "status", "--porcelain"], capture_output=True, text=True,
            cwd=REPO,
        ).stdout.strip(),
        "nproc": os.cpu_count(),
        "cpu_max": ssfp._read("/sys/fs/cgroup/cpu.max"),
        "memory_max_bytes": ssfp._int(ssfp._read("/sys/fs/cgroup/memory.max")),
        "date": time.strftime("%Y-%m-%d %H:%M:%S %z"),
    }
    with open(os.path.join(ROOT, "env.json"), "w") as f:
        json.dump(meta, f, indent=2)
    print(json.dumps(meta, indent=2))


# -------------------------------- arms ---------------------------------------


def _run_arm(tag, budget):
    state_path = os.path.join(STATE, f"arm_{tag}.json")
    if os.path.exists(state_path):
        with open(state_path) as f:
            print(f"arm {tag}: already done (state exists)")
        return
    snap = os.path.join(SNAPS, f"{tag}.tt")
    print(f"arm {tag}: starting ({budget}s budget) ...", flush=True)
    rec = solve_raw(ROOT_FEN, budget, snap, log_stem=f"arm_{tag}")
    rec["budget_secs"] = budget
    rec["tt_snap_solved"] = None
    rec["tt_snap_unsolved"] = None
    rec["tt_fill_point_bounded"] = None
    rec["validate"] = None
    rec["reconstruct_wall"] = None
    rec["reconstruct_rss_kb"] = None
    rec["combined_wall"] = None
    rec["artifact"] = False
    if rec["nodes"] is not None and rec["wall"] > 0:
        rec["rate_nodes_per_s"] = round(rec["nodes"] / rec["wall"], 1)
    if os.path.exists(snap):
        s, u = tt_occupancy(snap)
        rec["tt_snap_solved"] = s
        rec["tt_snap_unsolved"] = u
        occ = s + u
        # Fill point (plan7.md §2 secondary metric): if the table is at
        # capacity at censoring, it filled at or before the arm end.
        if occ >= TT128_CAPACITY_KEYS:
            rec["tt_fill_point_bounded"] = (
                f"<= {rec['wall']:.0f}s (table at capacity at censoring)"
            )
    if not rec["censored"] and rec.get("outcome") in ("win", "loss"):
        pt = os.path.join(ROOT, f"{tag}_reconstructed.bin")
        rc, validate, outcome, wall, rss_kb = reconstruct(
            snap, pt, f"arm_{tag}_reconstruct"
        )
        rec["reconstruct_rc"] = rc
        rec["validate"] = validate
        rec["reconstruct_outcome"] = outcome
        rec["reconstruct_wall"] = wall
        rec["reconstruct_rss_kb"] = rss_kb
        rec["combined_wall"] = round(rec["wall"] + wall, 3)
        rec["artifact"] = validate == "ok" and outcome == rec["outcome"]
    with open(state_path, "w") as f:
        json.dump(rec, f, indent=1)
    print(
        f"arm {tag}: outcome={rec['outcome']} nodes={rec['nodes']} "
        f"wall={rec['wall']} rate={rec.get('rate_nodes_per_s')} "
        f"rss={rec['rss_kb']}kB censored={rec['censored']} "
        f"tt_occ={rec['tt_snap_solved']}+{rec['tt_snap_unsolved']} "
        f"validate={rec['validate']}",
        flush=True,
    )


def cmd_ladder(args):
    ensure_dirs()
    if args.arm in (None, "both"):
        arms = ARMS  # strictly sequential, registered order L1 then L4
    else:
        arms = tuple(a for a in ARMS if a[0] == args.arm)
        if not arms:
            raise SystemExit(f"unknown arm {args.arm}")
    for tag, budget in arms:
        _run_arm(tag, budget)


# ------------------------------- analysis ------------------------------------


def _load(tag):
    with open(os.path.join(STATE, f"arm_{tag}.json")) as f:
        return json.load(f)


def _load_a1():
    """plan3 A1 (the ladder's M2 midpoint), reused not re-run."""
    with open(os.path.join(PLAN3_STATE, "arm_a1.json")) as f:
        return json.load(f)


def _fit(points):
    """Least-squares slope of log(nodes) vs log(wall) (the work-growth
    exponent eta) plus max |residual| in log space."""
    xs = [math.log(t) for t, _ in points]
    ys = [math.log(n) for _, n in points]
    n = len(xs)
    mx, my = sum(xs) / n, sum(ys) / n
    sxx = sum((x - mx) ** 2 for x in xs)
    sxy = sum((x - mx) * (y - my) for x, y in zip(xs, ys))
    eta = sxy / sxx
    resid = max(abs(y - (my + eta * (x - mx))) for x, y in zip(xs, ys))
    return eta, resid


def cmd_analyze(_args):
    l1 = _load("ladder1")
    l4 = _load("ladder4")
    a1 = _load_a1()
    a1_rate = a1["nodes"] / a1["wall"]
    out = {
        "root_fen": ROOT_FEN,
        "gate_constants": {
            "comparability_band_pm": COMPARABILITY_BAND,
            "eta_linear": [ETA_LINEAR_LO, ETA_LINEAR_HI],
        },
        "m2_reused": {
            "source": "../plan3/state/arm_a1.json",
            "budget_secs": a1["budget_secs"],
            "nodes": a1["nodes"],
            "wall": a1["wall"],
            "rate_nodes_per_s": round(a1_rate, 1),
            "rss_kb": a1["rss_kb"],
            "tt_solved": a1["tt_solved"],
            "tt_unsolved": a1["tt_unsolved"],
        },
        "arms": {},
    }

    rates = {}
    for tag, rec in (("ladder1", l1), ("ladder4", l4)):
        rate = rec.get("rate_nodes_per_s")
        rates[tag] = rate
        out["arms"][tag] = {
            "budget_secs": rec["budget_secs"],
            "nodes": rec["nodes"],
            "wall": rec["wall"],
            "rate_nodes_per_s": rate,
            "censored": rec["censored"],
            "rss_kb": rec["rss_kb"],
            "tt_stdout_solved": rec["tt_solved"],
            "tt_stdout_unsolved": rec["tt_unsolved"],
            "tt_snap_solved": rec["tt_snap_solved"],
            "tt_snap_unsolved": rec["tt_snap_unsolved"],
            "tt_occupancy": (
                (rec["tt_snap_solved"] or 0) + (rec["tt_snap_unsolved"] or 0)
                if rec["tt_snap_solved"] is not None else None
            ),
            "tt_fill_point_bounded": rec["tt_fill_point_bounded"],
            "validate": rec["validate"],
        }

    # Comparability check (pre-registered): fresh rates within ±10% of A1.
    comp = {}
    for tag in ("ladder1", "ladder4"):
        r = rates.get(tag)
        comp[tag] = (
            r is not None
            and abs(r - a1_rate) / a1_rate <= COMPARABILITY_BAND
        )
    a1_admitted = all(comp.values())
    out["comparability_check"] = {
        "a1_rate_nodes_per_s": round(a1_rate, 1),
        "per_arm_within_band": comp,
        "a1_admitted_as_midpoint": a1_admitted,
    }

    # Gate (pre-registered): COMPLETED / LINEAR / DEGRADING / SUPERLINEAR.
    verdict = None
    detail = {}
    if not (l1["censored"] and l4["censored"]):
        done = "ladder1" if not l1["censored"] else "ladder4"
        verdict = "COMPLETED"
        detail = {
            "completed_arm": done,
            "artifact": (l1 if done == "ladder1" else l4)["artifact"],
            "validate": (l1 if done == "ladder1" else l4)["validate"],
            "combined_wall": (l1 if done == "ladder1" else l4)[
                "combined_wall"
            ],
        }
    else:
        n1, n4 = l1["nodes"], l4["nodes"]
        eta2, resid2 = _fit([(3600, n1), (14400, n4)])
        detail["eta_two_point"] = round(eta2, 4)
        detail["two_point_max_log_residual"] = round(resid2, 4)
        detail["monotone_floors"] = bool(n1 < a1["nodes"] < n4)
        if a1_admitted:
            eta3, resid3 = _fit(
                [(3600, n1), (7200, a1["nodes"]), (14400, n4)]
            )
            detail["eta_three_point"] = round(eta3, 4)
            detail["three_point_max_log_residual"] = round(resid3, 4)
        eta = eta3 if a1_admitted else eta2
        if ETA_LINEAR_LO <= eta <= ETA_LINEAR_HI:
            verdict = "LINEAR"
        elif eta < ETA_LINEAR_LO:
            verdict = "DEGRADING"
        else:
            verdict = "SUPERLINEAR"
    out["gate"] = {"verdict": verdict, "detail": detail}

    # Rate law summary: the R_seq certification line for report7 §4.
    out["r_seq"] = {
        "certified_band": [3600, 14400 if l4["censored"] else l4["wall"]],
        "rates_nodes_per_s": {
            "L1": rates.get("ladder1"),
            "M2(plan3 A1)": round(a1_rate, 1),
            "L4": rates.get("ladder4"),
        },
        "floor_nodes": {
            "N_L1": l1["nodes"],
            "N_M2": a1["nodes"],
            "N_L4": l4["nodes"],
        },
    }

    with open(os.path.join(ROOT, "analysis.json"), "w") as f:
        json.dump(out, f, indent=1)
    print(json.dumps(out, indent=1))


# -------------------------------- status -------------------------------------


def cmd_status(_args):
    for tag, budget in ARMS:
        path = os.path.join(STATE, f"arm_{tag}.json")
        if os.path.exists(path):
            with open(path) as f:
                r = json.load(f)
            print(
                f"arm {tag}: outcome={r['outcome']} nodes={r['nodes']} "
                f"wall={r['wall']} rate={r.get('rate_nodes_per_s')} "
                f"censored={r['censored']} validate={r.get('validate')}"
            )
        else:
            print(f"arm {tag}: not run (budget {budget}s)")
    if os.path.exists(os.path.join(ROOT, "analysis.json")):
        with open(os.path.join(ROOT, "analysis.json")) as f:
            a = json.load(f)
        print(
            f"gate: {a['gate']['verdict']} "
            f"detail={a['gate'].get('detail', {})}"
        )
    else:
        print("analysis: not run")


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("env").set_defaults(fn=cmd_env)
    p = sub.add_parser("ladder")
    p.add_argument("--arm", choices=("ladder1", "ladder4", "both"),
                   default="both")
    p.set_defaults(fn=cmd_ladder)
    sub.add_parser("analyze").set_defaults(fn=cmd_analyze)
    sub.add_parser("status").set_defaults(fn=cmd_status)
    args = ap.parse_args()
    ensure_dirs()
    args.fn(args)


if __name__ == "__main__":
    main()
