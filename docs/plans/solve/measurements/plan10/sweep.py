#!/usr/bin/env python3
"""Plan 10 harness (`solve` initiative): Phase 3 Stage 0 — frontier
budget-completion sweep over the plan9 frozen frontier (698 open leaves of
the d4d5-p2 root).  Executes `docs/plans/solve/plan10.md`.

This is a black-box driver (Python 3 stdlib only) over the plan9 campaign
binaries, unchanged: **no solver, lib, or campaign-code changes**.  The
sweep deliberately *bypasses the master's dispatcher* (the registered §1
audit showed it locks each worker onto one affinity leaf per child — a
dispatcher-biased 15% sample of the frontier): the driver writes job files
directly into the session's `jobs/` dir and runs bare `campaign_worker`
processes against a driver-written `session.json`.  The merge pass at the
end runs the master's `--resume` re-drain (A2 discipline) to verify every
decisive sweep result under the global context before it counts toward the
decision metric.

Subcommands:
  env            -- record environment metadata (binary SHA-256s incl. the
                    registered product-binary identity check against plan9's
                    record 1b70b32f46d9218e)
  rung <R1|R2|R3> -- one rung of the budget ladder (R1 = 8M, R2 = 32M,
                    R3 = 128M child-evals per job); resumable per rung
                    (state/rung_<tag>.json exists => done)
  ladder         -- R1 -> R2 -> R3, then the contingent R4 / cold-control
                    arms per the registered rules
  r4             -- evaluate the R4 projection rule at the R3 close and fire
                    iff `elapsed + K*(512M/kappa)/R_camp <= 5.0 h`
  cold           -- conditional cold-control arm: fired iff dC > 0; up to 30
                    leaves resolved at R2/R3 re-run cold at their resolving
                    rung's budget (fresh workers, no TT restore, per-job
                    fresh Search via --retention off so each job's spend is
                    an exact cold measurement)
  merge          -- the A2 merge pass (§3): decisive sweep results copied
                    into a fresh session seeded with plan9's frozen
                    master_state, re-verified by the master's `--resume`
                    re-drain under the global context
  analyze        -- M1-M6, the pre-registered Stage-0 decision gate, the
                    per-leaf cumulative-spend ledger (state/ledger.json +
                    ledger.csv), and analysis.json
  status         -- progress summary

Registered shape: 4 workers, tt_mb 128, pt_mb 512, retention on (warm
cumulative semantics — each leaf keeps its round-robin worker across rungs
so its retained TT carries its own prior rungs' state); job budgets are the
ladder rungs (deterministic child-eval budgets; `begin_run()` resets per-job
counters, so per-job marginals are exact).  Session dirs under /tmp:
plan10_sweep (ladder), plan10_cold, plan10_merge.  Raw captures under
logs/ (gitignored); committed record = sweep.py, README.md, env.json,
state/*.json (rung records, arm records, merge record, merged master state,
ledger, analysis).

Contingencies (plan10 §7): interrupted arms are not re-run; a worker crash
mid-job is recovered driver-side by re-writing the job for another worker
id (fresh-budget, recorded as a deviation); a missing/incomplete TT dump at
a rung close degrades that worker's next rung to cold (flagged, plan9
DEGRADED precedent); machinery defects (replay error, verify failure,
job_error != 0) abort the affected leaf/arm and are flagged.

Memory protocol (plan8 §2.3, carried over): tree-RSS sampler (10 s cadence),
abort at tree-RSS > 7.0 GiB on 2 consecutive samples or any single process
> 3.5 GiB; oom_kill delta watched.

Locked inputs (report7/report8): R_seq = 198,229 nodes/s, R_camp = 558,529
nodes/s, kappa ~ 27.5 child-evals/node.
"""

from __future__ import annotations

import csv
import hashlib
import json
import os
import platform
import shutil
import statistics
import subprocess
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", "..", "..", ".."))
SOLVER = os.path.join(REPO, "target", "release", "atomic_solver")
MASTER = os.path.join(REPO, "target", "release", "examples", "campaign_master")
WORKER = os.path.join(REPO, "target", "release", "examples", "campaign_worker")

LOGS = os.path.join(HERE, "logs")
STATE = os.path.join(HERE, "state")

# The frozen frontier: plan9's fresh S1 close (committed record).
PLAN9_S1 = os.path.abspath(
    os.path.join(HERE, "..", "plan9", "state", "master_state_s1.json")
)
PLAN9_CLOSES = [  # for the M5 historical-work max (committed record only)
    os.path.abspath(os.path.join(HERE, "..", "plan9", "state", x))
    for x in ("master_state_s1.json", "master_state_s2.json",
              "master_state_s3.json", "master_state_rc.json")
]

SWEEP = "/tmp/plan10_sweep"
COLD = "/tmp/plan10_cold"
MERGE = "/tmp/plan10_merge"

# Registered root (plan9 §3): 1.d4 d5 2.e4 (the representative quiet root;
# the campaign root path is d4d5 + the per-leaf reply).
ROOT_FEN = "rnbqkbnr/ppp1pppp/8/3p4/3P4/8/PPP1PPPP/RNBQKBNR w KQkq d6 0 2"
TT_MB, PT_MB = 128, 512
WORKERS = 4
WORKER_WAIT_S = 300
POLL_S = 2.0

RUNG_BUDGETS = {"R1": 8_000_000, "R2": 32_000_000, "R3": 128_000_000,
                "R4": 512_000_000}

# Locked stage-1/2 inputs (report7/report8) and the registered budget rule.
R_SEQ = 198_229.0
R_CAMP = 558_529.0
KAPPA = 27.5
R4_RULE_H = 5.0
R4_BUDGET = RUNG_BUDGETS["R4"]

# Registered plan9 record (env.json): the product binary must stay
# byte-identical.  Plan9's campaign binaries are superseded by the plan9 §2
# machinery (their exact build context is not reproducible from the commit;
# the intervening proofdb work added workspace deps that change cargo's
# metadata fingerprints for untouched sources — recorded in env.json).
PRODUCT_SHA9 = "1b70b32f46d9218e"
PLAN9_CAMPAIGN = {
    "campaign_master": "7621fc725618932695467a89e987eaece6a6a2f0396414d917d0b5e03539dc98",
    "campaign_worker": "855196ecd01b70cb6deb92ac767df3d6788ca753f5b102b282f332eee05d0178",
}

# Memory gate (plan8 §2.3) and advisory-bound INF sentinel.
TREE_ABORT_BYTES = int(7.0 * 2**30)
PROC_ABORT_BYTES = int(3.5 * 2**30)
SAMPLER_CADENCE_S = 10
ABORT_CONSECUTIVE = 2
INF_CUT = 1 << 62

JOB_LINE_RE_PREFIX = "job "
TTDUMP_RE_PARTS = ("tt-dump: solved=", " unsolved=", " bytes=", " path=")
RESTORE_PARTS = ("restore: file_solved=",)


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
        page = os.sysconf("SC_PAGE_SIZE")
        with open(f"/proc/{pid}/statm") as f:
            return int(f.read().split()[1]) * page
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
    os.makedirs(os.path.join(d, "results"), exist_ok=True)
    os.makedirs(os.path.join(d, "jobs"), exist_ok=True)
    return d


