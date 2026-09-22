#!/usr/bin/env python3
"""Plan 2 SSFP harness (`solve` initiative): snapshot regeneration, the M1
substrate metric, S-store merge, and the pilot loop, driving the unmodified
release binaries as a black box (Python 3 stdlib only).

Subcommands:
  env        -- record environment metadata to env.json
  regen      -- regenerate the TT snapshots of the 48 censored plan1 cost
                runs (sequential, commands replayed from
                ../plan1/state/cost_*.json; snapshots kept this time)
  m1         -- compute the M1 cross-system value-share metric + the
                secondary proof-tree key share; apply the pre-registered
                gate (GO >= 5%, NO-GO < 1%)
  status     -- print progress summary

Layout and conventions follow ../plan1/spike.py and its README: raw
stdout/stderr captures under logs/, state JSONs under state/, snapshots
under snaps/. Censored runs are wall-clock bounded, so the regenerated
snapshots are content-equivalent, not byte-identical, to the (deleted)
plan1 originals; the commands are replayed unchanged.
"""

from __future__ import annotations

import argparse
import glob
import json
import os
import re
import struct
import subprocess
import time

ROOT = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(ROOT, "..", "..", "..", "..", ".."))
SOLVER = os.path.join(REPO, "target", "release", "atomic_solver")
PT_KEYS = os.path.join(REPO, "target", "release", "examples", "pt_keys")
RECONSTRUCT = os.path.join(REPO, "target", "release", "examples", "reconstruct_pt")
LIST_LEGAL = os.path.join(REPO, "target", "release", "examples", "list_legal")
REPLAY = os.path.join(REPO, "target", "release", "examples", "replay")

LOGS = os.path.join(ROOT, "logs")
STATE = os.path.join(ROOT, "state")
SNAPS = os.path.join(ROOT, "snaps")
STORE = os.path.join(ROOT, "store")

PLAN1 = os.path.join(ROOT, "..", "plan1")
PLAN1_STATE = os.path.join(PLAN1, "state")

COST_TIMEOUT = 120  # must match plan1's cost runs

# Pre-registered in plan2.md §4 (do not tune after seeing data).
M1_GO = 0.05
M1_NOGO = 0.01


def run(cmd, timeout=None):
    return subprocess.run(
        cmd, capture_output=True, text=True, stdin=subprocess.DEVNULL, timeout=timeout
    )


def solve(fen, timeout, tt_dump=None, extra=None, log_path=None, tt_size=None):
    """Run the solver (non-outcome-only pass, --first-outcome); parse stdout.
    Same parsing as plan1's spike.py, so state files stay comparable."""
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
        elif line.startswith("tt_load: "):
            parsed["tt_load"] = line[9:].strip()
        elif line.startswith("frontier: "):
            parsed["frontier"] = line[10:].strip()
    parsed["censored"] = parsed["exit_reason"] == "Timeout" or parsed["timeout_flag"]
    return parsed


def ensure_dirs():
    for d in (LOGS, STATE, SNAPS, STORE):
        os.makedirs(d, exist_ok=True)


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


# --------------------------- binary readers ---------------------------------
#
# Readers for the solver's compact binary formats (see src/tt_snapshot/mod.rs
# and src/proof_tree/binary.rs). Strict: bad magic/version/flags/truncation
# raise ValueError.


def read_tt_snapshot(path):
    """Parse a TT snapshot v1 file -> (header dict, solved list, unsolved list).

    solved record: dict(key, outcome, depth, best_move)
    unsolved record: dict(key, pn, dn, depth, remaining_depth, best_move, work)
    """
    with open(path, "rb") as f:
        data = f.read()
    if data[:8] != b"ATOMTTSN":
        raise ValueError(f"{path}: bad magic")
    version, flags = data[8], data[9]
    if version != 1:
        raise ValueError(f"{path}: unsupported version {version}")
    if flags != 0:
        raise ValueError(f"{path}: unsupported flags {flags:#x}")
    nl = data.index(b"\n", 10)
    root_fen = data[10:nl].decode("utf-8")
    off = nl + 1
    tt_size_mb, generation, solved_count, unsolved_count = struct.unpack_from(
        "<IIQQ", data, off
    )
    off += 24

    solved = []
    for _ in range(solved_count):
        key, = struct.unpack_from("<Q", data, off)
        ob = data[off + 8]
        outcome = {0: "draw", 1: "win", 2: "loss"}.get(ob)
        if outcome is None:
            raise ValueError(f"{path}: invalid solved outcome byte {ob}")
        (depth,) = struct.unpack_from("<I", data, off + 9)
        (best_move,) = struct.unpack_from("<H", data, off + 13)
        solved.append(
            {"key": key, "outcome": outcome, "depth": depth, "best_move": best_move}
        )
        off += 15

    unsolved = []
    for _ in range(unsolved_count):
        key, pn, dn, depth, remaining_depth, best_move, work = struct.unpack_from(
            "<QQQIIHQ", data, off
        )
        unsolved.append(
            {
                "key": key,
                "pn": pn,
                "dn": dn,
                "depth": depth,
                "remaining_depth": remaining_depth,
                "best_move": best_move,
                "work": work,
            }
        )
        off += 42

    if off != len(data):
        raise ValueError(f"{path}: trailing bytes after last record")
    header = {
        "root_fen": root_fen,
        "tt_size_mb": tt_size_mb,
        "generation": generation,
        "solved_count": solved_count,
        "unsolved_count": unsolved_count,
    }
    return header, solved, unsolved


