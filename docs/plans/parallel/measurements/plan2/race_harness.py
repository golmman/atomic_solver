#!/usr/bin/env python3
"""Option-C root-race sizing harness (plan2 spike).

Measurement artifact only -- NOT shipped code. Python 3, stdlib only.
Drives the unmodified release binary (`atomic_solver`) and the existing
example binaries (`list_legal`, `replay`, `find_winning_child`) as black
boxes. If backlog #3 GOes, plan 3 promotes this logic into a real
`examples/` binary.

Perspective mapping (explicit, cross-validated by `check`):

    A root move is PROVEN WINNING for the root side iff
      - the child position is already terminal (replay's `outcome:` is
        not None), or
      - the child solve (child's side to move) returns `loss`.
    Child `win`  => the root move loses for the root side (eliminated).
    Child `draw` (incl. budget timeout) => unproven, consumed as such.
    All children eliminated/unproven with no proven move => the root is
    not proven by this race (a full disproof requires ALL children to
    return `win`; reported when observed, not a spike goal).

Subcommands:
    children        enumerate root children (move, child FEN, terminal outcome)
    solve-children  sequential per-child solves, wall pass + work (nodes) pass
    root            sequential root solve baseline (wall pass + work pass)
    race            N-worker root-child race, first proven child stops all
    check           proven-move set vs `find_winning_child` (exact agreement)
    simulate        deterministic N-worker schedule over measured per-child data

Work proxy: `nodes` is the documented proxy for the CLI-invisible
`child_evals` (parsed from the solver's `pre_exit: ... nodes=N` line,
which requires a non-`--outcome-only` pass). For workers killed
mid-flight the last stderr chunk-log `nodes=` is a lower bound.
"""

import argparse
import json
import os
import re
import shutil
import statistics
import subprocess
import sys
import threading
import time

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../../../.."))
SOLVER = os.path.join(REPO, "target/release/atomic_solver")
LIST_LEGAL = os.path.join(REPO, "target/release/examples/list_legal")
REPLAY = os.path.join(REPO, "target/release/examples/replay")
FIND_WINNING = os.path.join(REPO, "target/release/examples/find_winning_child")

CHUNK_NODES_RE = re.compile(r"nodes=(\d+)")
PRE_EXIT_RE = re.compile(r"pre_exit: reason=(\S+) outcome=(\S+) nodes=(\d+)")
OUTCOME_RE = re.compile(r"^outcome: (\S+) length: (\d+)", re.M)


def tool(binary, name):
    if not os.path.exists(binary):
        sys.exit(f"missing {name} at {binary}; build with "
                 "`cargo build --release --examples`")
    return binary


def parse_outcome(stdout):
    m = OUTCOME_RE.search(stdout)
    return (m.group(1), int(m.group(2))) if m else (None, None)


def parse_nodes(stdout, stderr):
    m = PRE_EXIT_RE.search(stdout)
    if m:
        return int(m.group(3)), m.group(1)
    # killed worker: last chunk log is a lower bound
    ns = CHUNK_NODES_RE.findall(stderr or "")
    return (int(ns[-1]) if ns else 0), "killed"


def run_solver(fen, timeout, work_pass=False):
    """Run one solver process. work_pass: non-`--outcome-only` so the
    `pre_exit:` nodes line is printed (stdin=DEVNULL -> reader sees EOF)."""
    cmd = [tool(SOLVER, "solver"), "--fen", fen, "--timeout", str(timeout),
           "--first-outcome"]
    if not work_pass:
        cmd.append("--outcome-only")
    t0 = time.monotonic()
    p = subprocess.run(cmd, capture_output=True, text=True,
                       stdin=subprocess.DEVNULL)
    wall = time.monotonic() - t0
    outcome, pv_len = parse_outcome(p.stdout)
    nodes, reason = parse_nodes(p.stdout, p.stderr) if work_pass else (None, None)
    return {"outcome": outcome, "pv_len": pv_len, "wall_s": round(wall, 3),
            "nodes": nodes, "reason": reason}


