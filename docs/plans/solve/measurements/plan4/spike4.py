#!/usr/bin/env python3
"""Plan 4 harness (`solve` initiative): sharpness-first pilot. Full-width
ply-1/2 sharp-class density screen (Task A), sibling-thickness probe
(Task B), and the artifact build + validation census (Task C), driving
the unmodified release binaries as a black box (Python 3 stdlib only).

Subcommands:
  env       -- record environment metadata to env.json
  screen    -- Task A: all 20 first moves + all legal replies (8 s screen
               solves, plan1's steer budget, snapshots dumped)
  promote   -- Task A: seeded random sample of 5 censored ply-2 positions
               re-solved at 120 s (plateau-transfer check)
  siblings  -- Task B: all legal moves at the two pre-registered tactical
               roots (e4e5 p2, d4d5 p32), 8 s screen each
  artifact  -- Task C: reconstruct + validate every decided position from
               Tasks A/B plus the 26 plan1 ladder tactical positions
               (re-solved at 120 s, deduped by FEN); writes
               artifact/index.json + per-position binary trees
  status    -- print progress summary

Gates are pre-registered in ../../plan4.md (do not tune after seeing
data): VALIDATE hard (100% of reconstructions `validate: ok` with the
search's outcome); DENSITY on the ply-2 screen (>= 10% GO for a scaled
plan, < 2% thin).

Layout follows the plan1-3 conventions: raw stdout/stderr captures
under logs/ (`<stem>.out` / `<stem>.err`), state JSONs under state/,
snapshots under snaps/ (kept until the report, then pruned to empty
`.keep` markers). The snapshot reader is reused from ../plan2/ssfp.py
verbatim; the max-RSS method is plan3's (os.wait4 ru_maxrss; no
/usr/bin/time in this container).
"""

from __future__ import annotations

import argparse
import glob
import json
import os
import random
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
ARTIFACT = os.path.join(ROOT, "artifact")

STARTPOS = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
SCREEN_TIMEOUT = 8  # plan1's steer budget; results comparable to ladders
COST_TIMEOUT = 120  # plan1/2/3 cost budget
PROMOTE_SEED = 20260923  # pre-registered in plan4.md §2
PROMOTE_N = 5

# Pre-registered in plan4.md §3 (do not tune after seeing data).
SIBLING_ROOTS = (
    ("e4e5_p2", "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2"),
    ("d4d5_p32", "rnbqkbnr/4pppp/8/P2P4/4PPPP/2p5/8/2BQKBNR w Kkq - 0 17"),
)


def ensure_dirs():
    for d in (LOGS, STATE, SNAPS, ARTIFACT):
        os.makedirs(d, exist_ok=True)


def run_raw(cmd, timeout, out_path, err_path):
    """Child with stdin=DEVNULL, raw stdout/stderr to files, max-RSS via
    os.wait4. Returns (wall, rc, rss_kb)."""
    t0 = time.monotonic()
    with open(out_path, "w") as fo, open(err_path, "w") as fe:
        p = subprocess.Popen(
            cmd, stdin=subprocess.DEVNULL, stdout=fo, stderr=fe
        )
        killer = threading.Timer(timeout + 300, p.kill)
        killer.start()
        try:
            _pid, status, ru = os.wait4(p.pid, 0)
        finally:
            killer.cancel()
        p.returncode = os.waitstatus_to_exitcode(status)
    return round(time.monotonic() - t0, 3), p.returncode, ru.ru_maxrss


def parse_stdout(text):
    """Same field semantics as plan2 ssfp.solve / plan3 probe.parse_stdout."""
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


def solve_raw(fen, timeout, snap, extra=None, log_stem=None):
    cmd = [ssfp.SOLVER, "--fen", fen, "--timeout", str(timeout),
           "--first-outcome"]
    if snap:
        cmd += ["--tt-dump-path", snap]
    if extra:
        cmd += extra
    out_path = os.path.join(LOGS, f"{log_stem}.out")
    err_path = os.path.join(LOGS, f"{log_stem}.err")
    wall, rc, rss_kb = run_raw(cmd, timeout, out_path, err_path)
    with open(out_path) as f:
        rec = parse_stdout(f.read())
    rec.update(fen=fen, wall=wall, rc=rc, rss_kb=rss_kb, cmd=" ".join(cmd))
    return rec