def read_proof_tree_keys(path):
    """Node records of a proof-tree dump, via the pt_keys example (replay).

    Returns a list of dicts (id, parent_id, key, outcome, depth, mv_bits) in
    DFS pre-order. The binary dump format does not store hashes, so keys are
    recomputed by replaying the tree from its root FEN (examples/pt_keys.rs,
    mirroring the offline validator's replay mechanics).
    """
    r = run([PT_KEYS, path], timeout=600)
    if r.returncode != 0:
        raise RuntimeError(f"pt_keys failed on {path}: {r.stderr[:300]}")
    out = []
    for line in r.stdout.splitlines():
        id_s, parent_s, key_s, outcome, depth_s, mv_s = line.split()
        out.append(
            {
                "id": int(id_s),
                "parent_id": int(parent_s),
                "key": int(key_s),
                "outcome": outcome,
                "depth": int(depth_s),
                "mv_bits": int(mv_s),
            }
        )
    return out


def verified_records_from_tree(tree_path):
    """SolvedStore verified records from a (validated) proof-tree dump.

    Every tree node becomes a record; a Win node's best_move is the min-depth
    child's move (bottom-up Win = min+1 semantics), a Loss node's best_move
    is the NONE sentinel (all replies lose). Nodes carry replay-derived keys.
    """
    nodes = read_proof_tree_keys(tree_path)
    children = {}
    for n in nodes:
        if n["parent_id"] >= 0:
            children.setdefault(n["parent_id"], []).append(n)
    records = []
    for n in nodes:
        best = 0xFFFF  # MOVE_NONE_BITS (snapshot solved-record sentinel)
        if n["outcome"] == "win" and n["id"] in children:
            best_child = min(children[n["id"]], key=lambda c: c["depth"])
            best = best_child["mv_bits"]
        records.append(
            {
                "key": n["key"],
                "outcome": n["outcome"],
                "depth": n["depth"],
                "best_move": best,
            }
        )
    return records


# -------------------------------- env ---------------------------------------


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


# -------------------------------- regen -------------------------------------


def censored_plan1_jobs():
    """The plan1 censored cost runs to regenerate (ply >= 2; the ply < 2
    informational seed positions are excluded, giving the plan's 48)."""
    jobs = []
    for p in sorted(glob.glob(os.path.join(PLAN1_STATE, "cost_*_p*.json"))):
        if re.search(r"_\d+mb_p\d+\.json$", os.path.basename(p)):
            continue  # addendum runs excluded
        with open(p) as f:
            r = json.load(f)
        if r.get("censored") and r.get("rc") == 0 and not r.get(
            "informational_seed_position"
        ):
            jobs.append(
                {
                    "line": r["line"],
                    "ply": r["ply"],
                    "fen": r["fen"],
                    "plan1_nodes": r["nodes"],
                }
            )
    return jobs


