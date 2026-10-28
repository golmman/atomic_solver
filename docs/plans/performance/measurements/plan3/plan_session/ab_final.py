#!/usr/bin/env python3
"""plan3 plan-session A/B: baseline HEAD vs the /tmp pre-probe prototype."""
import json, os, re, statistics, subprocess, sys, tempfile, time
sys.dont_write_bytecode = True
BASE = "/tmp/plan3_base/target/release/atomic_solver"
PROTO = "/tmp/plan3_proto/target/release/atomic_solver"
DEC10 = "3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41"
M22 = "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22"
STRESS = "4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21"
DEC13 = "r1bq1k1r/ppN4p/n1p1p3/3p1n1P/1b1P2P1/2P5/PP6/RNBQKB1R w KQ - 1 14"
REM12 = "rnbqkbnr/8/6pp/pppppp1B/3PPP2/N5PN/PPP4P/R1BQ1RK1 w kq - 0 10"
CASES = [
    ("mate1", "3Qk3/8/8/8/8/8/8/4K3 w - - 0 1", ["--timeout", "2"], 15),
    ("easy_kq", "4k3/8/8/8/8/8/8/4KQ2 w - - 0 1", ["--timeout", "2"], 15),
    ("dec13", DEC13, ["--timeout", "10"], 11),
    ("dec10", DEC10, ["--timeout", "10"], 11),
    ("m22", M22, ["--timeout", "10"], 11),
    ("m22_b", M22, ["--budget", "10000000"], 11),
    ("stress_b", STRESS, ["--budget", "10000000"], 11),
    ("rem12", REM12, ["--timeout", "60"], 7),
]
RE_EVALS = re.compile(r"^evals: (\d+)$", re.MULTILINE)
def run(binary, fen, extra):
    cmd = [binary, "--fen", fen, *extra, "--tt-size", "256", "--first-outcome", "--outcome-only"]
    with tempfile.TemporaryFile() as o, tempfile.TemporaryFile() as e:
        t0 = time.monotonic()
        pr = subprocess.Popen(cmd, stdout=o, stderr=e)
        _, st, ru = os.wait4(pr.pid, 0)
        wall = time.monotonic() - t0
        o.seek(0); e.seek(0)
        out = o.read().decode(); err = e.read().decode()
    m = RE_EVALS.search(err)
    return {"wall": wall, "evals": int(m.group(1)) if m else None,
            "maxrss": ru.ru_maxrss, "minflt": ru.ru_minflt,
            "stdout": [l for l in out.splitlines() if l.strip()]}
def main():
    results = {}
    for name, fen, extra, reps in CASES:
        arm = {"base": [], "proto": []}
        for _ in range(reps):
            arm["base"].append(run(BASE, fen, extra))
            arm["proto"].append(run(PROTO, fen, extra))
        b, p = arm["base"], arm["proto"]
        row = {"name": name, "reps": reps,
               "base_wall": statistics.median(r["wall"] for r in b),
               "proto_wall": statistics.median(r["wall"] for r in p),
               "base_min": min(r["wall"] for r in b),
               "proto_min": min(r["wall"] for r in p),
               "evals_base": sorted({r["evals"] for r in b}),
               "evals_proto": sorted({r["evals"] for r in p}),
               "stdout_identical": {tuple(r["stdout"]) for r in b} == {tuple(r["stdout"]) for r in p},
               "rss_base": statistics.median(r["maxrss"] for r in b),
               "rss_proto": statistics.median(r["maxrss"] for r in p),
               "minflt_base": statistics.median(r["minflt"] for r in b),
               "minflt_proto": statistics.median(r["minflt"] for r in p)}
        row["ratio"] = row["proto_wall"] / row["base_wall"]
        row["evals_identical"] = row["evals_base"] == row["evals_proto"]
        results[name] = row
        print(json.dumps(row))
        sys.stdout.flush()
    json.dump(results, open("/tmp/plan3_probe/ab_final.json", "w"), indent=1)
main()
