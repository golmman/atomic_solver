#!/usr/bin/env python3
"""Pilot loop driver (`solve` initiative plan2 §4): Arm A baseline, Arm B
SSFP queue push, and the substrate metric.

Pre-registered protocol (do not tune after seeing data):

- Root: d4d5 p2 (`rnbqkbnr/ppp1pppp/8/3p4/3P4/8/PPP1PPPP/RNBQKBNR w KQkq
  d6 0 2`, censored at 23.4M nodes / 120 s in plan1).
- Budget B = 2 h wall per arm, sequential, defaults everywhere.
- Arm A (baseline): one `--timeout 7200` solve of the root.
- Arm B (push): queue iterations summing to <= B: root solve -> frontier
  dump -> select cheapest targets (expected-cost order: min(pn, dn)
  heuristic, work as tiebreak; cheapest-first) -> solve isolated ->
  validate -> deposit -> repeat. Per-target timeout 120 s (the plan1
  censor cap; pre-registered here). All in-loop wall time (solves,
  reconstructions, merges) counts against B.
- Primary metric: validated-value yield = verified-class node count per
  wall-hour (Arm A deposits nothing unless it completes — reported as
  such, not celebrated alone).
- Substrate metric (the honest one): re-solve the pilot root with the
  accumulated S preloaded vs. fresh TT, equal wall (120 s): node-count
  delta and outcome delta. Gate: >= 2x node reduction (or an outcome
  change) -> campaign prototype GO; < 1x -> the mechanism is measured
  empty at pilot scale; between: judgment call.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import sstore  # noqa: E402
import ssfp  # noqa: E402

ROOT_FEN = "rnbqkbnr/ppp1pppp/8/3p4/3P4/8/PPP1PPPP/RNBQKBNR w KQkq d6 0 2"
BUDGET_SECS = 7200
PER_TARGET_TIMEOUT = 120
SUBSTRATE_TIMEOUT = 120
FRONTIER_COUNT = 4096

ARM_A_SNAP = os.path.join(ssfp.SNAPS, "pilot_armA.tt")
ARM_A_PT = os.path.join(ssfp.ROOT, "armA_reconstructed.bin")
STORE = os.path.join(ssfp.STORE, "s.bin")
EXPORT = os.path.join(ssfp.STORE, "s_export.tt")


def solve(fen, timeout, snap, extra=None, log_path=None):
    """One solver run; returns the parsed record (ssfp.solve semantics)."""
    rec = ssfp.solve(fen, timeout, tt_dump=snap, extra=extra, log_path=log_path)
    return rec


def run_reconstruct(snap, out_pt, log_path):
    r = ssfp.run(
        [ssfp.RECONSTRUCT, "--snapshot", snap, "--out", out_pt], timeout=7200
    )
    with open(log_path, "w") as f:
        f.write(r.stdout + "\n## stderr\n" + r.stderr)
    validate = None
    outcome = None
    for line in r.stdout.splitlines():
        if line.startswith("validate: "):
            validate = line[10:].strip()
        elif line.startswith("outcome: "):
            outcome = line[9:].strip()
    return r.returncode, validate, outcome


def merge_store(snapshot=None, tree=None, label=""):
    cmd = [sys.executable, os.path.join(HERE, "sstore.py"), "merge",
           "--store", STORE, "--label", label]
    if snapshot:
        cmd += ["--snapshot", snapshot]
    if tree:
        cmd += ["--tree", tree]
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(f"sstore merge failed: {r.stderr}")
    return json.loads(r.stdout)


def export_store():
    r = subprocess.run(
        [sys.executable, os.path.join(HERE, "sstore.py"), "export",
         "--store", STORE, "--out", EXPORT, "--root-fen", ROOT_FEN],
        capture_output=True, text=True,
    )
    if r.returncode != 0:
        raise RuntimeError(f"sstore export failed: {r.stderr}")
    return int(r.stdout.split()[1])


# --------------------------------- Arm A ------------------------------------


def cmd_arm_a(_args):
    rec = solve(
        ROOT_FEN,
        BUDGET_SECS,
        ARM_A_SNAP,
        log_path=os.path.join(ssfp.LOGS, "pilot_armA.log"),
    )
    rec["fen"] = ROOT_FEN
    rec["budget_secs"] = BUDGET_SECS
    rec["verified_records"] = 0
    rec["validate"] = None
    if not rec["censored"] and rec.get("outcome") in ("win", "loss"):
        rc, validate, outcome = run_reconstruct(
            ARM_A_SNAP, ARM_A_PT, os.path.join(ssfp.LOGS, "pilot_armA_reconstruct.log")
        )
        rec["reconstruct_rc"] = rc
        rec["validate"] = validate
        rec["reconstruct_outcome"] = outcome
        if validate == "ok" and outcome == rec["outcome"]:
            rec["verified_records"] = _tree_node_count(ARM_A_PT)
    with open(os.path.join(ssfp.STATE, "pilot_armA.json"), "w") as f:
        json.dump(rec, f, indent=1)
    print(json.dumps(rec, indent=1))


def _tree_node_count(tree_path):
    return len(ssfp.read_proof_tree_keys(tree_path))


# --------------------------------- Arm B ------------------------------------


def _target_cost(t):
    """Pre-registered expected-cost order: min(pn, dn) ascending, work as
    tiebreak; the root target (pn/dn unknown) goes first."""
    if t.get("root"):
        return (-1, 0)
    return (min(t["pn"], t["dn"]), t["work"])


def cmd_arm_b(args):
    budget = args.budget
    # Bootstrap: an empty store + its export, so iteration 1's --tt-load-path
    # (strict loader) has a file to read.
    if not os.path.exists(STORE):
        sstore.SStore(label="pilot armB bootstrap").save(STORE)
    export_store()
    queue = [{"root": True, "fen": ROOT_FEN, "pn": None, "dn": None, "work": 0}]
    seen_keys = set()
    iterations = []
    t_start = time.monotonic()
    it = 0
    while time.monotonic() - t_start < budget and queue:
        remaining = budget - (time.monotonic() - t_start)
        if remaining < 5:
            break
        queue.sort(key=_target_cost)
        target = queue.pop(0)
        it += 1
        timeout = min(PER_TARGET_TIMEOUT, int(remaining))
        tag = f"armB_it{it:03d}"
        snap = os.path.join(ssfp.SNAPS, f"{tag}.tt")
        frontier = os.path.join(ssfp.LOGS, f"{tag}_frontier.txt")
        log = os.path.join(ssfp.LOGS, f"{tag}.log")

        rec = solve(
            target["fen"],
            timeout,
            snap,
            extra=[
                "--tt-load-path", EXPORT,
                "--frontier-dump", frontier,
                "--frontier-count", str(FRONTIER_COUNT),
            ],
            log_path=log,
        )
        rec["iteration"] = it
        rec["target"] = {k: v for k, v in target.items() if k != "root"}
        rec["timeout_used"] = timeout
        wall = time.monotonic() - t_start

        deposited_verified = 0
        new_targets = 0
        if not rec["censored"] and rec.get("outcome") in ("win", "loss"):
            pt = os.path.join(ssfp.ROOT, f"{tag}.bin")
            rc, validate, outcome = run_reconstruct(
                snap, pt, os.path.join(ssfp.LOGS, f"{tag}_reconstruct.log")
            )
            rec["reconstruct_rc"] = rc
            rec["validate"] = validate
            rec["reconstruct_outcome"] = outcome
            if validate == "ok" and outcome == rec["outcome"]:
                stats = merge_store(snapshot=snap, tree=pt, label=tag)
                deposited_verified = stats["verified"]
            else:
                stats = merge_store(snapshot=snap, label=tag)
        else:
            stats = merge_store(snapshot=snap, label=tag)
            if os.path.exists(frontier):
                seen_before = len(seen_keys)
                with open(frontier) as f:
                    for line in f:
                        parts = line.split(" ", 4)
                        if len(parts) != 5:
                            continue
                        key, pn, dn, work, fen = (
                            int(parts[0]), int(parts[1]), int(parts[2]),
                            int(parts[3]), parts[4].strip(),
                        )
                        if key in seen_keys:
                            continue
                        seen_keys.add(key)
                        queue.append(
                            {"fen": fen, "key": key, "pn": pn, "dn": dn, "work": work}
                        )
                new_targets = len(seen_keys) - seen_before

        exported = export_store()
        iterations.append(
            {
                "iteration": it,
                "target_fen": target["fen"],
                "censored": rec["censored"],
                "outcome": rec["outcome"],
                "nodes": rec["nodes"],
                "wall_cumulative": round(wall, 1),
                "validate": rec.get("validate"),
                "deposited_verified_total": deposited_verified,
                "new_queue_targets": new_targets,
                "queue_depth": len(queue),
                "store_total": stats["total"],
                "store_verified": stats["verified"],
                "store_provisional": stats["provisional"],
                "exported_records": exported,
            }
        )
        print(json.dumps(iterations[-1]), flush=True)
        with open(os.path.join(ssfp.STATE, "pilot_armB_iterations.json"), "w") as f:
            json.dump(iterations, f, indent=1)

    total_wall = time.monotonic() - t_start
    summary = {
        "budget_secs": budget,
        "wall_used": round(total_wall, 1),
        "iterations": len(iterations),
        "queue_remaining": len(queue),
        "store": sstore.SStore.load(STORE).stats() if os.path.exists(STORE) else None,
        "per_iteration": iterations,
    }
    with open(os.path.join(ssfp.STATE, "pilot_armB.json"), "w") as f:
        json.dump(summary, f, indent=1)
    print(json.dumps({k: v for k, v in summary.items() if k != "per_iteration"}, indent=1))


# ------------------------------- substrate ----------------------------------


def cmd_substrate(_args):
    results = {}
    for tag, extra in (
        ("fresh", None),
        ("loaded", ["--tt-load-path", EXPORT]),
    ):
        snap = os.path.join(ssfp.SNAPS, f"substrate_{tag}.tt")
        rec = solve(
            ROOT_FEN,
            SUBSTRATE_TIMEOUT,
            snap,
            extra=extra,
            log_path=os.path.join(ssfp.LOGS, f"substrate_{tag}.log"),
        )
        results[tag] = rec
    fresh, loaded = results["fresh"], results["loaded"]
    if fresh["nodes"] and loaded["nodes"]:
        ratio = fresh["nodes"] / max(1, loaded["nodes"])
    else:
        ratio = None
    out = {
        "root": ROOT_FEN,
        "timeout": SUBSTRATE_TIMEOUT,
        "fresh": {k: fresh[k] for k in ("outcome", "nodes", "wall", "exit_reason")},
        "loaded": {k: loaded[k] for k in ("outcome", "nodes", "wall", "exit_reason")},
        "node_reduction_ratio": ratio,
        "outcome_changed": fresh["outcome"] != loaded["outcome"],
        "gate": "GO >= 2x (or outcome change); < 1x rethink; between: judgment",
    }
    with open(os.path.join(ssfp.STATE, "substrate.json"), "w") as f:
        json.dump(out, f, indent=1)
    print(json.dumps(out, indent=1))


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("armA").set_defaults(fn=cmd_arm_a)
    p = sub.add_parser("armB")
    p.add_argument("--budget", type=int, default=BUDGET_SECS)
    p.set_defaults(fn=cmd_arm_b)
    sub.add_parser("substrate").set_defaults(fn=cmd_substrate)
    args = ap.parse_args()
    ssfp.ensure_dirs()
    args.fn(args)


if __name__ == "__main__":
    main()
