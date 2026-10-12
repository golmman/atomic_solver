#!/usr/bin/env python3
"""plan11 stage-1: mechanism probes (M1) + facts-volume probe.

Per probed case (dec13 s0, dec10 s0, m22 s0 — the highest-emission-rate and
largest-volume plan10 cases), 5 interleaved same-binary pairs: plain vs
`--facts-out` (H-mech M1). Pair order alternates across pairs to decorrelate
ambient drift. Machine-exclusive by convention.

M1 gate (pre-registered): median wall ratio <= 1.03 on every probed case,
stdout byte-identical and evals identical in every pair.

Volume probe: per --facts-out run, the `facts_export:` stderr line (records
seen / emitted / filtered / bytes) plus a post-hoc file scan (CRC-verified
distinct keys, path_len histogram) for the pass-rate analysis.

Writes state/probes11.json (raw transcripts under logs/probes11/, never
committed).
"""

import hashlib
import json
import os
import statistics
import struct
import subprocess
import sys
import time
import zlib

sys.dont_write_bytecode = True

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "../../../../.."))
BIN = os.environ.get("BIN", os.path.join(REPO, "target/release/atomic_solver"))
STATE = os.path.join(HERE, "state")
LOGS = os.path.join(HERE, "logs")


# Probed cases (plan11 stage-1 table): plain vs --facts-out, salt 0.
PROBES = {
    "dec13": "r1bq1k1r/ppN4p/n1p1p3/3p1n1P/1b1P2P1/2P5/PP6/RNBQKB1R w KQ - 1 14",
    "dec10": "3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41",
    "m22_white": "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22",
}
PAIRS = 5
TT_MB = "128"

# Facts-file mount: tmpfs (see the facts_out comment for the overlayfs
# confound this avoids).
FACTS_TMP = os.environ.get("FACTS_TMP", "/tmp/plan11_facts")


def run_one(tag, fen, facts_out, timeout=120):
    cmd = [
        BIN, "--fen", fen, "--tt-size", TT_MB, "--first-outcome",
        "--outcome-only", "--salt", "0", "--timeout", str(timeout),
    ]
    if facts_out:
        cmd += ["--facts-out", facts_out]
    logdir = os.path.join(LOGS, "probes11")
    os.makedirs(logdir, exist_ok=True)
    base = os.path.join(logdir, tag)
    t0 = time.time()
    with open(base + ".out", "w") as fo, open(base + ".err", "w") as fe:
        p = subprocess.run(cmd, stdout=fo, stderr=fe, stdin=subprocess.DEVNULL)
    wall = time.time() - t0
    err = open(base + ".err").read()
    stdout = open(base + ".out").read()
    evals = None
    export = None
    for line in err.splitlines():
        if line.startswith("evals: "):
            evals = int(line.split()[1])
        elif line.startswith("facts_export: "):
            export = {
                kv.split("=")[0]: int(kv.split("=")[1])
                for kv in line[len("facts_export: "):].split()
            }
    return {
        "tag": tag,
        "cmd": cmd,
        "rc": p.returncode,
        "wall_s": wall,
        "stdout_md5": hashlib.md5(stdout.encode()).hexdigest(),
        "evals": evals,
        "export": export,
        "stdout": stdout,
    }


def scan_file(path):
    """Post-hoc record scan: CRC check, distinct keys, path_len stats."""
    data = open(path, "rb").read()
    n = len(data) // 24
    keys, lens, crc_fail = set(), [], 0
    for i in range(n):
        rec = data[i * 24:(i + 1) * 24]
        key, tag, pl = struct.unpack("<QBB", rec[:10])
        crc = struct.unpack("<I", rec[16:20])[0]
        if zlib.crc32(rec[:16]) & 0xFFFFFFFF != crc or tag not in (1, 2):
            crc_fail += 1
            continue
        keys.add(key)
        lens.append(pl)
    return {
        "records": n,
        "crc_failures": crc_fail,
        "distinct_keys": len(keys),
        "distinct_share": round(len(keys) / n, 4) if n else None,
        "path_len_min": min(lens) if lens else None,
        "path_len_max": max(lens) if lens else None,
        "bytes": len(data),
    }


