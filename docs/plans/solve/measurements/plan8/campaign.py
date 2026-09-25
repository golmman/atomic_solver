#!/usr/bin/env python3
"""Plan 8 harness (`solve` initiative): campaign arms at 4 workers on the
d4d5-p2 root (item 8, stage 2 of 3), driving the unmodified release
binaries as a black box (Python 3 stdlib only).  No product changes.

Subcommands:
  env              -- record environment metadata to env.json (plan6
                      binary-SHA convention + plan7 cgroup convention)
  arm <NAME>       -- one registered arm, strictly sequential, resumable
                      per arm (state/arm_<tag>.json exists => done):
               SM   : master + 4 workers, --max-wall 600 (harness
                      validation, early 4-worker rate, memory profile)
               S600 : master --mode seq --seq-timeout 600 (sequential
                      nodes<->child-evals conversion on this root)
               C4   : master + 4 workers, --max-wall 14400 (headline:
                      R_camp, m2 at the stage-2 design point)
               C1   : master + 4 workers, --max-wall 3600 (rate
                      stability point over the same 4x span as stage 1)
  confirm          -- the registered INFEASIBLE confirmation run: 2
                      workers, 1800 s, same shape (only on a §2.3
                      abort signal at 4 workers)
  analyze          -- apply the pre-registered stage-2 gate (plan8 §3):
                      m2 per arm, verdict, kappa both sides, memory
                      profile, job health, floor extension.  Writes
                      analysis.json.
  status           -- print progress summary

Registered shape (plan8 §2, incumbent V1 — no variants): master
`--slice 4000000 --max-slice 8000000` (feedback on, no --nf), retention
on (default), no --abandon; `--tt-mb 128 --pt-mb 512`; workers
`--tt-mb 128 --retention on`.  Tags carry arm AND worker count (plan6
tag-collision lesson): `<arm>w<workers>_rep<k>`.

Memory protocol (plan8 §2.3): a tree-RSS sampler (sum of
/proc/<pid>/statm RSS over master + workers, 10 s cadence) enforces the
registered abort rule — tree-RSS > 7.0 GiB on two consecutive samples,
or any single process > 3.5 GiB => SIGTERM the session, record the
INFEASIBLE signal; a worker vanishing (OOM SIGKILL / cgroup oom_kill)
during the run is the same signal.  Max-RSS is additionally recorded
per process via os.wait4 ru_maxrss (plan7 convention; no /usr/bin/time
in this container).

Protocol (plan8 §2): strictly sequential arms, nothing else running
concurrently, stdin from DEVNULL, raw stdout/stderr captures under
logs/, state JSONs under state/.  An arm interrupted > 30 min in is
NOT re-run (compute cap, plan8 §7).

R_seq = 198,229 nodes/s is locked from stage 1 (report7 §2/§3).
"""

from __future__ import annotations

import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", "..", "..", ".."))
SOLVER = os.path.join(REPO, "target", "release", "atomic_solver")
MASTER = os.path.join(REPO, "target", "release", "examples", "campaign_master")
WORKER = os.path.join(REPO, "target", "release", "examples", "campaign_worker")
INSPECT = os.path.join(REPO, "target", "release", "examples", "inspect_pt")

LOGS = os.path.join(HERE, "logs")
STATE = os.path.join(HERE, "state")
SESSION_BASE = "/tmp/plan8_arms"

# Registered root and shape (plan8 §2, fixed before any run).
ROOT_FEN = "rnbqkbnr/ppp1pppp/8/3p4/3P4/8/PPP1PPPP/RNBQKBNR w KQkq d6 0 2"
SLICE, MAX_SLICE = 4_000_000, 8_000_000
TT_MB, PT_MB = 128, 512

# Locked stage-1 input (report7 §2/§3) and its censoring floor.
R_SEQ = 198_229.0
N_FLOOR_STAGE1 = 2_870_968_320