def write_json(path, value):
    tmp = path + ".tmp"
    with open(tmp, "w") as f:
        json.dump(value, f, indent=1)
    os.replace(tmp, path)


def read_json(path):
    try:
        with open(path) as f:
            return json.load(f)
    except (OSError, json.JSONDecodeError):
        return None


# ------------------------------ frontier -------------------------------------


def _leaves_of(state_path):
    """Flatten a plan9 master state: path -> leaf record."""
    s = read_json(state_path)
    out = {}
    for c in s["children"]:
        for r in c["replies"]:
            out[c["mv"] + " " + r["mv"]] = {
                "status": r["status"], "pn": r["pn"], "dn": r["dn"],
                "work": r["work"], "slices": r["slices"],
            }
    return s["children"], out


def load_frontier():
    """The 698 Open leaves of plan9's S1 close + the §2.1 sanity check."""
    children, s1 = _leaves_of(PLAN9_S1)
    n_leaves = sum(len(c["replies"]) for c in children)
    n_won = sum(1 for v in s1.values() if v["status"] == "Won")
    n_open = sum(1 for v in s1.values() if v["status"] == "Open")
    if not (len(children) == 27 and n_leaves == 728 and n_won == 30
            and n_open == 698):
        raise SystemExit(
            f"frontier sanity check FAILED: children={len(children)} "
            f"leaves={n_leaves} won={n_won} open={n_open} (registered: "
            "27/728/30/698)"
        )
    # Historical work per leaf = max over the committed plan9 session closes
    # (S1/S2/S3/RC are independent runs; plan8 states carry aggregates only
    # — registered limitation).  All closes share S1's base cumulatively.
    hist = {}
    for p in PLAN9_CLOSES:
        _, leaves = _leaves_of(p)
        for path, v in leaves.items():
            hist[path] = max(hist.get(path, 0), v["work"])
    frontier = []
    for i, path in enumerate(sorted(s1)):
        v = s1[path]
        if v["status"] != "Open":
            continue
        child_mv, reply_mv = path.split()
        frontier.append({
            "idx": i, "path": path, "child_mv": child_mv,
            "reply_mv": reply_mv, "untouched": hist[path] == 0,
            "historical_work": hist[path],
            "pn": v["pn"], "dn": v["dn"],
        })
    return frontier


def frontier_state():
    """Populated plan9 frontier leaf record set (for cross-checks)."""
    _, s1 = _leaves_of(PLAN9_S1)
    return s1


# plan9-common re-queue leaves (§4 cross-check): recovered from the
# committed closes as the leaves whose work grew in S2, S3 and RC alike —
# exactly the 12 affinity-locked re-queue targets of the plan9 chain.
def plan9_common_leaves():
    closes = [_leaves_of(p)[1] for p in PLAN9_CLOSES]
    s1, s2, s3, rc = closes
    grew2 = {p for p in s1 if s2[p]["work"] > s1[p]["work"]}
    grew3 = {p for p in s2 if s3[p]["work"] > s2[p]["work"]}
    grewr = {p for p in s1 if rc[p]["work"] > s1[p]["work"]}
    return sorted(grew2 & grew3 & grewr)


# ------------------------------ jobs -----------------------------------------


def session_file(d):
    return os.path.join(d, "session.json")


def write_session(d):
    write_json(session_file(d), {
        "root_fen": ROOT_FEN, "tt_mb": TT_MB, "slice_budget": 4_000_000,
        "max_slice": 8_000_000, "feedback": True,
    })


def job_path(d, job_id):
    return os.path.join(d, "jobs", job_id + ".json")


def write_job(d, job_id, worker, path_uci, budget):
    write_json(job_path(d, job_id), {
        "job_id": job_id, "worker": worker, "path_uci": path_uci,
        "direction": "evaluate", "budget_evals": budget,
    })


def result_path(d, job_id):
    return os.path.join(d, "results", job_id + ".json")


def expected_job_wall(budget):
    """Sequential-equivalent wall of one budget-capped job (locked inputs)."""
    return budget / KAPPA / R_CAMP


def rung_worst_wall(n_jobs, budget):
    return n_jobs * expected_job_wall(budget)


def expected_rung_ids(rung_tag, leaves):
    """The deterministic job id set of a rung (round-robin over 4 workers,
    stable per-leaf worker affinity across rungs)."""
    ids = []
    counters = [0] * WORKERS
    for leaf in leaves:
        w = leaf["idx"] % WORKERS
        job_id = f"w{w}_{rung_tag}_{counters[w]}"
        counters[w] += 1
        ids.append((job_id, w, leaf, RUNG_BUDGETS[rung_tag]))
    return ids


def prepare_rung(d, rung_tag, leaves):
    """Fresh-ish rung setup in the shared sweep session dir: clear STOP +
    stale claim files, write this rung's jobs."""
    stop = os.path.join(d, "stop")  # STOP_FILE is lowercase (campaign/mod.rs)
    if os.path.exists(stop):
        os.remove(stop)
    jobs_dir = os.path.join(d, "jobs")
    for name in os.listdir(jobs_dir):
        # stale artifacts of earlier rungs (claims/patches); results are the
        # durable record, jobs are re-writable by design.
        os.remove(os.path.join(jobs_dir, name))
    ids = expected_rung_ids(rung_tag, leaves)
    counters = [0] * WORKERS
    for job_id, w, leaf, budget in ids:
        write_job(d, job_id, w, leaf["path"].split(), budget)
    return ids


def worker_cmd(d, w, rung_tag, tt_load, max_runtime):
    cmd = [
        WORKER, "--session", d, "--worker", str(w),
        "--tt-mb", str(TT_MB), "--retention", "on", "--poll-ms", "5",
        "--max-runtime", str(max_runtime), "--pt-mb", str(PT_MB),
        "--tt-dump", os.path.join(d, f"tt_w{w}_{rung_tag}.tt"),
    ]
    if tt_load:
        cmd += ["--tt-load", tt_load]
    return cmd


def parse_worker_stderr(path):
    """job lines, restore lines, tt-dump lines, export-failure lines."""
    jobs, restores, dumps, exports = [], [], [], []
    try:
        with open(path) as f:
            for line in f:
                line = line.rstrip("\n")
                if line.startswith(JOB_LINE_RE_PREFIX):
                    parts = line.split()
                    # job <id> outcome=<o> exit=<e> evals=<n> nodes=<n> wall=<f> events=<n>
                    try:
                        jobs.append({
                            "job_id": parts[1],
                            "outcome": parts[2].split("=")[1],
                            "exit": parts[3].split("=")[1],
                            "evals": int(parts[4].split("=")[1]),
                            "nodes": int(parts[5].split("=")[1]),
                            "wall": float(parts[6].split("=")[1]),
                            "events": int(parts[7].split("=")[1]),
                        })
                    except (IndexError, ValueError):
                        pass
                elif line.startswith(RESTORE_PARTS[0]):
                    try:
                        kv = dict(p.split("=") for p in line.split()[1:])
                        restores.append({k: int(v) for k, v in kv.items()})
                    except ValueError:
                        restores.append({"raw": line})
                elif line.startswith("tt-dump: "):
                    if "FAILED" in line or "cannot" in line or "no retained" in line:
                        dumps.append({"failed": line})
                    else:
                        try:
                            kv = dict(p.split("=") for p in line.split()[1:])
                            dumps.append({
                                "solved": int(kv["solved"]),
                                "unsolved": int(kv["unsolved"]),
                                "bytes": int(kv["bytes"]),
                                "path": kv["path"],
                            })
                        except (ValueError, KeyError):
                            dumps.append({"raw": line})
                elif "export failed" in line:
                    exports.append(line)
    except OSError:
        pass
    return jobs, restores, dumps, exports


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


