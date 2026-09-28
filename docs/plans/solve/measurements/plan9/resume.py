#!/usr/bin/env python3
"""Plan 9 harness (`solve` initiative): multi-session checkpoint-resume
accumulation on the d4d5-p2 root (item 8, stage 3 of 3).  Black-box driver
over the release binaries (Python 3 stdlib only); product path untouched.

Subcommands:
  env              -- record environment metadata to env.json (binary
                      SHA-256s incl. the registered product-binary identity
                      check against plan8's record 1b70b32f46d9218e)
  smoke            -- SMOKE arm: 5 min fresh -> checkpoint -> 5 min
                      warm-resume; machinery validation only, NOT a
                      datapoint (a driver defect is fixed and SMOKE re-run)
  arm <NAME>       -- one registered arm, strictly sequential, resumable
                      per arm (state/arm_<tag>.json exists => done):
               S1   : fresh 3600 s session (seed 0) + checkpoint at close
                      (the chain's base; fresh-session control on the new
                      binaries)
               S2   : warm resume from S1 (state v2 + 4 worker TTs), 3600 s
               S3   : warm resume from S2, 3600 s (second boundary)
               RC   : COLD resume from S1 in a copy of S1's session dir
                      (fresh workers, no TT restore), 3600 s -- attribution
                      control isolating the worker-TT-restore contribution
  analyze          -- rho per arm, plan7 §4 verdict bands with the plan9 §4
                      degenerate rules, failure-mode attribution via the
                      work-to-censor substrate metric.  Writes analysis.json
  status           -- print progress summary

Registered shape (plan8's incumbent V1, unchanged): master `--slice
4000000 --max-slice 8000000`, feedback on, retention on, no --abandon,
4 workers, `--tt-mb 128 --pt-mb 512`.  Session dirs under /tmp/plan9_*:
the S1->S2->S3 chain shares one session dir (durable results re-drain);
RC runs in a copy filtered to S1's seed-0 results.

Checkpoint close protocol (plan9 §2.4): the master writes STOP, workers
finish their in-flight job, dump their TT (--tt-dump), exit; the driver
waits bounded (300 s) for worker exits + the complete TT file set, then
versions the checkpoint artifacts (master_state_s<k>.json into state/,
worker TTs stay under /tmp with versioned names).

Memory protocol (plan8 §2.3, carried over): tree-RSS sampler (10 s
cadence), abort at tree-RSS > 7.0 GiB on 2 consecutive samples or any
single process > 3.5 GiB; oom_kill delta watched; plan9 abort rule
throughout.

Marginal accounting (plan9 §2, registered rule): all report metrics are
marginals by subtraction of prior session closes; the in-flight
stragglers of session k-1 drain at the start of session k and count
toward session k.

Locked inputs: R_seq = 198,229 nodes/s (report7), m2 = 2.8176 (report8),
plan8 C1 fresh-hour harvest = 30 facts (denominator corroboration).
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
CHAIN = "/tmp/plan9_chain"
SMOKE_DIR = "/tmp/plan9_smoke"
RC_DIR = "/tmp/plan9_rc"

# Registered root and shape (plan9 §3, plan8 V1 incumbent).
ROOT_FEN = "rnbqkbnr/ppp1pppp/8/3p4/3P4/8/PPP1PPPP/RNBQKBNR w KQkq d6 0 2"
SLICE, MAX_SLICE = 4_000_000, 8_000_000
TT_MB, PT_MB = 128, 512
WORKERS = 4
BUDGET_S = 3600
SMOKE_BUDGET_S = 300
WORKER_WAIT_S = 300

# Registered plan8 record (env.json): the product binary must stay
# byte-identical; the campaign binaries are superseded by the plan9
# machinery (expected, recorded).
PRODUCT_SHA8 = "1b70b32f46d9218e"
PRE_PLAN9_CAMPAIGN = {
    "campaign_master": "91c2c50bdce425c6c6349d1e6c9e01b904617891c067928eb594358fa530596e",
    "campaign_worker": "f9a1eb81c522630273ac3d5e757716bc1d2b639f272eeee6f3fc757d3a11da00",
}

# Locked stage-1/2 inputs (report7/report8).
R_SEQ = 198_229.0
M2_LOCKED = 2.8176
PLAN8_C1_FACTS = 30

# Registered memory gate (plan8 §2.3) and the restore-integrity wait.
TREE_ABORT_BYTES = int(7.0 * 2**30)
PROC_ABORT_BYTES = int(3.5 * 2**30)
SAMPLER_CADENCE_S = 10
ABORT_CONSECUTIVE = 2
WATCHDOG_MARGIN = 600

PAGE = os.sysconf("SC_PAGE_SIZE")
JOB_RE = re.compile(
    r"^job (\S+) outcome=(\S+) exit=(\S+) evals=(\d+) nodes=(\d+)"
    r" wall=([0-9.]+) events=(\d+)"
)
RESTORE_RE = re.compile(
    r"^restore: file_solved=(\d+) file_unsolved=(\d+) "
    r"table_solved=(\d+) table_unsolved=(\d+) "
    r"probe_checked=(\d+) probe_mismatched=(\d+)"
)
RESTORE_DEGRADED_RE = re.compile(r"^restore: DEGRADED path=(\S+) reason=(.*)$")
TTDUMP_RE = re.compile(
    r"^tt-dump: solved=(\d+) unsolved=(\d+) bytes=(\d+) path=(\S+)$"
)
RESUMED_RE = re.compile(r"^master: resumed state: (\d+) leaves, (\d+) synthesized")


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


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def fresh_session(d):
    shutil.rmtree(d, ignore_errors=True)
    os.makedirs(d)
    return d


class Sampler:
    """Tree-RSS sampler with the registered abort rule (plan8 §2.3)."""

    def __init__(self, pids):
        self.pids = list(pids)
        self.samples = 0
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
        self.abort_event.set()
        return {
            "aborted": self.aborted,
            "vanished": list(self.vanished),
            "peak_total_bytes": self.peak_total,
            "peak_single_bytes": dict(self.peak_single),
            "samples_taken": self.samples,
        }

    def _loop(self):
        while not self.abort_event.is_set():
            total = 0
            per = {}
            missing = []
            for name, pid in self.pids:
                r = rss_of(pid)
                if r is None:
                    missing.append(name)
                    continue
                per[name] = r
                total += r
                if r > self.peak_single.get(name, 0):
                    self.peak_single[name] = r
            self.samples += 1
            if total > self.peak_total:
                self.peak_total = total
            for name in missing:
                if name not in self.vanished:
                    self.vanished.append(name)
            if total > TREE_ABORT_BYTES:
                self._over += 1
            else:
                self._over = 0
            if self._over >= ABORT_CONSECUTIVE or any(
                r > PROC_ABORT_BYTES for r in per.values()
            ):
                self.aborted = True
                self.abort_event.set()
                return
            time.sleep(SAMPLER_CADENCE_S)


def _parse_err(path):
    """Parse a worker stderr capture: job lines + restore + tt-dump lines."""
    jobs, restores, dumps, degraded = [], [], [], []
    try:
        with open(path) as f:
            for line in f:
                line = line.rstrip("\n")
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
                    continue
                m = RESTORE_RE.match(line)
                if m:
                    restores.append({
                        "file_solved": int(m.group(1)),
                        "file_unsolved": int(m.group(2)),
                        "table_solved": int(m.group(3)),
                        "table_unsolved": int(m.group(4)),
                        "probe_checked": int(m.group(5)),
                        "probe_mismatched": int(m.group(6)),
                    })
                    continue
                m = RESTORE_DEGRADED_RE.match(line)
                if m:
                    degraded.append({"path": m.group(1), "reason": m.group(2)})
                    continue
                m = TTDUMP_RE.match(line)
                if m:
                    dumps.append({
                        "solved": int(m.group(1)),
                        "unsolved": int(m.group(2)),
                        "bytes": int(m.group(3)),
                        "path": m.group(4),
                    })
    except OSError:
        pass
    return jobs, restores, dumps, degraded


def _sum_results(session_dir, seed=None):
    """Sum durable per-job result JSONs, optionally filtered to a seed."""
    rdir = os.path.join(session_dir, "results")
    nodes = evals = n = 0
    per_leaf = {}
    if os.path.isdir(rdir):
        for name in sorted(os.listdir(rdir)):
            if not name.endswith(".json"):
                continue
            if seed is not None:
                parts = name[: -len(".json")].split("_")
                if len(parts) < 3 or parts[1] != str(seed):
                    continue
            try:
                with open(os.path.join(rdir, name)) as f:
                    r = json.load(f)
                nodes += r.get("nodes", 0)
                evals += r.get("child_evals", 0)
                n += 1
                leaf = " ".join(r.get("path_uci", []))
                d = per_leaf.setdefault(leaf, {"evals": [], "dns": []})
                d["evals"].append(r.get("child_evals", 0))
                if r.get("root_dn"):
                    d["dns"].append(r.get("root_dn", 0))
            except (OSError, json.JSONDecodeError, KeyError, TypeError):
                pass
    return {
        "results_files": n,
        "results_nodes": nodes,
        "results_child_evals": evals,
        "per_leaf": per_leaf,
    }


def _read_summary(session_dir):
    sp = os.path.join(session_dir, "summary.json")
    if os.path.exists(sp):
        try:
            with open(sp) as f:
                return json.load(f)
        except json.JSONDecodeError:
            return None
    return None


def run_session(tag, session_dir, label, maxwall, resume, job_seed,
                tt_load_tpl, tt_dump_tpl):
    """One master+workers session with the full close protocol.

    label: session index within the arm (e.g. "s0"/"s1", or the arm tag);
           used for log names and checkpoint versioning.
    tt_load_tpl / tt_dump_tpl: worker --tt-load/--tt-dump path templates
           with `{w}` substituted; empty string = flag omitted.
    """
    oom_before = read_oom_events()
    master_cmd = [
        MASTER, "--session", session_dir, "--fen", ROOT_FEN,
        "--workers", str(WORKERS),
        "--slice", str(SLICE), "--max-slice", str(MAX_SLICE),
        "--max-wall", str(maxwall), "--tt-mb", str(TT_MB),
        "--pt-mb", str(PT_MB), "--out",
        os.path.join(session_dir, "proof_tree.bin"),
    ]
    if resume:
        master_cmd += ["--resume", "--job-seed", str(job_seed)]
    mout = os.path.join(LOGS, f"arm_{tag}_{label}.out")
    merr = os.path.join(LOGS, f"arm_{tag}_{label}.err")
    master_out = open(mout, "w")
    master_err = open(merr, "w")
    t0 = time.time()
    master = subprocess.Popen(
        master_cmd, stdin=subprocess.DEVNULL, stdout=master_out,
        stderr=master_err,
    )
    wrecs = []
    for i in range(WORKERS):
        cmd = [
            WORKER, "--session", session_dir, "--worker", str(i),
            "--tt-mb", str(TT_MB), "--retention", "on",
        ]
        if tt_load_tpl:
            cmd += ["--tt-load", tt_load_tpl.format(w=i)]
        if tt_dump_tpl:
            cmd += ["--tt-dump", tt_dump_tpl.format(w=i)]
        werr = open(os.path.join(LOGS, f"arm_{tag}_{label}_w{i}.err"), "w")
        p = subprocess.Popen(
            cmd, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
            stderr=werr,
        )
        wrecs.append((f"w{i}", p, werr))

    sampler = Sampler([("master", master.pid)] +
                      [(n, p.pid) for n, p, _ in wrecs])
    sampler.start()

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
        try:
            _, m_status, m_ru = os.wait4(master.pid, 0)
        except ChildProcessError:
            pass
    master_out.close()
    master_err.close()
    wall_master = round(time.time() - t0, 3)

    # Checkpoint close protocol (plan9 §2.4): workers observe STOP, finish
    # their in-flight job, dump their TT, and exit on their own; the driver
    # waits bounded (300 s) for exits + the complete TT file set.
    worker_rec = {}
    t_wait0 = time.monotonic()
    for name, p, werr in wrecs:
        werr.flush()
        rc, ru = None, None
        exited = False
        while time.monotonic() - t_wait0 < WORKER_WAIT_S:
            try:
                pid_, status, wru = os.wait4(p.pid, os.WNOHANG)
            except ChildProcessError:
                exited = True
                rc = p.returncode
                break
            if pid_ == p.pid:
                exited = True
                rc = os.waitstatus_to_exitcode(status)
                ru = wru
                break
            time.sleep(0.5)
        if not exited:
            try:
                p.terminate()
            except ProcessLookupError:
                pass
            try:
                _, status, wru = os.wait4(p.pid, 0)
                rc = os.waitstatus_to_exitcode(status)
                ru = wru
            except (ChildProcessError, ProcessLookupError):
                rc = -1
        p.returncode = rc if rc is not None else -1
        worker_rec[name] = {
            "exitcode": rc,
            "ru_maxrss_kb": ru.ru_maxrss if ru else None,
        }
    wall_total = round(time.time() - t0, 3)
    snap = sampler.stop()

    oom_after = read_oom_events()

    # Parse worker stderr + master stderr.
    jobs_all = []
    restores = {}
    dumps = {}
    degraded = {}
    for name, _, _ in wrecs:
        jobs, res, dmp, deg = _parse_err(
            os.path.join(LOGS, f"arm_{tag}_{label}_{name}.err")
        )
        jobs_all.extend(jobs)
        restores[name] = res
        dumps[name] = dmp
        degraded[name] = deg
    resumed_state = None
    try:
        with open(merr) as f:
            for line in f:
                m = RESUMED_RE.match(line.rstrip("\n"))
                if m:
                    resumed_state = {
                        "leaves": int(m.group(1)),
                        "synthesized_children": int(m.group(2)),
                    }
    except OSError:
        pass

    # TT file set completeness (checkpoint integrity).
    tt_files = {}
    if tt_dump_tpl:
        for i in range(WORKERS):
            fp = tt_dump_tpl.format(w=i)
            tt_files[os.path.basename(fp)] = {
                "exists": os.path.exists(fp),
                "bytes": os.path.getsize(fp) if os.path.exists(fp) else 0,
            }

    # Version the master state checkpoint into the committed state dir.
    ms_src = os.path.join(session_dir, "master_state.json")
    ms_versioned = None
    if os.path.exists(ms_src):
        ms_versioned = os.path.join(STATE, f"master_state_{label}.json")
        shutil.copy(ms_src, ms_versioned)

    summary = _read_summary(session_dir)
    xcheck = _sum_results(session_dir, seed=job_seed)
    s_nodes = (summary or {}).get("worker_nodes")
    s_evals = (summary or {}).get("worker_child_evals")
    nodes = s_nodes if s_nodes is not None else xcheck["results_nodes"]
    evals = s_evals if s_evals is not None else xcheck["results_child_evals"]
    m_exitcode = (os.waitstatus_to_exitcode(m_status)
                  if m_status is not None else None)

    rec = {
        "tag": tag,
        "session_label": label,
        "session_dir": session_dir,
        "resumed": resume,
        "job_seed": job_seed,
        "workers": WORKERS,
        "budget_secs": maxwall,
        "wall_master_s": wall_master,
        "wall_total_s": wall_total,
        "aborted_rss": aborted,
        "sampler": snap,
        "oom_kill_delta": (
            oom_after - oom_before
            if oom_before is not None and oom_after is not None else None
        ),
        "master_exitcode": m_exitcode,
        "master_exit": (summary or {}).get("exit"),
        "master_ru_maxrss_kb": m_ru.ru_maxrss if m_ru else None,
        "worker_rec": worker_rec,
        "summary": summary,
        "resumed_state": resumed_state,
        "restore": restores,
        "restore_degraded": degraded,
        "tt_dump_lines": dumps,
        "tt_files": tt_files,
        "master_state_checkpoint": ms_versioned,
        "worker_nodes": nodes,
        "child_evals": evals,
        "cross_check_seed": {
            k: v for k, v in xcheck.items() if k != "per_leaf"
        },
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
        "children_refuted": (summary or {}).get("children_refuted"),
        "leaves_open": (summary or {}).get("leaves_open"),
        "leaves_won": (summary or {}).get("leaves_won"),
        "leaves_lost": (summary or {}).get("leaves_lost"),
        "outcome": (summary or {}).get("outcome"),
        "job_count": len(jobs_all),
        "job_outcomes": {
            o: sum(1 for j in jobs_all if j["outcome"] == o)
            for o in sorted({j["outcome"] for j in jobs_all})
        },
        "jobs_per_h": round(len(jobs_all) * 3600.0 / max(wall_total, 1), 1),
    }
    if wall_total > 0 and nodes:
        rec["nodes_per_s"] = round(nodes / wall_total, 1)
        rec["m2"] = round(rec["nodes_per_s"] / R_SEQ, 4)
    if nodes and evals:
        rec["kappa"] = round(evals / nodes, 2)

    # facts at close (marginal accounting is analyze's job).
    rec["facts_close"] = (
        (rec["leaves_won"] or 0) + (rec["leaves_lost"] or 0)
        + (rec["children_resolved"] or 0)
    )

    # COMPLETED branch (plan9 §4): validate the composed artifact.
    if rec["master_exit"] == "proven":
        pt = os.path.join(session_dir, "proof_tree.bin")
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
            rec["combined_wall_s"] = round(wall_total + vwall, 3)
            rec["artifact"] = validate == "ok"
            shutil.copy(pt, os.path.join(STATE, f"{tag}_proof_tree.bin"))

    rec["infeasible_signal"] = bool(
        snap["vanished"]
        or rec["master_exit"] == "verify_failed"
        or (rec["oom_kill_delta"] or 0) > 0
    )

    # Machinery-integrity flags (plan9 §3.3/§3.5): a probe mismatch or a
    # verify failure on re-drain is a resume-machinery defect => abort the
    # dependent arm, flag, do not re-run within the cap.
    mm = sum(r["probe_mismatched"] for rs in restores.values() for r in rs)
    deg_n = sum(len(v) for v in degraded.values())
    tt_missing = [k for k, v in tt_files.items() if not v["exists"]] \
        if tt_files else []
    rec["machinery"] = {
        "probe_mismatches": mm,
        "restore_degraded_workers": deg_n,
        "tt_files_missing": tt_missing,
        "defect": bool(mm or rec["verify_failures"] or deg_n),
    }

    # Worker job lines are part of the state record (plan6 convention).
    rec["worker_job_lines"] = {
        name: [
            f"{j['job_id']} outcome={j['outcome']} exit={j['exit']} "
            f"evals={j['evals']} nodes={j['nodes']} wall={j['wall_s']} "
            f"events={j['events']}"
            for j in _parse_err(
                os.path.join(LOGS, f"arm_{tag}_{label}_{name}.err")
            )[0]
        ]
        for name, _, _ in wrecs
    }

    with open(os.path.join(STATE, f"arm_{tag}.json"), "w") as f:
        json.dump(rec, f, indent=1)
    slim = {
        k: v for k, v in rec.items()
        if k not in ("worker_job_lines", "sampler", "summary", "restore",
                     "tt_dump_lines")
    }
    print(json.dumps(slim))
    return rec


def make_rc_session_dir():
    """Copy S1's checkpoint into RC's own session dir: session.json +
    seed-0 results + the versioned S1 master state as master_state.json."""
    fresh_session(RC_DIR)
    os.makedirs(os.path.join(RC_DIR, "results"), exist_ok=True)
    os.makedirs(os.path.join(RC_DIR, "jobs"), exist_ok=True)
    shutil.copy(os.path.join(CHAIN, "session.json"),
                os.path.join(RC_DIR, "session.json"))
    src = os.path.join(CHAIN, "results")
    n = 0
    if os.path.isdir(src):
        for name in sorted(os.listdir(src)):
            if not name.endswith(".json"):
                continue
            parts = name[: -len(".json")].split("_")
            if len(parts) >= 3 and parts[1] == "0":
                shutil.copy(os.path.join(src, name),
                            os.path.join(RC_DIR, "results", name))
                n += 1
    shutil.copy(os.path.join(STATE, "master_state_s1.json"),
                os.path.join(RC_DIR, "master_state.json"))
    return n


def _run_arm(tag):
    if os.path.exists(os.path.join(STATE, f"arm_{tag}.json")):
        print(f"arm {tag}: already done (state exists)")
        return
    print(f"arm {tag}: starting ...", flush=True)
    if tag == "S1":
        fresh_session(CHAIN)
        run_session("S1", CHAIN, "s1", BUDGET_S, resume=False, job_seed=0,
                    tt_load_tpl="", tt_dump_tpl=os.path.join(
                        CHAIN, "worker{w}_s1.tt"))
    elif tag == "S2":
        run_session("S2", CHAIN, "s2", BUDGET_S, resume=True, job_seed=1,
                    tt_load_tpl=os.path.join(CHAIN, "worker{w}_s1.tt"),
                    tt_dump_tpl=os.path.join(CHAIN, "worker{w}_s2.tt"))
    elif tag == "S3":
        run_session("S3", CHAIN, "s3", BUDGET_S, resume=True, job_seed=2,
                    tt_load_tpl=os.path.join(CHAIN, "worker{w}_s2.tt"),
                    tt_dump_tpl=os.path.join(CHAIN, "worker{w}_s3.tt"))
    elif tag == "RC":
        n = make_rc_session_dir()
        print(f"RC session dir prepared: {n} seed-0 result files copied")
        run_session("RC", RC_DIR, "rc", BUDGET_S, resume=True, job_seed=9,
                    tt_load_tpl="", tt_dump_tpl="")
    else:
        raise SystemExit(f"unknown arm {tag}")


def run_smoke():
    if os.path.exists(os.path.join(STATE, "arm_SMOKE.json")):
        print("smoke: already done (state exists)")
        return
    fresh_session(SMOKE_DIR)
    r0 = run_session("SMOKE", SMOKE_DIR, "smoke_s0", SMOKE_BUDGET_S,
                     resume=False, job_seed=0, tt_load_tpl="",
                     tt_dump_tpl=os.path.join(SMOKE_DIR, "worker{w}_s0.tt"))
    r1 = run_session("SMOKE", SMOKE_DIR, "smoke_s1", SMOKE_BUDGET_S,
                     resume=True, job_seed=1,
                     tt_load_tpl=os.path.join(SMOKE_DIR, "worker{w}_s0.tt"),
                     tt_dump_tpl=os.path.join(SMOKE_DIR, "worker{w}_s1.tt"))
    checks = smoke_checks(r0, r1, SMOKE_DIR)
    with open(os.path.join(STATE, "arm_SMOKE.json"), "w") as f:
        json.dump({"s0": r0, "s1": r1, "checks": checks}, f, indent=1)
    print(json.dumps(checks, indent=1))
    if not all(checks.values()):
        raise SystemExit("SMOKE failed checks (machinery defect)")


def smoke_checks(r0, r1, session_dir):
    c = {}
    # v2 fields present in the versioned master state.
    msp = r0.get("master_state_checkpoint")
    try:
        with open(msp) as f:
            ms = json.load(f)
        leaf = ms["children"][0]["replies"][0]
        c["state_v2_fields"] = (
            ms.get("version") == 2
            and "depth" in leaf and "last_worker" in leaf
            and "depth" in ms["children"][0]
            and "synthesized" in ms["children"][0]
        )
    except (OSError, KeyError, TypeError, IndexError):
        c["state_v2_fields"] = False
    # Complete TT file set at the first close.
    c["tt_files_complete"] = (
        len(r0["tt_files"]) == WORKERS
        and all(v["exists"] and v["bytes"] > 0
                for v in r0["tt_files"].values())
    )
    # Every worker restored something and probed clean.
    try:
        c["restore_integrity"] = all(
            len(rs) == 1
            and rs[0]["probe_mismatched"] == 0
            and rs[0]["file_solved"] > 0
            and rs[0]["table_solved"] == rs[0]["file_solved"]
            for rs in r1["restore"].values()
        ) and len(r1["restore"]) == WORKERS
    except (KeyError, TypeError):
        c["restore_integrity"] = False
    # Re-drain: the resumed master processed at least the prior close's
    # durable results (jobs_completed counts every drained result).
    c["redrain"] = (
        (r1.get("jobs_completed") or 0)
        >= (r0.get("jobs_completed") or 0)
        and r1.get("resumed_state") is not None
    )
    # Soundness counters must be zero on re-drain.
    c["clean_counters"] = (
        r1.get("verify_failures") == 0 and r1.get("job_errors") == 0
        and r0.get("verify_failures") == 0 and r0.get("job_errors") == 0
    )
    # No memory incident, no degraded worker.
    c["no_incidents"] = (
        not r0["infeasible_signal"] and not r1["infeasible_signal"]
        and r0["machinery"]["restore_degraded_workers"] == 0
        and r1["machinery"]["restore_degraded_workers"] == 0
    )
    # Facts are monotone across the boundary (checkpoint carried them).
    c["facts_monotone"] = (r1.get("facts_close") or 0) >= (
        r0.get("facts_close") or 0
    )
    return c


# -------------------------------- analysis -----------------------------------


def _load(tag):
    with open(os.path.join(STATE, f"arm_{tag}.json")) as f:
        return json.load(f)


def _done(tag):
    return os.path.exists(os.path.join(STATE, f"arm_{tag}.json"))


def _substrate(chain_seed_stats, rc_seed_stats):
    """Work-to-censor discount (plan9 §3.4): mean/median per-job child_evals
    and the advisory root_dn per leaf, restricted to the re-queued leaves
    (present in BOTH the fresh S1 session and the warm session), so the
    fresh baseline is not diluted by the early cheap class.  INF-scale
    root_dn values (>= 2^62) are excluded from the dn aggregates; the
    cap-pinned fraction (evals >= 0.99 * max_slice) is reported because
    both sides pin at the 8M budget, which bounds the metric's resolution.
    """
    CAP = MAX_SLICE
    INF_CUT = 1 << 62

    def agg(per_leaf, only_leaves=None):
        evals, dns, pinned = [], [], 0
        for leaf, d in per_leaf.items():
            if only_leaves is not None and leaf not in only_leaves:
                continue
            evals.extend(d["evals"])
            pinned += sum(1 for e in d["evals"] if e >= 0.99 * CAP)
            good = [x for x in d["dns"] if x < INF_CUT]
            if good:
                dns.append(max(good))
        evals.sort()
        dns.sort()
        n = len(evals)
        return {
            "jobs": n,
            "cap_pinned_frac": round(pinned / n, 3) if n else None,
            "mean_child_evals": round(sum(evals) / n, 1) if n else None,
            "median_child_evals": evals[n // 2] if n else None,
            "leaves_with_dn": len(dns),
            "median_final_root_dn": dns[len(dns) // 2] if dns else None,
        }

    fresh_all = chain_seed_stats.get("0", {}).get("per_leaf", {})
    out = {}
    common = set(fresh_all)
    for seed, tag in (("1", "S2"), ("2", "S3")):
        warm = chain_seed_stats.get(seed, {}).get("per_leaf", {})
        common &= set(warm)
    if "9" in rc_seed_stats:
        common &= set(rc_seed_stats["9"].get("per_leaf", {}))
    out["requeued_leaves"] = len(common)
    out["fresh_s1_requeued"] = agg(fresh_all, only_leaves=common)
    out["fresh_s1_all"] = agg(fresh_all)
    for seed, tag in (("1", "S2"), ("2", "S3")):
        warm = chain_seed_stats.get(seed, {}).get("per_leaf", {})
        out[f"warm_{tag}"] = agg(warm, only_leaves=common)
    if "9" in rc_seed_stats:
        warm = rc_seed_stats["9"].get("per_leaf", {})
        out["warm_RC"] = agg(warm, only_leaves=common)
    return out


def cmd_analyze(_args):
    arms = {t: _load(t) for t in ("S1", "S2", "S3", "RC") if _done(t)}
    chain_seed_stats = {
        seed: _sum_results(CHAIN, seed=seed) for seed in ("0", "1", "2")
    } if os.path.isdir(CHAIN) else {}
    rc_seed_stats = {"9": _sum_results(RC_DIR, seed=9)} \
        if os.path.isdir(RC_DIR) else {}
    substrate = _substrate(chain_seed_stats, rc_seed_stats)

    out = {
        "root_fen": ROOT_FEN,
        "shape": {"slice": SLICE, "max_slice": MAX_SLICE, "tt_mb": TT_MB,
                  "pt_mb": PT_MB, "feedback": True, "retention": True,
                  "abandon": False, "workers": WORKERS},
        "locked_inputs": {"r_seq": R_SEQ, "m2": M2_LOCKED,
                          "plan8_c1_facts": PLAN8_C1_FACTS},
        "gate_constants": {"positive_rho": 0.7, "negative_rho": 0.4,
                           "tree_abort_gib": 7.0, "proc_abort_gib": 3.5},
        "arms": {},
        "substrate": substrate,
    }
    for t, r in arms.items():
        row = {
            "resumed": r["resumed"], "job_seed": r["job_seed"],
            "budget_secs": r["budget_secs"], "wall_total_s": r["wall_total_s"],
            "worker_nodes": r["worker_nodes"], "m2": r.get("m2"),
            "kappa": r.get("kappa"),
            "master_exit": r["master_exit"],
            "jobs_completed": r["jobs_completed"],
            "jobs_dispatched": r["jobs_dispatched"],
            "job_errors": r["job_errors"],
            "verify_failures": r["verify_failures"],
            "leaves_open": r["leaves_open"], "leaves_won": r["leaves_won"],
            "leaves_lost": r["leaves_lost"],
            "children_resolved": r["children_resolved"],
            "facts_close": r["facts_close"],
            "machinery": r["machinery"],
            "infeasible_signal": r["infeasible_signal"],
            "restore_degraded_workers":
                r["machinery"]["restore_degraded_workers"],
            "peak_tree_rss_bytes": r["sampler"]["peak_total_bytes"],
            "oom_kill_delta": r["oom_kill_delta"],
            "outcome": r["outcome"],
            "artifact": r.get("artifact"),
            "inspect_validate": r.get("inspect_validate"),
            "combined_wall_s": r.get("combined_wall_s"),
        }
        if t == "S1":
            row["restore_probe_mismatches"] = None
        else:
            row["restore_probe_mismatches"] = sum(
                x["probe_mismatched"]
                for lst in r["restore"].values() for x in lst
            ) if r.get("restore") else None
        out["arms"][t] = row

    # Marginal work accounting (plan9 §2 registered rule): a resumed
    # session's summary counters include the re-drained prior results, so
    # the session-marginal nodes/evals are the differences of closes.
    prev = None
    for t in ("S1", "S2", "S3", "RC"):
        if t not in arms:
            continue
        if t == "RC":
            base = arms["S1"]["worker_nodes"]
            base_e = arms["S1"]["child_evals"]
        elif prev is not None:
            base, base_e = arms[prev]["worker_nodes"], arms[prev]["child_evals"]
        else:
            base = base_e = 0
        out["arms"][t]["marginal_nodes"] = max(
            (arms[t]["worker_nodes"] or 0) - base, 0)
        out["arms"][t]["marginal_child_evals"] = max(
            (arms[t]["child_evals"] or 0) - base_e, 0)
        prev = t

    # COMPLETED gate (plan9 §4).
    completed = [t for t, r in arms.items() if r.get("master_exit") == "proven"]
    if completed:
        t = completed[0]
        out["gate"] = {
            "verdict": "COMPLETED",
            "detail": {"completed_arm": t,
                       "artifact": arms[t].get("artifact"),
                       "validate": arms[t].get("inspect_validate"),
                       "combined_wall_s": arms[t].get("combined_wall_s")},
        }
        _write_analysis(out)
        return out

    # rho per plan9 §3.2.
    deltas = {}
    if "S1" in arms:
        d1 = arms["S1"]["facts_close"]
        deltas["S1"] = d1
        for t, prev in (("S2", "S1"), ("S3", "S2"), ("RC", "S1")):
            if t in arms:
                deltas[t] = arms[t]["facts_close"] - arms[prev]["facts_close"]
    out["delta_facts"] = deltas

    verdict = None
    detail = {}
    if "S1" not in arms:
        verdict = "INCOMPLETE"
        detail = {"note": "S1 missing"}
    elif deltas["S1"] == 0:
        # Degenerate rule 1 (plan9 §4): halt-and-investigate, no verdict.
        verdict = "ANOMALY_DELTA_S1_ZERO"
        detail = {
            "rule": "plan9 §4 degenerate rule 1",
            "note": "contradicts plan8's on-record 30; protocol or "
                    "environment anomaly; verdict deferred",
        }
    else:
        d1 = deltas["S1"]
        rho2 = deltas.get("S2", 0) / d1 if "S2" in arms else None
        rho3 = deltas.get("S3", 0) / d1 if "S3" in arms else None
        rho_cold = deltas.get("RC", 0) / d1 if "RC" in arms else None
        if rho2 is not None and rho3 is not None:
            headline = (deltas.get("S2", 0) + deltas.get("S3", 0)) / (2 * d1)
        elif rho2 is not None:
            headline = rho2
            detail["short_arm_flag"] = "S3 dropped (plan9 §7 contingency)"
        else:
            headline = rho_cold
            detail["short_arm_flag"] = "verdict on rho_cold only"
        out["rho"] = {"rho_2": rho2, "rho_3": rho3,
                      "rho_cold": rho_cold,
                      "headline": round(headline, 4)
                      if headline is not None else None}
        if headline >= 0.7:
            verdict = "POSITIVE"
        elif headline < 0.4:
            verdict = "NEGATIVE"
        else:
            verdict = "MARGINAL"
        detail.update({"headline_rho": round(headline, 4),
                       "m2_locked": M2_LOCKED})
        if verdict == "NEGATIVE":
            # Failure-mode attribution via the substrate metric (plan9 §3.4):
            # warm ≈ fresh work-to-censor => resume-mechanics; warm discount
            # present with Δfacts ≈ 0 => frozen-frontier.  The ratio uses
            # the requeued-leaf-restricted, budget-capped medians; the
            # cap-pinned fractions qualify the reading (both sides pin at
            # the 8M budget).
            mode = "frozen_frontier"
            fresh_v = substrate.get("fresh_s1_requeued", {}).get(
                "median_child_evals")
            warm_vals = [
                v["median_child_evals"] for k, v in substrate.items()
                if k.startswith("warm_") and v.get("median_child_evals")
            ]
            if warm_vals and fresh_v:
                ratio = min(warm_vals) / fresh_v
                out["substrate"]["warm_over_fresh_min"] = round(ratio, 3)
                if ratio > 0.9:
                    mode = "resume_mechanics"
            out["failure_mode"] = mode
            detail["failure_mode"] = mode

    out["gate"] = {"verdict": verdict, "detail": detail}
    _write_analysis(out)
    print(json.dumps(out["gate"], indent=1))
    return out


def _write_analysis(out):
    with open(os.path.join(HERE, "analysis.json"), "w") as f:
        json.dump(out, f, indent=1)


# ---------------------------------- env --------------------------------------


def cmd_env(_args):
    product_ok = sha256(SOLVER).startswith(PRODUCT_SHA8)
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
        "product_binary_sha256": sha256(SOLVER),
        "product_identity_check_vs_plan8": {
            "expected_prefix": PRODUCT_SHA8, "match": product_ok,
        },
        "pre_plan9_campaign_binaries": PRE_PLAN9_CAMPAIGN,
        "campaign_master_sha256": sha256(MASTER),
        "campaign_worker_sha256": sha256(WORKER),
        "root_fen": ROOT_FEN,
        "arms": ["SMOKE", "S1", "S2", "S3", "RC"],
        "shape": {"slice": SLICE, "max_slice": MAX_SLICE, "tt_mb": TT_MB,
                  "pt_mb": PT_MB, "nf": False, "abandon": False,
                  "retention": "on", "workers": WORKERS,
                  "budget_s": BUDGET_S},
        "session_dirs": {"chain": CHAIN, "smoke": SMOKE_DIR, "rc": RC_DIR},
    }
    with open(os.path.join(HERE, "env.json"), "w") as f:
        json.dump(meta, f, indent=2)
    print(json.dumps(meta, indent=2))
    if not product_ok:
        raise SystemExit("product binary identity check FAILED")


def cmd_status(_args):
    for tag in ("SMOKE", "S1", "S2", "S3", "RC"):
        sp = os.path.join(STATE, f"arm_{tag}.json")
        if os.path.exists(sp):
            r = _load(tag)
            if tag == "SMOKE":
                checks = r.get("checks", {})
                print(f"smoke: checks={'PASS' if all(checks.values()) else 'FAIL'}"
                      f" {checks}")
            else:
                print(
                    f"arm {tag}: resumed={r['resumed']} seed={r['job_seed']} "
                    f"facts_close={r['facts_close']} "
                    f"(won={r['leaves_won']} lost={r['leaves_lost']} "
                    f"children={r['children_resolved']}) "
                    f"nodes={r['worker_nodes']} m2={r.get('m2')} "
                    f"exit={r['master_exit']}"
                )
        else:
            print(f"arm {tag}: not run")
    ap = os.path.join(HERE, "analysis.json")
    if os.path.exists(ap):
        with open(ap) as f:
            a = json.load(f)
        print(f"gate: {a['gate']['verdict']} "
              f"rho={a.get('rho')} deltas={a.get('delta_facts')}")
    else:
        print("analysis: not run")


def main():
    ensure_dirs()
    cmd = sys.argv[1] if len(sys.argv) > 1 else ""
    if cmd == "env":
        cmd_env(None)
    elif cmd == "smoke":
        run_smoke()
    elif cmd == "arm":
        _run_arm(sys.argv[2].upper())
    elif cmd == "analyze":
        cmd_analyze(None)
    elif cmd == "status":
        cmd_status(None)
    else:
        raise SystemExit("unknown cmd %r (usage: env|smoke|arm <S1|S2|S3|RC>"
                         "|analyze|status)" % cmd)


if __name__ == "__main__":
    main()