def cmd_regen(_args):
    ensure_dirs()
    jobs = censored_plan1_jobs()
    todo = []
    for j in jobs:
        out_state = os.path.join(STATE, f"regen_{j['line']}_p{j['ply']}.json")
        if not os.path.exists(out_state):
            todo.append(j)
    print(f"regen: {len(todo)}/{len(jobs)} runs to do", flush=True)
    for i, j in enumerate(todo):
        name = f"{j['line']}_p{j['ply']}"
        snap = os.path.join(SNAPS, f"{name}.tt")
        log = os.path.join(LOGS, f"regen_{name}.log")
        cmd = [
            SOLVER,
            "--fen",
            j["fen"],
            "--timeout",
            str(COST_TIMEOUT),
            "--first-outcome",
            "--tt-dump-path",
            snap,
        ]
        t0 = time.monotonic()
        r = run(cmd, timeout=COST_TIMEOUT + 60)
        wall = time.monotonic() - t0
        rec = dict(j)
        rec["wall"] = round(wall, 3)
        rec["rc"] = r.returncode
        rec["cmd"] = " ".join(cmd)
        for line in r.stdout.splitlines():
            if line.startswith("pre_exit: "):
                kv = dict(p.split("=", 1) for p in line[10:].split())
                rec["exit_reason"] = kv.get("reason")
                rec["nodes"] = int(kv["nodes"]) if "nodes" in kv else None
            elif line.startswith("tt_snapshot: "):
                kv = dict(p.split("=", 1) for p in line[13:].split()[1:])
                rec["tt_bytes"] = int(kv["bytes"]) if "bytes" in kv else None
                rec["tt_solved"] = int(kv["solved"]) if "solved" in kv else None
                rec["tt_unsolved"] = int(kv["unsolved"]) if "unsolved" in kv else None
        with open(log, "w") as f:
            f.write(f"# cmd: {' '.join(cmd)}\n# wall: {wall:.3f}\n")
            f.write("## stdout\n" + r.stdout + "\n## stderr\n" + r.stderr)
        if r.returncode == 0 and os.path.exists(snap):
            with open(os.path.join(SNAPS, f"{name}.tt.keep"), "w") as f:
                f.write("")
        else:
            print(f"regen {name}: FAILED rc={r.returncode}", flush=True)
        with open(os.path.join(STATE, f"regen_{name}.json"), "w") as f:
            json.dump(rec, f, indent=1)
        print(
            f"[{i + 1}/{len(todo)}] {name}: nodes={rec.get('nodes')} "
            f"solved={rec.get('tt_solved')} unsolved={rec.get('tt_unsolved')} "
            f"wall={rec['wall']}s (plan1 nodes={j['plan1_nodes']})",
            flush=True,
        )


# --------------------------------- m1 ---------------------------------------


def _system_of(line):
    """Snapshot file stem -> system (seed line). The d4d5 plies share the
    'd4d5' seed; the ply tag is split off the trailing _p<number>."""
    m = re.match(r"^(.*)_p(\d+)$", line)
    if not m:
        raise ValueError(f"bad snapshot stem {line}")
    return m.group(1)


# Pre-registered systems (plan2 §4 M1). The startpos snapshots are loaded
# for the secondary metric but are not a system of the M1 metric.
M1_SYSTEMS = {"e4", "d4", "nf3", "e4e5", "e4c5", "d4d5", "nf3d5"}