# Registered memory gate (plan8 §2.3).
TREE_ABORT_BYTES = int(7.0 * 2**30)
PROC_ABORT_BYTES = int(3.5 * 2**30)
SAMPLER_CADENCE_S = 10
ABORT_CONSECUTIVE = 2
WATCHDOG_MARGIN = 600

# Registered arm table (plan8 §2): tag -> (mode, budget_s, workers).
ARMS = {
    "SM": ("campaign", 600, 4),
    "S600": ("seq", 600, 0),
    "C4": ("campaign", 14400, 4),
    "C1": ("campaign", 3600, 4),
}
ARM_ORDER = ("SM", "S600", "C4", "C1")

PAGE = os.sysconf("SC_PAGE_SIZE")
JOB_RE = re.compile(
    r"^job (\S+) outcome=(\S+) exit=(\S+) evals=(\d+) nodes=(\d+)"
    r" wall=([0-9.]+) events=(\d+)"
)


def ensure_dirs():
    for d in (LOGS, STATE):
        os.makedirs(d, exist_ok=True)


def _read(path):
    try:
        with open(path) as f:
            return f.read().strip()
    except OSError:
        return None


def read_oom_events():
    """cgroup v2 memory.events oom_kill counter (cumulative)."""
    txt = _read("/sys/fs/cgroup/memory.events") or ""
    for line in txt.splitlines():
        if line.startswith("oom_kill "):
            return int(line.split()[1])
    return None


def rss_of(pid):
    try:
        with open(f"/proc/{pid}/statm") as f:
            return int(f.read().split()[1]) * PAGE
    except (OSError, IndexError, ValueError):
        return None


def fresh_session(tag):
    d = os.path.join(SESSION_BASE, tag)
    shutil.rmtree(d, ignore_errors=True)
    os.makedirs(d)
    return d


# ------------------------------- sampler -------------------------------------


class Sampler:
    """Tree-RSS sampler with the registered abort rule (plan8 §2.3)."""

    def __init__(self, pids):
        self.pids = pids  # [(name, pid)]
        self.samples = []
        self.aborted = False
        self.vanished = []
        self.peak_total = 0
        self.peak_single = {}
        self.abort_event = threading.Event()
        self._over = 0
        self._thread = threading.Thread(target=self._loop, daemon=True)

    def start(self):
        self._thread.start()

    def stop(self):
        """Stop sampling and snapshot the state at stop time.  Called
        when the master exits, BEFORE the workers are terminated, so a
        worker dying during teardown is not misread as a crash."""
        self.abort_event.set()
        return {
            "aborted": self.aborted,
            "vanished": list(self.vanished),
            "peak_total_bytes": self.peak_total,
            "peak_single_bytes": dict(self.peak_single),
            "samples_taken": len(self.samples),
        }

    def _loop(self):
        t0 = time.monotonic()
        while not self.abort_event.is_set():
            row = {"t": round(time.monotonic() - t0, 1), "total": 0,
                   "per": {}}
            missing = []
            for name, pid in self.pids:
                r = rss_of(pid)
                if r is None:
                    missing.append(name)
                    continue
                row["per"][name] = r
                row["total"] += r
                if r > self.peak_single.get(name, 0):
                    self.peak_single[name] = r
            self.samples.append(row)
            if row["total"] > self.peak_total:
                self.peak_total = row["total"]
            # registered rule: tree-RSS watermark on 2 consecutive samples
            if row["total"] > TREE_ABORT_BYTES:
                self._over += 1
            else:
                self._over = 0
            single_over = any(r > PROC_ABORT_BYTES
                              for r in row["per"].values())
            # a worker gone while the session is live: OOM kill or crash
            for name in missing:
                if name not in self.vanished:
                    self.vanished.append(name)
            if self._over >= ABORT_CONSECUTIVE or single_over:
                self.aborted = True
                self.abort_event.set()
            time.sleep(SAMPLER_CADENCE_S)



