#!/usr/bin/env python3
"""Plan5 arm driver: S baseline + C2/C4/C2-nr/C2-nf campaign arms.

Protocol (pre-registered in docs/plans/solve/plan5.md §4):
- strictly sequential across arms; nothing else runs concurrently;
- 5 reps per arm, wall medians; deterministic work caps (child evals);
- arms: S (in-process sequential first-outcome baseline, reports
  nodes AND child_evals), C2 (2 persistent workers), C4 (4 workers),
  C2-nr (worker TT reset per job), C2-nf (no feedback: jobs run to
  completion, static-order dispatch).

Usage:
  run_arms.py env                     # write env.json
  run_arms.py seq <name> <FEN> <reps> # sequential baseline reps
  run_arms.py arm <name> <FEN> <arm> <workers> <reps> <maxwall> [nf] [nr]
  run_arms.py drift <name> <FEN>      # one CLI binary drift run
"""

import json
import os
import platform
import shutil
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(ROOT)))))
MASTER = os.path.join(REPO, "target", "release", "examples", "campaign_master")
WORKER = os.path.join(REPO, "target", "release", "examples", "campaign_worker")
SOLVER = os.path.join(REPO, "target", "release", "atomic_solver")
STATE = os.path.join(ROOT, "state")
SESSION_BASE = "/tmp/plan5_arms"

SLICE = 1_000_000
MAX_SLICE = 8_000_000


def session_dir(name):
    d = os.path.join(SESSION_BASE, name)
    shutil.rmtree(d, ignore_errors=True)
    os.makedirs(d)
    return d


def run_seq(name, fen, reps):
    rows = []
    for rep in range(reps):
        d = session_dir(f"seq_{name}_{rep}")
        t0 = time.time()
        out = subprocess.run(
            [MASTER, "--session", d, "--mode", "seq", "--fen", fen,
             "--seq-timeout", "3600"],
            capture_output=True, text=True, timeout=4000,
        )
        wall_incl = time.time() - t0
        summary = json.loads(out.stdout)
        row = {
            "rep": rep,
            "fen": fen,
            "outcome": summary["outcome"],
            "wall_s": summary["wall_s"],
            "nodes": summary["worker_nodes"],
            "child_evals": summary["worker_child_evals"],
            "wall_incl_startup_s": wall_incl,
        }
        rows.append(row)
        print(json.dumps(row))
        with open(os.path.join(STATE, f"seq_{name}_rep{rep}.json"), "w") as f:
            json.dump(row, f, indent=2)


def run_drift(name, fen):
    t0 = time.time()
    out = subprocess.run(
        [SOLVER, "--fen", fen, "--first-outcome", "--outcome-only", "--timeout", "600"],
        capture_output=True, text=True, timeout=4000,
    )
    wall = time.time() - t0
    outcome, nodes = None, None
    for line in (out.stderr + out.stdout).splitlines():
        if line.startswith("outcome:"):
            outcome = line.split()[1]
        if line.startswith("pre_exit:"):
            for tok in line.split():
                if tok.startswith("nodes="):
                    nodes = int(tok.split("=")[1])
    row = {"name": name, "source": "cli-drift", "outcome": outcome,
           "wall_s": wall, "nodes": nodes}
    with open(os.path.join(STATE, f"drift_{name}.json"), "w") as f:
        json.dump(row, f, indent=2)
    print(json.dumps(row))


