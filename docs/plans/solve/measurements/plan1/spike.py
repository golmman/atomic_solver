#!/usr/bin/env python3
"""Plan 1 spike harness (`solve` initiative): reach-vs-depth ladder +
random-playout control + cost recording, driving the unmodified release
binaries as a black box (Python 3 stdlib only).

Subcommands:
  env                -- record environment metadata to env.json
  random             -- uniform-random atomic playouts from startpos
  ladder             -- run/extend the 8 self-play ladders (resumable)
  cost               -- depth-ladder cost runs at every ladder position
                        (resumable; --tt-size for the 1 GB addendum)
  verify             -- reconstruct_pt --validate on recorded snapshots
  status             -- print progress summary

Layout and conventions follow parallel/measurements/plan2/README.md:
raw stdout/stderr captures under logs/, state JSONs beside this script.
`nodes` is the documented work proxy for the CLI-invisible `child_evals`.
"""

from __future__ import annotations

import argparse
import glob
import json
import os
import random
import re
import subprocess
import time

ROOT = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(ROOT, "..", "..", "..", "..", ".."))
SOLVER = os.path.join(REPO, "target", "release", "atomic_solver")
EXAMPLES = os.path.join(REPO, "target", "release", "examples")
REPLAY = os.path.join(EXAMPLES, "replay")
LIST_LEGAL = os.path.join(EXAMPLES, "list_legal")
RECONSTRUCT = os.path.join(EXAMPLES, "reconstruct_pt")

LOGS = os.path.join(ROOT, "logs")
STATE = os.path.join(ROOT, "state")
SNAPS = os.path.join(ROOT, "snaps")

STARTPOS = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"

# Pre-registered in plan1.md: 8 seeds, startpos-reachable, ply 0..2.
SEEDS = [
    ("startpos", []),
    ("e4", ["e2e4"]),
    ("d4", ["d2d4"]),
    ("nf3", ["g1f3"]),
    ("e4e5", ["e2e4", "e7e5"]),
    ("e4c5", ["e2e4", "c7c5"]),
    ("d4d5", ["d2d4", "d7d5"]),
    ("nf3d5", ["g1f3", "d7d5"]),
]

LADDER_MAX_PLY = 44
STEP_TIMEOUT = 8
COST_TIMEOUT = 120


def run(cmd, timeout=None):
    return subprocess.run(
        cmd, capture_output=True, text=True, stdin=subprocess.DEVNULL, timeout=timeout
    )


def ensure_dirs():
    for d in (LOGS, STATE, SNAPS):
        os.makedirs(d, exist_ok=True)


def replay_fen(fen, moves):
    """Return the FEN after playing `moves` (replay validates legality)."""
    r = run([REPLAY, fen, *moves])
    if r.returncode != 0:
        raise RuntimeError(f"replay failed for {fen} + {moves}: {r.stderr[:300]}")
    for line in r.stdout.splitlines():
        if line.startswith("fen: "):
            return line[5:].strip()
    raise RuntimeError(f"no fen in replay output: {r.stdout[:300]}")


def list_legal(fen):
    """Return (ordered legal uci moves, outcome-string-or-None)."""
    r = run([LIST_LEGAL, "--fen", fen])
    if r.returncode != 0:
        raise RuntimeError(f"list_legal failed for {fen}: {r.stderr[:300]}")
    moves, outcome = [], None
    for line in r.stdout.splitlines():
        if line.startswith("  ") and len(line.split()) == 1:
            moves.append(line.strip())
        elif line.startswith("outcome: "):
            tok = line[9:].strip()
            outcome = None if tok == "None" else tok
    return moves, outcome


def fen_men(fen):
    return sum(1 for c in fen.split()[0] if c.isalpha())


def fen_stm(fen):
    return fen.split()[1]