def root_children(fen):
    """(move, child_fen, terminal_outcome) for every legal root move."""
    p = subprocess.run([tool(LIST_LEGAL, "list_legal"), fen],
                       capture_output=True, text=True, check=True)
    moves = re.search(r"legal_moves \((\d+)\):\n((?:  \S+\n)+)", p.stdout)
    out = []
    for mv in moves.group(2).split():
        r = subprocess.run([tool(REPLAY, "replay"), fen, mv],
                           capture_output=True, text=True, check=True)
        cfen = re.search(r"^fen: (.+)$", r.stdout, re.M).group(1)
        term = re.search(r"^outcome: (.+)$", r.stdout, re.M).group(1)
        out.append({"move": mv, "child_fen": cfen.strip(),
                    "terminal": None if term.strip() == "None" else term.strip()})
    assert len(out) == int(moves.group(1)), "move count mismatch"
    return out


def is_proven(child):
    """Root move proven winning <=> child terminal or child solve `loss`."""
    return child["terminal"] is not None or child["outcome"] == "loss"


# ---------------------------------------------------------------- subcommands

def cmd_children(args):
    ch = root_children(args.fen)
    print(json.dumps(ch, indent=1))
    if args.out:
        json.dump(ch, open(args.out, "w"), indent=1)


def cmd_solve_children(args):
    ch = json.load(open(args.children)) if args.children else root_children(args.fen)
    for c in ch:
        if c["terminal"] is not None:
            c["wall_pass"] = {"outcome": "terminal", "wall_s": 0.0}
            c["work_pass"] = {"nodes": 0, "note": "terminal child, not searched"}
            continue
        c["wall_pass"] = run_solver(c["child_fen"], args.cap, work_pass=False)
        c["work_pass"] = run_solver(c["child_fen"], args.cap, work_pass=True)
        c["proven"] = is_proven({"terminal": c["terminal"],
                                 "outcome": c["wall_pass"]["outcome"]})
        print(f'{c["move"]:6s} outcome={c["wall_pass"]["outcome"]} '
              f'wall={c["wall_pass"]["wall_s"]:7.2f}s '
              f'nodes={c["work_pass"]["nodes"]} proven={c["proven"]}',
              flush=True)
    if args.out:
        json.dump(ch, open(args.out, "w"), indent=1)


def cmd_root(args):
    r = {"fen": args.fen, "wall_pass": run_solver(args.fen, args.timeout, False),
         "work_pass": run_solver(args.fen, args.timeout, True)}
    print(json.dumps(r, indent=1))
    if args.out:
        json.dump(r, open(args.out, "w"), indent=1)


class Worker:
    """One solver process + stderr/stdout reader threads + VmHWM sampler."""

    def __init__(self, child_fen, cap):
        self.child_fen = child_fen
        self.cmd = [tool(SOLVER, "solver"), "--fen", child_fen,
                    "--timeout", str(cap), "--first-outcome", "--outcome-only"]
        self.proc = None
        self.stdout_data = []
        self.stderr_data = []
        self.peak_rss_kb = 0
        self.t_start = None
        self.drain_threads = []

    def start(self):
        self.proc = subprocess.Popen(self.cmd, stdout=subprocess.PIPE,
                                     stderr=subprocess.PIPE, text=True)
        self.t_start = time.monotonic()
        for stream, sink in ((self.proc.stdout, self.stdout_data),
                             (self.proc.stderr, self.stderr_data)):
            t = threading.Thread(target=self._drain, args=(stream, sink),
                                 daemon=True)
            t.start()
            self.drain_threads.append(t)
        self._sample_rss()

    def _drain(self, stream, sink):
        for line in stream:
            sink.append(line)

    def _sample_rss(self):
        def sampler():
            while self.proc.poll() is None:
                try:
                    with open(f"/proc/{self.proc.pid}/status") as f:
                        for line in f:
                            if line.startswith("VmHWM:"):
                                kb = int(line.split()[1])
                                self.peak_rss_kb = max(self.peak_rss_kb, kb)
                except OSError:
                    break
                time.sleep(0.02)
        threading.Thread(target=sampler, daemon=True).start()

    def poll(self):
        return self.proc.poll()

    def kill(self):
        if self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                self.proc.kill()

    def result(self):
        if self.proc.poll() is not None:
            # exited: ensure the pipe readers have drained before parsing
            for t in self.drain_threads:
                t.join(timeout=2.0)
        wall = time.monotonic() - self.t_start
        out, pvl = parse_outcome("".join(self.stdout_data))
        nodes, reason = parse_nodes("".join(self.stdout_data),
                                    "".join(self.stderr_data))
        return {"move": getattr(self, "move_tag", None),
                "outcome": out, "pv_len": pvl, "wall_s": round(wall, 3),
                "nodes": nodes, "reason": reason,
                "peak_rss_mb": round(self.peak_rss_kb / 1024, 1)}


