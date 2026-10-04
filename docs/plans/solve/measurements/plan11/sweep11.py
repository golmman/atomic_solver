#!/usr/bin/env python3
"""Plan 11 harness (`solve` initiative): Phase 3 Stage 2a — advisory-dn
censor-signal validation at larger n over a stratified sample of the frozen
frontier's censored body.  Executes `docs/plans/solve/plan11.md`.

Black-box driver (Python 3 stdlib only) over the plan9 campaign binaries,
unchanged: **no solver, lib, or campaign-code changes**.  Same conventions
as plan10's sweep.py: driver-written job files, bare `campaign_worker`
processes (master bypassed), per-rung close = barrier -> lowercase `stop`
file -> bounded wait for exits + complete TT-dump set, warm relaunch with
`--tt-load`, tree-RSS sampler, oom watch, crash recovery.  A final merge
pass (`campaign_master --resume`, --job-seed 11) re-verifies every decisive
result under the global context (A2 discipline) over plan10's committed
merged state.

Subcommands:
  env             -- record env.json (binary SHA-256s incl. the registered
                     product-binary identity check vs plan9's 1b70b32f46d9218e)
  strata          -- build state/strata.json (deterministic registered
                     strata over the plan10 merged close's 660 Open leaves);
                     must be committed before the ladder runs
  smoke           -- SMOKE: 8 leaves x 100k evals, one rung, FULL
                     close/restore protocol (fresh phase + warm restore
                     phase); not a datapoint
  rung <R1|R2|R3> -- one rung of the ladder (R1 = 256M fresh, R2 = 1G warm
                     on R1 survivors, R3 = 2G warm on R2 survivors,
                     contingent); resumable per rung
  ladder          -- SMOKE-guarded R1 -> R2 -> R3-decision (contingent rule
                     recorded either way)
  r3              -- evaluate the contingent R3 rule at the R2 close and
                     fire iff `elapsed_h + survivors*2G/kappa/R_camp <= 4.5 h`
  merge           -- A2 merge pass over all decisive results, seeded with
                     plan10's merged master state
  analyze         -- M2 metrics (per-rung labels, covariates, AUC/Youden,
                     theta, S(theta), directional inversions, held-out FNs),
                     the pre-registered M2 gate verdict, the M2 corpus CSV,
                     ledger11.json (plan10 ledger extended)
  status          -- progress summary

Registered shape (plan11 §2): sampling frame = the 660 Open leaves of
plan10's committed merged close (`../plan10/state/master_state_merge.json`,
sanity-checked 728/68/660/27).  Covariates: committed advisory (pn, dn,
work) from plan9's frozen S1 close + plan10's ledger cumulative spend.
Strata: H = 24 (top committed-dn, ties -> higher work, force-including the
c1g5 near-misses f7f6/g8f6), M = 12 (touched, ~55th-75th pct of the touched
set's committed dn), L = 12 (touched, ~15th-35th pct), U = 8 (untouched
control, round-robin over children by static rank).  Primary corpus =
H U M U L (48); U analyzed separately, excluded from the primary gate.

Ladder: R1' = 256M (fresh workers, no --tt-load), R2' = 1G (warm from R1'
dumps), contingent R3' = 2G (warm from R2' dumps).  Workers = 4, stable
per-leaf worker affinity across rungs, round-robin job naming
(`w{w}_{rung}_{n}`).  Contingent rule (fixed before any run): fire R3' iff
`elapsed + survivors * 2G / kappa / R_camp <= 4.5 h` with kappa = 27.5 and
R_camp = 558,529 nodes/s (the locked plan8 constants); decision recorded in
state/r3_decision.json either way.

Covariate regime matching (plan11 §4, binding): R1' covariate = committed
advisory dn from plan9's frozen S1 close (fresh regime; secondary).  R2'/R3'
covariate = the prior rung's post-run advisory dn (the prior rung's job
result root_dn; warm regime; PRIMARY — it matches the certificate
consumption regime).  Cumulative work is registered as a control covariate
(plan10 M3 measured AUC 0.000 — it must not separate; if it does, the
covariate pipeline is defective and the gate verdict is void until the
defect is fixed and the affected rung re-analyzed).

Registered analysis readings (fixed before any run; documented in README):
  - percentile rank of a covariate value x in a cohort C:
        pct(x, C) = (#c in C with c < x + 0.5 * #c in C with c == x) / |C|
  - "directional inversion" (plan11 §4): a positive whose regime-matching
    pre-rung advisory dn sits in stratum L's registered percentile range
    (or below it) — operationalized scale-free across regimes as
    pct(pre_dn, rung entrant cohort) <= 0.35 (L's band upper edge).
  - "pooled warm-regime AUC by rank-attachment": each warm rung's pre-rung
    covariates are mapped to within-cohort percentile ranks; ranks are
    pooled across warm rungs; AUC on the pooled ranks.
  - held-out false negative: a positive in a held-out rung whose
    regime-matching pre-rung dn <= theta.
  - gate-band gaps (P >= 3, warm AUC >= 0.80, clean direction, but
    S(theta) < 0.40; and P >= 3, warm AUC in [0.60, 0.80), <= 1 inversion,
    0 held-out FNs) are registered as MARGINAL — the GO band's full
    conjunction is not met and NO-GO's disjunction is not fired either.

Session dirs under /tmp: plan11_sweep (ladder), plan11_smoke, plan11_merge.
TT dumps kept only for the latest rung, deleted at close.  Raw captures
under logs/ (gitignored); committed record = sweep11.py, README.md,
env.json, state/*.json (strata, rung records, smoke, r3_decision, merge,
merged master state, ledger11, analysis).
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

# Sampling frame: plan10's committed merged close (the post-Stage-0 record).
PLAN10_MERGE = os.path.abspath(
    os.path.join(HERE, "..", "plan10", "state", "master_state_merge.json")
)
PLAN10_LEDGER = os.path.abspath(
    os.path.join(HERE, "..", "plan10", "state", "ledger.json")
)
# Committed advisory covariates: plan9's frozen S1 close (the fresh-regime
# covariate source; identical to the plan10 merged state's open-leaf
# advisory — verified by the strata builder).
PLAN9_S1 = os.path.abspath(
    os.path.join(HERE, "..", "plan9", "state", "master_state_s1.json")
)

SWEEP = "/tmp/plan11_sweep"
SMOKE = "/tmp/plan11_smoke"
MERGE = "/tmp/plan11_merge"

# Registered root (plan9 §3): 1.d4 d5 2.e4 (the representative quiet root).
ROOT_FEN = "rnbqkbnr/ppp1pppp/8/3p4/3P4/8/PPP1PPPP/RNBQKBNR w KQkq d6 0 2"
TT_MB, PT_MB = 128, 512
WORKERS = 4
WORKER_WAIT_S = 300
POLL_S = 2.0

RUNG_BUDGETS = {"R1": 256_000_000, "R2": 1_000_000_000, "R3": 2_000_000_000}
SMOKE_BUDGET = 100_000

# Locked stage-1/2 inputs (report7/report8) and the registered contingent
# rule — projections use the LOCKED values (the plan10 R4-not-fired lesson).
R_CAMP = 558_529.0
KAPPA = 27.5
R3_RULE_H = 4.5
R3_BUDGET = RUNG_BUDGETS["R3"]

# Registered plan9 record (env.json): the product binary must stay
# byte-identical.  Campaign binary hashes differ from plan9's record (plan9's
# exact build context not reproducible from the commit; plan10 §9.2) —
# behavior risk carried by the A2 merge-pass verification.
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

# The c1g5 near-misses (plan10 §5): force-included in stratum H.
NEAR_MISSES = ("c1g5 f7f6", "c1g5 g8f6")

JOB_LINE_RE_PREFIX = "job "
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
    """Flatten a plan9-lineage master state: path -> leaf record."""
    s = read_json(state_path)
    out = {}
    for c in s["children"]:
        for r in c["replies"]:
            out[c["mv"] + " " + r["mv"]] = {
                "status": r["status"], "pn": r["pn"], "dn": r["dn"],
                "work": r["work"], "slices": r["slices"],
            }
    return s, out


def load_frame():
    """The sampling frame: the 660 Open leaves of plan10's merged close,
    with the registered covariates, plus the registered sanity check."""
    _, merged = _leaves_of(PLAN10_MERGE)
    _, s1 = _leaves_of(PLAN9_S1)
    ledger = {r["path"]: r for r in read_json(PLAN10_LEDGER)}
    n_leaves = sum(1 for v in merged.values())
    n_won = sum(1 for v in merged.values() if v["status"] == "Won")
    n_open = sum(1 for v in merged.values() if v["status"] == "Open")
    n_children = len(read_json(PLAN10_MERGE)["children"])
    if not (n_children == 27 and n_leaves == 728 and n_won == 68
            and n_open == 660):
        raise SystemExit(
            f"frame sanity check FAILED: children={n_children} "
            f"leaves={n_leaves} won={n_won} open={n_open} (registered: "
            "27/728/68/660)"
        )
    # The committed advisory of every open leaf must be identical in the
    # plan10 merged state and plan9's frozen S1 close (the re-drain never
    # touches open leaves) — otherwise "committed advisory dn" is ambiguous.
    diverged = [p for p in merged
                if merged[p]["status"] == "Open"
                and (merged[p]["dn"], merged[p]["pn"], merged[p]["work"])
                != (s1[p]["dn"], s1[p]["pn"], s1[p]["work"])]
    if diverged:
        raise SystemExit(
            f"committed-advisory identity check FAILED for {len(diverged)} "
            "open leaves (plan10 merge state vs plan9 S1 close)"
        )
    frame = []
    for i, path in enumerate(sorted(p for p in merged
                                    if merged[p]["status"] == "Open")):
        v = merged[path]
        frame.append({
            "idx": i,  # stable affinity/naming index within the open set
            "path": path,
            "child_mv": path.split()[0], "reply_mv": path.split()[1],
            "committed_pn": v["pn"], "committed_dn": v["dn"],
            "committed_work": v["work"],
            "touched": v["work"] > 0,
            "plan10_spend": ledger[path]["cumulative_sweep_spend"],
        })
    return frame


# ------------------------------ strata ---------------------------------------

def _band_picks(n, lo_pct, hi_pct, k):
    """Evenly spaced rank picks inside the [lo_pct, hi_pct] percentile band
    of a sorted list of n items (deterministic; registered reading)."""
    lo = int(lo_pct / 100.0 * (n - 1))
    hi = int(hi_pct / 100.0 * (n - 1))
    if hi - lo + 1 < k:
        raise SystemExit(f"band [{lo_pct},{hi_pct}] of n={n} too narrow for k={k}")
    return [lo + round((hi - lo) * j / (k - 1)) for j in range(k)]


def build_strata():
    """The registered strata (plan11 §2) over the 660-open-leaf frame."""
    frame = load_frame()
    touched = sorted((l for l in frame if l["touched"]),
                     key=lambda l: (l["committed_dn"], l["committed_work"],
                                    l["path"]))
    untouched = [l for l in frame if not l["touched"]]

    # H: top-24 open leaves by committed advisory dn (ties -> higher work),
    # force-including the two c1g5 near-misses.
    h_sorted = sorted(frame,
                      key=lambda l: (-l["committed_dn"], -l["committed_work"],
                                     l["path"]))
    h = h_sorted[:24]
    forced = []
    for nm in NEAR_MISSES:
        leaf = next(l for l in frame if l["path"] == nm)
        if leaf not in h:
            forced.append(leaf)
            h = h[:-1]  # drop the smallest (dn, work) of the top-24
    h = h + forced
    h.sort(key=lambda l: (-l["committed_dn"], -l["committed_work"], l["path"]))

    # M / L: touched-open leaves at the registered percentile bands of the
    # touched set's committed dn.
    m_idx = _band_picks(len(touched), 55, 75, 12)
    l_idx = _band_picks(len(touched), 15, 35, 12)
    m = [touched[i] for i in m_idx]
    low = [touched[i] for i in l_idx]

    # U: untouched open leaves, round-robin over children by static rank.
    children = read_json(PLAN10_MERGE)["children"]
    rank = {c["mv"]: (c["static_rank"], ci) for ci, c in
            enumerate(sorted(children, key=lambda c: c["mv"]))}
    per_child = {}
    for l in untouched:
        per_child.setdefault(l["child_mv"], []).append(l)
    child_order = sorted(per_child,
                         key=lambda c: (rank[c][0], rank[c][1]))
    queues = {c: sorted(per_child[c], key=lambda l: l["path"])
              for c in child_order}
    u, ci = [], 0
    while len(u) < 8 and any(queues[c] for c in child_order):
        c = child_order[ci % len(child_order)]
        if queues[c]:
            u.append(queues[c].pop(0))
        ci += 1

    strata = {
        "frame": {
            "source": "measurements/plan10/state/master_state_merge.json",
            "open_leaves": len(frame), "touched": len(touched),
            "untouched": len(untouched),
            "covariates": "committed advisory (pn,dn,work) from plan9's "
                          "frozen S1 close (identity-checked vs the plan10 "
                          "merged state) + plan10 ledger cumulative spend",
        },
        "selection_rules": {
            "H": "top-24 open leaves by committed advisory dn (ties -> "
                 "higher work), force-including c1g5 f7f6 and c1g5 g8f6",
            "M": "touched-open leaves at the ~55th-75th percentile of the "
                 "touched set's committed dn (sorted dn asc, work asc, path); "
                 f"band ranks {min(m_idx)}..{max(m_idx)} of "
                 f"{len(touched)}, 12 evenly spaced",
            "L": "touched-open leaves at the ~15th-35th percentile of the "
                 "touched set's committed dn (same ordering); "
                 f"band ranks {min(l_idx)}..{max(l_idx)} of "
                 f"{len(touched)}, 12 evenly spaced",
            "U": "untouched open leaves (committed work = 0), round-robin "
                 "over children by static rank",
        },
        "forced_into_H": [l["path"] for l in forced],
        "H": [l["path"] for l in h],
        "M": [l["path"] for l in m],
        "L": [l["path"] for l in low],
        "U": [l["path"] for l in u],
        "leaves": {l["path"]: {
            "idx": l["idx"], "child_mv": l["child_mv"],
            "reply_mv": l["reply_mv"], "committed_pn": l["committed_pn"],
            "committed_dn": l["committed_dn"],
            "committed_work": l["committed_work"],
            "touched": l["touched"], "plan10_spend": l["plan10_spend"],
        } for l in h + m + low + u},
    }
    # The near-misses must be present exactly once.
    assert len(strata["H"]) == 24 and len(strata["M"]) == 12 \
        and len(strata["L"]) == 12 and len(strata["U"]) == 8
    assert len(set(strata["H"]) | set(strata["M"]) | set(strata["L"])
               | set(strata["U"])) == 56, "strata must be disjoint"
    for nm in NEAR_MISSES:
        assert nm in strata["H"], f"{nm} not force-included"
    write_json(os.path.join(STATE, "strata.json"), strata)
    print(json.dumps({k: v for k, v in strata.items()
                      if k not in ("leaves",)}, indent=1))
    print(f"strata: committed (H={len(strata['H'])}, M={len(strata['M'])}, "
          f"L={len(strata['L'])}, U={len(strata['U'])})")
    return strata


def load_strata():
    p = os.path.join(STATE, "strata.json")
    s = read_json(p)
    if not s:
        raise SystemExit("strata.json missing — run `sweep11.py strata` and "
                         "commit it before the ladder runs")
    return s


# ------------------------------ jobs -----------------------------------------


def session_file(d):
    return os.path.join(d, "session.json")


def write_session(d):
    write_json(session_file(d), {
        "root_fen": ROOT_FEN, "tt_mb": TT_MB, "slice_budget": 4_000_000,
        "max_slice": 8_000_000, "feedback": True,
    })


def write_job(d, job_id, worker, path_uci, budget):
    write_json(os.path.join(d, "jobs", job_id + ".json"), {
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


def affinity(leaf_idx):
    return leaf_idx % WORKERS


def prepare_rung(d, rung_tag, leaves, budgets=None):
    """Rung setup in the shared session dir: clear stop + stale job files,
    write this rung's jobs (round-robin naming, stable per-leaf affinity)."""
    stop = os.path.join(d, "stop")  # STOP_FILE is lowercase (campaign/mod.rs)
    if os.path.exists(stop):
        os.remove(stop)
    jobs_dir = os.path.join(d, "jobs")
    for name in os.listdir(jobs_dir):
        os.remove(os.path.join(jobs_dir, name))
    ids = []
    counters = [0] * WORKERS
    for leaf in sorted(leaves, key=lambda l: l["path"]):
        w = affinity(leaf["idx"])
        job_id = f"w{w}_{rung_tag}_{counters[w]}"
        counters[w] += 1
        default_budget = (budgets or {}).get("*", RUNG_BUDGETS.get(rung_tag))
        budget = (budgets or {}).get(leaf["path"], default_budget)
        write_job(d, job_id, w, leaf["path"].split(), budget)
        ids.append((job_id, w, leaf, budget))
    return ids