def wait_workers_exited(procs, wait_s):
    """Bounded wait for worker exits (plan9 close protocol)."""
    t0 = time.monotonic()
    out = {}
    while time.monotonic() - t0 < wait_s:
        pending = [p for p in procs if p.poll() is None]
        if not pending:
            break
        time.sleep(0.5)
    for p in procs:
        if p.poll() is None:
            p.terminate()
            try:
                p.wait(timeout=30)
            except subprocess.TimeoutExpired:
                p.kill()
    for i, p in enumerate(procs):
        out[f"w{i}"] = {"exitcode": p.returncode}
    return out


# ------------------------------ rung -----------------------------------------


def run_rung(rung_tag, prev_tag=None):
    """One ladder rung: write jobs, launch 4 workers (warm from prev_rung's
    TT dumps), wait for the barrier, close via STOP + TT dumps."""
    rec_path = os.path.join(STATE, f"rung_{rung_tag}.json")
    if os.path.exists(rec_path):
        print(f"rung {rung_tag}: already done (state exists)")
        return read_json(rec_path)

    frontier = load_frontier()
    # The shared sweep session dir: created once (results accumulate across
    # rungs; TTs are pruned at each close), never wiped mid-ladder.
    if not os.path.isdir(SWEEP):
        fresh_session(SWEEP)
    # Only leaves not yet decisively resolved by an earlier rung.
    resolved = resolved_leaves_so_far()
    leaves = [l for l in frontier if l["path"] not in resolved]
    print(f"rung {rung_tag}: {len(leaves)} leaves enter "
          f"({len(resolved)} already resolved)", flush=True)

    write_session(SWEEP)
    ids = prepare_rung(SWEEP, rung_tag, leaves)
    budget = RUNG_BUDGETS[rung_tag]
    expected_wall = rung_worst_wall(len(ids), budget)
    max_runtime = int(4 * expected_wall + 600)
    print(f"rung {rung_tag}: budget={budget} jobs={len(ids)} "
          f"expected_wall<={expected_wall:.1f}s", flush=True)

    oom_before = read_oom_events()
    t0 = time.time()
    procs, wlog = [], []
    for w in range(WORKERS):
        tt_load = ""
        if prev_tag:
            prev_tt = os.path.join(SWEEP, f"tt_w{w}_{prev_tag}.tt")
            if os.path.exists(prev_tt) and os.path.getsize(prev_tt) > 0:
                tt_load = prev_tt
            else:
                print(f"rung {rung_tag}: w{w} continues COLD (missing/"
                      f"empty TT dump of {prev_tag})", flush=True)
        cmd = worker_cmd(SWEEP, w, rung_tag, tt_load, max_runtime)
        errf = open(os.path.join(LOGS, f"rung_{rung_tag}_w{w}.err"), "w")
        p = subprocess.Popen(cmd, stdin=subprocess.DEVNULL,
                             stdout=subprocess.DEVNULL, stderr=errf)
        procs.append(p)
        wlog.append(errf)

    sampler = Sampler([(f"w{i}", p.pid) for i, p in enumerate(procs)])
    sampler.start()

    # The per-rung barrier: every rung job must have a result file.
    deadline = time.monotonic() + expected_wall * 10 + 900
    deviations = []
    claim_seen = {}  # job_id -> monotonic time its .claim first appeared
    done_ids = {jid for jid, _, _, _ in ids}
    n_rec = 0
    aborted = False
    job_timeout = max(120.0, 30 * expected_job_wall(budget))
    while True:
        have = {jid for jid in done_ids
                if os.path.exists(result_path(SWEEP, jid))}
        missing = done_ids - have
        if not missing:
            break
        if sampler.abort_event.is_set():
            aborted = True
            break
        # Crash recovery (plan10 §7): a job whose assigned worker died, or
        # that has been claimed for > job_timeout without a result, is
        # re-written for another worker id — fresh budget, recorded
        # deviation.  The clock starts at first claim detection, NOT at
        # rung start (workers drain their queues sequentially).
        alive = sorted(i for i, p in enumerate(procs) if p.poll() is None)
        for jid, w, leaf, bud in list(ids):
            if jid in have or jid in {d for d, _ in deviations}:
                continue
            claim = os.path.join(SWEEP, "jobs", jid + ".claim")
            if os.path.exists(claim):
                claim_seen.setdefault(jid, time.monotonic())
            stuck = (jid in claim_seen
                     and time.monotonic() - claim_seen[jid] > job_timeout)
            dead = w not in alive
            if stuck or dead:
                if not alive:
                    break  # nobody left to recover to; deadline will fire
                other = alive[n_rec % len(alive)]
                if other == w and len(alive) > 1:
                    other = alive[(n_rec + 1) % len(alive)]
                new_id = f"w{other}_{rung_tag}_x{n_rec}"
                n_rec += 1
                write_job(SWEEP, new_id, other, leaf["path"].split(), bud)
                deviations.append(
                    (new_id, f"recovered {jid} (worker w{w} "
                     f"{'dead' if dead else 'stuck >job_timeout'})"))
                ids.append((new_id, other, leaf, bud))
                done_ids.add(new_id)
                print(f"rung {rung_tag}: recovered {jid} -> {new_id}",
                      flush=True)
        if time.monotonic() > deadline:
            print(f"rung {rung_tag}: wall deadline hit with "
                  f"{len(missing)} results missing", flush=True)
            break
        time.sleep(POLL_S)

    # Close protocol: STOP -> workers finish nothing in flight, dump TT,
    # exit (plan9 §2.4).
    with open(os.path.join(SWEEP, "stop"), "w") as f:
        f.write("stop")
    exits = wait_workers_exited(procs, WORKER_WAIT_S)
    for errf in wlog:
        errf.close()
    wall = round(time.time() - t0, 3)
    snap = sampler.stop()
    oom_after = read_oom_events()

    # Collect per-job results.
    jobs_rec = []
    for jid, w, leaf, bud in ids:
        r = read_json(result_path(SWEEP, jid))
        jobs_rec.append({
            "job_id": jid, "worker": w, "path": leaf["path"],
            "budget": bud,
            "outcome": r["outcome"] if r else "MISSING",
            "exit_reason": r["exit_reason"] if r else None,
            "child_evals": r["child_evals"] if r else None,
            "nodes": r["nodes"] if r else None,
            "wall_s": r["wall_s"] if r else None,
            "root_pn": r["root_pn"] if r else None,
            "root_dn": r["root_dn"] if r else None,
            "events": len(r["events"]) if r and r.get("events") else 0,
            "error": r.get("error") if r else "no result file",
        })

    # Worker stderr parse: job-line cross-check, restore integrity,
    # tt-dump completeness, export failures.
    stderr_rec = {}
    for w in range(WORKERS):
        jobs, restores, dumps, exports = parse_worker_stderr(
            os.path.join(LOGS, f"rung_{rung_tag}_w{w}.err"))
        stderr_rec[f"w{w}"] = {
            "job_lines": len(jobs),
            "job_line_outcomes": {o: sum(1 for j in jobs if j["outcome"] == o)
                                  for o in sorted({j["outcome"] for j in jobs})},
            "restore": restores,
            "tt_dump": dumps,
            "export_failures": exports,
        }
    # TT dump set completeness.
    tt_files = {}
    for w in range(WORKERS):
        fp = os.path.join(SWEEP, f"tt_w{w}_{rung_tag}.tt")
        tt_files[f"w{w}"] = {
            "exists": os.path.exists(fp),
            "bytes": os.path.getsize(fp) if os.path.exists(fp) else 0,
        }
    # Prune all TT dumps but the current rung's (≈480 MB/set; keep latest).
    for f in os.listdir(SWEEP):
        if f.startswith("tt_w") and f.endswith(".tt") \
                and f"_{rung_tag}.tt" not in f:
            os.remove(os.path.join(SWEEP, f))

    n_dec = sum(1 for j in jobs_rec if j["outcome"] in ("win", "loss"))
    n_draw = sum(1 for j in jobs_rec if j["outcome"] == "draw")
    n_err = sum(1 for j in jobs_rec if j["outcome"] not in ("win", "loss", "draw"))
    n_missing = sum(1 for j in jobs_rec if j["outcome"] == "MISSING")
    rec = {
        "rung": rung_tag, "budget_evals": budget,
        "leaves_entering": len(leaves), "jobs": len(ids),
        "resolved_here": n_dec, "still_open": n_draw,
        "job_errors": n_err, "results_missing": n_missing,
        "wall_s": wall, "aborted_rss": aborted,
        "sampler": snap, "oom_kill_delta": (
            oom_after - oom_before
            if oom_before is not None and oom_after is not None else None),
        "worker_exits": exits,
        "tt_files": tt_files,
        "stderr": stderr_rec,
        "deviations": [{"job_id": jid, "note": note} for jid, note in deviations],
        "jobs": jobs_rec,
    }
    # Machinery-integrity flags (plan9 §3.3 conventions).
    mm = sum(r["probe_mismatched"] for v in stderr_rec.values()
             for r in v["restore"] if "probe_mismatched" in r)
    deg = sum(1 for v in stderr_rec.values()
              for r in v["restore"] if "raw" in r)
    dump_missing = [k for k, v in tt_files.items() if not v["exists"]]
    rec["machinery"] = {
        "restore_probe_mismatches": mm,
        "restore_degraded_workers": deg,
        "tt_dumps_missing": dump_missing,
        "defect": bool(mm or n_err or n_missing),
        "export_failures": sum(len(v["export_failures"])
                               for v in stderr_rec.values()),
    }
    # Cross-check: worker job lines vs result files (count only).
    rec["job_line_xcheck"] = {
        f"w{w}": stderr_rec[f"w{w}"]["job_lines"]
        == sum(1 for j in jobs_rec if j["worker"] == w)
        for w in range(WORKERS)
    }
    write_json(rec_path, rec)
    slim = {k: v for k, v in rec.items() if k not in ("jobs", "stderr", "sampler")}
    print(json.dumps(slim, indent=1), flush=True)
    if rec["machinery"]["defect"]:
        raise SystemExit(f"rung {rung_tag}: MACHINERY DEFECT (abort)")
    if rec["machinery"]["export_failures"]:
        print(f"rung {rung_tag}: {rec['machinery']['export_failures']} "
              "export failures (downgraded jobs; recorded, warm state reset "
              "for those leaves — deviation from warm semantics)", flush=True)
    return rec