def solve(fen, timeout, tt_dump=None, extra=None, log_path=None, tt_size=None):
    """Run the solver (non-outcome-only pass, --first-outcome); parse stdout."""
    cmd = [SOLVER, "--fen", fen, "--timeout", str(timeout), "--first-outcome"]
    if tt_size:
        cmd += ["--tt-size", str(tt_size)]
    if tt_dump:
        cmd += ["--tt-dump-path", tt_dump]
    if extra:
        cmd += extra
    t0 = time.monotonic()
    r = run(cmd, timeout=timeout + 60)
    wall = time.monotonic() - t0
    if log_path:
        with open(log_path, "w") as f:
            f.write(f"# cmd: {' '.join(cmd)}\n# wall: {wall:.3f}\n")
            f.write("## stdout\n" + r.stdout + "\n## stderr\n" + r.stderr)
    parsed = {
        "fen": fen,
        "wall": round(wall, 3),
        "rc": r.returncode,
        "outcome": None,
        "pv": [],
        "pv_status": None,
        "exit_reason": None,
        "exit_outcome": None,
        "nodes": None,
        "tt_bytes": None,
        "timeout_flag": False,
    }
    for line in r.stdout.splitlines():
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
    parsed["censored"] = parsed["exit_reason"] == "Timeout" or parsed["timeout_flag"]
    return parsed


# ------------------------------- env ---------------------------------------


def cmd_env(_args):
    meta = {
        "git_rev": subprocess.run(
            ["git", "rev-parse", "HEAD"], capture_output=True, text=True, cwd=REPO
        ).stdout.strip(),
        "git_dirty": subprocess.run(
            ["git", "status", "--porcelain"], capture_output=True, text=True, cwd=REPO
        ).stdout.strip(),
        "nproc": os.cpu_count(),
        "cpu_max": _read("/sys/fs/cgroup/cpu.max"),
        "memory_max_bytes": _int(_read("/sys/fs/cgroup/memory.max")),
        "date": time.strftime("%Y-%m-%d %H:%M:%S %z"),
    }
    with open(os.path.join(ROOT, "env.json"), "w") as f:
        json.dump(meta, f, indent=2)
    print(json.dumps(meta, indent=2))


def _read(path):
    try:
        with open(path) as f:
            return f.read().strip()
    except OSError:
        return None


def _int(s):
    try:
        return int(s)
    except (TypeError, ValueError):
        return None


# ------------------------------ random -------------------------------------


def cmd_random(args):
    random.seed(args.seed)
    games = []
    for gi in range(args.games):
        fen = STARTPOS
        plies = 0
        final_men = 32
        terminal = None
        men_path = []
        while plies < 1000:
            moves, outcome = list_legal(fen)
            if outcome is not None:
                terminal = outcome
                final_men = fen_men(fen)
                men_path.append(final_men)
                break
            mv = random.choice(moves)
            fen = replay_fen(fen, [mv])
            plies += 1
            men_path.append(fen_men(fen))
        else:
            terminal = "cap"
            final_men = fen_men(fen)
        games.append({"game": gi, "plies": plies, "final_men": final_men, "terminal": terminal, "men_path": men_path})
        if (gi + 1) % 25 == 0:
            print(f"random: {gi + 1}/{args.games}", flush=True)
    with open(os.path.join(ROOT, "random_playouts.json"), "w") as f:
        json.dump({"seed": args.seed, "n": args.games, "games": games}, f, indent=1)
    print(f"wrote random_playouts.json ({args.games} games)")


# ------------------------------ ladder -------------------------------------


def ladder_path(name):
    return os.path.join(STATE, f"ladder_{name}.json")


def ladder_entry(fen, ply):
    return {
        "ply": ply,
        "fen": fen,
        "men": fen_men(fen),
        "stm": fen_stm(fen),
        "step": None,  # bounded solve result (no tt dump)
        "moves_played": [],
        "stop_reason": None,
    }


def advance_two_plies(fen, pv, ll_cache):
    """Play up to 2 plies: PV move first, list_legal-order fallback, stop at
    terminal. Returns (moves_played, final_fen)."""
    played = []
    for k in range(2):
        moves, outcome = ll_cache_get(fen, ll_cache)
        if outcome is not None:
            break
        mv = pv[k] if k < len(pv) and pv[k] in moves else moves[0]
        fen = replay_fen(fen, [mv])
        played.append(mv)
    return played, fen