def worker_cmd(d, w, tag, tt_load, max_runtime, dump=True):
    cmd = [
        WORKER, "--session", d, "--worker", str(w),
        "--tt-mb", str(TT_MB), "--retention", "on", "--poll-ms", "5",
        "--max-runtime", str(max_runtime), "--pt-mb", str(PT_MB),
    ]
    if dump:
        cmd += ["--tt-dump", os.path.join(d, f"tt_w{w}_{tag}.tt")]
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


def sampled_leaves():
    """The 56 sampled leaves (strata order), with frame indices."""
    strata = load_strata()
    out = []
    for s in ("H", "M", "L", "U"):
        for path in strata[s]:
            lv = strata["leaves"][path]
            out.append({
                "idx": lv["idx"], "path": path,
                "child_mv": lv["child_mv"], "reply_mv": lv["reply_mv"],
                "stratum": s,
                "committed_pn": lv["committed_pn"],
                "committed_dn": lv["committed_dn"],
                "committed_work": lv["committed_work"],
                "touched": lv["touched"], "plan10_spend": lv["plan10_spend"],
            })
    return out


def resolved_leaves_so_far(tags=("R1", "R2", "R3")):
    """Leaves with a decisive result in any completed rung record."""
    out = {}
    for tag in tags:
        r = read_json(os.path.join(STATE, f"rung_{tag}.json"))
        if not r:
            continue
        for j in r["jobs"]:
            if j["outcome"] in ("win", "loss"):
                out.setdefault(j["path"], {
                    "path": j["path"], "rung": tag, "outcome": j["outcome"],
                    "marginal_child_evals": j["child_evals"],
                })
    return out