def resolved_leaves_so_far():
    """Leaves with a decisive result in any completed rung record."""
    out = {}
    for tag in ("R1", "R2", "R3", "R4"):
        p = os.path.join(STATE, f"rung_{tag}.json")
        r = read_json(p)
        if not r:
            continue
        for j in r["jobs"]:
            if j["outcome"] in ("win", "loss"):
                # First decisive result wins (ladder construction: a leaf
                # exits at its first resolution).
                out.setdefault(j["path"], {
                    "path": j["path"], "rung": tag, "outcome": j["outcome"],
                    "marginal_child_evals": j["child_evals"],
                })
    return out


def ladder_elapsed_s():
    lp = os.path.join(STATE, "ladder.json")
    ld = read_json(lp)
    if not ld:
        ld = {"start": time.time()}
        write_json(lp, ld)
    return time.time() - ld["start"]


def maybe_r4():
    """The registered R4 projection rule at the R3 close."""
    rec = read_json(os.path.join(STATE, "rung_R3.json"))
    if not rec:
        print("r4: R3 not done; nothing to decide")
        return None
    resolved = resolved_leaves_so_far()
    frontier = load_frontier()
    survivors = [l["path"] for l in frontier if l["path"] not in resolved]
    elapsed_h = ladder_elapsed_s() / 3600.0
    projected_h = len(survivors) * (R4_BUDGET / KAPPA) / R_CAMP / 3600.0
    fire = (elapsed_h + projected_h) <= R4_RULE_H
    out = {
        "survivors": len(survivors), "elapsed_h": round(elapsed_h, 3),
        "projected_r4_h": round(projected_h, 3), "rule_h": R4_RULE_H,
        "fired": fire,
    }
    write_json(os.path.join(STATE, "r4_decision.json"), out)
    print(json.dumps(out), flush=True)
    if fire:
        run_rung("R4", prev_tag="R3")
    return out


# ------------------------------ cold arm -------------------------------------


