#!/usr/bin/env python3
"""Plan 3 harness (`solve` initiative): quiet-root finishability probe
(Task A, arms A1/A2) and the ±1-ply substrate measurement (Task B),
driving the unmodified release binaries as a black box (Python 3
stdlib only).

Subcommands:
  env    -- record environment metadata to env.json
  arma   -- Task A: the 2 h finishability probe on the d4d5 p2 root,
            strictly sequential arms (default both, resumable):
              A1: --timeout 7200 (TT default 128 MB)
              A2: --timeout 7200 --tt-size 1024
            On a decisive completion: reconstruct_pt must print
            `validate: ok` for the result to count as an artifact;
            the combined metric wall(find) + wall(verify) is recorded.
  armb   -- Task B: ±1-ply substrate measurement. For each of the 5
            quiet lines, the shallowest plan1 ladder position whose
            8 s steer solve emitted a PV (`steer: "pv"` in
            ../../plan1/state/ladder_<line>.json) is the parent; the
            child = parent + first steer-PV move (exact ±1 step, clock
            semantics preserved by replaying through the engine).
            Parent and child each run one 120 s cost run with the
            snapshot kept; metrics (pre-registered, do not tune):
              avail(P→C) = |P.solved ∩ C.all| / |C.all|
              carry(P→C) = |P.solved ∩ C.all| / |P.solved|
            Gate: median avail >= 5% GO (follow-up anchor-preload test
            proposal), < 1% dead, between: judgment call.
  status -- print progress summary

Layout and conventions follow ../plan1/spike.py and ../plan2/ssfp.py:
raw stdout/stderr captures under logs/, state JSONs under state/,
snapshots under snaps/ (kept until the report, then pruned to counts +
`.keep` markers). The snapshot reader is reused from ../plan2/ssfp.py
verbatim (`import ssfp`). Max-RSS is recorded per child process via
os.wait4 ru_maxrss (plan protocol's `/usr/bin/time -v` fallback; no
/usr/bin/time in this container).
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = HERE
REPO = os.path.abspath(os.path.join(ROOT, "..", "..", "..", "..", ".."))
PLAN2 = os.path.join(ROOT, "..", "plan2")
PLAN1 = os.path.join(ROOT, "..", "plan1")
PLAN1_STATE = os.path.join(PLAN1, "state")
sys.path.insert(0, PLAN2)

import ssfp  # noqa: E402  (verbatim snapshot reader + binary paths)

LOGS = os.path.join(ROOT, "logs")
STATE = os.path.join(ROOT, "state")
SNAPS = os.path.join(ROOT, "snaps")

# Pre-registered in plan3.md §2/§3 (fixed before any run).
ROOT_FEN = "rnbqkbnr/ppp1pppp/8/3p4/3P4/8/PPP1PPPP/RNBQKBNR w KQkq d6 0 2"
ARM_TIMEOUT = 7200
COST_TIMEOUT = 120  # must match plan1/plan2 cost runs
LINES = ("startpos", "e4", "d4", "nf3", "d4d5")
WATCHDOG_MARGIN = 600  # solver self-limits at --timeout; kill margin

GATE_AVAIL_GO = 0.05
GATE_AVAIL_DEAD = 0.01


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
    """Parse solver stdout; same field semantics as plan2 ssfp.solve so
    state files stay comparable."""
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


def solve_raw(fen, timeout, snap, extra=None, log_stem=None, tt_size=None):
    """One solver run with RSS accounting; log files <stem>.out/.err hold
    the raw captures."""
    cmd = [ssfp.SOLVER, "--fen", fen, "--timeout", str(timeout),
           "--first-outcome", "--tt-dump-path", snap]
    if tt_size:
        cmd += ["--tt-size", str(tt_size)]
    if extra:
        cmd += extra
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
        tt_size_flag=tt_size,
        cmd=" ".join(cmd),
    )
    return rec


def reconstruct(snap, out_pt, log_stem):
    """Offline reconstruction; returns (rc, validate, outcome, wall, rss_kb)."""
    cmd = [ssfp.RECONSTRUCT, "--snapshot", snap, "--out", out_pt]
    out_path = os.path.join(LOGS, f"{log_stem}.out")
    err_path = os.path.join(LOGS, f"{log_stem}.err")
    wall, rc, rss_kb = run_raw(cmd, ARM_TIMEOUT, out_path, err_path)
    with open(out_path) as f:
        text = f.read()
    validate = outcome = None
    for line in text.splitlines():
        if line.startswith("validate: "):
            validate = line[10:].strip()
        elif line.startswith("outcome: "):
            outcome = line[9:].strip()
    return rc, validate, outcome, wall, rss_kb


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


# -------------------------------- Task A -------------------------------------


def _run_arm(tag, tt_size):
    state_path = os.path.join(STATE, f"arm_{tag}.json")
    if os.path.exists(state_path):
        with open(state_path) as f:
            print(f"arm {tag}: already done (state exists)")
        return
    snap = os.path.join(SNAPS, f"arm{tag}.tt")
    print(f"arm {tag}: starting ({ARM_TIMEOUT}s budget) ...", flush=True)
    rec = solve_raw(
        ROOT_FEN, ARM_TIMEOUT, snap, tt_size=tt_size, log_stem=f"arm_{tag}"
    )
    rec["budget_secs"] = ARM_TIMEOUT
    rec["validate"] = None
    rec["reconstruct_wall"] = None
    rec["reconstruct_rss_kb"] = None
    rec["combined_wall"] = None
    rec["artifact"] = False
    if not rec["censored"] and rec.get("outcome") in ("win", "loss"):
        pt = os.path.join(ROOT, f"arm{tag}_reconstructed.bin")
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
        f"wall={rec['wall']} rss={rec['rss_kb']}kB censored={rec['censored']} "
        f"validate={rec['validate']}",
        flush=True,
    )


def cmd_arma(args):
    ensure_dirs()
    arms = ("a1", "a2") if args.arm in (None, "both") else (args.arm,)
    for tag in arms:
        _run_arm(tag, None if tag == "a1" else 1024)


# -------------------------------- Task B -------------------------------------


def _parents():
    """Pre-registered parents: the shallowest plan1 ladder position with
    steer == "pv" per quiet line (ties by lower ply = first hit)."""
    parents = []
    for line in LINES:
        with open(os.path.join(PLAN1_STATE, f"ladder_{line}.json")) as f:
            d = json.load(f)
        p = next(
            (p for p in d["positions"] if p.get("steer") == "pv"), None
        )
        if p is None:
            raise RuntimeError(f"no pv-steered ladder position for {line}")
        parents.append(
            {
                "line": line,
                "ply": p["ply"],
                "fen": p["fen"],
                "pv_move": p["step"]["pv"][0],
                "steer_outcome": p["step"]["outcome"],
                "steer_nodes": p["step"]["nodes"],
            }
        )
    return parents


def _child_fen(parent):
    """parent + first PV move, replayed through the engine so clock
    semantics are preserved by construction (examples/replay)."""
    r = subprocess.run(
        [ssfp.REPLAY, parent["fen"], parent["pv_move"]],
        capture_output=True, text=True, stdin=subprocess.DEVNULL, timeout=60,
    )
    if r.returncode != 0:
        raise RuntimeError(f"replay failed for {parent}: {r.stderr[:300]}")
    m = re.search(r"^fen: (.+)$", r.stdout, re.M)
    if not m:
        raise RuntimeError(f"replay printed no fen: {r.stdout[:300]}")
    return m.group(1).strip()


def cmd_armb(args):
    ensure_dirs()
    parents = _parents()
    pairs = []
    for parent in parents:
        line, ply = parent["line"], parent["ply"]
        p_state = os.path.join(STATE, f"tb_{line}_p{ply}.json")
        c_state = os.path.join(STATE, f"tc_{line}.json")
        if not os.path.exists(p_state):
            snap = os.path.join(SNAPS, f"tb_{line}_p{ply}.tt")
            rec = solve_raw(
                parent["fen"], COST_TIMEOUT, snap,
                log_stem=f"tb_{line}_p{ply}",
            )
            rec.update(parent)
            with open(p_state, "w") as f:
                json.dump(rec, f, indent=1)
        if not os.path.exists(c_state):
            child_fen = _child_fen(parent)
            snap = os.path.join(SNAPS, f"tc_{line}.tt")
            rec = solve_raw(
                child_fen, COST_TIMEOUT, snap, log_stem=f"tc_{line}"
            )
            rec.update(
                line=line, parent_ply=ply, parent_fen=parent["fen"],
                pv_move=parent["pv_move"], child_fen=child_fen,
            )
            with open(c_state, "w") as f:
                json.dump(rec, f, indent=1)
        pair = _pair_metrics(line, ply)
        pairs.append(pair)
        print(
            f"{line} p{ply}: parent nodes={pair['parent_nodes']} "
            f"solved={pair['parent_solved']} | child nodes="
            f"{pair['child_nodes']} all={pair['child_all']} | "
            f"avail={pair['avail']:.4%} carry={pair['carry']:.4%}",
            flush=True,
        )
    avails = sorted(p["avail"] for p in pairs)
    carries = sorted(p["carry"] for p in pairs)

    def median(v):
        return v[len(v) // 2] if len(v) % 2 else (v[len(v) // 2 - 1] + v[len(v) // 2]) / 2

    med_avail, med_carry = median(avails), median(carries)
    if med_avail >= GATE_AVAIL_GO:
        verdict = "GO"
    elif med_avail < GATE_AVAIL_DEAD:
        verdict = "DEAD"
    else:
        verdict = "JUDGMENT"
    out = {
        "timeout": COST_TIMEOUT,
        "n_pairs": len(pairs),
        "median_avail": med_avail,
        "median_carry": med_carry,
        "gate": {
            "GO>=": GATE_AVAIL_GO,
            "DEAD<": GATE_AVAIL_DEAD,
            "verdict": verdict,
        },
        "pairs": pairs,
    }
    with open(os.path.join(ROOT, "substrate.json"), "w") as f:
        json.dump(out, f, indent=1)
    print(
        f"Task B: median avail={med_avail:.4%} carry={med_carry:.4%} "
        f"over {len(pairs)} pairs -> {verdict}"
    )


def _pair_metrics(line, ply):
    p_state = os.path.join(STATE, f"tb_{line}_p{ply}.json")
    c_state = os.path.join(STATE, f"tc_{line}.json")
    with open(p_state) as f:
        p_rec = json.load(f)
    with open(c_state) as f:
        c_rec = json.load(f)
    _h, p_solved, _pu = ssfp.read_tt_snapshot(
        os.path.join(SNAPS, f"tb_{line}_p{ply}.tt")
    )
    _h, c_solved, c_unsolved = ssfp.read_tt_snapshot(
        os.path.join(SNAPS, f"tc_{line}.tt")
    )
    p_solved_keys = {r["key"] for r in p_solved}
    c_all = {r["key"] for r in c_solved}
    c_all.update(r["key"] for r in c_unsolved)
    c_solved_keys = {r["key"] for r in c_solved}
    inter = len(p_solved_keys & c_all)
    p_solved_in_c_solved = len(p_solved_keys & c_solved_keys)
    return {
        "line": line,
        "parent_ply": ply,
        "parent_fen": p_rec["fen"],
        "child_fen": c_rec["fen"],
        "pv_move": p_rec.get("pv_move"),
        "parent_nodes": p_rec["nodes"],
        "parent_censored": p_rec["censored"],
        "parent_outcome": p_rec["outcome"],
        "parent_solved": len(p_solved_keys),
        "parent_unsolved": p_rec.get("tt_unsolved"),
        "child_nodes": c_rec["nodes"],
        "child_censored": c_rec["censored"],
        "child_outcome": c_rec["outcome"],
        "child_solved": len(c_solved_keys),
        "child_unsolved": len(c_unsolved),
        "child_all": len(c_all),
        "intersection_p_solved_in_c_all": inter,
        "avail": inter / max(1, len(c_all)),
        "carry": inter / max(1, len(p_solved_keys)),
        "vshare_p_to_c": inter / max(1, len(p_solved_keys)),
        "vshare_c_to_p": p_solved_in_c_solved / max(1, len(c_solved_keys)),
    }


# -------------------------------- status -------------------------------------


def cmd_status(_args):
    for tag in ("a1", "a2"):
        path = os.path.join(STATE, f"arm_{tag}.json")
        if os.path.exists(path):
            with open(path) as f:
                r = json.load(f)
            print(
                f"arm {tag}: outcome={r['outcome']} nodes={r['nodes']} "
                f"wall={r['wall']} censored={r['censored']} "
                f"validate={r.get('validate')}"
            )
        else:
            print(f"arm {tag}: not run")
    if os.path.exists(os.path.join(ROOT, "substrate.json")):
        with open(os.path.join(ROOT, "substrate.json")) as f:
            s = json.load(f)
        print(
            f"task B: median avail={s['median_avail']:.4%} "
            f"carry={s['median_carry']:.4%} verdict={s['gate']['verdict']}"
        )
    else:
        tb = sorted(
            f for f in os.listdir(STATE) if f.startswith(("tb_", "tc_"))
        ) if os.path.isdir(STATE) else []
        print(f"task B: not complete ({len(tb)} per-run state files)")


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("env").set_defaults(fn=cmd_env)
    p = sub.add_parser("arma")
    p.add_argument("--arm", choices=("a1", "a2", "both"), default="both")
    p.set_defaults(fn=cmd_arma)
    sub.add_parser("armb").set_defaults(fn=cmd_armb)
    sub.add_parser("status").set_defaults(fn=cmd_status)
    args = ap.parse_args()
    ensure_dirs()
    args.fn(args)


if __name__ == "__main__":
    main()