def cmd_race(args):
    ch = json.load(open(args.children))
    terminal = [c["move"] for c in ch if c["terminal"] is not None]
    if terminal:
        sys.exit(f"terminal root children {terminal}: race trivially won "
                 f"immediately; race run not meaningful")
    pending = [c for c in ch]
    reps = []
    for rep in range(args.reps):
        proven_move, wall, winner_nodes = None, None, None
        queue = list(pending)
        workers, started, all_results = [], [], []
        t0 = time.monotonic()
        while queue and len(workers) < args.workers:
            c = queue.pop(0)
            w = Worker(c["child_fen"], args.cap)
            w.move_tag = c["move"]
            w.start()
            workers.append(w)
            started.append(c["move"])
        while True:
            done = [w for w in workers if w.poll() is not None]
            for w in done:
                res = w.result()
                res["move"] = w.move_tag
                all_results.append(res)
                if res["outcome"] == "loss":  # terminal children cannot occur
                    proven_move, wall = res["move"], time.monotonic() - t0
                    winner_nodes = res["nodes"]
                    break
                if queue:
                    c = queue.pop(0)
                    nw = Worker(c["child_fen"], args.cap)
                    nw.move_tag = c["move"]
                    nw.start()
                    workers[workers.index(w)] = nw
                    started.append(c["move"])
                else:
                    workers.remove(w)
            if proven_move:
                for w in workers:
                    w.kill()
                break
            if not workers and not queue:
                wall = time.monotonic() - t0
                break
            time.sleep(0.005)
        per_worker = []
        for w in workers:
            r = w.result()
            r["move"] = w.move_tag
            per_worker.append(r)
        agg_nodes = sum(r["nodes"] or 0 for r in all_results + per_worker)
        reps.append({"rep": rep, "wall_s": round(wall, 3),
                     "proven_move": proven_move,
                     "winner_nodes": winner_nodes,
                     "aggregate_nodes_lower_bound": agg_nodes,
                     "started": started, "workers": per_worker,
                     "all_worker_results": all_results + per_worker})
        print(f'rep {rep}: wall={reps[-1]["wall_s"]}s proven={proven_move} '
              f'agg_nodes={agg_nodes}', flush=True)
    walls = [r["wall_s"] for r in reps]
    print(json.dumps({"fen": args.fen, "workers": args.workers, "cap": args.cap,
                      "reps": reps,
                      "median_wall_s": statistics.median(walls)}, indent=1))
    if args.out:
        json.dump({"fen": args.fen, "workers": args.workers, "cap": args.cap,
                   "reps": reps,
                   "median_wall_s": statistics.median(walls)},
                  open(args.out, "w"), indent=1)


def cmd_check(args):
    ch = root_children(args.fen)
    for c in ch:
        if c["terminal"] is not None:
            c["outcome"] = "terminal"
            c["proven"] = True
            continue
        r = run_solver(c["child_fen"], args.cap, work_pass=False)
        c["outcome"] = r["outcome"]
        c["proven"] = r["outcome"] == "loss"
    proven_in_order = [c["move"] for c in ch if c["proven"]]
    p = subprocess.run([tool(FIND_WINNING, "find_winning_child"), args.fen],
                       capture_output=True, text=True)
    theirs = re.findall(r"WINNING MOVE for root: (\S+)", p.stderr)
    # find_winning_child stops at the FIRST winning move in its (identical
    # MoveList) enumeration order, so exact agreement means: its reported
    # move equals the harness's first proven move. (The harness's full
    # proven set is a superset by construction; a false proven move there
    # would surface as a mismatch on the first move or a wrong root
    # outcome, which the caller cross-checks against the suite's
    # `expected` field.)
    first = proven_in_order[:1]
    ok = theirs == first
    print(json.dumps({"fen": args.fen, "harness_proven_in_order": proven_in_order,
                      "harness_proven_set": sorted(proven_in_order),
                      "find_winning_child": theirs, "exact_agreement": ok},
                     indent=1))
    sys.exit(0 if ok else 1)