def run_rung(rung_tag, prev_tag=None):
    """One ladder rung: write jobs, launch 4 workers (warm from prev_rung's
    TT dumps), wait for the barrier, close via stop + TT dumps."""
    rec_path = os.path.join(STATE, f"rung_{rung_tag}.json")
    if os.path.exists(rec_path):
        print(f"rung {rung_tag}: already done (state exists)")
        return read_json(rec_path)
    if not os.path.exists(os.path.join(STATE, "strata.json")):
        raise SystemExit("rung: strata.json missing (commit it first)")

    leaves = sampled_leaves()
    if not os.path.isdir(SWEEP):
        fresh_session(SWEEP)
    prior_tags = {"R1": (), "R2": ("R1",), "R3": ("R1", "R2")}[rung_tag]
    resolved = resolved_leaves_so_far(prior_tags)
    ents = [l for l in leaves if l["path"] not in resolved]
    print(f"rung {rung_tag}: {len(ents)} leaves enter "
          f"({len(resolved)} already resolved)", flush=True)

    write_session(SWEEP)
    ids = prepare_rung(SWEEP, rung_tag, ents)
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

    # Close protocol: stop -> workers dump TT, exit (plan9 §2.4; the file
    # is lowercase — report10 §9.3 lesson).
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
            "stratum": leaf["stratum"], "budget": bud,
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
    tt_files = {}
    for w in range(WORKERS):
        fp = os.path.join(SWEEP, f"tt_w{w}_{rung_tag}.tt")
        tt_files[f"w{w}"] = {
            "exists": os.path.exists(fp),
            "bytes": os.path.getsize(fp) if os.path.exists(fp) else 0,
        }
    # Prune all TT dumps but the current rung's (keep latest only).
    for f in os.listdir(SWEEP):
        if f.startswith("tt_w") and f.endswith(".tt") \
                and f"_{rung_tag}.tt" not in f:
            os.remove(os.path.join(SWEEP, f))

    n_dec = sum(1 for j in jobs_rec if j["outcome"] in ("win", "loss"))
    n_draw = sum(1 for j in jobs_rec if j["outcome"] == "draw")
    n_err = sum(1 for j in jobs_rec if j["outcome"] not in ("win", "loss", "draw"))
    n_missing = sum(1 for j in jobs_rec if j["outcome"] == "MISSING")
    n_lost = sum(1 for j in jobs_rec if j["outcome"] == "loss")
    rec = {
        "rung": rung_tag, "budget_evals": budget,
        "leaves_entering": len(ents), "jobs": len(ids),
        "resolved_here": n_dec, "still_open": n_draw, "lost_here": n_lost,
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
    rec["job_line_xcheck"] = {
        f"w{w}": stderr_rec[f"w{w}"]["job_lines"]
        == sum(1 for j in jobs_rec if j["worker"] == w)
        for w in range(WORKERS)
    }
    write_json(rec_path, rec)
    slim = {k: v for k, v in rec.items() if k not in ("jobs", "stderr", "sampler")}
    print(json.dumps(slim, indent=1), flush=True)
    if n_lost:
        print(f"rung {rung_tag}: {n_lost} LOST result(s) — registered as "
              "decided labels, machinery-noteworthy", flush=True)
    if rec["machinery"]["defect"]:
        raise SystemExit(f"rung {rung_tag}: MACHINERY DEFECT (abort)")
    if rec["machinery"]["export_failures"]:
        print(f"rung {rung_tag}: {rec['machinery']['export_failures']} "
              "export failures (downgraded jobs; recorded, warm state reset "
              "for those leaves — deviation from warm semantics)", flush=True)
    return rec


def ladder_elapsed_s():
    lp = os.path.join(STATE, "ladder.json")
    ld = read_json(lp)
    if not ld:
        ld = {"start": time.time()}
        write_json(lp, ld)
    return time.time() - ld["start"]


def maybe_r3():
    """The registered contingent R3 rule at the R2 close (plan11 §2)."""
    rec = read_json(os.path.join(STATE, "rung_R2.json"))
    if not rec:
        print("r3: R2 not done; nothing to decide")
        return None
    resolved = resolved_leaves_so_far(("R1", "R2"))
    leaves = sampled_leaves()
    survivors = [l["path"] for l in leaves if l["path"] not in resolved]
    elapsed_h = ladder_elapsed_s() / 3600.0
    projected_h = len(survivors) * (R3_BUDGET / KAPPA) / R_CAMP / 3600.0
    fire = (elapsed_h + projected_h) <= R3_RULE_H
    out = {
        "survivors": len(survivors), "elapsed_h": round(elapsed_h, 3),
        "projected_r3_h": round(projected_h, 3), "rule_h": R3_RULE_H,
        "constants": {"kappa": KAPPA, "r_camp": R_CAMP},
        "fired": fire,
    }
    write_json(os.path.join(STATE, "r3_decision.json"), out)
    print(json.dumps(out), flush=True)
    if fire:
        run_rung("R3", prev_tag="R2")
    return out


# ------------------------------ smoke ----------------------------------------


def run_smoke():
    """SMOKE (plan11 §2): 8 leaves x 100k evals, full close/restore
    protocol (fresh phase + warm restore phase).  Not a datapoint."""
    rec_path = os.path.join(STATE, "arm_SMOKE.json")
    if os.path.exists(rec_path):
        print("smoke: already done (state exists)")
        return read_json(rec_path)
    leaves = sampled_leaves()
    # 2 per stratum, in strata order (deterministic).
    sample, counts = [], {"H": 0, "M": 0, "L": 0, "U": 0}
    for l in leaves:
        if counts[l["stratum"]] < 2:
            sample.append(l)
            counts[l["stratum"]] += 1
    print(f"smoke: {len(sample)} leaves x {SMOKE_BUDGET} evals "
          f"({counts})", flush=True)

    fresh_session(SMOKE)
    write_session(SMOKE)
    phases = []
    for phase, prev_tag in (("S0", None), ("S1", "S0")):
        ids = prepare_rung(SMOKE, phase, sample,
                           budgets={l["path"]: SMOKE_BUDGET for l in sample})
        oom_before = read_oom_events()
        t0 = time.time()
        procs, wlog = [], []
        for w in range(WORKERS):
            tt_load = ""
            if prev_tag:
                prev_tt = os.path.join(SMOKE, f"tt_w{w}_{prev_tag}.tt")
                if os.path.exists(prev_tt) and os.path.getsize(prev_tt) > 0:
                    tt_load = prev_tt
            cmd = worker_cmd(SMOKE, w, phase, tt_load, 600)
            errf = open(os.path.join(LOGS, f"smoke_{phase}_w{w}.err"), "w")
            p = subprocess.Popen(cmd, stdin=subprocess.DEVNULL,
                                 stdout=subprocess.DEVNULL, stderr=errf)
            procs.append(p)
            wlog.append(errf)
        sampler = Sampler([(f"w{i}", p.pid) for i, p in enumerate(procs)])
        sampler.start()
        deadline = time.monotonic() + 600
        while True:
            have = [jid for jid, _, _, _ in ids
                    if os.path.exists(result_path(SMOKE, jid))]
            if len(have) == len(ids) or time.monotonic() > deadline \
                    or sampler.abort_event.is_set():
                break
            time.sleep(POLL_S)
        with open(os.path.join(SMOKE, "stop"), "w") as f:
            f.write("stop")
        exits = wait_workers_exited(procs, WORKER_WAIT_S)
        for errf in wlog:
            errf.close()
        wall = round(time.time() - t0, 3)
        snap = sampler.stop()
        oom_after = read_oom_events()
        jobs_rec = []
        for jid, w, leaf, bud in ids:
            r = read_json(result_path(SMOKE, jid))
            jobs_rec.append({
                "job_id": jid, "worker": w, "path": leaf["path"],
                "stratum": leaf["stratum"],
                "outcome": r["outcome"] if r else "MISSING",
                "child_evals": r["child_evals"] if r else None,
                "root_pn": r["root_pn"] if r else None,
                "root_dn": r["root_dn"] if r else None,
                "error": r.get("error") if r else "no result file",
            })
        stderr_rec = {}
        for w in range(WORKERS):
            jobs, restores, dumps, exports = parse_worker_stderr(
                os.path.join(LOGS, f"smoke_{phase}_w{w}.err"))
            stderr_rec[f"w{w}"] = {
                "job_lines": len(jobs), "restore": restores,
                "tt_dump": dumps, "export_failures": exports,
            }
        tt_files = {}
        for w in range(WORKERS):
            fp = os.path.join(SMOKE, f"tt_w{w}_{phase}.tt")
            tt_files[f"w{w}"] = {
                "exists": os.path.exists(fp),
                "bytes": os.path.getsize(fp) if os.path.exists(fp) else 0,
            }
        mm = sum(r["probe_mismatched"] for v in stderr_rec.values()
                 for r in v["restore"] if "probe_mismatched" in r)
        phases.append({
            "phase": phase, "warm": prev_tag is not None,
            "jobs": len(ids),
            "resolved": sum(1 for j in jobs_rec
                            if j["outcome"] in ("win", "loss")),
            "job_errors": sum(1 for j in jobs_rec
                              if j["outcome"] not in ("win", "loss", "draw")),
            "wall_s": wall, "sampler": snap,
            "oom_kill_delta": (
                oom_after - oom_before
                if oom_before is not None and oom_after is not None else None),
            "worker_exits": exits, "tt_files": tt_files,
            "stderr": stderr_rec, "jobs_rec": jobs_rec,
            "restore_probe_mismatches": mm,
            "restore_degraded_workers": sum(
                1 for v in stderr_rec.values()
                for r in v["restore"] if "raw" in r),
            "tt_dumps_missing": [k for k, v in tt_files.items()
                                 if not v["exists"]],
        })
        # Prune dumps but the current phase's (the next phase restores them).
        for f in os.listdir(SMOKE):
            if f.startswith("tt_w") and f.endswith(".tt") \
                    and f"_{phase}.tt" not in f:
                os.remove(os.path.join(SMOKE, f))
    total_spent = sum(
        j["child_evals"] or 0 for ph in phases for j in ph["jobs_rec"])
    rec = {
        "budget_evals": SMOKE_BUDGET, "n_leaves": len(sample),
        "sample": [l["path"] for l in sample],
        "strata_counts": counts, "phases": phases,
        "total_child_evals": total_spent,
        "defect": any(ph["restore_probe_mismatches"] or ph["job_errors"]
                      or ph["tt_dumps_missing"]
                      or ph["restore_degraded_workers"] for ph in phases),
    }
    write_json(rec_path, rec)
    slim = {k: v for k, v in rec.items()
            if k not in ("phases",)}
    slim["phase_summary"] = [
        {k: v for k, v in ph.items()
         if k in ("phase", "warm", "jobs", "resolved", "job_errors",
                  "wall_s", "restore_probe_mismatches",
                  "restore_degraded_workers", "tt_dumps_missing")}
        for ph in phases]
    print(json.dumps(slim, indent=1), flush=True)
    if rec["defect"]:
        raise SystemExit("smoke: MACHINERY DEFECT (abort)")
    # Cleanup: the smoke session dir is deleted at close (not a datapoint).
    shutil.rmtree(SMOKE, ignore_errors=True)
    return rec


# ------------------------------ merge pass -----------------------------------


def run_merge():
    """The A2 merge pass (plan11 §4): decisive results re-verified over
    plan10's committed merged state."""
    rec_path = os.path.join(STATE, "merge.json")
    if os.path.exists(rec_path):
        print("merge: already done (state exists)")
        return read_json(rec_path)
    fresh_session(MERGE)
    shutil.copy(PLAN10_MERGE, os.path.join(MERGE, "master_state.json"))
    # Copy ONLY the decisive sweep results (the ladder arms; SMOKE is not a
    # datapoint and is never merged).  Uniqueness: <= 1 per leaf by ladder
    # construction — verified here.
    seen = {}
    n_copied = 0
    for tag in ("R1", "R2", "R3"):
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
    summary = None
    exitcode = None
    run_master = n_copied > 0
    master_reason = (
        "decisive results present" if run_master else
        "no decisive results — nothing to re-verify; the master is not "
        "invoked because in resume mode it dispatches new jobs from t=0 "
        "(plan10's 'results never arrive within the wall' slop assumption "
        "breaks with an empty result set), which would add unregistered "
        "work to the committed close; the close state is committed as the "
        "verified identity of the plan10 merged close")
    if run_master:
        cmd = [
            MASTER, "--session", MERGE, "--fen", ROOT_FEN,
            "--workers", str(WORKERS),
            "--slice", "4000000", "--max-slice", "8000000",
            "--max-wall", "60", "--tt-mb", str(TT_MB), "--pt-mb", str(PT_MB),
            "--out", os.path.join(MERGE, "proof_tree.bin"),
            "--resume", "--job-seed", "11", "--state-every", "30",
        ]
        mout = open(os.path.join(LOGS, "merge_master.out"), "w")
        merr = open(os.path.join(LOGS, "merge_master.err"), "w")
        sampler = Sampler([])
        sampler.start()
        p = subprocess.Popen(cmd, stdin=subprocess.DEVNULL, stdout=mout,
                             stderr=merr)
        _, status, ru = os.wait4(p.pid, 0)
        exitcode = os.waitstatus_to_exitcode(status)
        mout.close()
        merr.close()
        summary = read_json(os.path.join(MERGE, "summary.json"))
        sampler.stop()
    else:
        # Registered no-op: the re-drain input set is empty; the close
        # state is the identity of the seed (verified below).
        summary = None
    wall = round(time.time() - t0, 3)
    oom_after = read_oom_events()
    # The merged close state -> committed record.
    ms = os.path.join(MERGE, "master_state.json")
    ms_versioned = None
    close_is_identity = None
    if os.path.exists(ms):
        ms_versioned = os.path.join(STATE, "master_state_merge11.json")
        shutil.copy(ms, ms_versioned)
        close_is_identity = (sha256(ms) == sha256(PLAN10_MERGE))
    if not run_master and not close_is_identity:
        raise SystemExit("merge: no-master close is not the seed identity "
                         "— machinery defect")
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
        "master_run": run_master, "master_reason": master_reason,
        "close_is_identity_of_seed": close_is_identity,
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


def _pct_rank(x, cohort):
    """Scale-free percentile rank within a cohort (registered reading)."""
    n = len(cohort)
    if not n:
        return None
    below = sum(1 for c in cohort if c < x)
    equal = sum(1 for c in cohort if c == x)
    return (below + 0.5 * equal) / n


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


def _combined_score(rows):
    """Sign-corrected combined rank mean: dn ascending-rank + pn
    descending-rank (positives = upper tail of dn, lower tail of pn)."""
    n = len(rows)
    scores = []
    for key, sign in (("dn", 1.0), ("pn", -1.0)):
        order = sorted(range(n), key=lambda i: rows[i][key])
        norm = {}
        for rank, i in enumerate(order):
            norm[i] = rank / max(1, n - 1)
        scores.append([sign * norm[i] for i in range(n)])
    return [0.5 * (scores[0][i] + scores[1][i]) for i in range(n)]


def _norm_ranks(values):
    """Values -> within-list normalized ascending ranks in [0, 1]."""
    n = len(values)
    order = sorted(range(n), key=lambda i: values[i])
    norm = [0.0] * n
    for rank, i in enumerate(order):
        norm[i] = rank / max(1, n - 1)
    return norm


def cmd_analyze(_args):
    strata = load_strata()
    frame = {l["path"]: l for l in sampled_leaves()}
    primary = [p for s in ("H", "M", "L") for p in strata[s]]
    u_leaves = list(strata["U"])
    rungs = {t: read_json(os.path.join(STATE, f"rung_{t}.json"))
             for t in ("R1", "R2", "R3")}
    rungs = {t: r for t, r in rungs.items() if r}
    merge_rec = read_json(os.path.join(STATE, "merge.json"))
    r3dec = read_json(os.path.join(STATE, "r3_decision.json"))
    smoke = read_json(os.path.join(STATE, "arm_SMOKE.json"))
    if merge_rec is None:
        raise SystemExit("analyze: merge pass missing — run `sweep11.py merge` first")
    invalidated = merge_rec.get("verify_failures") or 0
    if invalidated:
        print(f"WARNING: {invalidated} merge verify failures — affected "
              "leaves excluded from the decision metric", flush=True)

    # ---- Per-leaf, per-rung labels + covariates (regime-matched).
    # pre-rung covariate: R1 = committed advisory (fresh regime);
    # R2/R3 = the prior rung's post-run advisory (warm regime, primary).
    rows = []       # one row per (rung, leaf) over the PRIMARY corpus
    u_rows = []     # U-stratum control rows
    leaf_marginals = {}   # path -> {rung: evals}
    leaf_outcome = {}     # path -> first resolving rung/outcome
    for t in sorted(rungs):
        prev_tag = {"R2": "R1", "R3": "R2"}.get(t)
        # Pre-rung advisory per leaf (warm regime: prior rung's post-run).
        pre = {}
        if prev_tag:
            for j in rungs[prev_tag]["jobs"]:
                if j["outcome"] in ("win", "loss", "draw"):
                    pre.setdefault(j["path"], {
                        "pn": j["root_pn"], "dn": j["root_dn"]})
        # Group this rung's jobs per leaf (recovered jobs -> possibly two).
        by_leaf = {}
        for j in rungs[t]["jobs"]:
            by_leaf.setdefault(j["path"], []).append(j)
        for path, js in sorted(by_leaf.items()):
            if path not in frame:
                continue
            dec = [j for j in js if j["outcome"] in ("win", "loss")]
            draws = [j for j in js if j["outcome"] == "draw"]
            main = (dec or draws or js)
            main = max(main, key=lambda j: (j["child_evals"] or 0))
            spent = sum(j["child_evals"] or 0 for j in js)
            leaf_marginals.setdefault(path, {})[t] = spent
            label = ("pos" if dec else "neg")
            outcome = dec[0]["outcome"] if dec else "draw"
            if dec and path not in leaf_outcome:
                leaf_outcome[path] = {"rung": t, "outcome": dec[0]["outcome"]}
            if prev_tag:
                p = (pre.get(path) or {}).get("pn")
                d = (pre.get(path) or {}).get("dn")
                regime = "warm"
            else:
                p, d = frame[path]["committed_pn"], frame[path]["committed_dn"]
                regime = "fresh"
            row = {
                "rung": t, "path": path, "stratum": frame[path]["stratum"],
                "regime": regime, "label": label, "outcome": outcome,
                "evals_spent": spent, "budget": RUNG_BUDGETS[t],
                "pre_pn": min(p, INF_CUT) if p is not None else None,
                "pre_dn": min(d, INF_CUT) if d is not None else None,
                "post_pn": main["root_pn"], "post_dn": main["root_dn"],
                "job_id": main["job_id"], "worker": main["worker"],
            }
            if row["pre_dn"] is None:
                row["pre_dn"] = INF_CUT
                row["pre_pn"] = INF_CUT
            (rows if frame[path]["stratum"] != "U" else u_rows).append(row)

    # ---- Per-rung signal metrics (primary corpus).
    per_rung = {}
    for t in sorted(rungs):
        rr = [r for r in rows if r["rung"] == t]
        if not rr:
            continue
        pos = [r for r in rr if r["label"] == "pos"]
        neg = [r for r in rr if r["label"] == "neg"]
        cohort_dn = [r["pre_dn"] for r in rr]
        for r in rr:
            r["pre_dn_pct"] = _pct_rank(r["pre_dn"], cohort_dn)
        rec = {
            "regime": rr[0]["regime"], "entrants": len(rr),
            "positives": len(pos), "negatives": len(neg),
            "lost": sum(1 for r in rr if r["outcome"] == "loss"),
            "evals_spent_total": sum(r["evals_spent"] for r in rr),
        }
        if pos:
            rec["signals"] = {
                "pre_dn": _youden_auc([r["pre_dn"] for r in pos],
                                      [r["pre_dn"] for r in neg]),
                "pre_pn": _youden_auc([r["pre_pn"] for r in pos],
                                      [r["pre_pn"] for r in neg]),
            }
            comb = _combined_score(rr)
            cpos = [c for c, r in zip(comb, rr) if r["label"] == "pos"]
            cneg = [c for c, r in zip(comb, rr) if r["label"] == "neg"]
            rec["signals"]["combined"] = _youden_auc(cpos, cneg)
            # Cumulative-work control covariate (registered: must NOT
            # separate; plan10 M3 measured AUC 0.000).
            cum = {l["path"]: l["committed_work"] + l["plan10_spend"]
                   for l in frame.values()}
            rec["signals"]["cumulative_work_control"] = _youden_auc(
                [cum[r["path"]] for r in pos], [cum[r["path"]] for r in neg])
        per_rung[t] = rec

    # ---- Pooled warm-regime AUC by rank-attachment (primary).
    warm_tags = [t for t in ("R2", "R3") if t in per_rung]
    pooled = None
    if warm_tags:
        pool = []
        for t in warm_tags:
            rr = [r for r in rows if r["rung"] == t]
            dn = [r["pre_dn"] for r in rr]
            ranks = _norm_ranks(dn)
            for r, rk in zip(rr, ranks):
                pool.append((rk, 1 if r["label"] == "pos" else 0, r["path"]))
        pos = [x for x, lab, _ in pool if lab == 1]
        neg = [x for x, lab, _ in pool if lab == 0]
        if pos and neg:
            pooled = _youden_auc(pos, neg)
        pooled = {
            "warm_rungs": warm_tags,
            "pooled_positives": len(pos), "pooled_negatives": len(neg),
            "auc_by_rank_attachment": pooled,
        }

    # ---- theta procedure (registered).
    r1_pos = [r for r in rows if r["rung"] == "R1" and r["label"] == "pos"]
    r2_pos = [r for r in rows if r["rung"] == "R2" and r["label"] == "pos"]
    r3_pos = [r for r in rows if r["rung"] == "R3" and r["label"] == "pos"]
    theta_rec = {"branch": None, "theta": None, "heldout_rungs": [],
                 "heldout_false_negatives": []}
    heldout_fn = 0
    if r1_pos:
        theta = min(r["pre_dn"] for r in r1_pos)
        heldout = r2_pos + r3_pos
        theta_rec.update({"branch": "R1-threshold (fresh regime)",
                          "theta": theta,
                          "heldout_rungs": [t for t in ("R2", "R3")
                                            if t in rungs]})
        fns = [r for r in heldout if r["pre_dn"] <= theta]
        heldout_fn = len(fns)
        theta_rec["heldout_false_negatives"] = [
            {"rung": r["rung"], "path": r["path"], "pre_dn": r["pre_dn"]}
            for r in fns]
    elif r2_pos:
        theta = min(r["pre_dn"] for r in r2_pos)
        heldout = r3_pos
        theta_rec.update({
            "branch": "R2-threshold (warm regime)" + (
                ", R3 held-out" if "R3" in rungs
                else " — NO held-out rung: in-sample-only analysis"),
            "theta": theta,
            "heldout_rungs": ["R3"] if "R3" in rungs else []})
        fns = [r for r in heldout if r["pre_dn"] <= theta]
        heldout_fn = len(fns)
        theta_rec["heldout_false_negatives"] = [
            {"rung": r["rung"], "path": r["path"], "pre_dn": r["pre_dn"]}
            for r in fns]
    else:
        theta = None
        theta_rec["branch"] = "no positives — theta undefined"
    r2_entrants = [r for r in rows if r["rung"] == "R2"]
    s_theta = None
    if theta is not None and r2_entrants:
        s_theta = round(sum(1 for r in r2_entrants
                            if r["pre_dn"] <= theta) / len(r2_entrants), 4)
    theta_rec["s_theta"] = s_theta

    # ---- Directional inversions (registered reading, scale-free).
    inversions = [r for r in rows if r["label"] == "pos"
                  and r.get("pre_dn_pct") is not None
                  and r["pre_dn_pct"] <= 0.35]

    # ---- The pre-registered M2 gate (plan11 §4).
    P = sum(1 for r in rows if r["label"] == "pos")
    auc_pool = (pooled or {}).get("auc_by_rank_attachment") \
        if pooled else None
    gate = {
        "positives_P": P,
        "pooled_warm_auc": auc_pool,
        "theta": theta, "s_theta": s_theta,
        "heldout_false_negatives": heldout_fn,
        "directional_inversions": len(inversions),
        "inversion_rows": [{"rung": r["rung"], "path": r["path"],
                            "pre_dn": r["pre_dn"],
                            "pre_dn_pct": r.get("pre_dn_pct")}
                           for r in inversions],
        "r3_fired": bool(r3dec and r3dec.get("fired")),
        "lost_labels": sum(1 for r in rows if r["outcome"] == "loss"),
    }
    if P == 0:
        if r3dec and r3dec.get("fired"):
            gate["verdict"] = "SUBSTRATE_EMPTY"
            gate["detail"] = ("P = 0 at the full ladder including fired R3': "
                              "the advisory-dn channel produced no positives "
                              "to validate at all (stronger M2-MARGINAL "
                              "wording; Stage 2 halts)")
        else:
            gate["verdict"] = "M2-MARGINAL"
            gate["detail"] = ("P = 0 without a fired R3'; treated as "
                              "M2-MARGINAL (unvalidated; no plan12 draft)")
    elif P < 3:
        gate["verdict"] = "M2-MARGINAL"
        gate["detail"] = ("0 < P < 3: the substrate cannot be validated at "
                          "affordable n on this frontier (registered action: "
                          "halt + sharpened reopener record)")
    else:
        if auc_pool is not None and auc_pool < 0.60:
            gate["verdict"] = "M2-NO-GO"
            gate["detail"] = "pooled warm-regime AUC < 0.60"
        elif len(inversions) >= 2:
            gate["verdict"] = "M2-NO-GO"
            gate["detail"] = ">= 2 directional inversions"
        elif heldout_fn >= 1:
            gate["verdict"] = "M2-NO-GO"
            gate["detail"] = "a held-out false negative"
        elif (auc_pool is not None and auc_pool >= 0.80
              and s_theta is not None and s_theta >= 0.40
              and len(inversions) < 2 and heldout_fn == 0):
            gate["verdict"] = "M2-GO"
            gate["detail"] = ("P >= 3, zero held-out FNs, pooled warm AUC "
                              ">= 0.80, S(theta) >= 0.40, < 2 inversions")
        else:
            gate["verdict"] = "M2-MARGINAL"
            gate["detail"] = (
                "P >= 3 but the GO conjunction is not met and NO-GO's "
                "disjunction is not fired either (registered gap reading: "
                "warm AUC in [0.60, 0.80) with clean direction, or warm "
                "AUC >= 0.80 with S(theta) < 0.40) — no plan12 draft")
    # The in-sample-only caveat: a GO verdict without any held-out rung
    # additionally requires pooled AUC >= 0.85 (registered).
    if gate["verdict"] == "M2-GO" and not theta_rec["heldout_rungs"]:
        if auc_pool is None or auc_pool < 0.85:
            gate["verdict"] = "M2-MARGINAL"
            gate["detail"] = ("GO conditions met but NO held-out rung "
                              "exists (in-sample-only analysis; registered "
                              "caveat requires pooled AUC >= 0.85)")

    # ---- U-stratum control (analyzed separately; gates nothing).
    u_summary = {
        "entrants": len({r["path"] for r in u_rows}),
        "resolved": len({r["path"] for r in u_rows if r["label"] == "pos"}),
        "child_evals_total": sum(r["evals_spent"] for r in u_rows),
        "per_leaf": u_rows,
    }

    # ---- Merged-status overlay (the A2-verified record).
    merged_status = {}
    if merge_rec.get("master_state_checkpoint"):
        _, mst = _leaves_of(merge_rec["master_state_checkpoint"])
        merged_status = {p: v["status"] for p, v in mst.items()
                         if p in frame}
    delta_facts = {
        "resolved_in_sample": len(leaf_outcome),
        "merged_won": merge_rec.get("leaves_won"),
        "merged_open": merge_rec.get("leaves_open"),
        "merged_children_resolved": merge_rec.get("children_resolved"),
        "note": "plan10 merged close: 68 Won / 660 Open / children 0/27",
    }

    # ---- ledger11 (plan10 ledger extended with plan11 spend).
    plan10_ledger = read_json(PLAN10_LEDGER)
    ledger11 = []
    for row in plan10_ledger:
        path = row["path"]
        entry = dict(row)
        entry["plan11_marginals"] = leaf_marginals.get(path, {})
        entry["plan11_total_spend"] = sum(
            leaf_marginals.get(path, {}).values())
        entry["plan11_resolving_rung"] = leaf_outcome.get(
            path, {}).get("rung")
        entry["plan11_outcome"] = leaf_outcome.get(path, {}).get("outcome")
        entry["stratum"] = frame[path]["stratum"] if path in frame else None
        entry["merged11_status"] = merged_status.get(path)
        ledger11.append(entry)
    write_json(os.path.join(STATE, "ledger11.json"), ledger11)
    with open(os.path.join(STATE, "ledger11.csv"), "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["path", "untouched", "historical_work",
                    "plan10_spend", "plan11_r1_evals", "plan11_r2_evals",
                    "plan11_r3_evals", "plan11_total", "stratum",
                    "plan11_resolving_rung", "plan11_outcome",
                    "merged11_status"])
        for e in ledger11:
            if e["plan11_total_spend"] or e["stratum"]:
                w.writerow([
                    e["path"], e["untouched"], e["historical_work"],
                    e["cumulative_sweep_spend"],
                    e["plan11_marginals"].get("R1"),
                    e["plan11_marginals"].get("R2"),
                    e["plan11_marginals"].get("R3"),
                    e["plan11_total_spend"], e["stratum"],
                    e["plan11_resolving_rung"], e["plan11_outcome"],
                    e["merged11_status"]])

    # ---- M2 corpus CSV (the deliverable dataset).
    with open(os.path.join(STATE, "m2_corpus.csv"), "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["rung", "path", "stratum", "regime", "label", "outcome",
                    "evals_spent", "budget", "pre_pn", "pre_dn",
                    "pre_dn_pct", "post_pn", "post_dn", "worker", "job_id"])
        for r in sorted(rows + u_rows, key=lambda r: (r["rung"], r["path"])):
            w.writerow([r["rung"], r["path"], r["stratum"], r["regime"],
                        r["label"], r["outcome"], r["evals_spent"],
                        r["budget"], r["pre_pn"], r["pre_dn"],
                        (round(r["pre_dn_pct"], 4)
                         if r.get("pre_dn_pct") is not None else None),
                        r["post_pn"], r["post_dn"], r["worker"],
                        r["job_id"]])

    # ---- Spend accounting.
    per_rung_spend = {
        t: sum(j["child_evals"] or 0 for j in rungs[t]["jobs"])
        for t in sorted(rungs)}
    spend = {
        "per_rung_child_evals": per_rung_spend,
        "total_child_evals": sum(per_rung_spend.values()),
        "smoke_child_evals": (smoke or {}).get("total_child_evals"),
        "total_nodes_est": round(sum(per_rung_spend.values()) / KAPPA),
        "constants": {"kappa": KAPPA, "r_camp": R_CAMP},
        "observed_rate_note": "rates measured from rung walls; projections "
                              "used the locked constants only",
    }

    # ---- Cross-checks / deviations.
    xchecks = {
        "job_errors": {t: rungs[t]["job_errors"] for t in sorted(rungs)},
        "missing_results": {t: rungs[t]["results_missing"]
                            for t in sorted(rungs)},
        "merge_job_errors": merge_rec.get("job_errors"),
        "merge_verify_failures": invalidated,
        "lost_labels": {t: rungs[t].get("lost_here", 0) for t in sorted(rungs)},
        "deviations": {t: rungs[t]["deviations"]
                       for t in sorted(rungs) if rungs[t]["deviations"]},
        "restore_probe_mismatches": {
            t: rungs[t]["machinery"]["restore_probe_mismatches"]
            for t in sorted(rungs)},
        "covariate_control_check": {
            "cumulative_work_auc_per_rung": {
                t: (per_rung[t].get("signals", {})
                    .get("cumulative_work_control"))
                for t in sorted(per_rung)},
            "note": "registered control: cumulative work must NOT separate "
                    "(plan10 M3 AUC 0.000); a separating control voids the "
                    "gate verdict until the defect is fixed",
        },
    }

    out = {
        "root_fen": ROOT_FEN,
        "shape": {"workers": WORKERS, "tt_mb": TT_MB, "pt_mb": PT_MB,
                  "rung_budgets": RUNG_BUDGETS,
                  "strata": {s: len(strata[s]) for s in ("H", "M", "L", "U")}},
        "registered_readings": {
            "percentile_rank": "pct(x,C) = (#c<x + 0.5*#c==x)/|C|",
            "directional_inversion": "positive with pct(pre_dn, rung "
                                     "entrant cohort) <= 0.35",
            "pooled_warm_auc": "rank-attachment: within-cohort percentile "
                               "ranks pooled across warm rungs",
            "heldout_fn": "held-out positive with pre_dn <= theta",
        },
        "per_rung": per_rung,
        "pooled_warm": pooled,
        "theta": theta_rec,
        "gate": gate,
        "u_stratum": {k: v for k, v in u_summary.items() if k != "per_leaf"},
        "delta_facts": delta_facts,
        "spend": spend,
        "merge": {k: merge_rec.get(k) for k in (
            "decisive_copied", "master_exit", "verify_failures",
            "job_errors", "leaves_won", "leaves_open", "children_resolved")},
        "r3_decision": r3dec,
        "smoke": (None if not smoke else {
            "defect": smoke.get("defect"),
            "total_child_evals": smoke.get("total_child_evals")}),
        "cross_checks": xchecks,
        "verify_failures": invalidated,
    }
    write_json(os.path.join(STATE, "analysis.json"), out)
    print(json.dumps({"gate": gate, "theta": theta_rec,
                      "per_rung": per_rung, "pooled_warm": pooled,
                      "spend": spend}, indent=1))
    return out


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
            "metadata for untouched sources (plan10 §9.2). Behavior risk "
            "carried by the A2 merge-pass verification."),
        "root_fen": ROOT_FEN,
        "frame_source": "measurements/plan10/state/master_state_merge.json",
        "covariate_sources": [
            "measurements/plan9/state/master_state_s1.json (committed "
            "advisory pn/dn/work)",
            "measurements/plan10/state/ledger.json (cumulative sweep spend)",
        ],
        "rungs": RUNG_BUDGETS,
        "smoke_budget": SMOKE_BUDGET,
        "shape": {"workers": WORKERS, "tt_mb": TT_MB, "pt_mb": PT_MB,
                  "retention": "on", "master": "driver-bypassed during "
                  "the sweep"},
        "session_dirs": {"sweep": SWEEP, "smoke": SMOKE, "merge": MERGE},
    }
    write_json(os.path.join(HERE, "env.json"), meta)
    print(json.dumps(meta, indent=2))
    if not prod_ok:
        raise SystemExit("product binary identity check FAILED")