# ------------------------------ arm runners ----------------------------------


def _parse_job_lines(path):
    jobs = []
    try:
        with open(path) as f:
            for line in f:
                m = JOB_RE.match(line)
                if m:
                    jobs.append({
                        "job_id": m.group(1),
                        "outcome": m.group(2),
                        "exit": m.group(3),
                        "evals": int(m.group(4)),
                        "nodes": int(m.group(5)),
                        "wall_s": float(m.group(6)),
                        "events": int(m.group(7)),
                    })
    except OSError:
        pass
    return jobs


def _sum_results(session_dir):
    """Cross-check the master's worker_nodes accumulator against the
    durable per-job result JSONs (plan8 §2 unit protocol)."""
    rdir = os.path.join(session_dir, "results")
    nodes = evals = n = 0
    if os.path.isdir(rdir):
        for name in sorted(os.listdir(rdir)):
            try:
                with open(os.path.join(rdir, name)) as f:
                    r = json.load(f)
                nodes += r.get("nodes", 0)
                evals += r.get("child_evals", 0)
                n += 1
            except (OSError, json.JSONDecodeError, KeyError, TypeError):
                pass
    return {"results_files": n, "results_nodes": nodes,
            "results_child_evals": evals}


def _read_summary(session_dir, stdout_path):
    sp = os.path.join(session_dir, "summary.json")
    if os.path.exists(sp):
        try:
            with open(sp) as f:
                return json.load(f)
        except json.JSONDecodeError:
            pass
    try:
        with open(stdout_path) as f:
            txt = f.read()
        start = txt.find("{")
        return json.loads(txt[start:]) if start >= 0 else None
    except (OSError, json.JSONDecodeError):
        return None


def run_campaign_arm(tag, maxwall, workers):
    """One master+workers session with the full memory protocol."""
    d = fresh_session(tag)
    oom_before = read_oom_events()
    master_cmd = [
        MASTER, "--session", d, "--fen", ROOT_FEN,
        "--workers", str(workers),
        "--slice", str(SLICE), "--max-slice", str(MAX_SLICE),
        "--max-wall", str(maxwall), "--tt-mb", str(TT_MB),
        "--pt-mb", str(PT_MB), "--out", os.path.join(d, "proof_tree.bin"),
    ]
    t0 = time.time()
    master_out = open(os.path.join(LOGS, f"arm_{tag}.out"), "w")
    master_err = open(os.path.join(LOGS, f"arm_{tag}.err"), "w")
    master = subprocess.Popen(
        master_cmd, stdin=subprocess.DEVNULL, stdout=master_out,
        stderr=master_err,
    )
    wrecs = []  # (name, proc, stderr file)
    for i in range(workers):
        werr = open(os.path.join(LOGS, f"arm_{tag}_w{i}.err"), "w")
        p = subprocess.Popen(
            [WORKER, "--session", d, "--worker", str(i),
             "--tt-mb", str(TT_MB), "--retention", "on"],
            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
            stderr=werr,
        )
        wrecs.append((f"w{i}", p, werr))

    sampler = Sampler([("master", master.pid)] +
                      [(n, p.pid) for n, p, _ in wrecs])
    sampler.start()

    # Wait for the master (it self-limits at --max-wall); the sampler
    # enforces the abort rule; a watchdog is the backstop.
    m_status = m_ru = None
    aborted = False
    deadline = time.monotonic() + maxwall + WATCHDOG_MARGIN
    while True:
        if sampler.abort_event.is_set():
            aborted = True
            break
        try:
            pid_, status, ru = os.wait4(master.pid, os.WNOHANG)
        except ChildProcessError:
            break
        if pid_ == master.pid:
            m_status, m_ru = status, ru
            break
        if time.monotonic() > deadline:
            master.kill()
            try:
                _, m_status, m_ru = os.wait4(master.pid, 0)
            except ChildProcessError:
                pass
            break
        time.sleep(1.0)
    if aborted:
        master.terminate()
    # Freeze the memory protocol the moment the master is done: a worker
    # exiting during teardown is expected, not a crash signal.
    snap = sampler.stop()
    if aborted:
        # reap the terminated master (it may still write summary.json)
        try:
            _, m_status, m_ru = os.wait4(master.pid, 0)
        except ChildProcessError:
            pass
    master_out.close()
    master_err.close()
    wall = round(time.time() - t0, 3)
    # Stop and reap the workers (ru_maxrss each).
    for _, p, werr in wrecs:
        werr.flush()
        try:
            p.terminate()
        except ProcessLookupError:
            pass
    w_recs = {}
    for name, p, _ in wrecs:
        rc, ru = None, None
        t1 = time.monotonic()
        while time.monotonic() - t1 < 30:
            try:
                pid_, status, ru = os.wait4(p.pid, os.WNOHANG)
            except ChildProcessError:
                rc = p.returncode
                break
            if pid_ == p.pid:
                rc = os.waitstatus_to_exitcode(status)
                break
            time.sleep(0.2)
        if rc is None:
            p.kill()
            try:
                _, status, ru = os.wait4(p.pid, 0)
                rc = os.waitstatus_to_exitcode(status)
            except ChildProcessError:
                pass
        p.returncode = rc if rc is not None else -1
        w_recs[name] = {
            "exitcode": rc,
            "ru_maxrss_kb": ru.ru_maxrss if ru else None,
        }

    return _collect(
        tag, d, maxwall, workers, wall, m_status, m_ru, aborted,
        w_recs, snap, oom_before,
    )


