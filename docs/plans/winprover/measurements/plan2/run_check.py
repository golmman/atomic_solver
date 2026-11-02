#!/usr/bin/env python3
"""Check our root-Win proof trees with the pinned, independent provari checker.

For every `*.bin` shard in the sample directory:

1. read the `ATOMTREE` v1 header (magic, version, root FEN, root outcome,
   root depth) and the node count from the record payload;
2. skip root-`Loss` / root-`Draw` trees (out of scope: the JSON-lines reader
   assumes the side to move is the attacker);
3. export every root-`Win` tree to provari's JSON-lines format with
   `target/release/examples/provari_export --out <tmp>/<tag>.jsonl` (the
   exporter validates and replays first);
4. run `provari check <file> --seconds N` once per exported file and classify
   the verdict (`accepted`, `convention-repeat`, `unchecked`, `export-bug`,
   `suspect`).

Writes `sample_manifest.json` (one row per `.bin`, with sha256), `results.json`
(per-tree verdicts + per-class counts) and `results.csv`. Exported JSONL files
live in a `TemporaryDirectory` and are never committed; no raw transcripts are
written. Run from the repository root.
"""

import argparse
import csv
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path

sys.dont_write_bytecode = True

MAGIC = b"ATOMTREE"
VERSION = 1

ROOT_OUTCOMES = {0: "draw", 1: "win", 2: "loss"}

# Classes that are sample dispositions, not checker verdicts.
SAMPLE_CLASSES = ("not-a-tree", "root-loss-skipped", "root-draw-skipped")
VERDICT_CLASSES = ("accepted", "convention-repeat", "unchecked", "export-bug", "suspect")

# provari error fragments that implicate the exporter (bad line/FEN/move it
# produced), per the plan's class table.
EXPORT_BUG_PATTERNS = (
    "a bad fen",
    "a bad move",
    "is not legal in",
    "a bad line",
    "a line without a position",
    "a bad header",
    "an empty proof",
    "no root",
)


def parse_header(data):
    """Parse the `ATOMTREE` v1 header, or return None if `data` is not one."""
    if len(data) < 8 + 1 + 1 + 4 or data[:8] != MAGIC:
        return None
    if data[8] != VERSION:
        return None
    end = data.find(b"\n", 9)
    if end < 0:
        return None
    header_end = end + 1 + 1 + 4
    if header_end > len(data):
        return None
    try:
        fen = data[9:end].decode("utf-8")
    except UnicodeDecodeError:
        return None
    outcome_byte = data[end + 1]
    if outcome_byte not in ROOT_OUTCOMES:
        return None
    root_depth = int.from_bytes(data[end + 2 : header_end], "little")
    payload = len(data) - header_end
    if payload % 6 != 0 or payload == 0:
        return None
    return {
        "root_fen": fen,
        "root_outcome": ROOT_OUTCOMES[outcome_byte],
        "root_depth": root_depth,
        "node_count": payload // 6,
    }


def classify_verdict(line):
    """Classify one checked proof: (class, valid, won_positions, counter, error)."""
    valid = line.get("valid")
    error = line.get("error", "")
    if valid is True:
        return (
            "accepted",
            True,
            line.get("won_positions"),
            line.get("highest_counter"),
            "",
        )
    if valid is None:
        return "unchecked", None, None, None, error
    if valid is False:
        lowered = error.lower()
        if "can repeat the position" in lowered:
            return "convention-repeat", False, None, None, error
        if any(pattern in lowered for pattern in EXPORT_BUG_PATTERNS):
            return "export-bug", False, None, None, error
        return "suspect", False, None, None, error
    # No usable verdict line.
    detail = (proc.stdout or "").strip() or (proc.stderr or "").strip()
    return "suspect", None, None, None, f"unparseable verdict: {detail[:400]}"


