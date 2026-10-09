#!/usr/bin/env python3
import concurrent.futures
import os
import subprocess
import sys

sys.dont_write_bytecode = True

# PROBE_BIN: probe_solve built from a tree with probe.patch applied.
# PROBE_RESULTS: raw output directory (transcripts are not committed).
B = os.environ.get("PROBE_BIN", "/tmp/probe/target/release/examples/probe_solve")
R = os.environ.get("PROBE_RESULTS", "/tmp/probe/results")
FEN = {
    "stress": "4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21",
    "m22": "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22",
    "dec13": "r1bq1k1r/ppN4p/n1p1p3/3p1n1P/1b1P2P1/2P5/PP6/RNBQKB1R w KQ - 1 14",
    "dec10": "3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41",
}
BUDGET = {"stress": 2500000000, "m22": 1000000000, "dec13": 1000000000, "dec10": 1000000000}

jobs = []

def add(name, case, tt=128, depth=None, env=None):
    jobs.append(dict(name=name, case=case, tt=tt, depth=depth, env=env or {}))

# R1 baseline (the original session ran stress/dec13/dec10 by hand first)
for c in ("stress", "m22", "dec13", "dec10"):
    add(f"r1_{c}", c)
# R2 TT size, baseline
for tt in (256, 512, 1024):
    add(f"r2_stress_tt{tt}", "stress", tt=tt)
add("r2_m22_tt256", "m22", tt=256)
# R3 depth cap, baseline
for d in (100, 128, 160, 200, 256):
    add(f"r3_stress_d{d}", "stress", depth=d)
for d in (100, 128, 192):
    add(f"r3_m22_d{d}", "m22", depth=d)
# R4 noise salts
for s in (1, 2, 3, 4):
    for c in ("m22", "dec13", "dec10", "stress"):
        add(f"r4_{c}_s{s}", c, env={"PROBE_SALT": str(s)})
# R5 arms at salt 0
for c in ("stress", "m22", "dec13", "dec10"):
    add(f"r5_{c}_mob", c, env={"PROBE_INIT": "mob"})
    add(f"r5_{c}_wpns", c, env={"PROBE_SUM": "wpns"})
# R5 arms at salts 1-4 (not stress)
for s in (1, 2, 3, 4):
    for c in ("m22", "dec13", "dec10"):
        add(f"r5_{c}_mob_s{s}", c, env={"PROBE_INIT": "mob", "PROBE_SALT": str(s)})
        add(f"r5_{c}_wpns_s{s}", c, env={"PROBE_SUM": "wpns", "PROBE_SALT": str(s)})
# R7 chunk schedule
for ch in (8000000, 128000000, 4000000000):
    for c in ("stress", "m22", "dec13", "dec10"):
        add(f"r7_{c}_c{ch}", c, env={"PROBE_CHUNK0": str(ch)})


def run(j):
    out = os.path.join(R, j["name"] + ".json")
    if os.path.exists(out) and os.path.getsize(out) > 0:
        return j["name"], "skip"
    cmd = [B, "--fen", FEN[j["case"]], "--tt", str(j["tt"]),
           "--budget", str(BUDGET[j["case"]])]
    if j["depth"] is not None:
        cmd += ["--depth", str(j["depth"])]
    env = dict(os.environ)
    env.update(j["env"])
    with open(out, "w") as fo, open(out + ".err", "w") as fe:
        p = subprocess.run(cmd, stdout=fo, stderr=fe, env=env)
    return j["name"], p.returncode


with concurrent.futures.ThreadPoolExecutor(3) as ex:
    for name, rc in ex.map(run, jobs):
        print(name, rc, flush=True)
print("ALL DONE")