def legal_moves(fen):
    r = subprocess.run(
        [ssfp.LIST_LEGAL, "--fen", fen],
        capture_output=True, text=True, stdin=subprocess.DEVNULL, timeout=60,
    )
    if r.returncode != 0:
        raise RuntimeError(f"list_legal failed on {fen}: {r.stderr[:300]}")
    moves = []
    for line in r.stdout.splitlines():
        # list_legal prints "  <uci>" per move; headers are unindented.
        if line.startswith("  ") and line.strip():
            moves.append(line.strip())
    return moves


def child_fen(fen, uci):
    r = subprocess.run(
        [ssfp.REPLAY, fen, uci],
        capture_output=True, text=True, stdin=subprocess.DEVNULL, timeout=60,
    )
    if r.returncode != 0:
        raise RuntimeError(f"replay failed {fen} {uci}: {r.stderr[:300]}")
    for line in r.stdout.splitlines():
        if line.startswith("fen: "):
            return line[5:].strip()
    raise RuntimeError(f"replay printed no fen: {r.stdout[:300]}")


def _tag(moves):
    return "_".join(moves) if moves else "root"


def _screen_one(tier, moves, fen, timeout, plan1_nodes=None):
    stem = f"{tier}_{_tag(moves)}"
    state_path = os.path.join(STATE, f"{stem}.json")
    if os.path.exists(state_path):
        with open(state_path) as f:
            return json.load(f)
    snap = os.path.join(SNAPS, f"{stem}.tt")
    rec = solve_raw(fen, timeout, snap, log_stem=stem)
    rec.update(tier=tier, moves=moves, tag=stem, plan1_nodes=plan1_nodes)
    with open(state_path, "w") as f:
        json.dump(rec, f, indent=1)
    print(
        f"{stem}: {rec['outcome']} nodes={rec['nodes']} "
        f"censored={rec['censored']} wall={rec['wall']}",
        flush=True,
    )
    return rec


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


def cmd_screen(_args):
    ensure_dirs()
    t0 = time.monotonic()
    firsts = legal_moves(STARTPOS)
    print(f"ply 1: {len(firsts)} first moves", flush=True)
    n_done = 0
    for mv in firsts:
        fen1 = child_fen(STARTPOS, mv)
        _screen_one("p1", [mv], fen1, SCREEN_TIMEOUT)
        n_done += 1
        replies = legal_moves(fen1)
        for r in replies:
            fen2 = child_fen(fen1, r)
            _screen_one("p2", [mv, r], fen2, SCREEN_TIMEOUT)
            n_done += 1
        print(
            f"[ply2 after {mv}: {len(replies)} replies; "
            f"{n_done} runs, {time.monotonic() - t0:.0f}s]",
            flush=True,
        )


def cmd_promote(_args):
    """Seeded sample of 5 censored ply-2 positions -> 120 s (plateau
    transfer check)."""
    ensure_dirs()
    censored = []
    for path in sorted(glob.glob(os.path.join(STATE, "p2_*.json"))):
        with open(path) as f:
            rec = json.load(f)
        if rec["censored"] and rec["rc"] == 0:
            censored.append(rec)
    rng = random.Random(PROMOTE_SEED)
    sample = rng.sample(censored, min(PROMOTE_N, len(censored)))
    for rec in sample:
        stem = f"prom_{_tag(rec['moves'])}"
        state_path = os.path.join(STATE, f"{stem}.json")
        if os.path.exists(state_path):
            continue
        snap = os.path.join(SNAPS, f"{stem}.tt")
        out = solve_raw(rec["fen"], COST_TIMEOUT, snap, log_stem=stem)
        out.update(tier="promote", moves=rec["moves"],
                   screen_nodes=rec["nodes"])
        with open(state_path, "w") as f:
            json.dump(out, f, indent=1)
        print(
            f"{stem}: 120s -> {out['outcome']} nodes={out['nodes']} "
            f"censored={out['censored']} (8s screen nodes={rec['nodes']})",
            flush=True,
        )


# -------------------------------- Task B -------------------------------------