def _collect(tag, d, maxwall, workers, wall, m_status, m_ru, aborted,
             w_recs, snap, oom_before):
    oom_after = read_oom_events()
    summary = _read_summary_with_fallback(tag, d)

    xcheck = _sum_results(d)
    job_lines = {}
    for i in range(len(w_recs)):
        job_lines[f"w{i}"] = _parse_job_lines(
            os.path.join(LOGS, f"arm_{tag}_w{i}.err"))
    all_jobs = [j for js in job_lines.values() for j in js]
    node_buckets = {"<1M": 0, "1-4M": 0, "4-8M": 0, ">8M": 0}
    for j in all_jobs:
        n = j["nodes"]
        b = ("<1M" if n < 1e6 else "1-4M" if n < 4e6
             else "4-8M" if n <= 8e6 else ">8M")
        node_buckets[b] += 1

    m_exitcode = (os.waitstatus_to_exitcode(m_status)
                  if m_status is not None else None)

    # The master's worker_nodes accumulator is the registered unit
    # (plan8 §2); per-job result JSONs are the cross-check.
    s_nodes = (summary or {}).get("worker_nodes")
    s_evals = (summary or {}).get("worker_child_evals")
    nodes = s_nodes if s_nodes is not None else xcheck["results_nodes"]
    evals = s_evals if s_evals is not None else xcheck["results_child_evals"]

    rec = {
        "tag": tag,
        "arm": tag,
        "workers": workers,
        "mode": "campaign",
        "budget_secs": maxwall,
        "wall_s": wall,
        "aborted_rss": aborted,
        "sampler": snap,
        "oom_kill_delta": (
            oom_after - oom_before
            if oom_before is not None and oom_after is not None else None
        ),
        "master_exitcode": m_exitcode,
        "master_exit": (summary or {}).get("exit"),
        "master_ru_maxrss_kb": m_ru.ru_maxrss if m_ru else None,
        "worker_rec": w_recs,
        "summary": summary,
        "worker_nodes": nodes,
        "child_evals": evals,
        "cross_check": xcheck,
        "nodes_xcheck_match": (
            s_nodes == xcheck["results_nodes"]
            if s_nodes is not None else None
        ),
        "jobs_dispatched": (summary or {}).get("jobs_dispatched"),
        "jobs_completed": (summary or {}).get("jobs_completed"),
        "job_errors": (summary or {}).get("job_errors"),
        "verify_failures": (summary or {}).get("verify_failures"),
        "children_total": (summary or {}).get("children_total"),
        "children_resolved": (summary or {}).get("children_resolved"),
        "leaves_open": (summary or {}).get("leaves_open"),
        "leaves_won": (summary or {}).get("leaves_won"),
        "leaves_lost": (summary or {}).get("leaves_lost"),
        "outcome": (summary or {}).get("outcome"),
        "job_count": len(all_jobs),
        "job_wall_mean_s": (
            round(sum(j["wall_s"] for j in all_jobs) / len(all_jobs), 3)
            if all_jobs else None
        ),
        "job_node_buckets": node_buckets,
        "job_outcomes": {
            o: sum(1 for j in all_jobs if j["outcome"] == o)
            for o in sorted({j["outcome"] for j in all_jobs})
        },
    }
    if wall > 0 and nodes:
        rec["nodes_per_s"] = round(nodes / wall, 1)
        rec["m2"] = round(rec["nodes_per_s"] / R_SEQ, 4)
    if wall > 0 and nodes and evals:
        rec["kappa"] = round(evals / nodes, 2)

    # COMPLETED branch (plan8 §3): validate the composed artifact.
    if rec["master_exit"] == "proven":
        pt = os.path.join(d, "proof_tree.bin")
        if os.path.exists(pt):
            tv0 = time.time()
            out = subprocess.run(
                [INSPECT, "--validate", pt],
                capture_output=True, text=True, timeout=3600,
            )
            vwall = round(time.time() - tv0, 3)
            validate = None
            for line in (out.stdout + out.stderr).splitlines():
                if line.startswith("validate: "):
                    validate = line[10:].strip()
            rec["inspect_validate"] = validate
            rec["verify_wall_s"] = vwall
            rec["combined_wall_s"] = round(wall + vwall, 3)
            rec["artifact"] = validate == "ok"
            shutil.copy(pt, os.path.join(STATE, f"{tag}_proof_tree.bin"))

    crash = bool(snap["vanished"]) or (
        rec["master_exit"] == "verify_failed"
    ) or (rec["oom_kill_delta"] or 0) > 0
    rec["infeasible_signal"] = bool(aborted or crash)

    # worker job lines are part of the state record (plan6 convention)
    rec["worker_job_lines"] = {
        k: [f"{j['job_id']} outcome={j['outcome']} exit={j['exit']} "
            f"evals={j['evals']} nodes={j['nodes']} wall={j['wall_s']} "
            f"events={j['events']}" for j in js]
        for k, js in job_lines.items()
    }

    with open(os.path.join(STATE, f"arm_{tag}.json"), "w") as f:
        json.dump(rec, f, indent=1)
    slim = {k: v for k, v in rec.items()
            if k not in ("worker_job_lines", "sampler", "summary")}
    print(json.dumps(slim))
    return rec