def cmd_simulate(args):
    """Deterministic schedule over measured per-child (wall, nodes) data.

    Labeled SIMULATION throughout: at 4 cores, N=8/16 wall runs would
    measure the container, not the architecture. Children scheduled in
    the given (list_legal) order; --also-sjf adds a shortest-job-first
    sensitivity variant. Killed children's work is prorated linearly in
    time (approximation; the real solver's partial work is unknown).
    """
    ch = json.load(open(args.children))
    seq_wall = args.seq_wall
    orders = {"order": ch}
    if args.also_sjf:
        sjf = [c for c in ch if c["wall_pass"]["wall_s"] is not None]
        orders["sjf"] = sorted(sjf, key=lambda c: c["wall_pass"]["wall_s"])
    results = {}
    for name, pool in orders.items():
        queue = list(pool)
        live = []  # (finish_time, child)
        t = 0.0
        proven_t, proven_move = None, None
        work = 0.0
        started = []
        while queue and len(live) < args.workers:
            c = queue.pop(0)
            live.append((c["wall_pass"]["wall_s"], c))
            started.append(c["move"])
        while live:
            live.sort(key=lambda x: x[0])
            fin, c = live.pop(0)
            dt = fin - t
            t = fin
            # prorate work for all live children over this interval
            for _, other in live:
                w = other["wall_pass"]["wall_s"]
                work += other["work_pass"]["nodes"] * dt / w if w else 0
            c["outcome"] = c.get("outcome") or c["wall_pass"]["outcome"]
            if c["terminal"] is not None or c["wall_pass"]["outcome"] == "loss":
                proven_t, proven_move = t, c["move"]
                for _, other in live:
                    work += other["work_pass"]["nodes"] * 0  # killed, prorated above
                break
            if queue:
                c2 = queue.pop(0)
                live.append((t + c2["wall_pass"]["wall_s"], c2))
                started.append(c2["move"])
        if proven_t is None:
            proven_t = t
        results[name] = {"workers": args.workers, "projected_wall_s": round(t, 3),
                         "proven_move": proven_move,
                         "speedup_vs_seq": round(seq_wall / t, 2) if t else None,
                         "tail_s": round(t - (proven_t if proven_move else t), 3),
                         "aggregate_nodes_prorated": int(work),
                         "started": started}
    print(json.dumps({"simulation": True, "note": "SIMULATED schedule over "
                      "measured per-child data, not a wall measurement",
                      "seq_wall_s": seq_wall, "results": results}, indent=1))
    if args.out:
        json.dump(results, open(args.out, "w"), indent=1)


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("children")
    c.add_argument("--fen", required=True)
    c.add_argument("--out")
    c.set_defaults(fn=cmd_children)
    c = sub.add_parser("solve-children")
    c.add_argument("--fen", required=True)
    c.add_argument("--children", help="children JSON (default: enumerate)")
    c.add_argument("--cap", type=int, required=True)
    c.add_argument("--out")
    c.set_defaults(fn=cmd_solve_children)
    c = sub.add_parser("root")
    c.add_argument("--fen", required=True)
    c.add_argument("--timeout", type=int, required=True)
    c.add_argument("--out")
    c.set_defaults(fn=cmd_root)
    c = sub.add_parser("race")
    c.add_argument("--fen", required=True)
    c.add_argument("--children", required=True)
    c.add_argument("--workers", type=int, required=True)
    c.add_argument("--cap", type=int, required=True)
    c.add_argument("--reps", type=int, default=1)
    c.add_argument("--out")
    c.set_defaults(fn=cmd_race)
    c = sub.add_parser("check")
    c.add_argument("--fen", required=True)
    c.add_argument("--cap", type=int, default=5)
    c.set_defaults(fn=cmd_check)
    c = sub.add_parser("simulate")
    c.add_argument("--children", required=True)
    c.add_argument("--workers", type=int, required=True)
    c.add_argument("--seq-wall", type=float, required=True)
    c.add_argument("--also-sjf", action="store_true")
    c.add_argument("--out")
    c.set_defaults(fn=cmd_simulate)
    args = ap.parse_args()
    args.fn(args)


if __name__ == "__main__":
    main()