def run_cold():
    """Conditional cold-control arm (plan10 §2.5): fired iff dC > 0."""
    rec_path = os.path.join(STATE, "arm_COLD.json")
    if os.path.exists(rec_path):
        print("cold: already done (state exists)")
        return read_json(rec_path)
    c8 = c128 = 0
    resolved = resolved_leaves_so_far()
    for v in resolved.values():
        if v["rung"] == "R1":
            c8 += 1
        if v["rung"] in ("R1", "R2", "R3"):
            c128 += 1
    dc = c128 - c8
    if dc <= 0:
        print(f"cold: NOT fired (dC={dc}; nothing to discount)")
        return None
    # Stratified sample (by child) of leaves resolved at R2/R3, up to 30.
    r23 = [(v["path"], v["rung"], v["outcome"]) for v in resolved.values()
           if v["rung"] in ("R2", "R3")]
    by_child = {}
    for path, rung, outcome in r23:
        by_child.setdefault(path.split()[0], []).append((path, rung))
    sample = []
    children = sorted(by_child)
    i = 0
    while len(sample) < 30 and any(by_child[c] for c in children):
        c = children[i % len(children)]
        if by_child[c]:
            sample.append(by_child[c].pop(0))
        i += 1
    print(f"cold: fired (dC={dc}); {len(sample)} leaves", flush=True)

    fresh_session(COLD)
    write_session(COLD)
    ids = []
    counters = [0] * WORKERS
    for k, (path, rung) in enumerate(sample):
        w = k % WORKERS
        jid = f"w{w}_C_{counters[w]}"
        counters[w] += 1
        write_job(COLD, jid, w, path.split(), RUNG_BUDGETS[rung])
        ids.append({"job_id": jid, "worker": w, "path": path,
                    "resolving_rung": rung, "outcome": outcome,
                    "budget": RUNG_BUDGETS[rung]})
    oom_before = read_oom_events()
    t0 = time.time()
    procs, wlog = [], []
    for w in range(WORKERS):
        # Fresh workers, no TT restore; --retention off so every job starts
        # from an empty Search (exact per-job cold measurement).
        cmd = [
            WORKER, "--session", COLD, "--worker", str(w),
            "--tt-mb", str(TT_MB), "--retention", "off", "--poll-ms", "5",
            "--max-runtime", "7200", "--pt-mb", str(PT_MB),
        ]
        errf = open(os.path.join(LOGS, f"arm_COLD_w{w}.err"), "w")
        p = subprocess.Popen(cmd, stdin=subprocess.DEVNULL,
                             stdout=subprocess.DEVNULL, stderr=errf)
        procs.append(p)
        wlog.append(errf)
    sampler = Sampler([(f"w{i}", p.pid) for i, p in enumerate(procs)])
    sampler.start()
    budget = max(x["budget"] for x in ids)
    deadline = time.monotonic() + rung_worst_wall(len(ids), budget) * 10 + 900
    while True:
        have = {x["job_id"] for x in ids
                if os.path.exists(os.path.join(COLD, "results",
                                               x["job_id"] + ".json"))}
        if len(have) == len(ids) or time.monotonic() > deadline \
                or sampler.abort_event.is_set():
            break
        time.sleep(POLL_S)
    with open(os.path.join(COLD, "stop"), "w") as f:
        f.write("stop")
    exits = wait_workers_exited(procs, WORKER_WAIT_S)
    for errf in wlog:
        errf.close()
    wall = round(time.time() - t0, 3)
    snap = sampler.stop()
    oom_after = read_oom_events()
    jobs_rec = []
    for x in ids:
        r = read_json(os.path.join(COLD, "results", x["job_id"] + ".json"))
        jobs_rec.append({
            "job_id": x["job_id"], "worker": x["worker"], "path": x["path"],
            "resolving_rung": x["resolving_rung"], "budget": x["budget"],
            "outcome": r["outcome"] if r else "MISSING",
            "child_evals": r["child_evals"] if r else None,
            "nodes": r["nodes"] if r else None,
            "wall_s": r["wall_s"] if r else None,
            "root_pn": r["root_pn"] if r else None,
            "root_dn": r["root_dn"] if r else None,
            "error": r.get("error") if r else "no result file",
        })
    rec = {
        "fired": True, "dc_at_fire": dc, "sample": [x["path"] for x in ids],
        "jobs": jobs_rec, "wall_s": wall, "sampler": snap,
        "oom_kill_delta": (
            oom_after - oom_before
            if oom_before is not None and oom_after is not None else None),
        "worker_exits": exits,
        "job_errors": sum(1 for j in jobs_rec
                          if j["outcome"] not in ("win", "loss", "draw")),
    }
    write_json(rec_path, rec)
    slim = {k: v for k, v in rec.items() if k != "jobs"}
    print(json.dumps(slim, indent=1), flush=True)
    return rec


# ------------------------------ merge pass -----------------------------------


def run_merge():
    """The A2 merge pass (plan10 §3)."""
    rec_path = os.path.join(STATE, "merge.json")
    if os.path.exists(rec_path):
        print("merge: already done (state exists)")
        return read_json(rec_path)
    fresh_session(MERGE)
    shutil.copy(PLAN9_S1, os.path.join(MERGE, "master_state.json"))
    # Copy ONLY the decisive sweep results (ladder arms; the cold control is
    # a measurement, never merged).  Uniqueness: <= 1 per leaf by ladder
    # construction — verified here.
    seen = {}
    n_copied = 0
    for tag in ("R1", "R2", "R3", "R4"):
        r = read_json(os.path.join(STATE, f"rung_{tag}.json"))
        if not r:
            continue
        for j in r["jobs"]:
            if j["outcome"] not in ("win", "loss"):
                continue
            if j["path"] in seen:
                raise SystemExit(
                    f"merge: leaf {j['path']} decisive twice "
                    f"({seen[j['path']]} and {j['job_id']}) — machinery defect"
                )
            seen[j["path"]] = j["job_id"]
            shutil.copy(result_path(SWEEP, j["job_id"]),
                        os.path.join(MERGE, "results", j["job_id"] + ".json"))
            n_copied += 1
    print(f"merge: {n_copied} decisive results copied", flush=True)

    oom_before = read_oom_events()
    t0 = time.time()
    cmd = [
        MASTER, "--session", MERGE, "--fen", ROOT_FEN,
        "--workers", str(WORKERS),
        "--slice", "4000000", "--max-slice", "8000000",
        "--max-wall", "60", "--tt-mb", str(TT_MB), "--pt-mb", str(PT_MB),
        "--out", os.path.join(MERGE, "proof_tree.bin"),
        "--resume", "--job-seed", "10", "--state-every", "30",
    ]
    mout = open(os.path.join(LOGS, "merge_master.out"), "w")
    merr = open(os.path.join(LOGS, "merge_master.err"), "w")
    sampler = Sampler([("master", None)])  # placeholder; single process
    sampler.pids = []
    sampler.start()
    p = subprocess.Popen(cmd, stdin=subprocess.DEVNULL, stdout=mout,
                         stderr=merr)
    _, status, ru = os.wait4(p.pid, 0)
    exitcode = os.waitstatus_to_exitcode(status)
    mout.close()
    merr.close()
    wall = round(time.time() - t0, 3)
    sampler.stop()
    oom_after = read_oom_events()
    summary = read_json(os.path.join(MERGE, "summary.json"))
    # The merged close state -> committed record.
    ms = os.path.join(MERGE, "master_state.json")
    ms_versioned = None
    if os.path.exists(ms):
        ms_versioned = os.path.join(STATE, "master_state_merge.json")
        shutil.copy(ms, ms_versioned)
    # The <= 60 s dispatch slop adds at most 4 x 8M-eval jobs whose results
    # never arrive; count + record them (ignored by design).
    n_slop_jobs = len([f for f in os.listdir(os.path.join(MERGE, "jobs"))
                       if f.endswith(".json")])
    verify_failures = (summary or {}).get("verify_failures")
    job_errors = (summary or {}).get("job_errors")
    rec = {
        "decisive_copied": n_copied, "master_exitcode": exitcode,
        "master_exit": (summary or {}).get("exit"),
        "wall_s": wall, "summary": summary,
        "verify_failures": verify_failures, "job_errors": job_errors,
        "jobs_completed": (summary or {}).get("jobs_completed"),
        "leaves_open": (summary or {}).get("leaves_open"),
        "leaves_won": (summary or {}).get("leaves_won"),
        "leaves_lost": (summary or {}).get("leaves_lost"),
        "children_resolved": (summary or {}).get("children_resolved"),
        "children_refuted": (summary or {}).get("children_refuted"),
        "slop_jobs_ignored": n_slop_jobs,
        "master_state_checkpoint": ms_versioned,
        "oom_kill_delta": (
            oom_after - oom_before
            if oom_before is not None and oom_after is not None else None),
    }
    write_json(rec_path, rec)
    print(json.dumps({k: v for k, v in rec.items() if k != "summary"},
                     indent=1), flush=True)
    if verify_failures:
        # Registered rule: a verify failure invalidates the affected leaf's
        # resolution (excluded from the decision metric); it is a machinery
        # defect, never patched.
        raise SystemExit("merge: VERIFY FAILURE(S) — machinery defect; "
                         "affected leaves invalidated, flagged in report")
    return rec