def cmd_siblings(_args):
    ensure_dirs()
    for name, fen in SIBLING_ROOTS:
        moves = legal_moves(fen)
        print(f"siblings of {name}: {len(moves)} moves", flush=True)
        for i, mv in enumerate(moves):
            child = child_fen(fen, mv)
            _screen_one(f"sib_{name}", [mv], child, SCREEN_TIMEOUT)
            if (i + 1) % 10 == 0:
                print(f"  {i + 1}/{len(moves)}", flush=True)


# -------------------------------- Task C -------------------------------------


def _ladder_tactics():
    """The 26 plan1 ladder tactical positions (steer == 'pv'), deduped."""
    seen = {}
    for path in sorted(glob.glob(os.path.join(PLAN1_STATE, "ladder_*.json"))):
        line = os.path.basename(path)[len("ladder_"):-5]
        with open(path) as f:
            d = json.load(f)
        for p in d["positions"]:
            if p.get("steer") == "pv" and p["fen"] not in seen:
                seen[p["fen"]] = {
                    "line": line,
                    "ply": p["ply"],
                    "fen": p["fen"],
                    "pv_move": p["step"]["pv"][0],
                    "plan1_nodes": p["step"]["nodes"],
                    "plan1_outcome": p["step"]["outcome"],
                    "moves": None,
                }
    return list(seen.values())


def _decided_from_state(prefixes):
    out = []
    for pref in prefixes:
        for path in sorted(glob.glob(os.path.join(STATE, f"{pref}*.json"))):
            if os.path.basename(path).startswith("prom_"):
                continue
            with open(path) as f:
                rec = json.load(f)
            if not rec["censored"] and rec["rc"] == 0:
                out.append(rec)
    return out


def reconstruct(snap, out_pt, log_stem):
    cmd = [ssfp.RECONSTRUCT, "--snapshot", snap, "--out", out_pt]
    out_path = os.path.join(LOGS, f"{log_stem}.out")
    err_path = os.path.join(LOGS, f"{log_stem}.err")
    # 900 s: verify wall is not a small constant on top of find wall
    # (plan1 measured 1.18x at 23 M nodes); the two ~1 M-node screen
    # wins must not be verification-capped at the 120 s screen budget.
    wall, rc, rss_kb = run_raw(cmd, 900, out_path, err_path)
    with open(out_path) as f:
        text = f.read()
    validate = outcome = None
    for line in text.splitlines():
        if line.startswith("validate: "):
            validate = line[10:].strip()
        elif line.startswith("outcome: "):
            outcome = line[9:].strip()
    return rc, validate, outcome, wall, rss_kb


def _artifact_entry(tag, rec):
    """One artifact member: reconstruct from rec's snapshot + validate."""
    entry = {
        "tag": tag,
        "fen": rec["fen"],
        "moves": rec.get("moves"),
        "tier": rec.get("tier"),
        "outcome": rec["outcome"],
        "dtm_length": (int(rec["outcome"].split("length: ")[1])
                       if rec["outcome"] and "length: " in rec["outcome"]
                       else None),
        "nodes": rec["nodes"],
        "wall": rec["wall"],
        "plan1_nodes": rec.get("plan1_nodes"),
        "pv": rec.get("pv", [])[:1],
    }
    snap = os.path.join(SNAPS, f"{tag}.tt")
    pt = os.path.join(ARTIFACT, f"{tag}.bin")
    rc, validate, outcome, wall, rss_kb = reconstruct(
        snap, pt, f"recon_{tag}"
    )
    entry.update(
        reconstruct_rc=rc,
        validate=validate,
        reconstruct_outcome=outcome,
        reconstruct_wall=wall,
        outcome_match=(outcome == rec["outcome"]),
    )
    if validate == "ok":
        keys = ssfp.read_proof_tree_keys(pt)
        entry["tree_nodes"] = len(keys)
        entry["tree_unique_keys"] = len({k["key"] for k in keys})
    return entry