def run_arm(name, fen, arm, workers, reps, maxwall, nf=False, nr=False, tt_mb=128):
    rows = []
    for rep in range(reps):
        tag = f"{name}_{arm}_rep{rep}"
        d = session_dir(tag)
        t0 = time.time()
        master = subprocess.Popen(
            [MASTER, "--session", d, "--fen", fen, "--workers", str(workers),
             "--slice", str(SLICE), "--max-slice", str(MAX_SLICE),
             "--max-wall", str(maxwall), "--tt-mb", str(tt_mb), "--pt-mb", "512",
             "--out", os.path.join(d, "proof_tree.bin")]
            + (["--nf"] if nf else []),
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        )
        procs = [master]
        for i in range(workers):
            p = subprocess.Popen(
                [WORKER, "--session", d, "--worker", str(i),
                 "--tt-mb", str(tt_mb), "--retention", "off" if nr else "on"],
                stdout=subprocess.DEVNULL, stderr=open(os.path.join(d, f"w{i}.err"), "w"),
            )
            procs.append(p)
        out, err = master.communicate(timeout=maxwall + 300)
        # keep the master's rejection diagnostics (VERIFY FAILED lines etc.)
        with open(os.path.join(d, "master.err"), "w") as mf:
            mf.write(err)
        for p in procs[1:]:
            p.terminate()
        for p in procs[1:]:
            try:
                p.wait(timeout=10)
            except subprocess.TimeoutExpired:
                p.kill()
        wall = time.time() - t0
        summary = json.loads(out) if out else {"error": "no summary", "stderr": err[-2000:]}
        summary["rep"] = rep
        summary["arm"] = arm
        summary["position"] = name
        summary["wall_incl_startup_s"] = wall
        summary["worker_logs"] = {
            f"w{i}": sorted(
                l for l in open(os.path.join(d, f"w{i}.err")).read().splitlines()
                if l.startswith("job ")
            )
            for i in range(workers)
        }
        rows.append(summary)
        summary["master_verify_failures"] = [
            l for l in err.splitlines() if "VERIFY FAILED" in l or "export failed" in l
        ][:20]
        slim = {k: v for k, v in summary.items() if k != "worker_logs"}
        print(json.dumps(slim))
        shutil.copy(os.path.join(d, "summary.json"),
                    os.path.join(STATE, f"{tag}_summary.json")) \
            if os.path.exists(os.path.join(d, "summary.json")) else None
        with open(os.path.join(STATE, f"{tag}.json"), "w") as f:
            json.dump(summary, f, indent=2)
        # keep the proof artifact of rep 0 for the soundness audit
        if rep == 0 and os.path.exists(os.path.join(d, "proof_tree.bin")):
            shutil.copy(os.path.join(d, "proof_tree.bin"),
                        os.path.join(STATE, f"{tag}_proof_tree.bin"))


def write_env():
    import hashlib
    env = {
        "date": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "host": platform.node(),
        "kernel": platform.release(),
        "python": platform.python_version(),
        "nproc": os.cpu_count(),
        "mem_gib": os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES") / 2**30,
        "cargo_pkg": os.path.getmtime(os.path.join(REPO, "Cargo.toml")),
        "solver_binary_sha256": hashlib.sha256(open(SOLVER, "rb").read()).hexdigest()[:16],
        "campaign_master_sha256": hashlib.sha256(open(MASTER, "rb").read()).hexdigest()[:16],
        "campaign_worker_sha256": hashlib.sha256(open(WORKER, "rb").read()).hexdigest()[:16],
        "slice_budget": SLICE,
        "max_slice": MAX_SLICE,
    }
    with open(os.path.join(ROOT, "env.json"), "w") as f:
        json.dump(env, f, indent=2)
    print(json.dumps(env))


if __name__ == "__main__":
    os.makedirs(STATE, exist_ok=True)
    cmd = sys.argv[1]
    if cmd == "env":
        write_env()
    elif cmd == "seq":
        run_seq(sys.argv[2], sys.argv[3], int(sys.argv[4]))
    elif cmd == "drift":
        run_drift(sys.argv[2], sys.argv[3])
    elif cmd == "arm":
        name, fen, arm = sys.argv[2], sys.argv[3], sys.argv[4]
        workers, reps, maxwall = int(sys.argv[5]), int(sys.argv[6]), int(sys.argv[7])
        nf = "nf" in sys.argv[8:]
        nr = "nr" in sys.argv[8:]
        run_arm(name, fen, arm, workers, reps, maxwall, nf, nr)
    else:
        raise SystemExit(f"unknown cmd {cmd}")