def ll_cache_get(fen, cache):
    if fen not in cache:
        cache[fen] = list_legal(fen)
    return cache[fen]


def cmd_ladder(_args):
    ll_cache = {}
    for name, seed_moves in SEEDS:
        path = ladder_path(name)
        state = {"seed": seed_moves, "positions": []}
        if os.path.exists(path):
            with open(path) as f:
                state = json.load(f)
        pos = state["positions"]
        if pos and pos[-1]["stop_reason"]:
            print(f"ladder {name}: complete ({pos[-1]['stop_reason']})")
            continue
        if pos:
            fen = pos[-1]["fen"]
            ply = pos[-1]["ply"]
        else:
            fen = replay_fen(STARTPOS, seed_moves)
            ply = len(seed_moves)
        stop_reason = None
        consec_caps = 0
        while True:
            if not pos or pos[-1]["fen"] != fen or pos[-1]["ply"] != ply:
                pos.append(ladder_entry(fen, ply))
            entry = pos[-1]
            if ply >= LADDER_MAX_PLY:
                stop_reason = "ply44"
                break
            s = solve(fen, STEP_TIMEOUT, log_path=os.path.join(LOGS, f"step_{name}_p{ply}.log"))
            entry["step"] = s
            entry["steer"] = "capped" if s["censored"] else "pv"
            # Protocol note (pre-registered in README.md): a capped steer solve
            # does NOT stop the line -- the plan's ply-2..44 cost ladder needs
            # positions at every depth, so the fallback advance applies and the
            # line runs to ply 44 or a terminal position. Per-line reach = the
            # deepest PV-steered ply.
            played, fen2 = advance_two_plies(fen, s["pv"], ll_cache)
            entry["moves_played"] = played
            if len(played) < 2:
                stop_reason = "terminal"
                break
            ply += 2
            fen = fen2
            pos.append(ladder_entry(fen, ply))  # placeholder, filled next loop
        pos[-1]["stop_reason"] = stop_reason
        with open(path, "w") as f:
            json.dump(state, f, indent=1)
        print(f"ladder {name}: {len(pos)} positions, stop={stop_reason}")


def ladder_positions():
    for name, seed_moves in SEEDS:
        with open(ladder_path(name)) as f:
            state = json.load(f)
        for e in state["positions"]:
            yield name, e


# ------------------------------- cost --------------------------------------


def cost_key(line, ply, tt_size):
    tag = f"{tt_size}mb" if tt_size else "128mb"
    return f"{tag}:{line}:{ply}"


def cmd_cost(args):
    ensure_dirs()
    done_path = os.path.join(STATE, "cost_done.json")
    done = set()
    if os.path.exists(done_path):
        with open(done_path) as f:
            done = set(json.load(f))
    jobs = []
    for line, e in ladder_positions():
        # Seed positions with ply < 2 are included but flagged informational
        # (the pre-registered fit uses ply >= 2 only).
        informational = e["ply"] < 2
        key = cost_key(line, e["ply"], args.tt_size or 128)
        if key in done:
            continue
        jobs.append((line, e, informational, key))
    print(f"cost: {len(jobs)} runs to do (tt_size={args.tt_size or 128})")
    for i, (line, e, informational, key) in enumerate(jobs):
        ply = e["ply"]
        tt_dump = os.path.join(SNAPS, f"{line}_p{ply}.tt") if args.tt_size in (None, 128) else None
        log = os.path.join(LOGS, f"cost_{line}_p{ply}{'_' + str(args.tt_size) + 'mb' if args.tt_size else ''}.log")
        r = solve(
            e["fen"],
            COST_TIMEOUT,
            tt_dump=tt_dump,
            tt_size=args.tt_size,
            log_path=log,
        )
        r["line"] = line
        r["ply"] = ply
        r["informational_seed_position"] = informational
        r["tt_size_mb"] = args.tt_size or 128
        out = os.path.join(STATE, f"cost_{line}_p{ply}{'_' + str(args.tt_size) + 'mb' if args.tt_size else ''}.json")
        with open(out, "w") as f:
            json.dump(r, f, indent=1)
        if tt_dump and os.path.exists(tt_dump):
            if r["censored"]:
                os.remove(tt_dump)
            else:
                with open(tt_dump + ".keep", "w") as f:
                    f.write("")
        done.add(key)
        with open(done_path, "w") as f:
            json.dump(sorted(done), f)
        print(
            f"[{i + 1}/{len(jobs)}] {line} p{ply}: outcome={r['outcome']} "
            f"nodes={r['nodes']} wall={r['wall']}s censored={r['censored']} "
            f"tt={r['tt_bytes']}",
            flush=True,
        )