def cmd_m1(args):
    ensure_dirs()
    from array import array

    stems = sorted(
        os.path.basename(p)[:-3] for p in glob.glob(os.path.join(SNAPS, "*.tt"))
    )
    print(f"m1: {len(stems)} snapshots")

    # Memory-lean pass (the first cut OOM'd the 8 GiB cgroup: 48 resident
    # Python sets at ~80 MB each). Per-snapshot SOLVED keys stay resident as
    # sorted array('Q') (~12 MB each); intersections test membership against
    # the transient ALL-keys set of one snapshot at a time.
    solved_arrays = {}
    for stem in stems:
        _h, solved, _u = read_tt_snapshot(os.path.join(SNAPS, f"{stem}.tt"))
        arr = array("Q", sorted(r["key"] for r in solved))
        solved_arrays[stem] = arr
        print(f"  loaded {stem}: solved={len(arr)}", flush=True)

    def all_keys(stem):
        _h, solved, unsolved = read_tt_snapshot(os.path.join(SNAPS, f"{stem}.tt"))
        keys = {r["key"] for r in unsolved}
        keys.update(solved_arrays[stem])
        return keys

    m1_stems = [s for s in stems if _system_of(s) in M1_SYSTEMS]
    pairs = []
    for b in stems:
        if _system_of(b) not in M1_SYSTEMS or len(solved_arrays[b]) < args.min_solved:
            continue
        b_all = all_keys(b)
        for a in m1_stems:
            if a == b or _system_of(a) == _system_of(b):
                continue
            a_solved = solved_arrays[a]
            if len(a_solved) < args.min_solved:
                continue
            inter = sum(1 for k in a_solved if k in b_all)
            pairs.append(
                {
                    "a": a,
                    "b": b,
                    "a_solved": len(a_solved),
                    "b_solved": len(solved_arrays[b]),
                    "intersection": inter,
                    "vshare": inter / max(1, len(a_solved)),
                }
            )
        del b_all
        print(f"  paired {b}", flush=True)
    shares = sorted(p["vshare"] for p in pairs)
    m1 = None
    if shares:
        mid = len(shares) // 2
        m1 = shares[mid] if len(shares) % 2 else (shares[mid - 1] + shares[mid]) / 2

    # Secondary (non-gating): p30/p32 proof-tree node keys covered by each
    # quiet snapshot's solved section (transient set per snapshot).
    secondary = {}
    for tag in ("p30", "p32"):
        tree_path = os.path.join(PLAN1, f"reconstructed_d4d5_{tag}.bin")
        keys = {n["key"] for n in read_proof_tree_keys(tree_path)}
        per = {}
        for stem in stems:
            solved_set = set(solved_arrays[stem])
            per[stem] = len(keys & solved_set) / max(1, len(keys))
            del solved_set
        ordered = sorted(per.items(), key=lambda kv: -kv[1])
        secondary[tag] = {
            "tree_nodes": len(keys),
            "per_snapshot_top10": ordered[:10],
            "median_share": sorted(per.values())[len(per.values()) // 2],
            "max_share": ordered[0] if ordered else None,
        }

    verdict = (
        "GO" if m1 is not None and m1 >= M1_GO else ("NO-GO" if m1 is not None and m1 < M1_NOGO else "JUDGMENT")
    )
    out = {
        "min_solved": args.min_solved,
        "n_snapshots": len(stems),
        "n_directed_pairs": len(pairs),
        "M1_median_vshare": m1,
        "gate": {"GO>=": M1_GO, "NO-GO<": M1_NOGO, "verdict": verdict},
        "vshare_min": min(shares) if shares else None,
        "vshare_max": max(shares) if shares else None,
        "per_system_median": _per_system_medians(pairs),
        "secondary": secondary,
        "pairs": pairs,
    }
    with open(os.path.join(ROOT, "m1.json"), "w") as f:
        json.dump(out, f, indent=1)
    print(
        f"M1 = {m1:.4%} over {len(pairs)} directed pairs "
        f"(min {out['vshare_min']:.2%}, max {out['vshare_max']:.2%}) -> {verdict}"
    )
    for tag, sec in secondary.items():
        print(
            f"secondary {tag}: median {sec['median_share']:.2%}, "
            f"max {sec['max_share'][1]:.2%} at {sec['max_share'][0]} "
            f"({sec['tree_nodes']} tree keys)"
        )


def _per_system_medians(pairs):
    """Median vshare by (a.system -> b.system) pair of systems."""
    by_sys = {}
    for p in pairs:
        by_sys.setdefault((_system_of(p["a"]), _system_of(p["b"])), []).append(
            p["vshare"]
        )
    out = {}
    for k in sorted(by_sys):
        v = sorted(by_sys[k])
        out[f"{k[0]}->{k[1]}"] = v[len(v) // 2]
    return out


# ------------------------------- status -------------------------------------


def cmd_status(_args):
    jobs = censored_plan1_jobs()
    done = sum(
        1
        for j in jobs
        if os.path.exists(os.path.join(STATE, f"regen_{j['line']}_p{j['ply']}.json"))
    )
    print(f"regen: {done}/{len(jobs)}")
    snaps = glob.glob(os.path.join(SNAPS, "*.tt"))
    print(f"snapshots: {len(snaps)}")
    if os.path.exists(os.path.join(ROOT, "m1.json")):
        with open(os.path.join(ROOT, "m1.json")) as f:
            m1 = json.load(f)
        print(f"m1: M1={m1['M1_median_vshare']:.4%} verdict={m1['gate']['verdict']}")


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("env").set_defaults(fn=cmd_env)
    sub.add_parser("regen").set_defaults(fn=cmd_regen)
    p = sub.add_parser("m1")
    p.add_argument("--min-solved", type=int, default=1000)
    p.set_defaults(fn=cmd_m1)
    sub.add_parser("status").set_defaults(fn=cmd_status)
    args = ap.parse_args()
    ensure_dirs()
    args.fn(args)


if __name__ == "__main__":
    main()