def cmd_status(_args):
    for tag in ("R1", "R2", "R3"):
        r = read_json(os.path.join(STATE, f"rung_{tag}.json"))
        if r:
            print(f"rung {tag}: entering={r['leaves_entering']} "
                  f"resolved={r['resolved_here']} lost={r.get('lost_here', 0)} "
                  f"errors={r['job_errors']} wall={r['wall_s']}s "
                  f"defect={r['machinery']['defect']}")
        else:
            print(f"rung {tag}: not run")
    for name in ("arm_SMOKE", "r3_decision", "merge"):
        r = read_json(os.path.join(STATE, f"{name}.json"))
        if r:
            print(f"{name}: done")
        else:
            print(f"{name}: not run")
    ap = os.path.join(STATE, "analysis.json")
    if os.path.exists(ap):
        a = read_json(ap)
        print(f"gate: {a['gate']['verdict']} P={a['gate'].get('positives_P')}")
    else:
        print("analysis: not run")
    print(f"strata: "
          f"{sum(1 for _ in (read_json(os.path.join(STATE, 'strata.json')) or {}).get('H', []))} H leaves")


def main():
    ensure_dirs()
    cmd = sys.argv[1] if len(sys.argv) > 1 else ""
    if cmd == "env":
        cmd_env(None)
    elif cmd == "strata":
        build_strata()
    elif cmd == "smoke":
        run_smoke()
    elif cmd == "rung":
        prev = {"R1": None, "R2": "R1", "R3": "R2"}
        rung_tag = sys.argv[2].upper()
        lp = os.path.join(STATE, "ladder.json")
        if not os.path.exists(lp):
            write_json(lp, {"start": time.time()})
        run_rung(rung_tag, prev_tag=prev[rung_tag])
    elif cmd == "ladder":
        lp = os.path.join(STATE, "ladder.json")
        if not os.path.exists(lp):
            write_json(lp, {"start": time.time()})
        prev = {"R1": None, "R2": "R1", "R3": "R2"}
        for tag in ("R1", "R2"):
            if not os.path.exists(os.path.join(STATE, f"rung_{tag}.json")):
                run_rung(tag, prev_tag=prev[tag])
        maybe_r3()  # the contingent rule decides R3, never the loop
    elif cmd == "r3":
        maybe_r3()
    elif cmd == "merge":
        run_merge()
    elif cmd == "analyze":
        cmd_analyze(None)
    elif cmd == "status":
        cmd_status(None)
    else:
        raise SystemExit("unknown cmd %r (usage: env|strata|smoke|"
                         "rung <R1|R2|R3>|ladder|r3|merge|analyze|status)"
                         % cmd)


if __name__ == "__main__":
    main()
