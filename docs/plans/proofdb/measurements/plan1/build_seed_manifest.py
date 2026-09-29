#!/usr/bin/env python3
"""Build the conforming seed manifest (`manifest_seed.json`) for the proofdb
merger from the plan4 artifact (`solve` initiative).

The plan4 `index.json` is NOT conforming to the `global_proof_store.md`
manifest contract for 40 of its 95 entries:

- 55 `p2` entries carry `moves` = the full startpos path (conforming).
- 20 `sib_*` entries carry `moves` = the single move *relative to their
  sibling root* (`e4e5_p2` = after 1.e4 e5; `d4d5_p32` = the plan1 ladder
  `d4d5` position at ply 32).
- 20 `ladder` entries carry `moves = null`; their startpos path is only
  implicitly recorded in plan1's `ladder_<line>.json` state files, where each
  position's `moves_played` are the two moves leading to the *next* position
  and the line name implies the opening prefix.

This driver reconstructs the missing startpos paths and verifies every entry
by replaying the path from the startpos with the release `replay` binary and
requiring an exact FEN match with the entry's `fen` (the merger re-checks this
by Zobrist hash). Output: one conforming entry per shard, extra fields
(tier, dtm_length, tree_nodes, wall) preserved for provenance.

Usage: python3 build_seed_manifest.py [--verify/--no-verify]
Writes manifest_seed.json next to this script.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", "..", "..", ".."))
PLAN4_ARTIFACT = os.path.join(
    REPO, "docs", "plans", "solve", "measurements", "plan4", "artifact"
)
PLAN1_STATE = os.path.join(
    REPO, "docs", "plans", "solve", "measurements", "plan1", "state"
)
REPLAY = os.path.join(REPO, "target", "release", "examples", "replay")
STARTPOS = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"

# Line name -> opening prefix implied by the ladder line's first position.
LINE_PREFIX = {
    "d4": ["d2d4"],
    "d4d5": ["d2d4", "d7d5"],
    "e4": ["e2e4"],
    "e4c5": ["e2e4", "c7c5"],
    "e4e5": ["e2e4", "e7e5"],
    "nf3": ["g1f3"],
    "nf3d5": ["g1f3", "d7d5"],
    "startpos": [],
}

LADDER_CACHE: dict[str, tuple[list, list]] = {}


def ladder_path(line: str, ply: int) -> list[str]:
    """Path to the ladder `<line>` position at `ply`: opening prefix plus the
    `moves_played` of every position before `ply`."""
    if line not in LADDER_CACHE:
        with open(os.path.join(PLAN1_STATE, f"ladder_{line}.json")) as f:
            d = json.load(f)
        LADDER_CACHE[line] = (d["positions"], {p["ply"]: p for p in d["positions"]})
    positions, by_ply = LADDER_CACHE[line]
    if ply not in by_ply:
        raise SystemExit(f"ladder {line} has no ply {ply} position")
    path = list(LINE_PREFIX[line])
    for p in positions:
        if p["ply"] >= ply:
            break
        path.extend(p["moves_played"])
    return path


def replay_fen(uci_line: list[str]) -> str | None:
    r = subprocess.run(
        [REPLAY, STARTPOS] + list(uci_line),
        capture_output=True, text=True, stdin=subprocess.DEVNULL, timeout=60,
    )
    if r.returncode != 0:
        return None
    for line in r.stdout.splitlines():
        if line.startswith("fen: "):
            return line[5:].strip()
    return None


def startpos_path(entry: dict) -> list[str]:
    tier = entry.get("tier")
    if tier == "p2":
        return list(entry["moves"])
    if tier.startswith("sib_"):
        root = tier[len("sib_"):]
        if root == "e4e5_p2":
            prefix = ["e2e4", "e7e5"]
        elif root == "d4d5_p32":
            prefix = ladder_path("d4d5", 32)
        else:
            raise SystemExit(f"unknown sibling root {root}")
        return prefix + list(entry["moves"])
    if tier == "ladder":
        stem = entry["tag"][len("lad_"):]
        line, _, ply = stem.rpartition("_p")
        return ladder_path(line, int(ply))
    raise SystemExit(f"unknown tier {tier} for {entry['tag']}")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--verify", dest="verify", action="store_true", default=True)
    ap.add_argument("--no-verify", dest="verify", action="store_false")
    args = ap.parse_args()

    with open(os.path.join(PLAN4_ARTIFACT, "index.json")) as f:
        idx = json.load(f)
    entries = []
    problems = []
    for e in idx["entries"]:
        if e["validate"] != "ok":
            problems.append((e["tag"], "validate != ok"))
            continue
        path = startpos_path(e)
        out = {
            "tag": e["tag"],
            "file": f"{e['tag']}.bin",
            "fen": e["fen"],
            "moves": path,
            "outcome": e["outcome"],
            "validate": "ok",
            # provenance extras (ignored by the schema contract)
            "tier": e.get("tier"),
            "dtm_length": e.get("dtm_length"),
            "tree_nodes": e.get("tree_nodes"),
            "path_source": (
                "sibling-root+manifest" if str(e.get("tier", "")).startswith("sib_")
                else "plan1-ladder-state" if e.get("moves") is None
                else "manifest"
            ),
        }
        if args.verify:
            fen = replay_fen(path)
            if fen is None or " ".join(fen.split()[:5]) != " ".join(e["fen"].split()[:5]):
                problems.append((e["tag"], f"replay {fen!r} != manifest {e['fen']!r}"))
                continue
            out["replay_fen"] = fen
        entries.append(out)

    if problems:
        for tag, why in problems:
            print(f"PROBLEM {tag}: {why}", file=sys.stderr)
        raise SystemExit(f"{len(problems)} entries failed path reconstruction")

    out_path = os.path.join(HERE, "manifest_seed.json")
    summary = {
        "n_entries": len(entries),
        "path_sources": {
            src: sum(1 for e in entries if e["path_source"] == src)
            # sorted(): set iteration order is not deterministic across runs,
            # and the manifest bytes feed the DB's built_from digest.
            for src in sorted({e["path_source"] for e in entries})
        },
        "verified_by_replay": args.verify,
    }
    with open(out_path, "w") as f:
        json.dump({"entries": entries, "summary": summary}, f, indent=1)
    print(json.dumps(summary, indent=1))
    print(f"wrote {out_path}")


if __name__ == "__main__":
    main()