def cmd_artifact(_args):
    ensure_dirs()
    members = []
    # Tasks A/B decided positions (their `tag` is the screen stem and
    # names both the snapshot and the log files).
    for rec in _decided_from_state(("p1_", "p2_", "sib_")):
        members.append((rec["tag"], rec))
    # plan1 ladder tactics, deduped by FEN against the screen set.
    have = {m[1]["fen"] for m in members}
    for i, lt in enumerate(_ladder_tactics()):
        if lt["fen"] in have:
            continue
        tag = f"lad_{lt['line']}_p{lt['ply']}"
        state_path = os.path.join(STATE, f"{tag}.json")
        if not os.path.exists(state_path):
            snap = os.path.join(SNAPS, f"{tag}.tt")
            rec = solve_raw(lt["fen"], COST_TIMEOUT, snap, log_stem=tag)
            rec.update(tier="ladder", moves=None, **{
                k: lt[k] for k in ("line", "ply", "plan1_nodes",
                                   "plan1_outcome")
            })
            with open(state_path, "w") as f:
                json.dump(rec, f, indent=1)
        else:
            with open(state_path) as f:
                rec = json.load(f)
        members.append((tag, rec))

    entries = []
    t0 = time.monotonic()
    for tag, rec in members:
        entries.append(_artifact_entry(tag, rec))
        e = entries[-1]
        print(
            f"{tag}: {e['outcome']} validate={e['validate']} "
            f"match={e['outcome_match']} tree={e.get('tree_nodes')} nodes "
            f"({time.monotonic() - t0:.0f}s cum)",
            flush=True,
        )

    validate_ok = sum(1 for e in entries if e["validate"] == "ok")
    matches = sum(1 for e in entries if e["outcome_match"])
    conflicts = sum(
        1 for e in entries
        if e.get("tree_nodes") and e["tree_nodes"] != e.get("tree_unique_keys")
    )
    summary = {
        "n_members": len(entries),
        "validate_ok": validate_ok,
        "outcome_match": matches,
        "total_tree_nodes": sum(e.get("tree_nodes", 0) for e in entries),
        "total_unique_keys": sum(e.get("tree_unique_keys", 0) for e in entries),
        "members_with_duplicate_keys": conflicts,
        "total_find_wall": round(sum(e["wall"] for e in entries), 3),
        "total_verify_wall": round(
            sum(e["reconstruct_wall"] for e in entries), 3
        ),
        "artifact_bytes": sum(
            os.path.getsize(os.path.join(ARTIFACT, f"{e['tag']}.bin"))
            for e in entries if e["validate"] == "ok"
        ),
        "entries": entries,
    }
    with open(os.path.join(ARTIFACT, "index.json"), "w") as f:
        json.dump(summary, f, indent=1)
    print(
        f"artifact: {summary['n_members']} members, validate_ok="
        f"{validate_ok}/{len(entries)}, outcome_match={matches}, "
        f"tree nodes={summary['total_tree_nodes']}, "
        f"find+verify wall={summary['total_find_wall']}+"
        f"{summary['total_verify_wall']}s"
    )


# -------------------------------- status -------------------------------------


def cmd_status(_args):
    if not os.path.isdir(STATE):
        print("nothing run yet")
        return
    p1 = glob.glob(os.path.join(STATE, "p1_*.json"))
    p2 = glob.glob(os.path.join(STATE, "p2_*.json"))
    prom = glob.glob(os.path.join(STATE, "prom_*.json"))
    sib = glob.glob(os.path.join(STATE, "sib_*.json"))
    lad = glob.glob(os.path.join(STATE, "lad_*.json"))
    print(
        f"screen: p1 {len(p1)}/20, p2 {len(p2)}/? (censored "
        f"{sum(1 for p in p2 if json.load(open(p))['censored'])}), "
        f"promoted {len(prom)}/5, siblings {len(sib)}, ladder {len(lad)}"
    )
    idx = os.path.join(ARTIFACT, "index.json")
    if os.path.exists(idx):
        with open(idx) as f:
            a = json.load(f)
        print(
            f"artifact: {a['n_members']} members, validate_ok="
            f"{a['validate_ok']}, find+verify={a['total_find_wall']}+"
            f"{a['total_verify_wall']}s"
        )


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("env").set_defaults(fn=cmd_env)
    sub.add_parser("screen").set_defaults(fn=cmd_screen)
    sub.add_parser("promote").set_defaults(fn=cmd_promote)
    sub.add_parser("siblings").set_defaults(fn=cmd_siblings)
    sub.add_parser("artifact").set_defaults(fn=cmd_artifact)
    sub.add_parser("status").set_defaults(fn=cmd_status)
    args = ap.parse_args()
    ensure_dirs()
    args.fn(args)


if __name__ == "__main__":
    main()