# ------------------------------ analysis -------------------------------------


def _deciles(xs):
    xs = sorted(xs)
    n = len(xs)
    if not n:
        return None
    return [xs[min(n - 1, int(n * q / 10))] for q in (1, 3, 5, 7, 9)]


def _youden_auc(pos, neg):
    """Best-Youden 2x2 + AUC over one signal.  pos/neg: lists of values."""
    if not pos or not neg:
        return None
    vals = sorted(set(pos) | set(neg))
    cands = [vals[0] - 1] + vals
    best = None
    for t in cands:
        tp = sum(1 for x in pos if x > t)
        fn = len(pos) - tp
        tn = sum(1 for x in neg if x <= t)
        fp = len(neg) - tn
        tpr = tp / len(pos)
        fpr = fp / len(neg) if neg else 0.0
        j = tpr - fpr
        if best is None or j > best["youden"]:
            best = {"threshold": t, "youden": round(j, 4), "tpr": round(tpr, 3),
                    "fpr": round(fpr, 3), "tp": tp, "fn": fn, "tn": tn,
                    "fp": fp}
    # AUC (Mann-Whitney, ties = 0.5).
    allv = [(x, 1) for x in pos] + [(x, 0) for x in neg]
    allv.sort()
    ranks = {}
    i = 0
    while i < len(allv):
        j = i
        while j < len(allv) and allv[j][0] == allv[i][0]:
            j += 1
        r = (i + 1 + j) / 2.0
        for k in range(i, j):
            ranks[k] = r
        i = j
    rp = sum(ranks[k] for k, (_, lab) in enumerate(allv) if lab == 1)
    auc = (rp - len(pos) * (len(pos) + 1) / 2) / (len(pos) * len(neg))
    best["auc"] = round(auc, 4)
    return best