def _read_summary_with_fallback(tag, d):
    sp = os.path.join(d, "summary.json")
    if os.path.exists(sp):
        try:
            with open(sp) as f:
                return json.load(f)
        except json.JSONDecodeError:
            pass
    try:
        with open(os.path.join(LOGS, f"arm_{tag}.out")) as f:
            txt = f.read()
        start = txt.find("{")
        return json.loads(txt[start:]) if start >= 0 else None
    except (OSError, json.JSONDecodeError):
        return None


def run_seq_arm(tag, seq_timeout):
    """Sequential-side conversion run (`--mode seq`, S600)."""
    d = fresh_session(tag)
    out_path = os.path.join(LOGS, f"arm_{tag}.out")
    err_path = os.path.join(LOGS, f"arm_{tag}.err")
    t0 = time.time()
    with open(out_path, "w") as fo, open(err_path, "w") as fe:
        p = subprocess.Popen(
            [MASTER, "--session", d, "--mode", "seq", "--fen", ROOT_FEN,
             "--seq-timeout", str(seq_timeout)],
            stdin=subprocess.DEVNULL, stdout=fo, stderr=fe,
        )
        killer = threading.Timer(seq_timeout + WATCHDOG_MARGIN, p.kill)
        killer.start()
        try:
            _pid, status, ru = os.wait4(p.pid, 0)
        finally:
            killer.cancel()
        p.returncode = os.waitstatus_to_exitcode(status)
    wall = round(time.time() - t0, 3)
    summary = _read_summary_with_fallback(tag, d) or {}
    nodes = summary.get("worker_nodes")
    evals = summary.get("worker_child_evals")
    rec = {
        "tag": tag, "arm": tag, "mode": "seq", "workers": 0,
        "budget_secs": seq_timeout, "wall_s": wall,
        "outcome": summary.get("outcome"), "exit": summary.get("exit"),
        "nodes": nodes, "child_evals": evals,
        "kappa": round(evals / nodes, 2) if nodes and evals else None,
        "nodes_per_s": round(nodes / wall, 1) if nodes and wall else None,
        "ru_maxrss_kb": ru.ru_maxrss if ru else None,
        "censored": summary.get("exit") == "timeout",
        "master_exitcode": p.returncode,
    }
    with open(os.path.join(STATE, f"arm_{tag}.json"), "w") as f:
        json.dump(rec, f, indent=1)
    print(json.dumps(rec))
    return rec