def check_with_provari(provari, jsonl_path, seconds):
    """Run provari check on one exported file and return its verdict line."""
    proc = subprocess.run(
        [provari, "check", str(jsonl_path), "--seconds", str(seconds)],
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode not in (0, 1):
        detail = (proc.stderr or "").strip() or (proc.stdout or "").strip()
        return {
            "valid": None,
            "error": f"provari exited {proc.returncode}: {detail[:400]}",
        }
    for line in (proc.stdout or "").splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            parsed = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(parsed, dict) and "valid" in parsed:
            return parsed
    return {"valid": "unparseable"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--shard-dir", default="docs/plans/proofdb/shards")
    parser.add_argument(
        "--exporter", default="target/release/examples/provari_export"
    )
    parser.add_argument(
        "--provari", default="/tmp/winprover-oracle/target/release/provari"
    )
    parser.add_argument("--out-dir", default="docs/plans/winprover/measurements/plan2")
    parser.add_argument("--seconds", type=float, default=60)
    parser.add_argument("--limit", type=int, default=None)
    args = parser.parse_args()

    shard_dir = Path(args.shard_dir)
    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    shards = sorted(shard_dir.glob("*.bin"))
    if args.limit is not None:
        shards = shards[: args.limit]
    if not shards:
        raise SystemExit(f"no .bin shards under {shard_dir}")

    manifest_rows = []
    tree_rows = []

    with tempfile.TemporaryDirectory(prefix="provari-export-") as tmp:
        tmp_dir = Path(tmp)
        for shard in shards:
            data = shard.read_bytes()
            row = {
                "file": shard.name,
                "sha256": hashlib.sha256(data).hexdigest(),
                "size": len(data),
                "root_outcome": None,
                "root_depth": None,
                "node_count": None,
                "root_fen": None,
            }
            header = parse_header(data)
            if header is not None:
                row.update(header)
            manifest_rows.append(row)

            result = {
                "file": shard.name,
                "tag": shard.stem,
                "root_outcome": header["root_outcome"] if header else None,
                "nodes": header["node_count"] if header else None,
                "export_lines": None,
                "class": None,
                "valid": None,
                "won_positions": None,
                "highest_counter": None,
                "error": "",
            }

            if header is None:
                result["class"] = "not-a-tree"
                result["error"] = "no ATOMTREE v1 header"
                tree_rows.append(result)
                continue
            outcome = header["root_outcome"]
            if outcome == "loss":
                result["class"] = "root-loss-skipped"
                tree_rows.append(result)
                continue
            if outcome == "draw":
                result["class"] = "root-draw-skipped"
                tree_rows.append(result)
                continue

            jsonl_path = tmp_dir / f"{shard.stem}.jsonl"
            export = subprocess.run(
                [args.exporter, "--out", str(jsonl_path), str(shard)],
                capture_output=True,
                text=True,
                check=False,
            )
            if export.returncode != 0:
                detail = (export.stderr or "").strip() or (export.stdout or "").strip()
                lowered = detail.lower()
                # Our own validator rejecting a committed shard is a `suspect`
                # finding, not an exporter bug; a malformed tree / parse error
                # is the export path's problem.
                if "invalid proof tree" in lowered:
                    result["class"] = "suspect"
                elif "export failed" in lowered:
                    result["class"] = "export-bug"
                else:
                    result["class"] = "suspect"
                result["error"] = detail[:400]
                tree_rows.append(result)
                continue

            summary = (export.stderr or "").strip()
            positions = None
            # `positions: N, nodes: M, root: win`
            for part in summary.split(","):
                key, _, value = part.strip().partition(":")
                if key.strip() == "positions":
                    positions = int(value.strip())
            result["export_lines"] = positions

            verdict = check_with_provari(args.provari, jsonl_path, args.seconds)
            if verdict.get("valid") == "unparseable":
                result["class"] = "suspect"
                result["error"] = "no JSON verdict line from provari"
                tree_rows.append(result)
                continue
            klass, valid, won, counter, error = classify_verdict(verdict)
            result["class"] = klass
            result["valid"] = valid
            result["won_positions"] = won
            result["highest_counter"] = counter
            result["error"] = error
            tree_rows.append(result)

    class_counts = {}
    for row in tree_rows:
        class_counts[row["class"]] = class_counts.get(row["class"], 0) + 1

    scanned = len(manifest_rows)
    not_a_tree = sum(1 for row in manifest_rows if row["root_outcome"] is None)
    root_loss = sum(1 for row in manifest_rows if row["root_outcome"] == "loss")
    root_draw = sum(1 for row in manifest_rows if row["root_outcome"] == "draw")
    root_win = sum(1 for row in manifest_rows if row["root_outcome"] == "win")
    exported = sum(1 for row in tree_rows if row["export_lines"] is not None)
    export_errors = sum(
        1 for row in tree_rows if row["class"] in ("export-bug", "suspect") and row["export_lines"] is None
    )

    manifest = {
        "shard_dir": str(shard_dir),
        "exporter": args.exporter,
        "provari": args.provari,
        "seconds": args.seconds,
        "files": manifest_rows,
    }
    results = {
        "shard_dir": str(shard_dir),
        "exporter": args.exporter,
        "provari": args.provari,
        "seconds": args.seconds,
        "sample": {
            "scanned": scanned,
            "not_a_tree": not_a_tree,
            "root_loss_skipped": root_loss,
            "root_draw_skipped": root_draw,
            "root_win": root_win,
            "root_win_exported": exported,
            "export_errors": export_errors,
        },
        "classes": dict(sorted(class_counts.items())),
        "trees": tree_rows,
    }

    (out_dir / "sample_manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (out_dir / "results.json").write_text(json.dumps(results, indent=2) + "\n")

    with (out_dir / "results.csv").open("w", newline="") as handle:
        writer = csv.writer(handle)
        writer.writerow(
            [
                "file",
                "tag",
                "root_outcome",
                "nodes",
                "export_lines",
                "class",
                "valid",
                "won_positions",
                "highest_counter",
                "error",
            ]
        )
        for row in tree_rows:
            writer.writerow(
                [
                    row["file"],
                    row["tag"],
                    row["root_outcome"],
                    row["nodes"],
                    row["export_lines"],
                    row["class"],
                    row["valid"],
                    row["won_positions"],
                    row["highest_counter"],
                    row["error"],
                ]
            )

    print(json.dumps(results["sample"]))
    print(json.dumps(results["classes"], sort_keys=True))


if __name__ == "__main__":
    main()