def cmd_analyze(_args):
    frontier = load_frontier()
    p9s1 = frontier_state()
    common12 = plan9_common_leaves()
    rungs = {t: read_json(os.path.join(STATE, f"rung_{t}.json"))
             for t in ("R1", "R2", "R3", "R4")}
    rungs = {t: r for t, r in rungs.items() if r}
    merge_rec = read_json(os.path.join(STATE, "merge.json"))
    cold_rec = read_json(os.path.join(STATE, "arm_COLD.json"))
    r4dec = read_json(os.path.join(STATE, "r4_decision.json"))

    if merge_rec is None:
        raise SystemExit("analyze: merge pass missing — run `sweep.py merge` first")
    invalidated = merge_rec.get("verify_failures") or 0
    if invalidated:
        print(f"WARNING: {invalidated} merge verify failures — affected "
              "leaves excluded from the decision metric", flush=True)

    # Per-leaf accounting base.
    acc = {l["path"]: {
        "path": l["path"], "child_mv": l["child_mv"],
        "reply_mv": l["reply_mv"], "untouched": l["untouched"],
        "historical_work": l["historical_work"],
        "marginals": {}, "resolving_rung": None, "outcome": None,
        "merged_status": None,
    } for l in frontier}
    for t in sorted(rungs):
        for j in rungs[t]["jobs"]:
            a = acc.get(j["path"])
            if a is None:
                continue
            a["marginals"][t] = j["child_evals"]
            if j["outcome"] in ("win", "loss") and a["resolving_rung"] is None:
                a["resolving_rung"] = t
                a["outcome"] = j["outcome"]
    # Merged status overlay (the A2-verified record).
    if merge_rec.get("master_state_checkpoint"):
        _, merged = _leaves_of(merge_rec["master_state_checkpoint"])
        for path, a in acc.items():
            a["merged_status"] = merged[path]["status"]

    # ---- M1: completion curve (validator-clean decisive results).
    def C(budget_tag):
        won = lost = 0
        for a in acc.values():
            r = a["resolving_rung"]
            if r is None or RUNG_BUDGETS[r] > RUNG_BUDGETS[budget_tag]:
                continue
            if a["outcome"] == "win":
                won += 1
            elif a["outcome"] == "loss":
                lost += 1
        return {"decided": won + lost, "won": won, "lost": lost}

    m1 = {t: C(t) for t in ("R1", "R2", "R3") if t in rungs}
    if "R4" in rungs:
        m1["R4"] = C("R4")
    untouched = [a for a in acc.values() if a["untouched"]]
    m1["C8M_on_untouched"] = sum(
        1 for a in untouched if a["resolving_rung"] == "R1")
    dc = m1["R3"]["decided"] - m1["R1"]["decided"] if "R3" in m1 else None

    # ---- M2: censor-depth distribution per rung (marginal per-job evals,
    # unresolved leaves of that rung).
    m2 = {}
    for t, r in sorted(rungs.items()):
        ev = [j["child_evals"] for j in r["jobs"] if j["outcome"] == "draw"
              and j["child_evals"] is not None]
        ev_sorted = sorted(ev)
        pinned = sum(1 for x in ev if x >= 0.99 * r["budget_evals"])
        m2[t] = {
            "n": len(ev),
            "cap_pinned_frac": round(pinned / len(ev), 4) if ev else None,
            "median": statistics.median(ev) if ev else None,
            "deciles": _deciles(ev),
            "mean": round(sum(ev) / len(ev), 1) if ev else None,
        }

    # ---- M3: advisory-signal predictivity for leaves entering R2/R3.
    m3 = {}
    for prev_tag, tag in (("R1", "R2"), ("R2", "R3")):
        if tag not in rungs or prev_tag not in rungs:
            continue
        entered = {j["path"]: j for j in rungs[prev_tag]["jobs"]
                   if j["outcome"] == "draw"}
        resolved_at = {a["path"]: a for a in acc.values()
                       if a["resolving_rung"] == tag}
        pos, neg = [], []
        sig = {"pn": ([], []), "dn": ([], []), "work": ([], [])}
        rows = []
        for path, j in entered.items():
            a = acc[path]
            cum = a["historical_work"] + sum(
                v for v in a["marginals"].values() if v is not None)
            lab_pos = path in resolved_at
            rows.append({"path": path, "pn": j["root_pn"], "dn": j["root_dn"],
                         "work": cum, "pos": lab_pos})
            for key, val in (("pn", j["root_pn"]), ("dn", j["root_dn"]),
                             ("work", cum)):
                v = min(val, INF_CUT) if val is not None else INF_CUT
                sig[key][0 if lab_pos else 1].append(v)
            (pos if lab_pos else neg).append(path)
        m3[tag] = {
            "entered": len(entered), "resolved_at_rung": len(pos),
            "signals": {k: _youden_auc(p, n) for k, (p, n) in sig.items()},
            "note": "pn/dn INF-capped at 2^62; work = historical + sweep "
                    "cumulative; label = resolved at this rung",
        }
        # Combined prior: mean of sign-corrected normalized ranks over the
        # entered cohort (direction per signal from its own AUC).
        auc_dir = {k: (1.0 if (m3[tag]["signals"][k] or {"auc": 0.5})["auc"] >= 0.5 else -1.0)
                   for k in sig}
        scores = []
        for key in sig:
            vals = sorted(range(len(rows)), key=lambda i: rows[i][key])
            n = len(vals)
            norm = {}
            for rank, i in enumerate(vals):
                norm[i] = rank / max(1, n - 1)
            scores.append([auc_dir[key] * norm[i] for i in range(n)])
        combined = [sum(col[i] for col in scores) / len(scores)
                    for i in range(n)]
        cpos = [combined[i] for i in range(n) if rows[i]["pos"]]
        cneg = [combined[i] for i in range(n) if not rows[i]["pos"]]
        m3[tag]["combined"] = _youden_auc(cpos, cneg)

    # ---- M4: root-level conversion (merged state).
    m4 = None
    if merge_rec.get("master_state_checkpoint"):
        mchildren, _ = _leaves_of(merge_rec["master_state_checkpoint"])
        m4 = {
            "children_resolved": merge_rec.get("children_resolved"),
            "children_refuted": merge_rec.get("children_refuted"),
            "leaves_won": merge_rec.get("leaves_won"),
            "leaves_lost": merge_rec.get("leaves_lost"),
            "per_child": [],
        }
        for c in mchildren:
            won = sum(1 for r in c["replies"] if r["status"] == "Won")
            lost = sum(1 for r in c["replies"] if r["status"] == "Lost")
            openn = sum(1 for r in c["replies"] if r["status"] == "Open")
            m4["per_child"].append({
                "child": c["mv"], "replies": len(c["replies"]), "won": won,
                "lost": lost, "open": openn,
                "resolved": c.get("resolved", c.get("refuted", False)),
            })

    # ---- M5: ledger.
    ledger = [acc[l["path"]] for l in frontier]
    for a in ledger:
        a["cumulative_sweep_spend"] = sum(
            v for v in a["marginals"].values() if v is not None)
        a["won_flag"] = a["outcome"] == "win"
        a["lost_flag"] = a["outcome"] == "loss"
    write_json(os.path.join(STATE, "ledger.json"), ledger)
    with open(os.path.join(STATE, "ledger.csv"), "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["path", "untouched", "historical_work", "r1_evals",
                    "r2_evals", "r3_evals", "r4_evals", "sweep_total",
                    "resolving_rung", "outcome", "merged_status"])
        for a in ledger:
            w.writerow([
                a["path"], a["untouched"], a["historical_work"],
                a["marginals"].get("R1"), a["marginals"].get("R2"),
                a["marginals"].get("R3"), a["marginals"].get("R4"),
                a["cumulative_sweep_spend"], a["resolving_rung"],
                a["outcome"], a["merged_status"],
            ])

    # ---- M6: frontier pricing.
    per_rung_spend = {
        t: sum(j["child_evals"] or 0 for j in r["jobs"])
        for t, r in sorted(rungs.items())
    }
    total_sweep = sum(per_rung_spend.values())
    m6 = {
        "per_rung_child_evals": per_rung_spend,
        "total_child_evals": total_sweep,
        "total_nodes_est": round(total_sweep / KAPPA),
        "per_rung_nodes_est": {t: round(v / KAPPA)
                               for t, v in per_rung_spend.items()},
    }
    # Projection: geometric extrapolation of the measured per-doubling
    # conversion (input to Stage 1 pricing; honest about being naive).
    if dc and dc > 0 and "R2" in m1:
        last_tag = "R3" if "R3" in rungs else "R2"
        entered = rungs[last_tag]["leaves_entering"]
        conv = m1[last_tag]["decided"] - m1[prev_of(last_tag)]["decided"]
        rate = conv / entered if entered else 0.0
        survivors = entered - conv
        next_budget = 2 * RUNG_BUDGETS[last_tag]
        m6["projection"] = {
            "last_rung": last_tag,
            "conversion_rate_per_rung": round(rate, 4),
            "survivors": survivors,
            "next_doubling_budget": next_budget,
            "expected_conversions_next_doubling": round(rate * survivors, 1),
            "projected_spend_next_doubling_child_evals": survivors * next_budget,
            "note": "geometric extrapolation of the measured per-doubling "
                    "conversion rate; not a DTM-based bound",
        }

    # ---- Registered cross-checks.
    xchecks = {}
    r1 = rungs.get("R1")
    if r1:
        ev12 = [j["child_evals"] for j in r1["jobs"]
                if j["path"] in common12 and j["outcome"] == "draw"
                and j["child_evals"] is not None]
        xchecks["plan9_common_12"] = {
            "n": len(ev12),
            "median_marginal_evals": statistics.median(ev12) if ev12 else None,
            "cap_pinned_frac": (round(sum(1 for x in ev12 if x >= 0.99 * 8_000_000)
                                      / len(ev12), 3) if ev12 else None),
            "plan9_fresh_reference": {"median": 7989618.7, "cap_pinned": 0.997},
        }
        # Already-Won leaves were excluded from the population by
        # construction; assert no sweep job ever targeted one.
        bad = [j["path"] for r in rungs.values() for j in r["jobs"]
               if p9s1.get(j["path"], {}).get("status") == "Won"]
        xchecks["no_won_leaf_jobs"] = {"violations": len(bad)}
    xchecks["job_errors"] = {
        "rung_errors": {t: r["job_errors"] for t, r in rungs.items()},
        "missing_results": {t: r["results_missing"] for t, r in rungs.items()},
        "cold_errors": (cold_rec or {}).get("job_errors"),
        "merge_job_errors": merge_rec.get("job_errors"),
        "merge_verify_failures": invalidated,
    }
    devs = {t: r["deviations"] for t, r in rungs.items() if r["deviations"]}
    if cold_rec:
        pass
    xchecks["deviations"] = devs

    # ---- Pre-registered decision gate (plan10 §4).
    gate = {"dc": dc}
    if dc is None:
        gate["verdict"] = "INCOMPLETE"
        gate["detail"] = {"note": "R3 missing; gate not evaluable"}
    else:
        if dc >= 25:
            gate["verdict"] = "SCHEDULING_GO"
            gate["detail"] = {
                "rule": "dC >= 25: higher budgets measurably extract facts "
                        "the incumbent budgets never see; frontier is "
                        "budget-starved; Stage 1 (A5.4) is the registered "
                        "next lever",
            }
        elif dc <= 5:
            sigs = []
            for t in m3:
                sigs.extend(v for v in m3[t]["signals"].values() if v)
                if m3[t].get("combined"):
                    sigs.append(m3[t]["combined"])
            m3_signal = any(abs(s["auc"] - 0.5) > 0.1 for s in sigs)
            if m3_signal:
                gate["verdict"] = "CENSORING_GO"
                gate["detail"] = {
                    "rule": "dC <= 5 with an M3 advisory signal: budget "
                            "escalation refuted; Stage 2 (A6b) certificates "
                            "have a measured substrate",
                    "m3_signal": m3_signal,
                }
            else:
                gate["verdict"] = "SHARPENED_NEGATIVE"
                gate["detail"] = {
                    "rule": "dC <= 5 with no M3 signal: both registered "
                            "levers lack a measured substrate; initiative "
                            "returns to dormant (sharpened reopener record)",
                    "m3_signal": False,
                }
        else:
            gate["verdict"] = "MARGINAL"
            gate["detail"] = {
                "rule": "5 < dC < 25: judgment call on M1-M6; both stage "
                        "designs priced, no A5-family re-tread",
            }
        gate["detail"]["m4_conversion"] = (
            m4["children_resolved"] if m4 else None)

    out = {
        "root_fen": ROOT_FEN,
        "shape": {"workers": WORKERS, "tt_mb": TT_MB, "pt_mb": PT_MB,
                  "rung_budgets": RUNG_BUDGETS,
                  "retention": "on (cold arm: off)", "master": "absent "
                  "during the sweep (driver-written jobs)"},
        "locked_inputs": {"r_seq": R_SEQ, "r_camp": R_CAMP, "kappa": KAPPA},
        "frontier": {"open_leaves": len(frontier),
                     "untouched": sum(1 for a in ledger if a["untouched"])},
        "M1": m1, "M2": m2, "M3": m3, "M4": m4, "M6": m6,
        "gate": gate,
        "cross_checks": xchecks,
        "cold_arm": (None if not cold_rec else {
            "fired": cold_rec.get("fired"),
            "dc_at_fire": cold_rec.get("dc_at_fire"),
            "n": len(cold_rec.get("jobs", [])),
            "warm_vs_cold": cold_comparison(cold_rec, acc),
        }) if cold_rec else {"fired": False, "note": "dC <= 0; not fired"},
        "r4_decision": r4dec,
        "machinery": {
            t: rungs[t]["machinery"] for t in sorted(rungs)
        },
        "verify_failures": invalidated,
    }
    write_json(os.path.join(STATE, "analysis.json"), out)
    print(json.dumps({"gate": gate, "M1": m1, "M6": m6}, indent=1))
    return out