def _run_arm(tag):
    mode, budget, workers = ARMS[tag]
    sp = os.path.join(STATE, f"arm_{tag}.json")
    if os.path.exists(sp):
        print(f"arm {tag}: already done (state exists)")
        return
    print(f"arm {tag}: starting (mode={mode} budget={budget}s "
          f"workers={workers}) ...", flush=True)
    if mode == "seq":
        run_seq_arm(tag, budget)
    else:
        run_campaign_arm(tag, budget, workers)


# -------------------------------- analysis -----------------------------------


def _load(tag):
    with open(os.path.join(STATE, f"arm_{tag}.json")) as f:
        return json.load(f)


def _done(tag):
    return os.path.exists(os.path.join(STATE, f"arm_{tag}.json"))


def _arm_row(r):
    mode = r.get("mode")
    return {
        "mode": mode,
        "workers": r.get("workers"),
        "budget_secs": r["budget_secs"],
        "wall_s": r["wall_s"],
        "nodes": r.get("worker_nodes") if mode == "campaign"
                 else r.get("nodes"),
        "child_evals": r.get("child_evals"),
        "kappa": r.get("kappa"),
        "nodes_per_s": r.get("nodes_per_s"),
        "m2": r.get("m2"),
        "master_exit": r.get("master_exit") or r.get("exit"),
        "outcome": r.get("outcome"),
        "censored": (r.get("aborted_rss") is not True) and
                    (r.get("master_exit") == "timeout"
                     if mode == "campaign"
                     else r.get("exit") == "timeout"),
        "aborted_rss": r.get("aborted_rss"),
        "infeasible_signal": r.get("infeasible_signal"),
        "peak_tree_rss_bytes": (r.get("sampler") or {}).get(
            "peak_total_bytes"),
        "peak_single_bytes": (r.get("sampler") or {}).get(
            "peak_single_bytes"),
        "oom_kill_delta": r.get("oom_kill_delta"),
        "jobs_dispatched": r.get("jobs_dispatched"),
        "job_count": r.get("job_count"),
        "job_wall_mean_s": r.get("job_wall_mean_s"),
        "children_total": r.get("children_total"),
        "children_resolved": r.get("children_resolved"),
        "leaves_open": r.get("leaves_open"),
        "leaves_won": r.get("leaves_won"),
        "leaves_lost": r.get("leaves_lost"),
    }