# ------------------------------ verify -------------------------------------


def cmd_verify(_args):
    import glob

    cands = []
    for p in sorted(glob.glob(os.path.join(STATE, "cost_*_p*.json"))):
        if re.search(r"_\d+mb_p\d+", os.path.basename(p)):
            continue  # addendum runs excluded
        with open(p) as f:
            r = json.load(f)
        if r.get("censored") or r.get("rc") != 0:
            continue
        if r.get("outcome") not in ("win", "loss"):
            continue
        snap = os.path.join(SNAPS, f"{r['line']}_p{r['ply']}.tt")
        if not os.path.exists(snap):
            continue
        cands.append((r["ply"], r["line"], r["fen"], snap))
    cands.sort()
    # The plan asks for the 2 deepest uncensored positions; we take up to 5
    # (all d4d5 >= p30) so the trivial 1-9-node proofs sit next to the real
    # 23.2M-node search when reading the verify/find ratio.
    deepest = cands[-5:]
    results = []
    for ply, line, fen, snap in deepest:
        out_pt = os.path.join(ROOT, f"reconstructed_{line}_p{ply}.bin")
        t0 = time.monotonic()
        r = run([RECONSTRUCT, "--snapshot", snap, "--out", out_pt], timeout=3600)
        wall = time.monotonic() - t0
        rec = {"line": line, "ply": ply, "fen": fen, "snapshot": snap, "wall": round(wall, 3), "rc": r.returncode}
        for l in r.stdout.splitlines():
            if l.startswith("outcome: "):
                rec["recon_outcome"] = l[9:].strip()
            elif l.startswith("nodes: "):
                rec["recon_nodes"] = int(l[7:])
            elif l.startswith("validate: "):
                rec["validate"] = l[10:].strip()
        with open(os.path.join(LOGS, f"verify_{line}_p{ply}.log"), "w") as f:
            f.write(r.stdout + "\n## stderr\n" + r.stderr)
        results.append(rec)
        print(rec, flush=True)
    with open(os.path.join(STATE, "verify.json"), "w") as f:
        json.dump(results, f, indent=1)


# ------------------------------ status -------------------------------------


def cmd_status(_args):
    for name, _ in SEEDS:
        p = ladder_path(name)
        if os.path.exists(p):
            with open(p) as f:
                s = json.load(f)
            last = s["positions"][-1]
            print(f"{name}: {len(s['positions'])} pos, deepest ply {last['ply']}, stop={last['stop_reason']}")
        else:
            print(f"{name}: not started")
    print(f"cost logs: {len(glob.glob(os.path.join(LOGS, 'cost_*.log')))}")


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("env").set_defaults(fn=cmd_env)
    p = sub.add_parser("random")
    p.add_argument("--games", type=int, default=200)
    p.add_argument("--seed", type=int, default=20260922)
    p.set_defaults(fn=cmd_random)
    sub.add_parser("ladder").set_defaults(fn=cmd_ladder)
    p = sub.add_parser("cost")
    p.add_argument("--tt-size", type=int, default=None)
    p.set_defaults(fn=cmd_cost)
    sub.add_parser("verify").set_defaults(fn=cmd_verify)
    sub.add_parser("status").set_defaults(fn=cmd_status)
    args = ap.parse_args()
    ensure_dirs()
    args.fn(args)


if __name__ == "__main__":
    main()