def main():
    os.makedirs(STATE, exist_ok=True)
    out_path = os.path.join(STATE, "probes11.json")
    out = {}
    if os.path.exists(out_path):
        out = json.load(open(out_path))

    for case, fen in PROBES.items():
        if case in out:
            continue
        entry = {"pairs": [], "volume": None}
        for pair in range(1, PAIRS + 1):
            # Interleaved with alternating order (odd pair plain-first).
            order = ["plain", "facts"] if pair % 2 == 1 else ["facts", "plain"]
            for which in order:
                tag = f"{case}_p{pair}_{which}"
                facts_out = None
                if which == "facts":
                    # Facts files on tmpfs: facts dirs are per-race and
                    # ephemeral, and a backing filesystem's writeback
                    # blocking is an environmental confound that would swamp
                    # the carrier tax under measurement (see
                    # state/probes11_overlayfs_confound.json for the
                    # confounded first pass).
                    facts_dir = os.path.join(FACTS_TMP, "probes11")
                    os.makedirs(facts_dir, exist_ok=True)
                    facts_out = os.path.join(facts_dir, tag + ".bin")
                r = run_one(tag, fen, facts_out)
                r.pop("stdout")
                entry["pairs"].append({"which": which, **r})
        # Volume from the last facts run's file (deterministic run).
        last_facts_tag = f"{case}_p{PAIRS}_facts"
        vol = scan_file(
            os.path.join(FACTS_TMP, "probes11", last_facts_tag + ".bin")
        )
        # Merge the exporter stderr counters from every facts pair.
        exports = [p["export"] for p in entry["pairs"] if p["which"] == "facts"]
        entry["volume"] = {
            "file": vol,
            "exporter_lines": exports,
        }
        out[case] = entry
        # Interim mechanical M1 read.
        walls = {}
        for which in ("plain", "facts"):
            walls[which] = [p["wall_s"] for p in entry["pairs"] if p["which"] == which]
        ratio = statistics.median(walls["facts"]) / statistics.median(walls["plain"])
        evals_eq = all(
            p["evals"] == q["evals"]
            for p, q in zip(
                [p for p in entry["pairs"] if p["which"] == "plain"],
                [p for p in entry["pairs"] if p["which"] == "facts"],
            )
        )
        md5_eq = all(
            p["stdout_md5"] == q["stdout_md5"]
            for p, q in zip(
                [p for p in entry["pairs"] if p["which"] == "plain"],
                [p for p in entry["pairs"] if p["which"] == "facts"],
            )
        )
        print(
            f"{case}: wall_ratio={ratio:.3f} evals_eq={evals_eq} md5_eq={md5_eq} "
            f"volume={vol}",
            flush=True,
        )
        with open(out_path, "w") as fh:
            json.dump(out, fh, indent=1)

    # Final M1 evaluation.
    summary = {}
    m1_pass = True
    for case in PROBES:
        entry = out[case]
        walls = {
            which: [p["wall_s"] for p in entry["pairs"] if p["which"] == which]
            for which in ("plain", "facts")
        }
        ratio = statistics.median(walls["facts"]) / statistics.median(walls["plain"])
        evals_eq = all(
            p["evals"] == q["evals"]
            for p, q in zip(
                [p for p in entry["pairs"] if p["which"] == "plain"],
                [p for p in entry["pairs"] if p["which"] == "facts"],
            )
        )
        md5_eq = all(
            p["stdout_md5"] == q["stdout_md5"]
            for p, q in zip(
                [p for p in entry["pairs"] if p["which"] == "plain"],
                [p for p in entry["pairs"] if p["which"] == "facts"],
            )
        )
        ok = ratio <= 1.03 and evals_eq and md5_eq
        m1_pass &= ok
        summary[case] = {
            "median_wall_plain": statistics.median(walls["plain"]),
            "median_wall_facts": statistics.median(walls["facts"]),
            "wall_ratio": round(ratio, 4),
            "evals_identical": evals_eq,
            "stdout_identical": md5_eq,
            "M1_pass": ok,
        }
    out["_m1"] = summary
    out["_m1_pass"] = m1_pass
    with open(out_path, "w") as fh:
        json.dump(out, fh, indent=1)
    print("M1:", "PASS" if m1_pass else "FAIL", flush=True)
    for case, s in summary.items():
        print(f"  {case}: ratio={s['wall_ratio']} evals_eq={s['evals_identical']} md5_eq={s['stdout_identical']}", flush=True)
    print("PROBES DONE", flush=True)


if __name__ == "__main__":
    main()