def cmd_analyze(_args):
    arms = {t: _load(t) for t in ARM_ORDER if _done(t)}
    out = {
        "root_fen": ROOT_FEN,
        "r_seq_locked": R_SEQ,
        "shape": {"slice": SLICE, "max_slice": MAX_SLICE,
                  "tt_mb": TT_MB, "pt_mb": PT_MB, "feedback": True,
                  "retention": True, "abandon": False},
        "gate_constants": {"tree_abort_gib": 7.0, "proc_abort_gib": 3.5,
                           "abort_consecutive_samples": ABORT_CONSECUTIVE,
                           "decay_ratio": 0.8},
        "arms": {t: _arm_row(r) for t, r in arms.items()},
    }

    if "S600" in arms:
        out["kappa_seq"] = arms["S600"].get("kappa")

    camp = {t: r for t, r in arms.items()
            if r.get("mode") == "campaign" and t != "CONFw2_rep0"}
    verdict, detail = None, {}

    inf = [t for t, r in camp.items() if r.get("infeasible_signal")]
    if inf:
        verdict = "INFEASIBLE"
        detail = {"infeasible_arms": inf,
                  "registered_confirmation": "CONFw2 (2 workers, 1800 s)",
                  "confirm_done": _done("CONFw2_rep0")}
    else:
        completed = [t for t, r in camp.items()
                     if r.get("master_exit") == "proven"]
        if completed:
            t = completed[0]
            verdict = "COMPLETED"
            detail = {
                "completed_arm": t,
                "artifact": arms[t].get("artifact"),
                "validate": arms[t].get("inspect_validate"),
                "combined_wall_s": arms[t].get("combined_wall_s"),
            }
        else:
            headline = "C4" if "C4" in camp else (
                "C1" if "C1" in camp else "SM" if "SM" in camp else None)
            if headline is None:
                verdict = "INCOMPLETE"
                detail = {"note": "no campaign arm state found"}
            else:
                m2h = arms[headline].get("m2")
                decay = False
                if "C1" in camp and "C4" in camp:
                    m2c1, m2c4 = arms["C1"]["m2"], arms["C4"]["m2"]
                    decay = m2c4 < 0.8 * m2c1
                    detail["m2_C1"] = m2c1
                    detail["m2_C4"] = m2c4
                elif "C4" in camp and "SM" in camp:
                    decay = arms["C4"]["m2"] < 0.8 * arms["SM"]["m2"]
                    detail["m2_SM"] = arms["SM"]["m2"]
                if m2h is None:
                    verdict = "INCOMPLETE"
                    detail["note"] = "headline arm has no m2"
                else:
                    verdict = "GO-band" if m2h >= 1.0 else "SUB"
                    sigma_c4 = arms.get("C4", {}).get("worker_nodes") or 0
                    detail.update({
                        "headline_arm": headline,
                        "short_arm_flag": headline in ("SM", "C1"),
                        "m2_headline": m2h,
                        "r_camp_nodes_per_s":
                            round((m2h or 0) * R_SEQ, 1),
                        "decay": decay,
                        "floor_extension_nodes": max(
                            N_FLOOR_STAGE1, sigma_c4),
                    })

    out["gate"] = {"verdict": verdict, "detail": detail}

    if "C1" in camp and "C4" in camp and camp["C1"].get("m2"):
        out["decay_check"] = {
            "m2_C1": camp["C1"]["m2"], "m2_C4": camp["C4"]["m2"],
            "ratio_C4_over_C1":
                round(camp["C4"]["m2"] / camp["C1"]["m2"], 4),
            "decays": camp["C4"]["m2"] < 0.8 * camp["C1"]["m2"],
        }

    with open(os.path.join(HERE, "analysis.json"), "w") as f:
        json.dump(out, f, indent=1)
    print(json.dumps(out["gate"], indent=1))
    return out


# ---------------------------------- env --------------------------------------


