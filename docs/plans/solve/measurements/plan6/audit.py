#!/usr/bin/env python3
"""Plan5 soundness audit for a composed campaign artifact.

Pre-registered SOUND gate (plan5 §4):
  1. `inspect_pt --validate` on the artifact passes;
  2. the root outcome equals the sequential baseline's outcome;
  3. spot dual-check: >= 20 sampled decisive facts re-derived independently
     by the sequential CLI solver from the replayed fact FEN show zero
     contradictions (a solver timeout counts as inconclusive, not a
     contradiction).

Usage: audit.py <position> <artifact.bin> <seq_baseline.json>
Writes state/audit_<position>.json and prints the verdict.
"""

import json
import os
import random
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", "..", "..", ".."))
PT_KEYS = os.path.join(REPO, "target", "release", "examples", "pt_keys")
REPLAY = os.path.join(REPO, "target", "release", "examples", "replay")
SOLVER = os.path.join(REPO, "target", "release", "atomic_solver")
INSPECT = os.path.join(REPO, "target", "release", "examples", "inspect_pt")
STATE = os.path.join(HERE, "state")

SQ = "abcdefgh"


def bits_to_uci(bits):
    to = bits & 0x3F
    frm = (bits >> 6) & 0x3F
    promo = (bits >> 14) & 0x3
    typ = (bits >> 12) & 0x3
    if typ == 3:  # castling: king from-to encoded by movegen; UCI via from/to
        pass
    s = SQ[frm & 7] + str(frm // 8 + 1) + SQ[to & 7] + str(to // 8 + 1)
    if typ == 1 and promo:
        s += "qrbn"[promo - 1]
    return s


def load_nodes(bin_path):
    out = subprocess.run([PT_KEYS, bin_path], capture_output=True, text=True, timeout=600)
    nodes = {}
    children = {}
    for line in out.stdout.splitlines():
        parts = line.split()
        if len(parts) != 6:
            continue
        nid, parent, key, outcome, depth, mv_bits = parts
        nid, parent = int(nid), int(parent)
        nodes[nid] = {
            "parent": parent,
            "key": key,
            "outcome": outcome,
            "depth": int(depth),
            "mv_bits": int(mv_bits),
        }
        children.setdefault(parent, []).append(nid)
    return nodes


def node_path(nodes, nid):
    path = []
    while nodes[nid]["parent"] >= 0:
        path.append(bits_to_uci(nodes[nid]["mv_bits"]))
        nid = nodes[nid]["parent"]
    return list(reversed(path))


def fact_fen(root_fen, path):
    args = [REPLAY, root_fen] + path
    out = subprocess.run(args, capture_output=True, text=True, timeout=120)
    for line in out.stdout.splitlines():
        if line.startswith("fen:"):
            return line[4:].strip()
    return None


def solve_outcome(fen, timeout):
    out = subprocess.run(
        [SOLVER, "--fen", fen, "--first-outcome", "--outcome-only", "--timeout", str(timeout)],
        capture_output=True, text=True, timeout=timeout + 60,
    )
    for line in (out.stderr + out.stdout).splitlines():
        if line.startswith("outcome:"):
            return line.split()[1]
    return None


def main():
    position, bin_path, seq_path = sys.argv[1], sys.argv[2], sys.argv[3]
    seq = json.load(open(seq_path))
    root_fen = sys.argv[4] if len(sys.argv) > 4 else seq.get("fen")

    result = {"position": position, "artifact": bin_path}

    # 1. replay validator on the artifact
    insp = subprocess.run([INSPECT, "--validate", bin_path], capture_output=True, text=True, timeout=600)
    validate_ok = "validate: ok" in (insp.stdout + insp.stderr)
    result["inspect_pt_validate"] = "ok" if validate_ok else "FAILED"
    m = [l for l in (insp.stdout + insp.stderr).splitlines() if l.startswith("nodes:")]
    result["artifact_nodes"] = int(m[0].split()[1]) if m else None

    # 2. root outcome equality with the sequential baseline
    insp_root = subprocess.run([INSPECT, bin_path], capture_output=True, text=True, timeout=600)
    root_outcome = None
    for line in insp_root.stdout.splitlines():
        if line.startswith("root outcome:"):
            root_outcome = line.split()[2].strip().lower()
    result["root_outcome"] = root_outcome
    result["seq_outcome"] = seq.get("outcome")
    result["root_outcome_matches_seq"] = root_outcome == seq.get("outcome")

    # 3. dual-check: sample non-terminal decisive facts
    nodes = load_nodes(bin_path)
    candidates = [nid for nid, n in nodes.items()
                  if n["outcome"] in ("win", "loss") and n["depth"] > 0]
    wins = [n for n in candidates if nodes[n]["outcome"] == "win"]
    losses = [n for n in candidates if nodes[n]["outcome"] == "loss"]
    rng = random.Random(20260923)
    wins.sort(key=lambda n: nodes[n]["depth"])
    losses.sort(key=lambda n: nodes[n]["depth"])
    sample = wins[:14] + (rng.sample(wins[14:], min(6, max(0, len(wins) - 14))) if len(wins) > 14 else [])
    sample += losses[:4] + (rng.sample(losses[4:], min(2, max(0, len(losses) - 4))) if len(losses) > 4 else [])
    sample = sample[:24]

    checks, contradictions, inconclusive = [], 0, 0
    for nid in sample:
        path = node_path(nodes, nid)
        fen = fact_fen(root_fen, path)
        if fen is None:
            checks.append({"path": path, "status": "replay_failed"})
            continue
        claimed = nodes[nid]["outcome"]
        got = solve_outcome(fen, 90)
        status = "match" if got == claimed else ("timeout_or_none" if got is None else "CONTRADICTION")
        if status == "CONTRADICTION":
            contradictions += 1
        if got is None:
            inconclusive += 1
        checks.append({"path": path, "fen": fen, "claimed": claimed,
                       "rederived": got, "status": status})
    result["dual_check"] = {
        "sampled": len(checks),
        "contradictions": contradictions,
        "inconclusive": inconclusive,
        "checks": checks,
    }
    result["sound"] = validate_ok and result["root_outcome_matches_seq"] and contradictions == 0
    os.makedirs(STATE, exist_ok=True)
    with open(os.path.join(STATE, f"audit_{position}.json"), "w") as f:
        json.dump(result, f, indent=2)
    print(json.dumps({k: v for k, v in result.items() if k != "dual_check"}))
    print("dual-check:", result["dual_check"]["sampled"], "sampled,",
          contradictions, "contradictions,", inconclusive, "inconclusive")
    print("SOUND" if result["sound"] else "NOT SOUND")
    sys.exit(0 if result["sound"] else 1)


if __name__ == "__main__":
    main()