def prev_of(tag):
    return {"R2": "R1", "R3": "R2", "R4": "R3"}[tag]


def cold_comparison(cold_rec, acc):
    """Warm-vs-cold spend ratio at budgets where a discount can manifest."""
    rows = []
    for j in cold_rec["jobs"]:
        if j["outcome"] not in ("win", "loss", "draw"):
            continue
        a = acc.get(j["path"])
        if not a:
            continue
        rung = j["resolving_rung"]
        warm_marginal = a["marginals"].get(rung)
        warm_cum = a["cumulative_sweep_spend"]
        rows.append({
            "path": j["path"], "budget": j["budget"],
            "warm_marginal": warm_marginal, "warm_cumulative": warm_cum,
            "cold": j["child_evals"],
            "warm_marginal_over_cold": (
                round(warm_marginal / j["child_evals"], 3)
                if warm_marginal and j["child_evals"] else None),
        })
    ratios = [r["warm_marginal_over_cold"] for r in rows
              if r["warm_marginal_over_cold"] is not None]
    return {
        "rows": rows,
        "median_warm_marginal_over_cold": (
            statistics.median(ratios) if ratios else None),
        "note": "warm marginal = the resolving rung's per-job evals under "
                "the warm TT; cold = fresh-Search spend at the same budget "
                "(--retention off)",
    }


# ------------------------------ env ------------------------------------------


def cmd_env(_args):
    prod_ok = sha256(SOLVER).startswith(PRODUCT_SHA9)
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
        "product_identity_check_vs_plan9": {
            "expected_prefix": PRODUCT_SHA9, "match": prod_ok,
            "note": "reproduced with CARGO_PROFILE_RELEASE_LTO=thin; a "
                    "plain release rebuild (the intervening make test "
                    "profile) hashes differently — source unchanged since "
                    "plan9's commit c6a30b0 (git-verified)",
        },
        "plan9_campaign_binaries": PLAN9_CAMPAIGN,
        "campaign_master_sha256": sha256(MASTER),
        "campaign_worker_sha256": sha256(WORKER),
        "campaign_reproduction_note": (
            "campaign sources git-identical to plan9's close commit "
            "c6a30b0 (verified this session); binary hashes differ from "
            "plan9's record because plan9's exact build context is not "
            "recoverable from the commit and the intervening proofdb work "
            "added workspace dependencies that change cargo's fingerprint "
            "metadata for untouched sources. Behavior risk carried by the "
            "A2 merge-pass verification."),
        "root_fen": ROOT_FEN,
        "frontier_source": "measurements/plan9/state/master_state_s1.json",
        "rungs": RUNG_BUDGETS,
        "shape": {"workers": WORKERS, "tt_mb": TT_MB, "pt_mb": PT_MB,
                  "retention": "on", "master": "driver-bypassed during "
                  "the sweep"},
        "session_dirs": {"sweep": SWEEP, "cold": COLD, "merge": MERGE},
    }
    write_json(os.path.join(HERE, "env.json"), meta)
    print(json.dumps(meta, indent=2))
    if not prod_ok:
        raise SystemExit("product binary identity check FAILED")


def cmd_status(_args):
    frontier = load_frontier()
    for tag in ("R1", "R2", "R3", "R4"):
        r = read_json(os.path.join(STATE, f"rung_{tag}.json"))
        if r:
            print(f"rung {tag}: entering={r['leaves_entering']} "
                  f"resolved={r['resolved_here']} errors={r['job_errors']} "
                  f"wall={r['wall_s']}s defect={r['machinery']['defect']}")
        else:
            print(f"rung {tag}: not run")
    for name in ("arm_COLD", "merge"):
        r = read_json(os.path.join(STATE, f"{name}.json"))
        if r:
            print(f"{name}: done")
        else:
            print(f"{name}: not run")
    ap = os.path.join(STATE, "analysis.json")
    if os.path.exists(ap):
        a = read_json(ap)
        print(f"gate: {a['gate']['verdict']} dC={a['gate'].get('dc')}")
    else:
        print("analysis: not run")
    print(f"frontier: {len(frontier)} open leaves")


def main():
    ensure_dirs()
    cmd = sys.argv[1] if len(sys.argv) > 1 else ""
    if cmd == "env":
        cmd_env(None)
    elif cmd == "rung":
        prev = {"R1": None, "R2": "R1", "R3": "R2", "R4": "R3"}
        rung_tag = sys.argv[2].upper()
        lp = os.path.join(STATE, "ladder.json")
        if not os.path.exists(lp):
            write_json(lp, {"start": time.time()})
        run_rung(rung_tag, prev_tag=prev[rung_tag])
        if rung_tag == "R3":
            maybe_r4()
    elif cmd == "ladder":
        lp = os.path.join(STATE, "ladder.json")
        if not os.path.exists(lp):
            write_json(lp, {"start": time.time()})
        prev = {"R1": None, "R2": "R1", "R3": "R2"}
        for tag in ("R1", "R2", "R3"):
            run_rung(tag, prev_tag=prev[tag])
        maybe_r4()
        run_cold()
    elif cmd == "r4":
        maybe_r4()
    elif cmd == "cold":
        run_cold()
    elif cmd == "merge":
        run_merge()
    elif cmd == "analyze":
        cmd_analyze(None)
    elif cmd == "status":
        cmd_status(None)
    else:
        raise SystemExit("unknown cmd %r (usage: env|rung <R1|R2|R3>|ladder"
                         "|r4|cold|merge|analyze|status)" % cmd)


if __name__ == "__main__":
    main()