def cmd_env(_args):
    meta = {
        "date": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "host": platform.node(),
        "kernel": platform.release(),
        "python": platform.python_version(),
        "nproc": os.cpu_count(),
        "cpu_max": _read("/sys/fs/cgroup/cpu.max"),
        "memory_max_bytes": int(_read("/sys/fs/cgroup/memory.max") or 0),
        "git_rev": subprocess.run(
            ["git", "rev-parse", "HEAD"], capture_output=True, text=True,
            cwd=REPO,
        ).stdout.strip(),
        "git_dirty": subprocess.run(
            ["git", "status", "--porcelain"], capture_output=True,
            text=True, cwd=REPO,
        ).stdout.strip(),
        "solver_binary_sha256": hashlib.sha256(
            open(SOLVER, "rb").read()).hexdigest()[:16],
        "campaign_master_sha256": hashlib.sha256(
            open(MASTER, "rb").read()).hexdigest()[:16],
        "campaign_worker_sha256": hashlib.sha256(
            open(WORKER, "rb").read()).hexdigest()[:16],
        "root_fen": ROOT_FEN,
        "arms": {k: {"mode": v[0], "budget_secs": v[1], "workers": v[2]}
                 for k, v in ARMS.items()},
        "shape": {"slice": SLICE, "max_slice": MAX_SLICE,
                  "tt_mb": TT_MB, "pt_mb": PT_MB, "nf": False,
                  "abandon": False, "retention": "on"},
    }
    with open(os.path.join(HERE, "env.json"), "w") as f:
        json.dump(meta, f, indent=2)
    print(json.dumps(meta, indent=2))


def cmd_status(_args):
    for tag, (mode, budget, workers) in ARMS.items():
        sp = os.path.join(STATE, f"arm_{tag}.json")
        if os.path.exists(sp):
            r = _load(tag)
            nodes = r.get("worker_nodes") if mode == "campaign" \
                else r.get("nodes")
            print(
                f"arm {tag}: mode={mode} workers={workers} "
                f"nodes={nodes} wall={r['wall_s']}s "
                f"m2={r.get('m2')} kappa={r.get('kappa')} "
                f"exit={r.get('master_exit') or r.get('exit')} "
                f"aborted_rss={r.get('aborted_rss')}"
            )
        else:
            print(f"arm {tag}: not run (mode={mode} budget {budget}s)")
    if _done("CONFw2_rep0"):
        r = _load("CONFw2_rep0")
        print(f"confirm CONFw2_rep0: nodes={r.get('worker_nodes')} "
              f"aborted_rss={r.get('aborted_rss')}")
    if os.path.exists(os.path.join(HERE, "analysis.json")):
        with open(os.path.join(HERE, "analysis.json")) as f:
            a = json.load(f)
        print(f"gate: {a['gate']['verdict']} "
              f"detail={json.dumps(a['gate'].get('detail', {}))}")
    else:
        print("analysis: not run")


def cmd_confirm(_args):
    """Registered INFEASIBLE confirmation: 2 workers, 1800 s (plan8 §3)."""
    tag = "CONFw2_rep0"
    if _done(tag):
        print(f"arm {tag}: already done (state exists)")
        return
    run_campaign_arm(tag, 1800, 2)


def main():
    ensure_dirs()
    cmd = sys.argv[1] if len(sys.argv) > 1 else ""
    if cmd == "env":
        cmd_env(None)
    elif cmd == "arm":
        _run_arm(sys.argv[2].upper())
    elif cmd == "confirm":
        cmd_confirm(None)
    elif cmd == "analyze":
        cmd_analyze(None)
    elif cmd == "status":
        cmd_status(None)
    else:
        raise SystemExit(f"unknown cmd {cmd!r} "
                         "(usage: env|arm <SM|S600|C4|C1>|confirm|"
                         "analyze|status)")


if __name__ == "__main__":
    main()
